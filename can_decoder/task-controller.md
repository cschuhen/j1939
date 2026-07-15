# Implementation Plan: Advanced PGN Decoding & ISO-11783

## 1. Architectural Design

The system will move from a single-stage decoding process to a three-stage pipeline to handle transport protocols (TP) and complex, identity-dependent PGNs.

### Pipeline Stages
1.  **Transport Layer (`TpReassembler`):** Consumes `RawFrame`s. Handles J1939 TP (Connection Management, Data Transfer). Outputs `AssembledMessage` (complete payload + context).
2.  **Network Layer (`PgnDecoder`):** Consumes `AssembledMessage`. Performs PGN lookup. Dispatches to either a standard decoder or a `ComplexDecoder`. Outputs `DecodedMessage`.
3.  **Application Layer (Manager/Renderer):** Consumes `DecodedMessage`. Renders output to the user.

### Core Data Model

#### Decode Context
```rust
pub struct DecodeContext {
    pub pgn: PGN,
    pub priority: u8,
    pub src_addr: u8,
    pub dest_addr: u8,
    pub src_name: Option<u64>,  // Optional: 64-bit J1939 NAME
    pub dest_name: Option<u64>, // Optional: 64-bit J1939 NAME
    pub timestamp: u64,
}
```

#### Decoder Output (Intent-Based)
```rust
pub struct DecodedMessage {
    pub title: String,
    pub outputs: Vec<Field>,
}
```

#### Complex Decoder Trait
```rust
pub trait ComplexDecoder: Send {
    /// Decodes a complete, reassembled message.
    /// `&mut self` allows the decoder to maintain internal state (e.g. for sequence tracking)
    /// Returns `Ok(None)` if the message is part of a sequence not yet complete.
    fn decode(
        &mut self, 
        context: &DecodeContext, 
        payload: &[u8]
    ) -> Result<Option<DecodedMessage>, DecodeError>;
}
```

#### Filter & Renderer Traits
```rust
pub trait Filter: Send + Sync {
    fn name(&self) -> &str;
    fn matches<'a>(
        &'a self,
        message: &'a DecodedMessage,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

pub trait Renderer: Send + Sync {
    fn render<'a>(
        &'a self,
        message: &'a DecodedMessage,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'a>>;
}
```

## 2. Implementation Roadmap

### Phase 1: Foundational Types & Traits ✅ COMPLETED
*Goal: Define the new data structures and traits without changing existing logic.*
- [x] **1.1 Define Core Types:** Update `types.rs` with `DecodedMessage`.
- [x] **1.2 Define `ComplexDecoder` Trait:** Add the trait to `traits.rs`.
- [x] **1.3 Refine Pipeline Traits:** Update `Filter` and `Renderer` in `traits.rs` to accept `&DecodedMessage`.
- [x] **1.4 Unit Test:** Create `tests/decoder_types_test.rs` to verify construction of new types.

*Assessment: All items accurately marked done. Core types (`RawFrame`, `AssembledMessage`, `DecodedField`, `Numeric`, `Severity`, `FlagValue`, `DecodeError`) are well-defined and in use throughout the pipeline.*

### Phase 2: Identity Enrichment ✅ COMPLETED — Feasible (Medium Effort)
*Goal: Integrate `j1939_async` for identity management and enable `DeviceManager` to provide 64-bit NAMEs to the context.*

**Assessment:** Highly feasible. `j1939_async` already provides all necessary components (`Name`, `NameManager`). Split into two independent sub-tasks:

#### Sub-task 2a: DecodeContext Wiring (Low Effort) ✅ COMPLETED
- [x] **2a.1 Add `pgn` field to `DecodedMessage`:** Added `pub pgn: u32` field (`types.rs:222`). Updated constructor `new(pgn, title)` at `types.rs:229`. All construction sites updated in `pipeline.rs`, `pgn_decoder.rs`, and test files. PGN now available for structured access by filters and renderers without string parsing.
- [x] **2a.2 Unit Tests:** Added 5 new tests (`test_decoded_message_new_constructor`, `test_decoded_message_pgn_preserved_on_clone`, `test_decoded_message_pgn_various_values`, `test_decoded_message_pgn_with_outputs`). All 125 tests pass.

### Phase 2a: DecodedMessage Simplification ✅ COMPLETED
*Goal: Remove Option from assembled_message and redundant pgn field.*
- [x] **2a.1 Make AssembledMessage non-optional:** Every decoded message always has an attached AssembledMessage (from RawFrame or TP-assembled).
- [x] **2a.2 Remove redundant pgn field:** DecodedMessage.pgn() now delegates to assembled_message.pgn.
- [x] **2a.3 Update accessor methods:** source_address(), dest_address(), timestamp(), data_bytes() no longer return Option.
- [x] **2a.4 Add test helpers:** make_dummy_assembled() and make_dummy_assembled_with_data() for unit tests.
- [x] **2a.5 Update all construction sites:** pgn_decoder.rs, pipeline.rs, integration_test.rs, decoder_types_test.rs.
- [x] **2a.6 Regression Test:** All 154 tests pass (114 lib + 16 types + 17 device_manager + 7 integration).

