# Completed Tasks

## Phase 1: Foundation & Core Traits ✅ COMPLETED

- [x] Workspace setup with dependencies (clap, tokio, serde_yaml, owo-colors, socketcan, regex).
- [x] Define `Source`, `Decoder`, `Renderer`, `Filter`, and `ComplexDecoder` traits in `traits.rs`.
- [x] Implement core data types in `types.rs`: `RawFrame`, `AssembledMessage`, `DecodedField`, `Numeric`, `Severity`, `FlagValue`, `DecodeError`, `DecodeContext`, `DeviceUpdate`, `DecodedMessage`.
- [x] Implement basic Tokio-based multi-threaded pipeline with channels in `pipeline.rs`.
- [x] Unit tests for core traits, data types, and channel communication.

## Phase 2: Input Drivers & Device Management ✅ COMPLETED

### SocketCanSource & CandumpFileSource
- [x] Implement `SocketCanSource` (live mode via SocketCAN interface).
- [x] Implement "Request all address claims" broadcast in live mode (Source 254 -> Dest 255).
- [x] Implement `CandumpFileSource` (file-based input for post-processing).
- [x] Warn user if candump file lacks timestamps. Exit on EOF.

### DeviceManager
- [x] Dynamic address claim handling from PGN 0xEC00.
- [x] Parameter cache to enrich data interpretation.
- [x] TTL expiration mechanism (devices removed after inactivity).
- [x] Address conflict detection (duplicate claims from different NAMEs for same address).
- [x] Replace String names with `j1939_async::name::Name` — stores u64 NAME values.
- [x] Parse 8-byte NAME payloads from PGN 0xEC00 Address Claim frames.
- [x] Query DeviceManager during decode for Source/Dest NAME hex values.
- [x] Fixed `blocking_lock()` panic by switching from `tokio::sync::Mutex` to `std::sync::Mutex`.

### DecodeContext Wiring (Phase 2a)
- [x] Add `pgn: u32` field to `DecodedMessage`.
- [x] All construction sites updated in pipeline.rs, pgn_decoder.rs, and test files.
- [x] PGN now available for structured access by filters and renderers.

### AssembledMessage Simplification (Phase 2a)
- [x] Make AssembledMessage non-optional — every decoded message always has an attached AssembledMessage.
- [x] Remove redundant pgn field from DecodedMessage — delegates to `assembled_message.pgn()`.
- [x] Update accessor methods: `source_address()`, `dest_address()`, `timestamp()`, `data_bytes()` no longer return Option.
- [x] Add test helpers: `make_dummy_assembled()` and `make_dummy_assembled_with_data()`.

### NAME Storage in AssembledMessage (Phase 2c)
- [x] Add `source_name: Option<u64>` and `dest_name: Option<u64>` fields to AssembledMessage.
- [x] Populate names in Decoder::decode() by querying DeviceManager before creating AssembledMessage.
- [x] Add accessor methods on DecodedMessage: `source_name()` and `dest_name()`.

## Phase 3: J1939 & Protocol Decoding ✅ COMPLETED

### TpReassembler (tp_reassembler.rs)
- [x] Implement BAM (Broadcast Announce Message) state machine — PGN 0xEC00, control byte 0x20.
- [x] Implement RTS/CTS (Request-to-Send / Clear-to-Send) state machine — control bytes 0x10, 0x11, 0x13.
- [x] Assembly state management with timeout handling.
- [x] Duplicate/out-of-order packet rejection.
- [x] `--force-output-partial-tp` support: emit partially assembled message on timeout.
- [x] Fix PDU type detection for TP frames — added `extract_tp_addresses` helper.
- [x] Fix assembled message CAN ID construction with `build_assembled_can_id()` (PDU1 vs PDU2).
- [x] Add explicit `pgn: u32` field to AssembledMessage to avoid incorrect extraction from CAN ID for small PGNs in PDU1 format.

### J1939Decoder (pgn_decoder.rs)
- [x] Implement J1939 CAN ID parsing — extracts PGN from bits 8-20 of extended CAN ID.
- [x] Handle Transport Protocol messages (PGN 0xF000-0xFDFF): Connection Management, Data Transfer, Abort, CM Next Ext CSN.
- [x] YAML configuration support for standard PGN definitions.
- [x] 40+ standard J1939 PGNs decoded using YAML component-based definitions:
    - PGN 0x0EC00: Address Claim (NAME payload extraction)
    - PGN 0x0FEF4: Engine Speed (RPM, scale 0.25)
    - PGN 0x0FEF8: Coolant Temperature (Int8, °C)
    - PGN 0x0EF00: Pressure (UInt16, kPa)
    - PGN 0x0CF00: Vehicle Speed (km/h)
    - PGN 0x0FECC: Engine Oil Pressure (kPa)
    - PGN 0x0FECE: Engine Oil Temperature (°C)
    - PGN 0x0FF00: GPS Location (Float32 lat/lon)
