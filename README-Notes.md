# README Notes (draft material)

Items to fold into the project README when it is written. **Until then, treat this file as the canonical repo guide.**

## Repository layout on `main`

Each tutorial chapter has a **frozen code snapshot** under `phases/`. Narrative markdown lives at the repo root. Active development for the next chapter happens in the latest phase directory; earlier phases are copy-forward snapshots and are not rewritten.

| Phase directory | Chapter | Code |
|---|---|---|
| `phases/01_raw_sockets/` | Stage 1 | `up-frame-codec`, publisher, subscriber — UDS length-framed `UMessage` |
| `phases/02_uprotocol_semantics/` | Stage 2 (WIP) | above + `up-uds-transport`; binaries still Stage 1 style until 2.7–2.8 |

The pre-migration layout (`crates/` at repo root) is **retired**. All chapter code on `main` lives under `phases/`.

## Tutorial source documents

Stage-by-stage tutorial drafts at the repo root:

| File | Description |
|---|---|
| `Stage-0.md` | Historical baseline (`up-client` / `up-server` names) |
| `Stage-1.md` | Renamed crates, 1:1 UDS pub/sub, architectural wall narrative |
| `Stage-2.md` | uProtocol layers (2.1–2.3), `up-uds-transport` (2.4); binaries not refactored yet |
| `uProtocol-tutorial-draft-1.md` | Consolidated tutorial draft (in progress) |

## Build and run

Each phase is an independent Cargo workspace. Run all commands **from the repo root** unless noted.

General pattern:

```bash
cargo build   --manifest-path phases/<NN>_<name>/Cargo.toml
cargo run     --manifest-path phases/<NN>_<name>/Cargo.toml -p <package-name>
cargo test    --manifest-path phases/<NN>_<name>/Cargo.toml -p <package-name>
```

### Phase 01 — `phases/01_raw_sockets/` (Stage 1)

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

### Phase 02 — `phases/02_uprotocol_semantics/` (Stage 2, WIP)

Build the whole workspace:

```bash
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml
```

Run the demo (binaries still use Stage 1 style until sections 2.7–2.8):

```bash
# Terminal 1
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber

# Terminal 2
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```

Test the Stage 2 transport crate:

```bash
cargo test --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-uds-transport
```

Build individual crates:

```bash
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-frame-codec
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-uds-transport
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```

## Frozen checkpoints (git tags)

Tags mark a chapter snapshot. **`Stage-1-Baseline` uses the same `phases/` layout as `main`** — it points at the commit where Stage 1 was first frozen under `phases/01_raw_sockets/`.

| Tag | Stage | Code path | Run from tag checkout |
|---|---|---|---|
| `Stage-1-Baseline` | Stage 1 | `phases/01_raw_sockets/` | same `--manifest-path` commands as above |

To check out Stage 1 only:

```bash
git checkout Stage-1-Baseline
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-telemetry-subscriber
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-battery-telemetry-publisher
git checkout main   # return to latest chapter work
```

On `main` you get **all phases side by side**; on the tag you get Stage 1 (and whatever else existed at that commit — today, also Stage 2 WIP).

Future stages will receive their own tags (e.g. `Stage-2-Baseline`) when shipped.

## Stage 2 plan (in progress — paused after 2.4)

Completed: `Stage-2.md` (sections 2.1–2.4), `phases/02_uprotocol_semantics/crates/up-uds-transport`.

Pending before `Stage-2-Baseline` tag: 2.5–2.6 (benefits/limits narrative), 2.7–2.8 (refactor binaries), 2.9–2.10 (evaluate + tag).

See `TUTORIAL-ROADMAP.md` Phase 2 for full checklist.

## Suggested README sections (TODO — migrate from here when writing final README)

- Project purpose: beginner-friendly uProtocol tutorial for SDV engineers
- Repository layout: `phases/` snapshots + root `Stage-*.md` narratives
- Build/run quick start per phase (copy from **Build and run** above)
- Git tags: chapter checkpoints; `Stage-1-Baseline` → `phases/01_raw_sockets/`
- Link to final published tutorial (TBD)
