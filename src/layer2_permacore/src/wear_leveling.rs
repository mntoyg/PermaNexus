//! Dynamic host-side adapter for the allocator-free wear planner core.
//!
//! The Linux journal uses this owned-`Vec` wrapper. Embedded callers can use
//! `permanexus_wear_core::StaticWearLeveler` with their own fixed storage.

use std::vec::Vec;

pub use permanexus_wear_core::{
    SlotConfig, SlotHealth, SlotSnapshot, WearError, WriteLease, WriteOutcome,
};
pub const MAX_SLOTS: usize = permanexus_wear_core::MAX_SLOTS;

#[derive(Debug)]
pub struct WearLeveler {
    slots: Vec<SlotSnapshot>,
}

impl WearLeveler {
    pub fn new(configuration: Vec<SlotConfig>) -> Result<Self, WearError> {
        if configuration.is_empty() || configuration.len() > MAX_SLOTS {
            return Err(WearError::InvalidSlotCount);
        }

        let mut slots = vec![SlotSnapshot::default(); configuration.len()];
        let initialized = permanexus_wear_core::StaticWearLeveler::new(&configuration, &mut slots)?;
        drop(initialized);
        Ok(Self { slots })
    }

    /// Returns immutable copies; callers cannot bypass budget/health invariants.
    pub fn slots(&self) -> impl ExactSizeIterator<Item = SlotSnapshot> + '_ {
        self.slots.iter().copied()
    }

    pub fn remaining_writes(&self) -> u128 {
        permanexus_wear_core::remaining_writes(&self.slots)
    }

    /// Allocates no memory per reservation; only one lease may borrow this planner.
    pub fn reserve_write(&mut self) -> Result<WriteLease<'_>, WearError> {
        permanexus_wear_core::reserve_write(&mut self.slots)
    }

    /// Permanently excludes a failed slot without erasing its charged writes.
    pub fn retire(&mut self, id: u32) -> Result<(), WearError> {
        permanexus_wear_core::retire(&mut self.slots, id)
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
        match outcome {
            WriteOutcome::Committed => slot.health = SlotHealth::Healthy,
            WriteOutcome::NotWritten => {
                slot.charged_writes -= 1;
                slot.health = SlotHealth::Healthy;
            }
            WriteOutcome::Unknown => {}
        }
        Ok(())
    }
}