#### Sub-task 2b: DeviceManager NAME Support (Medium Effort) ✅ COMPLETED
- [x] **2b.1 Replace `String` names with `j1939_async::name::Name`:** Added `parse_name_from_bytes()` and `set_name_u64()` methods to `DeviceManager`. Stores u64 NAME values from PGN 0xEC00 Address Claim payloads.
- [x] **2b.2 Parse Address Claims:** Extract 8-byte NAME payloads from PGN 0xEC00 frames in `J1939Decoder::decode_raw_frame()` and store via DeviceManager's address→NAME mapping.
- [x] **2b.3 Query DeviceManager during decode:** Added `decode_assembled_with_context()` method that enriches output with Source/Dest Device NAME hex values by looking up addresses in DeviceManager. Wired into main.rs pipeline.
- [x] **2b.4 Verification:** All 145 tests pass (114 lib + 7 types + 17 device_manager + 7 integration). Added 20 new unit tests for NAME parsing, storage, retrieval, expiration cleanup, and decoder integration. Fixed `blocking_lock()` panic by switching from `tokio::sync::Mutex` to `std::sync::Mutex`.

*Note: Sub-task 2a can be done independently of 2b. The `j1939_async::name::Name` type provides bitfield accessors (`manufacturer()`, `function()`, `ecu_instance()`, etc.) for rich device identification.*

### Phase 2c: Source/Destination NAME Storage in AssembledMessage ✅ COMPLETED
*Goal: Store source and destination J1939 NAMEs directly in AssembledMessage, populated from DeviceManager before decoding.*
- [x] **2c.1 Add name fields to AssembledMessage:** Added `source_name: Option<u64>` and `dest_name: Option<u64>` fields (`types.rs:59-63`).
- [x] **2c.2 Update all construction sites:** tp_reassembler.rs, pgn_decoder.rs (decode_single_frame), pipeline.rs (NullDecoder) — all set None by default.
- [x] **2c.3 Populate names in Decoder::decode():** Query DeviceManager for source/dest NAMEs using addresses from RawFrame CAN ID before creating AssembledMessage (`pgn_decoder.rs:652-684`).
- [x] **2c.4 Add accessor methods on DecodedMessage:** `source_name()` and `dest_name()` delegating to assembled_message fields (`types.rs:279-287`).
- [x] **2c.5 Update decode_assembled_with_context():** Use names stored in AssembledMessage instead of looking up from DeviceManager again.
- [x] **2c.6 Regression Test:** All 163 tests pass (114 lib + 25 types + 17 device_manager + 7 integration) — added 9 new unit tests for source_name/dest_name functionality.

### Phase 3: The Transport Layer (High Risk) ✅ COMPLETED
*Goal: Implement TP reassembly as a standalone stage.*
- [x] **3.1 Implement `TpReassembler`:** Created in `tp_reassembler.rs` with BAM and RTS/CTS state machines, assembly state management, timeout handling, duplicate/out-of-order rejection, and 105 unit tests covering all scenarios.
- [x] **3.2 Unit Test (TP):** Comprehensive test suite including:
    - Successful multi-packet reassembly (`test_complete_multi_packet_transfer`, `test_bam_from_real_trace`)
    - Out-of-order packet handling (`test_out_of_order_packet_rejected`)
    - Timeouts/Aborts (`test_timeout_discards_assembly`, `test_rts_cts_with_abort`)
    - Very large messages up to 1785 bytes / 255 packets (`test_very_large_message_1785_bytes`)
- [x] **3.3 Integration Test:** Verified via `tests/integration_test.rs` — full pipeline `CandumpFileSource` $\rightarrow$ `TpReassembler` $\rightarrow$ `PgnDecoder` works with bam.log and rts.log traces (PGN 0xFF80, 0xE600).

*Cleanup items:*
- `extract_tp_addresses` function at `tp_reassembler.rs:116-132` is dead code (unused) — remove or add `#[allow(dead_code)]`.
- `handle_abort` at line 402-407 has a debug print saying "not yet implemented" but doesn't actually clean up assembly state.

### Phase 4: Complex Decoder Dispatch ✅ COMPLETED — Feasible (Low Effort)
*Goal: Enable the `PgnDecoder` to use the new `ComplexDecoder` registry.*

