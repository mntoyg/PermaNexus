use permanexus_permacore::{
    actuator::{
        hardware_actuator_client::HardwareActuatorClient, ActuatorCommand, ComponentTarget,
        MicroState, MicroStateSnapshot, ReadRegisterRequest, RegisterAddress, TelemetryRequest,
        TelemetryUnit, WriteRegisterRequest,
    },
    ActuatorClient,
};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tonic::{Code, Request};

struct ServerProcess(Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn address(offset: u64, width: i32) -> RegisterAddress {
    RegisterAddress {
        device_id: "simulated-0".into(),
        offset_bytes: offset,
        width,
    }
}

fn request<T>(message: T) -> Request<T> {
    let mut request = Request::new(message);
    request.set_timeout(Duration::from_secs(2));
    request
}

fn telemetry_request(component: &str, sensor: &str) -> TelemetryRequest {
    TelemetryRequest {
        target: Some(ComponentTarget {
            device_id: "simulated-0".into(),
            component_id: component.into(),
        }),
        sensor_id: sensor.into(),
    }
}

fn command_for(
    component: &str,
    desired_state: MicroState,
    expected_state: &MicroStateSnapshot,
    command_id: u8,
) -> ActuatorCommand {
    ActuatorCommand {
        command_id: vec![command_id; 16],
        target: Some(ComponentTarget {
            device_id: "simulated-0".into(),
            component_id: component.into(),
        }),
        desired_state: desired_state as i32,
        expected_state: Some(expected_state.clone()),
    }
}

async fn read_state(
    client: &mut HardwareActuatorClient<tonic::transport::Channel>,
    component: &str,
) -> MicroStateSnapshot {
    let sensor = if component == "core-0" {
        "synthetic-temperature"
    } else {
        "synthetic-voltage"
    };
    client
        .read_telemetry(request(telemetry_request(component, sensor)))
        .await
        .unwrap()
        .into_inner()
        .observed_state
        .expect("Component state snapshot")
}

async fn assert_command_rejected_unchanged(
    client: &mut ActuatorClient,
    raw: &mut HardwareActuatorClient<tonic::transport::Channel>,
    command: ActuatorCommand,
    component: &str,
    expected_snapshot: &MicroStateSnapshot,
    expected_code: Code,
) {
    assert_eq!(
        client.apply_command(command).await.unwrap_err().code(),
        expected_code
    );
    assert_eq!(read_state(raw, component).await, *expected_snapshot);
}

fn start_server() -> (ServerProcess, String) {
    let path_file =
        std::env::var("PERMANEXUS_ACTUATOR_PATH_FILE").expect("Missing executable path file");
    let executable = std::fs::read_to_string(path_file).expect("Read executable path");
    let child = Command::new(executable.trim())
        .args(["--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Start C++ simulator");
    let mut server = ServerProcess(child);
    let output = server.0.stdout.take().expect("Server stdout");
    let (sender, receiver) = std::sync::mpsc::channel();
    let startup_reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    let listening = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("Server startup deadline")
        .expect("Read server announcement");
    startup_reader.join().expect("Server announcement reader");
    let endpoint = format!(
        "http://{}",
        listening
            .trim()
            .strip_prefix("LISTENING ")
            .expect("Server address")
    );
    (server, endpoint)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires the C++ simulator; run make test-grpc"]
async fn cpp_server_and_rust_client_obey_the_contract() {
    let (_server, endpoint) = start_server();
    let mut client = ActuatorClient::connect(&endpoint)
        .await
        .expect("Connect Rust client");
    for (offset, width, maximum) in [
        (0, 1, 255),
        (2, 2, 65535),
        (4, 3, u32::MAX as u64),
        (8, 4, u64::MAX),
    ] {
        assert_eq!(
            client
                .compare_write(address(offset, width), maximum, 0)
                .await
                .unwrap(),
            0
        );
        assert_eq!(client.read(address(offset, width)).await.unwrap(), maximum);
        assert_eq!(
            client
                .compare_write(address(offset, width), 0, maximum)
                .await
                .unwrap(),
            maximum
        );
    }

    let mut raw = HardwareActuatorClient::connect(endpoint.clone())
        .await
        .unwrap();
    let target = ComponentTarget {
        device_id: "simulated-0".into(),
        component_id: "core-0".into(),
    };
    let sample = raw
        .read_telemetry(request(TelemetryRequest {
            target: Some(target.clone()),
            sensor_id: "synthetic-temperature".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(sample.value, Some(42.5));
    let initial = sample.observed_state.expect("Initial Micro-State snapshot");
    let applied = client
        .apply_command(ActuatorCommand {
            command_id: vec![1; 16],
            target: Some(target),
            desired_state: MicroState::LowPower as i32,
            expected_state: Some(initial.clone()),
        })
        .await
        .expect("Apply RAM-only command");
    assert_eq!(applied.previous_state, Some(initial));
    assert_eq!(
        applied.current_state.unwrap().state,
        MicroState::LowPower as i32
    );
    for (offset, width, value) in [(0, 1, 0), (2, 2, 0), (4, 3, 0), (8, 4, 0), (16, 3, 1)] {
        assert_eq!(client.read(address(offset, width)).await.unwrap(), value);
    }
    assert_eq!(
        raw.read_register(request(ReadRegisterRequest { address: None }))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    for invalid in [address(3, 3), address(4, 0), address(4, 99), address(4, 2)] {
        assert_eq!(
            client.read(invalid).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
    assert_eq!(
        client.read(address(20, 3)).await.unwrap_err().code(),
        Code::NotFound
    );
    let mut unknown = address(4, 3);
    unknown.device_id = "unknown".into();
    assert_eq!(
        client.read(unknown).await.unwrap_err().code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .compare_write(address(16, 3), 2, 1)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    assert_eq!(
        client
            .compare_write(address(0, 1), 256, 0)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    assert_eq!(
        raw.write_register(request(WriteRegisterRequest {
            address: Some(address(4, 3)),
            value: 1,
            expected_value: None
        }))
        .await
        .unwrap_err()
        .code(),
        Code::InvalidArgument
    );
    assert_eq!(
        raw.write_register(request(WriteRegisterRequest {
            address: None,
            value: 1,
            expected_value: Some(0)
        }))
        .await
        .unwrap_err()
        .code(),
        Code::InvalidArgument
    );

    let mut writers = Vec::new();
    for _ in 0..32 {
        let mut writer = client.clone();
        writers.push(tokio::spawn(async move {
            writer.compare_write(address(4, 3), 1, 0).await
        }));
    }
    let mut successes = 0;
    for writer in writers {
        match writer.await.unwrap() {
            Ok(previous) => {
                assert_eq!(previous, 0);
                successes += 1;
            }
            Err(error) => assert_eq!(error.code(), Code::Aborted),
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(client.read(address(4, 3)).await.unwrap(), 1);
    assert_eq!(
        client
            .compare_write(address(4, 3), 99, 0)
            .await
            .unwrap_err()
            .code(),
        Code::Aborted
    );
    assert_eq!(client.read(address(4, 3)).await.unwrap(), 1);

    let demo = Command::new(env!("CARGO_BIN_EXE_permanexus-permacore"))
        .arg(&endpoint)
        .output()
        .expect("Run Rust demo");
    assert!(
        demo.status.success(),
        "{}",
        String::from_utf8_lossy(&demo.stderr)
    );
    assert!(String::from_utf8_lossy(&demo.stdout).contains("1 -> 2"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires the C++ simulator; run this focused test through the documented test-grpc setup"]
async fn apply_command_obeys_ram_policy_preconditions_and_rpc_isolation() {
    let (server, endpoint) = start_server();
    assert!(endpoint.starts_with("http://127.0.0.1:"));
    let mut raw = HardwareActuatorClient::connect(endpoint.clone())
        .await
        .unwrap();
    let mut client = ActuatorClient::connect(&endpoint).await.unwrap();
    let mut registers = ActuatorClient::connect(&endpoint).await.unwrap();

    for (offset, width, value) in [(0, 1, 11), (2, 2, 22), (4, 3, 33), (8, 4, 44)] {
        registers
            .compare_write(address(offset, width), value, 0)
            .await
            .unwrap();
    }
    let initial_core0 = read_state(&mut raw, "core-0").await;
    let initial_core1 = read_state(&mut raw, "core-1").await;
    assert_eq!(initial_core0.state, MicroState::Active as i32);
    assert_eq!(initial_core0.revision, Some(0));
    assert_eq!(initial_core1.state, MicroState::Active as i32);
    assert_eq!(initial_core1.revision, Some(0));
    assert_eq!(initial_core0.instance_id.len(), 16);
    assert_eq!(initial_core1.instance_id, initial_core0.instance_id);

    let transitions = [
        (MicroState::Active, MicroState::Active, true),
        (MicroState::Active, MicroState::Idle, true),
        (MicroState::Idle, MicroState::Active, true),
        (MicroState::Active, MicroState::LowPower, true),
        (MicroState::LowPower, MicroState::Active, true),
        (MicroState::Active, MicroState::Idle, true),
        (MicroState::Idle, MicroState::LowPower, true),
        (MicroState::LowPower, MicroState::LowPower, true),
        (MicroState::LowPower, MicroState::Idle, false),
        (MicroState::LowPower, MicroState::Active, true),
        (MicroState::Active, MicroState::Idle, true),
        (MicroState::Idle, MicroState::Idle, true),
    ];

    async fn apply_table(
        client: &mut ActuatorClient,
        raw: &mut HardwareActuatorClient<tonic::transport::Channel>,
        component: &str,
        initial: MicroStateSnapshot,
        transitions: &[(MicroState, MicroState, bool)],
    ) -> MicroStateSnapshot {
        let mut snapshot = initial;
        for (index, (source, desired, allowed)) in transitions.iter().enumerate() {
            assert_eq!(snapshot.state, *source as i32);
            let input = command_for(component, *desired, &snapshot, index as u8 + 1);
            let outcome = client.apply_command(input).await;
            if *allowed {
                let result = outcome.unwrap();
                assert_eq!(result.command_id, vec![index as u8 + 1; 16]);
                assert_eq!(
                    result.target,
                    Some(ComponentTarget {
                        device_id: "simulated-0".into(),
                        component_id: component.into(),
                    })
                );
                assert_eq!(result.previous_state, Some(snapshot.clone()));
                let current = result.current_state.expect("Committed state snapshot");
                assert_eq!(current.state, *desired as i32);
                assert_eq!(current.revision, Some(snapshot.revision.unwrap() + 1));
                assert_eq!(current.instance_id, snapshot.instance_id);
                snapshot = current;
                assert_eq!(read_state(raw, component).await, snapshot);
            } else {
                assert_eq!(outcome.unwrap_err().code(), Code::PermissionDenied);
                assert_eq!(read_state(raw, component).await, snapshot);
            }
        }
        snapshot
    }

    let final_core0 = apply_table(
        &mut client,
        &mut raw,
        "core-0",
        initial_core0.clone(),
        &transitions,
    )
    .await;
    assert_eq!(read_state(&mut raw, "core-1").await, initial_core1);
    let final_core1 = apply_table(
        &mut client,
        &mut raw,
        "core-1",
        initial_core1.clone(),
        &transitions,
    )
    .await;
    assert_eq!(read_state(&mut raw, "core-0").await, final_core0);

    let stale = command_for("core-0", MicroState::Active, &initial_core0, 30);
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        stale,
        "core-0",
        &final_core0,
        Code::Aborted,
    )
    .await;
    let mut wrong_instance = final_core0.clone();
    wrong_instance.instance_id = vec![99; 16];
    let instance_mismatch = command_for("core-0", MicroState::Active, &wrong_instance, 31);
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        instance_mismatch,
        "core-0",
        &final_core0,
        Code::FailedPrecondition,
    )
    .await;
    let mut stale_revision = final_core0.clone();
    stale_revision.revision = Some(final_core0.revision.unwrap() + 1);
    let revision_mismatch = command_for("core-0", MicroState::Active, &stale_revision, 32);
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        revision_mismatch,
        "core-0",
        &final_core0,
        Code::Aborted,
    )
    .await;
    let mut malformed = command_for("core-0", MicroState::Active, &final_core0, 33);
    malformed.command_id.clear();
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        malformed,
        "core-0",
        &final_core0,
        Code::InvalidArgument,
    )
    .await;
    let missing_target = ActuatorCommand {
        command_id: vec![34; 16],
        target: None,
        desired_state: MicroState::Active as i32,
        expected_state: Some(final_core0.clone()),
    };
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        missing_target,
        "core-0",
        &final_core0,
        Code::InvalidArgument,
    )
    .await;
    let unknown_target = command_for("core-9", MicroState::Active, &final_core0, 35);
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        unknown_target,
        "core-0",
        &final_core0,
        Code::NotFound,
    )
    .await;

    let to_low_power = command_for("core-0", MicroState::LowPower, &final_core0, 36);
    let low_power = client
        .apply_command(to_low_power)
        .await
        .unwrap()
        .current_state
        .unwrap();
    let mut wrong_instance_and_stale_state = low_power.clone();
    wrong_instance_and_stale_state.instance_id = vec![98; 16];
    wrong_instance_and_stale_state.state = MicroState::Active as i32;
    let precedence_instance = command_for(
        "core-0",
        MicroState::Idle,
        &wrong_instance_and_stale_state,
        37,
    );
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        precedence_instance,
        "core-0",
        &low_power,
        Code::FailedPrecondition,
    )
    .await;
    let mut stale_state_and_revision = low_power.clone();
    stale_state_and_revision.state = MicroState::Active as i32;
    stale_state_and_revision.revision = Some(low_power.revision.unwrap() + 1);
    let precedence_snapshot =
        command_for("core-0", MicroState::Idle, &stale_state_and_revision, 38);
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        precedence_snapshot,
        "core-0",
        &low_power,
        Code::Aborted,
    )
    .await;
    let denied = command_for("core-0", MicroState::Idle, &low_power, 39);
    assert_command_rejected_unchanged(
        &mut client,
        &mut raw,
        denied,
        "core-0",
        &low_power,
        Code::PermissionDenied,
    )
    .await;

    let oversized = ActuatorCommand {
        command_id: vec![40; 5000],
        target: Some(ComponentTarget {
            device_id: "simulated-0".into(),
            component_id: "core-0".into(),
        }),
        desired_state: MicroState::Active as i32,
        expected_state: Some(low_power.clone()),
    };
    assert_eq!(
        raw.apply_command(request(oversized))
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    assert_eq!(read_state(&mut raw, "core-0").await, low_power);

    let concurrent_snapshot = low_power.clone();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(33));
    let mut contenders = Vec::new();
    for _ in 0..32 {
        let mut writer = client.clone();
        let expected = concurrent_snapshot.clone();
        let barrier = barrier.clone();
        contenders.push(tokio::spawn(async move {
            barrier.wait().await;
            writer
                .apply_command(command_for("core-0", MicroState::Active, &expected, 77))
                .await
        }));
    }
    barrier.wait().await;
    let mut winners = 0;
    for contender in contenders {
        match contender.await.unwrap() {
            Ok(result) => {
                winners += 1;
                assert_eq!(result.previous_state, Some(concurrent_snapshot.clone()));
                assert_eq!(
                    result.current_state.unwrap().revision,
                    Some(concurrent_snapshot.revision.unwrap() + 1)
                );
            }
            Err(status) => assert_eq!(status.code(), Code::Aborted),
        }
    }
    assert_eq!(winners, 1);
    let core0_after_race = read_state(&mut raw, "core-0").await;
    assert_eq!(core0_after_race.state, MicroState::Active as i32);
    assert_eq!(
        core0_after_race.revision,
        Some(concurrent_snapshot.revision.unwrap() + 1)
    );
    assert_eq!(read_state(&mut raw, "core-1").await, final_core1);

    let command_core1 = command_for("core-1", MicroState::LowPower, &final_core1, 78);
    let core1_result = client.apply_command(command_core1).await.unwrap();
    assert_eq!(
        core1_result.current_state.unwrap().state,
        MicroState::LowPower as i32
    );
    assert_eq!(read_state(&mut raw, "core-0").await, core0_after_race);

    for (offset, width, value) in [(0, 1, 11), (2, 2, 22), (4, 3, 33), (8, 4, 44), (16, 3, 1)] {
        assert_eq!(registers.read(address(offset, width)).await.unwrap(), value);
    }

    drop(raw);
    drop(registers);
    drop(client);
    drop(server);
    // The Rust wrapper uses a two-second deadline and has one RPC call site with no retry loop.
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires the C++ simulator; run make test-grpc"]
async fn synthetic_telemetry_is_allowlisted_consistent_and_read_only() {
    let (server, endpoint) = start_server();
    let mut raw = HardwareActuatorClient::connect(endpoint.clone())
        .await
        .unwrap();
    let mut registers = ActuatorClient::connect(&endpoint).await.unwrap();
    for (offset, width, value) in [(0, 1, 11), (2, 2, 22), (4, 3, 33), (8, 4, 44)] {
        registers
            .compare_write(address(offset, width), value, 0)
            .await
            .unwrap();
    }
    let fixtures = [
        (
            "core-0",
            "synthetic-temperature",
            42.5,
            TelemetryUnit::Celsius,
        ),
        ("core-0", "synthetic-utilization", 0.0, TelemetryUnit::Ratio),
        ("core-1", "synthetic-voltage", 1.0, TelemetryUnit::Volts),
    ];
    let mut instance_id = Vec::new();
    for (component, sensor, value, unit) in fixtures {
        let input = telemetry_request(component, sensor);
        let sample = raw
            .read_telemetry(request(input.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(sample.target, input.target);
        assert_eq!(sample.sensor_id, sensor);
        assert_eq!(sample.value, Some(value));
        assert!(sample.value.unwrap().is_finite());
        assert_eq!(sample.unit, unit as i32);
        assert_eq!(sample.sampled_at_unix_ns, Some(0));
        let state = sample.observed_state.unwrap();
        assert_eq!(state.state, MicroState::Active as i32);
        assert_eq!(state.revision, Some(0));
        assert_eq!(state.instance_id.len(), 16);
        if instance_id.is_empty() {
            instance_id = state.instance_id;
        } else {
            assert_eq!(state.instance_id, instance_id);
        }
    }
    let mut invalid_requests = vec![TelemetryRequest::default()];
    for invalid in [
        "".to_owned(),
        "a".repeat(65),
        "a/b".into(),
        "a b".into(),
        "a\0b".into(),
        "é".into(),
    ] {
        for field in 0..3 {
            let mut input = telemetry_request("core-0", "synthetic-temperature");
            match field {
                0 => input.target.as_mut().unwrap().device_id = invalid.clone(),
                1 => input.target.as_mut().unwrap().component_id = invalid.clone(),
                _ => input.sensor_id = invalid.clone(),
            }
            invalid_requests.push(input);
        }
    }
    for input in invalid_requests {
        assert_eq!(
            raw.read_telemetry(request(input)).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
    let mut unknown_device = telemetry_request("core-0", "synthetic-temperature");
    unknown_device.target.as_mut().unwrap().device_id = "unknown".into();
    for input in [
        unknown_device,
        telemetry_request("unknown", "synthetic-temperature"),
        telemetry_request("core-0", "unknown"),
        telemetry_request("core-1", "synthetic-temperature"),
        telemetry_request("core-0", &"x".repeat(64)),
    ] {
        assert_eq!(
            raw.read_telemetry(request(input)).await.unwrap_err().code(),
            Code::NotFound
        );
    }
    assert_eq!(
        raw.read_telemetry(request(telemetry_request(
            "core-1",
            "synthetic-unavailable"
        )))
        .await
        .unwrap_err()
        .code(),
        Code::Unavailable
    );
    assert_eq!(
        raw.read_telemetry(request(telemetry_request("core-0", &"x".repeat(5000))))
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(33));
    let mut readers = Vec::new();
    for index in 0..32 {
        let mut reader = raw.clone();
        let barrier = barrier.clone();
        let expected_id = instance_id.clone();
        readers.push(tokio::spawn(async move {
            barrier.wait().await;
            for _ in 0..10 {
                let (component, sensor, value, unit) = fixtures[index % fixtures.len()];
                let sample = reader
                    .read_telemetry(request(telemetry_request(component, sensor)))
                    .await
                    .unwrap()
                    .into_inner();
                assert_eq!(sample.target.as_ref().unwrap().component_id, component);
                assert_eq!(sample.value, Some(value));
                assert_eq!(sample.unit, unit as i32);
                let state = sample.observed_state.unwrap();
                assert_eq!(state.state, MicroState::Active as i32);
                assert_eq!(state.revision, Some(0));
                assert_eq!(state.instance_id, expected_id);
            }
        }));
    }
    barrier.wait().await;
    // Register writes have their own authority and cannot advance Micro-State.
    registers
        .compare_write(address(4, 3), 34, 33)
        .await
        .unwrap();
    for reader in readers {
        reader.await.unwrap();
    }
    for (offset, width, value) in [(0, 1, 11), (2, 2, 22), (4, 3, 34), (8, 4, 44), (16, 3, 1)] {
        assert_eq!(registers.read(address(offset, width)).await.unwrap(), value);
    }
    let final_sample = raw
        .read_telemetry(request(telemetry_request(
            "core-0",
            "synthetic-temperature",
        )))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        final_sample.observed_state.unwrap(),
        MicroStateSnapshot {
            state: MicroState::Active as i32,
            revision: Some(0),
            instance_id: instance_id.clone(),
        }
    );
    drop(raw);
    drop(registers);
    drop(server);
    let (_restarted, endpoint) = start_server();
    let mut restarted = HardwareActuatorClient::connect(endpoint).await.unwrap();
    let state = restarted
        .read_telemetry(request(telemetry_request(
            "core-0",
            "synthetic-temperature",
        )))
        .await
        .unwrap()
        .into_inner()
        .observed_state
        .unwrap();
    assert_eq!(state.revision, Some(0));
    assert_eq!(state.state, MicroState::Active as i32);
    assert_eq!(state.instance_id.len(), 16);
    assert_ne!(state.instance_id, instance_id);
}
