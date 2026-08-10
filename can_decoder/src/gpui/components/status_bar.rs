//! Status bar component — top and bottom status indicators.
//!
//! Phase 5 implementation target. Currently a stub.

use gpui::{IntoElement, Render, Window};

/// Top or bottom status bar entity.
pub struct StatusBar {
    // TODO: Add mode indicator, device count, etc. (Phase 5)
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // Stub — Phase 5 implementation
        gpui::div()
    }
}
