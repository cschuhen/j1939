use crate::data::{GlobalFunctionEntry, IndustryGroupEntry, ManufacturerIdEntry};
use calamine::{Reader, Xlsx, open_workbook};
use std::collections::HashMap;

/// Parse Manufacturer ID entries from an Excel file.
pub fn parse_manufacturer_ids(path: &str) -> Vec<ManufacturerIdEntry> {
    let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open Manufacturer IDs workbook");

    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        let mut id_to_name: HashMap<u8, String> = HashMap::new();

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let id_cell = &row.get(0);
            let name_cell = &row.get(1);

            let id: u8 = match id_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u8,
                    calamine::DataType::Int(i) => *i as u8,
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

            id_to_name.insert(id, name);
        }

        let mut entries: Vec<_> = id_to_name
            .into_iter()
            .map(|(id, name)| ManufacturerIdEntry { id, name })
            .collect();
        entries.sort_by_key(|e| e.id);
        return entries;
    }

    Vec::new()
}

/// Parse Industry Group entries from an Excel file.
pub fn parse_industry_groups(path: &str) -> Vec<IndustryGroupEntry> {
    let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open Industry Groups workbook");

    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        let mut id_to_desc: HashMap<u8, String> = HashMap::new();

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let id_cell = &row.get(0);
            let desc_cell = &row.get(1);

            let id: u8 = match id_cell {
                Some(c) => match c {
                    calamine::DataType::Float(f) => *f as u8,
                    calamine::DataType::Int(i) => *i as u8,
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

            id_to_desc.insert(id, desc);
        }

        let mut entries: Vec<_> = id_to_desc
            .into_iter()
            .map(|(id, desc)| IndustryGroupEntry { id, description: desc })
            .collect();
        entries.sort_by_key(|e| e.id);
        return entries;
    }

    Vec::new()
}

/// Parse Global NAME Function entries from an Excel file.
pub fn parse_global_functions(path: &str) -> Vec<GlobalFunctionEntry> {
    let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open Global NAME Functions workbook");

    let sheets = xlsx.sheet_names();
    if sheets.is_empty() {
        return Vec::new();
    }

    if let Ok(data) = xlsx.worksheet_range(&sheets[0]) {
        let mut id_to_desc: HashMap<u16, String> = HashMap::new();

        for (row_idx, row) in data.rows().enumerate() {
            if row_idx == 0 {
                continue; // skip headers
            }

            let id_cell = &row.get(0);
            let desc_cell = &row.get(1);

            let id: u16 = match id_cell {
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

            id_to_desc.insert(id, desc);
        }

        let mut entries: Vec<_> = id_to_desc
            .into_iter()
            .map(|(id, desc)| GlobalFunctionEntry { id, description: desc })
            .collect();
        entries.sort_by_key(|e| e.id);
        return entries;
    }

    Vec::new()
}
