# Pending Tasks

## Phase 5: Structured Output & Advanced UI ⏳ NOT STARTED

### Fixes and general
- [x] If detail-level is less than 6, Don't pass on the individual packets of a TP session to the pgn decider, just let the assembler handle it. (Implemented via DetailLevel enum: Raw/Assembled/Both)
- [x] Once an assembled TP(multi-frame) message is available, pass it to the pgn decider. This is independent of the detail-level setting. This means that when detail>5, there may be two outputs for one CAN frame. The data portion of the AssembledMessage should be full TP payload data bytes i.e. bytes 1-7 of each DT message. (Implemented via DetailLevel enum + decode_assembled always called)
- [ ] Assembled-message includes an explicit PGN field as well as the CAN id. There is a note that the CAN id is always the raw ID from the frame... However, that is not really relevant. Instead, we shoudd drop the PGN field an create a synthetic ID for the TP AssembledMessage. Source/Destination address, should be that of the RTS/BAM frame. Priorrity should be the lowest priority received for any of the CM or DT frames coming from the sender. PGN should be the PGN encoded into the data bytes of the RTS/BAM frame.
- [ ]  Move the text-based(console/CSV/JSON) renderers into a separate file.

### DecodeContext Wiring (Architecture Gap)
- [x] `DecodeContext` is now constructed in PgnDecoder::decode_raw_frame_with_context and passed to ComplexDecoder::decode() via decode_assembled_with_context.

### DeviceUpdate Wiring (Architecture Gap)
- [x] `DeviceUpdate` values are now populated from decoder output for DeviceManager state changes. The `decode_raw_frame_with_context` method returns `(Vec<DecodedField>, Vec<DeviceUpdate>)`, and the Decoder trait implementation collects updates into DecodedMessage.updates.

### PgnFilter Refactor (Architecture Gap)
- [x] `PgnFilter` now uses `message.assembled_message.pgn()` directly for accurate PGN matching instead of substring search on title strings.

### CSV Formatter
- [ ] Implement CSV renderer — output DecodedField rows with headers.
- [ ] Each row should have certian fixed columns for every message, these will be the first columns: 
  - Timestamp
  - CAN ID
  - Priority
  - PGN
  - Source address
  - Destination address
  - Source NAME
  - Destination NAME
  - Data bytes
  - Concatination of any StringMessages in the output list
- [ ] The renderer should track(remember) the remaining columns for every (PGN, title) combination. 
    - [ ] Every Title in list of DecodedField(except StringMessages) seen should get it's own column.
    - [ ] That column should not be reused for the same (PGN, title) combination.
    - [ ] The column header for Value DecodedFields is that of the 'title'
    - [ ] Whenever new columns are added to a PGN/title combination, the column header should be re-printed with the updated headers.


### Condensed Formatter
- [ ] Implement Condensed renderer — output single-line per message
- [ ] Each line should have certian fixed columns for every message, these will be the first columns, these fields will not have titles: 
  - Timestamp
  - CAN ID in hex (no 0x prefix)
  - Priority
  - PGN
  - PGN in hex
  - Source address in hex -> Destination address
  - (Source NAME -> Destination NAME)
  - Data bytes in hex, no 0x prefix
  - Concatination of any StringMessages in the output list
- [ ] Every Title in list of DecodedField seen should get it's own field. These fields should get headers in bold. 
- [ ] Flag fields should be coloured red for Error, white for off, Green for on.


### TUI Integration (ratatui)
- [ ] Develop terminal UI using ratatui/crossterm.
- [ ] Live scrollable list of all messages (full history retained in RAM).
- [ ] Dynamic runtime filter editing without restart.
- [ ] Device manager panel showing active devices and NAMEs.

### GPUI Integration
- [ ] Develop graphical UI integration (future scope, high effort).

### End-to-End Testing
- [ ] Filter logic tests with complex expressions.
- [ ] End-to-end console rendering tests.
- [ ] Full pipeline integration test: Candump File -> TpReassembler -> PgnDecoder -> DeviceManager Update -> Filter -> Renderer.

