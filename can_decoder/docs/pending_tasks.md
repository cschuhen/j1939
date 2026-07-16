# Pending Tasks

## Phase 5: Structured Output & Advanced UI ⏳ NOT STARTED

### JSON and CSV Formatters
- [ ] Implement JSON renderer — output DecodedMessage as structured JSON.
- [ ] Implement CSV renderer — output DecodedField rows with headers.
- [ ] Wire output-format flag to select between console, json, csv renderers.

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
