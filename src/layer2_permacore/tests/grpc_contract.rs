use permanexus_permacore::{
    actuator::{
        hardware_actuator_client::HardwareActuatorClient, ReadRegisterRequest, RegisterAddress,
        WriteRegisterRequest,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires the C++ simulator; run make test-grpc"]
async fn cpp_server_and_rust_client_obey_the_contract() {
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
