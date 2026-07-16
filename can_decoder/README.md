# can_decoder

High-performance, multi-threaded CAN frame decoder for J1939 diagnostics. The system handles real-time SocketCAN data and `candump` post-processing, providing a pretty-printed view of the bus while maintaining structured data for future UI integrations.

**Robustness Guarantee**: The program never crashes due to malformed or invalid CAN message content. Bad/invalid data bytes are reported as errors in the output pipeline.

## Quick Start

```bash
# Live mode (SocketCAN)
cargo run -- --interface can0

# Post-processing from candump file
cargo run -- --source candump --input-file capture.log
```

## Project Structure

```
can_decoder/
├── src/                    # Source code
│   ├── main.rs             # CLI entry point, arg parsing, pipeline wiring
│   ├── traits.rs           # Source, Decoder, ComplexDecoder, Renderer, Filter traits
│   ├── types.rs            # RawFrame, AssembledMessage, DecodedField, Numeric, etc.
│   ├── sources.rs          # SocketCanSource, CandumpFileSource
│   ├── pipeline.rs         # Pipeline struct + NullDecoder, PassThroughFilter, ConsoleRenderer
│   ├── pgn_decoder.rs      # J1939 PGN decoder engine with 40+ standard PGNs
│   ├── tp_reassembler.rs   # Transport Protocol reassembly: BAM and RTS/CTS state machines
│   ├── device_manager.rs   # Device tracking, NAME storage, parameter cache, TTL expiration
│   ├── filters.rs          # RegexFilter, NumericFilter, FlagFilter, SeverityFilter, etc.
│   ├── task_controller.rs  # TaskController ComplexDecoder (PGN 51968 / 0xCB00)
│   └── pgn_decoders/       # YAML-based decoder definitions directory
├── tests/                  # Integration and unit test files
├── docs/                   # Project documentation (see below)
├── Cargo.toml              # Dependencies: clap, tokio, serde_yaml, owo-colors, socketcan, regex
└── requirements_and_plan.md  # Original project spec and implementation roadmap
```

## Documentation

| Document | Description |
|---|---|
| [Requirements](docs/requirements.md) | Full requirements specification: architecture overview, data models, CLI structure, testing strategy, logging/error reporting plans |
| [Design](docs/design.md) | Architecture design with pipeline diagram, module structure, trait definitions, processing stages, filter engine details, and known architecture gaps |
| [Completed Tasks](docs/completed_tasks.md) | All completed phases (1–6): foundation traits, input drivers, device management, TP reassembly, PGN decoder, complex decoder registry, filtering engine, console renderer, TaskController decoder — 194 passing tests |
| [Pending Tasks](docs/pending_tasks.md) | Remaining work: JSON/CSV/TUI/GPUI renderers, crash prevention verification, debug logging, dead code cleanup, DecodeContext/DeviceUpdate wiring, PgnFilter refactor, additional complex PGN decoders |
| [Notes](docs/notes.md) | Coding conventions, reference code sources, development workflow, test traces, key implementation decisions (TP address extraction, CAN ID construction), file interaction map, future considerations |

## Pipeline Architecture

```
RawFrame ──► TpReassembler ──► AssembledMessage ──► PgnDecoder ──► DecodedMessage
   │                │                    │                      │              │
   │            (BAM/RTS-CTS)        (PGN + payload)     (YAML or Complex)    │
   ▼                ▼                    ▼                      ▼              ▼
SocketCan       Assembled          DecodedField             Filter         Console
CandumpFile     Message           Vec<Value/Flag/Str>      PGN, Title,    Colorized
  Source        + NAMEs                                    Numeric, Regex Output
```

Four-layer pipeline: **Input** (SocketCAN / candump file) → **Processing** (TP reassembly + J1939 decoding + device management) → **Filtering** (PGN, title, numeric range, regex, severity) → **Output** (console renderer with colorized columnar display).

## CLI Reference

```
can_decoder [OPTIONS]

Required:
  --interface <IFACE>       Socket CAN interface (e.g., can0) for live mode

Optional:
  --config <PATH>           Path to YAML configuration file
  --detail-level <LEVEL>    Output detail level (raw, assembled, both) — default: assembled
  --force-output-partial-tp Emit partially reassembled TP messages on timeout
  --filter <EXPRESSION>     Add a filter rule (repeatable; see Filtering Engine)
  --output-format <FORMAT>  Output format (console, json, csv) — default: console
  --source <SOURCE>         Input source type (socketcan, candump)
  --input-file <PATH>       Path to candump-style input file (used with --source candump)
```

### Filter Syntax

| Filter | Example | Description |
|---|---|---|
| `title:` | `title:engine` | Case-insensitive substring match on titles and text |
| `pgn:` | `pgn:0xEA00` or `pgn:59904` | Match by PGN value (hex or decimal) |
| `severity:` | `severity:error` | Match severity level (info, warning, error) |
| `flag:` | `flag:status=on` | Match flag state (off, on, error, unavailable) |
| `numeric:` | `numeric:RPM:0-5000` | Match numeric values in range (supports >= and <=) |
| `regex:` | `regex:.*fault.*` | Regular expression match on string messages |

## Test Coverage

```
Module                  Tests   Status
──────────────────────── ─────── ──────
lib.rs (all modules)      145    PASS
types tests                25    PASS
device_manager tests       17    PASS
integration tests           7    PASS
──────────────────────── ─────── ──────
Total                     194    ALL PASS
```

Key test areas: TP reassembly (BAM/RTS-CTS with real traces), TaskController payload parsing, ComplexDecoder dispatch routing, device NAME management, filter parsing.

## Available Test Traces

| File | Protocol | Source ECU | Dest ECU | Payload PGN |
|---|---|---|---|---|
| `bam.log` | BAM broadcast | 0x22 (34) | 0xFF (broadcast) | 0xFF80 (Proprietary A) |
| `rts.log` | RTS/CTS unicast | 0x26 (38) | 0xEB (235) | 0xE600 (Virtual Terminal-to-Node) |

## Dependencies

| Crate | Purpose |
|---|---|
| `clap` | CLI argument parsing with derive |
| `tokio` | Async runtime and channel communication |
| `serde_yaml` | YAML configuration loading |
| `owo-colors` | Console colorization |
| `socketcan` | SocketCAN interface support |
| `regex` | Regex filter pattern matching |
| `j1939-async` | J1939 protocol fundamentals (Name, Id traits) |

## Development Workflow

1. Run `cargo build` after each change to verify compilation.
2. Add relevant unit tests alongside code changes.
3. Run `cargo test` — currently 194 tests passing.
4. For candump testing: generate sample data with `candump can0 -t a > capture.log`.

## Current Status

**Completed**: Phases 1–4 and Phase 6 (TaskController decoder). All core pipeline stages implemented with comprehensive test coverage.

**Next steps**: JSON/CSV renderers, TUI integration, crash prevention verification tests, debug logging, dead code cleanup, DecodeContext wiring, additional complex PGN decoders (ECU Status, Active Faults). See [Pending Tasks](docs/pending_tasks.md) for the full list.
