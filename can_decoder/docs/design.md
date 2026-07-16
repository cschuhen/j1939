# Design Document: CAN Frame Decoder

## 1. Architecture Overview

The system follows a decoupled, multi-threaded pipeline architecture using Tokio for async runtime and `tokio::mpsc` channels for inter-stage communication. Data flows through four layers: Input -> Processing -> Filtering -> Output.

### Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CAN FRAME DECODER PIPELINE                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────┐    ┌──────────────────┐    ┌──────────────────────────────┐   │
│  │  INPUT   │    │   PROCESSING     │    │         FILTERING            │   │
│  │  LAYER   │───>│      LAYER       │───>│          LAYER               │   │
│  │          │    │                  │    │                              │   │
│  │ ┌────────┴┐   │ ┌───────────────┐│    │ ┌──────────────────────────┐ │   │
│  │ │SocketCan│   │ │ TpReassembler ││    │ │ FilterParser             │ │   │
│  │ │Source   │   │ │ (BAM/RTS-CTS) ││    │ │                          │ │   │
│  │ └─────────┘   │ └───────┬───────┘│    │ │ • PgnFilter              │ │   │
│  │               │         │        │    │ │ • TitleFilter            │ │   │
│  │ ┌───────────┐ │ ┌───────▼───────┐│    │ │ • NumericFilter          │ │   │
│  │ │CandumpFile│   │ │ PgnDecoder  ││    │ │ • RegexFilter            │ │   │
│  │ │Source     │   │ │             ││    │ │ • FlagFilter             │ │   │
│  │ └───────────┘ │ │ • YAML config ││    │ │ • SeverityFilter         │ │   │
│  │               │ │ • ComplexDec  ││    │ │ • CompositeFilter        │ │   │
│  │ "Request all │ │ • DeviceMgr   ││    │ └───────────┬──────────────┘ │   │
│  │  addr claims" │ │ (NAME lookup) ││    │            │                │   │
│  └───────────────┘ └───────────────┘│    │            │                │   │
│                                     │    │            ▼                │   │
│  ┌──────────────────────────────────┴──┐│    │ ┌──────────────────────────┐ │   │
│  │           DEVICE MANAGER            ││    │ │     RENDERING            │ │   │
│  │                                     ││    │ │      LAYER             │ │   │
│  │ • Device tracking with TTL          ││    │ │                          │ │   │
│  │ • NAME storage (PGN 0xEC00)         ││    │ │ ConsoleRenderer          │   │
│  │ • Parameter cache                   ││    │ │ (colorized, columnar)    │   │
│  │ • Address conflict detection        ││    │ │                          │ │   │
│  │ • Expiration cleanup                ││    │ │ ┌──────────────────────┐ │ │   │
│  └─────────────────────────────────────┘│    │ │ JSON Renderer (TODO)   │ │ │   │
│                                         │    │ │ CSV Renderer (TODO)    │ │ │   │
│  ┌─────────────────────────────────────┐│    │ │ TUI Renderer (TODO)    │ │ │   │
│  │     COMPLEX DECODER REGISTRY        ││    │ │ GPUI Renderer (TODO)   │ │ │   │
│  │                                     ││    │ └──────────────────────────┘ │ │   │
│  │ HashMap<PGN, Box<dyn ComplexDecoder>>│    │ └──────────────────────────┘ │   │
│  │ • TaskController (PGN 0xCB00)       ││    │                              │   │
│  │ • YAML fallback for unregistered    ││    │  ┌──────────────────────┐    │   │
│  │ • Ok(None) falls back to YAML       ││    │  │  OUTPUT              │    │   │
│  └─────────────────────────────────────┘│    │  │  stdout / file        │    │   │
│                                         │    │  └──────────────────────┘    │   │
│  ┌─────────────────────────────────────┐│    │                              │   │
│  │     YAML CONFIG (pgn_decoders/)     ││    │  ┌──────────────────────┐    │   │
│  │                                     ││    │  │  DEBUG LOG            │    │   │
│  │ • Standard PGN definitions          ││    │  │  (stderr / file)       │    │   │
│  │ • Component-based field mappings    ││    │  │  Independent of filter │    │   │
│  │ • Extensible without code changes   ││    │  └──────────────────────┘    │   │
│  └─────────────────────────────────────┘│    │                              │   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Flow

```
RawFrame ──► TpReassembler ──► AssembledMessage ──► PgnDecoder ──► DecodedMessage
   │                │                    │                      │              │
   │            (BAM/RTS-CTS)        (PGN + payload)     (YAML or Complex)    │
   │                │                    │                      │              │
   ▼                ▼                    ▼                      ▼              ▼
SocketCan       Assembled          DecodedField             Filter         Console
CandumpFile     Message           Vec<Value/Flag/Str>      PGN, Title,    Colorized
  Source        + NAMEs                                    Numeric, Regex Output
```

