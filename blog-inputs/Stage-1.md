# Stage 1: The Illusion of Success — and the Architectural Wall

## Why This Tutorial Exists

Software-Defined Vehicles (SDVs) run dozens of microservices that must share sensor and actuator data safely, across ECUs, zones, and network boundaries. **uProtocol** is designed to be the common semantic layer for that communication: a way to express *what* is being sent, *from whom*, and *to whom* — independent of the underlying transport (Unix sockets today, automotive Ethernet tomorrow).

This stage establishes a **working baseline** that looks successful on the surface. By the end, you will see why raw transport plumbing — even when wrapped in uProtocol bytes — hits a hard wall when a second consumer enters the picture.

> **Prerequisites:** Stage 0 (`Stage-0.md`) documents the original crate names (`up-client`, `up-server`). From Stage 1 onward the workspace uses clearer automotive names, but the transport mechanics are the same.

---

## What We Built

A minimal **uProtocol Layer 1 (uP-L1)** proof-of-concept: a battery telemetry **publisher** sends quantized CAN-style BMS data (State of Charge, cell temperature) wrapped in `UMessage` protobuf bytes to a **battery telemetry subscriber** over a length-framed **Unix Domain Socket** at `/tmp/uprotocol_twin.sock`.

The Cargo workspace has three crates:

| Crate | Binary | Role |
|---|---|---|
| `up-frame-codec` | _(library)_ | Shared length-prefix framing for `UMessage` |
| `up-battery-telemetry-publisher` | `up-battery-telemetry-publisher` | Publishes 5 telemetry messages, then exits |
| `up-telemetry-subscriber` | `up-telemetry-subscriber` | Listens on the socket, decodes, prints SoC and temperature |

```
┌──────────────────────────────┐         length-framed          ┌─────────────────────────────┐
│ up-battery-telemetry-        │  UMessage bytes over UDS       │ up-telemetry-subscriber     │
│ publisher                    │ ─────────────────────────────► │ (battery telemetry)         │
└──────────────────────────────┘   /tmp/uprotocol_twin.sock     └─────────────────────────────┘
```

---

## uProtocol in One Paragraph (First Principles)

At its core, uProtocol defines a **message envelope** — the `UMessage` — with two parts:

1. **`UAttributes`** — metadata describing intent: message type (e.g. `PUBLISH`), source address (`UUri`), optional destination, TTL, and a unique message ID.
2. **Payload** — application bytes plus a format hint (`UPayloadFormat`, e.g. `RAW`).

In Rust, `up-rust` maps the envelope to Protobuf on the wire: `UMessage::write_to_bytes()` to send, `UMessage::parse_from_bytes()` to receive. That integration gives you a **typed envelope** on both sides. What happens *inside* the payload is a separate question — and in this stage we deliberately keep that part untyped.

---

## The Subscriber — What It Does

**File:** `crates/up-telemetry-subscriber/src/main.rs`

While the publisher sends, the **battery telemetry subscriber** sits on the other end of the Unix socket and reverses the process: accept a connection, pull bytes off the wire, recover the `UMessage`, then interpret the payload as raw BMS data. It does not use `UAttributes` (source URI, message type, TTL) at this stage — it reaches straight for the payload bytes.

```rust
const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";

let listener = UnixListener::bind(SOCKET_PATH)?;

loop {
    let (mut stream, _) = listener.accept().await?;

    tokio::spawn(async move {
        // 1. Read a 4-byte length header, then exactly that many body bytes
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await?;
        let expected_len = u32::from_be_bytes(len_bytes) as usize;

        let mut body_bytes = vec![0u8; expected_len];
        stream.read_exact(&mut body_bytes).await?;

        // 2. Decode the body into a UMessage (envelope only — attributes ignored here)
        match UMessage::parse_from_bytes(&body_bytes[..]) {
            Ok(u_message) => {
                if let Some(payload_data) = u_message.payload.as_ref() {
                    let extracted_bytes: Vec<u8> = payload_data.clone().into();
                    let (soc, temp) = unpack_bms_can_frame(&extracted_bytes);

                    let output = format!(
                        "[Battery telemetry subscriber] Processing incoming CAN telemetry...\n\
                         -> State of Charge: {:.1}%\n\
                         -> Cell Temp: {} °C",
                        soc, temp,
                    );
                    println!("{}", output);
                    let _ = stdout().flush();
                }
            }
            Err(e) => eprintln!("Decode error: {:?}", e),
        }
    });
}
```

Three layers are visible even in this short sketch: **transport** (socket read), **uProtocol envelope** (protobuf decode), and **application** (CAN byte unpacking). The next section explains why step 1 needs an explicit length prefix on a stream socket.

---

## Why Length-Prefix Framing?

