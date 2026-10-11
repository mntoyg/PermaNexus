//! Deterministic, in-memory write-budget allocation for simulated storage slots.
//!
//! Counts are a model of write attempts, not measurements of physical wear.
//! Reserving charges one write immediately and quarantines the slot until the
//! outcome is known. Dropped or forgotten leases therefore remain conservative.

use std::cmp::Ordering;
use std::fmt;

pub const MAX_SLOTS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotConfig {
    pub id: u32,
    pub write_limit: u64,
    pub charged_writes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotHealth {
    Healthy,
    Quarantined,
    Retired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotSnapshot {
    pub id: u32,
    pub write_limit: u64,
    pub charged_writes: u64,
    pub health: SlotHealth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WearError {
    InvalidSlotCount,
    DuplicateSlot(u32),
    InvalidBudget(u32),
    UnknownSlot(u32),
    InvalidCompletion(u32),
    NoEligibleSlot,
}

impl fmt::Display for WearError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlotCount => {
                write!(formatter, "Slot count must be between 1 and {MAX_SLOTS}")
            }
            Self::DuplicateSlot(id) => write!(formatter, "Duplicate slot ID: {id}"),
            Self::InvalidBudget(id) => write!(formatter, "Invalid write budget for slot {id}"),
            Self::UnknownSlot(id) => write!(formatter, "Unknown slot ID: {id}"),
            Self::InvalidCompletion(id) => write!(formatter, "No pending write for slot {id}"),
            Self::NoEligibleSlot => {
                formatter.write_str("No healthy slot has remaining write budget")
            }
        }
    }
}

impl std::error::Error for WearError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Committed,
    /// Only use when authoritative evidence proves that no write occurred.
    NotWritten,
    /// Timeouts, cancellations after dispatch, and uncertain transport errors.
    Unknown,
}

#[derive(Debug)]
pub struct WearLeveler {
    slots: Vec<SlotSnapshot>,
}

impl WearLeveler {
    pub fn new(mut configuration: Vec<SlotConfig>) -> Result<Self, WearError> {
        if configuration.is_empty() || configuration.len() > MAX_SLOTS {
            return Err(WearError::InvalidSlotCount);
        }
        configuration.sort_unstable_by_key(|slot| slot.id);
        for (index, slot) in configuration.iter().enumerate() {
            if slot.write_limit == 0 || slot.charged_writes > slot.write_limit {
                return Err(WearError::InvalidBudget(slot.id));
            }
            if index > 0 && configuration[index - 1].id == slot.id {
                return Err(WearError::DuplicateSlot(slot.id));
            }
        }
        let slots = configuration
            .into_iter()
            .map(|slot| SlotSnapshot {
                id: slot.id,
                write_limit: slot.write_limit,
                charged_writes: slot.charged_writes,
                health: SlotHealth::Healthy,
            })
            .collect();
        Ok(Self { slots })
    }

    /// Returns immutable copies; callers cannot bypass budget/health invariants.
    pub fn slots(&self) -> impl ExactSizeIterator<Item = SlotSnapshot> + '_ {
        self.slots.iter().copied()
    }

    /// Uses u128 so the sum of bounded u64 slot budgets cannot overflow.
    pub fn remaining_writes(&self) -> u128 {
        self.slots
            .iter()
            .filter(|slot| slot.health == SlotHealth::Healthy)
            .map(|slot| u128::from(slot.write_limit - slot.charged_writes))
            .sum()
    }

    /// Allocates no memory per reservation; only one lease may borrow this planner.
    pub fn reserve_write(&mut self) -> Result<WriteLease<'_>, WearError> {
        let index = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                slot.health == SlotHealth::Healthy && slot.charged_writes < slot.write_limit
            })
            .min_by(|(_, left), (_, right)| compare_wear(left, right))
            .map(|(index, _)| index)
            .ok_or(WearError::NoEligibleSlot)?;
        let slot = &mut self.slots[index];
        // Eligibility guarantees this increment cannot overflow or exceed the limit.
        slot.charged_writes += 1;
        slot.health = SlotHealth::Quarantined;
        Ok(WriteLease { slot })
    }

    /// Permanently excludes a failed slot without erasing its charged writes.
    pub fn retire(&mut self, id: u32) -> Result<(), WearError> {
        let index = self
            .slots
            .binary_search_by_key(&id, |slot| slot.id)
            .map_err(|_| WearError::UnknownSlot(id))?;
        self.slots[index].health = SlotHealth::Retired;
        Ok(())
    }

    // Only replay of a validated durable completion record may call this method.
    pub(crate) fn replay_completion(
        &mut self,
        id: u32,
        outcome: WriteOutcome,
    ) -> Result<(), WearError> {
        let index = self
            .slots
            .binary_search_by_key(&id, |slot| slot.id)
            .map_err(|_| WearError::UnknownSlot(id))?;
        let slot = &mut self.slots[index];
        if slot.health != SlotHealth::Quarantined || slot.charged_writes == 0 {
            return Err(WearError::InvalidCompletion(id));
        }
        WriteLease { slot }.complete(outcome);
        Ok(())
    }
}

fn compare_wear(left: &SlotSnapshot, right: &SlotSnapshot) -> Ordering {
    // Compare exact utilization ratios without floating-point rounding or u64 overflow.
    let left_score = u128::from(left.charged_writes) * u128::from(right.write_limit);
    let right_score = u128::from(right.charged_writes) * u128::from(left.write_limit);
    left_score
        .cmp(&right_score)
        .then_with(|| left.id.cmp(&right.id))
}

#[derive(Debug)]
#[must_use = "An unresolved lease keeps its slot quarantined and its write charged"]
pub struct WriteLease<'a> {
    slot: &'a mut SlotSnapshot,
}

impl WriteLease<'_> {
    pub fn slot_id(&self) -> u32 {
        self.slot.id
    }

    /// Consumes the lease, preventing completion/replay of the same reservation.
    pub fn complete(self, outcome: WriteOutcome) {
        match outcome {
            WriteOutcome::Committed => self.slot.health = SlotHealth::Healthy,
            WriteOutcome::NotWritten => {
                self.slot.charged_writes -= 1;
                self.slot.health = SlotHealth::Healthy;
            }
            WriteOutcome::Unknown => {}
        }
    }
}
