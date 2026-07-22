# Completed Tasks

## Phase 0: Investigation — Excel File Analysis ✅ **COMPLETE**

**Goal**: Confirm the proposed Rust library design is feasible by loading and analyzing the actual Excel files.

**Completed steps**:
1. ✅ Create `generator` crate skeleton with `calamine`, `reqwest`, `zip` dependencies.
2. ✅ Implement minimal code to download all 3 sources from isobus.net URLs.
3. ✅ Open each file, list available sheets, and dump sheet names + row/column counts. Full dumps saved to `downloads/`.
4. ✅ Identify the relevant sheet(s) — discovered that PGN data lives in `"SPNs and PGNs.xlsx"` inside extracted zip (not `ISOBUSStandardParts.xlsx` as originally planned). ISOBUS Parameters target file within 13 xlsx files needs identification.
5. ✅ Extract sample rows from each target source and print them to confirm column structure.
6. ✅ SAE vs ASAM approval distinction — **resolved**: no explicit approval type column exists in any source file. All 13 xlsx files were searched; "SAE" only appears as documentation URLs, zero "ASAM" mentions. Generated code will omit such comments.

**Deliverable**: A summary of findings confirming which sheets/columns to parse, and any data quality issues encountered. See **Current Status** section in `requirements_and_plan.md`.

## Phase 1: Generator Tool — Core Parsing & Codegen ✅ **COMPLETE**

**Goal**: Build the generator that produces all three generated modules (now extended to 4 with NAME module).

**Completed steps**:
1. ✅ PGN parser integrated into main binary (`parsers/pgn_parser.rs`) — extracts, deduplicates, and sorts 3,213 unique PGNs from `"SPNs and PGNs.xlsx"`.
2. ✅ Task Controller DDI parser wired into main binary (`parsers/task_controller_ddi_parser.rs`) — parses DD Entity blocks from TXT format (DDI number, name, unit, resolution). Offset always 0 since source has no Offset field.
3. ✅ ISOBUS params parser implemented (`parsers/isobus_params_parser.rs`) — uses `AEF Functionalities.xlsx` as default source (value→meaning mapping, 30 entries). Configurable via `--params-file` flag.
4. ✅ NAME parsers implemented (`parsers/name_parsers.rs`, `parsers/name_parsers_ig.rs`) — parses manufacturer IDs (1,653), industry groups (9), global functions (97), IG-specific functions (287), and vehicle systems (~15) from extracted Excel files.
5. ✅ Validation logic — deduplication + 24-bit range check for PGNs; deduplication for ISOBUS params, DDI entries, and NAME lookups. Warnings printed to stderr.
6. ✅ Rust source code generation (`codegen.rs`) — produces all four `.rs` files with static slices, binary search lookup functions, named constants for well-known PGNs, and bit-packing helpers for multi-argument NAME lookups.
7. ✅ CLI `generate` command with module selection (`--module pgn|isobus_params|task_controller_ddi|name`) and custom file paths (`--pgn-file`, `--params-file`, `--ddi-file`). Also supports `info` subcommand.
8. ✅ Revision tracking (`generator/revision.json`) — tracks source file, revision number, and download date per module. Revision increments only when content changes.

**Deliverable**: Running generator that produces all four `.rs` files in the repo root `src/` directory. Verified:
- `pgn.rs` (160KB, 3213 entries)
- `isobus_params.rs` (1.7KB, 30 entries)
- `task_controller_ddi.rs` (45KB, 383 entries)
- `name.rs` (~XX KB, ~2052 entries)

## Phase 2: Library Crate — Assembly & Testing ✅ **COMPLETE**

**Goal**: Assemble the library crate with generated code and verify it compiles as both `std` and `no_std`.

**Completed steps**:
1. ✅ Library crate skeleton exists (`Cargo.toml` with features).
2. ✅ Generated `.rs` files produced by Phase 1 generator — `pgn.rs`, `isobus_params.rs`, `task_controller_ddi.rs`, and `name.rs` all present in `src/`.
3. ✅ `lib.rs` updated with feature-gated module declarations (`#![no_std]`). Includes `name` module with `#[cfg(feature = "name")]`.
4. ✅ Compilation verified in `std` mode (default) — `cargo check` passes.
5. ✅ Compilation verified in `no_std` mode (`--target thumbv7m-none-eabi`) — passes.
6. ✅ Feature gating verified — compiles with individual features (`pgn`, `isobus_params`, `task_controller_ddi`, `name`) in both std and no_std modes.
7. ✅ Integration tests written and passing — 27 tests total:
   - `tests/pgn_tests.rs` (8 tests): lookup, sorted order, list count, binary search efficiency
   - `tests/isobus_params_tests.rs` (7 tests): lookup, sorted order, list count
   - `tests/task_controller_ddi_tests.rs` (12 tests): lookup, to_physical, sorted order, list count, name validation

**Deliverable**: Library crate compiles, passes 27 tests, and is ready for use in `can_decoder`.

## General Testing Achievements

- ✅ All 27 tests passing (8 pgn + 7 isobus_params + 12 task_controller_ddi).
- ✅ no_std compilation verified with `--target thumbv7m-none-eabi` — passes.
- ✅ Feature gating verified — each module compiles independently in both std and no_std modes.

## Key Findings from Investigation

1. **PGN source**: The planned `ISOBUSStandardParts.xlsx` is only a version catalog (19 rows). The actual PGN data lives in `"SPNs and PGNs.xlsx"` inside the zip extracted from `isoExport_xlsx.zip` (the ISOBUS Parameters download). This file has 2 columns: PGN value (column A) and name (column B), with ~17,950 total rows producing 3,213 unique entries after deduplication.

2. **ISOBUS Parameters**: The zip contains 13 xlsx files. `AEF Functionalities.xlsx` was identified as the target file for parameter NAME values (simple value→meaning mapping, 30 entries).

3. **Task Controller DDI source**: Downloaded as TXT format (`TaskControllerDDI.txt`, version 2026050501, May 2026). Uses "DD Entity:" blocks with Definition, Unit, and Resolution fields. No Offset field in the source data (all offsets default to 0).

4. **NAME lookups**: Extracted from multiple Excel files within `isoExport_xlsx.zip`:
   - Manufacturer IDs: 1,653 entries
   - Industry Groups: 9 entries
   - Global NAME Functions: 97 entries
   - IG Specific NAME Functions: 287 entries
   - Vehicle Systems: ~15 unique systems derived from IG-specific data

5. **Data quality**: One PGN name mismatch found — PGN 65032 has a trailing space difference between duplicate entries ("Required Tractor Facilities message" vs "Required Tractor Facilities message "). Deduplication handles this correctly.
