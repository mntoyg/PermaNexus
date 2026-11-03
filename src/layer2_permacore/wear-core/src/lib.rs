#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

//! Allocator-free wear-budget selection for caller-owned storage.
//!
//! This crate models configured write attempts. It does not access hardware,
//! persist state, or measure physical endurance. Embedded callers provide a
//! fixed-capacity snapshot buffer; persistence remains the caller's job.

#[cfg(test)]
extern crate std;

use core::cmp::Ordering;
use core::fmt;

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

impl Default for SlotSnapshot {
    fn default() -> Self {
        Self {
            id: 0,
            write_limit: 0,
            charged_writes: 0,
            health: SlotHealth::Retired,
        }
    }
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

#[cfg(feature = "std")]
impl std::error::Error for WearError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Committed,
    /// Only use when authoritative evidence proves that no write occurred.
    NotWritten,
    /// Timeouts, cancellations after dispatch, and uncertain transport errors.
    Unknown,
}

/// Owns a caller-provided buffer for a bounded, allocation-free planner.
#[derive(Debug)]
pub struct StaticWearLeveler<'storage> {
    slots: &'storage mut [SlotSnapshot],
    len: usize,
}

impl<'storage> StaticWearLeveler<'storage> {
    /// Validates the full configuration before mutating the supplied buffer.
    pub fn new(
        configuration: &[SlotConfig],
        storage: &'storage mut [SlotSnapshot],
    ) -> Result<Self, WearError> {
        if configuration.is_empty()
            || configuration.len() > MAX_SLOTS
            || configuration.len() > storage.len()
        {
            return Err(WearError::InvalidSlotCount);
        }

        for (index, slot) in configuration.iter().enumerate() {
            if slot.write_limit == 0 || slot.charged_writes > slot.write_limit {
                return Err(WearError::InvalidBudget(slot.id));
            }
            if configuration[..index]
                .iter()
                .any(|previous| previous.id == slot.id)
            {
                return Err(WearError::DuplicateSlot(slot.id));
            }
        }

        let len = configuration.len();
        for (destination, source) in storage[..len].iter_mut().zip(configuration) {
            *destination = SlotSnapshot {
                id: source.id,
                write_limit: source.write_limit,
                charged_writes: source.charged_writes,
                health: SlotHealth::Healthy,
            };
        }
        storage[..len].sort_unstable_by_key(|slot| slot.id);

        Ok(Self {
            slots: &mut storage[..len],
            len,
        })
    }

    /// Returns immutable copies in stable slot-ID order.
    pub fn slots(&self) -> impl ExactSizeIterator<Item = SlotSnapshot> + '_ {
        self.slots[..self.len].iter().copied()
    }

    pub fn remaining_writes(&self) -> u128 {
        remaining_writes(&self.slots[..self.len])
    }

    /// Charges and quarantines a slot before returning its exclusive lease.
    pub fn reserve_write(&mut self) -> Result<WriteLease<'_>, WearError> {
        reserve_write(&mut self.slots[..self.len])
    }

    pub fn retire(&mut self, id: u32) -> Result<(), WearError> {
        retire(&mut self.slots[..self.len], id)
    }
}

pub fn remaining_writes(slots: &[SlotSnapshot]) -> u128 {
    slots
        .iter()
        .filter(|slot| slot.health == SlotHealth::Healthy)
        .map(|slot| u128::from(slot.write_limit.saturating_sub(slot.charged_writes)))
        .sum()
}

pub fn reserve_write(slots: &mut [SlotSnapshot]) -> Result<WriteLease<'_>, WearError> {
    let index = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| {
            slot.health == SlotHealth::Healthy && slot.charged_writes < slot.write_limit
        })
        .min_by(|(_, left), (_, right)| compare_wear(left, right))
        .map(|(index, _)| index)
        .ok_or(WearError::NoEligibleSlot)?;
    let slot = &mut slots[index];
    slot.charged_writes += 1;
    slot.health = SlotHealth::Quarantined;
    Ok(WriteLease { slot })
}

pub fn retire(slots: &mut [SlotSnapshot], id: u32) -> Result<(), WearError> {
    let index = slots
        .binary_search_by_key(&id, |slot| slot.id)
        .map_err(|_| WearError::UnknownSlot(id))?;
    slots[index].health = SlotHealth::Retired;
    Ok(())
}

