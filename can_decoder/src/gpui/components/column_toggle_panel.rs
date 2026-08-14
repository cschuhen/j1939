//! Column configuration popup — GPUI centered modal for toggling column visibility.
//!
//! Modeled after rgitui's settings window pattern: centered modal with backdrop,
//! section-based layout, card-style entries, and direct keyboard handling.

use std::rc::Rc;

use can_decoder::columns::{Column, ColumnConfig};
use gpui::prelude::*;
use gpui::{
    div, px, Context, Entity, FocusHandle, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Render, Styled, Window,
};

use super::super::keybindings::{CancelFilterEdit, CloseColumns, ScrollDown, ScrollUp, SelectRow};

/// Column configuration popup state.
pub struct ColumnConfigPopup {
    pub column_config: ColumnConfig,
    pub selected_index: usize,
    pub focus_handle: FocusHandle,
    message_list: Option<Rc<Entity<super::message_list::MessageList>>>,
}

impl ColumnConfigPopup {
    /// Create a new column config popup from the given config and message list reference.
    pub fn new(
        column_config: ColumnConfig,
        message_list: Entity<super::message_list::MessageList>,
        cx: &mut Context<Self>,
    ) -> Self {
        let all_columns = Column::all();
        // Start selection on first enabled column, or first column overall
        let selected_index = all_columns
            .iter()
            .position(|col| {
                if let Some(state) = column_config.states.iter().find(|s| s.column == *col) {
                    state.enabled
                } else {
                    col.default_enabled()
                }
            })
            .unwrap_or(0);

        ColumnConfigPopup {
            column_config,
            selected_index,
            focus_handle: cx.focus_handle(),
            message_list: Some(Rc::new(message_list)),
        }
    }

    /// Get the focus handle for this popup.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// Toggle a column's visibility and immediately apply to MessageList.
    pub fn toggle_column(&mut self, column: Column, cx: &mut Context<Self>) {
        self.column_config.toggle(column);
        if let Some(ref msg_list) = self.message_list {
            msg_list.update(cx, |list, _| {
                list.column_config = self.column_config.clone();
            });
        }
    }

    /// Move selection up by one row.
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down by one row.
    pub fn select_next(&mut self) {
        let count = Column::all().len();
        if self.selected_index + 1 < count {
            self.selected_index += 1;
        }
    }

    /// Toggle the currently selected column.
    pub fn toggle_selected(&mut self, cx: &mut Context<Self>) {
        let columns = Column::all();
        if self.selected_index < columns.len() {
            self.toggle_column(columns[self.selected_index], cx);
        }
    }

    fn close_and_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Changes already applied live via toggle_column — just close
        window.dispatch_action(Box::new(CloseColumns), cx);
    }

    fn close_and_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Changes already applied live — just close
        window.dispatch_action(Box::new(CloseColumns), cx);
    }
}

impl Render for ColumnConfigPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let columns = Column::all();

        div()
            .id("column-config-popup")
            .track_focus(&self.focus_handle)
            .absolute()
            .top(px(40.0))
            .left(px(250.0))
            .w_72()
            .bg(gpui::rgb(0x1e1e32))
            .border_1()
            .border_color(gpui::rgb(0x333355))
            .rounded(px(8.0))
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
            })
            .on_action(cx.listener(|this, _: &ScrollUp, _window, cx| {
                this.select_prev();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ScrollDown, _window, cx| {
                this.select_next();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SelectRow, window, cx| {
                this.toggle_selected(cx);
                window.focus(&this.focus_handle, cx);
            }))
            .on_action(cx.listener(|this, _: &CancelFilterEdit, window, cx| {
                this.close_and_cancel(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseColumns, window, cx| {
                this.close_and_commit(window, cx);
            }))
            .child(
                // Header bar
                div()
                    .h_9()
                    .w_full()
                    .bg(gpui::rgb(0x282848))
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(gpui::rgb(0x333355))
                    .child(
                        div().flex_row().gap_2().child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(gpui::rgb(0xffffff))
                                .child("Configure Columns"),
                        ),
                    )
                    .child(
                        div()
                            .w_6()
                            .h_6()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(gpui::rgb(0x8888aa))
                            .hover(|this| {
                                this.bg(gpui::rgb(0x3a3a5e)).text_color(gpui::rgb(0xffffff))
                            })
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                window.dispatch_action(Box::new(CloseColumns), cx);
                            })
                            .child("x"),
                    ),
            )
            // Column list area
            .child(
                div()
                    .h_full()
                    .flex_1()
                    .children(columns.iter().enumerate().map(|(i, col)| {
                        let is_selected = i == self.selected_index;
                        let enabled = if let Some(state) =
                            self.column_config.states.iter().find(|s| s.column == *col)
                        {
                            state.enabled
                        } else {
                            col.default_enabled()
                        };

                        let entity: Entity<Self> = cx.entity().clone();
                        div()
                            .id(format!("column-row-{}", i as u32))
                            .h_8()
                            .w_full()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .gap_3()
                            .cursor_pointer()
                            .bg(if is_selected {
                                gpui::rgb(0x2a2a5e)
                            } else if i % 2 == 0 {
                                gpui::rgb(0x1a1a30)
                            } else {
                                gpui::rgb(0x1c1c34)
                            })
                            .border_l_2()
                            .border_color(if is_selected {
                                gpui::rgb(0x4a9eff)
                            } else {
                                gpui::rgb(0x0f0f23)
                            })
                            .hover(|this| this.bg(gpui::rgb(0x252550)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                entity.update(cx, |popup, cx| {
                                    popup.selected_index = i;
                                    popup.toggle_column(*col, cx);
                                });
                            })
                            .child(
                                div()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        // Checkbox indicator
                                        div()
                                            .w_4()
                                            .h_4()
                                            .flex_row()
                                            .items_center()
                                            .justify_center()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(if enabled {
                                                gpui::rgb(0x4a9eff)
                                            } else {
                                                gpui::rgb(0x333355)
                                            })
                                            .bg(if enabled {
                                                gpui::rgb(0x4a9eff)
                                            } else {
                                                gpui::rgb(0x141428)
                                            })
                                            .when(enabled, |this| {
                                                this.child(
                                                    div()
                                                        .w_2()
                                                        .h_2()
                                                        .rounded_sm()
                                                        .bg(gpui::rgb(0xffffff)),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(if enabled {
                                                gpui::rgb(0xcccccc)
                                            } else {
                                                gpui::rgb(0x555577)
                                            })
                                            .child(col.label().to_string()),
                                    ),
                            )
                            // Width hint on the right side
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(gpui::rgb(0x444466))
                                    .child(format!("w={}", col.base_width())),
                            )
                    })),
            )
            // Footer bar with keyboard hints
            .child(
                div()
                    .h_8()
                    .w_full()
                    .bg(gpui::rgb(0x282848))
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_t_1()
                    .border_color(gpui::rgb(0x333355))
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(gpui::rgb(0x666688))
                            .child("[↑↓] Navigate  [Space] Toggle"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(gpui::rgb(0x666688))
                            .child("[Esc] Close"),
                    ),
            )
    }
}
