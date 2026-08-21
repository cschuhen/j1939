//! Status bar component — top and bottom status indicators.

use gpui::{div, IntoElement, ParentElement, SharedString, Styled};

/// Top or bottom status bar element factory.
pub struct StatusBar {
    text: SharedString,
    is_top: bool,
}

impl StatusBar {
    pub fn new(text: SharedString, is_top: bool) -> Self {
        StatusBar { text, is_top }
    }

    pub fn render(&self) -> impl IntoElement {
        let bg_color = if self.is_top {
            gpui::rgb(0x4A4A4A)
        } else {
            gpui::rgb(0x1E3A5F)
        };

        div()
            .flex()
            .h_6()
            .w_full()
            .bg(bg_color)
            .flex_row()
            .items_center()
            .px_4()
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::rgb(0xCCCCCC))
                    .child(self.text.clone()),
            )
    }
}
