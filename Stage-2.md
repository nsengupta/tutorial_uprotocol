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

## 2.5 — Why `up-uds-transport` is worth the refactor

Section 2.4 gave you the mechanics. This section answers the **so what**: why introduce a transport crate at all, when Stage 1 already “worked”?

The short answer: Stage 1 proved uProtocol **bytes** move over a socket. Stage 2 proves uProtocol **semantics** can sit in front of the wire — and that your application code can target a **stable L1 API** instead of re-implementing sockets in every binary.

### Benefit 1 — Application code speaks `UTransport` + `UListener`, not socket loops

In Stage 1, the subscriber’s `main` is a transport program in disguise:

```
accept → read_exact(4) → read_exact(N) → parse_from_bytes → application logic
```

The publisher’s `main` is the mirror image: build message → frame → connect → write → close. Both binaries embed **how** bytes move.

With `up-uds-transport`, that I/O choreography moves behind two familiar L1 entry points:

| Role | Stage 1 (today’s binaries) | Stage 2 target (2.7–2.8) |
|---|---|---|
| **Send** | Manual `UnixStream::connect` + `write_all` | `UTransport::send(message)` via `UdsTransportClient` |
| **Receive** | Manual `read_exact` loop in a spawned task | `UListener::on_receive(message)` after `register_listener` |

Your battery telemetry logic should care about **SoC and temperature**, not about whether the next four bytes are a Big-Endian length prefix. `UListener::on_receive` is the hook where that separation becomes real: the callback receives an already-decoded `UMessage`; socket reads and framing stay in the transport crate.

This matches how production uProtocol stacks are structured: entities implement listeners; transports move messages and invoke matching callbacks.

### Benefit 2 — Contracts are URI filters, not “whatever arrives on the socket”

Stage 1’s subscriber accepts **every** framed message on `/tmp/uprotocol_twin.sock`. There is no declared interest — if another process connected and sent bytes, the subscriber would try to decode them. The only “filter” is accidental: one publisher, one socket, one demo.

`up-uds-transport` makes the contract **explicit**. A listener registers with:

- a **source URI filter** (which publisher / resource / event it cares about), and
- an optional **sink URI filter** (for request/response patterns; `None` for our PUBLISH-only demo).

Dispatch uses the same matching rules as `up-rust`’s in-process `LocalTransport`:

```rust
// Simplified from up-uds-transport — dispatch only fires matching listeners
if registered.matches_msg(&message) {
    registered.on_receive(message.clone()).await;
}
```

A listener fires when `source_filter.matches(message.source)` and the sink side aligns. That means:

- The **thermal logging engine** from Stage 1’s thought experiment could register its **own** filter URI without sharing the battery subscriber’s code path — *once* fan-out exists at the transport layer (Stage 3).
- Even today, registering filters documents **intent**: “I subscribe to resource `0x8001` from entity `0x1010`,” not “I read whatever bytes show up.”

Stage 1 metadata (`UAttributes`, source `UUri`) was set but ignored on receive. The transport layer now **uses** that metadata for routing decisions.

### Benefit 3 — Same abstraction surface as production transports

`UTransport` is not a tutorial-only interface. It is the uProtocol L1 boundary in `up-rust` — the same trait implemented by:

- **`LocalTransport`** — in-process dispatch (used in tests and embedded scenarios)
- **Zenoh / MQTT / SOME/IP transports** — in full vehicle stacks (Stage 3 preview)

Our `UdsTransport` / `UdsTransportClient` are **another implementation of the same trait**. That is deliberate pedagogy: code written against `UTransport::send` and `register_listener` does not need to change when the wire underneath changes.

```
Stage 2 (this chapter)          Stage 3 (preview)
─────────────────────          ─────────────────
UdsTransportClient      →      Zenoh-backed UTransport
UdsTransport::serve     →      (same register_listener API)
        │                                │
        └──────── UTransport trait ──────┘
                 business logic unchanged
```

When we swap UDS for Zenoh, the publisher’s publish loop and the subscriber’s `on_receive` body should remain **semantically identical** — only configuration (transport plugin, endpoint) changes. Stage 1’s raw-socket code would not survive that swap without a rewrite.

### Benefit 4 — Transport concerns live in one crate, not in every binary

Stage 1 spreads transport knowledge across three places:

| Concern | Where it lives in Stage 1 |
|---|---|
| Length-prefix framing (send) | `up-frame-codec` + publisher `main` |
| Length-prefix framing (receive) | subscriber `main` (read loop + reassembly) |
| Socket bind / accept / connect | subscriber / publisher `main` |
| `UMessage` protobuf decode | subscriber `main` |
| URI-based dispatch | *nowhere* |

Stage 2 consolidates the **receive-side pipeline** in `up-uds-transport`:

```
Unix socket  →  read_framed_message  →  UMessage  →  filter match  →  UListener
                     ▲
                     └── uses up-frame-codec on send; parallel read/decode path on receive
```

Framing on **send** already goes through `serialize_for_unix_socket` in `up-frame-codec`. The transport crate completes the picture on **receive** and adds **listener dispatch** — concerns that do not belong in battery telemetry business logic.

After 2.7–2.8, the binaries shrink to:

- **Publisher:** build payload + call `send` (via L2 `SimplePublisher` helper).
- **Subscriber:** implement `on_receive`, register a filter once at startup.

Fix a framing bug once in `up-uds-transport` (or `up-frame-codec`), not in every application.

### What 2.5 does *not* claim

These benefits are about **API shape and separation of concerns** — not about solving Stage 1’s fan-out problem yet. We still have one socket, one subscriber process, and point-to-point UDS. Section **2.6** documents honestly where this design stops short and why Stage 3 needs a different wire.

### Checkpoint reminder

At **2.5**, the transport crate embodies these benefits, but the **demo binaries still run Stage 1 code**. Sections **2.7–2.8** wire the publisher and subscriber to `UdsTransport` so the running application matches the architecture described here.

---

## What comes next (2.6 onward)

| TODO | Topic |
|---|---|
| **2.6** | Tutorial: honest limits of `up-uds-transport` (fan-out, filesystem path, no L3) → Stage 3 setup |
| **2.7** | Refactor publisher: `SimplePublisher`, protobuf BMS payload, `UdsTransportClient` |
| **2.8** | Refactor subscriber: `UListener`, `UdsTransport::serve`, drop manual read loop |
| **2.9–2.10** | Intermediate evaluation, tag `Stage-2-Baseline`, update `README-Notes.md` |

---

## Key takeaway at 2.5

`up-uds-transport` is not extra boilerplate — it is the **uProtocol L1 seam** between “move bytes on a Unix socket” and “battery telemetry logic.” It gives you **`UTransport` + `UListener`**, **URI-filter contracts**, **production-parity APIs**, and **one place to own wire concerns**. The demo still runs as Stage 1 until 2.7–2.8; the *architecture* you are building toward is already visible in the crate.
