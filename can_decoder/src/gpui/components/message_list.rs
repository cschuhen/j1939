//! Message list component — GPUI UniformList-based rendering of decoded CAN messages.
//!
//! Phase 2 implementation: displays filtered messages with keyboard navigation,
//! row selection, and column formatting using shared columns.rs.
//! Uses shared FilterEngine for message storage and filtering.

use crate::gpui::keybindings::{PageDown, PageUp, ScrollDown, ScrollUp, SelectRow};
use can_decoder::columns::{Column, ColumnConfig, ColumnState};
use can_decoder::filter_editor::FieldType;
use can_decoder::filter_engine::FilterEngine;
use can_decoder::filters::{
    DestNameFilter, PgnFilter, SourceFilter, SourceNameFilter, TitleFilter,
};

use can_decoder::types::DecodedMessage;
use gpui::{
    div, prelude::*, px, ElementId, Entity, IntoElement, MouseButton, ParentElement, Render,
    ScrollStrategy, Styled, Window,
};

// Re-export uniform_list for use in render()
pub fn make_uniform_list<R>(
    id: impl Into<gpui::ElementId>,
    item_count: usize,
    f: impl 'static + Fn(std::ops::Range<usize>, &mut gpui::Window, &mut gpui::App) -> Vec<R>,
) -> gpui::UniformList
where
    R: gpui::IntoElement,
{
    gpui::uniform_list(id, item_count, f)
}

/// Message list state — owns the FilterEngine and selection.
pub struct MessageList {
    engine: FilterEngine,
    pub selected_index: Option<usize>,
    global_start_time: Option<u64>,
    pub column_config: ColumnConfig,
    scroll_handle: gpui::UniformListScrollHandle,
}

impl MessageList {
    /// Create a new empty MessageList with default column configuration.
    pub fn new() -> Self {
        MessageList {
            engine: FilterEngine::new(),
            selected_index: None,
            global_start_time: None,
            column_config: ColumnConfig::default(),
            scroll_handle: gpui::UniformListScrollHandle::new(),
        }
    }

    /// Create a new empty MessageList with custom column configuration.
    pub fn with_column_config(column_config: ColumnConfig) -> Self {
        MessageList {
            engine: FilterEngine::new(),
            selected_index: None,
            global_start_time: None,
            column_config,
            scroll_handle: gpui::UniformListScrollHandle::new(),
        }
    }

    /// Toggle a column's visibility.
    pub fn toggle_column(&mut self, column: Column) {
        self.column_config.toggle(column);
    }

    /// Get the current column configuration.
    pub fn column_config(&self) -> &ColumnConfig {
        &self.column_config
    }

    /// Add a message and update global start time from the first message timestamp.
    pub fn add_message(&mut self, msg: DecodedMessage) {
        if self.engine.total_count() == 0 {
            self.global_start_time = Some(msg.timestamp());
        }
        self.engine.add_message(msg);
    }

    /// Clear all messages.
    pub fn clear_messages(&mut self) {
        self.engine.clear();
        self.selected_index = None;
    }

    /// Set the filter criteria and reapply to visible messages.
    pub fn set_filter(
        &mut self,
        source_addr: Option<u8>,
        _dest_addr: Option<u8>,
        pgn: Option<u32>,
        title_contains: Vec<String>,
        source_names: Vec<u64>,
        dest_names: Vec<u64>,
    ) {
        let mut filters = Vec::new();

        if let Some(src) = source_addr {
            filters.push(Box::new(SourceFilter::new(src)) as Box<dyn can_decoder::traits::Filter>);
        }
        if let Some(pgn) = pgn {
            filters.push(Box::new(PgnFilter::new(pgn)) as Box<dyn can_decoder::traits::Filter>);
        }
        for title in title_contains {
            filters
                .push(Box::new(TitleFilter::new(&title)) as Box<dyn can_decoder::traits::Filter>);
        }
        if !source_names.is_empty() {
            filters.push(Box::new(SourceNameFilter::from_list(source_names))
                as Box<dyn can_decoder::traits::Filter>);
        }
        if !dest_names.is_empty() {
            filters.push(Box::new(DestNameFilter::from_list(dest_names))
                as Box<dyn can_decoder::traits::Filter>);
        }

        self.engine.set_filters(filters);
    }

    /// Get the total message count.
    pub fn total_count(&self) -> usize {
        self.engine.total_count()
    }

    /// Get the filtered message count.
    pub fn filtered_count(&mut self) -> usize {
        self.engine.filtered_count()
    }

    /// Scroll to the last message.
    pub fn scroll_to_end(&mut self, handle: &gpui::UniformListScrollHandle) {
        let filtered_indices = self.engine.get_filtered_indices();
        if !filtered_indices.is_empty() {
            let last_idx = *filtered_indices.last().unwrap();
            handle.scroll_to_item(last_idx, ScrollStrategy::Center);
        }
    }

