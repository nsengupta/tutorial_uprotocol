### Phase 3: The Scaling Limit & The Zenoh Payoff

> **Prerequisites:** Phase 2 (`tutorial-text/Tutorial-Phase-2.md`, code in `phases/02_uprotocol_semantics/`).  
> **Active code:** `phases/03_zenoh_topology/` — copy-forward from Phase 2.

Phase 2 ended with an honest confession: UDS works for one publisher and one subscriber on a single Linux host, but it breaks the moment you need a second consumer — even on the same machine. Phase 3 swaps the wire for **Zenoh** and delivers the fan-out payoff that Phase 1's thermal logger narrative promised.

**Demo scope:** All processes run on **one Linux host** (separate terminals), same as Phases 1–2. The cross-ECU vehicle story is narrative only — we do not simulate multiple machines.

Let's dive in.

---

### Chapter 1: Why UDS had to retire (recap)

Phase 2's `up-uds-transport` solved the right problem: separating business logic from socket I/O using `UTransport`. But the UDS wire itself still had three hard limits **on one host**:

| Limit                                        | What broke                                                   | Why UDS cannot fix it                                                   |
| -------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------- |
| **1 — No fan-out**                           | Two subscriber processes cannot both receive the same stream | UDS is point-to-point; the server process owns the socket               |
| **2 — Filesystem path, not logical address** | `/tmp/uprotocol_twin.sock` is a filename, not a URI          | Two processes cannot `bind()` the same path; only one server can listen |
| **3 — No location transparency**             | Publisher and subscriber must share a kernel                 | A socket path on one ECU's filesystem is invisible on another           |

These limits are **independent of topology**. They bite you on a single laptop, which is why Phase 3 retires `up-uds-transport` even before we mention cross-ECU.

Let's draw the Phase 2 bottleneck so we can see it:

```
Same Linux host — Phase 2 UDS (still broken for fan-out)

  up-battery-telemetry-publisher          up-telemetry-subscriber
         │                                      ▲
         │  one UDS connection                  │ UdsTransport::serve
         └──────────────► /tmp/...sock ─────────┘ (one server process)

  up-thermal-logging-subscriber (what we want in Phase 3)
         │
         └── cannot share the stream — second process has no socket to read
             (Phase 1 broker pseudo-code trap all over again)
```
| Need on one host                         | UDS + `up-uds-transport`                    | Data-space transport (Zenoh)               |
| ---------------------------------------- | ------------------------------------------- | ------------------------------------------ |
| Second **process** as subscriber         | Not supported — turn subscriber into broker | Native pub/sub fan-out                     |
| Publisher unchanged when consumer added  | N/A — architectural hack                    | Subscribe independently                    |
| URI filter per consumer                  | In-process only                             | Each process registers its own `UListener` |
| Avoid manual re-serialize / forward loop | Fails — Phase 1 pseudo-code                 | Transport handles distribution             |

**We do not need a second machine to hit this wall.** Phase 1 introduced the thermal logging process as a second consumer on the same vehicle; Phase 2 made URI interest declarative but did not deliver bytes to a second process. That alone justifies retiring UDS for Phase 3 — before anyone moves an ECU.



> We are not exploring multiple Zone ECUs in this tutorial, intentionally. That's a very different area altogether. However, networked Zone ECUs is behaviorally similar to applications running on different Linux hosts. And, the aforementioned problem remains the same.



---

### Chapter 2: Zenoh as data-space transport

