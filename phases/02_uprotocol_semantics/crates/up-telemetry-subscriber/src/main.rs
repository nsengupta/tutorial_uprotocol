// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! Phase 2 subscriber: bind the socket, receive five telemetry messages, then unregister and exit.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use async_trait::async_trait;
use tokio::sync::Notify;
use up_bms_proto::BatteryTelemetry;
use up_bms_proto::constants::*;
use up_rust::{LocalUriProvider, StaticUriProvider, UListener, UMessage, UTransport};
use up_unix_domain_socket_transport::UnixDomainSocketTransport;

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

    let socket_path = up_frame_codec::ensure_socket_dir()?;
    // bind() starts the accept loop immediately; register_listener is next.
    let transport = UnixDomainSocketTransport::bind(&socket_path).await?;
    transport
        .register_listener(&source_filter, None, listener.clone())
        .await?;

    println!(
        "Battery telemetry subscriber listening on: {} (expecting {} messages)",
        socket_path.display(),
        EXPECTED_MESSAGE_COUNT
    );

    shutdown.notified().await;
    // Stop later deliveries from main — not from on_receive.
    transport
        .unregister_listener(&source_filter, None, listener)
        .await?;
    println!("Received {EXPECTED_MESSAGE_COUNT} messages — exiting.");

    Ok(())
}
