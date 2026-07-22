use crate::data::{DdiEntry, IndustryGroupEntry, ManufacturerIdEntry, ParamNameEntry, PgnEntry};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Clean up Excel encoding artifacts and convert to valid Rust identifier.
fn name_to_constant(name: &str) -> String {
    let cleaned = regex_replace_excel_escapes(name);
    let cleaned = cleaned.replace('\n', " ").replace('\t', " ");

    let mut result = String::new();
    let mut last_was_separator = false;

    for ch in cleaned.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_uppercase());
            last_was_separator = false;
        } else {
            if !last_was_separator && !result.is_empty() {
                result.push('_');
                last_was_separator = true;
            }
        }
    }

    while result.ends_with('_') {
        result.pop();
    }
    let collapsed: String = result.chars().fold(String::new(), |mut acc, c| {
        if !(c == '_' && acc.ends_with('_')) {
            acc.push(c);
        }
        acc
    });

    let truncated: String = collapsed.chars().take(63).collect();

    if truncated
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_digit())
    {
        return format!("CONST_{}", truncated);
    }

    truncated
}

/// Convert a name to a valid Rust module identifier (snake_case).
pub(crate) fn name_to_module(name: &str) -> String {
    let cleaned = regex_replace_excel_escapes(name);
    let cleaned = cleaned.replace('\n', " ").replace('\t', " ");

    let mut result = String::new();
    let mut last_was_separator = false;

    for ch in cleaned.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else {
            if !last_was_separator && !result.is_empty() {
                result.push('_');
                last_was_separator = true;
            }
        }
    }

    while result.ends_with('_') {
        result.pop();
    }
    let collapsed: String = result.chars().fold(String::new(), |mut acc, c| {
        if !(c == '_' && acc.ends_with('_')) {
            acc.push(c);
        }
        acc
    });

    let truncated: String = collapsed.chars().take(63).collect();

    if truncated
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_digit())
    {
        return format!("mod_{}", truncated);
    }

    truncated
}

/// Generate a unique constant name, adding a numeric suffix for duplicates.
fn unique_constant_name(name: &str, seen: &mut HashSet<String>) -> String {
    let base = name_to_constant(name);
    if seen.insert(base.clone()) {
        base
    } else {
        let mut i = 2;
        loop {
            let candidate = format!("{}_{}", base, i);
            if seen.insert(candidate.clone()) {
                return candidate;
            }
            i += 1;
        }
    }
}

fn regex_replace_excel_escapes(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 6 < len && chars[i] == '_' && chars[i + 1] == 'x' {
            let hex: String = chars[i + 2..i + 6].iter().collect();
            if hex.chars().all(|c| c.is_ascii_hexdigit()) {
                i += 7;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

// ============ PGN ============

pub fn generate_pgn_strings(entries: &[PgnEntry], source_file: &str, date: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));
    out.push_str("pub const PGN_LIST: &[(u32, &str)] = &[\n");

    for entry in entries {
        let name_escaped = entry.name.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("    ({}, \"{}\"),\n", entry.pgn, name_escaped));
    }

    out.push_str("];\n\n");
    out.push_str("/// Lookup PGN name by numeric value.\n");
    out.push_str("pub fn lookup(pgn: u32) -> Option<&'static str> {\n");
    out.push_str("    match PGN_LIST.binary_search_by_key(&pgn, |(value, _)| *value) {\n");
    out.push_str("        Ok(idx) => Some(PGN_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n");

    out
}

pub fn generate_pgn_constants(entries: &[PgnEntry], source_file: &str, date: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    let mut seen_names = HashSet::new();
    for entry in entries {
        let const_name = name_to_constant(&entry.name);
        if seen_names.insert(const_name.clone()) {
            out.push_str(&format!(
                "pub const {}: u32 = 0x{:06X}; // {}\n",
                const_name, entry.pgn, entry.pgn
            ));
        }
    }

    out
}

// ============ ISOBUS PARAMS ============

