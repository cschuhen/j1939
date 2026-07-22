# Design Document: ISO 11783 Data Crate

## 1. Architecture Overview

The system follows a two-crate architecture:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        ISO 11783 DATA SYSTEM                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐    ┌──────────────────────────────────────┐   │
│  │   GENERATOR      │    │        LIBRARY CRATE (no_std)         │   │
│  │   (std binary)   │    │                                      │   │
│  │                  │    │  src/                                │   │
│  │  ┌────────────┐  │    │  ├── lib.rs              # re-exports│   │
│  │  │ Download   │  │    │  ├── pgn.rs              # PGN look │   │
│  │  │ (reqwest)  │──┼───>│  ├── isobus_params.rs    # param look│   │
│  │  └────────────┘  │    │  ├── task_controller_ddi.rs # DDI look│   │
│  │                  │    │  └── name.rs             # NAME look │   │
│  │  ┌────────────┐  │    │                                      │   │
│  │  │ Parse      │  │    │  All modules:                          │   │
│  │  │ (calamine) │──┼───>│  • Static slices for binary search    │   │
│  │  │ + TXT parser│ │    │  • lookup() functions returning Option│   │
│  │  └────────────┘  │    │  • no_std compatible (no heap alloc)  │   │
│  │                  │    │                                      │   │
│  │  ┌────────────┐  │    │  Feature-gated compilation:           │   │
│  │  │ Validate & │  │    │  #[cfg(feature = "pgn")]              │   │
│  │  │ Sort       │──┼───>│  #[cfg(feature = "task_controller_ddi")]│   │
│  │  └────────────┘  │    │                                      │   │
│  │                  │    │  Generated files committed to git:     │   │
│  │  ┌────────────┐  │    │  • No runtime dependencies            │   │
│  │  │ Codegen    │──┼───>│  • Revision tracking in headers       │   │
│  │  └────────────┘  │    │                                      │   │
│  └──────────────────┘    └──────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Data Flow

```
Generator (std)                    Library (no_std)
────────────                       ──────────────

isobus.net URLs              downloads/
    │                              │
    ▼                              ▼
calamine / reqwest       TaskControllerDDI.txt
    │                      SPNs and PGNs.xlsx
    ▼                      AEF Functionalities.xlsx
Parsed entries             Manufacturer IDs.xlsx
    │                      Industry Groups.xlsx
    ▼                      Global NAME Functions.xlsx
Validated & sorted     IG Specific NAME Function.xlsx
    │                              │
    ▼                              ▼
codegen.rs               Generated .rs files
    │                  (pgn.rs, isobus_params.rs,
    │                   task_controller_ddi.rs, name.rs)
    ▼                              │
Write to src/                      │
    │                              ▼
    └──────────────>  lib.rs re-exports feature-gated modules
                         │
                         ▼
                    Consumer crates (can_decoder, etc.)
```

## 2. Module Structure

### Library Crate (`iso11783-data/src/`)

```
iso11783-data/src/
├── lib.rs                   # Feature-gated module exports + no_std declaration
├── pgn.rs                   # Generated: PGN constants + lookup (3,213 entries)
├── isobus_params.rs         # Generated: ISOBUS parameter NAME lookups (30 entries)
├── task_controller_ddi.rs   # Generated: Task Controller DDI lookups (383 entries)
└── name.rs                  # Generated: NAME field lookups (~2,052 entries)

iso11783-data/tests/
├── pgn_tests.rs             # PGN lookup tests (8 tests)
├── isobus_params_tests.rs   # ISOBUS params tests (7 tests)
└── task_controller_ddi_tests.rs  # DDI tests (12 tests)
```

### Generator Crate (`iso11783-data/generator/src/`)

```
generator/src/
├── main.rs                  # Entry point, CLI args, command dispatch
├── excel.rs                 # Download + ZIP extraction utilities
├── data.rs                  # Data structures for parsed entries
└── parsers/
    ├── pgn_parser.rs        # Excel parser for "SPNs and PGNs.xlsx"
    ├── isobus_params_parser.rs  # Excel parser for AEF Functionalities
    ├── task_controller_ddi_parser.rs  # TXT parser for TaskControllerDDI.txt
    ├── name_parsers.rs      # NAME lookup parsers (manufacturer, industry group, global)
    └── name_parsers_ig.rs   # IG-specific NAME function + vehicle system parsers
└── codegen.rs               # Rust source file generation for all modules
```

## 3. Trait Definitions / Function Signatures

### Library Lookup Functions

All lookup functions follow the same pattern: static slice + binary search returning `Option`.

#### PGN Lookup (`pgn.rs`)

