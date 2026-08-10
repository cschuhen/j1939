//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel dockable layout:
//! LHS filter widgets, center message list, RHS detail panel.

use can_decoder::types::DecodedMessage;
use gpui::{div, AppContext, Entity, IntoElement, ParentElement, Render, Styled, Window};
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

/// LHS filter panel — dockable container for filter widgets.
pub struct FilterPanel;

impl Render for FilterPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .w_72()
            .flex_col()
            .bg(gpui::rgb(0x1a1a2e))
            .border_r_1()
            .border_color(gpui::rgb(0x333355))
            .child(
                div()
                    .h_6()
                    .w_full()
                    .bg(gpui::rgb(0x2a2a4e))
                    .flex_row()
                    .items_center()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0xaaaaee))
                            .child(" Filters "),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x555577))
                            .child("Filter widgets (Phase 3)"),
                    ),
            )
    }
}

/// Center message list — UniformList-based rendering of decoded messages.
pub struct MessagePanel;

impl Render for MessagePanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex_col()
            .bg(gpui::rgb(0x0f0f23))
            .child(
                div()
                    .h_6()
                    .w_full()
                    .bg(gpui::rgb(0x1a1a3e))
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0xaaaaee))
                            .child(" Messages "),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x555577))
                            .child("0 messages"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .justify_center()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::rgb(0x444466))
                            .child("Message list (Phase 2)"),
                    ),
            )
    }
}

/// RHS detail panel — shows decoded message details for selected row.
pub struct DetailPanel;

impl Render for DetailPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .w_80()
            .flex_col()
            .bg(gpui::rgb(0x1a1a2e))
            .border_l_1()
            .border_color(gpui::rgb(0x333355))
            .child(
                div()
                    .h_6()
                    .w_full()
                    .bg(gpui::rgb(0x2a2a4e))
                    .flex_row()
                    .items_center()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0xaaaaee))
                            .child(" Details "),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x555577))
                            .child("Detail panel (Phase 4)"),
                    ),
            )
    }
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let top_bar = StatusBar::new("can_decoder GPUI | Press Ctrl+Q to quit".into(), true);
        let bottom_bar = StatusBar::new("Ready".into(), false);

        let filter_panel: Entity<FilterPanel> = cx.new(|_| FilterPanel);
        let message_panel: Entity<MessagePanel> = cx.new(|_| MessagePanel);
        let detail_panel: Entity<DetailPanel> = cx.new(|_| DetailPanel);

        div()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x0f0f23))
            .child(top_bar.render())
            .child(
                div()
                    .flex_row()
                    .size_full()
                    .child(filter_panel)
                    .child(message_panel)
                    .child(detail_panel),
            )
            .child(bottom_bar.render())
    }
}
