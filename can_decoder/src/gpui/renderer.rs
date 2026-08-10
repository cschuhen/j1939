//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel dockable layout:
//! LHS filter widgets, center message list, RHS detail panel.

use std::sync::Arc;

use can_decoder::types::DecodedMessage;
use gpui::{div, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use tokio::sync::mpsc;

use super::app_state::AppState;
use super::components::filter_widget::FilterWidget;
use super::components::message_list::MessageList;
use super::components::status_bar::StatusBar;

use can_decoder::filter_editor::FieldType;

/// Root view — three-panel layout with status bars.
pub struct MainView {
    app_state: AppState,
    msg_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<DecodedMessage>>>,
    message_list: Entity<MessageList>,
    receiver_started: bool,
}

impl MainView {
    pub fn new(
        app_state: AppState,
        msg_rx: mpsc::UnboundedReceiver<DecodedMessage>,
        message_list: Entity<MessageList>,
    ) -> Self {
        MainView {
            app_state,
            msg_rx: Arc::new(tokio::sync::Mutex::new(msg_rx)),
            message_list,
            receiver_started: false,
        }
    }

    /// Start the message receiver task that feeds messages into MessageList.
    fn start_message_receiver(&mut self, cx: &gpui::Context<Self>) {
        if self.receiver_started {
            return;
        }
        self.receiver_started = true;

        let msg_rx = self.msg_rx.clone();
        let message_list = self.message_list.clone();

        // Spawn a Tokio task that receives messages and updates GPUI state
        cx.spawn(async move |_this, cx| {
            let mut rx = msg_rx.lock().await;
            while let Some(msg) = rx.recv().await {
                let _timestamp = msg.timestamp();
                let _ = message_list.update(cx, |list, _cx| {
                    list.add_message(msg);
                });
            }
        })
        .detach();
    }
}

/// LHS filter panel — dockable container for filter widgets.
pub struct FilterPanel {
    widgets: Vec<Entity<FilterWidget>>,
}

impl FilterPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let field_types = vec![
            FieldType::SourceAddr,
            FieldType::DestAddr,
            FieldType::Pgn,
            FieldType::Title,
            FieldType::SrcName,
            FieldType::DstName,
        ];

        let widgets: Vec<Entity<FilterWidget>> = field_types
            .into_iter()
            .map(|ft| cx.new(|cx| FilterWidget::new(ft, cx)))
            .collect();

        Self { widgets }
    }
}

impl Render for FilterPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let widget_entities = self.widgets.clone();

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
                    .children(widget_entities.into_iter().map(|w| w.into_element())),
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
        // Start message receiver on first render
        if !self.receiver_started {
            self.start_message_receiver(&cx);
        }

        let msg_count = self.message_list.read(cx).messages.len();
        let top_bar = StatusBar::new("can_decoder GPUI | Press Ctrl+Q to quit".into(), true);
        let bottom_bar = StatusBar::new(format!("{} messages received", msg_count).into(), false);

        // Keyboard navigation actions (ScrollUp, ScrollDown, SelectRow) are defined in keybindings.rs
        // and wired via cx.on_action() when the GPUI keymap system is fully configured (Phase 2 completion).
        // MessageList provides select_prev(), select_next(), toggle_selection() methods ready for wiring.

        let filter_panel: Entity<FilterPanel> = cx.new(FilterPanel::new);
        let message_list_entity = self.message_list.clone();
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
                    .child(message_list_entity)
                    .child(detail_panel),
            )
            .child(bottom_bar.render())
    }
}
