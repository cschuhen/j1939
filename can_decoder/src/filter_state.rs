//! Serializable snapshot of the TUI's active filter widgets.
//!
//! The TUI stores its filters as a list of [`FilterWidget`]s; each widget knows how to
//! build its own runtime filter from `input_text` (see `FilterWidget::build_filter`).
//! Persisting the raw widget state (type, name, enabled flag, and input text) is the
//! single source of truth for what gets saved — on restart we rebuild identical widgets
//! so the applied filters never diverge from what was last shown.

use crate::app_state::{FilterType, FilterWidget};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One persisted filter widget (a single row in the TUI's left-hand filter panel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFilter {
    pub filter_type: FilterType,
    pub name: String,
    pub enabled: bool,
    pub input_text: String,
}

/// Serializable snapshot of all filter widgets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterState {
    pub filters: Vec<SavedFilter>,
}

impl FilterState {
    /// Snapshot the current widget list for persistence.
    pub fn from_widgets(widgets: &[FilterWidget]) -> Self {
        Self {
            filters: widgets
                .iter()
                .map(|w| SavedFilter {
                    filter_type: w.filter_type.clone(),
                    name: w.name.clone(),
                    enabled: w.enabled,
                    input_text: w.input_text.clone(),
                })
                .collect(),
        }
    }

    /// Rebuild widgets from a persisted snapshot.
    pub fn to_widgets(&self) -> Vec<FilterWidget> {
        self.filters
            .iter()
            .map(|sf| {
                let mut w = FilterWidget::new(&sf.name, sf.filter_type.clone());
                w.enabled = sf.enabled;
                w.input_text = sf.input_text.clone();
                w
            })
            .collect()
    }

    /// True when no widget has an active (enabled + non-empty) filter.
    pub fn is_empty(&self) -> bool {
        self.filters
            .iter()
            .all(|f| !f.enabled || f.input_text.trim().is_empty())
    }

    /// Number of widgets that currently produce a runtime filter.
    pub fn active_count(&self) -> usize {
        self.filters
            .iter()
            .filter(|f| f.enabled && !f.input_text.trim().is_empty())
            .count()
    }
}

/// Standard config directory (`~/.config/j1939_decoder`), honoring `CAN_DECODER_CONFIG_DIR`.
pub fn default_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CAN_DECODER_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    match std::env::var("HOME") {
        Ok(home) => home.into(),
        Err(_) => PathBuf::from("."),
    }
}

/// Path to the persisted filter file for a given frontend (`"tui"` or `"gpui"`).
pub fn filters_path(frontend: &str) -> PathBuf {
    default_config_dir().join(format!("filters_{frontend}.yaml"))
}

/// Load persisted filter state from disk. `None` if missing or invalid.
pub fn load_from_disk(path: &Path) -> Option<FilterState> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Save filter state to disk, creating parent directories as needed (best-effort).
pub fn save_to_disk(state: &FilterState, path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml::to_string(state) {
        let _ = std::fs::write(path, yaml);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_widgets() {
        let mut ws = vec![
            FilterWidget::new("Title", FilterType::Title),
            FilterWidget::new("PGN", FilterType::Pgn),
        ];
        ws[1].enabled = true;
        ws[1].input_text = "65288".into();

        let state = FilterState::from_widgets(&ws);
        assert_eq!(state.active_count(), 1);
        assert!(!state.is_empty());

        let rebuilt = state.to_widgets();
        assert_eq!(rebuilt.len(), 2);
        assert_eq!(rebuilt[0].filter_type, FilterType::Title);
        assert!(!rebuilt[0].enabled);
        assert_eq!(rebuilt[1].filter_type, FilterType::Pgn);
        assert!(rebuilt[1].enabled);
        assert_eq!(rebuilt[1].input_text, "65288");
    }

    #[test]
    fn empty_state_reports_empty() {
        let state = FilterState::default();
        assert!(state.is_empty());
        assert_eq!(state.active_count(), 0);
    }

    #[test]
    fn yaml_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("filter_state_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("filters_tui.yaml");

        let mut w = FilterWidget::new("Severity", FilterType::Severity);
        w.enabled = true;
        w.input_text = "Error".into();
        let state = FilterState::from_widgets(std::slice::from_ref(&w));

        save_to_disk(&state, &path);
        let loaded = load_from_disk(&path).expect("load");
        assert_eq!(loaded, state);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_returns_none() {
        let path = std::env::temp_dir().join("filter_state_nonexistent.yaml");
        assert!(load_from_disk(&path).is_none());
    }
}
