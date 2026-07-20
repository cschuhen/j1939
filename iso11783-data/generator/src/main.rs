mod codegen;
mod data;
mod excel;
mod parsers;
use parsers::{isobus_params_parser, name_parsers, name_parsers_ig, pgn_parser, task_controller_ddi};


use std::fs;
use std::path::PathBuf;

const DOWNLOADS_DIR: &str = "downloads";
const OUTPUT_DIR: &str = "src";
const REVISION_FILE: &str = "generator/revision.json";

const SOURCES: &[Source] = &[
    Source {
        name: "TaskControllerDDI",
        url: "https://www.isobus.net/isobus/exports/completeTXT",
        output_name: "TaskControllerDDI.txt",
    },
    Source {
        name: "ISOBUSParameters",
        url: "https://www.isobus.net/isobus/attachments/isoExport_xlsx.zip",
        output_name: "isoExport_xlsx.zip",
    },
];

struct Source {
    name: &'static str,
    url: &'static str,
    output_name: &'static str,
}

#[derive(Debug, Default)]
struct CliArgs {
    command: String,
    modules: Vec<String>,
    pgn_file: Option<PathBuf>,
    params_file: Option<PathBuf>,
    ddi_file: Option<PathBuf>,
    output_dir: Option<PathBuf>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cli = CliArgs::default();

    if args.is_empty() {
        cli.command = "generate".to_string();
        return cli;
    }

    cli.command = args[0].clone();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--module" => {
                i += 1;
                if i < args.len() {
                    cli.modules.push(args[i].clone());
                }
            }
            "--pgn-file" => {
                i += 1;
                if i < args.len() {
                    cli.pgn_file = Some(PathBuf::from(&args[i]));
                }
            }
            "--params-file" => {
                i += 1;
                if i < args.len() {
                    cli.params_file = Some(PathBuf::from(&args[i]));
                }
            }
            "--ddi-file" => {
                i += 1;
                if i < args.len() {
                    cli.ddi_file = Some(PathBuf::from(&args[i]));
                }
            }
            "--output-dir" => {
                i += 1;
                if i < args.len() {
                    cli.output_dir = Some(PathBuf::from(&args[i]));
                }
            }
            _ => {}
        }
        i += 1;
    }

    cli
}

fn find_xlsx_in_dir(dir: &PathBuf) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "xlsx") {
                return Some(path);
            }
            if path.is_dir() {
                if let Some(found) = find_xlsx_in_dir(&path) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn find_pgn_file(extract_dir: &PathBuf) -> Option<PathBuf> {
    // Look specifically for "SPNs and PGNs.xlsx" first, then any xlsx
    if let Ok(entries) = fs::read_dir(extract_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.contains("SPN") && name.contains("PGN") {
                    return Some(path);
                }
            }
        }
    }
    find_xlsx_in_dir(extract_dir)
}

fn download_and_extract(_args: &CliArgs) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let downloads_dir = PathBuf::from(DOWNLOADS_DIR);
    fs::create_dir_all(&downloads_dir).expect("failed to create downloads dir");

    let mut pgn_path = None;
    let mut params_path = None;
    let mut ddi_path = None;

    // Download sources
    for source in SOURCES {
        let path = downloads_dir.join(source.output_name);
        if !path.exists() {
            println!("Downloading {} ...", source.name);
            excel::download_file(source.url, &path)
                .unwrap_or_else(|e| eprintln!("  ERROR downloading {}: {}", source.name, e));
        } else {
            println!("Already exists: {}", path.display());
        }
    }

    // Handle zip extraction for ISOBUS Parameters (contains PGN data)
    let zip_path = downloads_dir.join("isoExport_xlsx.zip");
    if zip_path.exists() {
        let extract_dir = downloads_dir.join("extract");
        fs::create_dir_all(&extract_dir).ok();

        // Only extract if extract dir is empty or doesn't have xlsx files
        let needs_extract = fs::read_dir(&extract_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .all(|e| !e.path().extension().map_or(false, |ext| ext == "xlsx"))
            })
            .unwrap_or(true);

        if needs_extract {
            println!("  Extracting isoExport_xlsx.zip ...");
            excel::unzip(&zip_path, &extract_dir);
        }

        // Find PGN file
        pgn_path = find_pgn_file(&extract_dir);

        // For params, use AEF Functionalities as default (simple value→meaning mapping)
        if let Ok(entries) = fs::read_dir(&extract_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains("AEF") && name.contains("Functionalities") && !name.contains("Options") {
                        params_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    // Handle Task Controller DDI (TXT file)
    let ddi_file_path = downloads_dir.join("TaskControllerDDI.txt");
    if ddi_file_path.exists() {
        ddi_path = Some(ddi_file_path);
    }

    (pgn_path, params_path, ddi_path)
}

fn load_revision() -> serde_json::Value {
    let path = PathBuf::from(REVISION_FILE);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                return value;
            }
        }
    }
    serde_json::json!({})
}