Unix Domain Sockets with `SOCK_STREAM` expose a **continuous byte stream** with no message boundaries. Without framing, two back-to-back messages become an ambiguous blob.

Our fix — implemented in `up-frame-codec` — prepends each protobuf-encoded `UMessage` with a **4-byte Big-Endian length header**:

```
┌─────────────────────────────────────────────┐
│  4 bytes (u32 BE)  │   N bytes              │
│  payload length N  │   protobuf UMessage      │
└─────────────────────────────────────────────┘
```

**File:** `crates/up-frame-codec/src/lib.rs`

```rust
pub fn serialize_for_unix_socket(msg: &UMessage) -> Result<Vec<u8>, anyhow::Error> {
    let payload_bytes = msg.write_to_bytes()?;
    let msg_len = payload_bytes.len() as u32;
    let mut framed_buffer = msg_len.to_be_bytes().to_vec();
    framed_buffer.append(&mut payload_bytes.to_vec());
    Ok(framed_buffer)
}
```

---

## The Publisher — Building and Sending a `UMessage`

**File:** `crates/up-battery-telemetry-publisher/src/main.rs`

The publisher constructs a `UMessage` for each of five telemetry samples, frames it, opens a fresh UDS connection, sends, and closes.

```rust
const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";

// Source uURI — identifies *who* is publishing
let source_uri = UUri {
    authority_name: "local_vehicle".to_string(),
    ue_id: 0x1010,
    ue_version_major: 1,
    resource_id: 0x8001,
    ..Default::default()
};

// Pack simulated BMS CAN bytes (SoC + temperature)
let u_payload = UPayload::new(
    pack_bms_can_frame(battery_pct, temp_c).to_vec(),
    UPayloadFormat::UPAYLOAD_FORMAT_RAW,
);

let attributes = UAttributes {
    id: MessageField::from(Some(up_rust::UUID::build())),
    type_: UMessageType::UMESSAGE_TYPE_PUBLISH.into(),
    source: Some(source_uri.clone()).into(),
    ttl: Some(5000),
    ..Default::default()
};

let message = UMessage {
    attributes: Some(attributes).into(),
    payload: Some(u_payload.payload()),
    ..Default::default()
};

let framed = serialize_for_unix_socket(&message)?;
let mut stream = UnixStream::connect(SOCKET_PATH).await?;
stream.write_all(&framed).await?;
```

The CAN packing helper applies a fictional DBC scale (0.5% per LSB for SoC):

```rust
fn pack_bms_can_frame(battery_level_pct: f32, temperature_c: i8) -> [u8; 8] {
    let mut can_data = [0u8; 8];
    let raw_soc = (battery_level_pct / 0.5) as u8;
    can_data[0] = raw_soc;
    can_data[1] = temperature_c as u8;
    can_data
}
```

Notice: the publisher fills in `UMESSAGE_TYPE_PUBLISH`, a **source** `UUri`, TTL, and a message ID — but **none of that matters to the subscriber right now**. The subscriber never inspects `UAttributes`; it decodes the envelope only to reach the payload bytes, treats them as **raw application data**, and leaves all interpretation to `unpack_bms_can_frame`. The `UMessage` wrapper carries far richer semantics than we use in this stage — destination addressing, intent-based routing, format enforcement — and we will put that metadata to work in later stages of this tutorial.

---

## The Subscriber — Reading, Decoding, Unpacking

**File:** `crates/up-telemetry-subscriber/src/main.rs`

The subscriber binds to the socket path, accepts connections, and spawns a task per connection. Each task reverses the framing steps:

```rust
// Step A: Read 4-byte Big-Endian length
let mut len_bytes = [0u8; 4];
stream.read_exact(&mut len_bytes).await?;
let expected_len = u32::from_be_bytes(len_bytes) as usize;

// Step B: Read exactly `expected_len` body bytes
let mut body_bytes = vec![0u8; expected_len];
stream.read_exact(&mut body_bytes).await?;

// Step C: Decode uProtocol semantics
match UMessage::parse_from_bytes(&body_bytes[..]) {
    Ok(u_message) => {
        if let Some(payload_data) = u_message.payload.as_ref() {
            let extracted_bytes: Vec<u8> = payload_data.clone().into();
            let (soc, temp) = unpack_bms_can_frame(&extracted_bytes);

            let output = format!(
                "[Battery telemetry subscriber] Processing incoming CAN telemetry...\n\
                 -> State of Charge: {:.1}%\n\
                 -> Cell Temp: {} °C",
                soc, temp,
            );
            println!("{}", output);
            let _ = stdout().flush();
        }
    }
    Err(e) => eprintln!("Decode error: {:?}", e),
}
```

