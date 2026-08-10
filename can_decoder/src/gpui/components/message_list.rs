//! Message list component — GPUI UniformList-based rendering of decoded CAN messages.
//!
//! Phase 2 implementation target. Currently a stub.

use gpui::{IntoElement, Render, Window};

/// Entity holding message list state.
pub struct MessageList {
    // TODO: Add scroll handle, selected index, etc. (Phase 2)
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // Stub — Phase 2 implementation
        gpui::div()
    }
}
