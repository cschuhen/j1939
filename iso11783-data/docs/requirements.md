# Requirements: ISO 11783 Data Crate

## Overview

Two Cargo crates for managing ISO 11783 / ISOBUS constant definitions:

| Crate | Type | Target | Purpose |
|-------|------|--------|---------|
| `iso11783-data` | Library | `no_std` | Published to crates.io. Provides lookup functions mapping numeric values to human-readable strings and metadata. |
| `generator` | Binary | `std` | Standalone generator that parses Excel files from isobus.net and produces the `.rs` source files consumed by the library crate. |

The generated data files are committed to git alongside the library crate. They contain no runtime dependencies beyond what's needed for a `no_std` library (i.e., no heap allocation). The generation step is manual — CI automation is future scope.

## System Architecture

The system consists of two independent crates:

1. **Library Crate (`iso11783-data`)**: A `no_std` library providing lookup functions for ISO 11783 constants (PGNs, DDI values, NAME fields). Uses static slices and binary search — no heap allocation required.

2. **Generator Binary (`generator`)**: A `std` tool that downloads Excel/TXT files from isobus.net, parses them, validates the data, and generates Rust source files with lookup tables.

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        GENERATOR BINARY                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐   │
│  │   Download   │    │    Parse     │    │   Validate &     │   │
│  │              │───>│              │───>│   Sort           │   │
│  │ • isobus.net │    │ • calamine   │    │                  │   │
│  │ • ZIP extract│    │ • TXT parser │    │ • Deduplication  │   │
│  └──────────────┘    └──────────────┘    │ • Range checks   │   │
│                                         └────────┬─────────┘   │
│                                                  │              │
│                                         ┌────────▼─────────┐   │
│                                         │   Codegen        │   │
│                                         │                  │   │
│                                         │ • Static slices  │   │
│                                         │ • Binary search  │   │
│                                         │ • Lookup fn      │   │
│                                         └────────┬─────────┘   │
│                                                  │              │
└──────────────────────────────────────────────────┼──────────────┘
                                                   │
                                                   ▼
┌─────────────────────────────────────────────────────────────────┐
│                      LIBRARY CRATE (no_std)                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  src/                                                           │
│  ├── lib.rs                    # Feature-gated module exports   │
│  ├── pgn.rs                    # PGN name lookups (3,213 entr.)│
│  ├── isobus_params.rs          # ISOBUS param NAMEs (30 entries)│
│  ├── task_controller_ddi.rs    # DDI metadata (383 entries)    │
│  └── name.rs                   # NAME field lookups            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Data Models (Generated Code)

#### PGN Lookup (`pgn.rs`)

Maps PGN values to human-readable names. Uses static slice of tuples for binary search.

```rust
pub const PGN_LIST: &[(u32, &str)] = &[
    // ... entries sorted by PGN value for binary search ...
];

pub fn lookup(pgn: u32) -> Option<&'static str>;
```

#### ISOBUS Parameter NAME Lookup (`isobus_params.rs`)

Maps numeric parameter values to human-readable names. Same pattern as PGN.

```rust
pub const PARAM_NAME_LIST: &[(u32, &str)] = &[
    // ... entries sorted by value for binary search ...
];

pub fn lookup(value: u32) -> Option<&'static str>;
```

#### Task Controller DDI Lookup (`task_controller_ddi.rs`)

Maps DDI values to structured metadata including name, unit, resolution, and offset.

```rust
#[derive(Debug, Clone, Copy)]
pub struct DdiInfo {
    pub ddi: u16,
    pub name: &'static str,
    pub unit: Option<&'static str>,
    pub resolution: f64,
    pub offset: i32,
}

pub const DDI_LIST: &[DdiInfo] = &[...];
pub fn lookup(ddi: u16) -> Option<&'static DdiInfo>;
pub fn to_physical(ddi: u16, raw: i32) -> Option<f64>;
```

**Design notes:**
- `to_physical()` takes signed `i32` since most Task Controller DDI values are 32-bit signed integers.
- Special range handling (error, not available) is left to consumers — they can check raw values against known ranges using conventions in `docs/j1939_data_types.md`.

