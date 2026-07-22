use crate::data::ParamNameEntry;
use calamine::{open_workbook, Reader, Xlsx};
use std::collections::HashMap;

/// Parse ISOBUS parameter NAME entries from an Excel file with "value" and "meaning" columns.
/// Returns deduplicated, sorted list of parameter name entries.
pub fn parse(path: &str) -> Vec<ParamNameEntry> {
    let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open ISOBUS params workbook");

    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        eprintln!("WARNING: No sheets found in ISOBUS params file");
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        let mut value_to_name: HashMap<u32, String> = HashMap::new();
        let mut warnings = 0;

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let value_cell = &row.get(0);
            let name_cell = &row.get(1);

            let value: u32 = match value_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u32,
                    calamine::DataType::Int(i) => *i as u32,
                    _ => continue,
                },
                None => continue,
            };

            let name = match name_cell {
                Some(c) => match c {
                    calamine::DataType::String(s) => s.clone(),
                    _ => continue,
                },
                None => continue,
            };

            if let Some(existing_name) = value_to_name.get(&value) {
                if existing_name != &name {
                    eprintln!("WARNING: Value {} has conflicting names:", value);
                    eprintln!("  Previously: {}", existing_name);
                    eprintln!("  Now found:  {}", name);
                    warnings += 1;
                }
            } else {
                value_to_name.insert(value, name.clone());
            }
        }

        if warnings > 0 {
            eprintln!("Total ISOBUS params warnings: {}\n", warnings);
        }

        let mut sorted: Vec<_> = value_to_name
            .into_iter()
            .map(|(value, name)| ParamNameEntry { value, name })
            .collect();
        sorted.sort_by_key(|e| e.value);
        return sorted;
    }

    Vec::new()
}