pub fn generate_isobus_params_strings(
    entries: &[ParamNameEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));
    out.push_str("pub const PARAM_NAME_LIST: &[(u32, &str)] = &[\n");

    for entry in entries {
        let name_escaped = entry.name.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("    ({}, \"{}\"),\n", entry.value, name_escaped));
    }

    out.push_str("];\n\n");
    out.push_str("/// Lookup parameter NAME by numeric value.\n");
    out.push_str("pub fn lookup(value: u32) -> Option<&'static str> {\n");
    out.push_str("    match PARAM_NAME_LIST.binary_search_by_key(&value, |(v, _)| *v) {\n");
    out.push_str("        Ok(idx) => Some(PARAM_NAME_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n");

    out
}

pub fn generate_isobus_params_constants(
    _entries: &[ParamNameEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));
    out.push_str("// No named constants for ISOBUS parameter names\n");
    out
}

// ============ TASK CONTROLLER DDI ============

pub fn generate_task_controller_ddi_strings(
    entries: &[DdiEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq)]\npub struct DdiInfo {\n");
    out.push_str("    pub ddi: u16,\n");
    out.push_str("    pub name: &'static str,\n");
    out.push_str("    /// e.g., \"RPM\", \"degrees Celsius\", \"kPa\" — None if not specified\n");
    out.push_str("    pub unit: Option<&'static str>,\n");
    out.push_str(
        "    /// Resolution (scale factor) for physical value conversion: physical = (raw - offset) * scale\n",
    );
    out.push_str("    pub resolution: f64,\n");
    out.push_str("    /// Offset for physical value conversion\n");
    out.push_str("    pub offset: i32,\n");
    out.push_str("}\n\n");

    out.push_str("pub const DDI_LIST: &[DdiInfo] = &[\n");

    for entry in entries {
        let name_escaped = entry.name.replace('\\', "\\\\").replace('"', "\\\"");
        let unit_field = match &entry.unit {
            Some(u) => {
                let u_escaped = u.replace('\\', "\\\\").replace('"', "\\\"");
                format!("Some(\"{}\")", u_escaped)
            }
            None => "None".to_string(),
        };
        let res_str = format_float(entry.resolution);
        let off_str = entry.offset.to_string();
        out.push_str(&format!(
            "    DdiInfo {{ ddi: {}, name: \"{}\", unit: {}, resolution: {}, offset: {} }},\n",
            entry.ddi, name_escaped, unit_field, res_str, off_str
        ));
    }

    out.push_str("];\n\n");

    out.push_str("/// Lookup DDI info by DDI value.\n");
    out.push_str("pub fn lookup(ddi: u16) -> Option<&'static DdiInfo> {\n");
    out.push_str("    match DDI_LIST.binary_search_by_key(&ddi, |info| info.ddi) {\n");
    out.push_str("        Ok(idx) => Some(&DDI_LIST[idx]),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n\n");

    out.push_str(
        "/// Convert raw i32 value to physical value using the DDI's resolution and offset.\n",
    );
    out.push_str("pub fn to_physical(ddi: u16, raw: i32) -> Option<f64> {\n");
    out.push_str("    let info = lookup(ddi)?;\n");
    out.push_str("    Some((raw as f64) * info.resolution + info.offset as f64)\n");
    out.push_str("}\n");

    out
}

