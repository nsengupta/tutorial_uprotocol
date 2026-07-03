# Stage 2: The Semantic Epiphany — uProtocol Layers and `up-uds-transport`

> **Prerequisites:** Stage 1 (`Stage-1.md`, code in `phases/01_raw_sockets/`, tag `Stage-1-Baseline`). Same UDS socket path, one subscriber, five messages then exit.

Stage 2 is where we stop treating uProtocol as “protobuf bytes on a wire” and start using its **layers and constituents** deliberately. We introduce **`UTransport`** over Unix Domain Sockets, refactor the publisher and subscriber around **`SimplePublisher`** / **`UListener`**, and close the RAW-payload gap with a shared **protobuf** schema — then pause honestly where UDS still fails (setup for Stage 3).

### Tutorial thesis (Phase 2 story arc)

Phase 1 proved that **`UMessage` bytes can move** over a local socket. That is necessary but not sufficient for an SDV. Phase 2 is the story of **use cases Phase 1 cannot support** — and how uProtocol’s own facilities address them:

| Phase 1 gap | Why it hurts | uProtocol facility in Phase 2 |
|---|---|---|
| Socket I/O duplicated in every binary | Business logic mixed with framing/connect | **L1:** `UTransport`, `up-uds-transport` |
| Subscriber reads “whatever” arrives | No declared interest / routing | **L1:** `register_listener`, URI filters, `UListener` |
| Envelope typed, payload opaque (RAW + DBC secret) | Receiver needs out-of-band schema | **Envelope + L2:** protobuf payload, `try_from_protobuf` / `extract_protobuf` |
| Manual `UMessage` / `UAttributes` assembly | Easy to get message type, TTL, source wrong | **L2:** `SimplePublisher`, `StaticUriProvider`, `CallOptions` |
| No path to production transport | Raw UDS code cannot swap to Zenoh | **L1 trait boundary** — same API, different plugin in Stage 3 |

Phase 2 **builds the case**, then **shows the code**. Section **2.6** and `Stage-3.md` record what still does *not* work (fan-out, location transparency, L3) — that is intentional cliffhanger, not failure of Phase 2.

> **Authoring note:** After §2.9–2.10 ship Stage 2, consolidate this chapter’s voice into `uProtocol-tutorial-draft-2.md` (draft-1 covers Phase 1).

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

We adopt L2 helpers in the refactored publisher (§2.7): `SimplePublisher`, `StaticUriProvider`, `CallOptions`.

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

Stage 1 showed that Protobuf types the **envelope**, not necessarily the **payload** when using `UPAYLOAD_FORMAT_RAW`. Stage 2.7 closes that gap with `BatteryTelemetry` in `up-bms-proto` (§2.7).

---

## 2.3 — Stage 1 → Stage 2 contrast

| Concern | Stage 1 (`Stage-1-Baseline`) | Stage 2 (shipped at `Stage-2-Baseline`) |
|---|---|---|
| **Transport API** | Raw `UnixStream` read/write in each binary | `UdsTransport` / `UdsTransportClient` implement `UTransport` |
| **Listener model** | Ad-hoc decode in spawned task | `register_listener` + `UListener::on_receive` |
| **URI usage** | Source set; no filter registration | Filter matching via `UUri::matches` in transport dispatch |
| **Framing** | Duplicated length-prefix logic in subscriber | Centralised in `up-uds-transport` (+ `up-frame-codec` on send) |
| **Payload typing** | RAW bytes + shared CAN layout secret | Protobuf `BatteryTelemetry` via `up-bms-proto` |
| **Message assembly** | Hand-built `UMessage` + `UAttributes` in publisher `main` | `SimplePublisher::publish` (L2 builds valid PUBLISH envelope) |
| **Publisher / subscriber binaries** | Stage 1 style | Refactored (§2.7–2.8) |

What changes across Phase 2 is both **plumbing** (`up-uds-transport`) and **application shape** (L2 publish, L1 listen, typed payload). Phase 1 code remains frozen under `phases/01_raw_sockets/` for side-by-side comparison.

