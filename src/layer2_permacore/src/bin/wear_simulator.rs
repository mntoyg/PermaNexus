use permanexus_permacore::wear_journal::{JournalError, JournaledWearLeveler};
use permanexus_permacore::wear_leveling::{
    SlotConfig, SlotSnapshot, WearError, WearLeveler, WriteOutcome,
};
use std::path::PathBuf;

type Error = Box<dyn std::error::Error>;

enum Planner {
    Memory(WearLeveler),
    Journal(JournaledWearLeveler),
}

impl Planner {
    fn write_one(&mut self) -> Result<bool, Error> {
        match self {
            Self::Memory(planner) => match planner.reserve_write() {
                Ok(lease) => lease.complete(WriteOutcome::Committed),
                Err(WearError::NoEligibleSlot) => return Ok(false),
                Err(error) => return Err(error.into()),
            },
            Self::Journal(planner) => match planner.reserve_write() {
                Ok(lease) => lease.complete(WriteOutcome::Committed)?,
                Err(JournalError::Wear(WearError::NoEligibleSlot)) => return Ok(false),
                Err(error) => return Err(error.into()),
            },
        }
        Ok(true)
    }
    fn snapshots(&self) -> Vec<SlotSnapshot> {
        match self {
            Self::Memory(planner) => planner.slots().collect(),
            Self::Journal(planner) => planner.slots().collect(),
        }
    }
    fn remaining_writes(&self) -> u128 {
        match self {
            Self::Memory(planner) => planner.remaining_writes(),
            Self::Journal(planner) => planner.remaining_writes(),
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let mut positional = Vec::new();
    let mut state: Option<PathBuf> = None;
    let mut initialize = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--state" {
            if state.is_some() {
                return Err("Duplicate --state option".into());
            }
            let path = arguments
                .next()
                .filter(|value| !value.is_empty())
                .ok_or("--state requires a path")?;
            state = Some(path.into());
        } else if argument == "--init" {
            if initialize {
                return Err("Duplicate --init option".into());
            }
            initialize = true;
        } else {
            let value = argument
                .into_string()
                .map_err(|_| "Numeric arguments must be UTF-8")?;
            if value.starts_with('-') || positional.len() >= 3 {
                return Err(
                    "Usage: wear_simulator [writes [slots [write_limit]]] [--state PATH] [--init]"
                        .into(),
                );
            }
            positional.push(value);
        }
    }
    if initialize && state.is_none() {
        return Err("--init requires --state PATH".into());
    }
    let writes: u64 = positional
        .first()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1000);
    let slots: u32 = positional
        .get(1)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(4);
    let limit: u64 = positional
        .get(2)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1000);
    if writes > 100_000 || !(1..=256).contains(&slots) || limit == 0 {
        return Err("Requires writes <= 100000, slots in 1..256, and write_limit >= 1".into());
    }
    let configuration = || {
        (0..slots)
            .map(|id| SlotConfig {
                id,
                write_limit: limit,
                charged_writes: 0,
            })
            .collect()
    };
    let mut planner = if let Some(path) = &state {
        let journal = if initialize {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)?;
            }
            JournaledWearLeveler::create(path, configuration())?
        } else {
            JournaledWearLeveler::open(path)?
        };
        let snapshots: Vec<_> = journal.slots().collect();
        if snapshots.len() > 256
            || (positional.len() >= 2 && snapshots.len() != slots as usize)
            || (positional.len() >= 3 && snapshots.iter().any(|slot| slot.write_limit != limit))
        {
            return Err("CLI configuration does not match the existing journal".into());
        }
        Planner::Journal(journal)
    } else {
        Planner::Memory(WearLeveler::new(configuration())?)
    };
    let mut committed = 0;
    for _ in 0..writes {
        if !planner.write_one()? {
            break;
        }
        committed += 1;
    }
    println!("Committed writes: {committed}/{writes}");
    println!("Remaining write budget: {}", planner.remaining_writes());
    for slot in planner.snapshots() {
        println!(
            "slot={} charged_writes={} limit={} health={:?}",
            slot.id, slot.charged_writes, slot.write_limit, slot.health
        );
    }
    if let Some(path) = state {
        println!("State journal: {}", path.display());
    }
    Ok(())
}