#### NAME Lookup (`name.rs`)

Maps ISOBUS NAME structure fields to human-readable strings. Uses bit-packing for multi-argument lookups.

```rust
// Simple u8 -> string lookups
pub fn manufacturer_id_lookup(id: u8) -> Option<&'static str>;
pub fn industry_group_lookup(id: u8) -> Option<&'static str>;
pub fn global_function_lookup(func_id: u16) -> Option<&'static str>;

// Bit-packed multi-argument lookups
pub fn ig_specific_function_lookup(ig: u8, vs: u8, func_id: u16) -> Option<&'static str>;
pub fn vehicle_system_lookup(ig: u8, vs: u8) -> Option<&'static str>;
```

**Bit-packing design:**
- IG-specific functions key: `(ig << 24) | (vs << 16) | func_id` (32 bits total)
- Vehicle systems key: `(ig << 16) | vs` (16 bits, stored in u32 for consistency)

### 2. Generator Tool (`generator`)

#### CLI Interface

```bash
# Download all files and generate Rust source
cargo run -p generator -- generate

# Generate only specific modules
cargo run -p generator -- generate --module pgn
cargo run -p generator -- generate --module task_controller_ddi

# Specify custom Excel file paths (skip download)
cargo run -p generator -- generate \
    --pgn-file /path/to/SPNs\ and\ PGNs.xlsx \
    --params-file /path/to/AEF\ Functionalities.xlsx \
    --ddi-file /path/to/TaskControllerDDI.txt

# Show source metadata without regenerating
cargo run -p generator -- info
```

#### Generation Process

1. **Download**: Fetch from isobus.net URLs into local `downloads/` directory (ZIP extraction handled automatically).
2. **Parse**: Load each file, identify relevant sheets/columns, extract rows matching expected patterns.
3. **Validate**: Check for duplicates, missing values, out-of-range entries. Report warnings but continue.
4. **Sort**: Entries sorted by lookup key for binary search.
5. **Generate**: Produce `.rs` source files with header comments (source file name, revision number, download date), static slices, and lookup functions.
6. **Write output** to `src/` directory alongside `lib.rs`.

#### Revision Tracking

- Each generated file header includes: `// Source: <filename> rev <N>, downloaded <YYYY-MM-DD>`
- Revision number increments only when content changes (not on every run).
- Local `generator/revision.json` tracks current revision per source file.

### 3. Library Crate (`iso11783-data`)

#### Feature Gating

```toml
[features]
default = ["pgn", "isobus_params", "task_controller_ddi"]
pgn = []
isobus_params = []
task_controller_ddi = []
name = []
```

Consumers can reduce compile time / binary size by selecting only needed modules:

```toml
[dependencies]
iso11783-data = { version = "0.1", features = ["pgn"] }
# or with all modules:
iso11783-data = { version = "0.1" }
```

#### `no_std` Compatibility

- All generated code uses only `&'static str`, primitive types, and static slices — fully compatible with `no_std`.
- No heap allocation (`Vec`, `String`) in the library crate.
- Binary search on static slices works without `alloc`.
- The `to_physical()` function uses `f64` arithmetic available in `core`.

## Data Sources

| Source | URL | Target Module | Rows | Format |
|--------|-----|---------------|------|--------|
| PGN definitions | `isoExport_xlsx.zip` → `"SPNs and PGNs.xlsx"` inside zip | `pgn.rs` | 3,213 unique (from ~17,950 total) | Excel (.xlsx) |
| ISOBUS Parameters | `isoExport_xlsx.zip` → `AEF Functionalities.xlsx` inside zip | `isobus_params.rs` | 30 | Excel (.xlsx) |
| Task Controller DDI | `completeTXT` (TXT format) | `task_controller_ddi.rs` | 383 | TXT ("DD Entity:" blocks) |
| NAME lookups | `isoExport_xlsx.zip` → extracted Excel files | `name.rs` | ~2,052 total | Excel (.xlsx) |

