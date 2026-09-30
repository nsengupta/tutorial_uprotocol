// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! Phase 3 battery subscriber: same listener as Phase 2, registered on a Zenoh `UTransport`.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use async_trait::async_trait;
use tokio::sync::Notify;
use up_bms_proto::BatteryTelemetry;
use up_bms_proto::constants::*;
use up_rust::{LocalUriProvider, StaticUriProvider, UListener, UMessage, UTransport};
use up_transport_zenoh::{UPTransportZenoh, zenoh_config};

struct BatteryTelemetryListener {
    telemetry_seen: AtomicU32,
    shutdown: Arc<Notify>,
}

#[async_trait]
impl UListener for BatteryTelemetryListener {
    async fn on_receive(&self, msg: UMessage) {
        match msg.extract_protobuf::<BatteryTelemetry>() {
            Ok(telemetry) => {
                let count = self.telemetry_seen.fetch_add(1, Ordering::SeqCst) + 1;
                println!(
                    "[Battery telemetry subscriber] Processing incoming telemetry...\n\
                     -> State of Charge: {:.1}%\n\
                     -> Cell Temp: {} °C",
                    telemetry.soc_percent, telemetry.temp_celsius,
                );

                if count >= EXPECTED_MESSAGE_COUNT {
                    self.shutdown.notify_one();
                }
            }
            Err(err) => eprintln!("Failed to decode BatteryTelemetry payload: {err}"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let uri_provider =
        StaticUriProvider::new(AUTHORITY_NAME, PUBLISHER_UE_ID, PUBLISHER_UE_VERSION);
    let source_filter = uri_provider.get_resource_uri(BATTERY_TELEMETRY_RESOURCE_ID);

    let shutdown = Arc::new(Notify::new());
    let listener = Arc::new(BatteryTelemetryListener {
        telemetry_seen: AtomicU32::new(0),
        shutdown: shutdown.clone(),
    });

    // Same register_listener / on_receive as Phase 2. Only construction of the L1 plugin changes.
    let transport = UPTransportZenoh::builder(AUTHORITY_NAME)
        .map_err(|e| anyhow::anyhow!("builder failed: {e}"))?
        .with_config(zenoh_config::Config::default())
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("Zenoh transport build failed: {e}"))?;

    transport
        .register_listener(&source_filter, None, listener.clone())
        .await?;

    println!(
        "Battery telemetry subscriber listening (expecting {} messages)",
        EXPECTED_MESSAGE_COUNT
    );

    shutdown.notified().await;
    // Zenoh send does not wait for this process's on_receive — handshake first, then unregister.
    transport
        .unregister_listener(&source_filter, None, listener)
        .await?;
    println!("Received {EXPECTED_MESSAGE_COUNT} messages — exiting.");

    Ok(())
}
