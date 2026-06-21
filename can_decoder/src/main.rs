use std::path::PathBuf;
use std::sync::Arc;

/// Async channel-based pipeline wiring stages together.
pub mod pipeline;
/// Input sources (SocketCAN, candump files).
pub mod sources;
/// Core traits defining the pipeline architecture.
pub mod traits;
/// Data models for CAN frames, messages, and decoded output.
pub mod types;

use crate::traits::Source;
use clap::Parser;

/// CLI argument parser. Defines all options from the requirements plan.
#[derive(Parser, Debug)]
#[command(name = "can_decoder", about = "J1939 CAN Frame Decoder")]
struct Cli {
    /// SocketCAN interface for live mode (e.g., can0)
    #[arg(short, long)]
    interface: Option<String>,

    /// Input source type (socketcan, candump)
    #[arg(short, long, default_value = "socketcan")]
    source: SourceType,

    /// Path to candump-style input file (used with --source candump)
    #[arg(long)]
    input_file: Option<PathBuf>,

    /// Path to YAML configuration file defining PGN interpretations
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output detail level (raw frame bytes vs assembled/decoded output)
    #[arg(long, default_value = "assembled")]
    detail_level: DetailLevel,

    /// Emit partially reassembled Transport Protocol messages on timeout
    #[arg(long)]
    force_output_partial_tp: bool,

    /// Add a filter rule (repeatable; applied to decoded PrettyOutput)
    #[arg(long)]
    filter: Vec<String>,

    /// Output format (console colorized, JSON, CSV)
    #[arg(long, default_value = "console")]
    output_format: OutputFormat,
}

/// Input source type selection.
#[derive(clap::ValueEnum, Debug, Clone, PartialEq)]
enum SourceType {
    Socketcan,
    Candump,
}

/// Detail level for output visibility.
#[derive(clap::ValueEnum, Debug, Clone)]
enum DetailLevel {
    Raw,
    Assembled,
    Both,
}

/// Output format selection.
#[derive(clap::ValueEnum, Debug, Clone)]
enum OutputFormat {
    Console,
    Json,
    Csv,
}

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
    println!("Filters: {}", cli.filter.len());
    println!("Output format: {:?}", cli.output_format);

    let mut pipeline = pipeline::Pipeline::new();

    // Wire up the Source based on --source and available options
    match cli.source {
        SourceType::Socketcan => {
            if let Some(ref iface) = cli.interface {
                let source = Arc::new(sources::SocketCanSource::new(iface));
                println!("[{}] Starting live source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --interface required for socketcan source");
                std::process::exit(1);
            }
        }
        SourceType::Candump => {
            if let Some(ref file) = cli.input_file {
                let source = Arc::new(sources::CandumpFileSource::new(file));
                println!("[{}] Starting candump source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --input-file required for candump source");
                std::process::exit(1);
            }
        }
    }

    // Wire up Decoder → Filter → Renderer
    let decoder = Box::new(pipeline::NullDecoder);
    pipeline.spawn_decoder(decoder);

    let filter = Arc::new(tokio::sync::Mutex::new(pipeline::PassThroughFilter));
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);

    let renderer = Box::new(pipeline::ConsoleRenderer);
    pipeline::Pipeline::spawn_renderer(filter_rx, renderer);

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
