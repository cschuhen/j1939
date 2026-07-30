pub mod config;
pub mod device_manager;
/// Filter implementations and filter expression parsing.
pub mod filters;
/// PGN decoder engine with YAML configuration support.
pub mod pgn_decoder;
/// J1939 PGN decoders submodules.
pub mod pgn_decoders;
/// Async channel-based pipeline wiring stages together.
pub mod pipeline;
/// Proprietary DDI definitions module.
pub mod proprietary;
/// Output renderers (Console, JSON).
pub mod renderers;
/// Input sources (SocketCAN, candump files).
pub mod sources;
/// J1939 Transport Protocol reassembly module.
pub mod tp_reassembler;
/// Core traits defining the pipeline architecture.
pub mod traits;
/// Data models for CAN frames, messages, and decoded output.
pub mod types;

// TUI modules
pub mod tui;

// Re-export shared config types for convenience
pub use config::{DetailLevel, LayoutOrientation, OutputFormat, SharedConfig, SourceType};

use clap::Parser;

/// CLI-only argument parser (extends SharedConfig with filter + output_format).
#[derive(Parser, Debug)]
#[command(name = "can_decoder", about = "J1939 CAN Frame Decoder")]
pub struct Cli {
    #[command(flatten)]
    pub shared: SharedConfig,

    /// Add a filter rule (repeatable; applied to decoded DecodedField)
    #[arg(long)]
    pub filter: Vec<String>,

    /// Output format (console colorized, JSON, CSV)
    #[arg(long, default_value = "console")]
    pub output_format: OutputFormat,
}
