# Additional Notes: ISO 11783 Data Crate

## Coding Conventions

### Generated Code Style
- All generated files include header comment with source file name, revision number, and download date.
- Static slices sorted by lookup key for binary search efficiency.
- Lookup functions return `Option<&'static ...>` — callers handle missing entries gracefully.
- No heap allocation (`Vec`, `String`) in library crate — fully compatible with `no_std`.
- DDI struct derives `Debug, Clone, Copy, PartialEq` for convenience in consumer code.

### Generator Code Style
- Parsers return validated and deduplicated entry lists.
- Warnings printed to stderr for data quality issues (duplicates, out-of-range entries).
- Generation continues even if warnings are reported — non-fatal errors don't stop pipeline.
- Revision tracking compares generated content against existing files to avoid unnecessary updates.

### Naming Conventions
- Generated modules use snake_case: `pgn`, `isobus_params`, `task_controller_ddi`, `name`.
- Lookup functions named consistently: `lookup()`, `manufacturer_id_lookup()`, etc.
- Generator parsers follow pattern: `<module>_parser.rs` with `parse()` function.

## Reference Code

Existing Rust modules in `/home/cschuhen/rust/j1939` leverage `iso11783-data`:
- `can_decoder/src/task_controller.rs` — will use PGN and DDI lookups from this crate (Phase 3).
- `j1939-async/src/process_data.rs` — contains proprietary DDI constants that could be replaced with `iso11783-data::task_controller_ddi::lookup()`.

## Development Workflow

1. **Update source data**: Run generator to download and parse latest isobus.net files:
   ```bash
   cd /home/cschuhen/rust/j1939/iso11783-data
   cargo run -p generator -- generate
   ```

2. **Verify generation**: Check generated files in `src/` for correctness:
   ```bash
   cargo run -p generator -- info
   ls -lh src/*.rs
   ```

3. **Run tests**: Ensure all tests pass after regeneration:
   ```bash
   cargo test
   ```

4. **Verify no_std compilation**: Test compatibility without std library:
   ```bash
   cargo check --target thumbv7m-none-eabi
   ```

5. **Commit changes**: Generated `.rs` files are committed to git alongside generator source code.

### Generator Commands

```bash
# Download all sources and generate all modules (default)
cargo run -p generator -- generate

# Generate only specific modules
cargo run -p generator -- generate --module pgn
cargo run -p generator -- generate --generate --module task_controller_ddi

# Use custom file paths (skip download)
cargo run -p generator -- generate \
    --pgn-file /path/to/SPNs\ and\ PGNs.xlsx \
    --params-file /path/to/AEF\ Functionalities.xlsx \
    --ddi-file /path/to/TaskControllerDDI.txt

# Show source metadata without regenerating
cargo run -p generator -- info
```

### File Interaction Map

| From | To | What Flows |
|------|----|------------|
| `generator/main.rs` -> `generator/excel.rs` | Downloads files from isobus.net, extracts ZIP archives | Source files in `downloads/` and `downloads/extract/` |
| `generator/main.rs` -> `generator/parsers/*.rs` | Parses Excel/TXT source files into entry lists | Validated entry vectors |
| `generator/parsers/*.rs` -> `generator/codegen.rs` | Entry lists converted to Rust source code strings | Generated `.rs` file content |
| `generator/codegen.rs` -> `src/*.rs` | Written generated files alongside library crate | Committed to git |
| `src/lib.rs` -> `src/pgn.rs`, etc. | Feature-gated module exports for consumers | Library API surface |
| Consumer crates (e.g., `can_decoder`) -> `iso11783-data` | Uses lookup functions from library crate | PGN names, DDI metadata, NAME lookups |

## Test Statistics

```
Module                  Tests   Status
─────────────────────── ─────── ──────
pgn_tests.rs               8    PASS
isobus_params_tests.rs     7    PASS
task_controller_ddi_tests.rs 12   PASS
─────────────────────── ─────── ──────
Total                     27    ALL PASS
```

## Data Source URLs

| Source | URL | Notes |
|--------|-----|-------|
| ISOBUS Parameters (ZIP) | `https://www.isobus.net/isobus/attachments/isoExport_xlsx.zip` | Contains PGN data + 13 xlsx files for NAME lookups |
| Task Controller DDI | `https://www.isobus.net/isobus/exports/completeTXT` | TXT format, "DD Entity:" blocks |
| isobus.net exports page | `https://www.isobus.net/isobus/exports/completeWithRequests` | Reference only — not directly used by generator |
| isobus.net complete exports | `https://www.isobus.net/isobus/exports/complete` | Reference only — not directly used by generator |

## Revision Tracking

Revision numbers are tracked in `generator/revision.json`:

```json
{
  "pgn": {"source": "SPNs and PGNs.xlsx", "rev": N, "date": "YYYY-MM-DD"},
  "isobus_params": {"source": "AEF Functionalities.xlsx", "rev": N, "date": "YYYY-MM-DD"},
  "task_controller_ddi": {"source": "TaskControllerDDI.txt", "rev": N, "date": "YYYY-MM-DD"},
  "name": {"source": "NAME lookup tables", "rev": N, "date": "YYYY-MM-DD"}
}
```

- Revision increments only when content changes (not on every run).
- Each generated file header includes: `// Source: <filename> rev <N>, downloaded <YYYY-MM-DD>`
- Simple content comparison against existing files determines if revision should increment.

## no_std Compatibility Notes

All library modules are fully compatible with `no_std`:
- Static slices of tuples/structs — no heap allocation required.
- Binary search via `core::slice` — works without `alloc`.
- f64 arithmetic in `to_physical()` — available in `core`.
- No dependencies on `std` library features (collections, strings, error handling).

Verified with:
```bash
cargo check --target thumbv7m-none-eabi
```

## Future Considerations

### Performance Optimization
For very large lookup tables (PGN: 3,213 entries), binary search provides O(log n) access which is sufficient for CAN bus throughput. If O(1) access becomes necessary, consider hash-based lookups — but this would require `alloc` and break no_std compatibility.

### Plugin System
The current architecture uses compile-time generated code. A future dynamic plugin system (loading external data files at runtime) could enable third-party PGN/DDI definitions without recompiling the library. However, this would add complexity and is not aligned with the current no_std goals.

### Configuration Extensibility
Future enhancements could support:
- Custom unit conversions per device or application context.
- Device-specific DDI overrides (currently all DDIs use global lookup tables).
- Alert thresholds for physical values (e.g., warn when temperature > 105°C).
