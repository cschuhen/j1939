# Pending Tasks

## Phase 3: Integration with `can_decoder` ⏳ NOT STARTED — Medium Priority

**Goal**: Replace hardcoded DDI/PGN values in `can_decoder` with lookups from `iso11783-data`.

### Steps

- [ ] Add `iso11783-data` as a dependency to `can_decoder/Cargo.toml`:
  ```toml
  [dependencies]
  iso11783-data = { version = "0.1", path = "../iso11783-data" }
  ```

- [ ] Replace hardcoded PGN constants in `can_decoder/src/task_controller.rs` with `iso11783_data::pgn::lookup()`:
  - Current: `const PROCESS_DATA_PGN: u32 = 0x00cb00;` (51968)
  - Replace with: `iso11783_data::pgn::lookup(51968)` for display purposes

- [ ] Enhance Task Controller decoder output to use DDI metadata from `iso11783_data::task_controller_ddi::lookup()`:
  - Current: Raw DDI values displayed (e.g., `0xE002`)
  - Replace with: Human-readable names and units from `DdiInfo` struct

- [ ] Update display/rendering to show human-readable names instead of raw numeric values in console/JSON output.

**Deliverable**: `can_decoder` displays meaningful names and units for Task Controller messages.

## Phase 4: Polish & Publication (Future) — Low Priority

### README.md
- [ ] Add `README.md` with usage examples for the library crate.
- [ ] Include examples of PGN lookup, DDI metadata access, and physical value conversion.
- [ ] Document feature gating and no_std compatibility.

### crates.io Publication Preparation
- [ ] Prepare for crates.io publication (license, documentation, MSRV).
- [ ] Add `LICENSE-MIT` and `LICENSE-APACHE` files (currently MIT OR Apache-2.0 in Cargo.toml).
- [ ] Specify MSRV (Minimum Supported Rust Version) — likely 1.56+ for no_std compatibility.
- [ ] Add crate-level documentation comments to `lib.rs`.

### GitHub Actions Workflow (Future Scope)
- [ ] CI automation for periodic regeneration from isobus.net.
- [ ] Automated PR creation when data changes detected.
- [ ] Run tests on regenerated files before merging.

## Open Questions / Future Enhancements

1. **Data quality validation**: Current validation checks for duplicates and range errors but doesn't validate semantic correctness (e.g., missing names, empty units). Additional validation rules based on ISO 11783 specification could be added.

2. **Consumer integration testing**: End-to-end tests verifying `can_decoder` correctly uses `iso11783-data` lookups in real-world candump scenarios.

3. **Additional data modules**: Consider adding lookup tables for:
   - J1939 NAME bitfield constants (manufacturer IDs, function codes)
   - ISO 11783 error code definitions
   - ISOBUS AEF (Agri-Electronic Framework) definitions

4. **Performance optimization**: For very large lookup tables (PGN: 3,213 entries), consider hash-based lookups if O(1) access is needed. Current binary search approach is O(log n) which is sufficient for CAN bus throughput.

5. **Version pinning**: Consider adding version numbers to generated files to track which revision of isobus.net data was used. Currently only date and revision number are tracked in headers.
