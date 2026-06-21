# Agent Guidelines for can_decoder

## Workspace Structure

```
can_decoder/
├── src/
│   ├── main.rs       — CLI entry point, arg parsing, pipeline wiring
│   ├── traits.rs     — Source, Decoder, Renderer, Filter trait definitions
│   ├── types.rs      — RawFrame, AssembledMessage, PGN, PrettyOutput, Numeric, Severity, FlagValue
│   ├── sources.rs    — SocketCanSource, CandumpFileSource implementations
│   └── pipeline.rs   — Pipeline struct + NullDecoder, PassThroughFilter, ConsoleRenderer stubs
├── Cargo.toml        — Dependencies: clap, tokio, serde_yaml, owo-colors, socketcan
└── requirements_and_plan.md — Full project spec and implementation roadmap
```

## File Interaction Map

| From | To | What flows |
|---|---|---|
| `main.rs` → `sources.rs` | Creates `SocketCanSource` or `CandumpFileSource`, wraps in `Arc`, passes to `pipeline.spawn_source()` |
| `main.rs` → `pipeline.rs` | Calls `spawn_source()`, `spawn_decoder()`, `spawn_filter()`, `spawn_renderer()` to wire stages |
| `sources.rs` → `types.rs` | Produces `RawFrame` instances, sends via channel |
| `pipeline.rs` (decoder stage) → `traits.rs::Decoder` | Receives `RawFrame`, returns `Vec<PrettyOutput>` |
| `pipeline.rs` (filter stage) → `traits.rs::Filter` | Checks `matches(&PrettyOutput)` to decide pass/drop |
| `pipeline.rs` (renderer stage) → `traits.rs::Renderer` | Formats `PrettyOutput` into display string |
| `types.rs` → all modules | All data types are shared across the pipeline |

## Key Types Summary

- **`RawFrame`** (`types.rs:5`) — Single CAN frame with timestamp, can_id, data bytes. Flows from Source → Decoder.
- **`AssembledMessage`** (`types.rs:29`) — Multi-frame assembled message with PGN, source/dest address, payload. Not yet used; reserved for Phase 3 reassembly.
- **`PGN`** (`types.rs:60`) — J1939 Protocol Group Number parsed from CAN ID. Utility for decoding.
- **`PrettyOutput`** (`types.rs:94`) — Decoded output enum: `Value`, `StringMessage`, or `Flag`. Flows from Decoder → Filter → Renderer.
- **`Numeric`** (`types.rs:116`) — Value variant type: `Int`, `Float`, `Hex`, `Bool`.
- **`Severity`** (`types.rs:129`) — `Info`, `Warning`, `Error` for StringMessage.
- **`FlagValue`** (`types.rs:137`) — `Off=0, On=1, Error=2, Unavailable=3`.

## Trait Signatures Summary

| Trait | Method | Input | Output | File |
|---|---|---|---|---|
| `Source` | `start(Arc<Self>, tx)` | channel sender | `Result<(), Box<dyn Error>>` | traits.rs:12 |
| `Decoder` | `decode(frame)` | `RawFrame` | `Vec<PrettyOutput>` | traits.rs:25 |
| `Renderer` | `render(output)` | `PrettyOutput` | `String` | traits.rs:38 |
| `Filter` | `matches(output)` | `&PrettyOutput` | `bool` | traits.rs:51 |

## Current Stub Implementations (pipeline.rs)

- **`NullDecoder`** — Echoes raw CAN frame as a StringMessage with hex data. Replace with real J1939 decoder in Phase 3.
- **`PassThroughFilter`** — Always returns true. Replace with actual filter logic in Phase 4.
- **`ConsoleRenderer`** — Uses `format_output()` to produce plain text. Enhance with colorization via `owo-colors` in Phase 4.

## Development Workflow

1. Run `cargo build` after each change to verify compilation.
2. Add any relevant unit tests.
3. Run `cargo test` once tests are added.
4. For candump testing: generate sample data with `candump can0 -t a > capture.log` or create manually.
5. Each subagent should focus on one file or module at a time to minimize context usage.
6. At the end of each phase: Have a subagent check for updates to AGENTS.md and MODULE_INDEX.md('git diff' to speed this up). Check off completed items in planning document. Then commit(don't push) with a sensible consise commit message.
