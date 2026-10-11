use permanexus_permacore::wear_journal::{JournalError, JournaledWearLeveler};
use permanexus_permacore::wear_leveling::{SlotConfig, SlotHealth, WriteOutcome};
use std::fs;
use std::process::Command;

fn configuration() -> Vec<SlotConfig> {
    (0..4)
        .map(|id| SlotConfig {
            id,
            write_limit: 10,
            charged_writes: 0,
        })
        .collect()
}

#[test]
fn restart_preserves_writes_refunds_quarantine_and_retirement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("roundtrip.wearlog");
    let mut planner = JournaledWearLeveler::create(&path, configuration()).unwrap();
    planner
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::Committed)
        .unwrap();
    planner
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::NotWritten)
        .unwrap();
    planner
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::Unknown)
        .unwrap();
    {
        let _unfinished = planner.reserve_write().unwrap();
    }
    planner.retire(3).unwrap();
    let expected: Vec<_> = planner.slots().collect();
    drop(planner);
    let mut reopened = JournaledWearLeveler::open(&path).unwrap();
    assert_eq!(reopened.slots().collect::<Vec<_>>(), expected);
    assert_eq!(reopened.remaining_writes(), 9);
    assert_eq!(expected[0].charged_writes, 1);
    assert_eq!(expected[1].health, SlotHealth::Quarantined);
    assert_eq!(expected[2].health, SlotHealth::Quarantined);
    assert_eq!(expected[3].health, SlotHealth::Retired);
    let lease = reopened.reserve_write().unwrap();
    assert_eq!(lease.slot_id(), 0);
    lease.complete(WriteOutcome::Committed).unwrap();
}

#[test]
fn flushed_reservation_survives_termination_without_destructors() {
    const CHILD_FLAG: &str = "PERMANEXUS_JOURNAL_CRASH_TEST";
    if let Some(path) = std::env::var_os(CHILD_FLAG) {
        let mut planner = JournaledWearLeveler::open(path).unwrap();
        let _pending = planner.reserve_write().unwrap();
        std::process::exit(73);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("crash.wearlog");
    drop(JournaledWearLeveler::create(&path, configuration()).unwrap());
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "flushed_reservation_survives_termination_without_destructors",
        ])
        .env(CHILD_FLAG, &path)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut planner = JournaledWearLeveler::open(&path).unwrap();
    let interrupted = planner.slots().next().unwrap();
    assert_eq!(interrupted.health, SlotHealth::Quarantined);
    assert_eq!(interrupted.charged_writes, 1);
    let lease = planner.reserve_write().unwrap();
    assert_eq!(lease.slot_id(), 1);
    lease.complete(WriteOutcome::Committed).unwrap();
}

#[test]
fn exclusive_lock_blocks_another_process_and_releases_on_close() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("locked.wearlog");
    let planner = JournaledWearLeveler::create(&path, configuration()).unwrap();
    assert!(matches!(
        JournaledWearLeveler::open(&path),
        Err(JournalError::Locked)
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_wear_simulator"))
        .args(["1", "--state"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("locked"));
    drop(planner);
    assert!(JournaledWearLeveler::open(&path).is_ok());
}

#[test]
fn initialization_never_overwrites_and_missing_state_never_resets() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.wearlog");
    assert!(matches!(
        JournaledWearLeveler::open(&path),
        Err(JournalError::Io(_))
    ));
    assert!(!path.exists());
    drop(JournaledWearLeveler::create(&path, configuration()).unwrap());
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        JournaledWearLeveler::create(&path, configuration()),
        Err(JournalError::Io(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn corruption_and_any_incomplete_tail_fail_closed_without_modifying_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("damaged.wearlog");
    let mut planner = JournaledWearLeveler::create(&path, configuration()).unwrap();
    planner
        .reserve_write()
        .unwrap()
        .complete(WriteOutcome::Committed)
        .unwrap();
    drop(planner);
    let valid = fs::read(&path).unwrap();
    let mut fixtures = Vec::new();
    let mut header_damage = valid.clone();
    header_damage[20] ^= 1;
    fixtures.push(header_damage);
    let mut record_damage = valid.clone();
    let end = record_damage.len() - 1;
    record_damage[end] ^= 1;
    fixtures.push(record_damage);
    let mut wrong_version = valid.clone();
    wrong_version[7] = b'9';
    fixtures.push(wrong_version);
    for remaining in 1..17 {
        fixtures.push(valid[..valid.len() - remaining].to_vec());
    }
    fixtures.extend([vec![], valid[..10].to_vec()]);
    for fixture in fixtures {
        fs::write(&path, &fixture).unwrap();
        assert!(matches!(
            JournaledWearLeveler::open(&path),
            Err(JournalError::Corrupt(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), fixture);
    }
}

#[test]
fn persistent_cli_resumes_in_a_second_process_and_rejects_configuration_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cli.wearlog");
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_wear_simulator"))
            .args(arguments)
            .arg("--state")
            .arg(&path)
            .output()
            .unwrap()
    };
    let first = run(&["8", "4", "10", "--init"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run(&["4"]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8(second.stdout).unwrap();
    assert!(stdout.contains("Remaining write budget: 28"));
    for id in 0..4 {
        assert!(stdout.contains(&format!("slot={id} charged_writes=3 ")));
    }
    let before = fs::read(&path).unwrap();
    for arguments in [&["1", "2"][..], &["1", "4", "20"][..], &["1", "--init"][..]] {
        let output = run(arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