### Phase 1 code folded into uProtocol APIs

Use this table when writing the tutorial or `uProtocol-tutorial-draft-2.md`: it maps **removed Phase 1 code** to **what replaced it** and **why**.

| Phase 1 (hand-rolled) | Phase 2 (uProtocol) | Why fold it |
|---|---|---|
| **`UUri { … }` literal in publisher `main`** | `StaticUriProvider::new(authority, ue_id, version)` + `get_resource_uri(resource_id)` | Authority and entity identity are **stable**; resource ID varies per event. Provider avoids copy-paste URI fields and matches subscriber filter construction. |
| **`UAttributes { type_, source, id, ttl, … }` + `UMessage { attributes, payload }`** | `SimplePublisher::publish(resource_id, CallOptions::for_publish(…), Some(payload))` | L2 **`UMessageBuilder`** (inside `SimplePublisher`) sets `UMESSAGE_TYPE_PUBLISH`, source URI, message ID, TTL correctly. Hand-assembly is error-prone and obscures intent: you said “publish” in comments but the type system did not. |
| **`UPayload::new(raw_bytes, UPAYLOAD_FORMAT_RAW)`** + `pack_bms_can_frame` | `BatteryTelemetry` + `UPayload::try_from_protobuf(telemetry)` | Payload format becomes **`UPAYLOAD_FORMAT_PROTOBUF_WRAPPED_IN_ANY`**; schema lives in `.proto`, not DBC offset folklore. Receiver uses `extract_protobuf`, not `unpack_bms_can_frame`. |
| **`serialize_for_unix_socket` + `UnixStream::connect` + `write_all`** | `UdsTransportClient::send` via `SimplePublisher` → `UTransport::send` | Framing and connect-send-close live in **one transport crate**; publisher `main` only builds telemetry and calls `publish`. |
| **`UnixListener::bind` + accept loop + `read_exact` length/body** | `UdsTransport::serve` + background read/decode in `up-uds-transport` | Stream framing is not subscriber business logic; centralising it matches production L1 separation. |
| **`UMessage::parse_from_bytes` in subscriber task** | Inside `up-uds-transport::read_framed_message` before dispatch | Subscriber receives **already-decoded** `UMessage` in `on_receive`. |
| **No routing — decode everything on socket** | `register_listener(source_filter, None, listener)` | Declares **interest** using the same URI model as production stacks; dispatch drops non-matching messages. |
| **Inline print/decode in spawned task** | `impl UListener { async fn on_receive(&self, msg: UMessage) }` | Testable, swappable callback; same hook Zenoh transport will invoke in Stage 3. |

#### Why we no longer create `UMessage` by hand

In Phase 1 the publisher constructed the full envelope explicitly:

```rust
// Phase 1 — publisher main (phases/01_raw_sockets/)
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
```

That taught the **constituents** (good for Phase 1). For Phase 2 the tutorial moves envelope construction to **L2** because:

1. **Correctness** — PUBLISH messages require consistent `type_`, `source`, optional `id`/`ttl`; `SimplePublisher` + `UMessageBuilder::publish` apply the same rules as `up-rust` production code.
2. **Separation of concerns** — Publisher `main` states *what* (resource `0x8001`, `BatteryTelemetry` values), not *how to wrap bytes in uProtocol*.
3. **Stability across transports** — When Stage 3 swaps `UdsTransportClient` for Zenoh, the publish call site stays; only transport construction changes.
4. **Pedagogy** — Phase 1 already showed raw constituents; Phase 2 shows **which layer you should code against** day to day (L2 publish, L1 listen).

The envelope still exists on the wire — you simply **stop being the assembly line** for it.

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
├── up-bms-proto/            # shared BatteryTelemetry .proto + demo constants
├── up-uds-transport/        # UTransport over UDS
├── up-battery-telemetry-publisher/   # SimplePublisher + protobuf payload (2.7)
└── up-telemetry-subscriber/          # UListener + serve (2.8)
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

