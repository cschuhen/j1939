//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel dockable layout:
//! LHS filter widgets, center message list, RHS detail panel.

use std::sync::Arc;

use can_decoder::types::DecodedMessage;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, Styled, Window,
};
use j1939_async::Id;
use tokio::sync::mpsc;

use super::app_state::AppState;
use super::components::column_toggle_panel::ColumnTogglePanel;
use super::components::filter_panel::FilterPanel as NewFilterPanel;
use super::components::message_list::MessageList;
use super::components::status_bar::StatusBar;

use super::keybindings::{
    CancelFilterEdit, ClearMessages, CloseColumns, OpenColumns, PageDown, PageUp, ScrollDown,
    ScrollUp, SelectRow, ToggleColumns,
};

/// Root view — three-panel layout with status bars.
pub struct MainView {
    app_state: AppState,
    msg_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<DecodedMessage>>>,
    message_list: Entity<MessageList>,
    filter_panel: Entity<NewFilterPanel>,
    column_toggle_panel: Option<Entity<ColumnTogglePanel>>,
    receiver_started: bool,
    focus_handle: FocusHandle,
}

impl MainView {
    pub fn new(
        app_state: AppState,
        msg_rx: mpsc::UnboundedReceiver<DecodedMessage>,
        message_list: Entity<MessageList>,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter_panel = cx.new(|cx| NewFilterPanel::new(message_list.clone(), cx));
        let this_entity = cx.entity().clone();
        message_list.update(cx, |list, _| {
            list.set_parent_entity(this_entity);
        });
        MainView {
            app_state,
            msg_rx: Arc::new(tokio::sync::Mutex::new(msg_rx)),
            message_list,
            filter_panel,
            column_toggle_panel: None,
            receiver_started: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Show or hide the column toggle panel.
    pub fn toggle_columns(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = self.column_toggle_panel.take() {
            cx.notify();
        } else {
            let message_list = self.message_list.clone();
            let panel = cx.new(|cx| {
                let config = message_list.read(cx).column_config().clone();
                ColumnTogglePanel::new(config, cx)
            });
            self.column_toggle_panel = Some(panel);
            cx.notify();
        }
    }

    /// Update column configuration from the toggle panel to MessageList.
    fn update_columns_from_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = &self.column_toggle_panel {
            let config = panel.read(cx).column_config().clone();
            self.message_list.update(cx, |list, _| {
                list.column_config = config;
            });
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
        let filter_panel = self.filter_panel.clone();

        // Spawn a Tokio task that receives messages and updates GPUI state
        cx.spawn(async move |_this, cx| {
            let mut rx = msg_rx.lock().await;

            while let Some(msg) = rx.recv().await {
                message_list.update(cx, |list, _cx| {
                    list.add_message(msg.clone());
                });

                filter_panel.update(cx, |panel, _cx| {
                    panel.add_message(&msg);
                });

                if (rx.len() % 1000) == 0 {
                    filter_panel.update(cx, |panel, cx| {
                        panel.rebuild_items();
                        cx.notify();
                    });
                }
            }
        })
        .detach();
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
            .h_full()
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
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // Start message receiver on first render
        if !self.receiver_started {
            self.start_message_receiver(&cx);
        }

        let msg_count = self.message_list.read(cx).total_count();
        let top_bar = StatusBar::new("can_decoder GPUI | Press Ctrl+Q to quit".into(), true);
        let bottom_bar = StatusBar::new(format!("{} messages received", msg_count).into(), false);

        let filter_panel = self.filter_panel.clone();
        let detail_panel: Entity<DetailPanel> = cx.new(DetailPanel::new);

        let selected_msg = self.message_list.read(cx).selected_message().cloned();
        detail_panel.update(cx, |panel, _cx| {
            panel.selected_message = selected_msg;
        });

        let panel_focused = self.column_toggle_panel.is_some();

        if panel_focused {
            self.focus_handle.focus(window, cx);
        }

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x0f0f23))
            .child(top_bar.render())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h_0()
                    .flex_1()
                    .child(filter_panel)
                    .child(self.message_list.clone())
                    .child(detail_panel),
            )
            .child(bottom_bar.render())
            .when(panel_focused, |this| {
                this.child(div().absolute().inset_0().bg(hsla(0.0, 0.0, 0.0, 0.4)))
            })
            .when(panel_focused, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(40.0))
                        .left(px(250.0))
                        .child(self.column_toggle_panel.clone().unwrap()),
                )
            })
            .on_action(cx.listener(|this, _: &ScrollUp, _window, cx| {
                this.message_list.update(cx, |list, cx| {
                    list.select_prev(cx);
                });
            }))
            .on_action(cx.listener(|this, _: &ScrollDown, _window, cx| {
                this.message_list.update(cx, |list, cx| {
                    list.select_next(cx);
                });
            }))
            .on_action(cx.listener(|this, _: &PageUp, _window, cx| {
                this.message_list.update(cx, |list, cx| {
                    list.select_page_up(cx);
                });
            }))
            .on_action(cx.listener(|this, _: &PageDown, _window, cx| {
                this.message_list.update(cx, |list, cx| {
                    list.select_page_down(cx);
                });
            }))
            .on_action(cx.listener(|this, _: &SelectRow, _window, cx| {
                this.message_list.update(cx, |list, cx| {
                    list.toggle_selection(cx);
                });
            }))
            .on_action(cx.listener(|_this, _: &ClearMessages, _window, cx| {
                // TODO: Implement clear messages functionality
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleColumns, window, cx| {
                this.toggle_columns(cx);
                if let Some(panel) = &this.column_toggle_panel {
                    window.focus(&panel.read(cx).focus_handle(), cx);
                } else {
                    window.focus(&this.focus_handle, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ScrollUp, _window, cx| {
                if this.column_toggle_panel.is_some() {
                    if let Some(panel) = &this.column_toggle_panel {
                        panel.update(cx, |p, _| p.select_prev());
                    }
                } else {
                    this.message_list.update(cx, |list, cx| {
                        list.select_prev(cx);
                    });
                }
            }))
            .on_action(cx.listener(|this, _: &ScrollDown, _window, cx| {
                if this.column_toggle_panel.is_some() {
                    if let Some(panel) = &this.column_toggle_panel {
                        panel.update(cx, |p, _| p.select_next());
                    }
                } else {
                    this.message_list.update(cx, |list, cx| {
                        list.select_next(cx);
                    });
                }
            }))
            .on_action(cx.listener(|this, _: &SelectRow, _window, cx| {
                if let Some(panel) = &this.column_toggle_panel {
                    panel.update(cx, |p, _| p.toggle_selected());
                } else {
                    this.message_list.update(cx, |list, cx| {
                        list.toggle_selection(cx);
                    });
                }
            }))
            .on_action(cx.listener(|this, _: &CancelFilterEdit, _window, cx| {
                if this.column_toggle_panel.is_some() {
                    this.column_toggle_panel = None;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &CloseColumns, _window, cx| {
                this.column_toggle_panel = None;
                cx.notify();
            }))
    }
}
