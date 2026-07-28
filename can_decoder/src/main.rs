use std::sync::Arc;

use anyhow::Result;
use can_decoder::device_manager::DeviceManager;
use can_decoder::filters::{CompositeFilter, FilterParser};
use can_decoder::pipeline::{
    CondensedRenderer, ConsoleRenderer, CsvRenderer, FullCondensedRenderer, JsonRenderer, Pipeline,
};
use can_decoder::sources::{CandumpFileSource, SocketCanSource};
use can_decoder::traits::Source;
use can_decoder::Cli;
use can_decoder::SourceType;
use clap::Parser;

/// Validate proprietary DDI definition names and print available options if invalid.
fn validate_proprietary_definitions(names: &[String]) -> Vec<String> {
    let available = vec![("canot", "CANoT proprietary DDI definitions")];

    let mut valid_names = Vec::new();
    for name in names {
        match available.iter().find(|(n, _)| *n == name.as_str()) {
            Some((_, desc)) => {
                println!("Proprietary DDI: {} ({})", name, desc);
                valid_names.push(name.clone());
            }
            None => {
                eprintln!("Error: Unknown proprietary DDI definition '{}'", name);
                eprintln!("Available definitions:");
                for (n, desc) in &available {
                    eprintln!("  {} - {}", n, desc);
                }
                std::process::exit(1);
            }
        }
    }

    valid_names
}

/// Parse CLI filter expressions into a CompositeFilter.
fn build_filters(
    cli_filters: &[String],
) -> Result<Arc<tokio::sync::Mutex<dyn can_decoder::traits::Filter>>> {
    if cli_filters.is_empty() {
        // No filters specified - use pass-through (match all)
        let empty = Arc::new(tokio::sync::Mutex::new(CompositeFilter::new(vec![])));
        return Ok(empty);
    }

    let mut parsed: Vec<Box<dyn can_decoder::traits::Filter>> = Vec::new();

    for expr in cli_filters {
        let filter = FilterParser::parse(expr)?;
        parsed.push(filter);
    }

    let composite = Arc::new(tokio::sync::Mutex::new(CompositeFilter::new(parsed)));
    Ok(composite)
}

/// Entry point: parse CLI args and print configuration summary.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate filters early - help or invalid types should exit before pipeline setup
    for expr in &cli.filter {
        if expr == "help" {
            FilterParser::print_help();
            std::process::exit(0);
        }
        if let Err(e) = FilterParser::parse(expr) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }

    println!("can_decoder starting...");
    println!("Source: {:?}", cli.source);

    if let Some(ref iface) = cli.interface {
        println!("Interface: {}", iface);
    }

    if let Some(ref file) = cli.input_file {
        println!("Input file: {}", file.display());
    }

    if let Some(ref config) = cli.config {
        println!("Config: {}", config.display());
    }

    println!("Detail level: {:?}", cli.detail_level);
    println!("Force partial TP: {}", cli.force_output_partial_tp);
    println!("Debug mode: {}", cli.debug);
    println!("Filters: {}", cli.filter.len());
    println!("Output format: {:?}", cli.output_format);

    let proprietary_defs = validate_proprietary_definitions(&cli.use_proprietary_ddi_definitions);

    let mut pipeline = Pipeline::new();

    // Wire up the Source based on --source and available options
    match cli.source {
        SourceType::Socketcan => {
            if let Some(ref iface) = cli.interface {
                let source = Arc::new(SocketCanSource::new(iface));
                println!("[{}] Starting live source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --interface required for socketcan source");
                std::process::exit(1);
            }
        }
        SourceType::Candump => {
            if let Some(ref file) = cli.input_file {
                let source = Arc::new(CandumpFileSource::new(file));
                println!("[{}] Starting candump source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --input-file required for candump source");
                std::process::exit(1);
            }
        }
    }

    // Wire up Decoder → Filter → Renderer
    let device_manager = Arc::new(std::sync::Mutex::new(DeviceManager::new(60)));
    let decoder = can_decoder::pgn_decoder::J1939Decoder::with_device_manager_detail(
        cli.force_output_partial_tp,
        5000,
        cli.debug,
        Some(device_manager),
        cli.detail_level.clone(),
        &proprietary_defs,
    );
    pipeline.spawn_decoder(Box::new(decoder));

    let filter = build_filters(&cli.filter)?;
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);

    let renderer: Box<dyn can_decoder::traits::Renderer> = match cli.output_format {
        can_decoder::OutputFormat::Console => Box::new(ConsoleRenderer),
        can_decoder::OutputFormat::Json => Box::new(JsonRenderer),
        can_decoder::OutputFormat::Csv => Box::new(CsvRenderer::default()),
        can_decoder::OutputFormat::Condensed => Box::new(CondensedRenderer),
        can_decoder::OutputFormat::FullCondensed => Box::new(FullCondensedRenderer),
    };
    let renderer_handle = Pipeline::spawn_renderer(filter_rx, renderer);

    println!("Pipeline ready. Press Ctrl+C to stop.");

    if cli.source == SourceType::Candump {
        // Batch mode: wait for the renderer to finish
        drop(pipeline);
        let _ = renderer_handle.await;
        println!("\nShutting down...");
    } else {
        // Live mode: wait for Ctrl+C
        tokio::signal::ctrl_c().await.unwrap();
        println!("\nShutting down...");
    }

    Ok(())
}