fn compare_wear(left: &SlotSnapshot, right: &SlotSnapshot) -> Ordering {
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

#[cfg(test)]
mod tests {
    use super::{
        SlotConfig, SlotHealth, SlotSnapshot, StaticWearLeveler, WearError, WriteOutcome, MAX_SLOTS,
    };

    #[test]
    fn initializes_fixed_storage_and_validates_before_mutation() {
        let config = [
            SlotConfig {
                id: 8,
                write_limit: 20,
                charged_writes: 4,
            },
            SlotConfig {
                id: 3,
                write_limit: 10,
                charged_writes: 0,
            },
        ];
        let mut storage = [SlotSnapshot::default(); 3];
        {
            let planner = StaticWearLeveler::new(&config, &mut storage).unwrap();
            let slots: [SlotSnapshot; 2] = planner
                .slots()
                .collect::<std::vec::Vec<_>>()
                .try_into()
                .unwrap();
            assert_eq!(slots[0].id, 3);
            assert_eq!(slots[1].id, 8);
            assert_eq!(planner.remaining_writes(), 26);
        }

        let before = storage;
        let invalid = [SlotConfig {
            id: 1,
            write_limit: 1,
            charged_writes: 2,
        }];
        assert!(matches!(
            StaticWearLeveler::new(&invalid, &mut storage),
            Err(WearError::InvalidBudget(1))
        ));
        assert_eq!(storage, before);
        assert!(matches!(
            StaticWearLeveler::new(&config, &mut storage[..1]),
            Err(WearError::InvalidSlotCount)
        ));
    }

    #[test]
    fn balances_reservations_and_preserves_unknown_outcomes() {
        let config = [
            SlotConfig {
                id: 9,
                write_limit: 10,
                charged_writes: 0,
            },
            SlotConfig {
                id: 2,
                write_limit: 10,
                charged_writes: 0,
            },
        ];
        let mut storage = [SlotSnapshot::default(); 2];
        let mut planner = StaticWearLeveler::new(&config, &mut storage).unwrap();

        let first = planner.reserve_write().unwrap();
        assert_eq!(first.slot_id(), 2);
        first.complete(WriteOutcome::Committed);
        let second = planner.reserve_write().unwrap();
        assert_eq!(second.slot_id(), 9);
        second.complete(WriteOutcome::Unknown);

        let slots: [SlotSnapshot; 2] = planner
            .slots()
            .collect::<std::vec::Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(slots[0].health, SlotHealth::Healthy);
        assert_eq!(slots[1].health, SlotHealth::Quarantined);
        assert_eq!(slots[1].charged_writes, 1);
        assert_eq!(planner.remaining_writes(), 9);
    }

    #[test]
    fn dropped_reservations_stay_charged_and_quarantined() {
        let config = [SlotConfig {
            id: 4,
            write_limit: 1,
            charged_writes: 0,
        }];
        let mut storage = [SlotSnapshot::default(); 1];
        let mut planner = StaticWearLeveler::new(&config, &mut storage).unwrap();
        drop(planner.reserve_write().unwrap());
        assert_eq!(planner.remaining_writes(), 0);
        assert!(matches!(
            planner.reserve_write(),
            Err(WearError::NoEligibleSlot)
        ));
    }

    #[test]
    fn rejects_empty_duplicate_and_oversized_configurations() {
        let mut storage = [SlotSnapshot::default(); 2];
        assert!(matches!(
            StaticWearLeveler::new(&[], &mut storage),
            Err(WearError::InvalidSlotCount)
        ));
        let duplicates = [
            SlotConfig {
                id: 1,
                write_limit: 1,
                charged_writes: 0,
            },
            SlotConfig {
                id: 1,
                write_limit: 2,
                charged_writes: 0,
            },
        ];
        assert!(matches!(
            StaticWearLeveler::new(&duplicates, &mut storage),
            Err(WearError::DuplicateSlot(1))
        ));
        let oversized = [SlotConfig {
            id: 1,
            write_limit: 1,
            charged_writes: 0,
        }; MAX_SLOTS + 1];
        assert!(matches!(
            StaticWearLeveler::new(&oversized, &mut storage),
            Err(WearError::InvalidSlotCount)
        ));
    }

    #[test]
    fn refunds_only_proven_not_written_and_supports_retirement() {
        let config = [SlotConfig {
            id: 5,
            write_limit: u64::MAX,
            charged_writes: 7,
        }];
        let mut storage = [SlotSnapshot::default(); 1];
        let mut planner = StaticWearLeveler::new(&config, &mut storage).unwrap();
        planner
            .reserve_write()
            .unwrap()
            .complete(WriteOutcome::NotWritten);
        assert_eq!(planner.slots().next().unwrap().charged_writes, 7);
        planner.retire(5).unwrap();
        assert_eq!(planner.remaining_writes(), 0);
        assert!(matches!(
            planner.retire(99),
            Err(WearError::UnknownSlot(99))
        ));
    }
}