fn save_revision(rev: &serde_json::Value) {
    let path = PathBuf::from(REVISION_FILE);
    if let Ok(content) = serde_json::to_string_pretty(rev) {
        fs::write(&path, content).ok();
    }
}

fn get_today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    // Simple date calculation (not perfect but good enough)
    let days = secs / 86400;
    // Epoch day 0 is Thursday Jan 1, 1970
    let mut y = 1970;
    let mut remaining_days = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let year_days = if leap { 366 } else { 365 };
        if remaining_days < year_days {
            break;
        }
        remaining_days -= year_days;
        y += 1;
    }
    let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let mut m = 0;
    for &md in &months {
        let day_count = if m == 1 && leap { 29 } else { md };
        if remaining_days < day_count {
            break;
        }
        remaining_days -= day_count;
        m += 1;
    }
    let d = remaining_days + 1;
    format!("{:04}-{:02}-{:02}", y, m + 1, d)
}

fn main() {
    let cli = parse_args();

    match cli.command.as_str() {
        "generate" => cmd_generate(&cli),
        "info" => cmd_info(&cli),
        _ => {
            eprintln!("Unknown command: {}", cli.command);
            eprintln!("Usage: generator [generate|info] [--module pgn|isobus_params|task_controller_ddi] [--pgn-file PATH] [--params-file PATH] [--ddi-file PATH]");
            std::process::exit(1);
        }
    }
}

fn cmd_generate(cli: &CliArgs) {
    println!("=== ISO 11783 Data — Generator ===\n");

    let output_dir = cli.output_dir.clone().unwrap_or_else(|| PathBuf::from(OUTPUT_DIR));
    fs::create_dir_all(&output_dir).expect("failed to create output dir");

    // Download and extract source files
    let (downloaded_pgn, downloaded_params, downloaded_ddi) = download_and_extract(cli);

    // Determine file paths
    let pgn_path = cli.pgn_file.clone().or(downloaded_pgn);
    let params_path = cli.params_file.clone().or(downloaded_params);
    let ddi_path = cli.ddi_file.clone().or(downloaded_ddi);

    // Load revision tracking
    let mut rev_data = load_revision();

    let today = get_today();

    // Determine which modules to generate
    let all_modules = if cli.modules.is_empty() {
        vec![
            "pgn".to_string(),
            "isobus_params".to_string(),
            "task_controller_ddi".to_string(),
            "name".to_string(),
        ]
    } else {
        cli.modules.clone()
    };

    for module in &all_modules {
        match module.as_str() {
            "pgn" => generate_pgn(&output_dir, &pgn_path, today.as_str(), &mut rev_data),
            "isobus_params" => generate_isobus_params(&output_dir, &params_path, today.as_str(), &mut rev_data),
            "task_controller_ddi" => generate_task_controller_ddi(&output_dir, &ddi_path, today.as_str(), &mut rev_data),
            "name" => generate_name(&output_dir, today.as_str(), &mut rev_data),
            _ => eprintln!("Unknown module: {}", module),
        }
    }

    save_revision(&rev_data);
    println!("\nGeneration complete. Output directory: {}", output_dir.display());
}

fn generate_pgn(
    output_dir: &PathBuf,
    file_path: &Option<PathBuf>,
    date: &str,
    rev_data: &mut serde_json::Value,
) {
    let path = match file_path {
        Some(p) => p,
        None => {
            eprintln!("WARNING: No PGN source file found. Skipping pgn module.");
            return;
        }
    };

    println!("Generating PGN lookup from {} ...", path.display());
    let entries = pgn_parser::parse(path.to_str().unwrap());
    println!("  Found {} unique PGNs", entries.len());

    // Check if content changed vs previous revision
    let source_name = "SPNs and PGNs.xlsx";
    let current_content = codegen::generate_pgn(&entries, source_name, date);

    let prev_rev = rev_data.get("pgn").and_then(|v| v.get("rev")).and_then(|v| v.as_u64()).unwrap_or(0);
    let new_rev = if prev_rev == 0 {
        1
    } else {
        // Simple check: if file exists and has same content, keep revision
        let existing_path = output_dir.join("pgn.rs");
        if existing_path.exists() {
            if let Ok(existing) = fs::read_to_string(&existing_path) {
                if existing == current_content {
                    prev_rev
                } else {
                    prev_rev + 1
                }
            } else {
                prev_rev + 1
            }
        } else {
            prev_rev + 1
        }
    };

    rev_data["pgn"]["source"] = serde_json::json!(source_name);
    rev_data["pgn"]["rev"] = serde_json::json!(new_rev);
    rev_data["pgn"]["date"] = serde_json::json!(date);

    let content = codegen::generate_pgn(&entries, source_name, date);
    if let Err(e) = codegen::write_output(output_dir, "pgn", &content) {
        eprintln!("ERROR writing pgn.rs: {}", e);
    } else {
        println!("  Written pgn.rs (rev {})", new_rev);
    }
}

