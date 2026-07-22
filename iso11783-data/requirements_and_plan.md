# ISO 11783 Data Crate — Requirements & Plan

## Overview

Two Cargo crates for managing ISO 11783 / ISOBUS constant definitions:

| Crate | Type | Target | Purpose |
|-------|------|--------|---------|
| `iso11783-data` | Library | `no_std` | Published to crates.io. Provides lookup functions mapping numeric values to human-readable strings and metadata. |
| `generator` | Binary | `std` | Standalone generator that parses Excel files from isobus.net and produces the `.rs` source files consumed by the library crate. |

The generated data files are committed to git alongside the library crate. They contain no runtime dependencies beyond what's needed for a `no_std` library (i.e., no heap allocation). The generation step is manual — CI automation is future scope.

---

## Workspace Structure

```
iso11783-data/                    # repo root + library crate (no_std)
├── Cargo.toml                   # workspace root + library package definition
├── README.md                    # crate-level docs
├── src/                         # library source
│   ├── lib.rs                   # re-exports all modules
│   ├── strings/                 # generated: lookup tables and functions
│   │   ├── pgn.rs               # PGN_LIST + lookup()
│   │   ├── isobus_params.rs     # PARAM_NAME_LIST + lookup()
│   │   ├── task_controller_ddi.rs  # DDI_LIST + lookup() + to_physical()
│   │   └── name.rs              # NAME lookup tables and functions
│   └── constants/               # generated: named constants
│       ├── pgn.rs               # Named PGN constants (e.g., PROCESS_DATA)
│       ├── isobus_params.rs     # ISOBUS parameter constants
│       ├── task_controller_ddi.rs  # DDI constants
│       └── name/                # Hierarchical NAME constants
│           ├── mod.rs           # Re-exports manufacturer_ids, industry_groups
│           ├── manufacturer_ids.rs  # Manufacturer ID constants (u8)
│           └── industry_groups/     # Industry group + vehicle system constants
│               ├── mod.rs       # Industry group constants + vehicle_systems submodules
│               ├── TRACTOR.rs   # IG-specific function constants for Tractor
│               ├── EXCAVATOR.rs # IG-specific function constants for Excavator
│               └── ...          # One file per vehicle system with entries
└── generator/                    # generator binary crate (std)
    ├── Cargo.toml
    └── src/
        ├── main.rs              # entry point, CLI args
        ├── excel_loader.rs      # calamine-based Excel parsing
        ├── parsers/             # per-sheet parsers
        │   ├── pgn_parser.rs
        │   ├── isobus_params_parser.rs
        │   ├── task_controller_ddi_parser.rs
        │   └── name_parsers.rs  # NAME lookup table parsers
        └── codegen.rs           # Rust source file generation
```

The repo root `Cargo.toml` defines a workspace with two members: the library crate (defined inline via `[package]`) and the generator. The generated `.rs` files live in `src/strings/` (lookup tables) and `src/constants/` (named constants). The NAME constants are organized hierarchically by industry group, with one directory per IG containing a mod.rs and vehicle system submodule files.

---

## Data Models (Generated Code)

### PGN Lookup (`pgn.rs`)

Each entry maps a PGN value to its name. The generated module contains:

- A static slice of tuples for binary search: `pub const PGN_LIST: &[(u32, &str)] = ...`
- A lookup function: `pub fn lookup(pgn: u32) -> Option<&'static str>`
- Optional named constants for well-known PGNs (e.g., `PROCESS_DATA`, `REQUEST`)

```rust
/// Source: ISOBUSStandardParts.xlsx rev 1.4.2, downloaded 2026-07-19
pub const PGN_LIST: &[(u32, &str)] = &[
    // ... entries sorted by PGN value for binary search ...
];

pub fn lookup(pgn: u32) -> Option<&'static str> {
    match PGN_LIST.binary_search_by_key(&pgn, |(value, _)| *value) {
        Ok(idx) => Some(PGN_LIST[idx].1),
        Err(_) => None,
    }
}

// Optional: named constants for frequently-used PGNs
pub const PROCESS_DATA: u32 = 0x00cb00; // 51968
```

### ISOBUS Parameter NAME Lookup (`isobus_params.rs`)

