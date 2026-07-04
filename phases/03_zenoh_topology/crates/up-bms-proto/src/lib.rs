//! Shared BMS telemetry protobuf types and demo constants (Phase 2 → Phase 3 copy-forward).
//!
//! Schema: `proto/bms_telemetry.proto` — generated at build time by `build.rs`.

pub mod constants {
    /// Legacy Stage 2 UDS path — **retired** in Phase 3 active demo (§3.1).
    pub const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";

    pub const AUTHORITY_NAME: &str = "local_vehicle";
    pub const PUBLISHER_UE_ID: u32 = 0x1010;
    pub const PUBLISHER_UE_VERSION: u8 = 0x01;
    pub const BATTERY_TELEMETRY_RESOURCE_ID: u16 = 0x8001;
    pub const EXPECTED_MESSAGE_COUNT: u32 = 5;

    // §3.3 — Zenoh session / router endpoints (TBD when transport is wired).
    pub const ZENOH_CONNECT: &str = "tcp/127.0.0.1:7447";
}

mod generated {
    include!(concat!(env!("OUT_DIR"), "/gen/mod.rs"));
}

pub use generated::bms_telemetry::BatteryTelemetry;