```rust
pub const PGN_LIST: &[(u32, &str)] = &[...];

/// Look up a PGN by its 18-bit value. Returns None if not found.
pub fn lookup(pgn: u32) -> Option<&'static str> {
    match PGN_LIST.binary_search_by_key(&pgn, |(value, _)| *value) {
        Ok(idx) => Some(PGN_LIST[idx].1),
        Err(_) => None,
    }
}
```

#### ISOBUS Params Lookup (`isobus_params.rs`)

```rust
pub const PARAM_NAME_LIST: &[(u32, &str)] = &[...];

/// Look up a parameter NAME by its numeric value. Returns None if not found.
pub fn lookup(value: u32) -> Option<&'static str> {
    match PARAM_NAME_LIST.binary_search_by_key(&value, |(v, _)| *v) {
        Ok(idx) => Some(PARAM_NAME_LIST[idx].1),
        Err(_) => None,
    }
}
```

#### Task Controller DDI Lookup (`task_controller_ddi.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DdiInfo {
    pub ddi: u16,
    pub name: &'static str,
    pub unit: Option<&'static str>,
    pub resolution: f64,
    pub offset: i32,
}

pub const DDI_LIST: &[DdiInfo] = &[...];

/// Look up a DDI by its 16-bit value. Returns None if not found.
pub fn lookup(ddi: u16) -> Option<&'static DdiInfo> {
    match DDI_LIST.binary_search_by_key(&ddi, |info| info.ddi) {
        Ok(idx) => Some(&DDI_LIST[idx]),
        Err(_) => None,
    }
}

/// Convert raw i32 value to physical value using the DDI's resolution and offset.
/// Formula: physical = (raw as f64) * resolution + offset as f64
pub fn to_physical(ddi: u16, raw: i32) -> Option<f64> {
    let info = lookup(ddi)?;
    Some((raw as f64) * info.resolution + info.offset as f64)
}
```

#### NAME Lookup (`name.rs`)

```rust
// Simple lookups (single argument, bit-packing not needed)
pub fn manufacturer_id_lookup(id: u8) -> Option<&'static str>;
pub fn industry_group_lookup(id: u8) -> Option<&'static str>;
pub fn global_function_lookup(func_id: u16) -> Option<&'static str>;

// Bit-packed multi-argument lookups
/// Key = (ig << 24) | (vs << 16) | func_id
pub fn ig_specific_function_lookup(ig: u8, vs: u8, func_id: u16) -> Option<&'static str> {
    let key = ((ig as u32) << 24) | ((vs as u32) << 16) | (func_id as u32);
    match IG_SPECIFIC_FUNCTION_LIST.binary_search_by_key(&key, |(value, _)| *value) {
        Ok(idx) => Some(IG_SPECIFIC_FUNCTION_LIST[idx].1),
        Err(_) => None,
    }
}

/// Key = (ig << 16) | vs
pub fn vehicle_system_lookup(ig: u8, vs: u8) -> Option<&'static str> {
    let key = ((ig as u32) << 16) | (vs as u32);
    match VEHICLE_SYSTEM_LIST.binary_search_by_key(&key, |(value, _)| *value) {
        Ok(idx) => Some(VEHICLE_SYSTEM_LIST[idx].1),
        Err(_) => None,
    }
}
```

### Generator Functions (`generator/src/`)

#### CLI Entry Point (`main.rs`)

```rust
struct CliArgs {
    command: String,           // "generate" or "info"
    modules: Vec<String>,      // ["pgn", "isobus_params", ...]
    pgn_file: Option<PathBuf>,
    params_file: Option<PathBuf>,
    ddi_file: Option<PathBuf>,
    output_dir: Option<PathBuf>,
}

fn main() {
    let cli = parse_args();
    match cli.command.as_str() {
        "generate" => cmd_generate(&cli),
        "info" => cmd_info(&cli),
        _ => /* error */,
    }
}
```

#### Download & Extract (`excel.rs`)

```rust
struct Source {
    name: &'static str,
    url: &'static str,
    output_name: &'static str,
}

