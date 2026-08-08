/// Ratatui widget for rendering the column configuration modal overlay.
///
/// Renders a centered popup with:
/// - Title bar (" Configure Columns ")
/// - Scrollable checkbox list of all columns
/// - Key hints bar at bottom
use crate::tui::app::ColumnEditorModal;
use crate::tui::columns::Column;
use ratatui::prelude::*;
use ratatui::widgets::Widget;

pub struct ColumnEditorWidget<'a> {
    pub state: &'a mut ColumnEditorModal,
}

impl<'a> ColumnEditorWidget<'a> {
    pub fn new(state: &'a mut ColumnEditorModal) -> Self {
        Self { state }
    }
}

impl Widget for ColumnEditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_height = 14u16;
        let modal_width = 50u16;

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
                if !(x <= col
                    && col < x + modal_area.width
                    && y <= row
                    && row < y + modal_area.height)
                {
                    buf[(col, row)].set_style(Style::default().bg(Color::Rgb(10, 10, 16)));
                }
            }
        }

        // Modal background (dark theme)
        for row in y..y + modal_area.height {
            for col in x..x + modal_area.width {
                buf[(col, row)].set_style(Style::default().bg(Color::Rgb(30, 30, 46)));
            }
        }

        // Title bar (top border)
        let title_text = " Configure Columns ";
        for col in x..x + modal_area.width {
            buf[(col, y)].set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Cyan));
        }
        let title_start = x + 1;
        for (i, ch) in title_text.chars().enumerate() {
            let pos = title_start + (i as u16);
            if pos < x + modal_area.width - 1 {
                buf[(pos, y)]
                    .set_char(ch)
                    .set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Cyan));
            }
        }

        // Inner area (inside border)
        let inner = modal_area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });

        // Calculate layout within inner area:
        // - Checkbox list: inner.height - 3 (border + key hints)
        // - Key hints: 2 lines (border + text)
        let key_hints_height = 2u16;
        let list_area_height = inner.height.saturating_sub(key_hints_height);

        // Render checkbox list
        let columns = Column::all();
        let visible_count = list_area_height as usize;

        // Determine scroll offset based on selected_index to keep it visible
        let mut scroll_offset = 0usize;
        if self.state.selected_index >= visible_count {
            scroll_offset = self.state.selected_index - visible_count + 1;
        }

        let start_idx = scroll_offset.min(columns.len().saturating_sub(1));
        let end_idx = (start_idx + visible_count).min(columns.len());
        let visible_columns: Vec<&Column> = columns[start_idx..end_idx].iter().collect();

        // Build list items with checkboxes and selection highlight
        let mut list_lines: Vec<(String, Style)> = Vec::new();

        if start_idx > 0 {
            list_lines.push((" ▲".to_string(), Style::default().fg(Color::DarkGray)));
        }

        for (i, col) in visible_columns.iter().enumerate() {
            let global_idx = start_idx + i;
            let is_selected = global_idx == self.state.selected_index;

            // Find the column state in config
            let enabled =
                if let Some(state) = self.state.config.states.iter().find(|s| s.column == **col) {
                    state.enabled
                } else {
                    col.default_enabled()
                };

            let checkbox = if enabled { "☑" } else { "☐" };

            let text = format!("  {} {}", checkbox, col.label());

            let style = if is_selected {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };

            list_lines.push((text, style));
        }

        if end_idx < columns.len() {
            list_lines.push((" ▼".to_string(), Style::default().fg(Color::DarkGray)));
        }

        // Render the list in the middle area
        let list_y = y + 2;
        for (i, (text, style)) in list_lines.iter().enumerate() {
            if i < list_area_height as usize {
                let row = list_y + i as u16;
                if row < y + modal_area.height - key_hints_height {
                    for (j, ch) in text.chars().enumerate() {
                        let col_pos = x + 1 + j as u16;
                        if col_pos < x + modal_area.width - 1 {
                            buf[(col_pos, row)].set_char(ch).set_style(*style);
                        }
                    }
                }
            }
        }

        // Key hints bar (bottom border)
        let key_hints_y = y + modal_area.height - 2;
        for col in x..x + modal_area.width {
            buf[(col, key_hints_y)]
                .set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Gray));
        }

        // Key hints text
        let hints_text = " [Space] Toggle  [Enter] Confirm  [Esc] Cancel ";
        let hints_start = x + 1;
        for (i, ch) in hints_text.chars().enumerate() {
            let pos = hints_start + i as u16;
            if pos < x + modal_area.width - 1 {
                buf[(pos, key_hints_y)]
                    .set_char(ch)
                    .set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Gray));
            }
        }

        // Bottom border line
        let bottom_border_y = y + modal_area.height - 1;
        for col in x..x + modal_area.width {
            buf[(col, bottom_border_y)]
                .set_style(Style::default().bg(Color::Rgb(30, 30, 46)).fg(Color::Gray));
        }
    }
}