Maps numeric parameter values (excluding purely numeric fields like Identity/Instance) to human-readable names. Same pattern as PGN — static slice + binary search lookup.

```rust
/// Source: ISOBUSParameters.xlsx rev 2.1.0, downloaded 2026-07-19
pub const PARAM_NAME_LIST: &[(u32, &str)] = &[
    // ... entries sorted by value for binary search ...
];

pub fn lookup(value: u32) -> Option<&'static str> {
    match PARAM_NAME_LIST.binary_search_by_key(&value, |(v, _)| *v) {
        Ok(idx) => Some(PARAM_NAME_LIST[idx].1),
        Err(_) => None,
    }
}
```

### Task Controller DDI Lookup (`task_controller_ddi.rs`)

Maps DDI values to structured metadata including name, unit, resolution, and offset. Uses a static slice of structs for binary search. Most DDI values are 32-bit signed integers; scaling factor and unit are chosen to ensure sufficient resolution rather than varying data sizes.

```rust
/// Source: TaskControllerDDI.xlsx rev 3.0.1, downloaded 2026-07-19
#[derive(Debug, Clone, Copy)]
pub struct DdiInfo {
    pub ddi: u16,
    pub name: &'static str,
    /// e.g., "RPM", "degrees Celsius", "kPa" — None if not specified
    pub unit: Option<&'static str>,
    /// Resolution (factor) for physical value conversion: physical = raw * resolution + offset
    pub resolution: f64,
    /// Offset for physical value conversion
    pub offset: f64,
}

pub const DDI_LIST: &[DdiInfo] = &[
    // ... entries sorted by DDI value for binary search ...
];

pub fn lookup(ddi: u16) -> Option<&'static DdiInfo> {
    match DDI_LIST.binary_search_by_key(&ddi, |info| info.ddi) {
        Ok(idx) => Some(&DDI_LIST[idx]),
        Err(_) => None,
    }
}

/// Convert raw i32 value to physical value using the DDI's resolution and offset.
pub fn to_physical(ddi: u16, raw: i32) -> Option<f64> {
    let info = lookup(ddi)?;
    Some((raw as f64) * info.resolution + info.offset)
}
```

**Design notes:**
- `to_physical()` is included for convenience. It takes a signed `i32` since most Task Controller DDI values are 32-bit signed integers.
- Special range handling (error, not available) is left to consumers — they can check raw values against known ranges using the conventions in `docs/j1939_data_types.md`. This keeps the generated code minimal and extensible.

---

## NAME Lookup Module (`name`)

Maps ISOBUS NAME structure fields (part of ISO 11783-2 CAN Name) to human-readable strings. The NAME structure contains: Manufacturer ID, Industry Group, Vehicle System, Function, Function Instance, ECU Instance, Serial Number. This module provides lookups for all enumerated fields except Identity/Instance (which are purely numeric).

### Data Sources

| Table | Source File | Rows | Lookup Signature |
|-------|------------|------|------------------|
| Manufacturer IDs | `downloads/extract/Manufacturer IDs.xlsx` | 1,653 | `manufacturer_id_lookup(u8) -> Option<&str>` |
| Industry Groups | `downloads/extract/Industry Groups.xlsx` | 9 | `industry_group_lookup(u8) -> Option<&str>` |
| Global NAME Functions | `downloads/extract/Global NAME Functions.xlsx` | 97 | `global_function_lookup(u16) -> Option<&str>` |
| IG Specific NAME Functions | `downloads/extract/IG Specific NAME Function.xlsx` | 287 | `ig_specific_function_lookup(ig: u8, vs: u8, func: u16) -> Option<&str>` |
| Vehicle Systems | Derived from IG Specific NAME data | ~15 | `vehicle_system_lookup(ig: u8, vs: u8) -> Option<&str>` |

### Lookup Design (Option C — Flat tables with bit-packing)

All lookups use static slices of packed keys for binary search. Keys are constructed by bit-shifting multi-argument indices into a single 32-bit value.

