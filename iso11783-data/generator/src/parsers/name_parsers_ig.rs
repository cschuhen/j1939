use crate::data::{IgSpecificFunctionEntry, VehicleSystemEntry};
use calamine::{open_workbook, Reader, Xlsx};
use std::collections::{HashMap, HashSet};

/// Parse IG Specific NAME Function entries from an Excel file.
/// Returns deduplicated entries sorted by packed key (ig << 24 | vs << 16 | func).
pub fn parse_ig_specific_functions(path: &str) -> Vec<IgSpecificFunctionEntry> {
    let mut xlsx: Xlsx<_> =
        open_workbook(path).expect("failed to open IG Specific NAME Function workbook");

    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        // Use a map to deduplicate: key = (ig, vs, func_id) -> description
        let mut entries_map: HashMap<(u8, u8, u16), String> = HashMap::new();

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let ig_cell = &row.get(0);
            let vs_cell = &row.get(1);
            let func_cell = &row.get(3); // column D is function_id (0-indexed: A=0, B=1, C=2, D=3)
            let desc_cell = &row.get(4); // column E is function_description

            let ig: u8 = match ig_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u8,
                    calamine::DataType::Int(i) => *i as u8,
                    _ => continue,
                },
                None => continue,
            };

            let vs: u8 = match vs_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u8,
                    calamine::DataType::Int(i) => *i as u8,
                    _ => continue,
                },
                None => continue,
            };

            let func_id: u16 = match func_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u16,
                    calamine::DataType::Int(i) => *i as u16,
                    _ => continue,
                },
                None => continue,
            };

            let desc = match desc_cell {
                Some(c) => match c {
                    calamine::DataType::String(s) => s.clone(),
                    _ => continue,
                },
                None => continue,
            };

            entries_map.insert((ig, vs, func_id), desc);
        }

        let mut entries: Vec<_> = entries_map
            .into_iter()
            .map(|((ig, vs, func_id), desc)| IgSpecificFunctionEntry {
                industry_group_id: ig,
                vehicle_system_id: vs,
                function_id: func_id,
                description: desc,
            })
            .collect();

        // Sort by packed key for binary search
        entries.sort_by_key(|e| {
            ((e.industry_group_id as u32) << 24)
                | ((e.vehicle_system_id as u32) << 16)
                | (e.function_id as u32)
        });

        return entries;
    }

    Vec::new()
}

/// Parse Vehicle System entries directly from the IG Specific NAME Function Excel file.
/// This extracts column C (vehicle_system_description) for unique (ig, vs) pairs.
pub fn parse_vehicle_systems(path: &str) -> Vec<VehicleSystemEntry> {
    let mut xlsx: Xlsx<_> =
        open_workbook(path).expect("failed to open IG Specific NAME Function workbook");

    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        let mut seen: HashSet<(u8, u8)> = HashSet::new();
        let mut entries_map: HashMap<(u8, u8), String> = HashMap::new();

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let ig_cell = &row.get(0);
            let vs_cell = &row.get(1);
            let desc_cell = &row.get(2); // column C is vehicle_system_description

            let ig: u8 = match ig_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u8,
                    calamine::DataType::Int(i) => *i as u8,
                    _ => continue,
                },
                None => continue,
            };

            let vs: u8 = match vs_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u8,
                    calamine::DataType::Int(i) => *i as u8,
                    _ => continue,
                },
                None => continue,
            };

            let desc = match desc_cell {
                Some(c) => match c {
                    calamine::DataType::String(s) if !s.is_empty() => s.clone(),
                    _ => continue,
                },
                None => continue,
            };

            if seen.insert((ig, vs)) {
                entries_map.insert((ig, vs), desc);
            }
        }

        let mut entries: Vec<_> = entries_map
            .into_iter()
            .map(|((ig, vs), desc)| VehicleSystemEntry {
                industry_group_id: ig,
                vehicle_system_id: vs,
                description: desc,
            })
            .collect();

        entries
            .sort_by_key(|e| ((e.industry_group_id as u32) << 16) | (e.vehicle_system_id as u32));
        return entries;
    }

    Vec::new()
}
