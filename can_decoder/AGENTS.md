# Agent Guidelines for can_decoder

## Workspace Structure

```
can_decoder/
├── src/
│   ├── main.rs           — CLI entry point, arg parsing, pipeline wiring
│   ├── traits.rs          — Source, Decoder, ComplexDecoder, Renderer, Filter trait definitions
│   ├── types.rs           — RawFrame, AssembledMessage, DecodedField, Numeric, Severity, FlagValue, DecodeContext, DeviceUpdate, DecodedMessage, DecodeError
│   ├── sources.rs         — SocketCanSource, CandumpFileSource implementations
│   ├── pipeline.rs        — Pipeline struct + NullDecoder, PassThroughFilter, ConsoleRenderer stubs
│   ├── pgn_decoder.rs     — J1939 PGN decoder engine with 40+ standard PGNs, ComplexDecoder registry (Phase 4)
│   ├── tp_reassembler.rs  — Transport Protocol reassembler: BAM and RTS/CTS state machines (Phase 3)
│   ├── device_manager.rs  — Device tracking, address claims, parameter cache, TTL expiration (Phase 2)
│   ├── filter_engine.rs   — TUI filter evaluation engine + latest-per-key index (BTreeMap<LatestKey, usize>)
│   └── latest_index.rs    — LatestKey (source, destination, topic_id): identity for the TUI Latest Topics view
├── Cargo.toml             — Dependencies: clap, tokio, serde_yaml, owo-colors, socketcan
└── requirements_and_plan.md — Full project spec and implementation roadmap
```

## File Interaction Map

| From | To | What flows |
|---|---|---|
| `main.rs` → `sources.rs` | Creates `SocketCanSource` or `CandumpFileSource`, wraps in `Arc`, passes to `pipeline.spawn_source()` |
| `main.rs` → `pipeline.rs` | Calls `spawn_source()`, `spawn_decoder()`, `spawn_filter()`, `spawn_renderer()` to wire stages |
| `sources.rs` → `types.rs` | Produces `RawFrame` instances, sends via channel |
| `pipeline.rs` (decoder stage) → `traits.rs::Decoder` | Receives `RawFrame`, returns `Vec<DecodedField>` |
| `pipeline.rs` (filter stage) → `traits.rs::Filter` | Checks `matches(&DecodedField)` to decide pass/drop |
| `pipeline.rs` (renderer stage) → `traits.rs::Renderer` | Formats `DecodedField` into display string |
| `types.rs` → all modules | All data types are shared across the pipeline |

## Key Types Summary

- **`RawFrame`** (`types.rs:5`) — Single CAN frame with timestamp, can_id, data bytes. Flows from Source → Decoder.
- **`AssembledMessage`** (`types.rs:49`) — Multi-frame assembled message with PGN, source/dest address, payload, source/dest NAMEs. Used by PgnDecoder for J1939 TP reassembly (Phase 2c/3).
- **`DecodedField`** (`types.rs:138`) — Decoded output enum: `Value`, `StringMessage`, or `Flag`. Flows from Decoder → Filter → Renderer. Derives `PartialEq`.
- **`Numeric`** (`types.rs:154`) — Value variant type: `Int`, `Float`, `Hex`, `Bool`.
- **`Severity`** (`types.rs:167`) — `Info`, `Warning`, `Error` for StringMessage.
- **`FlagValue`** (`types.rs:175`) — `Off=0, On=1, Error=2, Unavailable=3`.
- **`DecodeContext`** (`types.rs:210`) — Context provided to ComplexDecoder: PGN, priority, src/dest addresses and NAMEs, timestamp.
- **`DeviceUpdate`** (`types.rs:223`) — Command returned by decoder for DeviceManager state changes (target_name, param_id, value).
- **`DecodedMessage`** (`types.rs:231`) — Final decode result with title, outputs, updates, and attached AssembledMessage.
- **`DecodeError`** (`types.rs:197`) — Decode error enum: `InvalidLength`, `MalformedData`, `UnknownPgn`, `Internal`.

## Trait Signatures Summary

| Trait | Method | Input | Output | File |
|---|---|---|---|---|
| `Source` | `start(Arc<Self>, tx)` | channel sender | `Result<(), Box<dyn Error>>` | traits.rs:12 |
| `Decoder` | `decode(frame)` | `RawFrame` | `Future<Result<DecodedMessage, ...>>` | traits.rs:31 |
| `ComplexDecoder` | `decode(context, payload)` | `&DecodeContext`, `&[u8]` | `Result<Option<DecodedMessage>, DecodeError>` | traits.rs:46 |
| `Renderer` | `render(message)` | `&DecodedMessage` | `Future<Result<String, ...>>` | traits.rs:61 |
| `Filter` | `matches(message)` | `&DecodedMessage` | `Future<bool>` | traits.rs:74 |

## Current Stub Implementations (pipeline.rs)

- **`NullDecoder`** — Echoes raw CAN frame as a StringMessage with hex data. Kept as fallback; real PgnDecoder is used by default (Phase 3).
- **`PassThroughFilter`** — Always returns true. Replace with actual filter logic in Phase 4.
- **`ConsoleRenderer`** — Uses `format_output()` to produce plain text. Enhance with colorization via `owo-colors` in Phase 4.

## PGN Decoder Summary (pgn_decoder.rs)

- **`J1939Decoder`** implements the `Decoder` trait, receives `RawFrame`, returns `DecodedMessage`.
- Parses J1939 CAN IDs: extracts PGN from bits 8–20 of extended CAN ID.
- Handles Transport Protocol (TP) messages (PGN 0xF000–0xFDFF): Connection Management, Data Transfer, Abort, CM Next Ext CSN.
- Implements reassembler with timeout logic and `--force-output-partial-tp` support.
- **Complex Decoder Registry** (Phase 4): `HashMap<PGN, Box<dyn ComplexDecoder>>` checked before YAML config during dispatch. Registered decoders override YAML definitions; returning `Ok(None)` falls back to YAML or unrecognized message.
- Decodes 40+ standard J1939 PGNs using YAML component-based definitions including:
  - **PGN 0x0EC00**: Address Claim (NAME payload extraction)
  - **PGN 0x0FEF4**: Engine Speed (RPM, scale 0.25)
  - **PGN 0x0FEF8**: Coolant Temperature (Int8, °C)
  - **PGN 0x0EF00**: Pressure (UInt16, kPa)
  - **PGN 0x0CF00**: Vehicle Speed (km/h)
  - **PGN 0x0FECC**: Engine Oil Pressure (kPa)
  - **PGN 0x0FECE**: Engine Oil Temperature (°C)
  - **PGN 0x0FF00**: GPS Location (Float32 lat/lon)
- Uses `DeviceManager` for source address tracking and device name resolution.
- All decoders use safe byte extraction with bounds checking — never panics on malformed data.

## Development Workflow

1. Run `cargo build` after each change to verify compilation.
2. Add any relevant unit tests.
3. Run `cargo test` once tests are added.
4. For candump testing: generate sample data with `candump can0 -t a > capture.log` or create manually.
5. Each subagent should focus on one file or module at a time to minimize context usage.
6. Once coding is complete and tests pass run `cargo +nightly fmt` to fix formating issues.
7. At the end of each phase: Have a subagent check for updates to AGENTS.md and MODULE_INDEX.md('git diff' to speed this up). Check off completed items in planning document. Then commit(don't push) with a sensible consise commit message.
