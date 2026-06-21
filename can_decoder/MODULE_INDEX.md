# Module Index

This document provides a compact reference for each source file, its public API, and inter-module dependencies. Use this to orient new tasks without re-reading every file.

## types.rs — Data Models

**Purpose:** All shared data structures used across the pipeline.

### Public Types

| Type | Line | Description |
|---|---|---|
| `RawFrame` | 5 | Single CAN frame: `timestamp`, `can_id`, `data` bytes |
| `AssembledMessage` | 29 | Multi-frame assembled message with PGN, source/dest address, payload |
| `PGN` | 60 | J1939 Protocol Group Number extracted from CAN ID fields (priority, data_page, pgn_specific) |
| `PrettyOutput` | 94 | Decoded output enum: `Value`, `StringMessage`, `Flag` |
| `Numeric` | 116 | Value variant: `Int(i64)`, `Float(f64)`, `Hex(Vec<u8>)`, `Bool(bool)` |
| `Severity` | 129 | Message severity: `Info`, `Warning`, `Error` |
| `FlagValue` | 137 | Flag state: `Off=0, On=1, Error=2, Unavailable=3` (with `From<u8>` impl) |

### Key Methods

- `RawFrame::new(can_id, data)` — Creates with current timestamp. Used by sources.rs.
- `PGN::from_can_id(can_id)` — Extracts priority/data_page/pgn_specific from 29-bit CAN ID.
- `PGN::to_u32()` — Reconstructs CAN ID from PGN fields.
- `FlagValue::from(u8)` — Maps raw byte to enum, unknown values → `Unavailable`.

### Dependencies

- None (standalone module). All other modules import from this one.

---

## traits.rs — Pipeline Interface Traits

**Purpose:** Abstract interfaces defining the four pipeline stages. Every concrete implementation must satisfy these contracts.

### Public Traits

| Trait | Line | Key Method |
|---|---|---|
| `Source` | 12 | `start(Arc<Self>, tx) → Result<(), Box<dyn Error>>` — Produces RawFrames into channel |
| `Decoder` | 25 | `decode(frame) → Vec<PrettyOutput>` — Transforms RawFrame into decoded output |
| `Renderer` | 38 | `render(output) → String` — Formats PrettyOutput for display |
| `Filter` | 51 | `matches(output) → bool` — Decides whether to pass/drop an output item |

### Design Notes

- All traits are async (methods return `Pin<Box<dyn Future>>`).
- `Source: Send + Sync`, others only require `Send`.
- Every trait has a `name(&self)` method for logging/debugging.

### Dependencies

- Imports `PrettyOutput` and `RawFrame` from `types.rs`.

---

## sources.rs — Input Drivers

**Purpose:** Concrete implementations of the `Source` trait that feed RawFrames into the pipeline.

### Public Types

| Type | Line | Description |
|---|---|---|
| `SocketCanSource` | 20 | Live SocketCAN interface reader. Sends "Request all address claims" on startup. |
| `CandumpFileSource` | 148 | File-based candump log parser. Reads all frames at once, sends sequentially. |

### Key Methods (SocketCanSource)

- `new(iface)` — Constructor from interface name string (e.g., "can0").
- `open_socket(iface)` — Opens CAN socket with full error filter (`CAN_ERR_MASK = 0xFFFFFFFF`).
- `frame_to_raw(frame)` — Converts `socketcan::CanFrame` → `RawFrame`.
- `send_request_all_address_claims(sock)` — Broadcasts PGN 0x0EA00 request on startup.

### Key Methods (CandumpFileSource)

- `new(path)` — Constructor from path (any `Into<PathBuf>`).
- `parse_line(line)` — Parses `(sec.usec) iface id#data` format. Returns None for comments/blanks. Handles both space-separated and continuous hex data.
- `read_file(path)` — Reads entire file in blocking task, returns `Vec<RawFrame>`.

### Error Handling

- SocketCanSource: Continues on read errors with 10ms sleep; never crashes on transient I/O failures.
- CandumpFileSource: Warns on malformed lines via `eprintln!`, skips them gracefully.

### Dependencies

- Imports `RawFrame` from `types.rs`, `Source` trait from `traits.rs`.
- Uses `socketcan` crate for live interface access.
- Uses `tokio::sync::mpsc` for channel communication.

---

## pipeline.rs — Pipeline Orchestration & Stubs

**Purpose:** Manages the async pipeline wiring (Source → Decoder → Filter → Renderer) using Tokio channels and tasks. Also contains placeholder implementations of each stage.

### Public Types

| Type | Line | Description |
|---|---|---|
| `Pipeline` | 14 | Orchestrator: holds channel senders/receivers, spawns pipeline stages as Tokio tasks |
| `NullDecoder` | 136 | Stub decoder — echoes raw frame as StringMessage with hex data |
| `PassThroughFilter` | 163 | Stub filter — always returns true (passes everything) |
| `ConsoleRenderer` | 179 | Stub renderer — formats via `format_output()` into plain text |

