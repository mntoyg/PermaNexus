use permanexus_permacore::wear_leveling::{
    SlotConfig, SlotHealth, SlotSnapshot, WearError, WearLeveler, WriteOutcome, MAX_SLOTS,
};

fn slot(id: u32, write_limit: u64, charged_writes: u64) -> SlotConfig {
    SlotConfig {
        id,
        write_limit,
        charged_writes,
    }
}

fn snapshot(planner: &WearLeveler) -> Vec<SlotSnapshot> {
    planner.slots().collect()
}

#[test]
fn rejects_empty_duplicate_and_invalid_budgets() {
    assert_eq!(
        WearLeveler::new(vec![]).unwrap_err(),
        WearError::InvalidSlotCount
    );
    assert_eq!(
        WearLeveler::new(vec![slot(1, 1, 0); MAX_SLOTS + 1]).unwrap_err(),
        WearError::InvalidSlotCount
    );
    assert_eq!(
        WearLeveler::new(vec![slot(7, 10, 0), slot(7, 20, 0)]).unwrap_err(),
        WearError::DuplicateSlot(7)
    );
    for invalid in [slot(1, 0, 0), slot(1, 10, 11)] {
        assert_eq!(
            WearLeveler::new(vec![invalid]).unwrap_err(),
            WearError::InvalidBudget(1)
        );
    }
}

#[test]
fn balances_uniform_slots_and_stops_at_the_limit() {
    let mut planner = WearLeveler::new((0..4).map(|id| slot(id, 128, 0)).collect()).unwrap();
    for completed in 1..=512 {
        planner
            .reserve_write()
            .unwrap()
            .complete(WriteOutcome::Committed);
        let states = snapshot(&planner);
        let minimum = states.iter().map(|slot| slot.charged_writes).min().unwrap();
        let maximum = states.iter().map(|slot| slot.charged_writes).max().unwrap();
        assert!(maximum - minimum <= 1);
        assert!(states
            .iter()
            .all(|slot| slot.charged_writes <= slot.write_limit));
        assert_eq!(planner.remaining_writes(), 512 - completed);
    }
    let before = snapshot(&planner);
    assert_eq!(
        planner.reserve_write().unwrap_err(),
        WearError::NoEligibleSlot
    );
    assert_eq!(snapshot(&planner), before);
    assert!(before.iter().all(|slot| slot.charged_writes == 128));
}

#[test]
fn respects_relative_wear_and_breaks_ties_by_id() {
    let mut planner = WearLeveler::new(vec![slot(20, 4, 0), slot(10, 2, 0)]).unwrap();
    let mut selected = Vec::new();
    for _ in 0..6 {
        let lease = planner.reserve_write().unwrap();
        selected.push(lease.slot_id());
        lease.complete(WriteOutcome::Committed);
    }
    assert_eq!(selected, [10, 20, 20, 10, 20, 20]);
    assert_eq!(planner.remaining_writes(), 0);
}

#[test]
fn compares_near_u64_max_without_rounding_or_overflow() {
    let maximum = u64::MAX;
    let mut planner = WearLeveler::new(vec![
        slot(0, maximum, maximum - 2),
        slot(1, maximum - 1, maximum - 3),
    ])
    .unwrap();
    let lease = planner.reserve_write().unwrap();
    assert_eq!(lease.slot_id(), 1);
    lease.complete(WriteOutcome::Committed);

    let mut final_write = WearLeveler::new(vec![slot(0, maximum, maximum - 1)]).unwrap();
    final_write
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::Committed);
    assert_eq!(final_write.slots().next().unwrap().charged_writes, maximum);
    assert_eq!(
        final_write.reserve_write().unwrap_err(),
        WearError::NoEligibleSlot
    );
    let total = WearLeveler::new(vec![slot(0, maximum, 0), slot(1, maximum, 0)]).unwrap();
    assert_eq!(total.remaining_writes(), u128::from(maximum) * 2);
}

#[test]
fn proven_non_write_refunds_the_reservation() {
    let mut planner = WearLeveler::new(vec![slot(1, 10, 3)]).unwrap();
    let before = snapshot(&planner);
    planner
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::NotWritten);
    assert_eq!(snapshot(&planner), before);
    assert_eq!(planner.remaining_writes(), 7);
}

#[test]
fn unknown_outcomes_and_unresolved_leases_stay_quarantined() {
    let mut planner = WearLeveler::new((0..3).map(|id| slot(id, 10, 0)).collect()).unwrap();
    planner
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::Unknown);
    {
        let _unfinished = planner.reserve_write().unwrap();
    }
    std::mem::forget(planner.reserve_write().unwrap());
    assert!(planner
        .slots()
        .all(|slot| slot.health == SlotHealth::Quarantined && slot.charged_writes == 1));
    assert_eq!(planner.remaining_writes(), 0);
    let before = snapshot(&planner);
    assert_eq!(
        planner.reserve_write().unwrap_err(),
        WearError::NoEligibleSlot
    );
    assert_eq!(snapshot(&planner), before);
}

#[test]
fn exhausted_and_retired_slots_are_never_selected() {
    let mut planner =
        WearLeveler::new(vec![slot(0, 1, 1), slot(1, 10, 2), slot(2, 10, 0)]).unwrap();
    planner.retire(2).unwrap();
    let lease = planner.reserve_write().unwrap();
    assert_eq!(lease.slot_id(), 1);
    lease.complete(WriteOutcome::Committed);
    assert_eq!(planner.slots().last().unwrap().health, SlotHealth::Retired);
    assert_eq!(planner.remaining_writes(), 7);
    let before = snapshot(&planner);
    assert_eq!(planner.retire(99), Err(WearError::UnknownSlot(99)));
    assert_eq!(snapshot(&planner), before);
}
