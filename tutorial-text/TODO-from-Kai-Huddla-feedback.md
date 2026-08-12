# TODO: Reorient tutorial from Kai Huddla feedback

**This is the only document that tracks feedback-driven tutorial changes.** Do not add parallel DESIGN/TODO trackers; update this file instead.

**Status:** Phase 1 (P1-1…P1-4) complete. Phase 2 (P2-1, P2-2 + L2 `UPayload`) complete. Phase 3+ open.  
**Source:** [`tutorial-text/uProtocol-tutorial-feedback-by-Kai-Huddla.txt`](uProtocol-tutorial-feedback-by-Kai-Huddla.txt)  
**Path convention:** Always list **repo-relative paths** (clickable in most editors), e.g. `phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs`.

---

## Guiding intent

Preserve the tutorial’s gradual evolution (custom processes → full uEntities), but align Phase 1–3 narrative and APIs with current / upcoming `up-rust` and uProtocol layer semantics (L1 vs L2, single metadata model, L3 services vs transport-native pub/sub).

## Delivery rules (agreed)

1. **Complete one phase fully** (tutorial text + code, always in sync), then the next.
2. Do not start Phase N+1 until Phase N’s docs and `phases/0N_*` examples agree.
3. **Wording:** always **Unix Domain Socket(s)** in full in docs — never the acronym `UDS` (exception: verbatim feedback file above).
4. **Single tracker:** this file only.

---

## Phase 1 — locked decisions & paths (complete 2026-08-12)

### Primary paths touched

| Role | Path |
|------|------|
| Tutorial | [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md) |
| Publisher | [`phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs`](../phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs) |
| Publisher manifest | [`phases/01_raw_sockets/crates/up-battery-telemetry-publisher/Cargo.toml`](../phases/01_raw_sockets/crates/up-battery-telemetry-publisher/Cargo.toml) |
| Subscriber | [`phases/01_raw_sockets/crates/up-telemetry-subscriber/src/main.rs`](../phases/01_raw_sockets/crates/up-telemetry-subscriber/src/main.rs) |
| Frame codec | [`phases/01_raw_sockets/crates/up-frame-codec/src/lib.rs`](../phases/01_raw_sockets/crates/up-frame-codec/src/lib.rs) |
| Workspace | [`phases/01_raw_sockets/Cargo.toml`](../phases/01_raw_sockets/Cargo.toml) |

### Demo ID registry (all distinct; authority always `my_own_car`)

| Topic | Authority | Entity | Resource | Where |
|-------|-----------|--------|----------|--------|
| Battery telemetry (runnable) | `my_own_car` | `0x1010` | `0x8001` | docs + publisher |
| Head-lamp is-on | `my_own_car` | `0x1020` | `0x8002` | [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md) only |
| Tyre pressure | `my_own_car` | `0x101F` | `0xA010` | [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md) only |

Tyre = entity; pressure = resource of that entity.

### Canonical publisher construction (text ↔ code)

In [`phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs`](../phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs) and matching snippets in [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md):

```rust
let source_uri = UUri::try_from_parts("my_own_car", 0x1010, 1, 0x8001)?;
let message = UMessageBuilder::publish(source_uri)
    .with_ttl(5000)
    .build_with_payload(bytes, UPayloadFormat::UPAYLOAD_FORMAT_RAW)?;
```

- No hand-built `UAttributes` / `UMessage`; no `UPayload` in Phase 1 (deferred to Phase 2).
- Attributes taught as option **C**: conceptual list + inspect built message (`type_unchecked`, `source_unchecked`, `ttl_unchecked`).

### Checklist

#### P1-1. `UMessageBuilder` instead of hand-built attributes/message

- [x] Docs: [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md)
- [x] Code: [`phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs`](../phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs)

#### P1-2. Defer `UPayload` to Phase 2

- [x] Docs: [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md) (no `╔ UPayload ╗`; foreshadow Phase 2)
- [x] Code: no `UPayload` in [`phases/01_raw_sockets/`](../phases/01_raw_sockets/)

#### P1-3. `UUri::try_from_parts` / `from_str`

- [x] Docs: [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md)
- [x] Code: [`phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs`](../phases/01_raw_sockets/crates/up-battery-telemetry-publisher/src/main.rs)

#### P1-4. Spec-aligned numeric UUri IDs (Approach A; three examples)

- [x] Docs: [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md)
- [x] Code constants aligned with battery row of registry above

---

## Phase 2 — Transport naming & architecture accuracy (complete 2026-08-12)

### Paths in scope

| Role | Path |
|------|------|
| Tutorial | [`tutorial-text/Tutorial-Phase-2.md`](Tutorial-Phase-2.md) |
| Transport crate | [`phases/02_uprotocol_semantics/crates/up-unix-domain-socket-transport/`](../phases/02_uprotocol_semantics/crates/up-unix-domain-socket-transport/) |
| Transport lib | [`phases/02_uprotocol_semantics/crates/up-unix-domain-socket-transport/src/lib.rs`](../phases/02_uprotocol_semantics/crates/up-unix-domain-socket-transport/src/lib.rs) |
| Frame codec | [`phases/02_uprotocol_semantics/crates/up-frame-codec/src/lib.rs`](../phases/02_uprotocol_semantics/crates/up-frame-codec/src/lib.rs) |
| BMS proto | [`phases/02_uprotocol_semantics/crates/up-bms-proto/src/lib.rs`](../phases/02_uprotocol_semantics/crates/up-bms-proto/src/lib.rs) |
| Publisher | [`phases/02_uprotocol_semantics/crates/up-battery-telemetry-publisher/src/main.rs`](../phases/02_uprotocol_semantics/crates/up-battery-telemetry-publisher/src/main.rs) |
| Subscriber | [`phases/02_uprotocol_semantics/crates/up-telemetry-subscriber/src/main.rs`](../phases/02_uprotocol_semantics/crates/up-telemetry-subscriber/src/main.rs) |
| Workspace | [`phases/02_uprotocol_semantics/Cargo.toml`](../phases/02_uprotocol_semantics/Cargo.toml) |