## 2.6 — Where `up-uds-transport` still falls short (Stage 3 setup)

Section 2.5 argued for the transport crate on **API shape** grounds. This section is the honest counterweight: **`up-uds-transport` is pedagogically useful, not production SDV transport.** It teaches uProtocol L1 on a wire you already understand; it does **not** remove the architectural wall from Stage 1.

That wall matters because real vehicles add services continuously. Remember the **Thermal Management Logging Engine** from `Stage-1.md`: an independent microservice that must monitor the same battery temperature stream without sharing the battery subscriber's process or internal state. Stage 2 improves how we *express* interest in that stream (URI filters, `UListener`) — but the **wire underneath is still a single local Unix socket with point-to-point semantics**.

```
Stage 1 wall                         Stage 2 (2.4–2.5)                    Still blocked
────────────────                     ─────────────────                    ─────────────
1 publisher → 1 subscriber           UTransport + URI filters             Thermal engine
on /tmp/uprotocol_twin.sock          + listener dispatch in-process       still cannot tap
                                     (same socket, same topology)         the live stream
```

Do not mistake cleaner application APIs for a solved topology problem. The refactor buys **separation of concerns and production-parity interfaces**; the **execution path** is still bottlenecked by UDS.

### Limitation summary

| Limitation | Why it matters for SDVs |
|---|---|
| **Point-to-point UDS** | A `SOCK_STREAM` connection has one reader; bytes are consumed on read. No native fan-out — the Stage 1 second-consumer problem is **unresolved**. |
| **Filesystem socket path** | `/tmp/uprotocol_twin.sock` exists only on **this** machine. Useless when publisher and subscriber live on different ECUs or zones connected by Automotive Ethernet. |
| **No location transparency** | Publisher and subscriber must agree on a **literal path string** before any message flows. There is no "publish here, subscribe anywhere in the vehicle" story. |
| **No L3 discovery / registration** | `register_listener` is a **local, in-process** registration on the server handle. It is not vehicle-wide PUBLISH pattern registration — new entities cannot discover the battery telemetry event without out-of-band configuration. |
| **Underlying stream semantics** | We still rely on **length-prefix framing** (`up-frame-codec`) over a byte stream. There is no broker, no topic namespace, no built-in multi-subscriber delivery — only read-one-message-per-connection and in-process callback dispatch. |

Each row is intentional. Stage 2 fixes **how application code talks to transport**; Stage 3 must fix **where and how messages propagate**.

### Limitation 1 — Point-to-point UDS (fan-out still missing)

`UdsTransport::serve` accepts a connection, reads **one** framed `UMessage`, and dispatches to registered listeners **inside that server process**:

```rust
// up-uds-transport — one connection → one read → in-process dispatch
if let Ok(message) = read_framed_message(stream).await {
    transport.dispatch(message).await;  // callbacks in THIS process only
}
```

That is a real improvement over Stage 1's ad-hoc decode loop — but it is **not** network pub/sub. A second **process** (the thermal logger) cannot call `register_listener` on your subscriber's server handle. If it opened its own listener on the same socket path, it would **compete** for the publisher's connection, not share the stream.

Stage 1's pseudo-code broker trap (`Stage-1.md`) — subscriber becomes fan-out hub, manual stream lists, back-pressure in the hot path — remains the only UDS-native escape hatch. **`up-uds-transport` does not implement that broker**, and we should not pretend URI filters magically create multi-process fan-out. Filters route **within** a transport instance; they do not clone bytes to arbitrary peers.

**Stage 3 payoff:** a data-space transport (Zenoh) provides native multi-subscriber delivery so the thermal engine can subscribe independently — without turning the battery subscriber into a broker.

### Limitation 2 — Filesystem socket path (local machine only)

Our socket path is a **host-local filesystem name**:

```text
/tmp/uprotocol_twin.sock
```

