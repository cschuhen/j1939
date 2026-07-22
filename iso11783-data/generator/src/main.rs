mod codegen;
mod data;
mod excel;
mod parsers;
use parsers::{
    isobus_params_parser, name_parsers, name_parsers_ig, pgn_parser, task_controller_ddi,
};

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

const DOWNLOADS_DIR: &str = "downloads";
const STRINGS_DIR: &str = "src/strings";
const CONSTANTS_DIR: &str = "src/constants";
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
                    if name.contains("AEF")
                        && name.contains("Functionalities")
                        && !name.contains("Options")
                    {
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

    let strings_dir = cli
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(STRINGS_DIR));
    fs::create_dir_all(&strings_dir).expect("failed to create strings dir");

    let constants_dir = cli
        .output_dir
        .clone()
        .map(|p| p.join("constants"))
        .unwrap_or_else(|| PathBuf::from(CONSTANTS_DIR));
    fs::create_dir_all(&constants_dir).expect("failed to create constants dir");

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
            "pgn" => generate_pgn(
                &strings_dir,
                &constants_dir,
                &pgn_path,
                today.as_str(),
                &mut rev_data,
            ),
            "isobus_params" => generate_isobus_params(
                &strings_dir,
                &constants_dir,
                &params_path,
                today.as_str(),
                &mut rev_data,
            ),
            "task_controller_ddi" => generate_task_controller_ddi(
                &strings_dir,
                &constants_dir,
                &ddi_path,
                today.as_str(),
                &mut rev_data,
            ),
            "name" => generate_name(&strings_dir, &constants_dir, today.as_str(), &mut rev_data),
            _ => eprintln!("Unknown module: {}", module),
        }
    }

    save_revision(&rev_data);
    println!(
        "\nGeneration complete. Strings: {}, Constants: {}",
        strings_dir.display(),
        constants_dir.display()
    );
}

fn generate_pgn(
    strings_dir: &PathBuf,
    constants_dir: &PathBuf,
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

    let source_name = "SPNs and PGNs.xlsx";

    // Generate strings file
    let strings_content = codegen::generate_pgn_strings(&entries, source_name, date);
    if let Err(e) = codegen::write_output(strings_dir, "pgn", &strings_content) {
        eprintln!("ERROR writing pgn.rs: {}", e);
    } else {
        println!("  Written strings/pgn.rs");
    }

    // Generate constants file
    let constants_content = codegen::generate_pgn_constants(&entries, source_name, date);
    if let Err(e) = codegen::write_output(constants_dir, "pgn", &constants_content) {
        eprintln!("ERROR writing pgn.rs: {}", e);
    } else {
        println!("  Written constants/pgn.rs");
    }

    rev_data["pgn"]["source"] = serde_json::json!(source_name);
    rev_data["pgn"]["date"] = serde_json::json!(date);
}

fn generate_isobus_params(
    strings_dir: &PathBuf,
    constants_dir: &PathBuf,
    file_path: &Option<PathBuf>,
    date: &str,
    rev_data: &mut serde_json::Value,
) {
    let path = match file_path {
        Some(p) => p,
        None => {
            eprintln!(
                "WARNING: No ISOBUS params source file found. Skipping isobus_params module."
            );
            return;
        }
    };

    println!(
        "Generating ISOBUS params lookup from {} ...",
        path.display()
    );
    let entries = isobus_params_parser::parse(path.to_str().unwrap());
    println!("  Found {} unique parameter names", entries.len());

    let source_name = path.file_name().unwrap_or_default().to_string_lossy();

    // Generate strings file
    let strings_content = codegen::generate_isobus_params_strings(&entries, &source_name, date);
    if let Err(e) = codegen::write_output(strings_dir, "isobus_params", &strings_content) {
        eprintln!("ERROR writing isobus_params.rs: {}", e);
    } else {
        println!("  Written strings/isobus_params.rs");
    }

    // Generate constants file (empty for now - no named constants for params)
    let constants_content = codegen::generate_isobus_params_constants(&entries, &source_name, date);
    if let Err(e) = codegen::write_output(constants_dir, "isobus_params", &constants_content) {
        eprintln!("ERROR writing isobus_params.rs: {}", e);
    } else {
        println!("  Written constants/isobus_params.rs");
    }

    rev_data["isobus_params"]["source"] = serde_json::json!(source_name);
    rev_data["isobus_params"]["date"] = serde_json::json!(date);
}