[Zenoh](https://zenoh.io/) is a data-space transport: publish/subscribe with location transparency, broker-capable distribution, and native pub/sub fan-out. Eclipse uProtocol provides [`up-client-zenoh-rust`](https://github.com/eclipse-uprotocol/up-client-zenoh-rust) — a `UTransport` implementation backed by Zenoh.

Zenoh decouples **who sends** from **who receives**:

```
Phase 2 — UDS (one server owns the socket)

  Publisher ───► UdsTransport::serve ───► one subscriber
                  (single process)


Phase 3 — Zenoh (data space, no single-owner socket)

  Publisher ───► Zenoh router ──┬──► subscriber 1 (battery telemetry)
                                └──► subscriber 2 (thermal logging)
```
Every process connects to a **local Zenoh router** (a lightweight daemon). The router handles distribution. Subscribers register `UListener` callbacks on URI filters — same API as Phase 2, but different transport plugin.

#### UDS vs Zenoh — five limits compared

| Dimension              | Phase 2 — UDS (`up-uds-transport`)                    | Phase 3 — Zenoh (`up-client-zenoh-rust`)                    |
| ---------------------- | ----------------------------------------------------- | ----------------------------------------------------------- |
| **Fan-out**            | Point-to-point; one server, one stream per connection | Native pub/sub; N subscribers receive the same message      |
| **Address**            | Filesystem path (`/tmp/…`) — single host              | URI + data space — network-native, no filesystem dependency |
| **Listener ownership** | In-process `register_listener` table                  | Each process registers independently with the Zenoh router  |
| **Framing**            | Manual 4-byte length prefix (`up-frame-codec`)        | Broker handles message boundaries                           |
| **Cross-host**         | Impossible — no shared kernel                         | Location transparent — same API across ECUs                 |

The result: **Phase 2's business logic survives unchanged.** Only the transport plugin and session configuration change.

#### What Zenoh is not

- **Not a replacement for uProtocol** — Zenoh is the *wire plugin* for uProtocol L1. uProtocol's `UUri`, `UAttributes`, `UMessage`, and `UListener` stay the same.
- **Not a COVESA deep dive** — we use Zenoh as a data-space transport; we do not explore its full feature surface.
- **Not a multi-host requirement** — the entire Phase 3 demo runs on one Linux host with a local Zenoh router.

---

### Chapter 3: What uProtocol puts alongside the actual message

Before we look at the code swap, let's understand what travels *with* each message. This matters because Zenoh uses this metadata in a way UDS could not.

uProtocol never sends a raw protobuf payload on the wire. Every `UMessage` carries **three layers of metadata** alongside the bytes:

| Layer                    | What it contains                                                                                             | Populated by                            |
| ------------------------ | ------------------------------------------------------------------------------------------------------------ | --------------------------------------- |
| **UUri (source + sink)** | Authority name, uEntity ID, uEntity version, resource ID — the fully-qualified "who and what" of the message | `SimplePublisher` / `StaticUriProvider` |
| **UAttributes**          | Priority, TTL, message type (PUBLISH, REQUEST, RESPONSE, etc.), correlation ID, timestamps                   | `CallOptions` + transport               |
| **UPayload**             | The application bytes (here, the serialised `BatteryTelemetry` protobuf) plus a format hint                  | `UPayload::try_from_protobuf`           |

When our publisher calls:

```rust
publisher.publish(BATTERY_TELEMETRY_RESOURCE_ID, CallOptions::for_publish(...), Some(payload))
```
`SimplePublisher` wraps the protobuf bytes into a `UMessage` with `UAttributes` (type=PUBLISH, TTL=5000ms) and a fully-qualified `UUri` (authority=`local_vehicle`, entity=`0x1010`, resource=`0x8001`). The resulting struct looks conceptually like:

```text
UMessage {
  attributes: UAttributes {
    type: PUBLISH,
    ttl: 5000,
    priority: STANDARD,
    source: UUri { authority: "local_vehicle", ue_id: 0x1010, ue_version: 1, resource_id: 0x8001 },
    sink: UUri { authority: "local_vehicle", ue_id: 0xFFFF, ue_version: 255, resource_id: 0x0000 },
  },
  payload: UPayload { value: [protobuf bytes], format: PROTOBUF },
}
```
This is not "raw data plus a header." It is a **self-describing message envelope** designed to be routed, filtered, and inspected without deserializing the payload.

#### Why UDS did not (and could not) make use of this metadata

In Phase 2, the `UMessage` envelope was assembled and serialised by `SimplePublisher`, then given to `UdsTransportClient::send`. But the UDS transport treats the envelope as **opaque bytes to stream over a socket**:

```text
UDS flow (Phase 2)

  Publisher                    UdsTransportClient          UdsTransport::serve          Subscriber
     │                                 │                          │                          │
     │──UMessage (UUri+Attr+Payload)──►│                          │                          │
     │                                 │──length-prefixed bytes──►│                          │
     │                                 │   (via up-frame-codec)   │──de-frame + UMessage─────►│
     │                                 │                          │──match UUri against──────►│
     │                                 │                          │   local listener table    │ on_receive
```
Key point: the UDS transport does **not** inspect the `UUri` or `UAttributes` to route the message. The metadata is embedded in the serialised bytes but is functionally dead weight for UDS — the socket path `/tmp/uprotocol_twin.sock` already decided the destination. UDS delivers everything to whoever is listening on that one path, regardless of what the `UUri` says. The `register_listener` matching happens in the **subscriber's own process**, inside `UdsTransport`'s local `HashMap` — it is not used by the wire.

> **UDS cannot use the uProtocol metadata for routing** because UDS has no routing layer. There is one socket, one connection, one direction. The `UUri` is present in the serialised bytes but plays no role in transport-level delivery.

#### How Zenoh makes the metadata useful

Zenoh is a **content-aware router**, not a byte pipe. When `up-transport-zenoh` receives a `UMessage` via `UTransport::send`, it does *not* forward opaque bytes. Instead:

1. It **reads the `UUri`** from the envelope and maps it to a Zenoh key expression — e.g. `local_vehicle/0x1010/1/0x8001`.
2. It **publishes** the payload bytes tagged with that key expression into the Zenoh data space.
3. On the subscriber side, `UPTransportZenoh` registers a **Zenoh subscriber** on the key expression matching the listener's `source_filter`.
4. When a Zenoh publication arrives, it reconstructs the `UMessage` and calls `UListener::on_receive`.

```text
Zenoh flow (Phase 3)

  Publisher                    UPTransportZenoh         Zenoh router (zenohd)       UPTransportZenoh          Subscriber
     │                                 │                          │                          │                    │
     │──UMessage (UUri+Attr+Payload)──►│                          │                          │                    │
     │                                 │──extract UUri────────    │                          │                    │
     │                                 │──publish(ZenohKey,──►────►──fan-out to matching────►────reconstruct─────► on_receive
     │                                 │   payload)               │   subscribers            │   UMessage         │
     │                                 │                          │                          │                    │
     │                                 │                          ├──fan-out to thermal──────►────reconstruct─────► on_receive
     │                                 │                          │   subscriber             │   UMessage         |      
```
**What changed:** the `UUri` and `UAttributes` that were opaque bytes on a UDS socket are now **first-class routing information** for the Zenoh data space. The Zenoh router matches key expressions to subscribers ; very similar to the way the UDS layer used the `UUri` <u>only inside</u> its own `HashMap`, but now the matching happens **in the network**.

|                   | UDS (Phase 2)                                    | Zenoh (Phase 3)                                           |
| ----------------- | ------------------------------------------------ | --------------------------------------------------------- |
| Metadata location | Encoded in opaque bytes                          | Inspected by transport, exposed in Zenoh key              |
| Routing           | Socket path decides destination                  | `UUri`-derived key expression decides fan-out             |
| Multi-subscriber  | Impossible (one server)                          | Native — each subscriber gets its own Zenoh subscription  |
| Listener matching | Local `HashMap` in `UdsTransport::serve` process | On the Zenoh router — any connected process can subscribe |

**This is why uProtocol is designed to work with routers like Zenoh.** The envelope (`UUri` + `UAttributes` + `UPayload`) is not a wire format chosen for convenience — it is a **routing contract** that a content-aware transport can act on. UDS ignored that contract because it did not need to route; it only needed to stream. Zenoh *uses* the contract, and the result is fan-out, location transparency, and zero metadata re-invention by the application programmer.

---

### Chapter 4: Code — swapping UDS for Zenoh

The Phase 3 workspace (`phases/03_zenoh_topology/`) is a copy-forward from Phase 2 with two removals and one addition:

| Retained from Phase 2                                          | Removed                      | Added                                              |
| -------------------------------------------------------------- | ---------------------------- | -------------------------------------------------- |
| `up-bms-proto` (protobuf schema, constants)                    | `up-uds-transport` — retired | `up-client-zenoh-rust` — Zenoh-backed `UTransport` |
| `up-battery-telemetry-publisher` (same `SimplePublisher` loop) | `up-frame-codec` — retired   |                                                    |
| `up-telemetry-subscriber` (same `UListener::on_receive` body)  |                              |                                                    |

The only code change in the publisher and subscriber is **how the `UTransport` object is constructed**:

```rust
// Phase 2 (UDS) — now retired
// let transport: Arc<dyn UTransport> = Arc::new(UdsTransportClient::new(SOCKET_PATH));

// Phase 3 (Zenoh)
let zenoh_config = zenoh::Config::default();
let session = zenoh::open(zenoh_config).await?;
let transport: Arc<dyn UTransport> = Arc::new(ZenohTransport::new(session).await?);
```
Everything after that — `SimplePublisher::publish`, `register_listener`, `on_receive` — is unchanged.

#### Publisher (same loop, Zenoh transport)

```rust
// up-battery-telemetry-publisher/src/main.rs — Phase 3
use up_client_zenoh_rust::ZenohTransportClient;

let transport: Arc<dyn UTransport> = Arc::new(ZenohTransportClient::new(config).await?);
let publisher = SimplePublisher::new(transport, uri_provider);

// The publish loop is identical to Phase 2 — see Tutorial-Phase-2 Chapter 7
for i in 1..=EXPECTED_MESSAGE_COUNT {
    let telemetry = BatteryTelemetry { /* … */ };
    let payload = UPayload::try_from_protobuf(telemetry)?;
    publisher.publish(BATTERY_TELEMETRY_RESOURCE_ID, CallOptions::for_publish(Some(5000), None, None), Some(payload)).await?;
}
```
#### Subscriber (same on_receive, Zenoh transport)

```rust
// up-telemetry-subscriber/src/main.rs — Phase 3
use up_client_zenoh_rust::ZenohTransportListener;

let transport = ZenohTransportListener::new(config).await?;
transport
    .register_listener(&source_filter, None, listener)
    .await?;

// on_receive body is identical to Phase 2 — see Tutorial-Phase-2 Chapter 8
```
The URI filter (`source_filter` with resource ID `0x8001`) works identically. The Zenoh router delivers only matching messages to each subscriber.

#### Transport config — `SOCKET_PATH` replaced

Phase 2's `SOCKET_PATH` constant (`/tmp/uprotocol_twin.sock`) is **legacy** — it exists in `up-bms-proto::constants` for historical comparison only. Phase 3 uses Zenoh session configuration:

```rust
// config for local Zenoh router — default UDP multicast scouting
let config = zenoh::Config::default();
let session = zenoh::open(config).await?;
```
If we wanted to connect to a remote Zenoh router instead, we would set an endpoint:

```rust
let mut config = zenoh::Config::default();
config.connect.endpoints = vec!["tcp/192.168.1.100:7447".parse()?];
let session = zenoh::open(config).await?;
```
On a single-host demo, the default config works — Zenoh discovers the local router via UDP multicast scouting: no path, port, or shared filesystem.

#### What stayed exactly as Phase 2

| Component                       | Phase 2                                       | Phase 3       |
| ------------------------------- | --------------------------------------------- | ------------- |
| `SimplePublisher::publish` loop | `publish(resource_id, call_options, payload)` | **Identical** |
| `UListener::on_receive` body    | `extract_protobuf::<BatteryTelemetry>()`      | **Identical** |
| URI filter                      | `source_filter` via `StaticUriProvider`       | **Identical** |
| `up-bms-proto` constants        | `AUTHORITY_NAME`, `PUBLISHER_UE_ID`, etc.     | **Identical** |
| `BatteryTelemetry` proto        | protobuf schema — SoC + temp                  | **Identical** |

---

### Chapter 5: L3 registration and the thermal fan-out payoff

Phase 1 mentioned the **Thermal Management Logging Application** as a second consumer of the battery telemetry stream. It checks the temperature of the batteries and prints a warning if the temperature is over 25*c! 

If we had implemented it, the loggiin application would have to run in the same process as telemetry subscriber, because UDS was point-to-point. Phase 3 delivers what Phase 1 only described: an **independent thermal logging subscriber**. it is a separate application, that receives the <u>exact same</u> protobuf messages from the Zenoh data space.

#### The thermal subscriber

```rust
// up-thermal-logging-subscriber/src/main.rs — new in Phase 3
use up_bms_proto::BatteryTelemetry;
use up_rust::{UListener, UMessage, UTransport};

struct ThermalLoggingListener;

#[async_trait]
impl UListener for ThermalLoggingListener {
    async fn on_receive(&self, msg: UMessage) {
        if let Ok(telemetry) = msg.extract_protobuf::<BatteryTelemetry>() {
            let temp = telemetry.temp_celsius;
            if temp > 25 {
                println!("⚠️  WARNING — cell temperature {temp}°C exceeds threshold");
            } else {
                println!("Cell temperature {temp}°C — OK");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let zenoh_config = zenoh::Config::default();
    let session = zenoh::open(zenoh_config).await?;
    let transport: Arc<dyn UTransport> = Arc::new(ZenohTransport::new(session).await?);
    let source_filter = /* resource URI for BATTERY_TELEMETRY_RESOURCE_ID */;
    transport
        .register_listener(&source_filter, None, Arc::new(ThermalLoggingListener))
        .await?;
    // … wait for EXPECTED_MESSAGE_COUNT …
}
```
This listener extracts the same `BatteryTelemetry` struct as the battery subscriber, but applies **thermal-specific logic**. We observe that there is no new schema; no socket path and no shared process.

#### L3 PUBLISH registration — what makes discovery work

Phase 2's `register_listener` was a **local operation**: it inserted the listener into an in-process hash table inside `UdsTransport::serve`. Only the process that owned the server socket could register.

Phase 3 uses Zenoh's data space. When a publisher sends a message with resource ID `0x8001`, the Zenoh router delivers it to **all processes** that registered a listener on that URI filter (showed interest in that particular resource), regardless of which process they live in. This is called **L3 PUBLISH pattern registration** in uProtocol's terminology:

| Layer                    | Phase 2 (UDS)                          | Phase 3 (Zenoh)                                               |
| ------------------------ | -------------------------------------- | ------------------------------------------------------------- |
| L1 (transport)           | UDS stream, length-prefix framed       | Zenoh pub/sub, broker-delivered                               |
| L2 (comm)                | `SimplePublisher`, `CallOptions`       | **Same API** — unchanged                                      |
| L3 (routing / discovery) | Local listener table in server process | Data-space routing — any process on the network can subscribe |

In practice, the application code is the same — `register_listener` with a `UUri` filter. What changed is the **transport plugin underneath**: Phase 2's plugin could only deliver to listeners in the same process; Phase 3's plugin delivers to any process connected to the same Zenoh router.

---

### Chapter 6: Running the multi-subscriber demo

We need four terminals (or four tmux panes) — one for the Zenoh router, one for the publisher, two for the subscribers:

```bash
# Terminal 1 — Zenoh router (start this first)
zenohd

# Terminal 2 — battery publisher
cargo run --manifest-path phases/03_zenoh_topology/Cargo.toml -p up-battery-telemetry-publisher

# Terminal 3 — battery telemetry subscriber
cargo run --manifest-path phases/03_zenoh_topology/Cargo.toml -p up-telemetry-subscriber

# Terminal 4 — thermal logging subscriber (new in Phase 3!)
cargo run --manifest-path phases/03_zenoh_topology/Cargo.toml -p up-thermal-logging-subscriber
```
The publisher sends five messages (same as Phases 1–2). Both subscribers receive all five. No process shares a socket, a listener table, or a filesystem path.

Build the whole workspace:

```bash
cargo build --manifest-path phases/03_zenoh_topology/Cargo.toml
```
---

### Chapter 7: A look at what we have done

Let's look back at the architecture:

```
╔═════════════════════════════════════════════════════════════╗
║ Publisher (Phase 3)                                         ║
║                                                             ║
║   battery_pct, temp_c                                       ║
║      ↓                                                      ║
║   BatteryTelemetry { soc_percent, temp_celsius }            ║
║      ↓                                                      ║
║   UPayload::try_from_protobuf → PUBLISH format              ║
║      ↓                                                      ║
║   SimplePublisher::publish(resource_id, ...)                ║
║      ↓                                                      ║
║   ZenohTransport::send  →  Zenoh data space                 ║
╚═════════════════════════════════════════════════════════════╝

╔═════════════════════════════════════════════════════════════╗
║ Battery Subscriber (Phase 3)                                ║
║                                                             ║
║   Zenoh data space  →  ZenohTransport                       ║
║      ↓                                                      ║
║   filter match → UListener::on_receive(msg)                 ║
║      ↓                                                      ║
║   msg.extract_protobuf::<BatteryTelemetry>()                ║
║      ↓                                                      ║
║   SoC: 76.3%, Temp: 22°C                                    ║
╚═════════════════════════════════════════════════════════════╝

╔═════════════════════════════════════════════════════════════╗
║ Thermal Subscriber (Phase 3 — new)                          ║
║                                                             ║
║   Zenoh data space  →  ZenohTransport                       ║
║      ↓                                                      ║
║   filter match → UListener::on_receive(msg)                 ║
║      ↓                                                      ║
║   msg.extract_protobuf::<BatteryTelemetry>()                ║
║      ↓                                                      ║
║   temp > 25°C? → ⚠️ WARNING  or  temp OK                    ║
╚═════════════════════════════════════════════════════════════╝
```
Notice the shape: both subscribers are **structurally identical**. They differ only in what they *do* with the telemetry data. The transport, the protobuf extraction, the URI filter — all the same. That is the payoff.

---

### Chapter 8: What we learned

- **Zenoh replaces UDS as the L1 transport** — same uProtocol API, different plugin.
- **Fan-out works on one host also** — the thermal logger attaches without touching the battery subscriber.
- **Business logic survived the swap** — `SimplePublisher`, `UListener`, and `BatteryTelemetry` are unchanged.
- **The uProtocol metadata that was opaque bytes on UDS is now first-class routing information** — `UUri` maps to Zenoh key expressions; the router delivers based on content, not socket path.
- **L3 registration bridges discovery** — the publisher announces its resource to the data space; any connected process can subscribe.
- **Phase 2 limits are resolved** — fan-out, address, listener ownership, framing, and cross-host readiness.

Phase 3 proves that uProtocol's layer design pays off: **swap the wire, keep the application**.

---

### Chapter 9: Where we still fall short

Phase 3 resolves the fan-out problem that Phase 1 identified and Phase 2 documented. Let's be honest about what we have not solved yet:

| Still open              | What would need to change                                                                  | Likely phase         |
| ----------------------- | ------------------------------------------------------------------------------------------ | -------------------- |
| No L4 service discovery | Listeners are still URI-filtered, but there is no central registry of "who publishes what" | Future               |
| Hardcoded constants     | `AUTHORITY_NAME`, `PUBLISHER_UE_ID`, resource IDs live in `up-bms-proto::constants`        | Future (config file) |
| Single-topic demo       | Only `BatteryTelemetry` (resource `0x8001`) — no mixed traffic                             | Future               |
| Manual `zenohd` startup | The router must be launched before any process connects                                    | Production (systemd) |

These are **configuration and integration** problems, not architecture problems; and as such, those are beyond the scope of this tutorial. The uProtocol layer stack remains the right foundation.

---

### Appendix: Key takeaways

1. **L1 (`UTransport` / `UListener`) absorbs the transport swap.** The publisher and subscriber binaries changed exactly one block of code — the transport construction. The business logic (publish loop, `on_receive`) is untouched.

2. **Zenoh's data space enables fan-out where UDS could not.** A second subscriber attaches by running a new binary and registering the same URI filter. No socket sharing, no broker process, no `SOCKET_PATH`.

3. **The uProtocol envelope is a routing contract, not a wire format.** `UUri` + `UAttributes` + `UPayload` exist alongside every message. UDS treated them as opaque bytes; Zenoh reads them as first-class routing information. The same metadata that was dead weight on a UDS socket becomes the key expression in the data space.

4. **Phase 1's thermal logger finally gets its own process.** Three years of tutorial narrative (across three phases) resolved in one `cargo run` command in a fourth terminal.

---

### Appendix B: Workspace diff — Phase 2 vs Phase 3

```
phases/02_uprotocol_semantics/           phases/03_zenoh_topology/
(frozen — phases/02_uprotocol_semantics/)              (active — Phase 3 work)
─────────────────────────────            ─────────────────────────────
up-bms-proto                       ──►   up-bms-proto (copy-forward)
up-battery-telemetry-publisher     ──►   up-battery-telemetry-publisher
up-telemetry-subscriber            ──►   up-telemetry-subscriber
up-uds-transport                   ──X   (retired)
up-frame-codec                     ──X   (retired)
                                         up-thermal-logging-subscriber (new)
```
**Nothing was deleted from the repo** — Phase 2 code remains in `phases/02_uprotocol_semantics/` for side-by-side study. Phase 3 starts clean, carrying forward only the crates that make the fan-out demo.

---

### Appendix C: Protobuf schema (bms_telemetry.proto)

Identical to Phase 2. The `.proto` file is unchanged because the payload contract has not changed:

```protobuf
syntax = "proto3";

package tutorial.bms.v1;

message BatteryTelemetry {
  float soc_percent = 1;
  int32 temp_celsius = 2;
}
```
The publisher still creates `BatteryTelemetry`, still serialises with `prost`, still wraps in `UPayload`. The subscriber still extracts the same struct. Zenoh does not care what the payload bytes contain — it routes by `UUri`, not by schema.

---

### Appendix D: References

- [Zenoh documentation](https://zenoh.io/docs/) — data-space transport overview
- [Eclipse uProtocol — up-client-zenoh-rust](https://github.com/eclipse-uprotocol/up-client-zenoh-rust)
- [Vehicle Signal Specification (VSS)](https://covesa.github.io/vehicle_signal_specification/) — COVESA standard for vehicle data modelling
