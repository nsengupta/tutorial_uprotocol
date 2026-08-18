# README Notes (draft material)

Items to fold into the project README when it is written. **Until then, treat this file as the canonical repo guide.**

## Repository layout on `main`

Each tutorial chapter has a **frozen code snapshot** under `phases/`. Narrative markdown lives at the repo root. Active development for the next chapter happens in the latest phase directory; earlier phases are copy-forward snapshots and are not rewritten.

| Phase directory | Chapter | Code |
|---|---|---|
| `phases/01_raw_sockets/` | Phase 1 | `up-frame-codec`, publisher, subscriber — Unix Domain Socket length-framed `UMessage` |
| `phases/02_uprotocol_semantics/` | Phase 2 | `up-bms-proto`, `up-unix-domain-socket-transport`, refactored publisher/subscriber — uProtocol L1/L2 over a Unix Domain Socket |

The pre-migration layout (`crates/` at repo root) is **retired**. All chapter code on `main` lives under `phases/`.

## Tutorial source documents

Phase-by-phase tutorial and reference documents:

| File | Description |
|---|---|
| `tutorial-text/Tutorial-Phase-1.md` | Consolidated Phase 1 tutorial |
| `tutorial-text/Tutorial-Phase-2.md` | Phase 2 tutorial narrative |
| `tutorial-text/Tutorial-Phase-3.md` | Phase 3 tutorial narrative |
| `Notes/uProtocol-tutorial-feedback-by-Kai-Huddla.txt` | Maintainer feedback on the tutorial |

## Build and run

Each phase is an independent Cargo workspace. Run all commands **from the repo root** unless noted.

General pattern:

```bash
cargo build   --manifest-path phases/<NN>_<name>/Cargo.toml
cargo run     --manifest-path phases/<NN>_<name>/Cargo.toml -p <package-name>
cargo test    --manifest-path phases/<NN>_<name>/Cargo.toml -p <package-name>
```
### Phase 01 — `phases/01_raw_sockets/` (Phase 1)

Build the whole workspace:

```bash
cargo build --manifest-path phases/01_raw_sockets/Cargo.toml
```
Run the demo (two terminals; subscriber first):

```bash
# Terminal 1 — subscriber (waits for publisher)
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-telemetry-subscriber

# Terminal 2 — publisher (sends 5 messages, then exits)
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-battery-telemetry-publisher
```
Build or run a single crate:

```bash
cargo build --manifest-path phases/01_raw_sockets/Cargo.toml -p up-frame-codec
cargo build --manifest-path phases/01_raw_sockets/Cargo.toml -p up-telemetry-subscriber
cargo build --manifest-path phases/01_raw_sockets/Cargo.toml -p up-battery-telemetry-publisher
```
### Phase 02 — `phases/02_uprotocol_semantics/` (Phase 2)

Build the whole workspace:

```bash
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml
```
Run the demo (two terminals; subscriber first):

```bash
# Terminal 1 — subscriber (exits after 5 messages)
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber

# Terminal 2 — publisher (sends 5 messages, then exits)
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```
Optional — trace logs confirming `up-unix-domain-socket-transport` path (`RUST_LOG=trace` on both).

Test the Phase 2 transport crate:

```bash
cargo test --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-unix-domain-socket-transport
```
Build individual crates:

```bash
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-frame-codec
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-bms-proto
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-unix-domain-socket-transport
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```
## Phase checkpoints

Each phase is a self-contained workspace under `phases/`. On `main`, all phases sit side by side — run any chapter directly with `--manifest-path`; no git checkout required.

| Phase | Code path |
|---|---|
| Phase 1 | `phases/01_raw_sockets/` |
| Phase 2 | `phases/02_uprotocol_semantics/` |
| Phase 3 | `phases/03_zenoh_topology/` |

## Phase 2 status

**Complete** (sections 2.1–2.10): `tutorial-text/Tutorial-Phase-2.md`, `phases/02_uprotocol_semantics/` (`up-bms-proto`, `up-unix-domain-socket-transport`, refactored binaries). Verified: build, `up-unix-domain-socket-transport` test, 5-message demo.

**Your step:** commit when ready. Next narrative: `tutorial-text/Tutorial-Phase-2.md`. Next code chapter: Phase 3 (`tutorial-text/Tutorial-Phase-3.md`).

## Suggested README sections (TODO — migrate from here when writing final README)

- Project purpose: beginner-friendly uProtocol tutorial for SDV engineers
- Repository layout: `phases/` snapshots + `tutorial-text/Tutorial-Phase-*.md` narratives
- Build/run quick start per phase (copy from **Build and run** above)
- Link to final published tutorial (TBD)