That works for a laptop demo or two binaries on one Linux node. It **dies** the moment the battery telemetry **uEntity** moves to a Zone ECU and the display subscriber stays on a central compute node — a common SDV topology shift.

Filesystem UDS is not a vehicle network. There is no DNS, no service registry, no routing across VLANs. Configuration that hard-codes `/tmp/...` is a **development shortcut**, not a deployment model.

### Limitation 3 — No location transparency

**Location transparency** means publisher and subscriber code do not embed **where** the peer lives on the network. Today both sides must share:

1. The same socket path string, and
2. The same physical machine (or shared mount — not realistic across ECUs).

Contrast with Stage 3's direction: configure a **Zenoh-backed** `UTransport` with session endpoints; business logic still calls `send` and `register_listener`, but the wire resolves peers through the data space instead of a path baked into `main`.

```
Stage 2 (today)                          Stage 3 (preview)
─────────────────                        ─────────────────
publisher ──connect──► /tmp/...sock      publisher ──► Zenoh session
subscriber ◄──bind──── /tmp/...sock      subscriber ◄── same L1 API, different plugin
         ▲                                         ▲
         └── path is the "address"                 └── address is logical (URI + L3)
```

The **`UMessage` envelope and URI filters stay**; only the **physical binding** changes. That is the copy-forward promise of uProtocol — and why we invested in L1 abstractions before swapping wires.

### Limitation 4 — No L3 discovery / registration

Stage 2.1 previewed L3; here is what we **do not** have yet:

| Mechanism | Stage 2 (`up-uds-transport`) | Production uProtocol stack |
|---|---|---|
| Listener registration | `UdsTransport::register_listener` on the local server | L3 PUBLISH registration over a vehicle data space |
| Service discovery | None — subscribers must know the socket path and URI filters upfront | Entities discover publishers by URI / service ID |
| Dynamic join | Adding a consumer requires code/config changes on the server side | New subscribers attach without modifying the publisher |

Our listener table is an **in-memory `HashSet`** inside one process. It is the same *shape* as `LocalTransport` in `up-rust` — correct for teaching L1 dispatch rules, insufficient for a moving vehicle with services joining and leaving at runtime.

