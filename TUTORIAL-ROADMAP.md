# Roadmap: uProtocol tutorial for SDV (#rustlang) engineers/programmers/enthusiasts

## 🎯 Strategic Thesis
Demonstrate to the Eclipse SDV community and advanced systems engineers that uProtocol is the mandatory semantic layer for Software-Defined Vehicles (SDVs). Prove that while raw transport layers (like local UDS) move bytes perfectly, they are inherently blind to automotive application semantics, data isolation, and location transparency.

---

## 🛠️ Phase 1: The Illusion of Success & The Architectural Wall
*Goal: Introduce the working multi-binary workspace, then immediately break it by introducing a second independent subscriber to expose raw transport limitations.*

- [x] **1.1 Establish the Baseline (Real Code):** Present the working multi-binary Cargo layout; rename `up-client` → `up-battery-telemetry-publisher`, `up-server` → `up-telemetry-subscriber`. UDS length-framed `UMessage` pub/sub. Tag: `Stage-1-Baseline`.
- [x] **1.2 Introduce the High-Impact Use Case (Narrative):** Thermal Management Logging Engine requirement — must share the same battery telemetry stream (markdown only; no crate yet).
- [x] **1.3 Demonstrate "Deep Water" (Pseudo-Code Friction):** Markdown pseudo-code showing accidental in-process broker / fan-out pain with raw UDS. See `Stage-1.md`.

---

