//! Application state for GPUI frontend.
//!
//! Holds shared references to core subsystems (pipeline, device manager, filter engine).

use std::path::PathBuf;

/// Root application state.
#[derive(Clone)]
pub struct AppState {
    pub config_dir: PathBuf,
}

impl AppState {
    /// Create a new AppState.
    /// TODO (Phase 2): Add FilterEngine, DeviceManager, ScrollManager references
    pub fn new(config_dir: &PathBuf) -> Self {
        AppState {
            config_dir: config_dir.clone(),
        }
    }
}