pub fn generate_task_controller_ddi_constants(
    entries: &[DdiEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    let mut seen_names = HashSet::new();
    for entry in entries {
        let const_name = name_to_constant(&entry.name);
        if seen_names.insert(const_name.clone()) {
            out.push_str(&format!("pub const {}: u16 = {};\n", const_name, entry.ddi));
        }
    }

    out
}

// ============ NAME ============

pub fn generate_name_strings(
    manufacturer_ids: &[ManufacturerIdEntry],
    industry_groups: &[crate::data::IndustryGroupEntry],
    global_functions: &[crate::data::GlobalFunctionEntry],
    ig_specific_functions: &[crate::data::IgSpecificFunctionEntry],
    vehicle_systems: &[crate::data::VehicleSystemEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    // Manufacturer IDs
    out.push_str("pub const MANUFACTURER_ID_LIST: &[(u8, &str)] = &[\n");
    for entry in manufacturer_ids {
        let name_escaped = entry.name.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("    ({}, \"{}\"),\n", entry.id, name_escaped));
    }
    out.push_str("];\n\n");
    out.push_str("/// Lookup manufacturer ID by numeric value.\n");
    out.push_str("pub fn manufacturer_id_lookup(id: u8) -> Option<&'static str> {\n");
    out.push_str(
        "    match MANUFACTURER_ID_LIST.binary_search_by_key(&id, |(value, _)| *value) {\n",
    );
    out.push_str("        Ok(idx) => Some(MANUFACTURER_ID_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n\n");

    // Industry Groups
    out.push_str("pub const INDUSTRY_GROUP_LIST: &[(u8, &str)] = &[\n");
    for entry in industry_groups {
        let desc_escaped = entry.description.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("    ({}, \"{}\"),\n", entry.id, desc_escaped));
    }
    out.push_str("];\n\n");
    out.push_str("/// Lookup industry group by numeric value.\n");
    out.push_str("pub fn industry_group_lookup(id: u8) -> Option<&'static str> {\n");
    out.push_str(
        "    match INDUSTRY_GROUP_LIST.binary_search_by_key(&id, |(value, _)| *value) {\n",
    );
    out.push_str("        Ok(idx) => Some(INDUSTRY_GROUP_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n\n");

    // Global NAME Functions
    out.push_str("pub const GLOBAL_FUNCTION_LIST: &[(u16, &str)] = &[\n");
    for entry in global_functions {
        let desc_escaped = entry.description.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("    ({}, \"{}\"),\n", entry.id, desc_escaped));
    }
    out.push_str("];\n\n");
    out.push_str("/// Lookup global NAME function by numeric value.\n");
    out.push_str("pub fn global_function_lookup(func_id: u16) -> Option<&'static str> {\n");
    out.push_str(
        "    match GLOBAL_FUNCTION_LIST.binary_search_by_key(&func_id, |(value, _)| *value) {\n",
    );
    out.push_str("        Ok(idx) => Some(GLOBAL_FUNCTION_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n\n");

    // IG Specific NAME Functions
    out.push_str("/// IG-specific NAME function lookup.\n");
    out.push_str(
        "/// Key is packed as: ((ig as u32) << 24) | ((vs as u32) << 16) | (func_id as u32)\n",
    );
    out.push_str("pub const IG_SPECIFIC_FUNCTION_LIST: &[(u32, &str)] = &[\n");
    for entry in ig_specific_functions {
        let desc_escaped = entry.description.replace('\\', "\\\\").replace('"', "\\\"");
        let key = ((entry.industry_group_id as u32) << 24)
            | ((entry.vehicle_system_id as u32) << 16)
            | (entry.function_id as u32);
        out.push_str(&format!("    ({}, \"{}\"),\n", key, desc_escaped));
    }
    out.push_str("];\n\n");
    out.push_str("/// Lookup IG-specific NAME function by industry group, vehicle system, and function ID.\n");
    out.push_str("pub fn ig_specific_function_lookup(ig: u8, vs: u8, func_id: u16) -> Option<&'static str> {\n");
    out.push_str("    let key = ((ig as u32) << 24) | ((vs as u32) << 16) | (func_id as u32);\n");
    out.push_str(
        "    match IG_SPECIFIC_FUNCTION_LIST.binary_search_by_key(&key, |(value, _)| *value) {\n",
    );
    out.push_str("        Ok(idx) => Some(IG_SPECIFIC_FUNCTION_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n\n");

    // Vehicle Systems
    out.push_str("/// Vehicle system lookup.\n");
    out.push_str("/// Key is packed as: ((ig as u32) << 16) | (vs as u32)\n");
    out.push_str("pub const VEHICLE_SYSTEM_LIST: &[(u32, &str)] = &[\n");
    for entry in vehicle_systems {
        let desc_escaped = entry.description.replace('\\', "\\\\").replace('"', "\\\"");
        let key = ((entry.industry_group_id as u32) << 16) | (entry.vehicle_system_id as u32);
        out.push_str(&format!("    ({}, \"{}\"),\n", key, desc_escaped));
    }
    out.push_str("];\n\n");
    out.push_str("/// Lookup vehicle system by industry group and vehicle system ID.\n");
    out.push_str("pub fn vehicle_system_lookup(ig: u8, vs: u8) -> Option<&'static str> {\n");
    out.push_str("    let key = ((ig as u32) << 16) | (vs as u32);\n");
    out.push_str(
        "    match VEHICLE_SYSTEM_LIST.binary_search_by_key(&key, |(value, _)| *value) {\n",
    );
    out.push_str("        Ok(idx) => Some(VEHICLE_SYSTEM_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n");

    out
}

