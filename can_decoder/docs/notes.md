# Additional Notes: CAN Frame Decoder

## Coding Conventions

- Keep modules in separate files organized by responsibility (sources, decoders, filters, renderers).
- Parts of the system that are extendable must be in separate files to facilitate modularity.
- Compile-time integration is the primary focus; plugins are first-class Rust code organized into modules rather than dynamic libraries.
- All decoders use safe byte extraction with bounds checking — never panic on malformed data.
- Use `std::sync::Mutex` (not `tokio::sync::Mutex`) for DeviceManager to avoid `blocking_lock()` panics in synchronous contexts.

## Reference Code

Existing Rust modules in `/home/cschuhen/rust/j1939` (the `j1939_async` library) are leveraged for J1939 protocol fundamentals:
- `Name` type with bitfield accessors (`manufacturer()`, `function()`, `ecu_instance()`, etc.)
- `Id` trait for CAN ID parsing (PDU1/PDU2 detection, address extraction)
- `process_data.rs` patterns for ISO-11783 Process Data decoding

## Development Workflow

1. Run `cargo build` after each change to verify compilation.
2. Add relevant unit tests alongside code changes.
3. Run `cargo test` once tests are added — currently 194 tests passing.
4. For candump testing: generate sample data with `candump can0 -t a > capture.log` or create manually.
5. Use subagents for discrete features to manage context size effectively.
6. After each phase completion: update documentation, then commit (don't push) with a concise commit message.

### TP Frame Address Extraction
TP.CM (PF=0xEC) and TP.DT (PF=0xEB) transport mechanism is **always PDU1** regardless of PF value. The `extract_tp_addresses` helper handles this special case: when PF is 0xEC or 0xEB, PS field is the destination address (not a group extension).

### Assembled Message CAN ID Construction
Small PGNs (< 0xF000) in PDU1 format require manual CAN ID construction preserving source/destination. The `build_assembled_can_id` function handles this:
- PDU2 (PGN >= 0xF000): full PGN in bits 8-25, destination = broadcast (0xFF).
- PDU1 (PGN < 0xF000): dest in PS field (bits 8-15), pgn_high in PF field (bits 16-25).

### AssembledMessage PGN Storage
`AssembledMessage::pgn()` returns the stored payload PGN directly rather than extracting from CAN ID, because `j1939_async::Id::pgn()` is incorrect for small PGNs (< 0xF000) in PDU1 format where destination address occupies bits 8-15.

### ComplexDecoder Registry Priority
Registered decoders are checked **before** YAML config during dispatch. This provides a clean override mechanism — compile-time Rust modules take precedence over YAML definitions for the same PGN. Returning `Ok(None)` from a ComplexDecoder falls back to YAML or unrecognized message handling.

## File Interaction Map

| From | To | What Flows |
|---|---|---|
| `main.rs` -> `sources.rs` | Creates SocketCanSource or CandumpFileSource, wraps in Arc, passes to pipeline.spawn_source() |
| `main.rs` -> `pipeline.rs` | Calls spawn_source(), spawn_decoder(), spawn_filter(), spawn_renderer() to wire stages |
| `sources.rs` -> `types.rs` | Produces RawFrame instances, sends via channel |
| `pipeline.rs` (decoder) -> `traits.rs::Decoder` | Receives RawFrame, returns DecodedMessage |
| `pipeline.rs` (filter) -> `traits.rs::Filter` | Checks matches(&DecodedMessage) to decide pass/drop |
| `pipeline.rs` (renderer) -> `traits.rs::Renderer` | Formats DecodedMessage into display string |
| `types.rs` -> all modules | All data types shared across the pipeline |

## Test Statistics

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

## Future Considerations

### Backpressure & Memory
No frames are dropped. CAN bus throughput is low enough that all messages can be retained in RAM for the lifetime of the session. This supports future TUI/GPUI modes where users may want to review historical data. For very long sessions, consider implementing a sliding window or LRU cache.

### Plugin System
The current architecture uses compile-time Rust modules for decoders and filters. A future dynamic plugin system (loading compiled .so files) could enable third-party PGN definitions without recompiling the core application.

### Configuration Extensibility
YAML config currently defines standard PGN interpretations. Future enhancements could support:
- Custom unit conversions and scaling factors per device.
- Device-specific PGN overrides.
- Alert thresholds for numeric values (e.g., warn when coolant temp > 105C).