**Assessment:** Low effort since the trait is already defined (`traits.rs:42-51`). The complex decoder registry checks first, then falls back to YAML config. This provides a clean override mechanism for PGNs that need custom decoding logic.

- [x] **4.1 Implement `ComplexDecoder` Registry:** Added `HashMap<PGN, Box<dyn ComplexDecoder>>` to `J1939Decoder`. Dispatch in `decode_assembled()` checks registry first (with `DecodeContext` construction), falls back to YAML config. Added `register_complex_decoder(pgn, decoder)` and `complex_decoder_count()` methods. Changed `decode_assembled()` and `decode_assembled_with_context()` to take `&mut self` for ComplexDecoder mutability.
- [x] **4.2 Mock Decoder:** Implemented `MockComplexDecoder` with configurable PGN handling, output messages, `should_return_none` flag, and atomic call counter for testing dispatch routing.
- [x] **4.3 Unit Test (Dispatch):** Added 6 new unit tests: `test_complex_decoder_dispatch_routes_to_mock`, `test_complex_decoder_falls_back_to_yaml_when_none`, `test_complex_decoder_ignores_unregistered_pgn`, `test_complex_decoder_multiple_registered_pgns`, `test_complex_decoder_registry_count`, `test_complex_decoder_overrides_yaml_config`. All 169 tests pass (120 lib + 25 types + 17 device_manager + 7 integration).
- [x] **Bonus:** Added `PartialEq` derive to `DecodedField` enum for test assertions.

*Note: The complex decoder registry provides a clean override mechanism — registered PGNs bypass YAML config entirely, and returning `Ok(None)` from a ComplexDecoder falls back to YAML or unrecognized message.*

### Phase 5: Real-World Implementation ⏳ NOT STARTED — High Effort, Scope Uncertain
*Goal: Implement actual logic for ISO-11783 and complex PGNs.*

**Assessment:** High effort, high risk. Before starting, review `j1939_async/src/process_data.rs` and docs in `../docs/` (especially `j1939_control_protocols.md`). Define specific PGNs to implement first rather than a vague "complex PGN decoders" goal.

- [ ] **5.1 Implement Specific Complex PGN Decoders:** Start with 2-3 high-priority PGNs that have sub-types or variable formats (e.g., PGN 0xEA00 ECU Status, PGN 0xFE8D Active Faults).
- [ ] **5.2 Implement ISO-11783 Process Data Logic:** Reference `j1939_async/src/process_data.rs` for existing patterns. Requires understanding of SAE J1939-81 Task Controller state machines.
- [ ] **5.3 Final Integration Test:** Full end-to-end test: `Candump File` $\rightarrow$ `TpReassembler` $\rightarrow$ `PgnDecoder` $\rightarrow$ `DeviceManager Update`.

### Phase 6: TaskController Process Data Decoder (PGN 51968 / 0xCB00) ⏳ NOT STARTED — High Effort

*Goal: Implement a `TaskControllerDecoder` ComplexDecoder that parses ISO-11783-10 Annex B.3 Process Data messages for command=0x3 (Value command), extracting Element ID, DDI, and Value fields from 8-byte payloads.*

**Assessment:** Medium-high effort. The packet format is confirmed from candump analysis. Requires implementing a ComplexDecoder with proper byte parsing, unit tests against real capture data, and integration with the existing decoder registry. No Transport Protocol reassembly needed — all TaskController Process Data messages are single-frame (8 bytes).

#### 6a. Packet Format Analysis ✅ COMPLETED
*Confirmed from candump captures of PGN 51968 messages (source=0x90/240, dest=0xFF/255).*

**ISO 11783-10 Annex B.3 Layout (verified against real data):**

| Field | Bytes | Type | Description |
|---|---|---|---|
| Command | Byte 0, bits [3:0] | u4 | Always `0x3` in captures (Value command per Table B.1) |
| Element ID | Byte 0, bits [7:4] + Byte 1 | u12 | Low nibble of element in byte[0], high byte in byte[1]. Range 0–4095. SPN 5200. |
| DDI | Bytes 2–3 | u16 LE | Data Dictionary Identifier. Always `0xE000` (57344) in captures — represents the controlled process variable. |
| Value | Bytes 4–7 | s32 LE | Signed 32-bit little-endian process variable value. |

**Verified against candump frames:**

```
Payload: a3 00 00 e0 b2 b2 88 9b → Element=10, Command=3, DDI=57344, Value=-1685540174 ✓
Payload: b3 00 00 e0 b3 b8 7e 9b → Element=11, Command=3, DDI=57344, Value=-1686193997 ✓
Payload: c3 00 00 e0 10 12 7e 9b → Element=12, Command=3, DDI=57344, Value=-1686236656 ✓
Payload: d3 00 00 e0 5f 99 76 9b → Element=13, Command=3, DDI=57344, Value=-1686726305 ✓
```

