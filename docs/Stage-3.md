# Stage 3: The Scaling Limit & The Zenoh Payoff

> **Prerequisites:** Stage 2 (`docs/Stage-2.md`, code in `phases/02_uprotocol_semantics/`, tag `Stage-2-Baseline`).  
> **Active code:** `phases/03_zenoh_topology/` — copy-forward from Phase 2.

Stage 3 addresses everything Stage 2 **documented but did not fix** — starting with problems that appear **even on a single Linux host** (fan-out, broker semantics), then the **thermal logger payoff**, and finally the **cross-ECU story** real vehicles face (narrative only in this tutorial). The code payoff: **Stage 2 business logic survives** — only the wire and discovery story change.

> **Demo scope (Phase 3):** All processes run on **one Linux machine** (separate terminals), same as Stages 1–2. We do **not** use multiple hosts, VMs, or containers to simulate ECUs. The Zone ECU / central compute split is **vehicle narrative** — it explains an *extra* benefit of Zenoh, not something we reproduce in the lab demo.

For normative detail, see [uProtocol up-spec](https://github.com/eclipse-uprotocol/up-spec) and [Eclipse Zenoh](https://zenoh.io/).

### Key tutorial idea — retired vs carried forward

Phase 3 is not “Phase 2 plus more crates.” It is a **deliberate subtraction** on the wire layer:

| | Phase 2 (`phases/02_uprotocol_semantics/`) | Phase 3 (`phases/03_zenoh_topology/`) |
|---|---|---|
| **Status** | Frozen at `Stage-2-Baseline` | Active development |
| **`up-uds-transport`** | Used — L1 over UDS | **Retired** — not copied |
| **`up-frame-codec`** | Used — length-prefix framing | **Retired** — not copied |
| **`up-bms-proto`** | Introduced | **Carried forward** unchanged |
| **Publisher / subscriber logic** | `SimplePublisher`, `UListener` | **Carried forward** — same loops |
| **Physical wire** | `/tmp/uprotocol_twin.sock` | Zenoh data space (§3.3) |

**Why retire `up-uds-transport` and `up-frame-codec` instead of reusing them?**

1. **Pedagogy** — Phase 2 taught uProtocol L1 on a wire you already knew (UDS). Phase 3 teaches that **production SDV transport is not UDS**; keeping the old crates in the active workspace would blur that lesson.
2. **Topology** — Those crates exist because UDS is a **byte stream** on a **filesystem path**. That fails for **fan-out on one host** (our demo driver) and fails again for **cross-ECU** deployments (vehicle narrative in §3.1). Either way, Phase 3 replaces the wire rather than extending UDS.
3. **Copy-forward snapshots** — Earlier phases stay frozen under `phases/` so readers can diff chapters. Stage 2 code remains inspectable at `Stage-2-Baseline`; Stage 3 starts clean without dragging UDS plumbing into the Zenoh story.
4. **The L1 seam pays off here** — Because Phase 2 moved socket work behind `UTransport`, retiring `up-uds-transport` does **not** mean rewriting battery telemetry logic — only replacing the object you pass to `SimplePublisher::new` and `register_listener`.

```
phases/02_uprotocol_semantics/       phases/03_zenoh_topology/
(frozen — Stage-2-Baseline)          (active — Stage 3 work)
─────────────────────────────        ─────────────────────────────
up-bms-proto                    ──►  up-bms-proto (copy-forward)
up-battery-telemetry-publisher  ──►  up-battery-telemetry-publisher
up-telemetry-subscriber         ──► up-telemetry-subscriber
up-uds-transport                ──X  (retired — study in Phase 02 only)
up-frame-codec                  ──X  (retired — study in Phase 02 only)
                                   + up-client-zenoh-rust     (§3.3)
                                   + up-thermal-logging-subscriber (§3.4)
```

When the tutorial says *“retired from Phase 03,”* it means: **not part of the active demo path** — still available for comparison in the Phase 2 snapshot, never deleted from the repo history.

---

## Tutorial thesis (Phase 3 story arc)

Two layers — **do not conflate them**:

| Layer | Question | Answered in |
|---|---|---|
| **A — Same Linux host (demo)** | Why swap UDS for Zenoh *even if nothing moved to another machine*? | §3.1 (first) |
| **B — Extra benefit (demo payoff)** | What does Zenoh unlock for the **thermal logger** on that same host? | §3.1 (second), §3.4 (code) |
| **C — Vehicle narrative (not simulated)** | What breaks when publisher and subscriber live on **different ECUs**? | §3.1 (third) — motivates production transport choice |

| Stage 2 gap (same host) | Phase 3 response |
|---|---|
| Point-to-point UDS, no fan-out | Zenoh pub/sub — second process subscribes independently |
| In-process listener dispatch only | Data-space delivery to **multiple** `UListener`s in **separate processes** |
| Manual stream framing | Broker-capable transport; `up-frame-codec` off hot path |
| Local `register_listener` table | L3 PUBLISH registration (§3.4) |

| Stage 2 gap (cross-ECU — narrative) | Phase 3 response |
|---|---|
| Filesystem socket path | Network data space; `/tmp/uprotocol_twin.sock` useless across ECUs |
| No location transparency | Logical URI + Zenoh session — not a path on one kernel |

**Copy-forward rule:** Envelope, `SimplePublisher`, `UListener`, `BatteryTelemetry` protobuf **stay**. Replace **transport execution** and add **L3**.

---

## 3.0 — Workspace setup (complete)

Phase 3 active development lives under `phases/03_zenoh_topology/`. See **Key tutorial idea — retired vs carried forward** (above) for the full retirement rationale; this section records the concrete layout.

### Active workspace (Phase 03)

```
phases/03_zenoh_topology/crates/
├── up-bms-proto/                    # copy-forward from Phase 2 (unchanged schema)
├── up-battery-telemetry-publisher/  # same publish loop; Zenoh transport in §3.3
└── up-telemetry-subscriber/         # same on_receive; Zenoh transport in §3.3
```

### Retired from Phase 03 — frozen in Phase 02 only

These crates **taught Stage 2 well** but **must not appear** in the Phase 3 active workspace:

| Crate | Role in Stage 2 | Why retired in Stage 3 |
|---|---|---|
| **`up-uds-transport`** | `UdsTransport` / `UdsTransportClient` — L1 over Unix sockets | No multi-process fan-out on one host; no cross-ECU path — replaced by Zenoh (§3.3) |
| **`up-frame-codec`** | 4-byte length-prefix framing for UDS byte streams | Broker-capable transport owns message boundaries; manual framing is no longer on the hot path |

They remain in `phases/02_uprotocol_semantics/` at tag **`Stage-2-Baseline`** for side-by-side study — not copied, not depended on, not imported from Phase 02 into Phase 03 binaries.

**Tutorial checkpoint question:** *If `UTransport` is the same trait in both phases, why can we drop two whole crates?* Because those crates were **implementations of L1 for UDS**, not part of uProtocol semantics. Phase 3 swaps the **implementation**, not the **trait or business logic**.

Constants note: `up_bms_proto::constants::SOCKET_PATH` still exists for historical comparison but is **legacy**; Phase 3 demo configuration moves to Zenoh endpoints (`ZENOH_CONNECT`, §3.3).

### Planned additions (§3.3–3.4)

| Crate / dependency | When |
|---|---|
| `up-client-zenoh-rust` (or current official crate) | §3.3 transport swap |
| `up-thermal-logging-subscriber` | §3.4 fan-out payoff |

Binaries currently use `todo!()` at the transport wiring site until §3.3.

---

## 3.1 — Why we need a data-space transport (Zenoh)

Stage 2 left us on **one Linux host** with a working 1:1 demo. Phase 3 asks a harder question first:

> *Even if publisher and subscribers never leave this machine, why is UDS the wrong long-term wire — and why is something like Zenoh the right replacement?*

Only after that do we ask what Zenoh buys for the **thermal logger**. The **cross-ECU** angle comes last — as **vehicle narrative**, not as something this tutorial simulates with multiple hosts or containers.

### Demo scope — one Linux host, multiple processes

| What we do (Phase 3 demo) | What we do *not* do |
|---|---|
| Run publisher + subscribers as **separate processes** on **your Linux machine** | Multi-host, Docker/Podman ECU simulation, VMs |
| Connect all processes to a **local Zenoh router** (§3.3) | Automotive Ethernet lab setup |
| Same pattern as Stages 1–2: **two or three terminals**, five messages, exit | Physical Zone ECU hardware |

The **Zone ECU vs central compute** story below explains why production SDVs need Zenoh **in addition to** the same-host lessons — readers should not confuse it with our lab layout.

---

### (1) The case for Zenoh — even on the same Linux host

Stages 1–2 used `/tmp/uprotocol_twin.sock`. That worked for **one publisher, one subscriber, one process each** — a deliberately narrow demo. It does **not** scale to how services actually attach in an SDV stack, **even when every binary runs on the same kernel**.

#### UDS is point-to-point, not publish/subscribe

`up-uds-transport` accepts a connection, reads **one** message, dispatches to listeners **inside the server process** (`UdsTransport::serve`). That is not the same as **two independent subscriber processes** both receiving the same live stream:

```
Same Linux host — Stage 2 UDS (still broken for fan-out)

  up-battery-telemetry-publisher          up-telemetry-subscriber
         │                                      ▲
         │  one UDS connection                  │ UdsTransport::serve
         └──────────────► /tmp/...sock ─────────┘ (one server process)

  up-thermal-logging-subscriber (§3.4)
         │
         └── cannot share the stream — second process has no socket to read
             (Stage 1 broker pseudo-code trap — docs/Stage-1.md)
```

| Need on one host | UDS + `up-uds-transport` | Data-space transport (Zenoh) |
|---|---|---|
| Second **process** as subscriber | Not supported — turn subscriber into broker | Native pub/sub fan-out |
| Publisher unchanged when consumer added | N/A — architectural hack | Subscribe independently |
| URI filter per consumer | In-process only | Each process registers its own `UListener` |
| Avoid manual re-serialize / forward loop | Fails — Stage 1 pseudo-code | Transport handles distribution |

**You do not need a second machine to hit this wall.** Stage 1 introduced the thermal logging engine as a **second consumer on the same vehicle**; Stage 2 made URI interest declarative but **did not deliver bytes to a second process**. That alone justifies retiring UDS for Phase 3 — before anyone moves an ECU.

#### Filesystem path is a poor logical address (even locally)

On one host, `/tmp/uprotocol_twin.sock` is a **filename**, not a uProtocol resource name:

- Two subscriber **processes** cannot both `bind()` the same path.
- A subscriber process cannot `register_listener` on another process's `UdsTransport` server handle.
- The **only** UDS-native pattern is point-to-point connect — fine for 1:1, wrong for N subscribers.

Stage 2's **`UUri`** (entity `0x1010`, resource `0x8001`) is the logical contract. Zenoh (and L3 in §3.4) align **wire delivery** with that contract. UDS aligned it only for a single pipe on a path string.

#### Why not patch UDS instead of Zenoh?

Because the tutorial goal is **production-shaped uProtocol**, not a smarter broker in the subscriber `main`:

| Approach | Problem |
|---|---|
| Extend `up-uds-transport` with fan-out lists | Reinvents a message broker inside application code (Stage 1 trap) |
| Keep UDS for 1:1, Zenoh only when ECU splits | Teaches two mental models; hides that fan-out was already missing |
| **Swap L1 plugin to Zenoh on same host** | One transport story; fan-out works in demo; cross-ECU ready in production |

Phase 3 retires `up-uds-transport` and `up-frame-codec` (§3.0) because the **same-host fan-out requirement** already disqualifies UDS — not only because a Zone ECU might appear later.

---

### (2) Extra benefit on the same host — the thermal logger

Once Zenoh replaces UDS, the **Stage 1 narrative payoff** lands **without moving any process to another machine**:

```
Same Linux host — Phase 3 target (demo)

  up-battery-telemetry-publisher
         │
         │  publish BatteryTelemetry (resource 0x8001)
         ▼
    ┌─────────────┐
    │ Zenoh       │  ← local router, one machine
    │ (data space)│
    └──────┬──────┘
           ├──────────────────► up-telemetry-subscriber
           │                      (SoC + temp display — unchanged on_receive)
           │
           └──────────────────► up-thermal-logging-subscriber (§3.4)
                                  (temperature thresholds — own process, own UListener)
```

| Consumer | Process | Cares about | Stage 2 UDS | Stage 3 Zenoh (same host) |
|---|---|---|---|---|
| Battery telemetry subscriber | 1 | SoC + temperature | ✓ (only consumer) | ✓ |
| Thermal logging engine | 2 | Temperature only | ✗ | ✓ — independent `UListener` |

The thermal crate does **not** require cross-ECU topology. It requires **fan-out** — which UDS could not provide on one host. That is the **extra benefit of Zenoh in this tutorial**: the second service from `docs/Stage-1.md` finally attaches cleanly, in a **third terminal**, while the battery subscriber code stays untouched.

§3.4 adds `up-thermal-logging-subscriber`. §3.3 makes Zenoh the wire so §3.4 is possible.

---

### (3) Vehicle narrative — cross-ECU (not simulated in the demo)

In a deployed SDV, the battery publisher often moves to a **Zone ECU** near the pack; HMI and logging services stay on **central compute**. Automotive Ethernet sits between them. **We tell this story; we do not reproduce it in the lab.**

| Before (Stages 1–2) | Vehicle story (narrative) |
|---|---|
| Same machine as subscriber (demo) | Publisher on **Zone ECU**; subscribers on **central compute** |
| `/tmp/uprotocol_twin.sock` on one kernel | No shared filesystem between ECUs — **path is dead across nodes** |

```
Vehicle story (motivation only — not our demo topology)

  ┌──────────────────┐     Automotive Ethernet       ┌──────────────────┐
  │ Zone ECU         │  ═════════════════════════►   │ Central compute  │
  │  publisher       │   (no shared /tmp)            │  subscribers +   │
  └──────────────────┘                               │  thermal logger  │
                                                     └──────────────────┘
                 Zenoh spans the network in production
```

This is an **additional** reason teams choose Zenoh over UDS — **location transparency** when peers are not on the same kernel. Our demo already needed Zenoh for §(1) and §(2); cross-ECU is why the same choice survives a topology refactor **without rewriting** `SimplePublisher` or `on_receive`.

**Do not conflate:** completing the tutorial on one Linux host proves fan-out and thermal logging. Understanding the Zone ECU story explains why that same transport plugin is used on real vehicles.

---

### What still works when UDS retires

| Layer | Same-host demo | Cross-ECU (production) |
|---|---|---|
| `BatteryTelemetry` / `up-bms-proto` | ✓ unchanged | ✓ unchanged |
| `SimplePublisher::publish` loop | ✓ unchanged | ✓ unchanged |
| `UListener::on_receive` bodies | ✓ unchanged | ✓ unchanged |
| `UUri` filter on resource `0x8001` | ✓ unchanged | ✓ unchanged |
| `UdsTransport` / `SOCKET_PATH` | ✗ retired | ✗ useless across ECUs |

**Only** `UTransport` construction and session config change in §3.3.

### What §3.1 does *not* solve yet

| Still blocked | Addressed in |
|---|---|
| Zenoh not wired | §3.2 (why Zenoh) + §3.3 (code) |
| Thermal subscriber missing | §3.4 |
| L3 registration | §3.4 |
| Phase 03 binaries `todo!()` at transport | §3.3 |

No SSH tunnels, no NFS `/tmp`, no copying `up-uds-transport` into Phase 03.

### Checkpoint reminder

§3.1 order matters: **(1) same-host case for Zenoh**, **(2) thermal logger benefit on that host**, **(3) cross-ECU narrative without multi-host demo**. Next: **§3.2** — what Zenoh is and how it plugs into uProtocol L1.

### Key takeaway at 3.1

**Zenoh is not only for distributed ECUs.** On a single Linux machine, UDS already fails the moment a **second independent process** must consume the same telemetry stream. Zenoh fixes that — and the **thermal logger** is the tutorial proof. Moving the publisher to a Zone ECU is the **production** story for the same transport choice; our demo stays on one host to keep focus on fan-out and uProtocol L1/L3, not lab networking.

---

## 3.2 — Introduce the specialized transport plugin (Zenoh)

- **Data-space transport** — Zenoh brokered pub/sub; any process connected to the router can publish or subscribe without a shared filesystem or single-owner socket
- **Location transparency** — same API across process and (eventually) network boundaries; the Zenoh router abstracts "where" the endpoint lives
- **Fan-out** — native pub/sub: N subscribers on the same URI filter all receive the message; no point-to-point bottleneck
- **uProtocol L1 trait compatibility** — `up-client-zenoh-rust` implements `UTransport` / `UListener`; application code written against these traits (publisher loop, `on_receive`) is **unchanged** from Phase 2
- **Links** — [Eclipse Zenoh](https://zenoh.io/), [up-client-zenoh-rust](https://github.com/eclipse-uprotocol/up-client-zenoh-rust) — not a COVESA deep dive

### What uProtocol puts alongside the actual message

uProtocol never sends a raw protobuf payload on the wire. Every `UMessage` carries **three layers of metadata** alongside the bytes:

| Layer | What it contains | Populated by |
|---|---|---|
| **UUri (source + sink)** | Authority name, uEntity ID, uEntity version, resource ID — the fully-qualified "who and what" of the message | `SimplePublisher` / `StaticUriProvider` |
| **UAttributes** | Priority, TTL, message type (PUBLISH, REQUEST, RESPONSE, etc.), correlation ID, timestamps | `CallOptions` + transport |
| **UPayload** | The application bytes (here, the serialized `BatteryTelemetry` protobuf) plus a format hint | `UPayload::try_from_protobuf` |

When the publisher calls:
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

### Why UDS did not (and could not) make use of this metadata

In Phase 2, the `UMessage` envelope was assembled and serialised by `SimplePublisher`, then given to `UdsTransportClient::send`. But the UDS transport treats the envelope as **opaque bytes to stream over a socket**:

```text
UDS flow (Phase 2)

  Publisher                    UdsTransportClient          UdsTransport::serve          Subscriber
     │                                │                          │                          │
     │──UMessage (UUri+Attr+Payload)──►│                          │                          │
     │                                │──length-prefixed bytes──►│                          │
     │                                │   (via up-frame-codec)   │──de-frame + UMessage─────►│
     │                                │                          │──match UUri against──────►│
     │                                │                          │   local listener table    │ on_receive
```

Key point: the UDS transport does **not** inspect the `UUri` or `UAttributes` to route the message. The metadata is embedded in the serialised bytes but is functionally dead weight for UDS — the socket path `/tmp/uprotocol_twin.sock` already decided the destination. UDS delivers everything to whoever is listening on that one path, regardless of what the `UUri` says. The `register_listener` matching happens in the **subscriber's own process**, inside `UdsTransport`'s local `HashMap` — it is not used by the wire.

> **UDS cannot use the uProtocol metadata for routing** because UDS has no routing layer. There is one socket, one connection, one direction. The `UUri` is present in the serialised bytes but plays no role in transport-level delivery.

### How Zenoh makes the metadata useful

Zenoh is a **content-aware router**, not a byte pipe. When `up-transport-zenoh` receives a `UMessage` via `UTransport::send`, it does *not* forward opaque bytes. Instead:

1. It **reads the `UUri`** from the envelope and maps it to a Zenoh key expression — e.g. `local_vehicle/0x1010/1/0x8001`.
2. It **publishes** the payload bytes tagged with that key expression into the Zenoh data space.
3. On the subscriber side, `UPTransportZenoh` registers a **Zenoh subscriber** on the key expression matching the listener's `source_filter`.
4. When a Zenoh publication arrives, it reconstructs the `UMessage` and calls `UListener::on_receive`.

```text
Zenoh flow (Phase 3)

  Publisher                    UPTransportZenoh         Zenoh router (zenohd)       UPTransportZenoh          Subscriber
     │                                │                          │                          │                    │
     │──UMessage (UUri+Attr+Payload)──►│                          │                          │                    │
     │                                │──extract UUri────────    │                          │                    │
     │                                │──publish(ZenohKey,──►────►──fan-out to matching────►────reconstruct─────► on_receive
     │                                │   payload)                │   subscribers             │   UMessage              │
     │                                │                          │                          │                    │
     │                                │                          ├──fan-out to thermal──────►────reconstruct─────►
     │                                │                          │   subscriber               │   UMessage      on_receive
```

**What changed:** the `UUri` and `UAttributes` that were opaque bytes on a UDS socket are now **first-class routing information** for the Zenoh data space. The Zenoh router matches key expressions to subscribers — the same way the UDS layer used the `UUri` only inside its own `HashMap`, but now the matching happens **in the network**.

| | UDS (Phase 2) | Zenoh (Phase 3) |
|---|---|---|
| Metadata location | Encoded in opaque bytes | Inspected by transport, exposed in Zenoh key |
| Routing | Socket path decides destination | `UUri`-derived key expression decides fan-out |
| Multi-subscriber | Impossible (one server) | Native — each subscriber gets its own Zenoh subscription |
| Listener matching | Local `HashMap` in `UdsTransport::serve` process | On the Zenoh router — any connected process can subscribe |

**This is why uProtocol is designed to work with routers like Zenoh.** The envelope (`UUri` + `UAttributes` + `UPayload`) is not a wire format chosen for convenience — it is a **routing contract** that a content-aware transport can act on. UDS ignored that contract because it did not need to route; it only needed to stream. Zenoh *uses* the contract, and the result is fan-out, location transparency, and zero metadata re-invention by the application programmer.

---

## 3.3 — Swap transport layers with zero business logic impact

### Workspace changes

Retained from Phase 2:
- `up-bms-proto` — protobuf schema + constants (unchanged)
- `up-battery-telemetry-publisher` — `SimplePublisher` loop (unchanged body)
- `up-telemetry-subscriber` — `BatteryTelemetryListener::on_receive` (unchanged body)

Removed (Phase 2 only):
- `up-uds-transport` — retired; UDS socket + listener-table logic not needed
- `up-frame-codec` — retired; Zenoh handles framing

Added:
- `up-client-zenoh-rust` — Zenoh-backed `UTransport` implementation
- `up-thermal-logging-subscriber` — second subscriber for fan-out demo (§3.4)

### Wiring changes (the only code diffs from Phase 2)

**Publisher** — replace transport construction:

```rust
// Phase 2 (UDS) — retired
// let transport: Arc<dyn UTransport> = Arc::new(UdsTransportClient::new(SOCKET_PATH));

// Phase 3 (Zenoh)
let zenoh_config = zenoh::Config::default();
let session = zenoh::open(zenoh_config).await?;
let transport: Arc<dyn UTransport> = Arc::new(ZenohTransport::new(session).await?);
```

**Subscriber** — replace transport construction + server start:

```rust
// Phase 2 (UDS) — retired
// let transport = UdsTransport::serve(SOCKET_PATH).await?;

// Phase 3 (Zenoh)
let zenoh_config = zenoh::Config::default();
let session = zenoh::open(zenoh_config).await?;
let transport: Arc<dyn UTransport> = Arc::new(ZenohTransport::new(session).await?);
```

**Thermal subscriber** — same pattern, same transport wiring, different `UListener`.

### What stayed exactly as Phase 2

| Component | Phase 2 | Phase 3 |
|---|---|---|
| `SimplePublisher::publish` loop | `publish(resource_id, call_options, payload)` | **Identical** |
| `UListener::on_receive` body | `extract_protobuf::<BatteryTelemetry>()` | **Identical** |
| URI filter | `source_filter` via `StaticUriProvider` | **Identical** |
| `up-bms-proto` constants | `AUTHORITY_NAME`, `PUBLISHER_UE_ID`, etc. | **Identical** |
| `BatteryTelemetry` proto | protobuf schema — SoC + temp | **Identical** |

### Build

```bash
cargo build --manifest-path phases/03_zenoh_topology/Cargo.toml
```

All four crate binaries compile. The `todo!()` placeholders at the transport site panic if executed — the full runtime wiring (Zenoh session, config) is resolved when `up-client-zenoh-rust` crate details are confirmed.

---

## 3.4 — L3 registration for PUBLISH + thermal fan-out payoff

### Goal

Wire the **Thermal Management Logging Engine** (introduced in Stage 1 §1.4) as an independent binary — no socket sharing with the battery subscriber.

### What was added

- **`up-thermal-logging-subscriber`** — new crate in `phases/03_zenoh_topology/crates/up-thermal-logging-subscriber/`
  - Own `ThermalLoggingListener` implementing `UListener`
  - Extracts `temp_celsius` from `BatteryTelemetry` protobuf
  - Prints warning when temp > 25°C
  - Same `todo!()` placeholder for Zenoh transport (wired in §3.3)
- **Workspace member** — added to `phases/03_zenoh_topology/Cargo.toml`

### L3 registration — what changed from Phase 2

| Aspect | Phase 2 (UDS) | Phase 3 (Zenoh) |
|---|---|---|
| Listener scope | In-process hash table in `UdsTransport::serve` | Data-space delivery via Zenoh router |
| Registration API | `register_listener(&source_filter, sink_filter, listener)` | **Same API** — transport plugin handles distribution |
| Process boundary | One server owns the socket; only its listeners receive | Any process connected to Zenoh router receives matching messages |
| Discovery | None — subscriber must know `SOCKET_PATH` ahead of time | L3 PUBLISH registration — resource announced to data space |

The application code (`register_listener`, `on_receive`) is **identical** between Phase 2 and Phase 3. Only the transport plugin changed from UDS to Zenoh.

### Multi-subscriber demo layout

```
Terminal 1:  zenohd                  (Zenoh router daemon)
Terminal 2:  up-battery-telemetry-publisher  (sends 5 messages)
Terminal 3:  up-telemetry-subscriber         (battery display)
Terminal 4:  up-thermal-logging-subscriber   (thermal warnings)
```

Both subscribers use `register_listener` with the same resource URI filter (`BATTERY_TELEMETRY_RESOURCE_ID`, `0x8001`). Each receives all five messages independently.

### Build verification

```bash
cargo build --manifest-path phases/03_zenoh_topology/Cargo.toml \
  -p up-telemetry-subscriber \
  -p up-thermal-logging-subscriber
```

---

## 3.5 — Evaluate the intermediate state

### Did fan-out work?

Yes — both the battery telemetry subscriber and the thermal logging subscriber received all 5 messages. Each subscriber runs in its own process, connected to the same Zenoh router. No UDS socket path; no shared filesystem. Zenoh delivers the publication to every subscriber whose URI filter matches — that is the fan-out that Stage 2's point-to-point UDS could not provide.

### Did business logic change?

No — the `SimplePublisher::publish` loop in the publisher and the `UListener::on_receive` bodies in both subscribers are **identical** to their Phase 2 counterparts. The only code change was replacing the transport construction:

```rust
// Phase 2 — UDS (retired)
// Arc::new(UdsTransportClient::new(SOCKET_PATH))

// Phase 3 — Zenoh
Arc::new(UPTransportZenoh::builder(AUTHORITY_NAME)?
    .with_config(zenoh_config::Config::default())
    .build().await?)
```

This is the payoff of the uProtocol L1 `UTransport` trait: the application layer never touches the wire.

### Did L3 registration matter?

In this single-host demo, the L3 `register_listener` call behaves identically to Phase 2 — but the mechanism is different. Phase 2's listener table was a `HashSet` inside the local `UdsTransport` process. Phase 3 registers the listener with the Zenoh data space, so any process connected to the same Zenoh router can publish to it — no shared socket path required.

### What stayed the same?

| Component | Unchanged from Phase 2 |
|---|---|
| `BatteryTelemetry` protobuf schema | Yes — same `.proto` compilation |
| URI constants (`AUTHORITY_NAME`, `PUBLISHER_UE_ID`, resource IDs) | Yes |
| `StaticUriProvider` | Yes |
| `SimplePublisher::publish` call | Yes — same `CallOptions`, same resource ID |
| `UListener::on_receive` body | Yes — same `extract_protobuf`, same fields |
| `register_listener` signature | Yes — same `source_filter` / `sink_filter` |

### One note: session teardown noise

The Zenoh library logs an `ERROR` message when the subscriber disconnects ("Unable to publish link event: session closed"). This is cosmetic — the subscriber has already processed all messages and exited cleanly. It can be suppressed with `RUST_LOG=off` or by setting up a custom tracing subscriber.

---

## 3.6 — Ship Stage 3 (`Stage-3-Baseline`)

| Check | Command | Status |
|---|---|---|
| Workspace build | `cargo build --manifest-path phases/03_zenoh_topology/Cargo.toml` | ✅ Passes clean |
| Multi-subscriber demo | Same Linux host — battery + thermal subscribers in separate terminals, then publisher | ✅ Both subscribers received all messages |
| Fresh checkout and build | `git clone ... && cd phases/03_zenoh_topology && cargo build` | ✅ Workspace self-contained |
| `docs/Stage-3.md` complete | All sections §3.0–§3.6 filled | ✅ Done |

### Tag and commit

```bash
git add .
git commit -m "Stage 3: Zenoh transport, fan-out demo, thermal subscriber"
git tag -a Stage-3-Baseline -m "Stage 3 baseline — Zenoh transport with multi-subscriber fan-out"
```

Verify the tag points at the right commit before pushing:

```bash
git log --oneline --graph --decorate Stage-3-Baseline~3..Stage-3-Baseline
```
| Tag | `Stage-3-Baseline` |

---

## Handoff reference (from Stage 2)

What Stage 2 delivered and Phase 3 must preserve:

| Layer | Carry forward |
|---|---|
| Envelope | `UMessage`, `UAttributes`, `UUri`, `UPayload` |
| L2 | `SimplePublisher`, `CallOptions`, protobuf payload |
| L1 API | `UTransport`, `register_listener`, `UListener::on_receive` |
| Business logic | SoC/temp publish loop; battery `on_receive` |

What Stage 2 could not fix (Phase 3 drivers):

**On one Linux host (demo):**

1. No multi-process fan-out  
2. Stream / broker semantics — length-prefix over point-to-point UDS  
3. Local `register_listener` only — no L3  

**Cross-ECU (vehicle narrative — not simulated):**

4. Filesystem socket path  
5. No location transparency  

See `docs/Stage-2.md` §2.6 for full prose.

---

## Key takeaway at Stage 3

Stage 3 proves that **uProtocol semantics decouple application logic from transport topology**. Stage 2 built the right abstractions; Stage 3 swaps UDS for Zenoh — first to unlock **fan-out on one machine** (thermal logger), then (in production stories) **cross-ECU** delivery without rewriting battery logic.

The demo ran on a single Linux host, but the transport replacement is the same one a vehicle would use: `up-transport-zenoh` provides network-location transparency via a Zenoh data space instead of a filesystem socket path. The publisher, the battery subscriber, and the thermal subscriber each connected to the same Zenoh router — no `/tmp/uprotocol_twin.sock`, no single-owner process.

**What survived unchanged:**
- `BatteryTelemetry` protobuf schema
- `SimplePublisher::publish` loop
- `UListener::on_receive` body (both subscribers)
- URI filter constants

**What retired (Phase 2 only):**
- `up-uds-transport` — stream transport
- `up-frame-codec` — length-prefix framing
- `SOCKET_PATH` — no longer needed

**Retirement recap:** `up-uds-transport` and `up-frame-codec` belong to the **Stage 2 UDS story**, frozen under `phases/02_uprotocol_semantics/`. They are intentionally absent from Phase 03 — not because they were wrong for a 1:1 demo, but because **even a second subscriber on the same host** outgrows point-to-point UDS. What survives is everything above L1: protobuf payloads, URI filters, `SimplePublisher`, and `UListener`.
