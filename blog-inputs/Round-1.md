# Round 1: Raw uProtocol Over Unix Domain Sockets

## What We Built

A minimal **uProtocol Layer 1 (uP-L1)** transport proof-of-concept that sends and receives uProtocol `UMessage` values over a local **Unix Domain Socket** (`SOCK_STREAM`). The project is structured as a Cargo workspace with three crates:

| Crate | Role |
|---|---|
| `up-frame-codec` | Shared library — frame serialization (length-prefix framing) |
| `up-server` | Binary — async Unix socket server, listens and decodes incoming messages |
| `up-client` | Binary — async Unix socket client, builds a `UMessage` and sends it |

---

## Why Length-Prefix Framing?

Unix Domain Sockets with `SOCK_STREAM` provide a continuous byte stream with no message boundaries. If two messages are sent back-to-back, the receiver cannot tell where one ends and the next begins. The fix: prepend each message with a **4-byte Big-Endian length header** so the receiver knows exactly how many bytes to read.

```
┌─────────────────────────────────────────────┐
│  4 bytes (u32 BE)  │   N bytes              │
│  payload length N  │   protobuf-encoded     │
│                    │   UMessage             │
└─────────────────────────────────────────────┘
```

---

## `up-frame-codec` — The Frame Serializer

**File:** `crates/up-frame-codec/src/lib.rs`

```rust
use protobuf::Message;
use up_rust::UMessage;

pub fn serialize_framed_message(msg: &UMessage) -> Result<Vec<u8>, anyhow::Error> {
    let payload_bytes = msg.write_to_bytes()?;
    let msg_len = payload_bytes.len() as u32;
    let mut framed_buffer = msg_len.to_be_bytes().to_vec();
    framed_buffer.append(&mut payload_bytes.to_vec());
    Ok(framed_buffer)
}
```

The function:
1. Serializes the `UMessage` into protobuf bytes via `write_to_bytes()`.
2. Gets the byte length as a `u32`.
3. Converts the length to 4 Big-Endian bytes and prepends them to the payload.

---

## `up-server` — The Listener

**File:** `crates/up-server/src/main.rs`

The server binds to `/tmp/uprotocol_twin.sock`, listens for connections, and spawns a tokio task per connection. Each task:

1. Reads exactly 4 bytes → interprets as Big-Endian `u32` length.
2. Reads exactly that many bytes → the serialized message body.
3. Decodes the body via `UMessage::parse_from_bytes()`.
4. Unpacks the raw payload bytes as if they were a **Battery Management System (BMS) CAN frame**, extracting state of charge and cell temperature.

```rust
// --- Server core: read frame, decode, unpack ---
let mut len_bytes = [0u8; 4];
stream.read_exact(&mut len_bytes).await?;
let expected_len = u32::from_be_bytes(len_bytes) as usize;

let mut body_bytes = vec![0u8; expected_len];
stream.read_exact(&mut body_bytes).await?;

match UMessage::parse_from_bytes(&body_bytes[..]) {
    Ok(u_message) => {
        if let Some(payload_data) = u_message.payload.as_ref() {
            let extracted_bytes: Vec<u8> = payload_data.clone().into();
            let (soc, temp) = unpack_bms_can_frame(&extracted_bytes);

            println!("[Digital Twin Server] Processing incoming CAN telemetry stream...");
            println!("-> State of Charge: {}%", soc);
            println!("-> Cell Temp: {} °C", temp);
        }
    }
    Err(e) => eprintln!("Decode error: {:?}", e),
}

// --- CAN frame unpack helper ---
fn unpack_bms_can_frame(can_data: &[u8]) -> (f32, i8) {
    if can_data.len() < 2 { return (0.0, 0); }

    // Scale: 1 LSB = 0.5% SoC (DBC rule)
    let raw_soc = can_data[0];
    let battery_level_pct = raw_soc as f32 * 0.5;
    let temperature_c = can_data[1] as i8;

    (battery_level_pct, temperature_c)
}
```

The server has no knowledge of what the payload *means* until it calls `unpack_bms_can_frame`. This cleanly demonstrates the layered architecture: the transport layer moves bytes, the application layer interprets them.

---

## `up-client` — The Sender

**File:** `crates/up-client/src/main.rs`

The client builds a `UMessage` whose payload is a **simulated BMS CAN frame** — raw bytes packed according to a fictional DBC (CAN database) specification.

```rust
// 1. Source URI — local_vehicle authority
let source_uri = UUri {
    authority_name: "local_vehicle".to_string(),
    ue_id: 0x1010,
    ue_version_major: 1,
    resource_id: 0x8001,
    ..Default::default()
};

// 2. Pack a BMS CAN frame (75% SoC, 25°C)
let u_payload = UPayload::new(
    pack_bms_can_frame(75.0).to_vec(),
    UPayloadFormat::UPAYLOAD_FORMAT_RAW,
);

// 3. Attributes with UUID, PUBLISH type, 5s TTL
let attributes = UAttributes {
    id: MessageField::from(Some(up_rust::UUID::build())),
    type_: UMessageType::UMESSAGE_TYPE_PUBLISH.into(),
    source: Some(source_uri.clone()).into(),
    ttl: Some(5000),
    ..Default::default()
};

// 4. Assemble, frame, send
let message = UMessage {
    attributes: Some(attributes).into(),
    payload: Some(u_payload.payload()),
    ..Default::default()
};
let framed = serialize_framed_message(&message)?;

let mut stream = UnixStream::connect("/tmp/uprotocol_twin.sock").await?;
stream.write_all(&framed).await?;

// --- CAN frame packing ---
fn pack_bms_can_frame(battery_level_pct: f32) -> [u8; 8] {
    let mut can_data = [0u8; 8];

    // Scale: raw = pct / 0.5  (e.g. 75.0% → 150)
    let raw_soc = (battery_level_pct / 0.5) as u8;
    can_data[0] = raw_soc;
    can_data[1] = 25; // 25°C raw temperature

    can_data
}
```

---

## How to Run

```bash
# Terminal 1 — start the server
cargo run -p up-server

# Terminal 2 — send a message
cargo run -p up-client
```

Expected server output:
```
uProtocol Socket Server listening on: /tmp/uprotocol_twin.sock
[Digital Twin Server] Processing incoming CAN telemetry stream...
-> State of Charge: 75%
-> Cell Temp: 25 °C
```

---

## Workspace Structure

```
up_twin_discovery/
├── Cargo.toml                  # workspace manifest
├── crates/
│   ├── up-frame-codec/         # shared library
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── up-server/              # server binary
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── up-client/              # client binary
│       ├── Cargo.toml
│       └── src/main.rs
└── blog-inputs/
    └── Round-1.md              # this file
```

---

## Key Takeaways

1. **uProtocol is transport-agnostic** — `UMessage` serializes cleanly to protobuf and can be sent over raw Unix sockets without a broker.
2. **Framing is mandatory** — without length-prefix headers, stream boundaries are ambiguous.
3. **The payload is opaque to the transport** — the server's `unpack_bms_can_frame` is a pure application-layer concern; the framing and protobuf layers just move bytes.
4. **End-to-end data integrity** — the client packs a BMS CAN frame with `75% SoC / 25°C`, frames it, sends it over Unix socket, and the server recovers the exact same values.
5. **This is the baseline** — Round 2 will introduce uProtocol's topic-based addressing and discovery semantics on top of this same transport.
