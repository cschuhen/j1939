//! Column toggle panel — GPUI popup for enabling/disabling columns.
//!
//! Phase 3 implementation: allows users to configure which columns are visible
//! in the message list using shared ColumnConfig from columns.rs.

use can_decoder::columns::{Column, ColumnConfig};
use gpui::prelude::*;
use gpui::{
    div, px, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled, Window,
};

/// Column toggle panel state.
pub struct ColumnTogglePanel {
    column_config: ColumnConfig,
    selected_index: Option<usize>,
    focus_handle: FocusHandle,
}

impl ColumnTogglePanel {
    /// Create a new empty ColumnTogglePanel.
    pub fn new(column_config: ColumnConfig, cx: &mut Context<Self>) -> Self {
        ColumnTogglePanel {
            column_config,
            selected_index: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Get the focus handle for this panel.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// Toggle a column's visibility.
    pub fn toggle_column(&mut self, column: Column) {
        self.column_config.toggle(column);
    }

    /// Move selection up by one row.
    pub fn select_prev(&mut self) {
        let count = Column::all().len();
        if count == 0 {
            return;
        }

        if self.selected_index.is_none() {
            self.selected_index = Some(0);
        } else if let Some(idx) = self.selected_index {
            if idx > 0 {
                self.selected_index = Some(idx - 1);
            }
        }
    }

    /// Move selection down by one row.
    pub fn select_next(&mut self) {
        let count = Column::all().len();
        if count == 0 {
            return;
        }

        if self.selected_index.is_none() {
            self.selected_index = Some(0);
        } else if let Some(idx) = self.selected_index {
            if idx + 1 < count {
                self.selected_index = Some(idx + 1);
            }
        }
    }

    /// Toggle the currently selected column.
    pub fn toggle_selected(&mut self) {
        if let Some(idx) = self.selected_index {
            let columns = Column::all();
            if idx < columns.len() {
                self.toggle_column(columns[idx]);
            }
        }
    }

    /// Get the current column configuration.
    pub fn column_config(&self) -> &ColumnConfig {
        &self.column_config
    }
}

impl Render for ColumnTogglePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let columns = Column::all();

        div()
            .absolute()
            .top(px(40.0))
            .left(px(250.0))
            .w_72()
            .bg(gpui::rgb(0x1a1a2e))
            .border_1()
            .border_color(gpui::rgb(0x333355))
            .rounded(px(6.0))
            .shadow_lg()
            .flex_col()
            .child(
                div()
                    .h_8()
                    .w_full()
                    .bg(gpui::rgb(0x2a2a4e))
                    .flex_row()
                    .items_center()
                    .px_3()
                    .border_b_1()
                    .border_color(gpui::rgb(0x333355))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(gpui::rgb(0xffffff))
                            .child("Configure Columns"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .children(columns.iter().enumerate().map(|(i, col)| {
                        let is_selected = Some(i) == self.selected_index;
                        let enabled = if let Some(state) =
                            self.column_config.states.iter().find(|s| s.column == *col)
                        {
                            state.enabled
                        } else {
                            col.default_enabled()
                        };

                        let entity: Entity<Self> = cx.entity().clone();
                        div()
                            .h_7()
                            .w_full()
                            .flex_row()
                            .items_center()
                            .px_3()
                            .gap_2()
                            .cursor_pointer()
                            .bg(if is_selected {
                                gpui::rgb(0x1a3a5f)
                            } else if i % 2 == 0 {
                                gpui::rgb(0x141428)
                            } else {
                                gpui::rgb(0x161630)
                            })
                            .hover(|this| this.bg(gpui::rgb(0x1f4570)))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                entity.update(cx, |panel, _| {
                                    panel.selected_index = Some(i);
                                });
                            })
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .flex_row()
                                    .items_center()
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
                                        gpui::rgb(0x1a1a2e)
                                    })
                                    .child(
                                        div()
                                            .w_2()
                                            .h_2()
                                            .rounded_sm()
                                            .bg(gpui::rgb(0xffffff))
                                            .when(!enabled, |this| this.opacity(0.3)),
                                    ),
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
                            )
                    })),
            )
            .child(
                div()
                    .h_8()
                    .w_full()
                    .bg(gpui::rgb(0x2a2a4e))
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .border_t_1()
                    .border_color(gpui::rgb(0x333355))
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(gpui::rgb(0x8888aa))
                            .child("[Space] Toggle  [Esc] Close"),
                    ),
            )
    }
}