```rust
/// Source: ISOBUS NAME lookup tables, downloaded YYYY-MM-DD

// Manufacturer IDs: simple u8 -> string
pub const MANUFACTURER_ID_LIST: &[(u8, &str)] = &[
    (0, "For experimental or developmental use only."),
    (1, "Bendix Commercial Vehicle Systems LLC"),
    // ... sorted by manufacturer ID
];

pub fn manufacturer_id_lookup(id: u8) -> Option<&'static str> {
    match MANUFACTURER_ID_LIST.binary_search_by_key(&id, |(value, _)| *value) {
        Ok(idx) => Some(MANUFACTURER_ID_LIST[idx].1),
        Err(_) => None,
    }
}

// Industry Groups: simple u8 -> string
pub const INDUSTRY_GROUP_LIST: &[(u8, &str)] = &[
    (0, "Global, applies to all"),
    (1, "On-Highway Equipment"),
    // ... sorted by industry group ID
];

pub fn industry_group_lookup(id: u8) -> Option<&'static str> {
    match INDUSTRY_GROUP_LIST.binary_search_by_key(&id, |(value, _)| *value) {
        Ok(idx) => Some(INDUSTRY_GROUP_LIST[idx].1),
        Err(_) => None,
    }
}

// Global NAME Functions: simple u16 -> string
pub const GLOBAL_FUNCTION_LIST: &[(u16, &str)] = &[
    (0, "Engine"),
    (1, "Auxiliary Power Unit (APU)"),
    // ... sorted by function ID
];

pub fn global_function_lookup(func_id: u16) -> Option<&'static str> {
    match GLOBAL_FUNCTION_LIST.binary_search_by_key(&func_id, |(value, _)| *value) {
        Ok(idx) => Some(GLOBAL_FUNCTION_LIST[idx].1),
        Err(_) => None,
    }
}

// IG Specific NAME Functions: 3-arg lookup via bit-packing
// Key = (ig << 24) | (vs << 16) | func_id
pub const IG_SPECIFIC_FUNCTION_LIST: &[(u32, &str)] = &[
    // packed key constructed as: ((ig as u32) << 24) | ((vs as u32) << 16) | (func_id as u32)
    // ... sorted by packed key
];

pub fn ig_specific_function_lookup(ig: u8, vs: u8, func_id: u16) -> Option<&'static str> {
    let key = ((ig as u32) << 24) | ((vs as u32) << 16) | (func_id as u32);
    match IG_SPECIFIC_FUNCTION_LIST.binary_search_by_key(&key, |(value, _)| *value) {
        Ok(idx) => Some(IG_SPECIFIC_FUNCTION_LIST[idx].1),
        Err(_) => None,
    }
}

// Vehicle Systems: 2-arg lookup via bit-packing
// Key = (ig << 16) | vs
pub const VEHICLE_SYSTEM_LIST: &[(u32, &str)] = &[
    // packed key constructed as: ((ig as u32) << 16) | (vs as u32)
    // ... sorted by packed key
];

pub fn vehicle_system_lookup(ig: u8, vs: u8) -> Option<&'static str> {
    let key = ((ig as u32) << 16) | (vs as u32);
    match VEHICLE_SYSTEM_LIST.binary_search_by_key(&key, |(value, _)| *value) {
        Ok(idx) => Some(VEHICLE_SYSTEM_LIST[idx].1),
        Err(_) => None,
    }
}
```

**Design notes:**
- Bit-packing avoids nested data structures while keeping lookup efficient (single binary search)
- `u8` indices are safe — industry groups use 0-5, vehicle systems use 0-255, function IDs use 0-65535
- The packed key for IG-specific functions uses 8 bits for ig, 8 bits for vs, 16 bits for func_id = 32 bits total
- The packed key for vehicle systems uses 8 bits for ig, 8 bits for vs = 16 bits (stored in u32 for consistency)
- All tables sorted by their lookup key for binary search

---

## Module Organization

The library crate has one module per data domain, split into two submodules:

| Module | Contents | When to include |
|--------|----------|-----------------|
| `pgn` | PGN name lookups | Always needed for any J1939 work |
| `isobus_params` | ISOBUS parameter NAME lookups | Only when working with ISOBUS/ARBI protocols |
| `task_controller_ddi` | Task Controller DDI lookups + metadata | Only when implementing ISO-11783-10 (Task Controller) |
| `name` | ISOBUS NAME field lookups (manufacturer, industry group, vehicle system, functions) | Only when working with ISOBUS/ARBI protocols and NAME structure decoding |

Each module is split into two submodules:

