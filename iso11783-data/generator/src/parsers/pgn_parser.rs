use crate::data::PgnEntry;
use calamine::{Reader, Xlsx, open_workbook};
use std::collections::HashMap;

/// Parse PGN entries from an Excel file (SPNs and PGNs.xlsx format).
/// Returns deduplicated, sorted list of PGN entries.
pub fn parse(path: &str) -> Vec<PgnEntry> {
    let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open PGN workbook");

    // Use first sheet
    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        eprintln!("WARNING: No sheets found in PGN file");
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        let mut pgn_to_name: HashMap<u32, String> = HashMap::new();
        let mut warnings = 0;
        const MAX_24BIT: u32 = 0x00FFFFFF;

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let pgn_cell = &row.get(0);
            let name_cell = &row.get(1);

            let pgn_value: u32 = match pgn_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u32,
                    calamine::DataType::Int(i) => *i as u32,
                    _ => continue,
                },
                None => continue,
            };

            if pgn_value > MAX_24BIT {
                eprintln!("WARNING: PGN {} exceeds 24-bit unsigned max ({}). Row {}", pgn_value, MAX_24BIT, row_idx);
                warnings += 1;
            }

            let name = match name_cell {
                Some(c) => match c {
                    calamine::DataType::String(s) => s.clone(),
                    _ => continue,
                },
                None => continue,
            };

            if let Some(existing_name) = pgn_to_name.get(&pgn_value) {
                if existing_name != &name {
                    eprintln!("WARNING: PGN {} has conflicting names:", pgn_value);
                    eprintln!("  Previously: {}", existing_name);
                    eprintln!("  Now found:  {}", name);
                    warnings += 1;
                }
            } else {
                pgn_to_name.insert(pgn_value, name.clone());
            }
        }

        if warnings > 0 {
            eprintln!("Total PGN warnings: {}\n", warnings);
        }

        let mut sorted: Vec<_> = pgn_to_name.into_iter().map(|(pgn, name)| PgnEntry { pgn, name }).collect();
        sorted.sort_by_key(|e| e.pgn);
        return sorted;
    }

    Vec::new()
}
