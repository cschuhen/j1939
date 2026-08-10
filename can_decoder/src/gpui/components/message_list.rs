//! Message list component — GPUI UniformList-based rendering of decoded CAN messages.
//!
//! Phase 2 implementation: displays filtered messages with keyboard navigation,
//! row selection, and column formatting using shared columns.rs.

use can_decoder::columns::Column;
use can_decoder::formats::{build_detail_string, format_elapsed_time};
use can_decoder::types::DecodedMessage;
use gpui::{
    div, prelude::*, ElementId, IntoElement, ParentElement, Render, ScrollStrategy, Styled, Window,
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

/// Default columns shown in the message list (matching TUI defaults).
const DEFAULT_COLUMNS: &[Column] = &[
    Column::Time,
    Column::Src,
    Column::Dest,
    Column::Pgn,
    Column::Title,
    Column::Detail,
];

/// Filter criteria for message filtering.
#[derive(Debug, Clone)]
pub struct MessageFilter {
    pub source_addr: Option<u8>,
    pub dest_addr: Option<u8>,
    pub pgn: Option<u32>,
    pub title_contains: Option<String>,
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self {
            source_addr: None,
            dest_addr: None,
            pgn: None,
            title_contains: None,
        }
    }
}

impl MessageFilter {
    /// Check if a message matches this filter.
    pub fn matches(&self, msg: &DecodedMessage) -> bool {
        if let Some(src) = self.source_addr {
            if msg.source_address() != src {
                return false;
            }
        }
        if let Some(dst) = self.dest_addr {
            if msg.dest_address() != dst {
                return false;
            }
        }
        if let Some(pgn) = self.pgn {
            if msg.pgn() != pgn {
                return false;
            }
        }
        if let Some(ref pattern) = self.title_contains {
            if !msg.title.contains(pattern.as_str()) {
                return false;
            }
        }
        true
    }
}

/// Message list state — owns the message buffer and selection.
pub struct MessageList {
    pub messages: Vec<DecodedMessage>,
    pub selected_index: Option<usize>,
    global_start_time: Option<u64>,
    filter: MessageFilter,
}

impl MessageList {
    /// Create a new empty MessageList.
    pub fn new() -> Self {
        MessageList {
            messages: Vec::new(),
            selected_index: None,
            global_start_time: None,
            filter: MessageFilter::default(),
        }
    }

    /// Add a message and update global start time from the first message timestamp.
    pub fn add_message(&mut self, msg: DecodedMessage) {
        if self.messages.is_empty() {
            self.global_start_time = Some(msg.timestamp());
        }
        self.messages.push(msg);
    }

    /// Clear all messages.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.selected_index = None;
    }

    /// Set the filter criteria and reapply to visible messages.
    pub fn set_filter(&mut self, filter: MessageFilter) {
        self.filter = filter;
    }

    /// Get the current filter.
    pub fn filter(&self) -> &MessageFilter {
        &self.filter
    }

    /// Scroll to the last message.
    pub fn scroll_to_end(&self, handle: &gpui::UniformListScrollHandle) {
        if !self.messages.is_empty() {
            let last_idx = self.messages.len() - 1;
            handle.scroll_to_item(last_idx, ScrollStrategy::Center);
        }
    }

    /// Move selection up by one row.
    pub fn select_prev(&mut self) {
        if self.selected_index.is_none() && !self.messages.is_empty() {
            self.selected_index = Some(self.messages.len() - 1);
        } else if let Some(idx) = self.selected_index {
            if idx > 0 {
                self.selected_index = Some(idx - 1);
            }
        }
    }

    /// Move selection down by one row.
    pub fn select_next(&mut self) {
        if self.selected_index.is_none() && !self.messages.is_empty() {
            self.selected_index = Some(0);
        } else if let Some(idx) = self.selected_index {
            if idx + 1 < self.messages.len() {
                self.selected_index = Some(idx + 1);
            }
        }
    }

    /// Toggle selection on current row (select/deselect).
    pub fn toggle_selection(&mut self) {
        self.selected_index = if self.selected_index.is_some() {
            None
        } else {
            Some(0)
        };
    }

    /// Get the currently selected message.
    pub fn selected_message(&self) -> Option<&DecodedMessage> {
        self.selected_index.and_then(|i| self.messages.get(i))
    }

    /// Get filtered messages based on current filter criteria.
    pub fn get_filtered_messages(&self) -> Vec<&DecodedMessage> {
        if self.filter.source_addr.is_none()
            && self.filter.dest_addr.is_none()
            && self.filter.pgn.is_none()
            && self.filter.title_contains.is_none()
        {
            self.messages.iter().collect()
        } else {
            self.messages
                .iter()
                .filter(|m| self.filter.matches(m))
                .collect()
        }
    }
}

impl Render for MessageList {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let messages = self.messages.clone();
        let filter = self.filter.clone();

        let filtered_indices: Vec<usize> = (0..messages.len())
            .filter(|&ix| filter.matches(&messages[ix]))
            .collect();

        make_uniform_list(
            "message_list",
            filtered_indices.len(),
            move |range, _window, _cx| {
                range
                    .map(|fi| {
                        let msg_ix = filtered_indices[fi];
                        render_row(&messages[msg_ix], false)
                    })
                    .collect()
            },
        )
        .size_full()
    }
}

/// Render a single message row with column formatting.
fn render_row(msg: &DecodedMessage, is_selected: bool) -> impl IntoElement {
    let bg_color = if is_selected {
        gpui::rgb(0x1a3a5f)
    } else {
        gpui::rgb(0x0f0f23)
    };

    let mut parts = Vec::new();
    for col in DEFAULT_COLUMNS {
        let value = match *col {
            Column::Time => format_elapsed_time(msg.timestamp(), None),
            Column::Src => format!("{:02X}", msg.source_address()),
            Column::Dest => format!("{:02X}", msg.dest_address()),
            Column::Pgn => format!("{:X}", msg.pgn()),
            Column::Title => msg.title.clone(),
            Column::Detail => build_detail_string(msg, 40),
            _ => String::new(),
        };
        parts.push(value);
    }

    let row_text = parts.join("  ");

    div()
        .id(ElementId::Integer(msg.timestamp() as u64))
        .w_full()
        .h_6()
        .bg(bg_color)
        .border_b_1()
        .border_color(gpui::rgb(0x1a1a2e))
        .flex_row()
        .px_3()
        .items_center()
        .child(
            div()
                .text_xs()
                .font_family("monospace")
                .text_color(if is_selected {
                    gpui::rgb(0xffffff)
                } else {
                    gpui::rgb(0xcccccc)
                })
                .child(row_text),
        )
}
