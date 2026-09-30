// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! Phase 3 publisher: same five telemetry publishes as Phase 2, on Zenoh instead of a Unix socket.

use std::sync::Arc;

use rand::Rng;
use up_bms_proto::BatteryTelemetry;
use up_bms_proto::constants::*;
use up_rust::StaticUriProvider;
use up_rust::communication::{CallOptions, Publisher, SimplePublisher, UPayload};
use up_transport_zenoh::{UPTransportZenoh, zenoh_config};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("--- Battery telemetry publisher starting ---");

    let uri_provider = Arc::new(StaticUriProvider::new(
        AUTHORITY_NAME,
        PUBLISHER_UE_ID,
        PUBLISHER_UE_VERSION,
    ));

    // Same L2 publish loop as Phase 2. Only the L1 plugin construction changes.
    // Config::default() is a Zenoh peer with UDP multicast scouting (no zenohd required).
    let transport = Arc::new(
        UPTransportZenoh::builder(AUTHORITY_NAME)
            .map_err(|e| anyhow::anyhow!("builder failed: {e}"))?
            .with_config(zenoh_config::Config::default())
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Zenoh transport build failed: {e}"))?,
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
        publisher
            .publish(
                BATTERY_TELEMETRY_RESOURCE_ID,
                CallOptions::for_publish(Some(5000), None, None),
                Some(payload),
            )
            .await
            .map_err(|err| anyhow::anyhow!("publish failed: {err}"))?;

        println!();
    }

    Ok(())
}
