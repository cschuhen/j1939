//! Application state entity for GPUI frontend.

use gpui::Entity;
use std::path::PathBuf;

/// Root application state entity.
/// Holds shared references to core subsystems (pipeline, device manager, filter engine).
pub struct AppState {
    pub config_dir: PathBuf,
}

impl AppState {
    /// Create a new AppState entity.
    /// TODO: Initialize pipeline, device_manager, filter_engine here (Phase 1 Step 5)
    pub fn new(_config_dir: &PathBuf) -> Entity<Self> {
        // Stub — will use proper GPUI entity creation in Phase 1 Step 5
        // For now, return a minimal entity handle
        unreachable!("AppState::new requires App context from GPUI window callback")
    }
}
