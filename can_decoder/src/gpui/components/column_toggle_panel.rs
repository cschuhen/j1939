//! Column configuration popup — GPUI modal for toggling column visibility.
//!
//! Layout, top to bottom:
//! - Title bar with a close button
//! - One horizontal row per column: checkbox (left) + label, width hint (right)
//! - Footer with keyboard hints
//!
//! Changes are applied live to the MessageList as they are made; closing the
//! popup simply dismisses it.

use std::rc::Rc;

use can_decoder::columns::{Column, ColumnConfig};
use gpui::prelude::*;
use gpui::{
    div, px, Context, Entity, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, Styled, Window,
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
    /// Create a new popup from the given config and message list reference.
    /// Selection starts on the first enabled column (or the first column).
    pub fn new(
        column_config: ColumnConfig,
        message_list: Entity<super::message_list::MessageList>,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected_index = Column::all()
            .iter()
            .position(|col| column_enabled(&column_config, *col))
            .unwrap_or(0);

        Self {
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

    /// Whether `column` is enabled in the current config (falls back to its default).
    fn enabled(&self, column: Column) -> bool {
        column_enabled(&self.column_config, column)
    }

    /// Toggle a column's visibility and immediately apply it to the MessageList.
    pub fn toggle_column(&mut self, column: Column, cx: &mut Context<Self>) {
        self.column_config.toggle(column);
        if let Some(ref msg_list) = self.message_list {
            msg_list.update(cx, |list, _| {
                list.column_config = self.column_config.clone()
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

    /// Close the popup. Changes are already applied live via [`Self::toggle_column`].
    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(CloseColumns), cx);
    }

    /// Title bar: popup title on the left, close button on the right.
    fn render_title_bar(&self) -> impl IntoElement {
        div()
            .h_9()
            .w_full()
            .bg(gpui::rgb(0x282848))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .border_b_1()
            .border_color(gpui::rgb(0x333355))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(gpui::rgb(0xffffff))
                    .child("Configure Columns"),
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
                    .hover(|this| this.bg(gpui::rgb(0x3a3a5e)).text_color(gpui::rgb(0xffffff)))
                    .on_mouse_down(MouseButton::Left, |_event, window, cx| {
                        window.dispatch_action(Box::new(CloseColumns), cx);
                    })
                    .child("x"),
            )
    }

    /// One horizontal row per column.
    fn render_column_rows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_1().children(
            Column::all()
                .iter()
                .enumerate()
                .map(|(index, col)| self.render_row(*col, index, cx)),
        )
    }

    /// A single column row: checkbox (left), label, width hint (right).
    fn render_row(&self, column: Column, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let is_selected = index == self.selected_index;
        let enabled = self.enabled(column);
        let entity = cx.entity().clone();

        div()
            .id(format!("column-row-{}", index))
            .h_8()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .px_4()
            .gap_3()
            .cursor_pointer()
            .bg(if is_selected {
                gpui::rgb(0x2a2a5e)
            } else if index % 2 == 0 {
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
            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                entity.update(cx, |popup, cx| {
                    popup.selected_index = index;
                    popup.toggle_column(column, cx);
                });
            })
            .child(Self::render_checkbox(enabled))
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(if enabled {
                        gpui::rgb(0xcccccc)
                    } else {
                        gpui::rgb(0x555577)
                    })
                    .child(column.label().to_string()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(gpui::rgb(0x444466))
                    .child(format!("w={}", column.base_width())),
            )
    }

    /// Checkbox indicator showing the current enabled state.
    fn render_checkbox(enabled: bool) -> impl IntoElement {
        div()
            .w_4()
            .h_4()
            .flex_shrink_0()
            .flex()
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
                this.child(div().w_2().h_2().rounded_sm().bg(gpui::rgb(0xffffff)))
            })
    }

    /// Footer bar with keyboard hints.
    fn render_footer() -> impl IntoElement {
        div()
            .h_8()
            .w_full()
            .bg(gpui::rgb(0x282848))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .border_t_1()
            .border_color(gpui::rgb(0x333355))
            .child(Self::render_key_hint("[↑↓] Navigate  [Space] Toggle"))
            .child(Self::render_key_hint("[Esc] Close"))
    }

    fn render_key_hint(text: &str) -> impl IntoElement {
        div()
            .text_xs()
            .font_family("monospace")
            .text_color(gpui::rgb(0x666688))
            .child(text.to_string())
    }
}

/// Whether `column` is enabled in `config`, falling back to its default state.
fn column_enabled(config: &ColumnConfig, column: Column) -> bool {
    config
        .states
        .iter()
        .find(|s| s.column == column)
        .map(|s| s.enabled)
        .unwrap_or_else(|| column.default_enabled())
}

impl Render for ColumnConfigPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("column-config-popup")
            .track_focus(&self.focus_handle)
            .w_72()
            .bg(gpui::rgb(0x1e1e32))
            .border_1()
            .border_color(gpui::rgb(0x333355))
            .rounded(px(8.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
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
                this.close(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseColumns, window, cx| {
                this.close(window, cx);
            }))
            .child(self.render_title_bar())
            .child(self.render_column_rows(cx))
            .child(Self::render_footer())
    }
}