## 2. Module Structure

```
can_decoder/src/
├── main.rs                  # CLI entry point (clap arg parsing, pipeline wiring)
├── lib.rs                   # Library root, re-exports
├── traits.rs                # Source, Decoder, ComplexDecoder, Renderer, Filter traits
├── types.rs                 # RawFrame, AssembledMessage, DecodedField, Numeric, Severity,
│                            # FlagValue, DecodeContext, DeviceUpdate, DecodedMessage, DecodeError
├── sources.rs               # SocketCanSource, CandumpFileSource implementations
├── pipeline.rs              # Pipeline struct + NullDecoder, PassThroughFilter, ConsoleRenderer
├── device_manager.rs        # Device tracking, NAME storage, parameter cache, TTL expiration
├── tp_reassembler.rs        # J1939 TP reassembly: BAM and RTS/CTS state machines
├── pgn_decoder.rs           # J1939 PGN decoder engine, ComplexDecoder registry, 40+ PGNs
├── filters.rs               # RegexFilter, NumericFilter, FlagFilter, SeverityFilter,
│                            # PgnFilter, TitleFilter, CompositeFilter, FilterParser
├── task_controller.rs       # TaskControllerComplexDecoder (PGN 51968 / 0xCB00)
└── pgn_decoders/            # YAML-based decoder definitions directory
    └── iso11783/tc/         # ISO-11783 Task Controller subdirectory (placeholder)
```

## 3. Trait Definitions

### Source Trait (`sources.rs`)
Defines how to pull/receive `RawFrame`s. Implemented by `SocketCanSource` (live SocketCAN) and `CandumpFileSource` (file-based post-processing).

```rust
trait Source {
    fn start(self: Arc<Self>, tx: tokio::sync::mpsc::Sender<RawFrame>) -> Result<(), Box<dyn Error>>;
}
```

### Decoder Trait (`traits.rs`)
Receives `RawFrame`, returns `DecodedMessage`. Implemented by `J1939Decoder` (the main decoder engine).

```rust
trait Decoder {
    type Output = DecodedMessage;
    async fn decode(&mut self, frame: RawFrame) -> Result<DecodedMessage, DecodeError>;
}
```

### ComplexDecoder Trait (`traits.rs`)
For complex sub-protocols that need custom decoding logic. Registered in `J1939Decoder`'s registry and checked before YAML config dispatch. Returning `Ok(None)` falls back to YAML or unrecognized message.

```rust
trait ComplexDecoder {
    fn decode(&mut self, context: &DecodeContext, payload: &[u8])
        -> Result<Option<DecodedMessage>, DecodeError>;
}
```

### Filter Trait (`traits.rs`)
Post-interpretation filtering on `DecodedMessage`. Multiple filter types implement this trait.

```rust
trait Filter {
    fn name(&self) -> &str;
    async fn matches(&self, message: &DecodedMessage) -> bool;
}
```

### Renderer Trait (`traits.rs`)
Formats `DecodedMessage` into display string. Currently only ConsoleRenderer implemented.

```rust
trait Renderer {
    async fn render(&self, message: &DecodedMessage) -> Result<String, Box<dyn Error>>;
}
```

## 4. Data Types

### RawFrame (`types.rs`)
Single CAN frame with timestamp, can_id, and data bytes. Flows from Source to Decoder.

### AssembledMessage (`types.rs`)
Multi-frame assembled message with PGN, source/dest address, payload, and source/dest NAMEs. Created by TpReassembler for J1939 TP reassembly or by PgnDecoder for single-frame messages. Always attached to DecodedMessage (non-optional).

### DecodedField (`types.rs`)
Primary output type for the renderer. Three variants:
- `Value`: Numeric value with title, unit, and decimal precision
- `StringMessage`: Text with severity level (Info/Warning/Error)
- `Flag`: Boolean-like state (Off/On/Error/Unavailable)

### DecodedMessage (`types.rs`)
Final decode result containing a title, vector of DecodedField outputs, DeviceUpdate list, and attached AssembledMessage.

### DecodeContext (`types.rs`)
Context provided to ComplexDecoder: PGN, priority, src/dest addresses and NAMEs, timestamp. Currently defined but not yet fully wired through the pipeline (architecture gap).

### DeviceUpdate (`types.rs`)
Command returned by decoder for DeviceManager state changes (target_name, param_id, value). Currently exists but always empty (architecture gap).

## 5. Processing Pipeline Stages

