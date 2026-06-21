use std::sync::Arc;
use can_decoder::{Cli, DetailLevel, OutputFormat, SourceType};
use can_decoder::pipeline::{ConsoleRenderer, NullDecoder, PassThroughFilter, Pipeline};
use can_decoder::sources::{CandumpFileSource, SocketCanSource};
use can_decoder::traits::Source;
use clap::Parser;

/// Entry point: parse CLI args and print configuration summary.
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

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
    let decoder = Box::new(can_decoder::pipeline::NullDecoder { debug: cli.debug });
    pipeline.spawn_decoder(decoder);

    let filter = Arc::new(tokio::sync::Mutex::new(PassThroughFilter));
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);

    let renderer = Box::new(ConsoleRenderer);
    Pipeline::spawn_renderer(filter_rx, renderer);

    println!("Pipeline ready. Press Ctrl+C to stop.");

    if cli.source == SourceType::Candump {
        // Batch mode: wait for source to finish processing the file
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        println!("\nShutting down...");
    } else {
        // Live mode: wait for Ctrl+C
        tokio::signal::ctrl_c().await.unwrap();
        println!("\nShutting down...");
    }
}
