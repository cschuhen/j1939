# Requirements: CAN Frame Decoder

## Overview

A high-performance, multi-threaded CAN frame decoder designed for J1939 diagnostics. The system handles real-time SocketCAN data and `candump` post-processing, providing a "pretty-printed" view of the bus while maintaining structured data for future UI integrations.

**Robustness Guarantee**: The program must **never crash** due to malformed or invalid CAN message content. Bad/invalid data bytes should be reported as errors in the output pipeline. A crash should only occur if there is a genuine program bug (e.g., logic error, unhandled enum variant).

## System Architecture

The application follows a decoupled, multi-threaded pipeline using **Tokio** for async runtime and inter-thread communication via `tokio::mpsc` channels:

1. **Input Layer**: Abstracted `Source` trait to handle different data origins.
2. **Processing Layer**: Responsible for multi-frame reassembly, J1939 decoding, and state management.
3. **Filtering Layer**: Post-interpretation pluggable engine to filter decoded outputs.
4. **Output Layer**: Handles rendering to various formats (Console, JSON, CSV, etc.).

**Backpressure Strategy**: No frames are ever dropped. CAN bus throughput is low enough that all messages can be retained in RAM for the lifetime of the session. This supports both console and future TUI/GPUI modes where users may want to review historical data.

## Core Components

### 1. Data Models

- **`RawFrame`**: Represents a single CAN frame (ID, Timestamp, Data).
- **`AssembledMessage`**: Represents a logical message (e.g., a fully reassembled J1939 Transport Protocol message).
- **`DecodedField`** Enum: The primary output type for the renderer.

```rust
enum DecodedField {
    Value { title: String, value: Numeric, unit: Option<String>, decimal_places: Option<u8> },
    StringMessage { severity: Severity, text: String },
    Flag { title: String, value: FlagValue },
}

enum Numeric { Int(i64), Float(f64), Hex(Vec<u8>), Bool(bool) }
enum Severity { Info, Warning, Error }
enum FlagValue { Off=0, On=1, Error=2, Unavailable=3 }
```

The output of the processing layer for each message includes:
- A title determined by the processing layer (can be more specialized than PGN title).
- A list (vector) of `DecodedField`. One frame/message may have many components.

### 2. Input & Abstraction

- **`Source` Trait**: Defines how to pull/receive `RawFrame`s.
    - `SocketCanSource`: Real-time input via Socket CAN.
    - `CandumpFileSource`: File-based input for post-processing. Warns if file lacks timestamps. Exits on EOF.
- **Live Mode**: Sends a "Request all address claims" broadcast (Source 254 -> Dest 255).
- Multi-threading: Dedicated tasks for Input, Processing, and Output to ensure low-latency real-time decoding.

### 3. Processing & Decoding

- **Reassembler**: Handles multi-frame sequences (J1939 TP) with robust timeout handling:
    - Log error/warning as soon as J1939-defined deadline is exceeded.
    - Continue waiting up to 1.2x the timeout for additional frames, provided no new transfer from same source/destination has started.
    - `--force-output-partial-tp` emits partially assembled message on timeout rather than discarding.
    - Otherwise, discard and log a timeout error.
- **Decoder Engine**:
    - YAML Config: For standard PGNs and complex sub-protocols like ISO11783 Process Data.
    - Plugin System: Compile-time Rust modules for complex sub-protocols and custom logic.
- **Device Manager**:
    - Tracks active devices on the bus with TTL/expiration mechanism.
    - Handles dynamic "Address Claimed" messages.
    - Maintains a Parameter Cache to enrich data interpretation.
    - Detects and reports address conflicts (duplicate claims from different NAMEs for same address).

### 4. Filtering & Output

- **Filter Engine**: Post-interpretation filtering applied to `DecodedField` items, not raw frames. Enables richer filter logic based on decoded meaning rather than raw bytes.
    - Source/Destination Address
    - PGN
    - Data-byte Regex (on raw input)
    - Resolved Source/Destination Names
    - Filtered by output type (`Value`, `StringMessage`, `Flag`) or severity level
- **Filter Types**:
    - **Value Filter**: Matches numeric values in decoded output, with optional HEX support.
    - **Regex Filter**: Matches string messages, titles, or PGN names.
    - **Enum Filter**: Matches based on predefined enum values.
- **TUI / GPUI Consideration**: Full message history retained in RAM. TUI/GPUI modes provide live scrollable list with dynamic runtime filter adjustment. Console mode uses upfront CLI filter options.
- **Renderer**:
    - Phase 1: Pretty-printed console output (Colorized, Columnar).
    - Phase 2: JSON, CSV, TUI, and GPUI support.
- **Detail Level**: `--detail-level` flag toggles raw frame visibility vs assembled/decoded output.

## CLI Structure

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

Global Config:
  Read from ~/.config/j1939_decoder/config.yaml if no --config is given.
```

## Configuration

- YAML configuration files define PGN interpretations, including ISO11783 Process Data sub-field mappings.
- Default config path: `~/.config/j1939_decoder/config.yaml`.
- Explicit `--config <PATH>` overrides the default location.
- Config supports defining new PGNs without modifying Rust code.

## Dependencies / Tech Stack

| Purpose | Crate |
|---|---|
| Socket CAN | `socketcan` or `socketcan-core` |
| CLI parsing | `clap` (with derive) |
| YAML config | `serde_yaml` |
| Serialization | `serde`, `serde_json` |
| Async runtime / channels | `tokio` |
| Console styling | `owo-colors` |
| Terminal UI (future) | `ratatui` / `crossterm` |
| Regex filtering | `regex` |

## Testing Strategy

| Level | Scope | Approach |
|---|---|---|
| **Unit Tests** | Individual decoders, filters, state transitions in DeviceManager, numeric parsing, TP reassembly logic | Known byte-sequence inputs -> expected outputs; edge cases (malformed data, timeout scenarios) |
| **Integration Tests** | Full pipeline: Input -> Processing -> Filtering -> Output | Mock CAN bus feed simulating realistic multi-device traffic with multi-frame messages |
| **Local Development** | Real-world validation | Use `can0` interface on PC with active device sending real J1939 data |

## Logging Strategy (Planned)

- Pretty-print output is the primary user-facing output (console, JSON, CSV).
- A separate debug log should be available (e.g., to stderr or a file) for troubleshooting protocol issues, timeout events, device expiration, and address conflicts. Independent of filtering applied to pretty-print output.

## Error Reporting vs Crashing (Planned)

- All unit and integration tests must verify that malformed CAN frames, invalid PGN payloads, and corrupted TP sequences produce error output rather than panics.
- Panics are reserved exclusively for unrecoverable program bugs (e.g., missing enum arms, assertion failures in invariant code).