fn download_file(url: &str, path: &PathBuf) -> Result<(), Box<dyn Error>>;
fn unzip(zip_path: &PathBuf, extract_dir: &PathBuf);
```

#### Parsers (`parsers/*.rs`)

```rust
// pgn_parser.rs
pub fn parse(file_path: &str) -> Vec<(u32, String)>;  // Returns deduplicated entries

// isobus_params_parser.rs
pub fn parse(file_path: &str) -> Vec<(u32, String)>;

// task_controller_ddi_parser.rs
pub struct DdiEntry { pub ddi_number: u16, pub definition: String, ... };
pub fn parse(file_path: &str) -> Vec<DdiEntry>;

// name_parsers.rs
pub fn parse_manufacturer_ids(path: &str) -> Vec<(u8, String)>;
pub fn parse_industry_groups(path: &str) -> Vec<(u8, String)>;
pub fn parse_global_functions(path: &str) -> Vec<(u16, String)>;

// name_parsers_ig.rs
pub fn parse_ig_specific_functions(path: &str) -> Vec<(...)>;
pub fn parse_vehicle_systems(path: &str) -> Vec<(...)>;
```

#### Codegen (`codegen.rs`)

```rust
pub fn generate_pgn(entries: &[(u32, String)], source_name: &str, date: &str) -> String;
pub fn generate_isobus_params(entries: &[(u32, String)], source_name: &str, date: &str) -> String;
pub fn generate_task_controller_ddi(entries: &[DdiEntry], source_name: &str, date: &str) -> String;
pub fn generate_name(
    manufacturer_ids: &[(u8, String)],
    industry_groups: &[(u8, String)],
    global_functions: &[(u16, String)],
    ig_specific_functions: &[...],
    vehicle_systems: &[...],
    source_name: &str,
    date: &str,
) -> String;

pub fn write_output(output_dir: &PathBuf, module_name: &str, content: &str) -> Result<(), Box<dyn Error>>;
```

## 4. Data Types

### Generated Library Types

#### PGN Entry (`pgn.rs`)

Static tuple `(u32, &'static str)` — PGN value + human-readable name. No heap allocation.

#### ISOBUS Param Entry (`isobus_params.rs`)

Static tuple `(u32, &'static str)` — parameter value + meaning string.

#### DDI Info (`task_controller_ddi.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DdiInfo {
    pub ddi: u16,           // 16-bit DDI identifier (0-65534)
    pub name: &'static str, // Human-readable definition/name
    pub unit: Option<&'static str>, // Unit of measurement (None if not specified)
    pub resolution: f64,    // Scale factor for physical value conversion
    pub offset: i32,        // Offset for physical value conversion
}
```

**Design notes:**
- `resolution` is `f64` to handle fractional scales (e.g., 0.01 for mm³/m²).
- `offset` is `i32` since most DDI values are signed integers.
- All fields are `Copy` — no heap allocation, fully compatible with `no_std`.

#### NAME Lookup Entries (`name.rs`)

Simple tuples `(u8/u16/u32, &'static str)` for single-argument lookups.
Bit-packed keys for multi-argument lookups (see Function Signatures section).

### Generator Data Types

#### PGN Entry (`data.rs` / `parsers/pgn_parser.rs`)

```rust
struct PgnEntry {
    pgn_value: u32,
    name: String,
}
```

#### DDI Entry (`data.rs` / `parsers/task_controller_ddi_parser.rs`)

```rust
pub struct DdiEntry {
    pub ddi_number: u16,
    pub definition: String,
    pub unit: Option<String>,
    pub resolution: f64,
}
```

## 5. Generator Pipeline Stages

### Stage 1: Download & Extract

Consumes source URLs from isobus.net:
- **PGN data**: Downloads `isoExport_xlsx.zip`, extracts to `downloads/extract/`. Finds `"SPNs and PGNs.xlsx"` inside.
- **ISOBUS params**: Same ZIP, finds `AEF Functionalities.xlsx` (simple value→meaning mapping).
- **Task Controller DDI**: Downloads TXT file directly from `completeTXT` endpoint.
- **NAME lookups**: Extracted Excel files from same ZIP (`Manufacturer IDs.xlsx`, etc.).

Handles ZIP extraction automatically. Skips download if file already exists locally.

### Stage 2: Parse & Validate

Consumes source files, produces validated entry lists:
- **PGN parser**: Extracts PGN value (column A) + name (column B). Deduplicates by PGN value. Validates 24-bit range (< 0x1FFFFF).
- **ISOBUS params parser**: Extracts parameter value + meaning from `AEF Functionalities.xlsx`. Deduplicates.
- **DDI parser**: Parses "DD Entity:" blocks from TXT format. Extracts DDI number, Definition, Unit, Resolution fields. Offset defaults to 0 (source has no Offset field).
- **NAME parsers**: Parse manufacturer IDs, industry groups, global functions, IG-specific functions, and vehicle systems from separate Excel files.

Reports warnings for duplicates/out-of-range entries but continues generation.

### Stage 3: Generate & Write

Consumes validated entries, produces `.rs` source files:
- Generates static slices sorted by lookup key.
- Includes binary search `lookup()` function.
- For DDI module: includes `to_physical()` convenience function.
- Adds header comment with source file name, revision number, and download date.
- Writes to `src/` directory alongside `lib.rs`.

Revision tracking: compares generated content against existing file. Increments revision only when content changes.

## 6. Library Module Organization

The library crate has one module per data domain, each feature-gated:

| Module | Contents | Feature Flag | When to Use |
|--------|----------|-------------|-------------|
| `pgn` | PGN name lookups | `pgn` | Always needed for J1939 work |
| `isobus_params` | ISOBUS parameter NAMEs | `isobus_params` | Working with ISOBUS/ARBI protocols |
| `task_controller_ddi` | DDI metadata + conversion | `task_controller_ddi` | Implementing ISO-11783-10 (Task Controller) |
| `name` | NAME field lookups | `name` | Working with ISOBUS/NAME structure decoding |

### Feature Gating Design

```rust
// lib.rs
#![no_std]

#[cfg(feature = "pgn")]
pub mod pgn;

#[cfg(feature = "isobus_params")]
pub mod isobus_params;

#[cfg(feature = "task_controller_ddi")]
pub mod task_controller_ddi;

#[cfg(feature = "name")]
pub mod name;
```

Consumers select only needed modules to reduce compile time and binary size:

```toml
# Minimal: just PGN lookups
iso11783-data = { version = "0.1", features = ["pgn"] }

# Full: all modules
iso11783-data = { version = "0.1" }
```

## 7. Known Design Decisions

### 7.1 Static Slices + Binary Search

All lookup tables use static slices of tuples/structs sorted by lookup key. Binary search provides O(log n) lookups without heap allocation. This is the core design pattern across all modules.

**Rationale**: `no_std` compatibility, zero runtime dependencies, minimal binary size.

### 7.2 Bit-Packing for Multi-Argument Lookups

NAME module uses bit-packing to combine multiple lookup arguments into a single key:
- IG-specific functions: `(ig << 24) | (vs << 16) | func_id` = 32 bits
- Vehicle systems: `(ig << 16) | vs` = 16 bits

**Rationale**: Avoids nested data structures while keeping lookup efficient (single binary search). `u8` indices are safe — industry groups use 0-5, vehicle systems use 0-255, function IDs use 0-65535.

### 7.3 `to_physical()` in Generated Code

The `to_physical()` convenience function is included directly in `task_controller_ddi.rs` rather than as a separate utility module. It takes signed `i32` since most Task Controller DDI values are 32-bit signed integers.

**Rationale**: Keeps generated code self-contained for consumers who need physical value conversion. Special range handling (error, not available) is left to consumers — they can check raw values against known ranges using conventions in `docs/j1939_data_types.md`.

### 7.4 Revision Tracking Without CI

Revision numbers are tracked locally in `generator/revision.json` and compared against existing file content. Increment only when actual content changes (not on every run). No CI automation yet — generation is manual.

**Rationale**: Simple approach for current development workflow. CI automation is future scope per original requirements.

### 7.5 Generated Files Committed to Git

The `.rs` files generated by the tool are committed to git alongside the generator source code. This allows consumers to use the library without running the generator.

**Rationale**: Simplifies consumer experience — no build-time code generation required. Generator is only needed when updating data from isobus.net.

## 8. Current Test Coverage

| Module | Tests | Status |
|--------|-------|--------|
| PGN tests (`tests/pgn_tests.rs`) | 8 | All passing |
| ISOBUS params tests (`tests/isobus_params_tests.rs`) | 7 | All passing |
| Task Controller DDI tests (`tests/task_controller_ddi_tests.rs`) | 12 | All passing |
| **Total** | **27** | **All passing** |

### Key Test Areas

- Lookup correctness: verify `lookup()` returns correct values for known entries.
- Sorted order validation: ensure binary search works correctly (entries must be sorted).
- List count verification: confirm entry counts match expected values from source files.
- Physical value conversion: test `to_physical()` with various resolution/offset combinations.
- no_std compilation: verified with `--target thumbv7m-none-eabi`.

## 9. Architecture Gaps / Future Work

### 9.1 CI Automation (Future)

No automated regeneration pipeline yet. Generation is manual — developer runs `cargo run -p generator -- generate` when updating data from isobus.net.

**Planned**: GitHub Actions workflow for periodic regeneration and PR creation when data changes.

### 9.2 Data Quality Validation

Current validation checks for duplicates and range errors but doesn't validate semantic correctness (e.g., missing names, empty units). Source files from isobus.net are assumed to be authoritative.

**Planned**: Additional validation rules based on ISO 11783 specification requirements.

### 9.3 Consumer Integration

Phase 3 integration with `can_decoder` is planned but not yet implemented:
- Replace hardcoded PGN constants in `can_decoder/src/task_controller.rs` with `iso11783_data::pgn::lookup()`.
- Enhance Task Controller decoder output to use DDI metadata (name, unit) from `iso11783_data::task_controller_ddi::lookup()`.

### 9.4 crates.io Publication

Library is not yet published to crates.io. Requires:
- README.md with usage examples.
- License file (currently MIT OR Apache-2.0 in Cargo.toml).
- Documentation preparation and MSRV specification.
