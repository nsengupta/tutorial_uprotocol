# Stage 2: The Semantic Epiphany — uProtocol Layers and `up-uds-transport`

> **Prerequisites:** Stage 1 (`Stage-1.md`, code in `phases/01_raw_sockets/`, tag `Stage-1-Baseline`). Same UDS socket path, one subscriber, five messages then exit.

Stage 2 is where we stop treating uProtocol as “protobuf bytes on a wire” and start using its **layers and constituents** deliberately. We introduce a real **`UTransport`** implementation over Unix Domain Sockets — then pause before refactoring the application binaries (Stages 2.7–2.8, coming next).

For normative detail beyond this tutorial, see the [uProtocol specification (up-spec)](https://github.com/eclipse-uprotocol/up-spec) and the [Eclipse uProtocol project](https://projects.eclipse.org/projects/automotive.uprotocol).

---

## 2.1 — The uProtocol stack: a guided map

uProtocol is organised in layers. You do not need to memorise every line of the spec — but you **do** need to know which layer you are working in when you read or write code.

```
┌─────────────────────────────────────────────────────────────┐
│  Application / uEntity logic                                 │
│  (battery SoC, temperature thresholds, vehicle services)     │
├─────────────────────────────────────────────────────────────┤
│  L2 — Communication                                          │
│  Publisher, Request/Response helpers, CallOptions            │
│  “I want to PUBLISH an event on this resource URI”           │
├─────────────────────────────────────────────────────────────┤
│  L1 — Transport                                              │
│  UTransport, UListener, register_listener, send/receive        │
│  “Move this UMessage; call matching listeners when it arrives” │
├─────────────────────────────────────────────────────────────┤
│  Envelope (every message)                                    │
│  UMessage, UAttributes, UUri, UPayload, UPayloadFormat       │
│  “Who sent it, what type, what bytes, what format hint”        │
├─────────────────────────────────────────────────────────────┤
│  Wire / physical transport                                   │
│  UDS (Stage 1–2), Zenoh (Stage 3), MQTT, …                   │
└─────────────────────────────────────────────────────────────┘
```

### L1 — Transport (`UTransport`, `UListener`)

The transport layer moves **`UMessage`** values and invokes **`UListener`** callbacks for incoming messages that match registered URI filters.

| API (Rust / `up-rust`) | Role |
|---|---|
| `UTransport::send` | Send a complete `UMessage` |
| `UTransport::register_listener` | Register a callback for messages matching source/sink URI patterns |
| `UListener::on_receive` | Handle one decoded message (keep it fast; offload heavy work) |

**Spec pointer:** [uProtocol L1 — Transport Layer](https://github.com/eclipse-uprotocol/up-spec/blob/main/up-l1/README.adoc)

In Stage 1 we **bypassed** L1 — raw `UnixStream::read_exact` in the subscriber and `write_all` in the publisher. Stage 2 introduces `up-uds-transport` as our first real `UTransport` implementation.

### L2 — Communication (`Publisher`, message patterns)

L2 builds on L1 with higher-level patterns: publish an event, invoke an RPC method, send a request/response. In Rust you will see types like `SimplePublisher` and helpers like `UMessageBuilder`.

**Spec pointer:** [uProtocol L2 — Communication Layer](https://github.com/eclipse-uprotocol/up-spec/blob/main/up-l2/README.adoc)

We adopt L2 helpers when we refactor the publisher in Stage 2.7 (not yet — binaries still use Stage 1 code at this checkpoint).

### L3 — Discovery & registration (preview only)

L3 covers how entities **find and register** services across a vehicle network (e.g. PUBLISH pattern registration over a data-space transport). Stage 3 introduces this when we swap UDS for Zenoh.

**Spec pointer:** [uProtocol L3](https://github.com/eclipse-uprotocol/up-spec/blob/main/up-l3/README.adoc)

---

## 2.2 — Constituents in our battery telemetry story

Every `UMessage` carries the same building blocks. Here is what each one means **for us**:

| Constituent | What it is | Stage 1 | Stage 2 direction |
|---|---|---|---|
| **`UUri`** | Address: authority + entity ID + resource ID | Set on publisher `source` only; subscriber ignored it | Source URI identifies the **event/topic**; listeners register URI **filters** |
| **`UAttributes`** | Intent: message type, TTL, message ID, optional sink | Filled in but unused on receive | Listeners match on attributes; routing becomes explicit |
| **`UPayload`** | Application bytes + format hint | RAW CAN-style bytes | Move toward **protobuf payload** (Stage 2.7) — typed data on receive |
| **`UPayloadFormat`** | How to decode payload (`RAW`, protobuf, …) | `UPAYLOAD_FORMAT_RAW` | Use format hint + `extract_protobuf` instead of offset math |
| **`UListener`** | Callback for incoming messages | Inline decode loop in `main` | `on_receive(msg)` — business logic separated from socket I/O |
| **`UTransport`** | Pluggable send/receive/listener API | Not used | `up-uds-transport` (server + client) |

### `UUri` — addressing, not just metadata

A `UUri` identifies **who** and **what resource**:

```rust
UUri {
    authority_name: "local_vehicle".to_string(),
    ue_id: 0x1010,              // uEntity instance
    ue_version_major: 1,
    resource_id: 0x8001,        // event / topic / method identity
    ..Default::default()
}
```

For a **PUBLISH** message, the **source URI’s resource ID** is the event identity subscribers listen for. Listeners register a **filter URI** (often with wildcards) and receive only matching messages.

**Spec pointer:** [uProtocol URI basics](https://github.com/eclipse-uprotocol/up-spec/blob/main/basics/uri.adoc)

### `UAttributes` — intent mapping

Attributes answer: *what kind of message is this, from whom, with what lifetime?*

- `type_` — e.g. `UMESSAGE_TYPE_PUBLISH`
- `source` — publisher’s `UUri`
- `sink` — optional; used for RPC/request patterns (typically absent for publish)
- `id`, `ttl` — correlation and time-to-live

Stage 1 filled these in but the subscriber never read them. Stage 2’s transport dispatches using URI filter matching on the decoded attributes.

**Spec pointer:** [uProtocol uAttributes](https://github.com/eclipse-uprotocol/up-spec/blob/main/basics/uattributes.adoc)

### `UPayload` / `UPayloadFormat` — type enforcement (Stage 2.7)

Stage 1 showed that Protobuf types the **envelope**, not necessarily the **payload** when using `UPAYLOAD_FORMAT_RAW`. Stage 2.7 will close that gap with a protobuf BMS message. At this checkpoint (2.4), the transport carries whatever payload the publisher already builds.

---

## 2.3 — Stage 1 → Stage 2 contrast

| Concern | Stage 1 (`Stage-1-Baseline`) | Stage 2 (through 2.4) |
|---|---|---|
| **Transport API** | Raw `UnixStream` read/write in each binary | `UdsTransport` / `UdsTransportClient` implement `UTransport` |
| **Listener model** | Ad-hoc decode in spawned task | `register_listener` + `UListener::on_receive` (API ready; binaries not wired yet) |
| **URI usage** | Source set; no filter registration | Filter matching via `UUri::matches` in transport dispatch |
| **Framing** | Duplicated length-prefix logic in subscriber | Serialize via `up-frame-codec`; read/decode centralized in `up-uds-transport` |
| **Payload typing** | RAW bytes + shared CAN layout secret | Unchanged at 2.4 — refactor in 2.7 |
| **Publisher / subscriber binaries** | Stage 1 code | **Still Stage 1 code** until 2.7–2.8 |

What changes at 2.4 is the **plumbing crate**, not the demo applications — intentionally, so we can review the transport layer before touching business logic.

---

## 2.4 — `up-uds-transport`: a `UTransport` over UDS

**Crate:** `phases/02_uprotocol_semantics/crates/up-uds-transport`

This crate implements uProtocol’s L1 interface over the same length-framed Unix socket path used since Stage 0 (`/tmp/uprotocol_twin.sock`). It sits between **`up-frame-codec`** (framing bytes) and the **application binaries** (publisher/subscriber).

```
┌──────────────────────────┐   UTransport::send    ┌─────────────────────────┐
│ up-battery-telemetry-    │ ────────────────────► │ up-uds-transport        │
│ publisher (Stage 2.7)     │   framed UMessage     │  UdsTransportClient     │
└──────────────────────────┘                       └───────────┬─────────────┘
                                                               │ UDS
┌──────────────────────────┐   register_listener   ┌───────────▼─────────────┐
│ up-telemetry-subscriber  │ ◄──────────────────── │ up-uds-transport        │
│ (Stage 2.8)              │   UListener dispatch  │  UdsTransport::serve    │
└──────────────────────────┘                       └─────────────────────────┘
         ▲                                                    │
         │                                                    │
         └──────── up-frame-codec (length-prefix framing) ───┘
```

### Server side — `UdsTransport::serve`

Binds the socket path, spawns an accept loop, and for each connection reads one framed `UMessage` and **dispatches** it to registered listeners whose URI filters match.

```rust
let server = UdsTransport::serve("/tmp/uprotocol_twin.sock").await?;

server
    .register_listener(&source_filter_uri, None, listener.clone())
    .await?;
```

Dispatch reuses the same URI matching rules as `up-rust`’s in-process `LocalTransport`: a registered listener fires when `source_filter.matches(message.source)` and the sink filter aligns with the message (or is absent).

### Client side — `UdsTransportClient`

Connects per `send` (same connect-send-close pattern as Stage 1 publisher), frames via `up-frame-codec`, and writes to the server socket.

```rust
let client = UdsTransportClient::new("/tmp/uprotocol_twin.sock");
client.send(message).await?;
```

`register_listener` on the client returns `UNIMPLEMENTED` — listeners belong on the **server** handle.

### Core dispatch path (simplified)

```rust
// After read_framed_message(stream) → UMessage:
for registered in listeners.iter() {
    if registered.matches_msg(&message) {
        registered.on_receive(message.clone()).await;
    }
}
```

### Workspace layout (new at 2.4)

```
phases/02_uprotocol_semantics/crates/
├── up-frame-codec/          # length-prefix serialize (unchanged)
├── up-uds-transport/        # NEW — UTransport over UDS
├── up-battery-telemetry-publisher/   # still Stage 1 code
└── up-telemetry-subscriber/          # still Stage 1 code
```

### Verify the transport crate

```bash
cargo test --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-uds-transport
```

---

## What comes next (2.5 onward — not in this checkpoint)

| TODO | Topic |
|---|---|
| **2.5–2.6** | Tutorial: benefits of `up-uds-transport` + honest limits (fan-out, filesystem path, no L3) → Stage 3 setup |
| **2.7** | Refactor publisher: `SimplePublisher`, protobuf BMS payload, `UdsTransportClient` |
| **2.8** | Refactor subscriber: `UListener`, `UdsTransport::serve`, drop manual read loop |
| **2.9–2.10** | Intermediate evaluation, tag `Stage-2-Baseline`, update `README-Notes.md` |

---

## Key takeaway at 2.4

We now have a **real L1 transport crate** that speaks `UTransport` instead of raw sockets — but the demo binaries still run as Stage 1 until we wire them in. That separation lets us review the abstraction before refactoring application logic.