| Submodule | Contents | Example |
|-----------|----------|---------|
| `strings` | Lookup tables (`*_LIST` slices) and lookup functions | `iso11783_data::strings::pgn::lookup()` |
| `constants` | Named constants for well-known values | `iso11783_data::constants::pgn::PROCESS_DATA` |

The main `lib.rs` re-exports all modules unconditionally. Consumers that want to reduce compile time / binary size can use conditional compilation via features:

```toml
# Cargo.toml of consumer
[dependencies]
iso11783-data = { version = "0.1", features = ["pgn"] }
# or with all modules:
iso11783-data = { version = "0.1" }
```

Each module is gated by a feature flag (`pgn`, `isobus_params`, `task_controller_ddi`). The generator always produces all three, but the library's `Cargo.toml` controls which are compiled in.

---

## Generator Tool (`generator`)

### Dependencies

- `calamine` — Excel (.xlsx) file parsing
- `serde` / `serde_json` — optional, for intermediate data representation and debugging
- `reqwest` (optional) — direct download from isobus.net URLs

### CLI Interface

```bash
# Download all files and generate Rust source
cargo run -p generator -- generate

# Generate only specific modules
cargo run -p generator -- generate --module pgn
cargo run -p generator -- generate --module task_controller_ddi

# Specify custom Excel file paths (skip download)
cargo run -p generator -- generate \
    --pgn-file /path/to/ISOBUSStandardParts.xlsx \
    --params-file /path/to/ISOBUSParameters.xlsx \
    --ddi-file /path/to/TaskControllerDDI.xlsx

# Show source metadata without regenerating
cargo run -p generator -- info
```

### Generation Process

1. **Download** (if files not provided): fetch from isobus.net URLs into a local `downloads/` directory.
2. **Parse**: load each `.xlsx` file, identify the relevant sheet(s), extract rows matching expected column patterns.
3. **Validate**: check for duplicates, missing values, out-of-range entries. Report warnings but continue.
4. **Sort**: entries sorted by their lookup key (PGN value, parameter value, or DDI value).
5. **Generate**: produce `.rs` source files with:
   - Header comment containing source file name, revision number, and download date
   - Constants / structs as defined in the Data Models section above
   - Lookup functions using binary search on static slices
6. **Write output** to the repo root `src/` directory (alongside `lib.rs`).

### Revision Tracking

The generator tracks a revision number that increments whenever there is an actual change in the generated data:

- Each generated file header includes: `// Source: <filename> rev <N>, downloaded <YYYY-MM-DD>`
- The revision number `<N>` starts at 1 and increments only when content changes (not on every run)
- A local `revision.json` file in the generator crate tracks the current revision per source file:
  ```json
  {
    "pgn": {"source": "ISOBUSStandardParts.xlsx", "rev": 4, "date": "2026-07-19"},
    "isobus_params": {"source": "ISOBUSParameters.xlsx", "rev": 2, "date": "2026-07-19"},
    "task_controller_ddi": {"source": "TaskControllerDDI.xlsx", "rev": 5, "date": "2026-07-19"}
  }
  ```

### Excel Sheet Analysis (Investigation Phase)

Before writing the generator, we need to confirm:
- Which sheet(s) in each workbook contain the data we need
- Column names and their positions (they may vary between sheets)
- Whether "SAE-approved" vs "ASAM-approved" values are distinguishable (for comments)
- Edge cases: merged cells, multi-line descriptions, numeric-only entries to exclude

---

## Library Crate (`iso11783-data`)

### `Cargo.toml`

```toml
[package]
name = "iso11783-data"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
description = "ISO 11783 / ISOBUS constant definitions and lookup functions"
keywords = ["j1939", "isobus", "iso11783", "pgn", "ddi"]
categories = ["embedded", "hardware-support", "no-std"]
repository = "https://github.com/cschuhen/j1939"

[features]
default = ["pgn", "isobus_params", "task_controller_ddi"]
pgn = []
isobus_params = []
task_controller_ddi = []
```

### `lib.rs`