/// Generate the root name/mod.rs file.
pub fn generate_name_mod(source_file: &str, date: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));
    out.push_str("pub mod manufacturer_ids;\npub mod global;\npub mod industry_groups;\n");
    out
}

/// Generate name/manufacturer_ids.rs with manufacturer ID constants.
pub fn generate_name_manufacturer_ids(
    manufacturer_ids: &[ManufacturerIdEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    for entry in manufacturer_ids {
        let const_name = name_to_constant(&entry.name);
        out.push_str(&format!("pub const {}: u8 = {};\n", const_name, entry.id));
    }

    out
}

/// Generate name/global.rs with IG-specific function constants for Global (IG 0) using inline modules.
pub fn generate_global_module(
    ig_functions: &[crate::data::IgSpecificFunctionEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    // Group functions by vehicle system description (only those with entries)
    let mut vs_functions: HashMap<String, Vec<crate::data::IgSpecificFunctionEntry>> =
        HashMap::new();
    for entry in ig_functions {
        if entry.industry_group_id == 0 && !entry.description.is_empty() {
            let key = name_to_module(&entry.description);
            vs_functions.entry(key).or_default().push(entry.clone());
        }
    }

    // Generate inline modules for each vehicle system (only those with at least one function entry)
    for (mod_name, functions) in &vs_functions {
        out.push_str(&format!(
            "/// Vehicle system: {}\n",
            mod_name.replace('_', " ")
        ));
        out.push_str("pub mod ");
        out.push_str(mod_name);
        out.push_str(" {\n");

        let mut seen_names = HashSet::new();
        for entry in functions {
            let const_name = unique_constant_name(&entry.description, &mut seen_names);
            // Use the function_id value from the spreadsheet (column D)
            out.push_str(&format!(
                "    pub const {}: u32 = {};\n",
                const_name, entry.function_id
            ));
        }

        out.push_str("}\n\n");
    }

    out
}

/// Generate a single industry group .rs file with inline modules for each vehicle system.
pub fn generate_industry_group_file(
    ig_entry: &IndustryGroupEntry,
    vs_vechicle_groups: &[crate::data::VehicleSystemEntry],
    vs_functions: &[crate::data::IgSpecificFunctionEntry],
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    let ig_id = ig_entry.id;

    // Collect unique vehicle system descriptions and their IDs for this industry group
    let mut vs_map: HashMap<String, u8> = HashMap::new();
    for entry in vs_vechicle_groups {
        if entry.industry_group_id == ig_id && !entry.description.is_empty() {
            let key = name_to_module(&entry.description);
            vs_map.entry(key).or_insert(entry.vehicle_system_id);
        }
    }
    let mut vs_sorted: Vec<(&String, &u8)> = vs_map.iter().collect();
    vs_sorted.sort_by(|a, b| a.1.cmp(b.1));
    // Write direct constants mapping vehicle system description to its numeric ID (column C)
    for (vs_name, vs_id) in &vs_sorted {
        out.push_str(&format!(
            "pub const {}: u32 = {};\n",
            vs_name.to_uppercase(),
            vs_id
        ));
    }

    out.push_str(&format!("\n\n\n"));

    for (vs_name, vs_id) in &vs_sorted {
        out.push_str(&format!(
            "/// Vehicle system: {}\n",
            vs_name.replace('_', " ")
        ));
        out.push_str("pub mod ");
        out.push_str(vs_name);
        out.push_str(" {\n");

        // Collect unique vehicle system descriptions and their IDs for this industry group
        let mut fun_map: HashMap<String, u16> = HashMap::new();
        for entry in vs_functions {
            if entry.industry_group_id == ig_id
                && entry.vehicle_system_id == **vs_id
                && !entry.description.is_empty()
            {
                let key = name_to_module(&entry.description);
                fun_map.entry(key).or_insert(entry.function_id);
            }
        }

        let mut functions_sorted: Vec<(&String, &u16)> = fun_map.iter().collect();
        functions_sorted.sort_by(|a, b| a.1.cmp(b.1));
        // Write direct constants mapping vehicle system description to its numeric ID (column C)
        for (fun_name, fun_id) in &functions_sorted {
            out.push_str(&format!(
                "    pub const {}: u16 = {};\n",
                fun_name.to_uppercase(),
                fun_id
            ));
        }

        out.push_str("}\n\n");
    }

    out
}

/// Generate name/industry_groups/mod.rs with industry group constants, submodule declarations.
pub fn generate_name_industry_groups_mod(
    industry_groups: &[IndustryGroupEntry],
    vs_with_entries: &HashSet<(u8, u8)>,
    source_file: &str,
    date: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));

    // Industry group constants (only those with entries in the data and IG > 0)
    for entry in industry_groups {
        if entry.id == 0 {
            continue;
        }

        let has_entries = vs_with_entries.iter().any(|(ig, _)| *ig == entry.id);
        if !has_entries {
            continue;
        }

        out.push_str(&format!(
            "pub const {}: u8 = {};\n",
            unique_constant_name(&entry.description, &mut HashSet::new()),
            entry.id
        ));
    }

    // Declare one submodule per industry group (only those with entries in the data and IG > 0)
    let mut seen_mod_names = HashSet::new();
    for entry in industry_groups {
        if entry.id == 0 {
            continue;
        }

        // Skip groups that have no function entries
        let has_entries = vs_with_entries.iter().any(|(ig, _)| *ig == entry.id);
        if !has_entries {
            continue;
        }

        let mod_name = name_to_module(&entry.description);

        // Deduplicate reserved groups (IG6 and IG7 have the same description)
        if !seen_mod_names.insert(mod_name.clone()) {
            out.push_str(&format!("pub mod {}_{};\n", mod_name, entry.id));
        } else {
            out.push_str(&format!("pub mod {};\n", mod_name));
        }
    }

    // Re-export all constants from each IG module so consumers can access them directly
    //let mut seen_mod_names = HashSet::new();
    for entry in industry_groups {
        if entry.id == 0 {
            continue;
        }

        // Skip groups that have no function entries
        let has_entries = vs_with_entries.iter().any(|(ig, _)| *ig == entry.id);
        if !has_entries {
            continue;
        }

        //let mod_name = name_to_module(&entry.description);

        // Deduplicate reserved groups (IG6 and IG7 have the same description)
        //if !seen_mod_names.insert(mod_name.clone()) {
        //     out.push_str(&format!("pub use {}_{}::*;\n", mod_name, entry.id));
        // } else {
        //     out.push_str(&format!("pub use {}::*;\n", mod_name));
        //}
    }

    out
}

/// Generate name/industry_groups/<vehicle_system_mod>.rs with IG-specific function constants for that vehicle system.
// ============ UTILITIES ============

/// Write generated source file to the output directory.
pub fn write_output(output_dir: &Path, module_name: &str, content: &str) -> std::io::Result<()> {
    let path = output_dir.join(format!("{}.rs", module_name));
    fs::write(&path, content)
}

/// Write generated source file to a subdirectory within the output directory.
pub fn write_output_subdir(
    output_dir: &Path,
    subdir: &str,
    filename: &str,
    content: &str,
) -> std::io::Result<()> {
    let dir = output_dir.join(subdir);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.rs", filename));
    fs::write(&path, content)
}

/// Write generated mod.rs file to a subdirectory within the output directory.
pub fn write_mod_subdir(output_dir: &Path, subdir: &str, content: &str) -> std::io::Result<()> {
    let dir = output_dir.join(subdir);
    fs::create_dir_all(&dir)?;
    let path = dir.join("mod.rs");
    fs::write(&path, content)
}

fn format_float(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        format!("{:.1}", v)
    } else {
        let s = format!("{}", v);
        if !s.contains('.') {
            format!("{}.0", s)
        } else {
            s
        }
    }
}