fn generate_task_controller_ddi(
    strings_dir: &PathBuf,
    constants_dir: &PathBuf,
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

    println!(
        "Generating Task Controller DDI lookup from {} ...",
        path.display()
    );
    let entries = task_controller_ddi::parse(path.to_str().unwrap());
    println!("  Found {} DDI entries", entries.len());

    let source_name = "TaskControllerDDI.txt";

    // Generate strings file
    let strings_content =
        codegen::generate_task_controller_ddi_strings(&entries, source_name, date);
    if let Err(e) = codegen::write_output(strings_dir, "task_controller_ddi", &strings_content) {
        eprintln!("ERROR writing task_controller_ddi.rs: {}", e);
    } else {
        println!("  Written strings/task_controller_ddi.rs");
    }

    // Generate constants file
    let constants_content =
        codegen::generate_task_controller_ddi_constants(&entries, source_name, date);
    if let Err(e) = codegen::write_output(constants_dir, "task_controller_ddi", &constants_content)
    {
        eprintln!("ERROR writing task_controller_ddi.rs: {}", e);
    } else {
        println!("  Written constants/task_controller_ddi.rs");
    }

    rev_data["task_controller_ddi"]["source"] = serde_json::json!(source_name);
    rev_data["task_controller_ddi"]["date"] = serde_json::json!(date);
}