```rust
#![no_std]

// Strings modules (lookup tables and functions)
#[cfg(feature = "pgn")]
mod pgn;

#[cfg(feature = "isobus_params")]
mod isobus_params;

#[cfg(feature = "task_controller_ddi")]
mod task_controller_ddi;

#[cfg(feature = "name")]
mod name;

// Public re-exports for strings (lookup tables)
pub mod strings {
    #[cfg(feature = "pgn")]
    pub mod pgn { pub use crate::strings_pgn::*; }
    #[cfg(feature = "isobus_params")]
    pub mod isobus_params { pub use crate::strings_isobus_params::*; }
    #[cfg(feature = "task_controller_ddi")]
    pub mod task_controller_ddi { pub use crate::strings_task_controller_ddi::*; }
    #[cfg(feature = "name")]
    pub mod name { pub use crate::strings_name::*; }
}

// Public re-exports for constants (named values)
pub mod constants {
    #[cfg(feature = "pgn")]
    pub mod pgn { pub use crate::pgn::*; }
    #[cfg(feature = "isobus_params")]
    pub mod isobus_params { pub use crate::isobus_params::*; }
    #[cfg(feature = "task_controller_ddi")]
    pub mod task_controller_ddi { pub use crate::task_controller_ddi::*; }
    #[cfg(feature = "name")]
    pub mod name { pub use crate::name::*; }
}
```

### NAME Constants Hierarchy (`constants/name/`)

The NAME constants module has a hierarchical structure organized by industry group:

```rust
// constants/name/mod.rs — re-exports submodules
pub mod manufacturer_ids;
pub mod global;
pub mod on_highway_equipment;
pub mod agricultural_and_forestry_equipment;
// ... one submodule per industry group (IG > 0)

// constants/name/manufacturer_ids.rs — 256 manufacturer ID constants (u8)
pub const CATERPILLAR_INC: u8 = 8;
pub const DEERE_COMPANY: u8 = 12;
// ...

// constants/name/global.rs — IG-specific function constants for Global (IG 0), grouped by vehicle system
/// Vehicle system: on board data logger
pub const ON_BOARD_DATA_LOGGER: u32 = 130;
// ...

// constants/name/on_highway_equipment/mod.rs — industry group constant + vehicle system submodule declarations
pub const ON_HIGHWAY_EQUIPMENT: u8 = 1;
pub mod tractor;        // Vehicle system submodule
pub mod trailer;        // Vehicle system submodule
// ...

// constants/name/on_highway_equipment/tractor.rs — IG-specific function constants for Tractor (IG=1)
pub const FORWARD_ROAD_IMAGE_PROCESSING: u32 = 16842880;
pub const FIFTH_WHEEL_SMART_SYSTEM: u32 = 16842881;
// ...
```

Usage:
```rust
use iso11783_data::constants::name::manufacturer_ids::CATERPILLAR_INC;
use iso11783_data::constants::name::on_highway_equipment::ON_HIGHWAY_EQUIPMENT;
use iso11783_data::constants::name::on_highway_equipment::tractor::FORWARD_ROAD_IMAGE_PROCESSING;
```

Usage:
```rust
use iso11783_data::constants::name::manufacturer_ids::CATERPILLAR_INC;
use iso11783_data::constants::name::industry_groups::ON_HIGHWAY_EQUIPMENT;
use iso11783_data::constants::name::industry_groups::TRACTOR::FORWARD_ROAD_IMAGE_PROCESSING;
```

### `no_std` Compatibility

- All generated code uses only `&'static str`, primitive types, and static slices — fully compatible with `no_std`.
- No heap allocation (`Vec`, `String`) in the library crate.
- Binary search on static slices works without `alloc`.
- The `to_physical()` function in `task_controller_ddi` uses `f64` arithmetic which is available in `core`.

---

## Current Status (Updated 2026-07-22)