fn generate_isobus_params(
    output_dir: &PathBuf,
    file_path: &Option<PathBuf>,
    date: &str,
    rev_data: &mut serde_json::Value,
) {
    let path = match file_path {
        Some(p) => p,
        None => {
            eprintln!("WARNING: No ISOBUS params source file found. Skipping isobus_params module.");
            return;
        }
    };

    println!("Generating ISOBUS params lookup from {} ...", path.display());
    let entries = isobus_params_parser::parse(path.to_str().unwrap());
    println!("  Found {} unique parameter names", entries.len());

    let source_name = path.file_name().unwrap_or_default().to_string_lossy();
    let current_content = codegen::generate_isobus_params(&entries, &source_name, date);

    let prev_rev = rev_data.get("isobus_params").and_then(|v| v.get("rev")).and_then(|v| v.as_u64()).unwrap_or(0);
    let new_rev = if prev_rev == 0 {
        1
    } else {
        let existing_path = output_dir.join("isobus_params.rs");
        if existing_path.exists() {
            if let Ok(existing) = fs::read_to_string(&existing_path) {
                if existing == current_content {
                    prev_rev
                } else {
                    prev_rev + 1
                }
            } else {
                prev_rev + 1
            }
        } else {
            prev_rev + 1
        }
    };

    rev_data["isobus_params"]["source"] = serde_json::json!(source_name);
    rev_data["isobus_params"]["rev"] = serde_json::json!(new_rev);
    rev_data["isobus_params"]["date"] = serde_json::json!(date);

    let content = codegen::generate_isobus_params(&entries, &source_name, date);
    if let Err(e) = codegen::write_output(output_dir, "isobus_params", &content) {
        eprintln!("ERROR writing isobus_params.rs: {}", e);
    } else {
        println!("  Written isobus_params.rs (rev {})", new_rev);
    }
}

fn generate_task_controller_ddi(
    output_dir: &PathBuf,
    file_path: &Option<PathBuf>,
    date: &str,
    rev_data: &mut serde_json::Value,
) {
    let path = match file_path {
        Some(p) => p,
        None => {
            eprintln!("WARNING: No Task Controller DDI source file found. Skipping task_controller_ddi module.");
            return;
        }
    };

    println!("Generating Task Controller DDI lookup from {} ...", path.display());
    let entries = task_controller_ddi::parse(path.to_str().unwrap());
    println!("  Found {} DDI entries", entries.len());

    let source_name = "TaskControllerDDI.txt";
    let prev_rev = rev_data.get("task_controller_ddi").and_then(|v| v.get("rev")).and_then(|v| v.as_u64()).unwrap_or(0);
    let new_rev = if prev_rev == 0 {
        1
    } else {
        let existing_path = output_dir.join("task_controller_ddi.rs");
        if existing_path.exists() {
            if let Ok(existing) = fs::read_to_string(&existing_path) {
                let current_content = codegen::generate_task_controller_ddi(&entries, source_name, date);
                if existing == current_content {
                    prev_rev
                } else {
                    prev_rev + 1
                }
            } else {
                prev_rev + 1
            }
        } else {
            prev_rev + 1
        }
    };

    rev_data["task_controller_ddi"]["source"] = serde_json::json!(source_name);
    rev_data["task_controller_ddi"]["rev"] = serde_json::json!(new_rev);
    rev_data["task_controller_ddi"]["date"] = serde_json::json!(date);

    let content = codegen::generate_task_controller_ddi(&entries, source_name, date);
    if let Err(e) = codegen::write_output(output_dir, "task_controller_ddi", &content) {
        eprintln!("ERROR writing task_controller_ddi.rs: {}", e);
    } else {
        println!("  Written task_controller_ddi.rs (rev {})", new_rev);
    }
}