The transport layer knows nothing about SoC or temperature — that interpretation lives entirely in `unpack_bms_can_frame`, an application-layer concern.

---

## Protobuf for the Envelope — Raw Bytes Inside

We use `UMessage` as a type on both sides, but that does **not** mean the receiver automatically gets a typed BMS object back. Two decoding steps are involved, and only the first is handled by uProtocol + Protobuf today.

**Step 1 — envelope (typed).** The framed body bytes decode into a `UMessage` via Protobuf:

```rust
// Publisher side — envelope to wire bytes
let payload_bytes = message.write_to_bytes()?;

// Subscriber side — wire bytes back to envelope
let u_message = UMessage::parse_from_bytes(&body_bytes[..])?;
// u_message.attributes, u_message.payload field — all typed Rust structs
```

This is the **uProtocol ↔ Protobuf integration** at work: the standard defines the envelope schema; `up-rust` generates Rust types; Protobuf handles serialisation. You get back a real `UMessage` with populated `UAttributes`.

**Step 2 — payload (not typed in our code).** Look at what the publisher placed inside:

```rust
UPayload::new(
    pack_bms_can_frame(battery_pct, temp_c).to_vec(),
    UPayloadFormat::UPAYLOAD_FORMAT_RAW,
);
```

`UPAYLOAD_FORMAT_RAW` declares that the payload is an **opaque byte sequence**. After Step 1, the subscriber has a `UMessage`, but `u_message.payload` is still just bytes — there is no `BmsTelemetry` struct reconstructed on receive. The subscriber cannot infer SoC or temperature from the envelope alone; it must call `unpack_bms_can_frame` with **prior knowledge** of the byte layout (byte 0 = scaled SoC, byte 1 = temperature).

| Layer | Protobuf / uProtocol provides | What Stage 1 does |
|---|---|---|
| **Envelope** (`UMessage`, `UAttributes`, `UUri`) | Typed encode/decode on the wire | ✅ Used |
| **Payload** (application data) | Format hint via `UPayloadFormat`; can carry protobuf, JSON, etc. | ❌ RAW bytes + hand-written offset math |

So we are saved in this demo only because **publisher and subscriber share a secret contract** — the fictional DBC layout — that lives outside the message. A new service (like the thermal logger we introduce later) would need the same out-of-band knowledge, or it would fail to interpret the stream.

That is precisely the gap `UPayloadFormat` is meant to close: a publisher can declare *how* payload bytes should be decoded, and uProtocol-aware consumers can use that hint instead of hard-coded offsets. Later stages will lean on richer payload contracts; Stage 1 keeps bytes raw to mirror legacy automotive data paths and to make the limitation visible.

---

## How to Run

```bash
# Terminal 1 — start the subscriber
cargo run -p up-telemetry-subscriber

# Terminal 2 — publish 5 telemetry messages
cargo run -p up-battery-telemetry-publisher
```

Expected subscriber output (values vary because the publisher randomises SoC and temperature):

```
Battery telemetry subscriber listening on: /tmp/uprotocol_twin.sock
[Battery telemetry subscriber] Processing incoming CAN telemetry...
-> State of Charge: 76.5%
-> Cell Temp: 23 °C
...
```

Expected publisher output:

```
--- Battery telemetry publisher starting ---
Message 1: SoC = 76.5%, Temp = 23°C
   Sent 142 bytes.
...
```

Five messages are sent; the publisher then exits. The subscriber keeps running, ready for the next session.

---

## Workspace Structure

```
up_twin_discovery/
├── Cargo.toml
├── crates/
│   ├── up-frame-codec/
│   ├── up-battery-telemetry-publisher/
│   └── up-telemetry-subscriber/
└── blog-inputs/
    ├── Stage-0.md    # historical baseline (original crate names)
    └── Stage-1.md    # this file
```

---

## The New Requirement: A Second Consumer

So far, one publisher and one subscriber on a local socket feels clean. In a real SDV, that is rarely enough.

**Scenario:** A **Thermal Management Logging Engine** — an independent microservice — must monitor the *same* battery cell temperature stream to detect threshold breaches and log thermal events. It cannot depend on the battery telemetry subscriber's internal state; it needs its own tap into the live stream.

```
                    ┌─────────────────────────────┐
                    │ up-telemetry-subscriber     │
                    │ (SoC + temperature display) │
                    └──────────────▲──────────────┘
                                   │
┌──────────────────────────────┐   │   ┌──────────────────────────────────┐
│ up-battery-telemetry-        │───┴──►│ Thermal Management Logging Engine │
│ publisher                    │   ?   │ (needs the same temperature data) │
└──────────────────────────────┘       └──────────────────────────────────┘
```

We have **not** implemented the thermal service yet — it arrives in a later stage when the transport can support it. But the requirement is real, and it exposes a flaw in our current design.