| Item | Status | Notes |
|------|--------|-------|
| Generator crate skeleton | **Done** | `main.rs`, `excel_loader.rs`, `data.rs` implemented with CLI commands |
| Download infrastructure | **Done** | Downloads all sources, extracts zip archives automatically |
| PGN parser & codegen | **Done** | Integrated into main binary; generates strings/pgn.rs (3,213 entries) + constants/pgn.rs |
| ISOBUS params parser & codegen | **Done** | Uses `AEF Functionalities.xlsx`; generates strings/isobus_params.rs (30 entries) |
| Task Controller DDI parser & codegen | **Done** | Parses TXT format; generates strings/task_controller_ddi.rs (383 entries) + constants/task_controller_ddi.rs |
| NAME lookup parser & codegen | **Done** | Parses 5 Excel sheets; generates strings/name.rs + hierarchical constants/name/ structure |
| CLI commands | **Done** | `generate`, `info` subcommands; `--module`, custom file path flags |
| Revision tracking | **Done** | `revision.json` tracks source, revision number, date per module |
| Library crate (`lib.rs`) | **Done** | Feature-gated modules, `#![no_std]`, compiles in std mode |
| Generated `.rs` files — strings | **Done** | Lookup tables and functions: pgn.rs (157KB), isobus_params.rs (1.7KB), task_controller_ddi.rs (44KB), name.rs (30KB) |
| Generated `.rs` files — constants | **Done** | Named constants: pgn.rs (191KB), task_controller_ddi.rs (20KB), name/ hierarchy (32KB + 60 vehicle system files) |
| Integration tests | **Done** | 53 tests passing: pgn (8), isobus_params (7), task_controller_ddi (12), name (26) |
| no_std compilation | **Done** | Verified with `--target thumbv7m-none-eabi` — passes |

### Key Findings from Investigation

1. **PGN source**: The planned `ISOBUSStandardParts.xlsx` is only a version catalog (19 rows). The actual PGN data lives in `"SPNs and PGNs.xlsx"` inside the zip extracted from `isoExport_xlsx.zip` (the ISOBUS Parameters download). This file has 2 columns: PGN value (column A) and name (column B), with ~17,950 total rows producing 3,213 unique entries after deduplication.

2. **ISOBUS Parameters**: The zip contains 13 xlsx files. `AEF Functionalities.xlsx` was identified as the source for parameter NAME values (30 entries). Configurable via `--params-file` flag.

3. **Task Controller DDI source**: Downloaded as TXT format (`TaskControllerDDI.txt`, version 2026050501, May 2026). Uses "DD Entity:" blocks with Definition, Unit, and Resolution fields. No Offset field in the source data (all offsets default to 0.0).

4. **Data quality**: One PGN name mismatch found — PGN 65032 has a trailing space difference between duplicate entries ("Required Tractor Facilities message" vs "Required Tractor Facilities message ").

### NAME Module Extraction Results

The NAME lookup module was extracted from 5 Excel source files:

| Table | Source File | Entries |
|-------|------------|---------|
| Manufacturer IDs | `downloads/extract/Manufacturer IDs.xlsx` | 256 unique IDs (u8) |
| Industry Groups | `downloads/extract/Industry Groups.xlsx` | 8 groups (u8) |
| Global NAME Functions | `downloads/extract/Global NAME Functions.xlsx` | 95 functions (u16) |
| IG Specific NAME Functions | `downloads/extract/IG Specific NAME Function.xlsx` | 284 entries (packed u32 keys) |
| Vehicle Systems | Derived from IG Specific data | 70 unique systems |

The constants module uses a hierarchical structure:
- `constants/name/mod.rs` — re-exports `manufacturer_ids` and `industry_groups` submodules
- `constants/name/manufacturer_ids.rs` — 256 manufacturer ID constant definitions (u8)
- `constants/name/industry_groups/mod.rs` — industry group constants + vehicle system submodule declarations
- `constants/name/industry_groups/<vehicle_system>.rs` — one file per vehicle system with IG-specific function constants (u32 packed keys)

Duplicate constant names are handled by appending numeric suffixes (e.g., `RESERVED_FOR_FUTURE_ASSIGNMENT_BY_SAE_2`).

### Phase 0: Investigation — Excel File Analysis ✅ **COMPLETE**

**Goal**: Confirm the proposed Rust library design is feasible by loading and analyzing the actual Excel files.

**Completed steps**:
1. ✅ Create `generator` crate skeleton with `calamine`, `reqwest`, `zip` dependencies.
2. ✅ Implement minimal code to download all 3 sources from isobus.net URLs.
3. ✅ Open each file, list available sheets, and dump sheet names + row/column counts. Full dumps saved to `downloads/`.
4. ✅ Identify the relevant sheet(s) — discovered that PGN data lives in `"SPNs and PGNs.xlsx"` inside extracted zip (not `ISOBUSStandardParts.xlsx` as originally planned). ISOBUS Parameters target file within 13 xlsx files needs identification.
5. ✅ Extract sample rows from each target source and print them to confirm column structure.
6. ✅ SAE vs ASAM approval distinction — **resolved**: no explicit approval type column exists in any source file. All 13 xlsx files were searched; "SAE" only appears as documentation URLs, zero "ASAM" mentions. Generated code will omit such comments.

