use permanexus_permacore::{actuator::RegisterAddress, ActuatorClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() > 1 {
        return Err("Usage: permanexus-permacore [http://127.0.0.1:50051]".into());
    }
    let endpoint = arguments
        .first()
        .map(String::as_str)
        .unwrap_or("http://127.0.0.1:50051");
    let mut client = ActuatorClient::connect(endpoint).await?;
    let address = RegisterAddress {
        device_id: "simulated-0".into(),
        offset_bytes: 4,
        width: 3,
    };
    let before = client.read(address.clone()).await?;
    let next = u64::from((before as u32).wrapping_add(1));
    let previous = client.compare_write(address.clone(), next, before).await?;
    let after = client.read(address).await?;
    if previous != before || after != next {
        return Err("Register changed during demo verification".into());
    }
    println!("simulated-0 offset=4 width=32: {before} -> {after}");
    Ok(())
}
