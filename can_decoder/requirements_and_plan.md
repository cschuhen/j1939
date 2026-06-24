# Project Plan: CAN Frame Decoder

## Project Overview
A high-performance, multi-threaded CAN frame decoder designed for J1939 diagnostics. The system will handle real-time SocketCAN data and `candump` post-processing, providing a "pretty-printed" view of the bus while maintaining structured data for future UI integrations.

**Robustness Guarantee**: The program must **never crash** due to malformed or invalid CAN message content. Bad/invalid data bytes should be reported as errors in the output pipeline. A crash should only occur if there is a genuine program bug (e.g., logic error, unhandled enum variant).

## Coding Style
- Keep modules in separate files.
- Parts of the system that are extendable (sources, decoders, renderers, filters) must be in separate files to facilitate modularity.
- Compile-time integration is the primary focus; plugins will be first-class Rust code organized into modules rather than dynamic libraries.

## System Architecture
The application follows a decoupled, multi-threaded pipeline using **Tokio** for async runtime and inter-thread communication via `tokio::mpsc` channels:

1. **Input Layer**: Abstracted `Source` trait to handle different data origins.
2. **Processing Layer**: Responsible for multi-frame reassembly, J1939 decoding, and state management.
3. **Filtering Layer**: Post-interpretation pluggable engine to filter decoded outputs.
4. **Output Layer**: Handles rendering to various formats (Console, JSON, CSV, etc.).

**Backpressure Strategy**: No frames are ever dropped. CAN bus throughput is low enough that all messages can be retained in RAM for the lifetime of the session. This supports both console and future TUI/GPUI modes where users may want to review historical data.

## Core Components

### 1. Data Models
- `RawFrame`: Represents a single CAN frame (ID, Timestamp, Data).
- `AssembledMessage`: Represents a logical message (e.g., a fully reassembled J1939 Transport Protocol message).
- `PrettyOutput` Enum: The primary output type for the renderer.

```rust
enum PrettyOutput {
    Value { title: String, value: Numeric, unit: Option<String>, decimal_places: Option<u8> },
    StringMessage { severity: Severity, text: String },
    Flag { title: String, value: FlagValue },
}

enum Numeric {
    Int(i64),
    Float(f64),
    Hex(Vec<u8>),
    Bool(bool),
}

enum Severity { Info, Warning, Error }
enum FlagValue { Off = 0, On = 1, Error = 2, Unavailable = 3 }
```

The output of the processing layer for each message includes:
- A title that the processing layer determines for the message, this can be further specailised than the PGN title.
- A list(vector?) of PrettyOutput. One can Frame/message may have many components.

### 2. Input & Abstraction
- `Source` Trait: Defines how to pull/receive `RawFrame`s.
    - `SocketCanSource`: Real-time input via Socket CAN.
    - `CandumpFileSource`: File-based input for post-processing. Should warn user if they supply a file without timestamps. On EOF, exit.
- **Live Mode**: When running in live mode, send a "Request all address claims" onto the bus (Source: 254 → Dest: 255).
- Multi-threading: Dedicated tasks for **Input**, **Processing**, and **Output** to ensure low-latency real-time decoding.

### 3. Processing & Decoding
- **Reassembler**: Handles multi-frame sequences (J1939 TP) with robust timeout handling:
    - Log an error/warning as soon as the J1939-defined deadline is exceeded.
    - Continue waiting up to **1.2× the timeout** for additional frames, provided no new transfer from the same source/destination has started.
    - If `--force-output-partial-tp` is set, emit the partially assembled message at timeout rather than discarding it.
    - Otherwise, discard and log a timeout error.
- **Decoder Engine**:
    - **YAML Config**: For standard PGNs and complex sub-protocols like ISO11783 Process Data.
    - **Plugin System**: Compile-time Rust modules for complex sub-protocols and custom logic.
- **Device Manager**:
    - Tracks active devices on the bus with a TTL/expiration mechanism (device removed after inactivity).
    - Handles dynamic "Address Claimed" messages.
    - Maintains a **Parameter Cache** to enrich data interpretation (e.g., caching engine state to decode other PGNs).
    - Detects and reports address conflicts (duplicate claims from different NAMEs for the same address).

### 4. Filtering & Output
- **Filter Engine**: Post-interpretation filtering applied to `PrettyOutput` items, not raw frames. This enables richer filter logic based on decoded meaning rather than raw bytes.
    - Source/Destination Address
    - PGN
    - Data-byte Regex (on raw input)
    - Resolved Source/Destination Names
    - Filtered by output type (`Value`, `StringMessage`, `Flag`) or severity level
- **Filter Engine API**: There should be a generic API to the list of possible filters. This should allow the different front-ends to generate a 'UI' of some description to the user. For the CLI, this would be a simple list of filter options with descriptions, also the ability for the program to generate and install bash-completion config. For TUI/GPUI, the filter UI would be generated dynamically based on the available filters. This generic API needs to support different types of filters:
- **Filter Types**: 
    - **Value Filter**: Matches numeric values in the decoded output. Some with the option to enter in HEX.
    - **Regex Filter**: Matches string messages in the decoded output or Tiles or PGN names.
    - **Enum Filter**: Matches based on a list of predefined enum values.