- [x] All decoders use safe byte extraction with bounds checking — never panics on malformed data.

### ComplexDecoder Registry (Phase 4)
- [x] Implement `HashMap<PGN, Box<dyn ComplexDecoder>>` in J1939Decoder.
- [x] Dispatch checks registry first, falls back to YAML config.
- [x] Added `register_complex_decoder(pgn, decoder)` and `complex_decoder_count()` methods.
- [x] MockComplexDecoder for testing dispatch routing (configurable PGN handling, output messages, `should_return_none` flag, atomic call counter).
- [x] 6 unit tests: `test_complex_decoder_dispatch_routes_to_mock`, `test_complex_decoder_falls_back_to_yaml_when_none`, `test_complex_decoder_ignores_unregistered_pgn`, `test_complex_decoder_multiple_registered_pgns`, `test_complex_decoder_registry_count`, `test_complex_decoder_overrides_yaml_config`.
- [x] Added `PartialEq` derive to `DecodedField` enum for test assertions.

### Testing (Phase 3)
- [x] 105 unit tests for TpReassembler covering BAM, RTS/CTS, edge cases, large PGNs, very large messages (up to 1785 bytes / 255 packets).
- [x] Integration tests: full pipeline CandumpFileSource -> TpReassembler -> PgnDecoder works with real traces (bam.log and rts.log).

## Phase 4: Filtering & Console UI ✅ COMPLETED

### CLI Structure
- [x] Full CLI structure with all flags from spec (--interface, --config, --detail-level, --force-output-partial-tp, --filter, --output-format, --source, --input-file).

### Filter Engine (filters.rs)
- [x] RegexFilter — matches string messages using regex patterns.
- [x] NumericFilter — matches numeric values by title and range (supports >=, <=, min-max).
- [x] FlagFilter — matches flag values by title and state.
- [x] SeverityFilter — matches severity levels (info, warning, error).
- [x] PgnFilter — matches by PGN value (currently uses title substring search — see architecture gaps).
- [x] TitleFilter — case-insensitive substring match on titles and text.
- [x] CompositeFilter — AND logic combining multiple filters.
- [x] FilterParser — parses CLI filter expressions into Box<dyn Filter>.

### Console Renderer
- [x] ConsoleRenderer with colorized columnar output using owo-colors.
- [x] Uses format_output() to produce plain text display strings.

## Phase 6: TaskController Process Data Decoder ✅ COMPLETED

### Packet Format Analysis (Phase 6a)
- [x] Confirmed from candump captures of PGN 51968 messages (source=0x90/240, dest=0xFF/255).
- [x] ISO 11783-10 Annex B.3 layout verified against real data:
    - Command: Byte 0, bits [3:0], u4 — always 0x3 in captures (Value command)
    - Element ID: Byte 0, bits [7:4] + Byte 1, u12 — range 0-4095, SPN 5200
    - DDI: Bytes 2-3, u16 LE — always 0xE000 (57344) in captures
    - Value: Bytes 4-7, s32 LE — signed 32-bit process variable

### TaskControllerDecoder Implementation (Phase 6b)
- [x] Created `task_controller.rs` module.
- [x] Defined `TaskControllerDecoder` struct implementing ComplexDecoder trait.
- [x] Parse payload bytes: Command (4 bits), Element ID (12-bit), DDI (u16 LE), Value (i32 LE).
- [x] Bounds checking for payload length < 8 returns DecodeError::InvalidLength.
- [x] Decode command=0x3 (Value): Returns DecodedMessage with Element ID, DDI, Value outputs.
- [x] Handle other commands (0x0-0x2, 0x4-0xF) as unrecognized StringMessage warnings.
- [x] Track last seen element IDs for sequence detection.

### TaskController Tests (Phase 6c)
- [x] Test payload parsing from all 4 verified candump frames (elements 10-13).
- [x] Test short payload handling — returns DecodeError::InvalidLength.
- [x] Test command dispatch — command=0x3 produces Value output; other commands produce warning StringMessage.
- [x] Test element ID extraction for all nibbles (0-15).
- [x] Test DDI various values (0xE000, 0xD3D8, 0x9240, 0x0000, 0xFFFF).
- [x] Test signed i32 LE value field (positive, negative, max, min).
- [x] Test ComplexDecoder trait integration.
- [x] Test decoder tracks elements across multiple decode calls.

## General Testing Achievements

- [x] All 194 tests passing (145 lib + 25 types + 17 device_manager + 7 integration).
- [x] BAM broadcast tests using exact bam.log packets — payload matches reference decoder output.
- [x] RTS/CTS unicast tests using exact rts.log packets — payload matches reference decoder output.
- [x] Large PGN tests (PGN >= 0xF000) for both BAM and RTS/CTS.
- [x] Very large message tests (1785 bytes / 255 DT packets, 1000 bytes).
- [x] Edge case tests: timeouts, aborts, duplicates, out-of-order, missing CM frames.
