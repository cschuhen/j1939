
pub fn render_pgn(pgn: u32) -> String {
    match iso11783_data::strings::pgn::lookup(pgn) {
        Some(pgn_name) => format!("{:06x} {}({})", pgn, pgn_name, pgn),
        None => format!("{:06x} ({})", pgn, pgn),
    }
}

pub fn render_name(name_u64: u64) -> String {
    use iso11783_data::strings::name as Namestrings;

    let name = j1939_async::name::Name(name_u64);
    let mut parts = Vec::new();

    // Manufacturer ID with lookup
    match Namestrings::manufacturer_id_lookup_u16(name.manufacturer() as u16) {
        Some(manufacturer) => {
            if let Some((first, _)) = manufacturer.split_once(' ') {
                parts.push(format!("{}({})", first.trim(), name.manufacturer()));
            } else {
                parts.push(manufacturer.to_string());
            }
        }
        None => parts.push(format!("{}", name.manufacturer())),
    }

    // Industry Group with lookup
    match Namestrings::industry_group_lookup(name.industry_group() as u8) {
        Some(ig) => {
            if let Some((first, _)) = ig.split_once(' ') {
                parts.push(first.to_string());
            } else {
                parts.push(ig.to_string());
            }
        }
        None => parts.push(format!("IG{}", name.industry_group())),
    }

    // Function with lookup (global or IG-specific)
    let func_id = name.function() as u16;
    let ig = name.industry_group();
    let vs = name.vehicle_system();

    if func_id <= 94 {
        match Namestrings::global_function_lookup(func_id) {
            Some(func_name) => parts.push(func_name.to_string()),
            None => parts.push(format!("Func{}", func_id)),
        }
    } else {
        match Namestrings::ig_specific_function_lookup(ig as u8, vs as u8, func_id) {
            Some(func_name) => parts.push(func_name.to_string()),
            None => parts.push(format!("Func{:02X}", func_id)),
        }
    }

    // Vehicle System with lookup
    match Namestrings::vehicle_system_lookup(ig as u8, vs as u8) {
        Some(vs_name) => {
            if let Some((first, _)) = vs_name.split_once(' ') {
                parts.push(first.to_string());
            } else {
                parts.push(vs_name.to_string());
            }
        }
        None => parts.push(format!("VS{}", vs)),
    }

    // ECU Instance (no lookup)
    parts.push(format!("ECU{}", name.ecu_instance()));

    // Function Instance (no lookup)
    parts.push(format!("FI{}", name.function_instance()));

    // Vehicle System Instance (no lookup)
    parts.push(format!("VSI{}", name.vehicle_system_instance()));

    // Identity (no lookup)
    parts.push(format!("#{}", name.identity()));

    // Self Configurable flag
    if name.self_configurable() {
        parts.push("SC".to_string());
    }

    parts.join(" | ")
}
