# uProtocol Tutorial — From Raw Sockets to uP-L1/L2

A hands-on tutorial for software engineers who want to understand [uProtocol](https://github.com/eclipse-uprotocol) by building working code, step by step.

*This is a tutorial that I have created, while learning uProtocol. I have captured the way I have
tried to understand the 'Why', 'What' and 'How' of uProtocol. Hopefully, this will be of help to
others, stepping into the world of uProtocol.*

We start with **raw Unix Domain Sockets** and manually frame uProtocol `UMessage` bytes. Then you 
refactor behind uProtocol's own L1 (`UTransport` / `UListener`) and L2 (`SimplePublisher` / `CallOptions`) abstractions — without changing the wire.

## Repository layout

Each tutorial chapter has a frozen code snapshot under `phases/`. Narrative drafts live in `tutorial-text/`.

```
├── phases/
│   ├── 01_raw_sockets/           # Stage 1 – UDS + length-prefix framing
│   └── 02_uprotocol_semantics/   # Stage 2 – uP-L1/L2 over the same UDS wire
├── tutorial-text/
│   ├── uProtocol-tutorial-draft-1.md    # Stage 1 narrative
│   ├── uProtocol-tutorial-draft-2.md    # Stage 2 narrative
│   └── uProtocol-tutorial-draft-3-phase3.md   # (placeholder for Stage 3)
└── docs/
    ├── Stage-0.md                # Historical baseline (original crate names)
    ├── Stage-1.md                # Stage 1 deep-dive
    ├── Stage-2.md                # Stage 2 deep-dive
    └── Stage-3.md                # Stage 3 planning brief
```

## The tutorial documents

Start with the narrative drafts under [`tutorial-text/`](./tutorial-text/):

| File | What it covers                                                                                                                                                                                                                                                                                                                                                                          |
|---|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| [`uProtocol-tutorial-draft-1.md`](./tutorial-text/uProtocol-tutorial-draft-1.md) | **Phase 1 — Raw sockets.** Build `UMessage` envelopes by hand, frame them with a 4-byte length prefix, and send over a Unix Domain Socket. Two processes — a publisher and a subscriber — exchange battery telemetry (SoC, temperature) as raw packed bytes. One sees every layer of the wire with no library hiding the details.                                                       |
| [`uProtocol-tutorial-draft-2.md`](./tutorial-text/uProtocol-tutorial-draft-2.md) | **Phase 2 — uProtocol semantics.** The same UDS wire is wrapped behind uProtocol's L1 (`UTransport`, `UListener`) and L2 (`SimplePublisher`, `CallOptions`). The raw CAN-frame packing is replaced by a protobuf schema (`bms_telemetry.proto`). The transport crate (`up-uds-transport`) centralizes framing and dispatch; application code no longer touches sockets or byte headers. |
| `uProtocol-tutorial-draft-3-phase3.md` | *(TODO — placeholder for Stage 3)*                                                                                                                                                                                                                                                                                                                                                      |

The `docs/` directory holds deeper technical reference for each stage, including code walkthroughs, architectural diagrams, and design decisions.

## Quick start — run the demo

Each phase is an independent Cargo workspace. Run all commands from the repo root.

### Phase 1 — Raw sockets

```bash
cargo build --manifest-path phases/01_raw_sockets/Cargo.toml

# Terminal 1 — subscriber
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-telemetry-subscriber

# Terminal 2 — publisher (sends 5 messages, then exits)
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-battery-telemetry-publisher
```

### Phase 2 — uProtocol semantics

```bash
cargo build --manifest-path phases/02_uprotocol_semantics/Cargo.toml

# Terminal 1 — subscriber
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-telemetry-subscriber

# Terminal 2 — publisher
cargo run --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-battery-telemetry-publisher
```

Optional — enable `RUST_LOG=trace` on both terminals to see `up-uds-transport` dispatch logs.

Run the Stage 2 transport crate tests:

```bash
cargo test --manifest-path phases/02_uprotocol_semantics/Cargo.toml -p up-uds-transport
```

## Git tags — frozen chapter checkpoints

Tags mark the exact commit of a completed chapter, so you can check out a single stage without later phases.

| Tag | Chapter | Code path |
|---|---|---|
| `Stage-1-Baseline` | Stage 1 | `phases/01_raw_sockets/` |
| `Stage-2-Baseline` | Stage 2 | `phases/02_uprotocol_semantics/` |

```bash
git checkout Stage-1-Baseline
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p up-telemetry-subscriber
# ... then back to main to read Stage 2
git checkout main
```

## What we will learn

- How uProtocol's `UUri`, `UAttributes`, `UPayload`, and `UMessage` map to the wire.
- Why stream transports (Unix Domain Sockets) need explicit length-prefix framing.
- How uProtocol's L1 (`UTransport` / `UListener`) separates message moving from message handling.
- How uProtocol's L2 (`SimplePublisher` / `CallOptions`) separates publishing intent from envelope construction.
- Why the same wire can carry different semantic models — and why that matters when you swap transports (Zenoh, etc.).

## Prerequisites

- Rust toolchain (edition 2024)
- Linux (Unix Domain Sockets)
- No prior uProtocol knowledge assumed

## License

TBD

---