use calamine::{open_workbook, Reader, Xlsx};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::ZipArchive;

pub fn download_file(url: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = reqwest::blocking::get(url)?;
    let mut bytes = Vec::new();
    response.read_to_end(&mut bytes)?;
    fs::write(path, &bytes)?;
    Ok(())
}

pub fn unzip(zip_path: &Path, dest_dir: &Path) {
    println!("  Extracting {} ...", zip_path.display());
    let file = fs::File::open(zip_path).expect("failed to open zip");
    let mut archive = ZipArchive::new(file).expect("failed to parse zip");

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("failed to get zip entry");
        let outpath = dest_dir.join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).ok();
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut outfile = fs::File::create(&outpath).expect("failed to create extraction file");
            std::io::copy(&mut file, &mut outfile).expect("failed to copy zip entry");
        }
    }

    println!("  Extracted {} files", archive.len());
}

pub fn analyze(path: &Path) {
    if path.extension().map_or(false, |ext| ext == "txt") {
        return analyze_txt(path);
    }

    let mut xlsx: Xlsx<_> = open_workbook(path).expect("failed to open workbook");

    println!("  Sheets:");
    for sheet in xlsx.sheet_names() {
        println!("    - {}", sheet);
    }

    // Read first sheet as a sample
    let sheets = xlsx.sheet_names();
    if !sheets.is_empty() {
        let first_sheet = &sheets[0];
        if let Ok(data) = xlsx.worksheet_range(first_sheet) {
            let row_count = data.rows().count();
            let col_count = data.rows().next().map_or(0, |r| r.len());
            println!(
                "  First sheet '{}' — {} rows, {} cols:",
                first_sheet, row_count, col_count
            );

            // Re-open to iterate (rows() consumed by count())
            if let Ok(data) = xlsx.worksheet_range(first_sheet) {
                // Print header row (row 0)
                if let Some(header_row) = data.rows().next() {
                    print!("  Headers:");
                    for cell in header_row {
                        print!(" {:?}", cell);
                    }
                    println!();
                }

                // Print first few data rows
                let mut count = 0;
                for row in data.rows().skip(1) {
                    if count >= 5 {
                        break;
                    }
                    print!("  Row {}: ", count + 1);
                    for cell in row {
                        print!("{:?} ", cell);
                    }
                    println!();
                    count += 1;
                }

                // Save full dump to file for detailed review
                let dump_path = path.with_file_name(format!(
                    "{}_dump.txt",
                    path.file_stem().unwrap_or_default().to_string_lossy()
                ));
                let mut file = fs::File::create(&dump_path).expect("failed to create dump file");
                writeln!(
                    file,
                    "=== {} ===",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )
                .unwrap();
                writeln!(file, "Sheets: {:?}", xlsx.sheet_names()).unwrap();

                // Re-open for full dump (sheet_names consumed the workbook)
                let mut xlsx2: Xlsx<_> = open_workbook(path).expect("failed to re-open workbook");
                for sheet in xlsx2.sheet_names() {
                    writeln!(file, "\n--- Sheet: {} ---", sheet).unwrap();
                    if let Ok(data) = xlsx2.worksheet_range(&sheet) {
                        for (i, row) in data.rows().enumerate() {
                            write!(file, "  Row {}: ", i).unwrap();
                            for cell in row {
                                write!(file, "{:?} | ", cell).unwrap();
                            }
                            writeln!(file).unwrap();
                        }
                    }
                }
                println!("  Full dump saved to: {}", dump_path.display());
            }
        }
    }
}

fn analyze_txt(path: &Path) {
    let content = fs::read_to_string(path).expect("failed to read txt file");
    let lines: Vec<&str> = content.lines().collect();
    println!("  {} total lines", lines.len());

    // Print first 20 lines as sample
    println!("  First 20 lines:");
    for (i, line) in lines.iter().take(20).enumerate() {
        println!("    {}: {}", i + 1, line);
    }

    // Save full dump to file for detailed review
    let dump_path = path.with_file_name(format!(
        "{}_dump.txt",
        path.file_stem().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&dump_path, &content).expect("failed to write dump");
    println!("  Full dump saved to: {}", dump_path.display());
}