    /// Toggle selection on current row (select/deselect).
    pub fn toggle_selection(&mut self, cx: &mut gpui::Context<Self>) {
        let filtered_indices = self.engine.get_filtered_indices();
        if !filtered_indices.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(*filtered_indices.first().unwrap());
        } else {
            self.selected_index = None;
        }
        cx.notify();
    }

    /// Get the currently selected message.
    pub fn selected_message(&self) -> Option<&DecodedMessage> {
        if let Some(global_idx) = self.selected_index {
            self.engine.get_message_by_global_index(global_idx)
        } else {
            None
        }
    }

    /// Get the filtered indices for rendering.
    pub fn get_filtered_indices(&self) -> Vec<usize> {
        // Note: We need to mutably access engine, so this is a limitation.
        // In practice, render() will use a different approach.
        vec![]
    }

    /// Get message count for UniformList item count.
    pub fn message_count(&self) -> usize {
        self.engine.total_count()
    }

    /// Get unique filter options from the shared FilterEngine's value cache.
    pub fn get_unique_options(
        &self,
        field_type: FieldType,
    ) -> Vec<can_decoder::filter_editor::FilterOption> {
        self.engine.get_unique_options(field_type)
    }

    /// Get the scroll handle for programmatic scrolling.
    pub fn scroll_handle(&self) -> &gpui::UniformListScrollHandle {
        &self.scroll_handle
    }

    /// Move selection up by one row and ensure it's visible.
    pub fn select_prev(&mut self, cx: &mut gpui::Context<Self>) {
        let filtered_indices = self.engine.get_filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }

        let new_idx = if self.selected_index.is_none() {
            *filtered_indices.last().unwrap()
        } else if let Some(global_idx) = self.selected_index {
            if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == global_idx) {
                if pos_in_filtered > 0 {
                    *filtered_indices.get(pos_in_filtered - 1).unwrap()
                } else {
                    global_idx
                }
            } else {
                *filtered_indices.last().unwrap()
            }
        } else {
            *filtered_indices.last().unwrap()
        };

        self.selected_index = Some(new_idx);
        if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == new_idx) {
            self.scroll_handle
                .scroll_to_item(pos_in_filtered, ScrollStrategy::Center);
        }
        cx.notify();
    }

    /// Move selection down by one row and ensure it's visible.
    pub fn select_next(&mut self, cx: &mut gpui::Context<Self>) {
        let filtered_indices = self.engine.get_filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }

        let new_idx = if self.selected_index.is_none() {
            *filtered_indices.first().unwrap()
        } else if let Some(global_idx) = self.selected_index {
            if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == global_idx) {
                if pos_in_filtered + 1 < filtered_indices.len() {
                    *filtered_indices.get(pos_in_filtered + 1).unwrap()
                } else {
                    global_idx
                }
            } else {
                *filtered_indices.first().unwrap()
            }
        } else {
            *filtered_indices.first().unwrap()
        };

        self.selected_index = Some(new_idx);
        if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == new_idx) {
            self.scroll_handle
                .scroll_to_item(pos_in_filtered, ScrollStrategy::Center);
        }
        cx.notify();
    }

    /// Move selection up by one viewport page.
    pub fn select_page_up(&mut self, cx: &mut gpui::Context<Self>) {
        let filtered_indices = self.engine.get_filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }

        let page_size = 10;
        let target_pos = if let Some(global_idx) = self.selected_index {
            if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == global_idx) {
                (pos_in_filtered as isize - page_size as isize).max(0) as usize
            } else {
                0
            }
        } else {
            0
        };

        let new_idx = *filtered_indices.get(target_pos).unwrap();
        self.selected_index = Some(new_idx);
        if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == new_idx) {
            self.scroll_handle
                .scroll_to_item(pos_in_filtered, ScrollStrategy::Center);
        }
        cx.notify();
    }

    /// Move selection down by one viewport page.
    pub fn select_page_down(&mut self, cx: &mut gpui::Context<Self>) {
        let filtered_indices = self.engine.get_filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }

        let page_size = 10;
        let target_pos = if let Some(global_idx) = self.selected_index {
            if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == global_idx) {
                (pos_in_filtered as isize + page_size as isize)
                    .min(filtered_indices.len() as isize - 1) as usize
            } else {
                0
            }
        } else {
            0
        };

        let new_idx = *filtered_indices.get(target_pos).unwrap();
        self.selected_index = Some(new_idx);
        if let Some(pos_in_filtered) = filtered_indices.iter().position(|&i| i == new_idx) {
            self.scroll_handle
                .scroll_to_item(pos_in_filtered, ScrollStrategy::Center);
        }
        cx.notify();
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let selected_index = self.selected_index;
        let entity: Entity<Self> = cx.entity().clone();
        let column_config = self.column_config.clone();
        let scroll_handle = self.scroll_handle.clone();

        // Get filtered indices (this will auto-recompute if dirty)
        let filtered_indices = self.engine.get_filtered_indices().to_vec();

        // Clone messages for the closure (UniformList requires 'static data)
        let messages: Vec<_> = filtered_indices
            .iter()
            .filter_map(|&idx| self.engine.get_message_by_global_index(idx).cloned())
            .collect();

        let enabled_states = column_config
            .enabled_column_states()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        div()
            .relative()
            .flex()
            .flex_col()
            .flex_grow()
            .size_full()
            .child(
                div()
                    .h_6()
                    .w_full()
                    .bg(gpui::rgb(0x1a1a2e))
                    .border_b_1()
                    .border_color(gpui::rgb(0x333355))
                    .child(render_headers(&enabled_states)),
            )
            .child(
                make_uniform_list(
                    "message_list",
                    messages.len(),
                    move |range, _window, _cx| {
                        range
                            .map(|fi| {
                                let msg = &messages[fi];
                                let global_idx = filtered_indices[fi];
                                let is_selected = Some(global_idx) == selected_index;
                                let entity = entity.clone();
                                render_row(
                                    msg,
                                    is_selected,
                                    column_config.clone(),
                                    move |_, _, cx| {
                                        entity.update(cx, |list, _| {
                                            list.selected_index = Some(global_idx);
                                        });
                                    },
                                )
                            })
                            .collect()
                    },
                )
                .track_scroll(&scroll_handle)
                .flex_grow(),
            )
            .on_action(cx.listener(|this, _: &ScrollUp, _window, cx| {
                this.select_prev(cx);
            }))
            .on_action(cx.listener(|this, _: &ScrollDown, _window, cx| {
                this.select_next(cx);
            }))
            .on_action(cx.listener(|this, _: &PageUp, _window, cx| {
                this.select_page_up(cx);
            }))
            .on_action(cx.listener(|this, _: &PageDown, _window, cx| {
                this.select_page_down(cx);
            }))
            .on_action(cx.listener(|this, _: &SelectRow, _window, cx| {
                this.toggle_selection(cx);
            }))
    }
}

