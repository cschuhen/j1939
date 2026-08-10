//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel dockable layout:
//! LHS filter widgets, center message list, RHS detail panel.

use std::sync::Arc;

use can_decoder::types::DecodedMessage;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window,
};
use j1939_async::Id;
use tokio::sync::mpsc;

use super::app_state::AppState;
use super::components::filter_widget::FilterWidget;
use super::components::message_list::MessageList;
use super::components::status_bar::StatusBar;

use can_decoder::filter_editor::FieldType;

use super::keybindings::{ScrollDown, ScrollUp, SelectRow};

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
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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

/// RHS detail panel — shows decoded message details for selected row.
pub struct DetailPanel {
    selected_message: Option<can_decoder::types::DecodedMessage>,
}

impl DetailPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            selected_message: None,
        }
    }

    pub fn set_selected_message(
        &mut self,
        msg: Option<can_decoder::types::DecodedMessage>,
        cx: &mut Context<Self>,
    ) {
        self.selected_message = msg;
        cx.notify();
    }
}

impl Render for DetailPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let msg = self.selected_message.clone();

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
                div().flex_1().p_3().child(match &msg {
                    None => div()
                        .text_xs()
                        .text_color(gpui::rgb(0x555577))
                        .child("No message selected")
                        .into_any_element(),
                    Some(msg) => render_detail(msg).into_any_element(),
                }),
            )
    }
}

fn render_detail(msg: &can_decoder::types::DecodedMessage) -> impl IntoElement {
    let assembled = &msg.assembled_message;
    let pgn_hex = format!("{:#05X}", assembled.pgn());
    let src_hex = format!("{:02X}", assembled.source());
    let dst_hex = format!("{:02X}", assembled.destination());

    div()
        .flex_col()
        .gap_2()
        .child(
            div().mb_2().child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(gpui::rgb(0xffffff))
                    .child(msg.title.clone()),
            ),
        )
        .child(
            div()
                .mb_2()
                .flex_col()
                .gap_1()
                .child(detail_row("PGN", &pgn_hex))
                .child(detail_row("Source", &src_hex))
                .child(detail_row("Dest", &dst_hex)),
        )
        .when(msg.outputs.len() > 0, |this| {
            this.child(
                div()
                    .mb_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(gpui::rgb(0x8888cc))
                            .mb_1()
                            .child("Outputs"),
                    )
                    .children(msg.outputs.iter().map(|output| render_output(output))),
            )
        })
        .when(msg.updates.len() > 0, |this| {
            this.child(
                div()
                    .mb_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(gpui::rgb(0x8888cc))
                            .mb_1()
                            .child("Updates"),
                    )
                    .children(msg.updates.iter().map(|update| render_update(update))),
            )
        })
}

fn detail_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex_row()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(gpui::rgb(0x8888aa))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_family("monospace")
                .text_color(gpui::rgb(0xcccccc))
                .child(value.to_string()),
        )
}

fn render_output(output: &can_decoder::types::DecodedField) -> impl IntoElement {
    match output {
        can_decoder::types::DecodedField::Value {
            title,
            value,
            unit,
            decimal_places,
        } => {
            let value_str = format_value(value, *decimal_places);
            let unit_str = unit.clone().unwrap_or_default();

            div()
                .mb_1()
                .px_2()
                .py_1()
                .rounded(px(3.0))
                .bg(gpui::rgb(0x1a1a3e))
                .child(
                    div()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(gpui::rgb(0xaabbcc))
                                .child(title.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(gpui::rgb(0xcccccc))
                                .child(format!("{} {}", value_str, unit_str)),
                        ),
                )
        }
        can_decoder::types::DecodedField::StringMessage { severity, text } => {
            let color = match severity {
                can_decoder::types::Severity::Info => gpui::rgb(0x88aacc),
                can_decoder::types::Severity::Warning => gpui::rgb(0xccaa44),
                can_decoder::types::Severity::Error => gpui::rgb(0xcc6644),
            };

            div()
                .mb_1()
                .px_2()
                .py_1()
                .rounded(px(3.0))
                .bg(gpui::rgb(0x1a1a3e))
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(color)
                        .child(text.clone()),
                )
        }
    }
}

fn render_update(update: &can_decoder::types::DeviceUpdate) -> impl IntoElement {
    let value_str = match &update.value {
        can_decoder::types::Numeric::Int(v) => format!("{}", v),
        can_decoder::types::Numeric::Float(v) => format!("{:.2}", v),
        can_decoder::types::Numeric::Hex(v) => format!("{:#X}", v),
        can_decoder::types::Numeric::Flag(v) => format!("{:?}", v),
        can_decoder::types::Numeric::Bool(v) => format!("{}", v),
    };

    div()
        .mb_1()
        .px_2()
        .py_1()
        .rounded(px(3.0))
        .bg(gpui::rgb(0x1a1a3e))
        .child(
            div()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(gpui::rgb(0xaabbcc))
                        .child(format!("Param {:04X}", update.param_id)),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(gpui::rgb(0xcccccc))
                        .child(value_str),
                ),
        )
}

fn format_value(value: &can_decoder::types::Numeric, decimal_places: Option<u8>) -> String {
    let places = decimal_places.unwrap_or(0);
    match value {
        can_decoder::types::Numeric::Int(v) => {
            if places > 0 {
                format!("{:.1}", *v as f64 / 10.0_f64.powi(places as i32))
            } else {
                format!("{}", v)
            }
        }
        can_decoder::types::Numeric::Float(v) => {
            format!("{:.*}", places as usize, v)
        }
        can_decoder::types::Numeric::Hex(v) => {
            format!("{:#X}", v)
        }
        can_decoder::types::Numeric::Flag(v) => {
            format!("{:?}", v)
        }
        can_decoder::types::Numeric::Bool(v) => {
            format!("{}", v)
        }
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
        let detail_panel: Entity<DetailPanel> = cx.new(DetailPanel::new);

        let selected_msg = message_list_entity.read(cx).selected_message().cloned();
        detail_panel.update(cx, |panel, _cx| {
            panel.selected_message = selected_msg;
        });

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
                    .child(message_list_entity.clone())
                    .child(detail_panel),
            )
            .child(bottom_bar.render())
            .on_action(cx.listener(|this, _: &ScrollUp, _window, cx| {
                this.message_list.update(cx, |list, _cx| {
                    list.select_prev();
                });
            }))
            .on_action(cx.listener(|this, _: &ScrollDown, _window, cx| {
                this.message_list.update(cx, |list, _cx| {
                    list.select_next();
                });
            }))
            .on_action(cx.listener(|this, _: &SelectRow, _window, cx| {
                this.message_list.update(cx, |list, _cx| {
                    list.toggle_selection();
                });
            }))
    }
}
