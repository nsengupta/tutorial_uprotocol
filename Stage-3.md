# Stage 3: Input brief (from Stage 2 limits)

> **Status:** Planning document — not the Stage 3 tutorial yet.  
> **Source:** `Stage-2.md` §2.6, `Stage-1.md` (second consumer / thermal scenario), `TUTORIAL-ROADMAP.md` Phase 3.  
> **Prerequisite when written:** Stage 2 shipped (`Stage-2-Baseline` tag, code in `phases/02_uprotocol_semantics/`).

Stage 2 improved **semantics and application structure**. Stage 3 must address everything Stage 2 **explicitly did not fix**. This file captures that handoff so `Stage-3.md` (full chapter) and `uProtocol-tutorial-draft-2.md` (consolidated narrative) can be written without re-deriving the cliffhanger.

---

## What Stage 2 delivered (do not throw away)

| Layer | Stage 2 outcome | Carries into Stage 3 |
|---|---|---|
| Envelope | `UMessage`, `UAttributes`, `UUri`, `UPayload` | Unchanged |
| Application payload | `BatteryTelemetry` protobuf (`up-bms-proto`) | Unchanged; thermal types added later |
| L2 | `SimplePublisher`, `CallOptions`, `UPayload::try_from_protobuf` | Unchanged |
| L1 API | `UTransport::send`, `register_listener`, `UListener::on_receive` | **Same trait** — different plugin |
| Business logic | SoC/temp publish loop; subscriber `on_receive` body | Should remain semantically identical |

**Copy-forward rule:** Stage 3 replaces **wire execution** and adds **L3**; Stage 2 semantics stay.

---

## What Stage 2 did not fix (Stage 3 must address)

These are the **use cases still infeasible** after Phase 2. Each row is a tutorial driver.

| # | Limitation | Phase 1 symptom | Still true after Phase 2 | Stage 3 direction |
|---|---|---|---|---|
| 1 | **No fan-out** | Second consumer (thermal logger) cannot tap the stream | URI filters route *in-process* only; UDS is point-to-point | Zenoh (or similar) native multi-subscriber delivery |
| 2 | **Filesystem socket path** | `/tmp/uprotocol_twin.sock` binds both sides to one host | Same path, same host — `UdsTransport` did not change topology | Network-facing transport; socket path deleted |
| 3 | **No location transparency** | Publisher/subscriber hard-code where the peer lives | Still hard-code `SOCKET_PATH`; logical URI does not resolve across machines | Zenoh session / data space; URI + L3 as address |
| 4 | **No L3 discovery / registration** | Out-of-band knowledge of socket + URI filters | `register_listener` is local `HashSet` in one process | L3 PUBLISH registration over vehicle data space |
| 5 | **Stream / broker semantics** | Length-prefix framing; no topic namespace | `up-frame-codec` + per-connection read; no broker | Drop local framing loop; broker-capable transport |

### Narrative anchor: thermal logging engine

From `Stage-1.md`: an independent **Thermal Management Logging Engine** must monitor the **same** battery temperature stream without sharing the battery subscriber's process.

- Stage 1 exposed the problem (UDS point-to-point).
- Stage 2 let each consumer **declare intent** (`UListener` + URI filter) — but only if fan-out existed.
- Stage 3 is the **payoff**: thermal crate subscribes independently; battery subscriber unchanged.

Scope when Stage 3 ships: add thermal consumer + fan-out; battery SoC/temp protobuf stays as-is.

---

## Topology trigger (planned §3.1)

Move the battery telemetry **uEntity** off the central compute node to a **Zone ECU** on Automotive Ethernet. At that moment:

```text
/tmp/uprotocol_twin.sock  →  dead (filesystem path on wrong machine)
```

Stage 3 opens with this constraint — not an optional optimisation.

---

## Planned chapter outline (from roadmap)

| Section | Topic |
|---|---|
| **3.1** | Network topology constraint — ECU move kills UDS path |
| **3.2** | Zenoh as supplementary transport (location transparency) |
| **3.3** | Swap `up-uds-transport` / `up-frame-codec` loop for `up-client-zenoh-rust`; business logic untouched |
| **3.4** | L3 PUBLISH registration; thermal subscriber + fan-out demo |

---

## Code migration sketch (Phase 2 → Phase 3)

```
phases/02_uprotocol_semantics/          phases/03_zenoh_topology/  (name TBD)
├── up-bms-proto/                  →   copy-forward (unchanged)
├── up-battery-telemetry-publisher/ →   same publish loop; Zenoh UTransport config
├── up-telemetry-subscriber/       →   same on_receive; Zenoh UTransport config
├── up-uds-transport/              →   retired from active demo (frozen snapshot)
└── up-frame-codec/                →   retired from active path (Zenoh handles wire)
                                       + thermal-logging-subscriber (new)
                                       + up-client-zenoh-rust (dependency)
```

---

## Links for authors

| Document | Role |
|---|---|
| `Stage-2.md` §2.6 | Full limitation prose + tables |
| `Stage-1.md` | Second consumer pseudo-code friction |
| `TUTORIAL-ROADMAP.md` Phase 3 | Checklist |
| `uProtocol-tutorial-draft-1.md` | Chapter 1 voice/style reference |
| `uProtocol-tutorial-draft-2.md` | **TBD** — consolidated Stage 2 narrative after §2.9–2.10 |