## 🛠️ Phase 2: The Semantic Epiphany (uProtocol over UDS)
*Goal: Teach uProtocol layers and constituents at tutorial depth (not overwhelming; link out to [uProtocol spec/site](https://github.com/eclipse-uprotocol/up-spec) for more). Introduce `up-uds-transport` implementing `UTransport`, refactor binaries around `UListener` and typed payloads — then honestly show where this design still falls short, setting up Stage 3.*

> **Scope guardrails (unchanged from planning):** one subscriber, same UDS socket path, 5 messages then exit, no Thermal crate yet, no Zenoh.

### A — Tutorial narrative (`Stage-2.md`)

- [x] **2.1 The uProtocol stack — a guided map (not a spec dump):**
  Introduce layers at a depth appropriate for beginners/enthusiasts:
  - **L1 (Transport):** `UTransport`, `UListener`, send/receive, listener registration — *how bytes move and callbacks fire*
  - **L2 (Communication):** `Publisher`, message types (`PUBLISH`, …), `CallOptions` — *how entities talk*
  - **Envelope (cross-cutting):** `UMessage`, `UAttributes`, `UUri`, `UPayload`, `UPayloadFormat` — *what wraps every message*
  Keep prose short; use diagrams/tables; point readers to uProtocol's own documentation for full normative detail. No COVESA organisational deep dive.

- [x] **2.2 Constituents in our story — what each piece does:**
  | Constituent | Tutorial focus |
  |---|---|
  | `UUri` | Addressing: authority, entity ID, resource ID; source URI as event/topic identity |
  | `UAttributes` | Intent: message type, TTL, ID; what listeners filter on |
  | `UPayload` / `UPayloadFormat` | Typed vs raw payload; closing the Stage 1 "envelope typed, payload opaque" gap |
  | `UListener` | Declarative `on_receive` — business logic separated from socket reads |
  | `UTransport` | Pluggable wire layer — same API whether UDS or (later) Zenoh |

- [x] **2.3 Stage 1 → Stage 2 contrast table:**
  Side-by-side: what we ignored in Stage 1 vs what we now use deliberately (attributes, URI filters, typed payload, listener callbacks).

### B — Code: `up-uds-transport` crate

- [x] **2.4 Create `phases/02_uprotocol_semantics/crates/up-uds-transport`:**
  Implement `UTransport` over the existing length-framed UDS path (`up-frame-codec` stays; transport crate owns connect/bind, send, listener dispatch).
  - `send(UMessage)` — publisher path
  - `register_listener` / `unregister_listener` — source/sink URI filters → `UListener::on_receive`
  - Bridge: read framed bytes → `UMessage::parse_from_bytes` → filter match → callback

- [x] **2.5 Document benefits of `up-uds-transport` (in Stage-2.md):**
  - Application code speaks **`UTransport` + `UListener`** — not raw `read_exact` loops
  - Publisher/subscriber **contracts** expressed via URI filters, not implicit "whatever is on the socket"
  - **Same abstraction surface** as production transports (`LocalTransport` today, Zenoh in Stage 3) — business logic won't need rewriting
  - Transport concerns **centralised** in one crate instead of duplicated across binaries

- [x] **2.6 Document where we still fall short (Stage 3 setup):**
  Be explicit — `up-uds-transport` is pedagogically useful but **not** a production SDV transport:
  | Limitation | Why it matters |
  |---|---|
  | Point-to-point UDS | Still no fan-out; second consumer problem from Stage 1 unresolved |
  | Filesystem socket path | Local machine only; useless across ECU/network boundaries |
  | No location transparency | Publisher and subscriber must share `/tmp/...sock` |
  | No L3 discovery / registration | Listeners are local registrations, not vehicle-wide service discovery |
  | Underlying stream semantics | We still need length-prefix framing; no broker semantics |
  → Stage 3 replaces transport; Stage 2 semantics stay.

### C — Refactor application binaries

- [x] **2.7 Refactor `up-battery-telemetry-publisher`:**
  - `StaticUriProvider` (or equivalent) for consistent URI construction
  - Protobuf BMS payload (`UPayload::try_from_protobuf`) — remove CAN offset packing from business path
  - Publish via `SimplePublisher` + `up-uds-transport` (proper source/event URI)

- [x] **2.8 Refactor `up-telemetry-subscriber`:**
  - Implement `UListener` (`on_receive`) — unpack typed payload, print SoC/temperature
  - Register listener with URI filter via `up-uds-transport`
  - Remove inline socket-read / manual decode loop from `main`

### D — Close the stage

- [x] **2.9 Evaluate the intermediate state (tutorial + code):**
  Confirm: application routing contracts are now well-defined and declarative; **transport execution is still bottlenecked** by point-to-point UDS. Cliffhanger: topology change + Zenoh in Stage 3; thermal subscriber + fan-out payoff there.

- [x] **2.10 Ship Stage 2:**
  - `Stage-2.md` (code snippets in-step, links to uProtocol docs)
  - Verify build + 5-message run
  - Commit; tag `Stage-2-Baseline`; update `README-Notes.md` *(commit/tag: maintainer)*


## 🛠️ Phase 3: The Scaling Limit & The Zenoh Payoff
*Goal: Force a physical topology shift to demonstrate location transparency, swap out the UDS transport for Zenoh, and showcase uProtocol L3 level registration for the PUBLISH pattern.*

- [ ] **3.1 Trigger the Network Topology Constraint:**
  Move the battery tracking microservice entity off the high-performance compute node to an external physical Zone ECU connected via Automotive Ethernet. Highlight that the local filesystem socket path `/tmp/uprotocol_twin.sock` is now completely dead and useless.
- [ ] **3.2 Introduce the Specialized Transport Plugin:**
  Explain how an advanced data-space protocol like **Eclipse Zenoh** acts as the perfect supplementary transport backend to achieve seamless network location transparency across distributed network boundaries.
- [ ] **3.3 Swap Transport Layers with Zero Business Logic Impact:**
  Drop the custom `up-frame-codec` local framing loop. Bring in the official `up-client-zenoh-rust` transport crate. 
- [ ] **3.4 Bring in L3 Registration for PUBLISH:**
  Showcase uProtocol’s L3 layer registration mechanics specifically for the `PUBLISH` pattern. Demonstrate the ultimate payoff: because the codebase was designed around uProtocol semantics, your core telemetry loop in `up-client` and your application listener blocks in `up-server` remain 100% untouched while data transits smoothly across the network interface.