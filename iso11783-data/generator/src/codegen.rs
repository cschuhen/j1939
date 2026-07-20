use crate::data::{DdiEntry, ParamNameEntry, PgnEntry};
use std::fs;
use std::path::Path;

/// Generate pgn.rs source file from parsed PGN entries.
pub fn generate_pgn(entries: &[PgnEntry], source_file: &str, date: &str) -> String {
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
    out.push_str(
        "    match PGN_LIST.binary_search_by_key(&pgn, |(value, _)| *value) {\n",
    );
    out.push_str("        Ok(idx) => Some(PGN_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n");

    // Generate named constants for well-known PGNs
    let well_known = [
        ("PROCESS_DATA", 0x00cb00),
        ("BROADCAST_COMMAND", 0x00cf00),
        ("REQUEST", 0x00e800),
        ("TRANSMIT_COMPRESSED_TIME_DATA", 0x00ea00),
        ("TRANSMIT_DECOMPRESSED_TIME_DATA", 0x00eb00),
        ("COMMAND", 0x00ec00),
        ("DYNAMIC_ADDRESS_ASSIGNMENT", 0x00f000),
    ];

    out.push_str("\n/// Named constants for frequently-used PGNs.\n");
    for (name, value) in &well_known {
        if entries.iter().any(|e| e.pgn == *value) {
            out.push_str(&format!("pub const {}: u32 = 0x{:06x}; // {}\n", name, value, value));
        }
    }

    out
}

/// Generate isobus_params.rs source file from parsed parameter NAME entries.
pub fn generate_isobus_params(entries: &[ParamNameEntry], source_file: &str, date: &str) -> String {
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
    out.push_str(
        "    match PARAM_NAME_LIST.binary_search_by_key(&value, |(v, _)| *v) {\n",
    );
    out.push_str("        Ok(idx) => Some(PARAM_NAME_LIST[idx].1),\n");
    out.push_str("        Err(_) => None,\n");
    out.push_str("    }\n}\n");

    out
}

/// Generate task_controller_ddi.rs source file from parsed DDI entries.
pub fn generate_task_controller_ddi(entries: &[DdiEntry], source_file: &str, date: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "/// Source: {} rev {}, downloaded {}\n",
        source_file, 1, date
    ));
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq)]\npub struct DdiInfo {\n");
    out.push_str("    pub ddi: u16,\n");
    out.push_str("    pub name: &'static str,\n");
    out.push_str(
        "    /// e.g., \"RPM\", \"degrees Celsius\", \"kPa\" — None if not specified\n",
    );
    out.push_str("    pub unit: Option<&'static str>,\n");
    out.push_str(
        "    /// Resolution (factor) for physical value conversion: physical = raw * resolution + offset\n",
    );
    out.push_str("    pub resolution: f64,\n");
    out.push_str("    /// Offset for physical value conversion\n");
    out.push_str("    pub offset: f64,\n");
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
        let off_str = format_float(entry.offset);
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
    out.push_str("    Some((raw as f64) * info.resolution + info.offset)\n");
    out.push_str("}\n");

    out
}

/// Write generated source file to the output directory.
pub fn write_output(output_dir: &Path, module_name: &str, content: &str) -> std::io::Result<()> {
    let path = output_dir.join(format!("{}.rs", module_name));
    fs::write(&path, content)
}

fn format_float(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        format!("{:.1}", v)
    } else {
        // Remove trailing zeros but keep at least one decimal place
        let s = format!("{}", v);
        if !s.contains('.') {
            format!("{}.0", s)
        } else {
            s
        }
    }
}


/// Generate name.rs source file from parsed NAME entries.
pub fn generate_name(
    manufacturer_ids: &[crate::data::ManufacturerIdEntry],
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

    // IG Specific NAME Functions (bit-packed key: ig << 24 | vs << 16 | func)
    out.push_str("/// IG-specific NAME function lookup.\n");
    out.push_str("/// Key is packed as: ((ig as u32) << 24) | ((vs as u32) << 16) | (func_id as u32)\n");
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

    // Vehicle Systems (bit-packed key: ig << 16 | vs)
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