**Deliverable**: A summary of findings confirming which sheets/columns to parse, and any data quality issues encountered. See **Current Status** section above.

### Phase 1: Generator Tool — Core Parsing & Codegen ✅ **COMPLETE**

**Goal**: Build the generator that produces all generated modules.

**Completed steps**:
1. ✅ PGN parser integrated into main binary (`parsers/pgn_parser.rs`) — extracts, deduplicates, and sorts 3,213 unique PGNs from `"SPNs and PGNs.xlsx"`.
2. ✅ Task Controller DDI parser wired into main binary (`parsers/task_controller_ddi_parser.rs`) — parses DD Entity blocks from TXT format (DDI number, name, unit, resolution). Offset always 0.0 since source has no Offset field.
3. ✅ ISOBUS params parser implemented (`parsers/isobus_params_parser.rs`) — uses `AEF Functionalities.xlsx` as default source (value→meaning mapping, 30 entries). Configurable via `--params-file` flag.
4. ✅ Validation logic — deduplication + 24-bit range check for PGNs; deduplication for ISOBUS params and DDI entries. Warnings printed to stderr.
5. ✅ Rust source code generation (`codegen.rs`) — produces lookup tables (static slices with binary search) in `src/strings/` and named constants in `src/constants/`.
6. ✅ CLI `generate` command with module selection (`--module pgn|isobus_params|task_controller_ddi|name`) and custom file paths (`--pgn-file`, `--params-file`, `--ddi-file`). Also supports `info` subcommand.
7. ✅ Revision tracking (`revision.json`) — tracks source file, revision number, and download date per module. Revision increments only when content changes.

**Deliverable**: Running generator that produces all `.rs` files in `src/strings/` (lookup tables) and `src/constants/` (named constants). Verified: strings/pgn.rs (157KB, 3213 entries), strings/isobus_params.rs (1.7KB, 30 entries), strings/task_controller_ddi.rs (44KB, 383 entries), strings/name.rs (30KB).

### Phase 2: Library Crate — Assembly & Testing ✅ **COMPLETE**

**Goal**: Assemble the library crate with generated code and verify it compiles as both `std` and `no_std`.

**Completed steps**:
1. ✅ Library crate skeleton exists (`Cargo.toml` with features).
2. ✅ Generated `.rs` files produced by Phase 1 generator — lookup tables in `src/strings/`, named constants in `src/constants/`.
3. ✅ `lib.rs` updated with feature-gated module declarations and public re-exports for `strings::` and `constants::` submodules (`#![no_std]`).
4. ✅ Compilation verified in `std` mode (default) — `cargo check` passes.
5. ✅ Compilation verified in `no_std` mode (`--target thumbv7m-none-eabi`) — passes.
6. ✅ Integration tests written and passing — 53 tests total:
   - `tests/pgn_tests.rs` (8 tests): lookup, sorted order, list count, binary search efficiency
   - `tests/isobus_params_tests.rs` (7 tests): lookup, sorted order, list count
   - `tests/task_controller_ddi_tests.rs` (12 tests): lookup, to_physical, sorted order, list count, name validation
   - `tests/name_tests.rs` (26 tests): manufacturer IDs, industry groups, global functions, IG-specific functions, vehicle systems lookups
7. ✅ Feature gating verified — compiles with individual features (`pgn`, `isobus_params`, `task_controller_ddi`, `name`) in both std and no_std modes.

**Deliverable**: Library crate compiles, passes 53 tests, and is ready for use in `can_decoder`.

### Phase 2.5: NAME Module Extraction & Hierarchical Constants ✅ **COMPLETE**

**Goal**: Extract NAME lookup data from Excel files and organize constants into a hierarchical module structure organized by industry group.

