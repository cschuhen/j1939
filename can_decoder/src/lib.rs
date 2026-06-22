pub mod device_manager;
/// Async channel-based pipeline wiring stages together.
pub mod pipeline;
/// Input sources (SocketCAN, candump files).
pub mod sources;
/// Core traits defining the pipeline architecture.
pub mod traits;
/// J1939 Transport Protocol reassembly module.
pub mod tp_reassembler;
/// PGN decoder engine with YAML configuration support.
pub mod pgn_decoder;
/// Data models for CAN frames, messages, and decoded output.
pub mod types;

use clap::Parser;
use std::path::PathBuf;

/// CLI argument parser. Defines all options from the requirements plan.
#[derive(Parser, Debug)]
#[command(name = "can_decoder", about = "J1939 CAN Frame Decoder")]
pub struct Cli {
    /// SocketCAN interface for live mode (e.g., can0)
    #[arg(short, long)]
    pub interface: Option<String>,

    /// Input source type (socketcan, candump)
    #[arg(short, long, default_value = "socketcan")]
    pub source: SourceType,

    /// Path to candump-style input file (used with --source candump)
    #[arg(long)]
    pub input_file: Option<PathBuf>,

    /// Path to YAML configuration file defining PGN interpretations
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Output detail level (raw frame bytes vs assembled/decoded output)
    #[arg(long, default_value = "assembled")]
    pub detail_level: DetailLevel,

    /// Emit partially reassembled Transport Protocol messages on timeout
    #[arg(long)]
    pub force_output_partial_tp: bool,

    /// Enable extra debugging information
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// Add a filter rule (repeatable; applied to decoded PrettyOutput)
    #[arg(long)]
    pub filter: Vec<String>,

    /// Output format (console colorized, JSON, CSV)
    #[arg(long, default_value = "console")]
    pub output_format: OutputFormat,
}

/// Input source type selection.
#[derive(clap::ValueEnum, Debug, Clone, PartialEq)]
pub enum SourceType {
    Socketcan,
    Candump,
}

/// Detail level for output visibility.
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum DetailLevel {
    Raw,
    Assembled,
    Both,
}

/// Output format selection.
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum OutputFormat {
    Console,
    Json,
    Csv,
}
