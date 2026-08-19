//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel dockable layout:
//! LHS filter widgets, center message list, RHS detail panel.

use std::sync::{Arc, Mutex};

use can_decoder::device_manager::DeviceManager;
use can_decoder::formats;
use can_decoder::types::DecodedMessage;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, Styled, Window,
};
use j1939_async::Id;
use tokio::sync::mpsc;

use super::app_state::AppState;
use super::components::column_toggle_panel::ColumnConfigPopup;
use super::components::filter_panel::FilterPanel as NewFilterPanel;
use super::components::message_list::MessageList;
use super::components::status_bar::StatusBar;

use super::keybindings::{
    ClearMessages, CloseColumns, PageDown, PageUp, ScrollDown, ScrollUp, SelectRow,
};

/// Root view — three-panel layout with status bars.
pub struct MainView {
    app_state: AppState,
    msg_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<DecodedMessage>>>,
    message_list: Entity<MessageList>,
    filter_panel: Entity<NewFilterPanel>,
    detail_panel: Entity<DetailPanel>,
    last_detail_index: Option<usize>,
    column_config_popup: Option<Entity<ColumnConfigPopup>>,
    receiver_started: bool,
    focus_handle: FocusHandle,
}

impl MainView {
    pub fn new(
        app_state: AppState,
        msg_rx: mpsc::UnboundedReceiver<DecodedMessage>,
        message_list: Entity<MessageList>,
        device_manager: Arc<Mutex<DeviceManager>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter_panel = cx.new(|cx| NewFilterPanel::new(message_list.clone(), cx));
        let detail_panel = cx.new(|cx| DetailPanel::new(device_manager.clone(), cx));
        MainView {
            app_state,
            msg_rx: Arc::new(tokio::sync::Mutex::new(msg_rx)),
            message_list,
            filter_panel,
            detail_panel,
            last_detail_index: None,
            column_config_popup: None,
            receiver_started: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Show or hide the column config popup.
    pub fn toggle_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.column_config_popup.is_some() {
            self.column_config_popup.take();
            // Return focus to the main view so message-list keys work again.
            window.focus(&self.focus_handle, cx);
            cx.notify();
        } else {
            let message_list = self.message_list.clone();
            let popup = cx.new(|cx| {
                let config = message_list.read(cx).column_config().clone();
                ColumnConfigPopup::new(config, message_list, cx)
            });
            // Move focus into the popup so its keybindings (arrows/space) are
            // dispatched to it instead of the message list.
            let popup_focus = popup.read(cx).focus_handle.clone();
            window.focus(&popup_focus, cx);
            self.column_config_popup = Some(popup);
            cx.notify();
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
    selected_message: Option<DecodedMessage>,
    device_manager: Arc<Mutex<DeviceManager>>,
}

impl DetailPanel {
    pub fn new(device_manager: Arc<Mutex<DeviceManager>>, _cx: &mut Context<Self>) -> Self {
        Self {
            selected_message: None,
            device_manager,
        }
    }

    pub fn set_selected_message(&mut self, msg: Option<DecodedMessage>, cx: &mut Context<Self>) {
        self.selected_message = msg;
        cx.notify();
    }
}

impl Render for DetailPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let msg = self.selected_message.clone();
        let device_manager = self.device_manager.clone();

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
                    Some(msg) => render_detail(msg, &device_manager).into_any_element(),
                }),
            )
    }
}

fn section_header(label: &str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(gpui::rgb(0x8888cc))
        .mb_1()
        .child(label.to_string())
}

