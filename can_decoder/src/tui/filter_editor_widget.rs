/// Ratatui widget for rendering the filter editor modal overlay.
///
/// Renders a centered popup with:
/// - Title bar (e.g., "Filter Editor: Source Addr")
/// - Text input field at top
/// - Scrollable checkbox list in middle
/// - Key hints bar at bottom

use crate::filter_editor::{FilterEditor, FilterEditorState, FieldType, TextValidation};
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Widget};

pub struct FilterEditorWidget<'a> {
    pub state: &'a mut FilterEditorState,
    pub editor: &'a FilterEditor,
}

impl<'a> FilterEditorWidget<'a> {
    pub fn new(state: &'a mut FilterEditorState, editor: &'a FilterEditor) -> Self {
        Self { state, editor }
    }
}

impl Widget for FilterEditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_height = 20u16;

        let is_name_filter = matches!(self.state.field_type, FieldType::SrcName | FieldType::DstName);
        let modal_width = if is_name_filter {
            (area.width as f64 * 0.9).round() as u16
        } else {
            70u16
        };

        let x = (area.width.saturating_sub(modal_width)) / 2;
        let y = (area.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect {
            x,
            y,
            width: modal_width.min(area.width),
            height: modal_height.min(area.height),
        };

        // Background overlay (darken everything behind modal)
        for row in 0..area.height {
            for col in 0..area.width {
                if !(x <= col && col < x + modal_area.width && y <= row && row < y + modal_area.height) {
                    buf[(col, row)].set_style(Style::default().bg(Color::Rgb(10, 10, 16)));
                }
            }
        }

        // Modal background (dark theme) - fill ALL cells including text field area
        for row in y..y + modal_area.height {
            for col in x..x + modal_area.width {
                buf[(col, row)].set_style(Style::default().bg(Color::Rgb(30, 30, 46)));
            }
        }

        // Title bar (top border)
        let title_text = format!(" Filter Editor: {} ", self.state.field_type.label());
        for col in x..x + modal_area.width {
            buf[(col, y)].set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Cyan));
        }
        // Title text (overwrite border chars)
        let title_start = x + 1;
        for (i, ch) in title_text.chars().enumerate() {
            let pos = title_start + (i as u16);
            if pos < x + modal_area.width - 1 {
                buf[(pos, y)].set_char(ch).set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Cyan));
            }
        }

        // Inner area (inside border)
        let inner = modal_area.inner(Margin { vertical: 1, horizontal: 1 });

        // Calculate layout within inner area:
        // - Text field: 2 lines (border + input)
        // - Checkbox list: remaining lines minus key hints
        // - Key hints: 1 line at bottom
        let text_field_height = 2u16;
        let key_hints_height = 1u16;
        let list_area_height = inner.height.saturating_sub(text_field_height).saturating_sub(key_hints_height);

        // Text field area (top section)
        let text_area = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            text_field_height.min(inner.height),
        );

        self.render_text_field(text_area, buf);

        // Checkbox list area (middle section)
        let list_inner_top = text_area.y + text_area.height;
        let remaining_height = inner.height.saturating_sub(text_field_height).saturating_sub(key_hints_height);
        let list_inner_height = list_area_height.min(remaining_height);

        if list_inner_height > 0 {
            let list_area = Rect::new(
                inner.x,
                list_inner_top,
                inner.width,
                list_inner_height,
            );
            self.render_checkbox_list(list_area, buf);
        }

        // Key hints bar (bottom section)
        let hints_y = modal_area.y + modal_area.height - 1;
        if hints_y < modal_area.y + modal_area.height {
            let hints_area = Rect::new(modal_area.x, hints_y, modal_area.width, 1);
            self.render_key_hints(hints_area, buf);
        }
    }
}

