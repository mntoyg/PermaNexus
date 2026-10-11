use permanexus_permacore::wear_leveling::{SlotConfig, WearError, WearLeveler, WriteOutcome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() > 3 {
        return Err("Usage: wear_simulator [writes [slots [write_limit]]]".into());
    }
    let writes: u64 = arguments
        .first()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1000);
    let slots: u32 = arguments
        .get(1)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(4);
    let limit: u64 = arguments
        .get(2)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1000);
    if writes > 100_000 || !(1..=256).contains(&slots) || limit == 0 {
        return Err("Requires writes <= 100000, slots in 1..256, and write_limit >= 1".into());
    }
    let mut planner = WearLeveler::new(
        (0..slots)
            .map(|id| SlotConfig {
                id,
                write_limit: limit,
                charged_writes: 0,
            })
            .collect(),
    )?;
    let mut committed = 0;
    for _ in 0..writes {
        match planner.reserve_write() {
            Ok(lease) => {
                lease.complete(WriteOutcome::Committed);
                committed += 1;
            }
            Err(WearError::NoEligibleSlot) => break,
            Err(error) => return Err(error.into()),
        }
    }
    println!("Committed writes: {committed}/{writes}");
    println!("Remaining write budget: {}", planner.remaining_writes());
    for slot in planner.slots() {
        println!(
            "slot={} charged_writes={} limit={} health={:?}",
            slot.id, slot.charged_writes, slot.write_limit, slot.health
        );
    }
    Ok(())
}
