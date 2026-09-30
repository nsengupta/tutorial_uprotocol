// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! Phase 3 thermal subscriber: a second process on the same Zenoh stream, watching cell temperature.

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

/// Second consumer on the same stream — its own process, not a second listener in the battery app.
struct ThermalLoggingListener {
    readings_seen: AtomicU32,
    shutdown: Arc<Notify>,
}

#[async_trait]
impl UListener for ThermalLoggingListener {
    async fn on_receive(&self, msg: UMessage) {
        match msg.extract_protobuf::<BatteryTelemetry>() {
            Ok(telemetry) => {
                let count = self.readings_seen.fetch_add(1, Ordering::SeqCst) + 1;

                let temp = telemetry.temp_celsius;
                if temp > 25 {
                    println!(
                        "[Thermal logging subscriber] ⚠️  WARNING — cell temperature {temp}°C exceeds 25°C threshold"
                    );
                } else {
                    println!("[Thermal logging subscriber] Cell temperature {temp}°C — OK");
                }

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
    let listener = Arc::new(ThermalLoggingListener {
        readings_seen: AtomicU32::new(0),
        shutdown: shutdown.clone(),
    });

    // Same construction and register_listener as the battery subscriber — a second process.
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
        "Thermal logging subscriber listening (expecting {} messages)",
        EXPECTED_MESSAGE_COUNT
    );

    shutdown.notified().await;
    transport
        .unregister_listener(&source_filter, None, listener)
        .await?;
    println!("Received {EXPECTED_MESSAGE_COUNT} messages — exiting.");

    Ok(())
}
