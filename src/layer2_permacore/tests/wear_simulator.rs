use std::process::Command;

#[test]
fn cli_distributes_writes_and_reports_exhaustion() {
    for (arguments, committed, remaining, counts) in [
        (["9", "4", "10"], "9/9", "31", [3, 2, 2, 2]),
        (["20", "4", "2"], "8/20", "0", [2, 2, 2, 2]),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wear_simulator"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(&format!("Committed writes: {committed}")));
        assert!(stdout.contains(&format!("Remaining write budget: {remaining}\n")));
        for (id, count) in counts.into_iter().enumerate() {
            assert!(stdout.contains(&format!("slot={id} charged_writes={count} ")));
        }
    }
}

#[test]
fn cli_rejects_invalid_and_unbounded_workloads() {
    for arguments in [
        vec!["-1"],
        vec!["100001"],
        vec!["1", "0"],
        vec!["1", "257"],
        vec!["1", "1", "0"],
        vec!["1", "1", "1", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_wear_simulator"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