### Stage 1: Transport Layer (TpReassembler)
Consumes `RawFrame`s and handles J1939 TP protocols:
- **BAM (Broadcast Announce Message)**: PGN 0xEC00, control byte 0x20, followed by TP.DT packets on PGN 0xEB00.
- **RTS/CTS (Request-to-Send / Clear-to-Send)**: PGN 0xEC00 with control bytes 0x10 (RTS), 0x11 (CTS), 0x13 (EOM).

Outputs `AssembledMessage` (complete payload + context) or partial assemblies on timeout. Handles duplicate/out-of-order rejection, timeout cleanup, and the `--force-output-partial-tp` flag.

### Stage 2: Network Layer (PgnDecoder)
Consumes `AssembledMessage`. Performs PGN lookup and dispatches to either:
1. **ComplexDecoder registry** (compile-time Rust modules, checked first)
2. **YAML config** (standard PGN definitions, fallback)

Outputs `DecodedMessage` with title and decoded fields. Enriched with source/dest NAMEs from DeviceManager.

### Stage 3: Application Layer (Filter + Renderer)
Consumes `DecodedMessage`. Filter engine applies user-specified rules (PGN, title, numeric range, regex, severity). Renderer formats output for the user (currently console-only with colorized columnar display).

## 6. Device Manager

Tracks active devices on the bus:
- **TTL Expiration**: Devices removed after inactivity period.
- **NAME Storage**: Parses 8-byte NAME payloads from PGN 0xEC00 Address Claim frames. Stores as u64 values with bitfield accessors (manufacturer, function, ecu_instance, etc.).
- **Parameter Cache**: Caches engine state and other parameters to enrich data interpretation for other PGNs.
- **Address Conflict Detection**: Reports duplicate claims from different NAMEs for the same address.

## 7. Filter Engine

Seven filter types with a generic `FilterParser` for CLI expression parsing:

| Filter | Syntax | Description |
|---|---|---|
| TitleFilter | `title:<substring>` | Case-insensitive substring match on titles and text |
| PgnFilter | `pgn:<hex_or_dec>` | Match by PGN value (e.g., `pgn:0xEA00` or `pgn:59904`) |
| SeverityFilter | `severity:<level>` | Match severity (info, warning, error) |
| FlagFilter | `flag:<title>=<value>` | Match flag state (off, on, error, unavailable) |
| NumericFilter | `numeric:<title>:<min>-<max>` | Match numeric values in range (supports >= and <=) |
| RegexFilter | `regex:<pattern>` | Regular expression match on string messages |
| CompositeFilter | N/A (programmatic) | AND logic combining multiple filters |

**Known Issue**: PgnFilter currently matches on title strings containing "PGN" via substring search rather than using the structured `pgn: u32` field from AssembledMessage. A future refactor should use `message.assembled_message.pgn()` directly.

## 8. Complex Decoder Registry

`J1939Decoder` maintains a `HashMap<PGN, Box<dyn ComplexDecoder>>`:
- Registered decoders are checked **before** YAML config during dispatch.
- Returning `Ok(None)` from a ComplexDecoder falls back to YAML or unrecognized message handling.
- Provides a clean override mechanism for PGNs needing custom decoding logic.

Currently registered:
- **TaskControllerDecoder** (PGN 0xCB00 / 51968): ISO-11783-10 Annex B.3 Process Data messages, parsing Element ID, DDI, and Value fields from 8-byte payloads.

## 9. Known Architecture Gaps

### 9.1 Unused Types
- **`DecodeContext`** (`types.rs`) is defined but never constructed or passed through the pipeline. The `ComplexDecoder::decode()` method takes a `&DecodeContext` parameter that no one calls.
- **`DeviceUpdate`** (`types.rs`) and `DecodedMessage.updates` field exist but are never populated (always `vec![]`).

### 9.2 Fragile Filter Implementation
- **PgnFilter** (`filters.rs`) matches on title strings containing "PGN" via substring search rather than using the structured `pgn: u32` field from AssembledMessage.

### 9.3 Dead Code / Stale Comments
- `extract_tp_addresses` in `tp_reassembler.rs` — unused function, adds compiler warning.
- `handle_abort` at line 402-407 has a debug print saying "not yet implemented" but doesn't actually clean up assembly state.
- `pgn_decoder.rs` — commented-out single-frame logic with incorrect PGN threshold.

## 10. Current Test Coverage

| Module | Tests | Status |
|---|---|---|
| Library (lib.rs) | 145 | All passing |
| Types tests | 25 | All passing |
| DeviceManager tests | 17 | All passing |
| Integration tests | 7 | All passing |
| **Total** | **194** | **All passing** |

Key test areas: TP reassembly (BAM/RTS-CTS with real traces), TaskController payload parsing, ComplexDecoder dispatch routing, device NAME management, filter parsing.