impl FilterEditorWidget<'_> {
    fn render_text_field(&self, area: Rect, buf: &mut Buffer) {
        let is_text_focused = self.state.is_text_focused();
        let border_style = if is_text_focused {
            Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Cyan)
        } else {
            Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Gray)
        };

        // Top border line
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(border_style);
        }

        // Input text with cursor (second row of text field)
        let input_y = area.y + 1;
        let input_x = area.x + 1;
        let max_text_width = (area.width as usize).saturating_sub(2);
        let bg_color = Color::Rgb(30, 30, 46);

        if is_text_focused {
            let text = &self.state.text_input;
            let scroll_offset = 0usize;

            // Draw text before cursor (with scroll offset)
            let chars_before: String = text.chars().skip(scroll_offset).take(self.state.cursor_pos.saturating_sub(scroll_offset)).collect();
            for (i, ch) in chars_before.chars().enumerate() {
                let pos_x = input_x + (i as u16);
                if pos_x < area.x + area.width - 1 {
                    buf[(pos_x, input_y)].set_char(ch).set_style(Style::default().bg(bg_color).fg(Color::White));
                }
            }

            // Draw cursor character at current position
            let cursor_offset = self.state.cursor_pos.saturating_sub(scroll_offset);
            let cursor_x = input_x + (cursor_offset as u16);
            if cursor_x < area.x + area.width - 1 && self.state.cursor_pos < text.len() {
                let ch = text.chars().nth(self.state.cursor_pos).unwrap_or(' ');
                buf[(cursor_x, input_y)].set_char(ch).set_style(Style::default().bg(Color::LightBlue).fg(Color::Black));
            } else if cursor_x < area.x + area.width - 1 && self.state.cursor_pos == text.len() {
                // Cursor at end of text (block cursor)
                buf[(cursor_x, input_y)].set_char(' ').set_style(Style::default().bg(Color::LightBlue).fg(Color::Black));
            }

            // Draw text after cursor
            if self.state.cursor_pos < text.len() {
                let chars_after: String = text.chars().skip(self.state.cursor_pos + 1).take(max_text_width.saturating_sub(self.state.cursor_pos - scroll_offset)).collect();
                for (i, ch) in chars_after.chars().enumerate() {
                    let pos_x = cursor_x + 1 + (i as u16);
                    if pos_x < area.x + area.width - 1 {
                        buf[(pos_x, input_y)].set_char(ch).set_style(Style::default().bg(bg_color).fg(Color::White));
                    }
                }
            }
        } else if !self.state.text_input.is_empty() {
            // Show text without cursor (read-only view)
            let display: String = self.state.text_input.chars().take(max_text_width).collect();
            for (i, ch) in display.chars().enumerate() {
                let pos_x = input_x + (i as u16);
                if pos_x < area.x + area.width - 1 {
                    buf[(pos_x, input_y)].set_char(ch).set_style(Style::default().bg(bg_color).fg(Color::White));
                }
            }
        }

        // Validation indicator (right side)
        let val_x = area.x + area.width.saturating_sub(20);
        if val_x < area.x + area.width - 1 {
            match &self.state.validation {
                TextValidation::Valid => {
                    buf[(val_x, input_y)].set_char('\u{2713}').set_style(Style::default().bg(bg_color).fg(Color::LightGreen));
                }
                TextValidation::Invalid(msg) => {
                    buf[(val_x, input_y)].set_char('\u{2717}').set_style(Style::default().bg(bg_color).fg(Color::LightRed));
                    let remaining = (area.x + area.width - 1).saturating_sub(val_x + 1);
                    let display: String = msg.chars().take(remaining as usize).collect();
                    for (i, ch) in display.chars().enumerate() {
                        let pos_x = val_x + 1 + (i as u16);
                        if pos_x < area.x + area.width - 1 {
                            buf[(pos_x, input_y)].set_char(ch).set_style(Style::default().bg(bg_color).fg(Color::LightRed));
                        }
                    }
                }
            }
        }

        // Bottom border of text field (shared with list top)
        let bottom_y = area.y + area.height - 1;
        for x in area.x..area.x + area.width {
            buf[(x, bottom_y)].set_style(border_style);
        }
    }

    fn render_checkbox_list(&self, area: Rect, buf: &mut Buffer) {
        let is_list_focused = self.state.is_list_focused();
        let bg_color = Color::Rgb(30, 30, 46);
        let style = Style::default().bg(bg_color).fg(Color::Rgb(200, 200, 200));

        // Top border (already drawn by text field bottom) - just ensure it's correct
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(style);
        }

        let inner = area.inner(Margin { vertical: 0, horizontal: 1 });
        let visible_height = inner.height as usize;

        if self.state.options.is_empty() {
            let msg = Paragraph::new("No options available")
                .style(Style::default().bg(bg_color).fg(Color::DarkGray))
                .alignment(Alignment::Center);
            msg.render(area, buf);
            return;
        }

        // Compute visible range using local scroll state (avoid mutating shared scroll_mgr)
        let mut local_scroll = self.state.scroll_mgr.clone();
        local_scroll.set_num_rows(visible_height);
        let (start_idx, end_idx) = local_scroll.get_visible_range();

        for (i, opt_idx) in (start_idx..end_idx).enumerate() {
            if i >= visible_height {
                break;
            }

            let row_y = area.y + 1 + i as u16;
            if row_y >= area.y + area.height {
                break;
            }

            let opt = &self.state.options[opt_idx];
            let is_selected = self.state.is_selected(opt.id);
            let is_cursor_row = is_list_focused && self.state.scroll_mgr.selected_index == opt_idx;

            // Checkbox indicator (leftmost column)
            if inner.x < area.x + area.width {
                let check_char = if is_selected { '\u{2611}' } else { '\u{2610}' };
                let check_style = if is_selected {
                    Style::default().bg(bg_color).fg(Color::LightGreen)
                } else {
                    Style::default().bg(bg_color).fg(Color::Gray)
                };
                buf[(inner.x, row_y)].set_char(check_char).set_style(check_style);

                // Space after checkbox
                if inner.x + 1 < area.x + area.width {
                    buf[(inner.x + 1, row_y)].set_style(Style::default().bg(bg_color));
                }
            }

            // Option display text
            let max_text_width = (inner.width as usize).saturating_sub(2);
            let display: String = opt.display.chars().take(max_text_width).collect();

            for (j, ch) in display.chars().enumerate() {
                let pos_x = inner.x + j as u16;
                if pos_x >= area.x + area.width {
                    break;
                }

                let mut cell_style = style;
                if is_cursor_row {
                    cell_style = Style::default().bg(Color::Blue).fg(Color::White);
                } else if is_selected && !is_cursor_row {
                    cell_style = Style::default().bg(bg_color).fg(Color::LightGreen);
                }

                buf[(pos_x, row_y)].set_char(ch).set_style(cell_style);
            }

            // Fill remaining cells in row with background
            for j in display.len() as u16..inner.width {
                let pos_x = inner.x + j;
                if pos_x < area.x + area.width {
                    let mut cell_style = style;
                    if is_cursor_row {
                        cell_style = Style::default().bg(Color::Blue).fg(Color::White);
                    } else if is_selected && !is_cursor_row {
                        cell_style = Style::default().bg(bg_color).fg(Color::LightGreen);
                    }
                    buf[(pos_x, row_y)].set_char(' ').set_style(cell_style);
                }
            }

            // Scroll indicators (rightmost column)
            let indicator_x = area.x + area.width - 1;
            if i == 0 && start_idx > 0 {
                buf[(indicator_x, row_y)].set_char('\u{25B2}').set_style(Style::default().bg(bg_color).fg(Color::Yellow));
            }
            if i == visible_height.saturating_sub(1) && end_idx < self.state.options.len() {
                buf[(indicator_x, row_y)].set_char('\u{25BC}').set_style(Style::default().bg(bg_color).fg(Color::Yellow));
            }
        }

        // Bottom border of list
        let bottom_y = area.y + area.height - 1;
        if bottom_y >= area.y && bottom_y < area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, bottom_y)].set_style(style);
            }
        }
    }

    fn render_key_hints(&self, area: Rect, buf: &mut Buffer) {
        let bg_color = Color::Rgb(30, 30, 46);
        let hints = vec![
            ("[Space] Toggle", Color::Yellow),
            ("[Tab] Focus", Color::White),
            ("[Enter] Accept", Color::Green),
            ("[Esc] Cancel", Color::Red),
        ];

        let mut x = area.x + 1;
        for (hint, color) in hints {
            if x >= area.x + area.width - 1 {
                break;
            }
            let remaining = (area.x + area.width - 1).saturating_sub(x);
            let display: String = hint.chars().take(remaining as usize).collect();
            for (i, ch) in display.chars().enumerate() {
                buf[(x + (i as u16), area.y)].set_char(ch).set_style(Style::default().bg(bg_color).fg(color));
            }
            x += display.len() as u16 + 2;
        }
    }
}
