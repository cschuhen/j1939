# Pending Tasks

## Phase 5: Structured Output & Advanced UI ⏳ NOT STARTED

### Fixes and general
- [x] If detail-level is less than 6, Don't pass on the individual packets of a TP session to the pgn decider, just let the assembler handle it. (Implemented via DetailLevel enum: Raw/Assembled/Both)
- [x] Once an assembled TP(multi-frame) message is available, pass it to the pgn decider. This is independent of the detail-level setting. This means that when detail>5, there may be two outputs for one CAN frame. The data portion of the AssembledMessage should be full TP payload data bytes i.e. bytes 1-7 of each DT message. (Implemented via DetailLevel enum + decode_assembled always called)
- [ ] Assembled-message includes an explicit PGN field as well as the CAN id. There is a note that the CAN id is always the raw ID from the frame... However, that is not really relevant. Instead, we shoudd drop the PGN field an create a synthetic ID for the TP AssembledMessage. Source/Destination address, should be that of the RTS/BAM frame. Priorrity should be the lowest priority received for any of the CM or DT frames coming from the sender. PGN should be the PGN encoded into the data bytes of the RTS/BAM frame.
- [ ]  Move the text-based(console/CSV/JSON) renderers into a separate file.

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
- [ ] All unit and integration tests must verify that malformed CAN frames, invalid PGN payloads, and corrupted TP sequences produce error output rather than panics.
- [ ] Panics reserved exclusively for unrecoverable program bugs (missing enum arms, assertion failures in invariant code).

### Debug Logging
- [ ] Implement separate debug log (stderr or file) independent of pretty-print filtering.
- [ ] Log protocol issues, timeout events, device expiration, and address conflicts.

## Code Cleanup ⏳ NOT STARTED

### Dead Code Removal
- [ ] Remove `extract_tp_addresses` in `tp_reassembler.rs:116-132` — unused function causing compiler warning (reassembler uses inline address extraction instead).
- [ ] Clean up commented-out single-frame logic in `pgn_decoder.rs:483-486` with incorrect PGN threshold.

### Stale Comments & Debug Prints
- [ ] Fix `handle_abort` at tp_reassembler.rs:402-407 — has debug print saying "not yet implemented" but doesn't actually clean up assembly state. Either implement abort handling or remove the stub.
- [ ] Remove any remaining debug print statements from reassembler.

### Variable Naming & Dead Code
- [ ] Prefix unused test variables with `_` (`num_packets`, `other`, `pgn` compiler warnings in tp_reassembler tests).
- [ ] Remove or add `#[allow(dead_code)]` to `extract_tp_addresses` in `tp_reassembler.rs:116-132` — added but reassembler uses inline address extraction instead.

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
