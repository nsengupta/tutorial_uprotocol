use std::sync::Arc;

use rand::Rng;
use up_bms_proto::constants::*;
use up_bms_proto::BatteryTelemetry;
use up_rust::communication::{CallOptions, Publisher, SimplePublisher, UPayload};
use up_rust::{StaticUriProvider, UTransport};
use up_uds_transport::UdsTransportClient;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    println!("--- Battery telemetry publisher starting ---");

    let uri_provider = Arc::new(StaticUriProvider::new(
        AUTHORITY_NAME,
        PUBLISHER_UE_ID,
        PUBLISHER_UE_VERSION,
    ));
    let transport: Arc<dyn UTransport> = Arc::new(UdsTransportClient::new(SOCKET_PATH));
    log::trace!(
        "using up-uds-transport send path: UdsTransportClient → {} (Stage 2 UTransport, not raw UnixStream)",
        SOCKET_PATH
    );
    let publisher = SimplePublisher::new(transport, uri_provider);

    let mut rng = rand::rng();

    for i in 1..=EXPECTED_MESSAGE_COUNT {
        let telemetry = BatteryTelemetry {
            soc_percent: rng.random_range(75.0..78.9),
            temp_celsius: rng.random_range(20..=25),
            ..Default::default()
        };

        println!(
            "Message {}: SoC = {:.1}%, Temp = {}°C",
            i, telemetry.soc_percent, telemetry.temp_celsius
        );

        let payload = UPayload::try_from_protobuf(telemetry)?;
        log::trace!(
            "SimplePublisher::publish → UTransport::send (resource_id=0x{BATTERY_TELEMETRY_RESOURCE_ID:04x})"
        );
        publisher
            .publish(
                BATTERY_TELEMETRY_RESOURCE_ID,
                CallOptions::for_publish(None, None, None),
                Some(payload),
            )
            .await
            .map_err(|err| anyhow::anyhow!("publish failed: {err}"))?;

        println!();
    }

    Ok(())
}
