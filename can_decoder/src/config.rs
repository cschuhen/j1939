use clap::Parser;
use std::path::PathBuf;

/// Input source type selection.
#[derive(clap::ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum SourceType {
    Socketcan,
    Candump,
}

/// Output detail level for visibility.
#[derive(clap::ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum DetailLevel {
    Raw,
    Assembled,
    Both,
}

/// Output format selection (CLI only; TUI has its own renderer).
#[derive(clap::ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Console,
    Json,
    Csv,
    Condensed,
    FullCondensed,
}

/// Panel layout orientation for the TUI.
#[derive(clap::ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum LayoutOrientation {
    Horizontal,
    Vertical,
}

/// Shared CLI configuration used by both the CLI app and TUI app.
#[derive(Parser, Debug, Clone)]
pub struct SharedConfig {
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

    /// Use proprietary DDI definitions by name (repeatable; checked in order specified). Available: canot
    #[arg(long)]
    pub use_proprietary_ddi_definitions: Vec<String>,
}

/// TUI-specific CLI arguments (extends SharedConfig with TUI-only options).
#[derive(Parser, Debug, Clone)]
pub struct TuiCli {
    #[command(flatten)]
    pub shared: SharedConfig,

    /// Panel layout orientation: horizontal | vertical
    #[arg(long, default_value = "horizontal")]
    pub layout: LayoutOrientation,
}

/// Validate proprietary DDI definition names and print available options if invalid.
pub fn validate_proprietary_definitions(names: &[String]) -> Vec<String> {
    let available = vec![("canot", "CANoT proprietary DDI definitions")];

    let mut valid_names = Vec::new();
    for name in names {
        match available.iter().find(|(n, _)| *n == name.as_str()) {
            Some((_n, _desc)) => {
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
