# Agent Guidelines for can_decoder

## Workspace Structure

```
can_decoder/
├── src/
│   ├── main.rs           — CLI entry point, arg parsing, pipeline wiring
│   ├── traits.rs          — Source, Decoder, Renderer, Filter trait definitions
│   ├── types.rs           — RawFrame, AssembledMessage, PGN, DecodedField, Numeric, Severity, FlagValue
│   ├── sources.rs         — SocketCanSource, CandumpFileSource implementations
│   ├── pipeline.rs        — Pipeline struct + NullDecoder, PassThroughFilter, ConsoleRenderer stubs
│   ├── pgn_decoder.rs     — J1939 PGN decoder engine with 40+ standard PGNs (Phase 3)
│   └── device_manager.rs  — Device tracking, address claims, parameter cache, TTL expiration (Phase 2)
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
- **`AssembledMessage`** (`types.rs:29`) — Multi-frame assembled message with PGN, source/dest address, payload. Used by PgnDecoder for J1939 TP reassembly (Phase 3).
- **`PGN`** (`types.rs:60`) — J1939 Protocol Group Number parsed from CAN ID. Utility for decoding.
- **`DecodedField`** (`types.rs:94`) — Decoded output enum: `Value`, `StringMessage`, or `Flag`. Flows from Decoder → Filter → Renderer.
- **`Numeric`** (`types.rs:116`) — Value variant type: `Int`, `Float`, `Hex`, `Bool`.
- **`Severity`** (`types.rs:129`) — `Info`, `Warning`, `Error` for StringMessage.
- **`FlagValue`** (`types.rs:137`) — `Off=0, On=1, Error=2, Unavailable=3`.

## Trait Signatures Summary

| Trait | Method | Input | Output | File |
|---|---|---|---|---|
| `Source` | `start(Arc<Self>, tx)` | channel sender | `Result<(), Box<dyn Error>>` | traits.rs:12 |
| `Decoder` | `decode(frame)` | `RawFrame` | `Vec<DecodedField>` | traits.rs:25 |
| `Renderer` | `render(output)` | `DecodedField` | `String` | traits.rs:38 |
| `Filter` | `matches(output)` | `&DecodedField` | `bool` | traits.rs:51 |

## Current Stub Implementations (pipeline.rs)

- **`NullDecoder`** — Echoes raw CAN frame as a StringMessage with hex data. Kept as fallback; real PgnDecoder is used by default (Phase 3).
- **`PassThroughFilter`** — Always returns true. Replace with actual filter logic in Phase 4.
- **`ConsoleRenderer`** — Uses `format_output()` to produce plain text. Enhance with colorization via `owo-colors` in Phase 4.

## PGN Decoder Summary (pgn_decoder.rs)

- **`PgnDecoder`** implements the `Decoder` trait, receives `RawFrame`, returns `Vec<DecodedField>`.
- Parses J1939 CAN IDs: extracts PGN from bits 8–20 of extended CAN ID.
- Handles Transport Protocol (TP) messages (PGN 0xF000–0xFDFF): Connection Management, Data Transfer, Abort, CM Next Ext CSN.
- Implements reassembler with timeout logic and `--force-output-partial-tp` support.
- Decodes 40+ standard J1939 PGNs including:
  - **PGN 0xEA00**: ECU Status (8 numeric values + severity)
  - **PGN 0xFE8D**: Active Faults (up to 50 fault records with status, priority, source address)
  - **PGN 0x00FF–0x0EFU**: Engine/Rail parameters (RPM, speed, fuel rate, temperatures, pressures)
  - **PGN 0x0700–0x07FF**: Diagnostic trouble codes, odometer, intake manifold, battery voltage, oil temp/pressure
  - **PGN 0x0800–0x08FF**: Transmission parameters (gear, clutch, SAE J1939-81)
  - **PGN 0x0903–0x09FE**: Aftertreatment, DPF soot/ash load, NOx sensors
  - **PGN 0x0A00–0x0AFF**: Level sensors (fuel, AdBlue), position (lat/lon), heading
  - **PGN 0x0B00–0x0BEF**: Vehicle parameters (mileage, trip, cruise control)
  - **PGN 0x0BFU**: Driver ID, operator messages
  - **PGN 0x0CFU**: Parameter list requests/responses
  - **PGN 0x0D00–0x0DFF**: Calibration (ID, verification, history), VIN
  - **PGN 0x0E00–0x0EFF**: Event data recording (up to 50 events)
  - **PGN 0x0F00–0x0FFF**: Vehicle ID (NAME, manufacturer code, ECU instance, software/firmware versions)
- Uses `DeviceManager` for source address tracking and device name resolution.
- All decoders use safe byte extraction with bounds checking — never panics on malformed data.

## Development Workflow

1. Run `cargo build` after each change to verify compilation.
2. Add any relevant unit tests.
3. Run `cargo test` once tests are added.
4. For candump testing: generate sample data with `candump can0 -t a > capture.log` or create manually.
5. Each subagent should focus on one file or module at a time to minimize context usage.
6. At the end of each phase: Have a subagent check for updates to AGENTS.md and MODULE_INDEX.md('git diff' to speed this up). Check off completed items in planning document. Then commit(don't push) with a sensible consise commit message.