- **TUI / GPUI Consideration**: The system retains the full message history in RAM. In TUI/GPUI modes, users see a live scrollable list of all messages and can adjust filters dynamically at runtime without restarting. Console mode uses upfront CLI filter options.
- **Renderer**:
    - **Phase 1**: Pretty-printed console output (Colorized, Columnar).
    - **Phase 2**: JSON, CSV, TUI, and GPUI support.
- **Detail Level**: `--detail-level` flag to toggle raw frame visibility vs. assembled/decoded output.

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
  A global config may also be read from ~/.config/j1939_decoder/config.yaml if no --config is given.
```

## Configuration

- YAML configuration files define PGN interpretations, including ISO11783 Process Data sub-field mappings.
- Default config path: `~/.config/j1939_decoder/config.yaml`.
- Explicit `--config <PATH>` overrides the default location.
- Config supports defining new PGNs without modifying Rust code — ideal for standard/supported PGNs.

## Dependencies / Tech Stack

| Purpose | Crate |
|---|---|
| Socket CAN | `socketcan` or `socketcan-core` |
| CLI parsing | `clap` (with derive) |
| YAML config | `serde_yaml` |
| Serialization | `serde`, `serde_json` |
| Async runtime / channels | `tokio` |
| Console styling | `colored` or `owo-colors` |
| Terminal UI (future) | `ratatui` / `crossterm` |

**Reference Code**: Existing Rust modules in `/home/cschuhen/rust/j1939` will be leveraged for J1939 protocol fundamentals. This planning document and its artifacts will eventually migrate to that repository once planning is complete. We can use this library or reference it. Prefer to use it directly.

## Implementation Roadmap

### Phase 1: Foundation & Core Traits
- [x] Workspace setup with dependencies listed above.
- [x] Define `Source`, `Decoder`, `Renderer`, and `Filter` traits.
- [x] Implement core data types (`RawFrame`, `AssembledMessage`, `PrettyOutput`, `Numeric`).
- [x] Implement basic Tokio-based multi-threaded pipeline with channels.
- [x] **Testing**: Unit tests for core traits, data types, and channel communication.

### Phase 2: Input Drivers & Device Management
- [x] Implement `SocketCanSource` (live mode with "Request all address claims" broadcast).
- [x] Implement `CandumpFileSource`.
- [x] Implement `DeviceManager`: dynamic claim handling, parameter cache, TTL expiration, address conflict detection.
- [x] **Testing**: Integration tests for data flow from source through processing pipeline.

### Phase 3: J1939 & Protocol Decoding
- [x] Implement J1939 Transport Protocol (TP) reassembly with timeout logic and `--force-output-partial-tp`.
- [x] Implement PGN decoder engine with YAML configuration support.
- [x] Implement compile-time plugin modules for complex sub-protocols.
- [x] **Testing**: 97 unit + integration tests covering all decoders, TP reassembly, and edge cases (malformed data, timeout scenarios).

### Phase 4: Filtering & Console UI
- [x] Full CLI structure with all flags from the spec.
- [x] Implement post-interpretation pluggable Filtering Engine (address, PGN, name, severity, regex).
- [x] Implement Console Renderer (colorized, columnar output using `owo-colors`).
- [ ] **Testing**: Filter logic tests with complex expressions; end-to-end console rendering tests.

### Phase 5: Structured Output & Advanced UI
- [ ] Implement JSON and CSV formatters.
- [ ] Develop TUI (ratatui) integration with live filter editing.
- [ ] Develop GPUI integration.
- [ ] **Testing**: End-to-end integration tests mocking a full CAN bus sequence including multi-frame reassembly, filtering, and output rendering.

## Testing Strategy

| Level | Scope | Approach |
|---|---|---|
| **Unit Tests** | Individual decoders, filters, state transitions in `DeviceManager`, numeric parsing, TP reassembly logic | Known byte-sequence inputs → expected outputs; edge cases (malformed data, timeout scenarios) |
| **Integration Tests** | Full pipeline: Input → Processing → Filtering → Output | Mock CAN bus feed simulating realistic multi-device traffic with multi-frame messages |
| **Local Development** | Real-world validation | Use `can0` interface on this PC with active device sending real J1939 data |

### Error Reporting vs Crashing
- [ ] All unit and integration tests must verify that malformed CAN frames, invalid PGN payloads, and corrupted TP sequences produce error output rather than panics.
- [ ] Panics are reserved exclusively for unrecoverable program bugs (e.g., missing enum arms, assertion failures in invariant code).

## Logging Strategy

- [ ] **Pretty-print output** is the primary user-facing output (console, JSON, CSV).
- [ ] A separate **debug log** should be available (e.g., to stderr or a file) for troubleshooting protocol issues, timeout events, device expiration, and address conflicts. This is independent of the filtering applied to pretty-print output.

## Agent Workflow Guidelines

When working on this project, use subagents to manage context size effectively:

- **One task per subagent**: Delegate discrete features (e.g., "Implement DeviceManager", "Add JSON renderer") to separate subagent invocations rather than handling everything in a single conversation.
- **File-level scoping**: When assigning a task, specify exactly which files the agent should read and modify. This prevents unnecessary context loading.
- **Incremental verification**: After each subagent completes its work, run `cargo build` and `cargo test` to verify correctness before proceeding to the next task.
- **Documentation first**: Before starting complex features, have a subagent create or update the module documentation (doc comments) so subsequent agents understand the existing API surface without re-reading every file.
