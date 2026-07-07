// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! Shared BMS telemetry protobuf types and Stage 2 demo constants.
//!
//! The `.proto` schema lives at `proto/bms_telemetry.proto`; Rust types are generated
//! at build time by `build.rs` (see Stage 2 tutorial).

pub mod constants {
    pub const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";
    pub const AUTHORITY_NAME: &str = "local_vehicle";
    pub const PUBLISHER_UE_ID: u32 = 0x1010;
    pub const PUBLISHER_UE_VERSION: u8 = 0x01;
    pub const BATTERY_TELEMETRY_RESOURCE_ID: u16 = 0x8001;
    pub const EXPECTED_MESSAGE_COUNT: u32 = 5;
}

mod generated {
    include!(concat!(env!("OUT_DIR"), "/gen/mod.rs"));
}

pub use generated::bms_telemetry::BatteryTelemetry;
