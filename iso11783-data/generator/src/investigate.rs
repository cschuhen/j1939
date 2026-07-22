use calamine::{open_workbook, Reader, Xlsx};
use std::collections::HashMap;

fn main() {
    let path = "downloads/extract/SPNs and PGNs.xlsx";

    // Open workbook once to get all rows from first sheet
    if let Ok(data) = {
        let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open workbook");
        xlsx.worksheet_range(&xlsx.sheet_names()[0])
    } {
        println!("=== PGN Extraction ===\n");

        // Track unique PGNs and their names, plus warnings for mismatches
        let mut pgn_to_name: HashMap<u32, String> = HashMap::new();
        let mut mismatch_count = 0;
        let mut total_rows = 0;

        // Skip header row (index 0), process data rows starting at index 1
        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue;
            } // skip headers

            total_rows += 1;

            let pgn_cell = &row[0];
            let name_cell = &row[1];

            // Parse PGN value from column A (should be integer, stored as float in Excel)
            let pgn_value: u32 = match pgn_cell {
                calamine::DataType::Float(f) => *f as u32,
                calamine::DataType::Int(i) => *i as u32,
                _ => continue, // skip empty or unexpected types
            };

            // Warn if PGN doesn't fit in 24-bit unsigned (max = 0x00FFFFFF = 16777215)
            const MAX_24BIT: u32 = 0x00FFFFFF;
            if pgn_value > MAX_24BIT {
                eprintln!(
                    "WARNING: PGN {} exceeds 24-bit unsigned max ({}). Row {}",
                    pgn_value, MAX_24BIT, row_idx
                );
            }

            // Parse name from column B
            let name = match name_cell {
                calamine::DataType::String(s) => s.clone(),
                _ => continue,
            };

            // De-duplicate: check if we've seen this PGN before
            if let Some(existing_name) = pgn_to_name.get(&pgn_value) {
                if existing_name != &name {
                    eprintln!("WARNING: PGN {} has conflicting names:", pgn_value);
                    eprintln!("  Previously: {}", existing_name);
                    eprintln!("  Now found:  {}", name);
                    mismatch_count += 1;
                }
            } else {
                pgn_to_name.insert(pgn_value, name.clone());
            }
        }

        // Sort by PGN value for clean output
        let mut sorted_entries: Vec<_> = pgn_to_name.iter().collect();
        sorted_entries.sort_by_key(|&(pgn, _)| *pgn);

        println!("Total rows processed: {}", total_rows);
        println!("Unique PGNs found: {}", sorted_entries.len());
        if mismatch_count > 0 {
            eprintln!("\nName mismatches detected: {}\n", mismatch_count);
        } else {
            println!("\nNo name mismatches.\n");
        }

        // Print deduplicated, sorted PGN table (column A = numeric value, column B = name)
        for &(pgn, name) in &sorted_entries {
            println!("{}: {}", pgn, name);
        }
    }
}