**Transmission pattern observed:** Round-robin across 4 elements (A/B/C/D = 10/11/12/13), each sending 3 CAN frames per cycle:
- Frame `xx 00`: Element value (command=3)
- Frame `xx 01`: Unknown sub-command (DDI varies, e.g., 0xD3D8)
- Frame `xx 02`: Unknown sub-command (DDI=0x9240 in some frames)

Cycle repeats every ~750ms–1s. Source address always 0x90 (240), destination always 0xFF (global).

#### 6b. Implement `TaskControllerDecoder` ComplexDecoder — NOT STARTED
- [ ] **6b.1 Create `task_controller.rs`:** New module in `src/`. Define `TaskControllerDecoder` struct implementing `ComplexDecoder` trait. Register for PGN 51968 (0xCB00).
- [ ] **6b.2 Parse payload bytes:** Extract Command (4 bits), Element ID (12-bit: `(byte[0] >> 4) << 8 | byte[1]`), DDI (`byte[2] | (byte[3] << 8)`), Value (`i32::from_le_bytes(bytes[4..8])`). Add bounds checking for payload length < 8.
- [ ] **6b.3 Decode command=0x3 (Value):** Return `DecodedMessage` with title `"TaskController Element {elem} DDI {ddi}"`, outputs: `Element(u12)`, `DDI(u16)`, `Value(s32)`. Handle other commands (0x0–0x2, 0x4–0x9, 0xa, 0xd–0xf) as unrecognized StringMessage warnings.
- [ ] **6b.4 Register in J1939Decoder:** Add `register_task_controller_decoder()` method or inline registration in `J1939Decoder::new()`. Use PGN constant from `j1939_async` (`PROCESS_DATA = 0x00cb00`).

#### 6c. Unit Tests — NOT STARTED
- [ ] **6c.1 Test payload parsing:** Verify Element/Command/DDI/Value extraction from all 4 verified candump frames.
- [ ] **6c.2 Test short payload handling:** Payload < 8 bytes returns `DecodeError::InvalidLength`.
- [ ] **6c.3 Test command dispatch:** Command=0x3 produces Value output; other commands produce warning StringMessage.
- [ ] **6c.4 Test round-robin detection (optional):** Track element sequence across multiple decode calls, detect missing elements in cycle.

#### 6d. Integration — NOT STARTED
- [ ] **6d.1 Wire into main.rs pipeline:** Ensure TaskController messages flow through `TpReassembler` → `PgnDecoder` (ComplexDecoder registry) → Filter → Renderer.
- [ ] **6d.2 Test with candump file:** Run against `example_candump.longer.decoded` or raw candump to verify end-to-end decoding of TaskController Process Data messages.

## 3. Architecture Gaps & Improvements

### 3.1 Unused Types
- **`DecodeContext`** (`types.rs:199-208`) is defined but never constructed or passed through the pipeline. The `ComplexDecoder::decode()` method takes a `&DecodeContext` parameter that no one calls.
- **`DeviceUpdate`** (`types.rs:213-217`) and `DecodedMessage.updates` field exist but are never populated (always `vec![]`).

### 3.2 Fragile Filter Implementation (Partially Resolved)
- **`PgnFilter`** (`filters.rs:139-148`) still matches on title strings containing "PGN" via substring search rather than using the structured `pgn: u32` field added in Phase 2a. The field is now available for a future refactor to use `output.pgn` directly.

### 3.3 Dead Code / Stale Comments
- `extract_tp_addresses` in `tp_reassembler.rs:116-132` — unused function, adds compiler warning.
- `pgn_decoder.rs:483-486` — commented-out single-frame logic with incorrect PGN threshold (`if pgn < 0xFE00`). Should be removed or cleaned up.

## 4. Suggested Work Order

1. **Phase 2a (DecodeContext wiring)** ✅ COMPLETED — Low effort, high value. Wires up existing types.
2. **Phase 2c (NAME storage in AssembledMessage)** ✅ COMPLETED — NAMEs now stored directly in assembled messages with DeviceManager lookup.
3. **Phase 4 (ComplexDecoder registry)** ✅ COMPLETED — Registry active, mock decoder tests pass.
4. **Cleanup** — Remove dead code (`extract_tp_addresses`, stale comments) to reduce compiler warnings.
5. **Phase 6 (TaskController PGN 51968 decoder)** — First concrete ComplexDecoder implementation. Packet format fully analyzed from real candump data. Start here before Phase 5 general work.
6. **Phase 5** — Expand with additional complex PGNs (e.g., PGN 0xEA00 ECU Status, PGN 0xFE8D Active Faults) after TaskController decoder validates the pattern.
