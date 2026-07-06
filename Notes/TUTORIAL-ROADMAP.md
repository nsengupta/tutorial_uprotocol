# Roadmap: uProtocol tutorial for SDV (#rustlang) engineers/programmers/enthusiasts

## 🎯 Strategic Thesis
Demonstrate to the Eclipse SDV community and advanced systems engineers that uProtocol is the mandatory semantic layer for Software-Defined Vehicles (SDVs). Prove that while raw transport layers (like local UDS) move bytes perfectly, they are inherently blind to automotive application semantics, data isolation, and location transparency.

---

## 🛠️ Phase 1: The Illusion of Success & The Architectural Wall
*Goal: Introduce the working multi-binary workspace, then immediately break it by introducing a second independent subscriber to expose raw transport limitations.*

- [x] **1.1 Establish the Baseline (Real Code):** Present the working multi-binary Cargo layout; rename `up-client` → `up-battery-telemetry-publisher`, `up-server` → `up-telemetry-subscriber`. UDS length-framed `UMessage` pub/sub. Code: `phases/01_raw_sockets/`.
- [x] **1.2 Introduce the High-Impact Use Case (Narrative):** Thermal Management Logging Engine requirement — must share the same battery telemetry stream (markdown only; no crate yet).
- [x] **1.3 Demonstrate "Deep Water" (Pseudo-Code Friction):** Markdown pseudo-code showing accidental in-process broker / fan-out pain with raw UDS. See `docs/Phase-1.md`.

---