fn generate_name(
    strings_dir: &PathBuf,
    constants_dir: &PathBuf,
    date: &str,
    rev_data: &mut serde_json::Value,
) {
    let extract_dir = PathBuf::from(DOWNLOADS_DIR).join("extract");

    let manufacturer_path = extract_dir.join("Manufacturer IDs.xlsx");
    let industry_groups_path = extract_dir.join("Industry Groups.xlsx");
    let global_functions_path = extract_dir.join("Global NAME Functions.xlsx");
    let ig_specific_path = extract_dir.join("IG Specific NAME Function.xlsx");

    if !manufacturer_path.exists()
        || !industry_groups_path.exists()
        || !global_functions_path.exists()
        || !ig_specific_path.exists()
    {
        eprintln!("WARNING: Required source files for 'name' module not found in downloads/extract/. Skipping name module.");
        return;
    }

    println!("Generating NAME lookup from extracted Excel files ...");

    let manufacturer_ids =
        name_parsers::parse_manufacturer_ids(manufacturer_path.to_str().unwrap());
    println!("  Found {} manufacturer IDs", manufacturer_ids.len());

    let industry_groups =
        name_parsers::parse_industry_groups(industry_groups_path.to_str().unwrap());
    println!("  Found {} industry groups", industry_groups.len());

    let global_functions =
        name_parsers::parse_global_functions(global_functions_path.to_str().unwrap());
    println!("  Found {} global NAME functions", global_functions.len());

    let ig_specific_functions =
        name_parsers_ig::parse_ig_specific_functions(ig_specific_path.to_str().unwrap());
    println!(
        "  Found {} IG-specific NAME function entries",
        ig_specific_functions.len()
    );

    let vehicle_systems =
        name_parsers_ig::parse_vehicle_systems(ig_specific_path.to_str().unwrap());
    println!("  Found {} unique vehicle systems", vehicle_systems.len());

    let source_name = "NAME lookup tables (Manufacturer IDs, Industry Groups, Global NAME Functions, IG Specific NAME Function)";

    // Generate strings file
    let strings_content = codegen::generate_name_strings(
        &manufacturer_ids,
        &industry_groups,
        &global_functions,
        &ig_specific_functions,
        &vehicle_systems,
        source_name,
        date,
    );
    if let Err(e) = codegen::write_output(strings_dir, "name", &strings_content) {
        eprintln!("ERROR writing name.rs: {}", e);
    } else {
        println!("  Written strings/name.rs");
    }

    // Build a set of vehicle system keys that have at least one IG-specific function entry
    let mut vs_with_entries: HashSet<(u8, u8)> = HashSet::new();
    for entry in &ig_specific_functions {
        vs_with_entries.insert((entry.industry_group_id, entry.vehicle_system_id));
    }

    // Generate constants content
    let mod_content = codegen::generate_name_mod(source_name, date);
    let manufacturer_ids_content =
        codegen::generate_name_manufacturer_ids(&manufacturer_ids, source_name, date);

    // Write name/mod.rs (re-exports manufacturer_ids and global)
    if let Err(e) = codegen::write_mod_subdir(constants_dir, "name", &mod_content) {
        eprintln!("ERROR writing constants/name/mod.rs: {}", e);
    } else {
        println!("  Written constants/name/");
    }

    // Write name/manufacturer_ids.rs
    if let Err(e) = codegen::write_output_subdir(
        constants_dir,
        "name",
        "manufacturer_ids",
        &manufacturer_ids_content,
    ) {
        eprintln!("ERROR writing constants/name/manufacturer_ids.rs: {}", e);
    } else {
        println!("  Written constants/name/manufacturer_ids.rs");
    }

    // Write name/global.rs (industry group 0 - global functions for all vehicle systems)
    let global_functions_filtered: Vec<_> = ig_specific_functions
        .iter()
        .filter(|e| e.industry_group_id == 0 && vs_with_entries.contains(&(0, e.vehicle_system_id)))
        .cloned()
        .collect();

    if !global_functions_filtered.is_empty() {
        let global_content =
            codegen::generate_global_module(&global_functions_filtered, source_name, date);
        if let Err(e) =
            codegen::write_output_subdir(constants_dir, "name", "global", &global_content)
        {
            eprintln!("ERROR writing constants/name/global.rs: {}", e);
        } else {
            println!(
                "  Written constants/name/global.rs ({} entries)",
                global_functions_filtered.len()
            );
        }
    }

    // Clean up old per-industry-group directories (one .rs file per vehicle system approach)
    let mut seen_mod_names = HashSet::new();
    for ig_entry in &industry_groups {
        if ig_entry.id == 0 {
            continue;
        }

        let ig_mod_name = codegen::name_to_module(&ig_entry.description);

        // Deduplicate reserved groups (IG6 and IG7 have the same description)
        let dir_name = if !seen_mod_names.insert(ig_mod_name.clone()) {
            format!("{}_{}", ig_mod_name, ig_entry.id)
        } else {
            ig_mod_name
        };

        let old_dir_path = constants_dir.join(format!("name/{}", dir_name));
        if old_dir_path.exists() && old_dir_path.is_dir() {
            fs::remove_dir_all(&old_dir_path).ok();
        }
    }

    // Write per-industry-group .rs files with inline modules for each vehicle system (IG > 0)
    let mut seen_mod_names = HashSet::new();
    for ig_entry in &industry_groups {
        if ig_entry.id == 0 {
            continue;
        }

        let ig_mod_name = codegen::name_to_module(&ig_entry.description);

        // Deduplicate reserved groups (IG6 and IG7 have the same description)
        let file_name = if !seen_mod_names.insert(ig_mod_name.clone()) {
            format!("{}_{}", ig_mod_name, ig_entry.id)
        } else {
            ig_mod_name
        };

        // Collect all IG-specific function entries for this industry group (only those with at least one entry)
        let vs_for_ig: Vec<_> = ig_specific_functions
            .iter()
            .filter(|e| {
                e.industry_group_id == ig_entry.id
                    && vs_with_entries.contains(&(ig_entry.id, e.vehicle_system_id))
            })
            .cloned()
            .collect();

        if vs_for_ig.is_empty() {
            continue;
        }

        let content = codegen::generate_industry_group_file(
            ig_entry,
            &vehicle_systems,
            &vs_for_ig,
            source_name,
            date,
        );
        if let Err(e) = codegen::write_output_subdir(
            constants_dir,
            "name/industry_groups",
            &file_name,
            &content,
        ) {
            eprintln!(
                "ERROR writing constants/name/industry_groups/{}.rs: {}",
                file_name, e
            );
        } else {
            println!(
                "  Written constants/name/industry_groups/{} ({} entries)",
                file_name,
                vs_for_ig.len()
            );
        }
    }

    // Write industry_groups/mod.rs with submodule declarations and re-exports (only for IGs that have entries)
    let ig_mod_content = codegen::generate_name_industry_groups_mod(
        &industry_groups,
        &vs_with_entries,
        source_name,
        date,
    );
    if let Err(e) = codegen::write_output_subdir(
        constants_dir,
        "name/industry_groups",
        "mod",
        &ig_mod_content,
    ) {
        eprintln!("ERROR writing constants/name/industry_groups/mod.rs: {}", e);
    } else {
        println!("  Written constants/name/industry_groups/mod.rs");
    }

    rev_data["name"]["source"] = serde_json::json!(source_name);
    rev_data["name"]["date"] = serde_json::json!(date);
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
    let strings_dir = PathBuf::from(STRINGS_DIR);
    let constants_dir = PathBuf::from(CONSTANTS_DIR);

    if strings_dir.exists() {
        println!("\nGenerated strings:");
        for fname in &["pgn.rs", "isobus_params.rs", "task_controller_ddi.rs"] {
            let path = strings_dir.join(fname);
            if path.exists() {
                if let Ok(metadata) = fs::metadata(&path) {
                    println!("  {} ({} bytes)", fname, metadata.len());
                }
            } else {
                println!("  {} — not generated", fname);
            }
        }
    }

    if constants_dir.exists() {
        println!("\nGenerated constants:");
        for fname in &["pgn.rs", "isobus_params.rs", "task_controller_ddi.rs"] {
            let path = constants_dir.join(fname);
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