---

## Why Raw Unix Sockets Break — Pseudo-Code Friction

A `SOCK_STREAM` Unix socket connection is **point-to-point**. When the subscriber reads bytes from the kernel buffer, those bytes are **consumed** — they cannot be read again by a second process on the same connection.

If we tried to bolt a second consumer onto today's design, a developer might start extending the subscriber into an accidental message broker. Below is **illustrative pseudo-code** (not compiled, not in the repo) showing the friction:

```rust
// ⚠️  PSEUDO-CODE — do not compile. Shows the trap, not the solution.

use std::sync::{Arc, Mutex};
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// PROBLEM 1: Global fan-out state leaked into what was a simple subscriber.
static SUBSCRIBER_STREAMS: Mutex<Vec<UnixStream>> = Mutex::new(Vec::new());

async fn broken_broker_loop(listener: UnixListener) {
    loop {
        let (publisher_stream, _) = listener.accept().await?;

        // Read one framed UMessage from the publisher connection...
        let framed_message = read_framed_umessage(publisher_stream).await?;

        // PROBLEM 2: Manual duplication — we re-serialize and forward to every
        // registered downstream consumer ourselves.
        let subscribers = SUBSCRIBER_STREAMS.lock().unwrap().clone();
        for mut sub in subscribers {
            if sub.write_all(&framed_message).await.is_err() {
                // PROBLEM 3: Dead subscriber detection and reconnection logic
                // now lives in our telemetry path — unrelated to battery SoC.
                eprintln!("subscriber write failed — who cleans up the list?");
            }
        }
    }
}

// PROBLEM 4: The thermal logging engine cannot simply "connect and listen"
// on the same socket path — it would compete with the battery subscriber
// for the publisher's single connection, or require this broker layer.

async fn thermal_engine_connects_to_broker() {
    // Another socket, another registration API, another failure mode...
    SUBSCRIBER_STREAMS.lock().unwrap().push(
        UnixStream::connect("/tmp/uprotocol_twin_broker.sock").await.unwrap()
    );
}

// PROBLEM 5: Business logic (SoC display, temperature thresholds) is now
// mixed with transport fan-out, back-pressure, and connection lifecycle.
// We have reinvented a fragile, in-process message broker.
```

**What goes wrong:**

| Issue | Consequence |
|---|---|
| Point-to-point UDS | Only one reader per connection; no native pub/sub |
| Manual fan-out list | Subscriber becomes a broker; complexity explodes |
| Buffer duplication | Extra copies, back-pressure, and error handling in the hot path |
| No semantic routing | Every consumer sees every message; filtering is ad hoc |
| Filesystem socket path | Works on one machine only — useless across ECU boundaries |

Here is the key observation: **`UMessage` is independent of the transport**. The same protobuf envelope — attributes, payload, format hint — could be carried over Unix Domain Sockets today, automotive Ethernet tomorrow, or a data-space protocol like Zenoh; the application semantics do not change when the wire underneath changes. UDS is simply what we plugged in for this stage.

That independence is real and valuable — but it does not solve everything by itself. Wrapping payloads in `UMessage` was a good first step; **the transport and routing story around it is still hand-rolled**. We have the illusion of a clean architecture with a 1:1 demo — and a wall waiting when the vehicle adds a second service.

---

## Key Takeaways

1. **uProtocol messages are transport-agnostic** — `UMessage` serializes to protobuf and rides over a raw Unix socket today.
2. **Protobuf types the envelope, not necessarily the payload** — `parse_from_bytes` gives you a `UMessage`, but `UPAYLOAD_FORMAT_RAW` leaves application data as bytes; the receiver needs shared schema knowledge (our CAN layout) to reconstruct meaning.
3. **Framing is mandatory** for stream transports — length-prefix headers define message boundaries.
4. **Payload meaning is application-layer** — the codec moves bytes; `pack_bms_can_frame` / `unpack_bms_can_frame` interpret them.
5. **One consumer is deceptively easy** — a second independent service exposes raw UDS limitations immediately.
6. **Wrapping bytes in `UMessage` ≠ full uProtocol semantics** — we still lack proper routing, listener abstractions, and location-transparent transport.

---

## What Comes Next (Stage 2 Preview)

In Stage 2 we keep the same lightweight UDS connection but **refactor the application layer** around uProtocol's core abstractions — `UUri` addressing, `UAttributes` intent, `UPayloadFormat` enforcement, and the `UListener` callback model — so business logic becomes declarative instead of a tangle of offset math and socket reads.

The thermal logging engine and true multi-subscriber fan-out wait until the transport story catches up in Stage 3. For now, recognise the wall — and why the next layer of uProtocol exists.
