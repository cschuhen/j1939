use crate::data::DdiEntry;
use std::fs;

pub fn parse(path: &str) -> Vec<DdiEntry> {
    let content = fs::read_to_string(path).expect("failed to read DDI file");
    let lines: Vec<&str> = content.lines().collect();

    let mut entries = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("DD Entity:") {
            // Parse DDI number and name from "DD Entity: <number> <name>"
            let rest = &line["DD Entity:".len()..].trim();
            if let Some(space_pos) = rest.find(' ') {
                let ddi_str = &rest[..space_pos];
                let name = rest[space_pos + 1..].to_string();

                if let Ok(ddi) = ddi_str.parse::<u16>() {
                    // Collect the next lines for this entry
                    let mut unit = None;
                    let mut resolution = 0.0f64;

                    i += 1;
                    while i < lines.len() {
                        let l = lines[i];
                        if l.starts_with("Unit:") {
                            let u = &l["Unit:".len()..].trim();
                            // Format: "symbol - description" — take just the symbol
                            if let Some(dash_pos) = u.find(" - ") {
                                unit = Some(u[..dash_pos].to_string());
                            } else {
                                unit = Some(u.to_string());
                            }
                        }
                        if l.starts_with("Resolution:") {
                            let r = &l["Resolution:".len()..].trim();
                            // Convert comma decimal separator to dot
                            resolution = r.replace(',', ".").parse().unwrap_or(0.0);
                        }
                        // Stop when we hit the next DD Entity or end of file
                        if l.starts_with("DD Entity:") || i + 1 >= lines.len() {
                            break;
                        }
                        i += 1;
                    }

                    entries.push(DdiEntry {
                        ddi,
                        name,
                        unit,
                        resolution,
                        offset: 0.0,
                    });
                }
            }
        }
        i += 1;
    }

    // Sort by DDI value for binary search
    entries.sort_by_key(|e| e.ddi);
    entries
}