## Error Reporting & Logging ⏳ NOT STARTED

### Crash Prevention Verification
- [x] All unit and integration tests verify that malformed CAN frames, invalid PGN payloads, and corrupted TP sequences produce error output rather than panics. Tests include: test_decode_empty_data, test_decode_short_data_returns_warning, test_dt_without_prior_cm, test_data_packet_too_short, test_bam_too_short_data, test_rts_cts_single_packet_transfer, and many more.
- [x] Panics reserved exclusively for unrecoverable program bugs (no panics in decode/parse paths).

### Debug Logging
- [ ] Implement separate debug log (stderr or file) independent of pretty-print filtering.
- [ ] Log protocol issues, timeout events, device expiration, and address conflicts.

## Code Cleanup ⏳ COMPLETED ✅

### Dead Code Removal
- [x] `extract_tp_addresses` in `tp_reassembler.rs:116-132` — kept with `#[allow(dead_code)]` attribute because it is used by unit tests for TP frame address extraction verification.
- [x] Commented-out single-frame logic in `pgn_decoder.rs` was cleaned up in a previous commit (no stale comments remain).

### Stale Comments & Debug Prints
- [x] `handle_abort` at tp_reassembler.rs:402-417 — fully implemented. Clears assembly state, handles broadcast fallback, and cleans up rts_pending_sources. Debug print is appropriate (wrapped in `if self.debug`).
- [x] All debug print statements in reassembler are properly wrapped in `if self.debug` blocks. No leftover "not yet implemented" stubs.

### Variable Naming & Dead Code
- [x] No unused test variable warnings (`num_packets`, `other`, `pgn` — all cleaned up or used).
- [x] `#[allow(dead_code)]` added to `extract_tp_addresses` and MockComplexDecoder struct/impl in pgn_decoder.rs.

## TP Reassembly Test Scenarios ⏳ NOT STARTED

### Mixed Protocol Testing
- [ ] **Mixed PDU1/PDU2 payloads**: Test a scenario where BAM announces one PGN but DT packets carry different data.
- [ ] **Address clash detection**: Verify that two simultaneous BAM sessions from same source to same dest are handled correctly.

## Architecture Gap Fixes ⏳ NOT STARTED

### DecodeContext Wiring
- [ ] `DecodeContext` (`types.rs`) is defined but never constructed or passed through the pipeline. The `ComplexDecoder::decode()` method takes a `&DecodeContext` parameter that no one calls.
- [ ] Wire DecodeContext construction in PgnDecoder when dispatching to ComplexDecoder.

### DeviceUpdate Wiring
- [ ] `DeviceUpdate` (`types.rs`) and `DecodedMessage.updates` field exist but are never populated (always `vec![]`).
- [ ] Populate updates from decoder output for DeviceManager state changes.

### PgnFilter Refactor
- [ ] `PgnFilter` (`filters.rs:139-148`) matches on title strings containing "PGN" via substring search rather than using the structured `pgn: u32` field from AssembledMessage.
- [ ] Refactor to use `message.assembled_message.pgn()` directly for accurate PGN matching.

## Phase 5: Real-World Complex PGN Decoders ⏳ NOT STARTED — High Effort, Scope Uncertain

Before starting, review `j1939_async/src/process_data.rs` and docs in `../docs/` (especially `j1939_control_protocols.md`). Define specific PGNs to implement first.

- [ ] Implement 2-3 high-priority complex PGN decoders:
    - PGN 0xEA00 — ECU Status
    - PGN 0xFE8D — Active Faults
- [ ] Implement ISO-11783 Process Data Logic referencing `j1939_async/src/process_data.rs`.
- [x] TaskController (PGN 51968) already implemented as the first concrete ComplexDecoder.

## Phase 6: Additional TaskController Features ⏳ OPTIONAL

- [ ] Round-robin detection — track element sequence across multiple decode calls, detect missing elements in cycle.
- [ ] Integration test with candump file — verify end-to-end decoding of TaskController Process Data messages through full pipeline.
