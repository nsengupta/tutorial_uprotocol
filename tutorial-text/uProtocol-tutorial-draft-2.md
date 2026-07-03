### Prologue

This is not a stand-alone chapter. This is the second phase of the tutorial where we will build 
on everything that was established in Phase-1 (the previous chapter) and then some. So, if we
have not read the previous tutorial [draft](uProtocol-tutorial-draft-1.md), please do so. The code 
corresponding to that (previous) tutorial is in `phases/01_raw_sockets/`. This chapter's 
code lives in `phases/02_uprotocol_semantics/`.

In this chapter, we are going to:

- See where the Phase-1 architecture falls short
- Introduce _uProtocol_'s built-in layering and how it helps
- Replace the hand-rolled socket code with `UdsTransport` and `UdsTransportClient` — uProtocol's 
  L1 transport over UDS
- Replace the hand-made `UPayload` (with RAW CAN layout) with typed Protobuf messages
- Replace the manual `UAttributes` + `UMessage` assembly with `SimplePublisher` (L2)
- Replace the ad-hoc subscriber decode loop with `UListener` callbacks
- Be honest about what still doesn't work

Let's dive in.

----

### Chapter 1: Where Phase-1 left us

In Phase-1, we successfully sent a `UMessage` from the battery-telemetry-publisher to the 
telemetry-subscriber over a Unix Domain Socket. We packed SoC and temperature into 8 RAW bytes 
using `pack_bms_can_frame` and unpacked with `unpack_bms_can_frame`. It worked.

But, let's re-read the subscriber code from Phase-1:

```rust
// Phase-1 subscriber — a transport program in disguise
loop {
    let (mut stream, _) = listener.accept().await?;
    tokio::spawn(async move {
        let mut len_bytes = [0u8; 4];
        if stream.read_exact(&mut len_bytes).await.is_err() { return; }
        let body_len = u32::from_be_bytes(len_bytes) as usize;
        let mut body_bytes = vec![0u8; body_len];
        if stream.read_exact(&mut body_bytes).await.is_err() { return; }
        // ... decode, unpack CAN frame, print ...
    });
}
```

And the publisher:

```rust
// Phase-1 publisher — envelope factory
let attributes = UAttributes {
    id: MessageField::from(Some(up_rust::UUID::build())),
    type_: UMessageType::UMESSAGE_TYPE_PUBLISH.into(),
    source: Some(source_uri.clone()).into(),

    // LEFT BLANK: This is a decoupled PUBLISH broadcast, not an RPC request!
    // sink: None.into(),

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

```
┌─────────────────────────────────────────────────────────┐
│              Phase-1 Architecture                       │
│                                                         │
│  Publisher (main.rs)             Subscriber (main.rs)   │
│  ┌───────────────────┐           ┌───────────────────┐  │
│  │ 1. Build UMessage │           │ 1. Accept socket  │  │
│  │ 2. Serialize      │   UDS     │ 2. Read + decode  │  │
│  │ 3. Connect + send │─────────▶ │ 3. Unpack CAN     │  │
│  │                   │           │ 4. Print telemetry│  │
│  └───────────────────┘           └───────────────────┘  │
│                                                         │
│  Everything in one binary — no separation of            │
│  transport logic from business logic.                   │
│  CAN byte layout is a shared secret.                    │
│  No way to add a second consumer.                       │
└─────────────────────────────────────────────────────────┘
```

### Something is not quite right there

Both binaries embed transport details that do not belong in battery telemetry code.

- The **publisher** does three jobs: build a `UMessage`, frame it, and connect+write to a socket.
- The **subscriber** does three jobs: accept a connection, read+decode a frame, interpret the payload
- If we want a second consumer for the same telemetry (say, a thermal management logging engine),
  we cannot give it access to the subscriber's socket — there is only one accept loop.
- The **payload** is RAW bytes with a secret CAN-byte layout. If we change the layout, then both 
  sides must be updated in lockstep.

Phase-2 fixes these using uProtocol's own facilities: L1 Transport, L2 Communication patterns,
and typed Protobuf payloads.

----

### Chapter 2: The layers of uProtocol (a map)

uProtocol is organized in layers. Understanding the layers, helps.

```
┌────────────────────────────────────────────────────────────────┐
│  Application / uEntity logic                                   │
│  (battery SoC, temperature thresholds, vehicle services)       │
├────────────────────────────────────────────────────────────────┤
│  L2 — Communication                                            │
│  Publisher, Request/Response helpers, CallOptions              │
│  "I want to PUBLISH an event on this resource URI"             │
├────────────────────────────────────────────────────────────────┤
│  L1 — Transport                                                │
│  UTransport, UListener, register_listener, send/receive        │
│  "Move this UMessage; call matching listeners when it arrives" │
├────────────────────────────────────────────────────────────────┤
│  Envelope (every message)                                      │
│  UMessage, UAttributes, UUri, UPayload, UPayloadFormat         │
│  "Who sent it, what type, what bytes, what format hint"        │
├────────────────────────────────────────────────────────────────┤
│  Wire / physical transport                                     │
│  UDS (Phase 1–2), Zenoh (Phase 3), MQTT, …                     │
└────────────────────────────────────────────────────────────────┘
```

Phase-1 lived at the **Envelope** and **Wire** layers. We assembled `UMessage` by hand and
serialised it directly into a socket.

Phase-2 introduces L1 (`UTransport`, `UListener`) and L2 (`SimplePublisher`, `CallOptions`). The
wire (UDS + length-prefix framing) stays the same — we wrap it behind uProtocol's own abstractions.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Phase-2 Architecture                             │
│                                                                     │
│  Publisher (main.rs)              Subscriber (main.rs)              │
│  ┌──────────────────────┐        ┌──────────────────────────┐       │
│  │ SimplePublisher      │        │ BatteryTelemetryListener │       │
│  │ (L2 — publish call)  │        │ (L1 — UListener impl)    │       │
│  └────────┬─────────────┘        └───────────▲──────────────┘       │
│           │                                  │                      │
│  ┌────────▼─────────────┐         ┌──────────┼───────────────┐      │
│  │ UdsTransportClient   │         │ UdsTransport (server)    │      │
│  │ (L1 — send)          │────────▶│ (L1 — dispatch to        │      │
│  └──────────────────────┘  UDS    │  registered listeners)   │      │
│                                   └──────────────────────────┘      │
│                                                                     │
│  Shared: up-bms-proto (protobuf schema)                             │
│  Shared: up-frame-codec (length-prefix framing)                     │
│  Transport code lives in up-uds-transport, NOT in main.rs           │
└─────────────────────────────────────────────────────────────────────┘
```

----

### Chapter 3: What changed in the workspace

Phase-2 lives in `phases/02_uprotocol_semantics/`. The workspace grew from 3 to 5 crates:

```
phases/02_uprotocol_semantics/
├── Cargo.toml
└── crates/
    ├── up-frame-codec/          # length-prefix serialize (unchanged from Phase 1)
    ├── up-bms-proto/            # NEW: shared protobuf schema + demo constants
    ├── up-uds-transport/        # NEW: UTransport over UDS (server + client)
    ├── up-battery-telemetry-publisher/  # REFACTORED: uses SimplePublisher
    └── up-telemetry-subscriber/         # REFACTORED: uses UListener
```

----

### Chapter 4: `up-bms-proto` — why typed protobuf payloads

Let's revisit the Phase-1 publisher code. At the point we hand data to uProtocol, we had:

```rust
let u_payload = UPayload::new(
    pack_bms_can_frame(battery_pct, temp_c).to_vec(), // 8 raw bytes
    UPayloadFormat::UPAYLOAD_FORMAT_RAW
);
```

And on the subscriber side:

```rust
let payload = msg.payload.unwrap();
let (pct, temp) = unpack_bms_can_frame(&payload);
```

`pack_bms_can_frame` and `unpack_bms_can_frame` are **C code in Rust clothing**. They exist
because the publisher and subscriber need a way to agree on how SoC and temperature are laid
out in a byte buffer. In C-based automotive systems, this agreement comes from a DBC file (CAN
database). Here, it comes from a pair of functions.

Let's look at the actual functions from Phase-1:

```rust
// 1 LSB = 0.5% SoC — this is a DBC convention in disguise
fn pack_bms_can_frame(soc_pct: f32, temp_c: f32) -> [u8; 8] {
    let soc_encoded = (soc_pct / 0.5) as u16;
    let temp_encoded = (temp_c * 10.0) as i16;
    let mut buf = [0u8; 8];
    buf[0..2].copy_from_slice(&soc_encoded.to_be_bytes());
    buf[2..4].copy_from_slice(&temp_encoded.to_be_bytes());
    buf
}
```

Spot the problems:

1. **Scale factors are buried in code.** `0.5` and `10.0` are kind of, magic numbers. If we 
   change the SoC resolution from 0.5% to 0.1%, we must find and update both pack *and* unpack functions
   in lockstep. If we miss one, the subscriber prints garbage.

2. **Byte layout is implicit.** `buf[0..2]` for SoC, `buf[2..4]` for temperature. These offsets
   are invisible to anyone reading the code unless they carefully trace both functions.

3. **There is no schema.** Nothing tells us "this buffer contains two fields named
   `soc_percent` and `temp_celsius`". The field names exist only in the programmer's head and
   in Rust's variable names. A DBC file at least names the signals.

4. **Adding a third field is error-prone.** Double the buffer? Shift offsets? Every change
   requires a coordinated deploy of both publisher and subscriber.

**Protobuf solves all four problems in one move.**

Instead of a pair of pack/unpack functions with hidden offsets, we write a schema file:

```protobuf
// proto/bms_telemetry.proto
syntax = "proto3";
package bms_telemetry;

message BatteryTelemetry {
    float soc_percent = 1;
    int32 temp_celsius = 2;
}
```