**Spec pointer:** [uProtocol L3 — Discovery & Registration](https://github.com/eclipse-uprotocol/up-spec/blob/main/up-l3/README.adoc)

### Limitation 5 — Stream semantics, not broker semantics

Unix domain sockets expose a **byte stream**, not messages. We bolt on message boundaries with a 4-byte length prefix (`up-frame-codec`). The transport crate centralises that read path — good — but the **semantic model** is still:

```
connect → read framed blob → decode UMessage → dispatch → close (publisher side per send)
```

There is no:

- Durable topic or key namespace shared across the vehicle
- Built-in quality-of-service or retention
- Automatic fan-out to N subscribers
- Decoupling of publisher lifetime from subscriber presence

Production SDV stacks use **broker-capable transports** (Zenoh, MQTT, SOME/IP with appropriate patterns, etc.) for these reasons. Our tutorial crate deliberately stays minimal: one subscriber, five messages, exit — scope guardrails unchanged.

### What Stage 3 changes vs what Stage 2 keeps

| Layer | Stage 2 (keep) | Stage 3 (replace wire) |
|---|---|---|
| **Envelope** | `UMessage`, `UAttributes`, `UUri`, `UPayload` | Unchanged |
| **L2 patterns** | `SimplePublisher`, typed protobuf payload (after 2.7) | Unchanged |
| **L1 API** | `UTransport::send`, `register_listener`, `UListener::on_receive` | **Same trait** — different plugin (`up-client-zenoh-rust`) |
| **Physical transport** | UDS + length framing | Zenoh (network-transparent) |
| **L3** | Not used | PUBLISH registration for vehicle-wide discovery |
| **Demo scope** | One battery subscriber | Thermal engine + fan-out payoff |

**Stage 3 replaces transport execution; Stage 2 semantics stay.** That sentence is the bridge to the next chapter: you are not relearning uProtocol — you are **unblocking topology**. Full handoff notes live in **`Stage-3.md`** (planning brief for the Stage 3 chapter).

### Checkpoint reminder

At **2.6**, we have documented both sides of the story: **2.5** (why L1 abstractions are worth it) and **2.6** (why they are not enough). Sections **2.7–2.8** (below) wire the demo binaries to that architecture. No Thermal crate yet; no Zenoh yet — by design.

---

## 2.7 — Refactor the publisher: typed protobuf + `SimplePublisher`

Stage 1 packed SoC and temperature into a fictional CAN byte layout (`pack_bms_can_frame`) and marked the payload `UPAYLOAD_FORMAT_RAW`. That worked for a demo, but it recreated the **shared secret** problem: meaning lived outside the uProtocol envelope.

Stage 2.7 closes the gap with a **protobuf application payload** and routes sends through **`SimplePublisher`** + **`UdsTransportClient`**.

### Shared schema crate: `up-bms-proto`

Publisher and subscriber must agree on payload shape. Rather than duplicating offset math in two binaries, we add a small shared crate:

```
phases/02_uprotocol_semantics/crates/up-bms-proto/
├── proto/bms_telemetry.proto   # canonical schema (human-readable contract)
├── build.rs                    # compiles .proto → Rust at build time
└── src/lib.rs                  # re-exports generated types + demo constants
```

**Why a separate crate?**

| Reason | Explanation |
|---|---|
| **Single contract** | One `.proto` file is the source of truth for SoC + temperature — both sides include the same generated Rust types. |
| **Tutorial visibility** | Readers can open `bms_telemetry.proto` without spelunking generated code. |
| **Stage 1 contrast** | RAW bytes required a DBC-style secret; protobuf makes the payload **self-describing** via `UPayloadFormat`. |
| **Scope guardrail** | Only battery SoC and cell temperature here — thermal logging and other consumers arrive in Stage 3. |

The schema is intentionally minimal:

```protobuf
// proto/bms_telemetry.proto
syntax = "proto3";
package tutorial.bms.v1;

message BatteryTelemetry {
  float soc_percent = 1;
  int32 temp_celsius = 2;
}
```

Shared demo constants (socket path, URI identity, message count) live in `up_bms_proto::constants` so publisher and subscriber cannot drift apart silently.

### Why `build.rs` matters

Rust does not compile `.proto` files natively. **`build.rs` is Cargo's hook that runs before your crate compiles** and generates Rust source into `OUT_DIR`:

```rust
// up-bms-proto/build.rs (simplified)
protobuf_codegen::Codegen::new()
    .protoc()
    .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
    .include("proto")
    .input("proto/bms_telemetry.proto")
    .cargo_out_dir("gen")
    .run_from_script();
```

What this gives you:

1. **`protoc`** reads `bms_telemetry.proto` and emits Rust structs implementing `protobuf::Message`.
2. **`protoc-bin-vendored`** ships a known `protoc` binary — tutorial builds do not depend on a system install.
3. **`src/lib.rs`** pulls generated code in via `include!(concat!(env!("OUT_DIR"), "/gen/mod.rs"))`.

If you change field numbers or add fields in the `.proto`, **`cargo build` regenerates types automatically**. That is the same pattern `up-rust` uses for uProtocol core messages — we mirror it at tutorial scale so readers recognise the workflow.

### Publisher flow after 2.7

```
BatteryTelemetry (protobuf)  →  UPayload::try_from_protobuf  →  SimplePublisher::publish
                                                                        │
                                                                        ▼
                                                              UdsTransportClient::send
```

Key code shape:

```rust
let uri_provider = Arc::new(StaticUriProvider::new(
    AUTHORITY_NAME, PUBLISHER_UE_ID, PUBLISHER_UE_VERSION,
));
let transport: Arc<dyn UTransport> = Arc::new(UdsTransportClient::new(SOCKET_PATH));
let publisher = SimplePublisher::new(transport, uri_provider);

let payload = UPayload::try_from_protobuf(telemetry)?;
publisher
    .publish(BATTERY_TELEMETRY_RESOURCE_ID, CallOptions::for_publish(None, None, None), Some(payload))
    .await?;
```

Gone from the publisher: manual `UnixStream::connect`, `serialize_for_unix_socket`, and CAN offset packing. **`StaticUriProvider`** keeps URI construction consistent with the subscriber's filter.

---

## 2.8 — Refactor the subscriber: `UListener` + exit after five messages

The subscriber drops its inline read loop and implements uProtocol's receive callback instead.

### Before vs after

| Concern | Stage 1 subscriber | Stage 2.8 subscriber |
|---|---|---|
| Socket bind / accept | `UnixListener` in `main` | `UdsTransport::serve(SOCKET_PATH)` |
| Framing / decode | Manual `read_exact` + `parse_from_bytes` | Inside `up-uds-transport` |
| Routing | Accept everything on the socket | `register_listener` with source URI filter |
| Application logic | Mixed into spawned read task | `UListener::on_receive` |
| Payload | `unpack_bms_can_frame` offset math | `msg.extract_protobuf::<BatteryTelemetry>()` |
| Shutdown | Infinite loop | Count to `EXPECTED_MESSAGE_COUNT` (5), then exit |

### `UListener` implementation

```rust
#[async_trait]
impl UListener for BatteryTelemetryListener {
    async fn on_receive(&self, msg: UMessage) {
        if let Ok(telemetry) = msg.extract_protobuf::<BatteryTelemetry>() {
            // print SoC / temperature ...
            if self.received.fetch_add(1, Ordering::SeqCst) + 1 >= EXPECTED_MESSAGE_COUNT {
                self.shutdown.notify_one();
            }
        }
    }
}
```

Startup registers the listener against the **same resource URI** the publisher uses:

```rust
let transport = UdsTransport::serve(SOCKET_PATH).await?;
transport
    .register_listener(&source_filter, None, listener)
    .await?;

shutdown.notified().await;  // unblock main after 5 callbacks
```

The accept/dispatch loop keeps running in the transport crate's background task — we simply **exit the process** once business logic has seen five messages (scope guardrail unchanged from Stage 1).

### Verify the refactored demo

Two terminals, subscriber first:

```bash
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```

Expected: five telemetry lines on the subscriber, then `Received 5 messages — exiting.`

---

## 2.9 — Evaluate the intermediate state (Stage 2 synthesis)

Section 2.9 is the **chapter close** before tagging `Stage-2-Baseline`. It does not add crates; it states what a reader should believe after running Phase 2 code.

### Before / after architecture (Phase 1 → Phase 2)

Same topology (one publisher, one subscriber, UDS, five messages). **What moved** is *where* each concern lives:

```
PHASE 1 — phases/01_raw_sockets/          PHASE 2 — phases/02_uprotocol_semantics/
────────────────────────────────          ────────────────────────────────────────

┌─────────────────────────┐               ┌─────────────────────────┐
│ publisher main          │               │ publisher main          │
│ · hand-built UMessage   │               │ · BatteryTelemetry      │
│ · RAW CAN pack          │               │ · SimplePublisher       │
│ · UnixStream connect    │               │   (L2 — no UMessage     │
│ · up-frame-codec send   │               │    by hand)             │
└───────────┬─────────────┘               └───────────┬─────────────┘
            │ write (framed)                          │ UTransport::send
            ▼                                         ▼
     /tmp/uprotocol_twin.sock                   ┌─────────────────────────┐
            ▲                                   │ up-uds-transport        │
            │ read loop in main                 │ · frame + decode        │
┌───────────┴─────────────┐                     │ · URI filter dispatch   │
│ subscriber main         │                     └───────────┬─────────────┘
│ · read_exact framing    │                                 │
│ · parse_from_bytes      │               ┌─────────────────▼─────────┐
│ · unpack_bms_can_frame  │               │ subscriber main           │
│ · infinite accept loop  │               │ · UListener::on_receive │
└─────────────────────────┘               │ · extract_protobuf      │
                                          │ · exit after 5 msgs     │
  Transport + routing in app code           └─────────────────────────┘
  Payload = opaque RAW bytes                  L1/L2 in uProtocol APIs
                                              Payload = up-bms-proto
```

**Unchanged wire (deliberate):** still point-to-point UDS on `/tmp/uprotocol_twin.sock`. Phase 2 refactors **layers above the socket**; Phase 3 replaces the bottom row.

### What Phase 2 made feasible (that Phase 1 could not)

| Capability | Evidence in code |
|---|---|
| Declarative subscribe intent | `register_listener` with source URI filter on resource `0x8001` |
| Typed application payload without DBC secrets | `up-bms-proto` + `extract_protobuf::<BatteryTelemetry>()` |
| Transport-pluggable publish/receive | `UdsTransportClient` / `UdsTransport::serve` behind `UTransport` trait |
| L2 publish without hand-built `UMessage` | `SimplePublisher::publish` + `StaticUriProvider` |
| Single place to fix framing/dispatch | `up-uds-transport` crate |

### What Phase 2 still cannot do (input for Stage 3)

Same five rows as §2.6 — summarised here as the **cliffhanger checklist**:

1. **Fan-out** — thermal logger still cannot attach independently.
2. **Cross-host** — `/tmp/uprotocol_twin.sock` dies if publisher moves to another ECU.
3. **Location transparency** — both sides still share a literal filesystem path.
4. **L3 discovery** — listeners are in-process registrations, not vehicle-wide.
5. **Broker semantics** — still length-framed stream over UDS, not a data space.

See **`Stage-3.md`** for the structured author brief (topology trigger, migration sketch, roadmap links).

### Reader takeaway (for `uProtocol-tutorial-draft-2.md`)

> Phase 2 is not “UDS with extra steps.” It is **learning which uProtocol layer to implement yourself** versus **which layer the stack provides**. You implemented battery logic; uProtocol L1/L2 implemented routing contracts and envelope discipline. The wire is still wrong for a vehicle — **on purpose** — so Stage 3’s Zenoh swap has something to prove.

---

## 2.10 — Ship Stage 2 (`Stage-2-Baseline`)

Stage 2 is **code- and narrative-complete** at this section. Remaining step for maintainers: **commit and tag** (not done in-repo by automation — you choose when to push).

### Verification (2026-07-02)

| Check | Command | Result |
|---|---|---|
| Workspace build | `cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml` | OK |
| Transport unit test | `cargo test --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-uds-transport` | 1 passed |
| 5-message demo | subscriber first, then publisher | 5 telemetry lines; subscriber `Received 5 messages — exiting.`; exit 0 |

Optional trace output (transport path confirmation):

```bash
RUST_LOG=trace cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber
RUST_LOG=trace cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```

### Tag checkpoint (maintainer)

When ready to freeze Stage 2 on `main`:

```bash
git tag -a Stage-2-Baseline -m "Stage 2: uProtocol semantics over UDS (phases/02_uprotocol_semantics/)"
git push origin Stage-2-Baseline   # when you choose to push
```

Checkout snapshot:

```bash
git checkout Stage-2-Baseline
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
git checkout main
```

### Next authoring step

Consolidate Phase 2 prose into **`uProtocol-tutorial-draft-2.md`** (draft-1 remains Phase 1). Planning brief for Phase 3 code: **`Stage-3.md`**.

---

## Key takeaway at Stage 2

Phase 2 completes the **semantic** story: routing contracts are well-defined, payloads are typed, and application code targets **`UTransport` + `UListener` + L2 publish**. **Transport execution** is still bottlenecked by point-to-point UDS — documented in §2.6 and **`Stage-3.md`**. That split is the deliberate chapter ending: improve the API surface first, swap the wire in Stage 3 without rewriting battery logic.
