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
    assert_eq!(
        raw.apply_command(request(ActuatorCommand {
            command_id: vec![1; 16],
            target: Some(target),
            desired_state: MicroState::LowPower as i32,
            expected_state: Some(MicroStateSnapshot {
                state: MicroState::Active as i32,
                revision: Some(0),
                instance_id: vec![2; 16],
            }),
        }))
        .await
        .unwrap_err()
        .code(),
        Code::Unimplemented
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
    assert_eq!(
        raw.apply_command(request(ActuatorCommand {
            command_id: vec![3; 16],
            target: telemetry_request("core-0", "synthetic-temperature").target,
            desired_state: MicroState::LowPower as i32,
            expected_state: Some(MicroStateSnapshot {
                state: MicroState::Active as i32,
                revision: Some(0),
                instance_id: instance_id.clone(),
            }),
        }))
        .await
        .unwrap_err()
        .code(),
        Code::Unimplemented
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
