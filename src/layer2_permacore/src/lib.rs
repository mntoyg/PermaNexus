#![forbid(unsafe_code)]

pub mod wear_leveling;

pub mod actuator {
    tonic::include_proto!("permanexus.actuator.v1");
}

use actuator::{hardware_actuator_client::HardwareActuatorClient, RegisterAddress};
use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};

const RPC_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct ActuatorClient {
    inner: HardwareActuatorClient<Channel>,
}

impl ActuatorClient {
    pub async fn connect(endpoint: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let uri: tonic::codegen::http::Uri = endpoint.parse()?;
        let port = uri.port_u16().filter(|port| *port != 0);
        let expected_authority = port.map(|port| format!("127.0.0.1:{port}"));
        if uri.scheme_str() != Some("http")
            || uri.authority().map(|authority| authority.as_str()) != expected_authority.as_deref()
            || !matches!(
                uri.path_and_query().map(|path| path.as_str()),
                None | Some("/")
            )
            || port.is_none()
        {
            return Err("The simulator requires http://127.0.0.1:<port>".into());
        }
        let channel = Endpoint::from_shared(endpoint.to_owned())?
            .connect_timeout(RPC_TIMEOUT)
            .connect()
            .await?;
        let inner = HardwareActuatorClient::new(channel)
            .max_decoding_message_size(4096)
            .max_encoding_message_size(4096);
        Ok(Self { inner })
    }

    pub async fn read(&mut self, address: RegisterAddress) -> Result<u64, Status> {
        let mut request = Request::new(actuator::ReadRegisterRequest {
            address: Some(address),
        });
        request.set_timeout(RPC_TIMEOUT);
        Ok(self.inner.read_register(request).await?.into_inner().value)
    }

    // Writes are issued once. A transport timeout may hide a completed write.
    pub async fn compare_write(
        &mut self,
        address: RegisterAddress,
        value: u64,
        expected_value: u64,
    ) -> Result<u64, Status> {
        let mut request = Request::new(actuator::WriteRegisterRequest {
            address: Some(address),
            value,
            expected_value: Some(expected_value),
        });
        request.set_timeout(RPC_TIMEOUT);
        Ok(self
            .inner
            .write_register(request)
            .await?
            .into_inner()
            .previous_value)
    }
}

#[cfg(test)]
mod tests {
    use super::ActuatorClient;

    #[tokio::test]
    async fn rejects_non_loopback_or_ambiguous_endpoints() {
        for endpoint in [
            "http://example.com:50051",
            "http://0.0.0.0:50051",
            "https://127.0.0.1:50051",
            "http://user@127.0.0.1:50051",
            "http://127.0.0.1:50051/path",
            "http://127.0.0.1:50051/?query=1",
            "http://127.0.0.1:0",
            "http://127.0.0.1",
        ] {
            assert!(
                ActuatorClient::connect(endpoint).await.is_err(),
                "{endpoint}"
            );
        }
    }
}