- **No more bit-level encoding.** The raw CAN buffer required dividing SoC by 0.5 and
  multiplying temperature by 10 to fit into `u16`/`i16` — then the reverse on the subscriber
  side. Protobuf lets us express `soc_percent` as `float` and `temp_celsius` as `int32`
  directly. The scale conversion (how the sensor's ADC count maps to a physical value) is
  still an application concern — protobuf cannot annotate units — but the *wire-level*
  encode/decode arithmetic is gone.
- **Byte layout is gone.** Protobuf field numbers (`= 1`, `= 2`) replace byte offsets. The
  encoding is handled by the protobuf library.
- **The schema is the contract.** Both publisher and subscriber compile from the same `.proto`
  file. Any change to the schema is immediately visible at compile time — not at 2 AM when a
  subscriber prints -273.15°C because of a scale mismatch.
- **Adding a third field is safe.** Appending `optional int32 cell_voltage = 3;` to the message
  does not break existing publishers or subscribers — protobuf skips unknown fields at deserialisation.

The `up-bms-proto` crate wraps this proto file in a Cargo crate:

```rust
// build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    protobuf_codegen::Codegen::new()
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .include("proto")
        .input("proto/bms_telemetry.proto")
        .cargo_out_dir("gen")
        .run_from_script();
    Ok(())
}
```

It also exports demo constants so both the publisher and subscriber use the same socket path and
URI identifiers:

```rust
// up-bms-proto/src/lib.rs
pub mod constants {
    pub const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";
    pub const AUTHORITY_NAME: &str = "local_vehicle";
    pub const PUBLISHER_UE_ID: u32 = 0x1010;
    pub const PUBLISHER_UE_VERSION: u8 = 0x01;
    pub const BATTERY_TELEMETRY_RESOURCE_ID: u16 = 0x8001;
    pub const EXPECTED_MESSAGE_COUNT: u32 = 5;
}
```

Both the publisher and subscriber depend on `up-bms-proto`. There is no more DBC secret — the
`.proto` file is the **contract**.

----

A quick note on what this *does not* solve: protobuf gives us a typed data contract, but the
socket path (`/tmp/uprotocol_twin.sock`) and the URI conventions (`PUBLISHER_UE_ID = 0x1010`,
`BATTERY_TELEMETRY_RESOURCE_ID = 0x8001`) are still hardcoded into both binaries. Those will
move to configuration or discovery in later stages. For now, the constants crate is enough to
keep the demo honest.

### Chapter 5: What does the Phase-1 subscriber *actually* do?

Before we look at `up-uds-transport`, let's sit down with the Phase-1 subscriber code and trace
what it does for every incoming message. Copy the key section here:

```rust
// Phase-1 subscriber (simplified accept loop)
loop {
    let (mut stream, _) = listener.accept().await?;
    tokio::spawn(async move {
        // Step 1 — read the 4-byte length prefix
        let mut len_bytes = [0u8; 4];
        if stream.read_exact(&mut len_bytes).await.is_err() { return; }
        let body_len = u32::from_be_bytes(len_bytes) as usize;

        // Step 2 — read that many bytes
        let mut body_bytes = vec![0u8; body_len];
        if stream.read_exact(&mut body_bytes).await.is_err() { return; }

        // Step 3 — deserialise the protobuf UMessage
        let msg = UMessage::parse_from_bytes(&body_bytes[..]).ok()?;

        // Step 4 — extract the CAN payload and interpret
        let payload = msg.payload?;
        let (pct, temp) = unpack_bms_can_frame(&payload);
        println!("-> State of Charge: {:.1}%", pct);
        println!("-> Cell Temp: {} °C", temp);
    });
}
```

Let's count the implicit responsibilities in that one spawned closure:

| Step | Responsibility | Who should own it |
|---|---|---|
| 1–2 | Read framed bytes from a Unix socket | **Transport layer** — the socket code |
| 3 | Deserialise a protobuf `UMessage` | **Transport layer** — `read_framed_message` |
| 4 | Extract the typed payload and print | **Application** — the subscriber |

Three responsibilities. Two of them have nothing to do with battery telemetry.

If we wanted to add a second consumer — say, a thermal management logging engine — we would have
to copy the socket-reading code. If we wanted to switch from UDS to Zenoh, we would rewrite the
socket-reading code. If we wanted to run two listeners inside the same process (one for SoC,
one for temperature), we would need to duplicate the accept loop or build our own dispatch table.

**This is where a transport crate enters the story.**

The transport crate's job is to own steps 1–3 so the application code (step 4) does not need to
know about sockets, length prefixes, or protobuf envelope parsing. uProtocol gives us a trait
for this:

```
╔═══════════════════════════════════════════════╗
║   UTransport (trait)                          ║
╠═══════════════════════════════════════════════╣
║  send(message)                                ║
║  register_listener(filter, listener)          ║
║  unregister_listener(filter, listener)        ║
╚═══════════════════════════════════════════════╝
```

`UTransport` says: "I know how to move a `UMessage` from point A to point B. I know how to
accept registrations from interested parties. I know when to call those parties. You tell me
_who_ is interested and _what_ we want to send; the transport handles the rest."

This is not an abstraction for the joy of abstraction. It is a **responsibility boundary**.
The transport code lives in `up-uds-transport`. The subscriber code lives in
`up-telemetry-subscriber`. They depend on each other through the `UTransport` + `UListener`
traits, not through a shared socket path embedded in application code.

In the same way, the publisher should not know about socket I/O either. The publisher says
"publish this data"; the *transport* says "I will frame it, connect to the socket, and write
it." This is where `UdsTransportClient` enters — the client-side mirror of the same trait.

Let's look at `up-uds-transport` carefully.

----

### Chapter 6: `up-uds-transport` — the transport crate

`up-uds-transport` lives at `phases/02_uprotocol_semantics/crates/up-uds-transport/`. It
contains two implementations of `UTransport`:

**`UdsTransport` (server side)** — binds a socket path, spawns an accept loop, reads framed
messages, and dispatches to registered listeners:

```rust
let server = UdsTransport::serve("/tmp/uprotocol_twin.sock").await?;
server
    .register_listener(&source_filter_uri, None, listener.clone())
    .await?;
```

The dispatch logic (simplified from the crate):

```rust
for registered in listeners.iter() {
    if registered.matches_msg(&message) {
        registered.on_receive(message.clone()).await;
    }
}
```

Where `matches_msg` uses `up-rust`'s `UUri::matches` — the same URI matching rules that
`LocalTransport` uses. A listener fires when the source filter matches the message's source URI.

**`UdsTransportClient` (client side)** — connects per `send`, frames via `up-frame-codec`, writes
bytes:

```rust
let client = UdsTransportClient::new("/tmp/uprotocol_twin.sock");
client.send(message).await?;
```

`register_listener` on the client returns `UNIMPLEMENTED` — listeners live on the server side.

#### How does this compare to Phase-1?

Phase-1's subscriber was doing the transport layer's job. It read raw bytes from a socket,
decoded them, and interpreted the payload — all in one spawned task. The transport crate now
owns the read-decode-dispatch pipeline. The subscriber only implements `UListener::on_receive`.

```
Phase-1:  socket → read_exact(4) → read_exact(N) → parse → application logic
Phase-2:  socket → [up-uds-transport] → filter match → UListener::on_receive
                                     ↑
                            (centralised, reusable)
```

----

### Chapter 7: Refactored publisher — `SimplePublisher` + protobuf payload

Let's look at the publisher code. Compare it with the Phase-1 version from the previous chapter.

```rust
// up-battery-telemetry-publisher/src/main.rs (Phase 2)

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
    let publisher = SimplePublisher::new(transport, uri_provider);

    let mut rng = rand::rng();

    for i in 1..=EXPECTED_MESSAGE_COUNT {
        let telemetry = BatteryTelemetry {
            soc_percent: rng.random_range(75.0..78.9),
            temp_celsius: rng.random_range(20..=25),
            ..Default::default()
        };

        println!("Message {}: SoC = {:.1}%, Temp = {}°C",
            i, telemetry.soc_percent, telemetry.temp_celsius);

        let payload = UPayload::try_from_protobuf(telemetry)?;
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
```

#### What did we lose?

| Phase-1 code | Phase-2 equivalent |
|---|---|
| `UUri { authority_name, ue_id, ue_version_major, resource_id, .. }` | `StaticUriProvider::new(authority, ue_id, version)` |
| `UAttributes { id, type_, source, ttl, .. }` | Inside `SimplePublisher::publish` + `CallOptions` |
| `UPayload::new(raw_bytes, UPAYLOAD_FORMAT_RAW)` | `UPayload::try_from_protobuf(telemetry)` |
| `UMessage { attributes, payload, .. }` | Never constructed by hand |
| `serialize_for_unix_socket` + `UnixStream::connect` + `write_all` | `SimplePublisher::publish` → `UTransport::send` |

**We no longer create `UMessage` by hand.** The `SimplePublisher` (from `up-rust`'s L2 helpers)
constructs the envelope correctly — `UMESSAGE_TYPE_PUBLISH`, source URI, message ID, TTL. What
was 20+ lines of error-prone struct initialisation is now one call: `publisher.publish(...)`.

**We no longer pack CAN bytes.** `UPayload::try_from_protobuf(telemetry)` serialises the
`BatteryTelemetry` protobuf message and wraps it with format hint
`UPAYLOAD_FORMAT_PROTOBUF_WRAPPED_IN_ANY`. No DBC offsets, no bit-shifting.

**We no longer connect and write to the socket.** That is inside `UdsTransportClient::send`,
which implements `UTransport::send`. The publisher only says "publish this data"; the how of
moving bytes lives in the transport layer.

----

### Chapter 8: Refactored subscriber — `UListener` + `UdsTransport::serve`

Now the subscriber. This is a bigger change. The entire socket loop is gone.

```
╔════════════════════════════════╗
║   UListener (trait)            ║
╠════════════════════════════════╣
║   async fn on_receive(         ║
║       &self,                   ║
║       msg: UMessage            ║
║   )                            ║
╚════════════════════════════════╝
```

We implement this trait:

```rust
struct BatteryTelemetryListener {
    received: Arc<AtomicU32>,
    shutdown: Arc<Notify>,
}

#[async_trait]
impl UListener for BatteryTelemetryListener {
    async fn on_receive(&self, msg: UMessage) {
        if let Some(telemetry) = msg.extract_protobuf::<BatteryTelemetry>().ok() {
            let output = format!(
                "[Battery Telemetry Listener] Processing incoming CAN telemetry...\n\
                 -> State of Charge: {:.1}%\n\
                 -> Cell Temp: {} °C",
                telemetry.soc_percent, telemetry.temp_celsius,
            );
            println!("{}", output);

            let prev = self.received.fetch_add(1, Ordering::SeqCst);
            if prev + 1 >= EXPECTED_MESSAGE_COUNT {
                self.shutdown.notify_one();
            }
        }
    }
}
```

And wire it into `main`:

```rust
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    let uri_provider = StaticUriProvider::new(
        AUTHORITY_NAME,
        PUBLISHER_UE_ID,
        PUBLISHER_UE_VERSION,
    );

    let server = UdsTransport::serve(SOCKET_PATH).await
        .map_err(|e| anyhow::anyhow!("server bind failed: {e}"))?;

    let filter_uri = uri_provider.get_resource_uri(BATTERY_TELEMETRY_RESOURCE_ID);
    println!("Battery telemetry subscriber listening on: {}", SOCKET_PATH);

    let received = Arc::new(AtomicU32::new(0));
    let shutdown = Arc::new(Notify::new());

    let listener = Arc::new(BatteryTelemetryListener {
        received: received.clone(),
        shutdown: shutdown.clone(),
    });

    server
        .register_listener(&filter_uri, None, listener)
        .await
        .map_err(|e| anyhow::anyhow!("register_listener failed: {e}"))?;

    shutdown.notified().await;
    println!("Received {} messages, shutting down.", received.load(Ordering::SeqCst));
    Ok(())
}
```

#### What changed from Phase-1?

Every line related to socket I/O is gone. In its place:

- `UdsTransport::serve(SOCKET_PATH)` — binds the socket and runs the accept loop
- `register_listener(&filter_uri, None, listener)` — declares interest: "I want messages from
  resource 0x8001 from entity 0x1010"
- `UListener::on_receive` — receives an already-decoded `UMessage` with a typed protobuf payload

**The subscriber no longer reads raw bytes.** It does not know about length prefixes, protobuf
envelope decoding, or `read_exact`. It receives a `UMessage` and calls `extract_protobuf` to get
the typed `BatteryTelemetry` struct.

**Dispatch is declarative.** The transport crate's `dispatch` method iterates registered
listeners and calls only those whose filters match. Phase-1's subscriber accepted every byte
that arrived on the socket. Phase-2's subscriber says "only wake me for resource 0x8001 from
the battery entity."

----

### Chapter 9: Build and run

```shell
# From the repo root
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml

# Terminal 1 — subscriber
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber

# Terminal 2 — publisher
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```

Expected output (subscriber):

```
Battery telemetry subscriber listening on: /tmp/uprotocol_twin.sock
[Battery Telemetry Listener] Processing incoming CAN telemetry...
-> State of Charge: 76.3%
-> Cell Temp: 22 °C
[Battery Telemetry Listener] Processing incoming CAN telemetry...
-> State of Charge: 77.8%
-> Cell Temp: 24 °C
... (5 messages total)
Received 5 messages, shutting down.
```

----

### Chapter 10: A look at what we have done

Let's look back at the architecture:

```
╔═════════════════════════════════════════════════════════════╗
║ Publisher (Phase 2)                                         ║
║                                                             ║
║   battery_pct, temp_c                                       ║
║      ↓                                                      ║
║   BatteryTelemetry { soc_percent, temp_celsius }            ║
║      ↓                                                      ║
║   UPayload::try_from_protobuf → PUBLISH format              ║
║      ↓                                                      ║
║   SimplePublisher::publish(resource_id, ...)                ║
║      ↓                                                      ║
║   UdsTransportClient::send  →  UDS socket                   ║
╚═════════════════════════════════════════════════════════════╝

╔═════════════════════════════════════════════════════════════╗
║ Subscriber (Phase 2)                                        ║
║                                                             ║
║   UDS socket  →  UdsTransport::serve                        ║
║      ↓                                                      ║
║   read_framed_message → UMessage                            ║
║      ↓                                                      ║
║   filter match → UListener::on_receive(msg)                 ║
║      ↓                                                      ║
║   msg.extract_protobuf::<BatteryTelemetry>()                ║
║      ↓                                                      ║
║   SoC: 76.3%, Temp: 22°C                                    ║
╚═════════════════════════════════════════════════════════════╝
```

The publisher does not know about sockets. The subscriber does not know about sockets. The
transport crate owns the wire; the application code owns the business logic.

**This is the uProtocol promise:** separate _what_ we want to happen (publish this telemetry,
listen for that resource) from _how_ bytes move (UDS, Zenoh, MQTT, ...).

----

### Chapter 11: Where UDS still falls short

The refactor buys us cleaner APIs and production-parity interfaces. But the **execution path**
is still bottlenecked by UDS. Let's be honest about this.

#### Limitation 1 — Point-to-point UDS (no fan-out)

`UdsTransport::serve` accepts a connection, reads one message, and dispatches to listeners
**inside that server process**. A second process (say, a thermal management logging engine)
cannot independently subscribe to the same battery telemetry stream. The socket is consumed by
our subscriber's accept loop.

Stage 1's problem — "how do we add a second consumer?" — is **unresolved**. The URI filters help
within one process, but they do not clone bytes to arbitrary peers on the network.

```
Phase-1 wall:                    Phase-2:
1 publisher → 1 subscriber       Same socket, same topology
                                 UTransport + URI filters
                                 + listener dispatch in-process
                                 (still one process, one socket)
                                 Thermal engine still cannot tap the stream
```

#### Limitation 2 — Filesystem socket path (local only)

```text
/tmp/uprotocol_twin.sock
```

This path exists only on one machine. Move the battery uEntity to a Zone ECU and the display to
a central compute node, and UDS breaks — there is no cross-machine Unix socket.

#### Limitation 3 — No location transparency

Both sides must agree on a literal path string and be on the same machine. There is
no way to spin up a subscriber on a different ECU without changing the transport entirely.

#### Limitation 4 — No L3 discovery

`register_listener` is a local, in-process registration on the server handle. It is not
vehicle-wide PUBLISH pattern registration. A new uEntity cannot discover our battery telemetry
event without out-of-band configuration.

----

### Chapter 12: Looking ahead — Phase 3

The four limitations above trace to one root cause: UDS is a local, point-to-point transport.
uProtocol's trait-based design lets us swap it without touching publisher or subscriber
business logic.

#### What Phase 3 changes

| Aspect | Phase-2 (keep) | Phase-3 (replace wire) |
|---|---|---|
| Envelope | `UMessage`, `UAttributes`, `UUri`, `UPayload` | Unchanged |
| L2 patterns | `SimplePublisher`, typed protobuf payload | Unchanged |
| L1 API | `UTransport::send`, `register_listener`, `UListener::on_receive` | **Same trait** — different plugin |
| Physical transport | UDS + length framing | Zenoh (network-transparent) |
| L3 | Not used | PUBLISH registration for vehicle-wide discovery |
| Demo scope | One battery subscriber | Thermal engine + fan-out payoff |

The key insight: **Phase-3 replaces transport execution; Phase-2 semantics stay.** We are
not relearning uProtocol in Phase-3 — we are unblocking topology.

----

### Appendix: Key takeaways

1. **L1 (`UTransport` / `UListener`) separates message moving from message handling.**
   Your business logic should implement `on_receive`, not `read_exact`.

2. **L2 (`SimplePublisher` / `CallOptions`) separates intent from envelope construction.**
   You say "publish" — the library fills in `UMESSAGE_TYPE_PUBLISH`, source URI, message ID,
   and TTL correctly.

3. **Typed protobuf payloads make the data contract explicit.**
   The `.proto` file is the source of truth, not a DBC offset comment.

4. **The UDS wire stays the same; the semantics around it change.**
   Phase-3 swaps the wire (Zenoh) but keeps these same L1/L2 APIs. That is the uProtocol
   promise — code to the trait, not the transport.

----

### Appendix B: Protobuf schema (bms_telemetry.proto)

The `up-bms-proto` crate in Phase-2 defines the payload contract using protobuf.
This replaces the ad-hoc `pack_bms_can_frame` used in Phase-1 with a schema that
both publisher and subscriber derive code from.

```protobuf
syntax = "proto3";

package tutorial.bms.v1;

// Battery management telemetry published by the demo publisher.
// SoC and cell temperature only.
message BatteryTelemetry {
  float soc_percent = 1;
  int32 temp_celsius = 2;
}
```

The publisher creates a `BatteryTelemetry` protobuf message, serialises it with
`prost`, and wraps the resulting bytes in a `UPayload`. The subscriber parses
the bytes back into the same type — the `.proto` file is the single source of truth.

----