fn generate_name(
    output_dir: &PathBuf,
    date: &str,
    rev_data: &mut serde_json::Value,
) {
    let extract_dir = PathBuf::from(DOWNLOADS_DIR).join("extract");

    let manufacturer_path = extract_dir.join("Manufacturer IDs.xlsx");
    let industry_groups_path = extract_dir.join("Industry Groups.xlsx");
    let global_functions_path = extract_dir.join("Global NAME Functions.xlsx");
    let ig_specific_path = extract_dir.join("IG Specific NAME Function.xlsx");

    if !manufacturer_path.exists() || !industry_groups_path.exists() || !global_functions_path.exists() || !ig_specific_path.exists() {
        eprintln!("WARNING: Required source files for 'name' module not found in downloads/extract/. Skipping name module.");
        return;
    }

    println!("Generating NAME lookup from extracted Excel files ...");

    let manufacturer_ids = name_parsers::parse_manufacturer_ids(manufacturer_path.to_str().unwrap());
    println!("  Found {} manufacturer IDs", manufacturer_ids.len());

    let industry_groups = name_parsers::parse_industry_groups(industry_groups_path.to_str().unwrap());
    println!("  Found {} industry groups", industry_groups.len());

    let global_functions = name_parsers::parse_global_functions(global_functions_path.to_str().unwrap());
    println!("  Found {} global NAME functions", global_functions.len());

    let ig_specific_functions = name_parsers_ig::parse_ig_specific_functions(ig_specific_path.to_str().unwrap());
    println!("  Found {} IG-specific NAME function entries", ig_specific_functions.len());

    let vehicle_systems = name_parsers_ig::parse_vehicle_systems(ig_specific_path.to_str().unwrap());
    println!("  Found {} unique vehicle systems", vehicle_systems.len());

    let source_name = "NAME lookup tables (Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function)";
    let prev_rev = rev_data.get("name").and_then(|v| v.get("rev")).and_then(|v| v.as_u64()).unwrap_or(0);

    let current_content = codegen::generate_name(
        &manufacturer_ids,
        &industry_groups,
        &global_functions,
        &ig_specific_functions,
        &vehicle_systems,
        source_name,
        date,
    );

    let new_rev = if prev_rev == 0 {
        1
    } else {
        let existing_path = output_dir.join("name.rs");
        if existing_path.exists() {
            if let Ok(existing) = fs::read_to_string(&existing_path) {
                if existing == current_content {
                    prev_rev
                } else {
                    prev_rev + 1
                }
            } else {
                prev_rev + 1
            }
        } else {
            prev_rev + 1
        }
    };

    rev_data["name"]["source"] = serde_json::json!(source_name);
    rev_data["name"]["rev"] = serde_json::json!(new_rev);
    rev_data["name"]["date"] = serde_json::json!(date);

    if let Err(e) = codegen::write_output(output_dir, "name", &current_content) {
        eprintln!("ERROR writing name.rs: {}", e);
    } else {
        println!("  Written name.rs (rev {})", new_rev);
    }
}


fn cmd_info(_cli: &CliArgs) {
    println!("=== ISO 11783 Data — Source Info ===\n");

    let rev_data = load_revision();
    println!("Revision tracking:");
    if let Some(modules) = rev_data.as_object() {
        for (name, info) in modules {
            if let Some(source) = info.get("source").and_then(|v| v.as_str()) {
                if let Some(rev) = info.get("rev").and_then(|v| v.as_u64()) {
                    if let Some(date) = info.get("date").and_then(|v| v.as_str()) {
                        println!("  {}: {} rev {} ({}))", name, source, rev, date);
                    }
                }
            }
        }
    }

    // Show downloaded files
    let downloads_dir = PathBuf::from(DOWNLOADS_DIR);
    if downloads_dir.exists() {
        println!("\nDownloaded files:");
        if let Ok(entries) = fs::read_dir(&downloads_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(metadata) = fs::metadata(&path) {
                        println!("  {} ({} bytes)", name, metadata.len());
                    }
                }
            }
        }
    }

    // Show extracted files
    let extract_dir = downloads_dir.join("extract");
    if extract_dir.exists() {
        println!("\nExtracted files:");
        if let Ok(entries) = fs::read_dir(&extract_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(metadata) = fs::metadata(&path) {
                        println!("  {} ({} bytes)", name, metadata.len());
                    }
                }
            }
        }
    }

    // Show generated files
    let output_dir = PathBuf::from(OUTPUT_DIR);
    if output_dir.exists() {
        println!("\nGenerated files:");
        for fname in &["pgn.rs", "isobus_params.rs", "task_controller_ddi.rs"] {
            let path = output_dir.join(fname);
            if path.exists() {
                if let Ok(metadata) = fs::metadata(&path) {
                    println!("  {} ({} bytes)", fname, metadata.len());
                }
            } else {
                println!("  {} — not generated", fname);
            }
        }
    }
}
