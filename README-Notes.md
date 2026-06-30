# README Notes (draft material)

Items to fold into the project README when it is written.

## Tutorial source documents

Stage-by-stage tutorial drafts live under `blog-inputs/`:

| File | Description |
|---|---|
| `Stage-0.md` | Historical baseline (`up-client` / `up-server` names) |
| `Stage-1.md` | Renamed crates, 1:1 UDS pub/sub, architectural wall narrative |

A consolidated multi-part tutorial will be produced from these stage documents later. **`main` HEAD always accompanies the latest stage** as the workspace evolves.

## Frozen checkpoints (git tags)

Readers who want the **exact code and docs for a completed stage** should check out the matching tag instead of `main`.

| Tag | Stage | What it captures |
|---|---|---|
| `Stage-1-Baseline` | Stage 1 | `up-battery-telemetry-publisher`, `up-telemetry-subscriber`, `up-frame-codec`; UDS length-framed `UMessage`; `blog-inputs/Stage-0.md` + `Stage-1.md` |

```bash
git checkout Stage-1-Baseline
cargo run -p up-telemetry-subscriber      # terminal 1
cargo run -p up-battery-telemetry-publisher  # terminal 2
```

Future stages will receive their own tags (e.g. `Stage-2-Baseline`) as the code structure changes.

## Suggested README sections (TODO)

- Project purpose: beginner-friendly uProtocol tutorial for SDV engineers
- Pointer to `blog-inputs/` for stage narratives
- Pointer to tags for stage-frozen snapshots
- Build/run quick start (from current HEAD or tagged stage)
- Link to final published tutorial (TBD)