### Pipeline Methods

- `new()` — Creates unbounded channels for Source→Decoder and Decoder→Filter+Renderer.
- `source_sender() → UnboundedSender<RawFrame>` — Returns sender for external frame injection (not currently used).
- `spawn_source(source) → JoinHandle` — Spawns Source task in its own Tokio context.
- `spawn_decoder(decoder) → JoinHandle` — Consumes decoder_rx, calls `decoder.decode()` per frame, sends results to output_tx. On decode error: logs via `eprintln!`.
- `spawn_filter(filter) → (receiver, handle)` — Consumes output_rx, applies filter per item, returns filtered receiver + task handle.
- `spawn_renderer(rx, renderer) → JoinHandle` — Consumes receiver and renderer, prints each rendered line to stdout. On render error: logs via `eprintln!`.

### Private Functions

- `format_output(output)` (line 195) — Converts PrettyOutput enum variants into display strings with formatting for Value (with unit/decimal support), StringMessage (severity-prefixed), and Flag (ON/OFF/ERR/N/A labels).

### Design Notes

- Uses `take()` pattern on Option<Receiver> to ensure each stage gets exclusive channel ownership.
- All channels are unbounded — no frames ever dropped per requirements.
- Decoder errors and render errors are logged but do not crash the pipeline.

### Dependencies

- Imports all traits from `traits.rs`, all types from `types.rs`.
- Uses `tokio::sync::{mpsc, Mutex}` for concurrency primitives.

---

## main.rs — CLI Entry Point

**Purpose:** Parses CLI arguments with clap, selects source type, wires pipeline stages together, and manages the event loop (live vs batch mode).

### Public Types

| Type | Line | Description |
|---|---|---|
| `Cli` | 17 | Clap-derived argument parser with all flags from requirements spec |
| `SourceType` | 54 | Enum: `Socketcan`, `Candump` |
| `DetailLevel` | 62 | Enum: `Raw`, `Assembled`, `Both` (parsed but not yet wired to logic) |
| `OutputFormat` | 70 | Enum: `Console`, `Json`, `Csv` (parsed but not yet wired to logic) |

### Execution Flow (`main()` at line 78)

1. Parse CLI args with clap.
2. Print configuration summary (source, interface, file, config, detail level, filters, format).
3. Create `Pipeline::new()`.
4. Match on source type:
   - `Socketcan` → requires `--interface`, creates `SocketCanSource`, calls `spawn_source()`. Exits if no interface provided.
   - `Candump` → requires `--input-file`, creates `CandumpFileSource`, calls `spawn_source()`. Exits if no file provided.
5. Wire Decoder (`NullDecoder`) → Filter (`PassThroughFilter`) → Renderer (`ConsoleRenderer`).
6. Event loop:
   - Candump mode: sleep 1s then shutdown (batch processing).
   - SocketCan mode: wait for Ctrl+C signal, then shutdown.

### Currently Unwired Options

- `--config` — Parsed but not loaded or used anywhere yet.
- `--detail-level` — Parsed but always produces assembled output (NullDecoder doesn't respect this flag).
- `--force-output-partial-tp` — Parsed but no TP reassembly exists yet to honor it.
- `--filter` — Parsed into a Vec<String> but PassThroughFilter ignores them entirely.
- `--output-format` — Parsed but always produces console output (ConsoleRenderer is hardcoded).

### Dependencies

- Imports `Source` trait from `traits.rs`.
- Uses `sources::*`, `pipeline::*` for concrete types and pipeline wiring.
- Uses `clap::Parser` for CLI parsing.

---

## device_manager.rs — Device Management & State

**Purpose:** Tracks active devices on the bus, handles address claims, and maintains a parameter cache for enriched decoding.

### Public Types

| Type | Line | Description |
|---|---|---|
| `Device` | 5 | Represents a device on the bus: `address`, `name`, `last_seen_timestamp`, `is_claimed` |
| `DeviceEvent` | 13 | Events emitted by manager: `Claimed`, `Conflict`, `Expired` |
| `DeviceManager` | 19 | The stateful manager for device tracking and parameter caching |

### Key Methods

- `new(ttl_seconds)` — Constructor with TTL for device expiration.
- `update(timestamp, address, name)` — Updates device state and handles expiration/conflicts.
- `handle_claim(address, name, timestamp)` — Processes J1939 address claim messages.
- `update_parameter(address, pgn, data)` — Stores data in the parameter cache for enrichment.
- `get_parameter(address, pgn)` — Retrieves cached data for a specific device/PGN.

### Dependencies

- Imports `PGN` and `RawFrame` from `types.rs`.
- Uses `std::collections::HashMap` for storage.

---
