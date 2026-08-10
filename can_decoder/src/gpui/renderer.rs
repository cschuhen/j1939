//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel layout skeleton.

use can_decoder::types::DecodedMessage;
use gpui::{div, IntoElement, ParentElement, Render, Styled, Window};
use tokio::sync::mpsc;

use super::app_state::AppState;
use super::components::status_bar::StatusBar;

/// Root view — three-panel layout with status bars.
pub struct MainView {
    app_state: AppState,
    msg_rx: mpsc::UnboundedReceiver<DecodedMessage>,
}

impl MainView {
    pub fn new(
        app_state: AppState,
        msg_rx: mpsc::UnboundedReceiver<DecodedMessage>,
    ) -> Self {
        MainView { app_state, msg_rx }
    }
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let top_bar = StatusBar::new("can_decoder GPUI | Press Ctrl+Q to quit".into(), true);
        let bottom_bar = StatusBar::new("Ready".into(), false);

        div()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x000000))
            .child(top_bar.render())
            .child(
                div().flex_row().size_full().bg(gpui::rgb(0x262626)).child(
                    div()
                        .flex_1()
                        .flex_col()
                        .justify_center()
                        .items_center()
                        .child(div().text_xl().text_color(gpui::rgb(0xFFFFFF)).child("can_decoder GPUI"))
                        .child(
                            div()
                                .mt_4()
                                .text_color(gpui::rgb(0x999999))
                                .child("Phase 1 - Foundation & Skeleton"),
                        ),
                ),
            )
            .child(bottom_bar.render())
    }
}