## 🛠️ Phase 2: The Semantic Epiphany (uProtocol over UDS)
*Goal: Teach uProtocol layers and constituents at tutorial depth (not overwhelming; link out to [uProtocol spec/site](https://github.com/eclipse-uprotocol/up-spec) for more). Introduce `up-uds-transport` implementing `UTransport`, refactor binaries around `UListener` and typed payloads — then honestly show where this design still falls short, setting up Phase 3.*

> **Scope guardrails (unchanged from planning):** one subscriber, same UDS socket path, 5 messages then exit, no Thermal crate yet, no Zenoh.

### A — Tutorial narrative (`docs/Phase-2.md`)

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
  | `UPayload` / `UPayloadFormat` | Typed vs raw payload; closing the Phase 1 "envelope typed, payload opaque" gap |
  | `UListener` | Declarative `on_receive` — business logic separated from socket reads |
  | `UTransport` | Pluggable wire layer — same API whether UDS or (later) Zenoh |

- [x] **2.3 Phase 1 → Phase 2 contrast table:**
  Side-by-side: what we ignored in Phase 1 vs what we now use deliberately (attributes, URI filters, typed payload, listener callbacks).

### B — Code: `up-uds-transport` crate

- [x] **2.4 Create `phases/02_uprotocol_semantics/crates/up-uds-transport`:**
  Implement `UTransport` over the existing length-framed UDS path (`up-frame-codec` stays; transport crate owns connect/bind, send, listener dispatch).
  - `send(UMessage)` — publisher path
  - `register_listener` / `unregister_listener` — source/sink URI filters → `UListener::on_receive`
  - Bridge: read framed bytes → `UMessage::parse_from_bytes` → filter match → callback

- [x] **2.5 Document benefits of `up-uds-transport` (in docs/Phase-2.md):**
  - Application code speaks **`UTransport` + `UListener`** — not raw `read_exact` loops
  - Publisher/subscriber **contracts** expressed via URI filters, not implicit "whatever is on the socket"
  - **Same abstraction surface** as production transports (`LocalTransport` today, Zenoh in Phase 3) — business logic won't need rewriting
  - Transport concerns **centralised** in one crate instead of duplicated across binaries

- [x] **2.6 Document where we still fall short (Phase 3 setup):**
  Be explicit — `up-uds-transport` is pedagogically useful but **not** a production SDV transport:
  | Limitation | Why it matters |
  |---|---|
  | Point-to-point UDS | Still no fan-out; second consumer problem from Phase 1 unresolved |
  | Filesystem socket path | Local machine only; useless across ECU/network boundaries |
  | No location transparency | Publisher and subscriber must share `/tmp/...sock` |
  | No L3 discovery / registration | Listeners are local registrations, not vehicle-wide service discovery |
  | Underlying stream semantics | We still need length-prefix framing; no broker semantics |
  → Phase 3 replaces transport; Phase 2 semantics stay.

### C — Refactor application binaries

- [x] **2.7 Refactor `up-battery-telemetry-publisher`:**
  - `StaticUriProvider` (or equivalent) for consistent URI construction
  - Protobuf BMS payload (`UPayload::try_from_protobuf`) — remove CAN offset packing from business path
  - Publish via `SimplePublisher` + `up-uds-transport` (proper source/event URI)

- [x] **2.8 Refactor `up-telemetry-subscriber`:**
  - Implement `UListener` (`on_receive`) — unpack typed payload, print SoC/temperature
  - Register listener with URI filter via `up-uds-transport`
  - Remove inline socket-read / manual decode loop from `main`

### D — Close the phase

- [x] **2.9 Evaluate the intermediate state (tutorial + code):**
  Confirm: application routing contracts are now well-defined and declarative; **transport execution is still bottlenecked** by point-to-point UDS. Cliffhanger: topology change + Zenoh in Phase 3; thermal subscriber + fan-out payoff there.

- [x] **2.10 Ship Phase 2:**
  - `docs/Phase-2.md` (code snippets in-step, links to uProtocol docs)
  - Verify build + 5-message run
  - Commit; update `README-Notes.md` *(maintainer)*


## 🛠️ Phase 3: The Scaling Limit & The Zenoh Payoff
*Goal: Force a physical topology shift to demonstrate location transparency, swap out the UDS transport for Zenoh, and showcase uProtocol L3 level registration for the PUBLISH pattern.*

> **Setup (3.0 / A.1–A.5):** `phases/03_zenoh_topology/` created; `up-bms-proto` + publisher/subscriber business logic copy-forward; `up-uds-transport` / `up-frame-codec` remain Phase 02 only; `docs/Phase-3.md` skeleton. Transport wired in §3.3.

- [x] **3.1 Why we need a data-space transport (Zenoh):**
  (1) Case for Zenoh **on the same Linux host** (fan-out / UDS limits) even before any ECU move; (2) **thermal logger** as extra same-host benefit; (3) cross-ECU **vehicle narrative** only — demo stays one machine, no containers. See `docs/Phase-3.md` §3.1.
- [x] **3.2 Zenoh as data-space transport (§3.2):**
  Full narrative with 5-dimension contrast table (UDS vs Zenoh) and code swap walkthrough. See `docs/Phase-3.md` §3.2 and `tutorial-text/Tutorial-Phase-3.md` §3.2.
- [x] **3.3 Swap Transport Layers with Zero Business Logic Impact (§3.3):**
  Workspace trimmed to 4 crates: `up-bms-proto`, `up-battery-telemetry-publisher`, `up-telemetry-subscriber`, `up-thermal-logging-subscriber`. `todo!()` placeholders for Zenoh transport wiring. Business logic (publish loop, `on_receive`, `URI filters`) unchanged from Phase 2. See `docs/Phase-3.md` §3.3.
- [x] **3.4 Bring in L3 Registration for PUBLISH + thermal fan-out (§3.4):**
  Created `up-thermal-logging-subscriber` — independent binary with `ThermalLoggingListener` reading `temp_celsius` from `BatteryTelemetry`. Fan-out demo across 4 terminals (zenohd + publisher + battery subscriber + thermal subscriber). L3 registration documented as data-space vs local-listener-table contrast. See `docs/Phase-3.md` §3.4 and `tutorial-text/Tutorial-Phase-3.md` §3.4.
- [x] **3.5 Close the demo gap — wiring summary + CLI commands (§3.5):**
  Declarative end-to-end wiring table and terminal commands to run all four processes. See `docs/Phase-3.md` §3.5.
- [x] **3.6 Key takeaway — the Phase 3 insight (§3.6):**
  The invariant: uProtocol semantics stay constant across transport swap; Zenoh enabled fan-out without application changes. See `docs/Phase-3.md` §3.6 and `docs/Phase-1.md` cliffhanger resolution.