### NAME Lookup Data Sources

| Table | Source File | Rows | Lookup Signature |
|-------|------------|------|------------------|
| Manufacturer IDs | `Manufacturer IDs.xlsx` | 1,653 | `manufacturer_id_lookup(u8)` |
| Industry Groups | `Industry Groups.xlsx` | 9 | `industry_group_lookup(u8)` |
| Global NAME Functions | `Global NAME Functions.xlsx` | 97 | `global_function_lookup(u16)` |
| IG Specific NAME Functions | `IG Specific NAME Function.xlsx` | 287 | `ig_specific_function_lookup(ig, vs, func)` |
| Vehicle Systems | Derived from IG Specific NAME data | ~15 | `vehicle_system_lookup(ig, vs)` |

## Module Organization

The library crate has one module per data domain:

| Module | Contents | When to include |
|--------|----------|-----------------|
| `pgn` | PGN name lookups | Always needed for any J1939 work |
| `isobus_params` | ISOBUS parameter NAME lookups | Only when working with ISOBUS/ARBI protocols |
| `task_controller_ddi` | Task Controller DDI lookups + metadata | Only when implementing ISO-11783-10 (Task Controller) |
| `name` | ISOBUS NAME field lookups | Only when working with ISOBUS/NAME structure decoding |

## Dependencies / Tech Stack

### Library Crate (`iso11783-data`)

| Purpose | Dependency | Notes |
|---------|-----------|-------|
| Binary search | `core::slice` | No external dependencies, works in `no_std` |
| f64 arithmetic | `core::f64` | Available in `no_std` |

### Generator Crate (`generator`)

| Purpose | Crate | Notes |
|---------|-------|-------|
| Excel parsing | `calamine` | .xlsx file parsing |
| HTTP download | `reqwest` | Download from isobus.net URLs |
| ZIP extraction | `zip` | Extract downloaded archives |
| JSON tracking | `serde_json` | Revision tracking in `revision.json` |

## Testing Strategy

| Level | Scope | Approach |
|-------|-------|----------|
| **Unit Tests** | Lookup functions, sorted order validation, list counts | Verify binary search correctness, entry count accuracy |
| **Integration Tests** | Full pipeline: download → parse → generate → compile | End-to-end verification of generator output |
| **no_std Compilation** | Library crate compilation without std library | Verified with `--target thumbv7m-none-eabi` |

### Current Test Coverage

| Module | Tests | Status |
|--------|-------|--------|
| PGN tests (`tests/pgn_tests.rs`) | 8 | All passing |
| ISOBUS params tests (`tests/isobus_params_tests.rs`) | 7 | All passing |
| Task Controller DDI tests (`tests/task_controller_ddi_tests.rs`) | 12 | All passing |
| **Total** | **27** | **All passing** |

## Configuration

- Generator uses hardcoded download URLs from isobus.net.
- Custom file paths can be specified via CLI flags to skip downloads.
- Revision tracking stored in `generator/revision.json`.
- Library features controlled via `Cargo.toml` feature flags.

## Error Handling

### Generator

- Download failures: prints error to stderr, continues with existing files if available.
- Parse errors: reports warnings for duplicates/out-of-range entries but continues generation.
- Missing source files: skips module generation with warning message.

### Library Crate

- Lookup functions return `Option<&'static ...>` — callers handle missing entries gracefully.
- `to_physical()` returns `None` if DDI not found in lookup table.
- No panics expected in normal usage — all access is bounds-checked via binary search.

## Open Questions

1. **Sheet naming**: Sheet names are stable across revisions. The generator locks onto specific sheet names once identified during Phase 0 investigation. No tolerance logic needed.
2. **SAE vs ASAM comments**: Resolved — no explicit approval type column exists in any source file. Generated code omits such comments.
3. **`to_physical()` placement**: Confirmed to stay inside `task_controller_ddi.rs`. Most DDI values are 32-bit signed integers; scaling factor and unit handle resolution needs without varying data sizes.