### Locked type/crate names

| Role | Name |
|------|------|
| L1 type | `UnixDomainSocketTransport` |
| Crate | `up-unix-domain-socket-transport` |
| Wire setup | `::connect` (publisher) · `::bind` (subscriber) |
| Authority | `my_own_car` |
| Socket path | `{cwd}/tmp/uprotocol_twin.sock` via `up_frame_codec::socket_path()` / `ensure_socket_dir()` |

### Checklist

#### P2-1. Unify transport component in architecture diagrams

- [x] Docs: [`tutorial-text/Tutorial-Phase-2.md`](Tutorial-Phase-2.md) — one type `UnixDomainSocketTransport` (`connect` / `bind`); no Client/Server twins.
- [x] Code: [`phases/02_uprotocol_semantics/crates/up-unix-domain-socket-transport/`](../phases/02_uprotocol_semantics/crates/up-unix-domain-socket-transport/) (old `up-uds-transport` removed)

#### P2-2. Never abbreviate as “UDS”; use `UnixDomainSocketTransport` + L2 `UPayload`

**House rule:** docs always say **Unix Domain Socket(s)**; never `UDS`.

- [x] Docs: [`tutorial-text/Tutorial-Phase-2.md`](Tutorial-Phase-2.md) — rename sweep + L2 `UPayload` teaching (Chapter 7)
- [x] Code: publisher / subscriber / bms-proto / workspace use new crate + `my_own_car`
- [ ] Docs rename remaining references in [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md) (when Phase 3 pass)

---

## Phase 3 — Metadata model, Zenoh topology, L3 semantics

### Paths in scope

| Role | Path |
|------|------|
| Tutorial | [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md) |
| Workspace | [`phases/03_zenoh_topology/Cargo.toml`](../phases/03_zenoh_topology/Cargo.toml) |
| Publisher | [`phases/03_zenoh_topology/crates/up-battery-telemetry-publisher/src/main.rs`](../phases/03_zenoh_topology/crates/up-battery-telemetry-publisher/src/main.rs) |
| Subscriber | [`phases/03_zenoh_topology/crates/up-telemetry-subscriber/src/main.rs`](../phases/03_zenoh_topology/crates/up-telemetry-subscriber/src/main.rs) |
| Thermal subscriber | [`phases/03_zenoh_topology/crates/up-thermal-logging-subscriber/src/main.rs`](../phases/03_zenoh_topology/crates/up-thermal-logging-subscriber/src/main.rs) |

### P3-1. Fix “three layers of metadata” model

- [ ] Docs: [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md) — one metadata level (`UAttributes`); L2 assembles from LocalUriProvider + CallOptions + UPayload format.

### P3-2. Optional Zenoh peer-to-peer (no broker required)

- [ ] Docs: [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md)
- [ ] Optional demo paths under [`phases/03_zenoh_topology/`](../phases/03_zenoh_topology/) if added

### P3-3. Correct “L3 PUBLISH registration” vs uSubscription / transport-native pub/sub

- [ ] Docs: [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md) (esp. Chapter 5 / learning outcomes)
- [x] Docs foreshadow softened in [`tutorial-text/Tutorial-Phase-2.md`](Tutorial-Phase-2.md) Chapter 12 (no incorrect L3 registration promise)

### P3-4. Fix Chapter 9 “L4 Discovery” → L3 uDiscovery

- [ ] Docs: [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md)
- [ ] Sweep: [`tutorial-text/Tutorial-Phase-2.md`](Tutorial-Phase-2.md), [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md)

---

## Cross-cutting / editorial follow-ups

### X-1. Layer map consistency (Phases 1–3)

- [ ] [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md)
- [ ] [`tutorial-text/Tutorial-Phase-2.md`](Tutorial-Phase-2.md)
- [ ] [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md)

### X-2. Spec / API references

- [ ] UUri spec citations in [`tutorial-text/Tutorial-Phase-1.md`](Tutorial-Phase-1.md) (and later phases as edited)
- [ ] Confirm pinned `up-rust` in phase `Cargo.toml` files under [`phases/`](../phases/)

### X-3. Repository placement (organizational, non-tutorial)

- [ ] Feedback note in [`tutorial-text/uProtocol-tutorial-feedback-by-Kai-Huddla.txt`](uProtocol-tutorial-feedback-by-Kai-Huddla.txt) — own-repo discussion with maintainers (no code path).

---

## Suggested remaining order

1. **Phase 3** (P3-1…P3-4) — [`tutorial-text/Tutorial-Phase-3.md`](Tutorial-Phase-3.md) + [`phases/03_zenoh_topology/`](../phases/03_zenoh_topology/) as needed
2. **Cross-phase** (X-1, X-2), then X-3
