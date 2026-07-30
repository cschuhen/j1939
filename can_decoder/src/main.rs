use std::sync::Arc;

use anyhow::Result;
use can_decoder::config::validate_proprietary_definitions;
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

/// Parse CLI filter expressions into a CompositeFilter.
fn build_filters(
    cli_filters: &[String],
) -> Result<Arc<tokio::sync::Mutex<dyn can_decoder::traits::Filter>>> {
    if cli_filters.is_empty() {
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
    let config = &cli.shared;

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
    println!("Source: {:?}", config.source);

    if let Some(ref iface) = config.interface {
        println!("Interface: {}", iface);
    }

    if let Some(ref file) = config.input_file {
        println!("Input file: {}", file.display());
    }

    if let Some(ref cfg) = config.config {
        println!("Config: {}", cfg.display());
    }

    println!("Detail level: {:?}", config.detail_level);
    println!("Force partial TP: {}", config.force_output_partial_tp);
    println!("Debug mode: {}", config.debug);
    println!("Filters: {}", cli.filter.len());
    println!("Output format: {:?}", cli.output_format);

    let proprietary_defs =
        validate_proprietary_definitions(&config.use_proprietary_ddi_definitions);

    let mut pipeline = Pipeline::new();

    // Wire up the Source based on --source and available options
    match config.source {
        SourceType::Socketcan => {
            if let Some(ref iface) = config.interface {
                let source = Arc::new(SocketCanSource::new(iface));
                println!("[{}] Starting live source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --interface required for socketcan source");
                std::process::exit(1);
            }
        }
        SourceType::Candump => {
            if let Some(ref file) = config.input_file {
                let source = Arc::new(CandumpFileSource::new(file));
                println!("[{}] Starting candump source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --input-file required for candump source");
                std::process::exit(1);
            }
        }
    }

    // Wire up Decoder -> Filter -> Renderer
    let device_manager = Arc::new(std::sync::Mutex::new(DeviceManager::new(60)));
    let decoder = can_decoder::pgn_decoder::J1939Decoder::with_device_manager_detail(
        config.force_output_partial_tp,
        5000,
        config.debug,
        Some(device_manager),
        config.detail_level.clone(),
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

    if config.source == SourceType::Candump {
        drop(pipeline);
        let _ = renderer_handle.await;
        println!("\nShutting down...");
    } else {
        tokio::signal::ctrl_c().await.unwrap();
        println!("\nShutting down...");
    }

    Ok(())
}