fn render_headers(enabled_columns: &[ColumnState]) -> impl IntoElement {
    let mut parts = Vec::new();

    let mut cumulative_offset = px(8.0);
    for col_state in enabled_columns {
        let label = col_state.column.label().to_string();
        let width = col_state.column.base_width();
        let px_width = px(width as f32 * 6.0);
        parts.push(
            div().relative().child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(cumulative_offset)
                    .text_xs()
                    .font_family("monospace")
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(gpui::rgb(0x8888cc))
                    .w(px_width)
                    .overflow_hidden()
                    .justify_start()
                    .py_1()
                    .child(label),
            ),
        );
        cumulative_offset = cumulative_offset + px_width + px(6.0);
    }

    div()
        .relative()
        .w_full()
        .h_6()
        .bg(gpui::rgb(0x1a1a2e))
        .border_b_1()
        .border_color(gpui::rgb(0x333355))
        .children(parts)
}

/// Render a single message row with column formatting.
fn render_row(
    msg: &DecodedMessage,
    is_selected: bool,
    column_config: ColumnConfig,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let bg_color = if is_selected {
        gpui::rgb(0x1a3a5f)
    } else {
        gpui::rgb(0x0f0f23)
    };

    let text_color = if is_selected {
        gpui::rgb(0xffffff)
    } else {
        gpui::rgb(0xcccccc)
    };

    let mut children = Vec::new();
    let mut cumulative_offset = px(8.0);
    for col_state in column_config.enabled_column_states() {
        let value = col_state.column.format(msg, 40, None);
        let width = col_state.column.base_width();
        let px_width = px(width as f32 * 6.0);
        children.push(
            div().relative().child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(cumulative_offset)
                    .text_xs()
                    .font_family("monospace")
                    .text_color(text_color)
                    .w(px_width)
                    .overflow_hidden()
                    .justify_start()
                    .child(value),
            ),
        );
        cumulative_offset = cumulative_offset + px_width + px(6.0);
    }

    div()
        .id(ElementId::Integer(msg.timestamp() as u64))
        .relative()
        .w_full()
        .h_6()
        .bg(bg_color)
        .border_b_1()
        .border_color(gpui::rgb(0x1a1a2e))
        .cursor_pointer()
        .hover(|this| {
            this.bg(if is_selected {
                gpui::rgb(0x1f4570)
            } else {
                gpui::rgb(0x1a2a4f)
            })
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .children(children)
}