**Completed steps**:
1. ✅ NAME parsers implemented (`parsers/name_parsers.rs`, `parsers/name_parsers_ig.rs`) — parse 5 Excel sheets: Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function, Vehicle Systems.
2. ✅ Strings generation — single file with lookup tables and functions for all NAME data types (manufacturer_id_lookup, industry_group_lookup, global_function_lookup, ig_specific_function_lookup, vehicle_system_lookup).
3. ✅ Constants hierarchical structure organized by industry group:
   - `constants/name/mod.rs` — re-exports manufacturer_ids, global, and one submodule per industry group (IG > 0)
   - `constants/name/manufacturer_ids.rs` — 256 manufacturer ID constants (u8)
   - `constants/name/global.rs` — IG-specific function constants for Global (IG 0), grouped by vehicle system with comments
   - `constants/name/<industry_group>/mod.rs` — industry group constant + vehicle system submodule declarations
   - `constants/name/<industry_group>/<vehicle_system>.rs` — one file per vehicle system within that IG with IG-specific function constants (u32 packed keys)
4. ✅ Duplicate constant name handling — appends numeric suffixes (e.g., `RESERVED_FOR_FUTURE_ASSIGNMENT_BY_SAE_2`).
5. ✅ Vehicle system submodule filtering — only generates files for vehicle systems that have at least one IG-specific function entry.

**Deliverable**: NAME module with 256 manufacturer IDs, 8 industry groups, 95 global functions, 284 IG-specific functions across ~70 vehicle systems (filtered to ~60 with entries). All accessible via `iso11783_data::strings::name` and `iso11783_data::constants::name`.

### Phase 3: Integration with `can_decoder`

**Goal**: Replace hardcoded DDI/PGN values in `can_decoder` with lookups from `iso11783-data`.

**Steps**:
1. Add `iso11783-data` as a dependency to `can_decoder/Cargo.toml`.
2. Replace hardcoded PGN constants in `can_decoder/src/task_controller.rs` with `iso11783_data::pgn::lookup()`.
3. Enhance Task Controller decoder output to use DDI metadata (name, unit) from `iso11783_data::task_controller_ddi::lookup()`.
4. Update display/rendering to show human-readable names instead of raw numeric values.

**Deliverable**: `can_decoder` displays meaningful names and units for Task Controller messages.

### Phase 4: Polish & Publication (Future)

- Add `README.md` with usage examples for the library crate.
- Prepare for crates.io publication (license, documentation, MSRV).
- GitHub Actions workflow for periodic regeneration (future scope per original requirements).

---

## Excel File URLs

https://www.isobus.net/isobus/exports/completeWithRequests
https://www.isobus.net/isobus/exports/complete

| Source | URL (Actual) | Target Module | Notes |
|--------|-------------|---------------|-------|
| PGN definitions | `https://www.isobus.net/isobus/attachments/isoExport_xlsx.zip` → `"SPNs and PGNs.xlsx"` inside zip | `pgn.rs` | Original plan (`ISOBUSStandardParts.xlsx`) is only a version catalog (19 rows) |
| ISOBUS Parameters / NAME data | `https://www.isobus.net/isobus/attachments/isoExport_xlsx.zip` → 13 xlsx files inside zip | `isobus_params.rs`, `name.rs` | `AEF Functionalities.xlsx` for params; Manufacturer IDs, Industry Groups, Global/IG Specific NAME Functions, Vehicle Systems sheets for NAME data |
| Task Controller DDI | `https://www.isobus.net/isobus/exports/completeTXT` (TXT format) | `task_controller_ddi.rs` | Not an xlsx — uses "DD Entity:" block format |

---

## Open Questions

1. **Sheet naming**: Sheet names are stable across revisions. The generator will lock onto specific sheet names once identified during Phase 0 investigation. No tolerance logic needed.
2. ~~**SAE vs ASAM comments**~~ — **Resolved (Phase 0)**: Searched all 13 xlsx files; no explicit approval type column exists. "SAE" only appears as documentation URLs, zero "ASAM" mentions anywhere. Generated code omits such comments.
3. ~~**ISOBUS Parameters target file**~~ — **Resolved (Phase 2.5)**: `AEF Functionalities.xlsx` identified as the source for parameter NAME values within the zip archive. Configurable via `--params-file` flag.
4. **`to_physical()` placement**: Confirmed to stay inside `task_controller_ddi.rs`. Most DDI values are 32-bit signed integers; scaling factor and unit handle resolution needs without varying data sizes. No separate crate needed.
