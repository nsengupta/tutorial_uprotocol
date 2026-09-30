// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! Phase 2 publisher: five battery telemetry messages over Unix Domain Socket via `SimplePublisher`.

use std::sync::Arc;

use rand::Rng;
use up_bms_proto::BatteryTelemetry;
use up_bms_proto::constants::*;
use up_rust::StaticUriProvider;
use up_rust::communication::{CallOptions, Publisher, SimplePublisher, UPayload};
use up_unix_domain_socket_transport::UnixDomainSocketTransport;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("--- Battery telemetry publisher starting ---");

    let uri_provider = Arc::new(StaticUriProvider::new(
        AUTHORITY_NAME,
        PUBLISHER_UE_ID,
        PUBLISHER_UE_VERSION,
    ));
    let socket_path = up_frame_codec::socket_path()?;
    // L1 plugin: wrap `Self` in `Arc` here — `SimplePublisher` needs a shared handle.
    let transport = Arc::new(UnixDomainSocketTransport::connect(&socket_path));
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

        // L2: UPayload wraps protobuf bytes + format for SimplePublisher.
        let payload = UPayload::try_from_protobuf(telemetry)?;
        publisher
            .publish(
                BATTERY_TELEMETRY_RESOURCE_ID,
                // Same 5 s window as Phase 1's `with_ttl(5000)`.
                CallOptions::for_publish(Some(5000), None, None),
                Some(payload),
            )
            .await
            .map_err(|err| anyhow::anyhow!("publish failed: {err}"))?;

        println!();
    }

    Ok(())
}