fn render_detail(
    msg: &DecodedMessage,
    device_manager: &Arc<Mutex<DeviceManager>>,
) -> impl IntoElement {
    let assembled = &msg.assembled_message;
    let pgn_hex = format!("{:#05X}", assembled.pgn());
    let can_id_hex = format!("{:08X}", assembled.id);
    let time_str = formats::format_timestamp(assembled.timestamp);

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
                .child(detail_row("Time", &time_str))
                .child(detail_row("PGN", &pgn_hex))
                .child(detail_row("CAN ID", &can_id_hex)),
        )
        .child(
            div()
                .mb_2()
                .flex_col()
                .gap_1()
                .child(render_device_row(
                    "Source",
                    assembled.source(),
                    device_manager,
                    assembled.source_name,
                ))
                .child(render_device_row(
                    "Dest",
                    assembled.destination(),
                    device_manager,
                    assembled.dest_name,
                )),
        )
        .when(!assembled.data.is_empty(), |this| {
            this.child(
                div()
                    .mb_2()
                    .child(section_header(&format!(
                        "Data ({} bytes)",
                        assembled.data.len()
                    )))
                    .child(render_hex_dump(&assembled.data)),
            )
        })
        .when(!msg.outputs.is_empty(), |this| {
            this.child(
                div()
                    .mb_2()
                    .child(section_header("Outputs"))
                    .children(msg.outputs.iter().map(|output| render_output(output))),
            )
        })
        .when(!msg.updates.is_empty(), |this| {
            this.child(
                div()
                    .mb_2()
                    .child(section_header("Updates"))
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

fn resolve_device_name(device_manager: &Arc<Mutex<DeviceManager>>, address: u8) -> Option<String> {
    device_manager
        .lock()
        .ok()?
        .get_device(address)?
        .name
        .clone()
}

fn render_device_row(
    label: &str,
    address: u8,
    device_manager: &Arc<Mutex<DeviceManager>>,
    raw_name: Option<u64>,
) -> impl IntoElement {
    let is_broadcast = address == 0xFF;
    let name = if is_broadcast {
        Some("(broadcast)".to_string())
    } else {
        resolve_device_name(device_manager, address)
    };

    let mut value = format!("{:02X}h", address);
    if let Some(name) = &name {
        value.push_str(&format!("  {}", name));
    }
    if let Some(raw) = raw_name {
        value.push_str(&format!("  NAME 0x{:016X}", raw));
    }

    detail_row(label, &value)
}

fn render_hex_dump(data: &[u8]) -> impl IntoElement {
    div()
        .flex_col()
        .gap_1()
        .children(data.chunks(16).enumerate().map(|(row_idx, chunk)| {
            let offset = row_idx * 16;
            div()
                .flex_row()
                .child(
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(gpui::rgb(0x555577))
                        .child(format!("{:04X}  ", offset)),
                )
                .child(div().flex_row().children(chunk.iter().map(|byte| {
                    let color = if *byte == 0 {
                        gpui::rgb(0x555577)
                    } else {
                        gpui::rgb(0xcccccc)
                    };
                    div()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(color)
                        .child(format!("{:02X} ", byte))
                })))
        }))
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

        let msg_count = self.message_list.read(cx).total_count();
        let top_bar = StatusBar::new("can_decoder GPUI | Press Ctrl+Q to quit".into(), true);
        let bottom_bar = StatusBar::new(format!("{} messages received", msg_count).into(), false);

        let filter_panel = self.filter_panel.clone();

        // Push the selected message into the persistent DetailPanel only when
        // the selection index changes (avoids re-rendering on every frame).
        {
            let list = self.message_list.read(cx);
            let sel_idx = list.selected_index;
            if sel_idx != self.last_detail_index {
                self.last_detail_index = sel_idx;
                let selected_msg = list.selected_message().cloned();
                self.detail_panel.update(cx, |panel, _cx| {
                    panel.selected_message = selected_msg;
                });
            }
        }

        let popup_open = self.column_config_popup.is_some();
        let main_view: Entity<Self> = cx.entity().clone();

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
                    .child(
                        div()
                            .relative()
                            .h_full()
                            .w_full()
                            .child(self.message_list.clone())
                            .child({
                                let gear_view = main_view.clone();
                                div()
                                    .absolute()
                                    .top(px(0.0))
                                    .right(px(0.0))
                                    .w_8()
                                    .h_8()
                                    .flex_row()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_sm()
                                    .text_color(gpui::rgb(0x9999bb))
                                    .hover(|this| {
                                        this.bg(gpui::rgb(0x2a2a4e)).text_color(gpui::rgb(0xffffff))
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                        // Suppress GPUI's mouse-down auto-focus so the main
                                        // view doesn't steal focus back from the popup.
                                        window.prevent_default();
                                        gear_view.update(cx, |this, cx| {
                                            this.toggle_columns(window, cx);
                                        });
                                    })
                                    .child("\u{2699}")
                            }),
                    )
                    .child(self.detail_panel.clone()),
            )
            .child(bottom_bar.render())
            .when(popup_open, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .block_mouse_except_scroll()
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(hsla(0.0, 0.0, 0.0, 0.4))
                                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                    main_view.update(cx, |this, cx| {
                                        this.toggle_columns(window, cx);
                                    });
                                }),
                        )
                        .child(
                            div()
                                .absolute()
                                .top(px(40.0))
                                .left(px(250.0))
                                .child(self.column_config_popup.clone().unwrap()),
                        ),
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
            // CloseColumns bubbles up from the column config popup (Esc / X button).
            .on_action(cx.listener(|this, _: &CloseColumns, window, cx| {
                if this.column_config_popup.is_some() {
                    this.toggle_columns(window, cx);
                }
            }))
            .on_key_down({
                let main_view: Entity<Self> = cx.entity().clone();
                move |e, window, cx| {
                    if e.keystroke.key.as_str() == "escape" {
                        main_view.update(cx, |this, cx| {
                            if this.column_config_popup.is_some() {
                                this.toggle_columns(window, cx);
                            }
                        });
                    } else if e.keystroke.modifiers.control
                        && e.keystroke.modifiers.shift
                        && e.keystroke.key.as_str() == "c"
                    {
                        main_view.update(cx, |this, cx| this.toggle_columns(window, cx));
                    }
                }
            })
    }
}
