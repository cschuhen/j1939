use crate::formats;
use crate::tui::app::{FilterType, Focus, InputMode, TuiApp};
use crate::tui::column_editor_widget::ColumnEditorWidget;
use crate::tui::columns::Column;
use crate::tui::filter_editor_widget::FilterEditorWidget;
use crate::types::{DecodedField, FlagValue, Numeric, Severity};
use ratatui::{prelude::*, widgets::*};

pub struct TuiRenderer {}

impl Default for TuiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiRenderer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, frame: &mut Frame, app: &mut TuiApp) {
        let area = frame.area();

        // Handle error log overlay
        if app.error_log_visible {
            let (popup_area, popup) = self.error_log_popup(area, app);
            frame.render_widget(popup, popup_area);
            return;
        }

        // Handle filter editor modal overlay
        if let Some(modal) = app.filter_editor.as_mut() {
            let widget = FilterEditorWidget::new(&mut modal.state, &modal.editor);
            frame.render_widget(widget, area);
            return;
        }

        // Handle column editor modal overlay
        if let Some(modal) = app.column_editor.as_mut() {
            let widget = ColumnEditorWidget::new(modal);
            frame.render_widget(widget, area);
            return;
        }

        // 1. Split into content area and status bar
        let chunks = Layout::default()
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        let content_area = chunks[0];
        let status_area = chunks[1];

        // 2. Split content based on layout orientation
        if app.layout_vertical {
            self.render_vertical(content_area, frame, app);
        } else {
            self.render_horizontal(frame, content_area, app);

            // Status bar (horizontal mode)
            let status_text = app.status_text();
            let block = Paragraph::new(status_text)
                .block(Block::default())
                .style(Style::default().bg(Color::Blue).fg(Color::White));
            frame.render_widget(block, status_area);
        }
    }

    fn render_horizontal(&self, frame: &mut Frame, content_area: Rect, app: &mut TuiApp) {
        let lhs_width = if app.lhs_visible { 30 } else { 0 };
        let rhs_width = if app.rhs_visible { 45 } else { 0 };

        let total_side = lhs_width + rhs_width;
        let _available = content_area.width.saturating_sub(total_side);

        // Use ratio-based constraints so main panel gets most space
        let (lhs_ratio, main_ratio, rhs_ratio) = if app.lhs_visible && app.rhs_visible {
            (3, 5, 2)
        } else if app.lhs_visible {
            (1, 4, 0)
        } else if app.rhs_visible {
            (0, 4, 1)
        } else {
            (0, 5, 0)
        };

        let constraints = vec![
            Constraint::Ratio(lhs_ratio, lhs_ratio + main_ratio + rhs_ratio),
            Constraint::Ratio(main_ratio, lhs_ratio + main_ratio + rhs_ratio),
            Constraint::Ratio(rhs_ratio, lhs_ratio + main_ratio + rhs_ratio),
        ];
        let areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(content_area);

        if app.lhs_visible {
            self.render_lhs(frame, areas[0], app);
        }

        self.render_main(frame, areas[1], app);

        if app.rhs_visible {
            self.render_rhs(frame, areas[2], app);
        }
    }

    fn render_vertical(&self, content_area: Rect, frame: &mut Frame, app: &mut TuiApp) {
        // Reserve space for status bar at bottom
        let status_height = 1u16;
        let main_content_area = Rect::new(
            content_area.x,
            content_area.y,
            content_area.width,
            content_area.height.saturating_sub(status_height),
        );

        // Split vertically: top = LHS (full width), bottom = Main | RHS
        let vertical_chunks = Layout::default()
            .constraints([Constraint::Length(15), Constraint::Min(0)])
            .split(main_content_area);

        // Render LHS at top (full width)
        if app.lhs_visible {
            self.render_lhs(frame, vertical_chunks[0], app);
        } else {
            let placeholder = Paragraph::new("LHS hidden - press F1 to show")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            frame.render_widget(placeholder, vertical_chunks[0]);
        }

        // Split bottom row into Main | RHS
        let rhs_width = if app.rhs_visible { 45 } else { 0 };
        let main_width = vertical_chunks[1].width.saturating_sub(rhs_width);

        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(main_width),
                Constraint::Length(rhs_width),
            ])
            .split(vertical_chunks[1]);

        self.render_main(frame, bottom_chunks[0], app);

        if app.rhs_visible {
            self.render_rhs(frame, bottom_chunks[1], app);
        } else {
            let placeholder = Paragraph::new("RHS hidden - press F2 to show")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            frame.render_widget(placeholder, bottom_chunks[1]);
        }

        // Status bar at very bottom
        let status_area = Rect::new(
            content_area.x,
            main_content_area.y + main_content_area.height,
            content_area.width,
            status_height,
        );

        let status_text = app.status_text();
        let block = Paragraph::new(status_text)
            .block(Block::default())
            .style(Style::default().bg(Color::Blue).fg(Color::White));
        frame.render_widget(block, status_area);
    }

    fn render_lhs(&self, frame: &mut Frame, area: Rect, app: &TuiApp) {
        let is_active = app.focus == Focus::Lhs;

        let block_style = if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Filters ")
            .style(block_style);
        frame.render_widget(block.clone(), area);

        // Build list of filter widget entries
        let items: Vec<Line> = app
            .lhs_widgets
            .iter()
            .enumerate()
            .map(|(i, widget)| {
                let is_selected = i == app.active_lhs_widget && is_active;

                if widget.enabled || widget.expanded {
                    // Expanded view - build as string with spans
                    let mut spans = vec![Span::styled(
                        format!("{} ", widget.name),
                        Style::default().fg(Color::Green).bold(),
                    )];

                    match widget.filter_type {
                        FilterType::Title => {
                            if is_selected && app.input_mode == InputMode::TextInput {
                                let before_cursor = &widget.input_text
                                    [..widget.cursor_pos.min(widget.input_text.len())];
                                let after_cursor = &widget.input_text
                                    [widget.cursor_pos.min(widget.input_text.len())..];
                                spans.push(Span::styled(
                                    format!("{}{}", before_cursor, "█"),
                                    Style::default().fg(Color::White).bg(Color::DarkGray),
                                ));
                                spans.push(Span::raw(after_cursor.to_string()));
                            } else {
                                spans.push(Span::raw(format!("input: {}", widget.input_text)));
                                if is_selected && !widget.enabled {
                                    spans.push(Span::styled(
                                        " [Enter to edit]",
                                        Style::default().fg(Color::Yellow),
                                    ));
                                }
                            }
                        }
                        FilterType::Pgn => {
                            if is_selected && app.input_mode == InputMode::TextInput {
                                let before_cursor = &widget.input_text
                                    [..widget.cursor_pos.min(widget.input_text.len())];
                                let after_cursor = &widget.input_text
                                    [widget.cursor_pos.min(widget.input_text.len())..];
                                spans.push(Span::styled(
                                    format!("{}{}", before_cursor, "█"),
                                    Style::default().fg(Color::White).bg(Color::DarkGray),
                                ));
                                spans.push(Span::raw(after_cursor.to_string()));
                            } else {
                                spans.push(Span::raw(format!("pgn: {}", widget.input_text)));
                                if is_selected && !widget.enabled {
                                    spans.push(Span::styled(
                                        " [Enter to edit]",
                                        Style::default().fg(Color::Yellow),
                                    ));
                                }
                            }
                        }
                        FilterType::Severity => {
                            let options = ["info", "warning", "error"];
                            for opt in &options {
                                if widget.enabled && widget.input_text == *opt {
                                    spans.push(Span::styled(
                                        format!("  {:>10} <-selected", opt),
                                        Style::default().fg(Color::Yellow),
                                    ));
                                } else {
                                    spans.push(Span::raw(format!("  {:>10}", opt)));
                                }
                            }
                        }
                        FilterType::Source | FilterType::Dest => {
                            if is_selected && app.input_mode == InputMode::TextInput {
                                let before_cursor = &widget.input_text
                                    [..widget.cursor_pos.min(widget.input_text.len())];
                                let after_cursor = &widget.input_text
                                    [widget.cursor_pos.min(widget.input_text.len())..];
                                spans.push(Span::styled(
                                    format!("{}{}", before_cursor, "█"),
                                    Style::default().fg(Color::White).bg(Color::DarkGray),
                                ));
                                spans.push(Span::raw(after_cursor.to_string()));
                            } else {
                                spans.push(Span::raw(format!("addr: {}", widget.input_text)));
                                if is_selected && !widget.enabled {
                                    spans.push(Span::styled(
                                        " [Enter to edit]",
                                        Style::default().fg(Color::Yellow),
                                    ));
                                }
                            }
                        }
                        FilterType::Numeric => {
                            if is_selected && app.input_mode == InputMode::TextInput {
                                let before_cursor = &widget.input_text
                                    [..widget.cursor_pos.min(widget.input_text.len())];
                                let after_cursor = &widget.input_text
                                    [widget.cursor_pos.min(widget.input_text.len())..];
                                spans.push(Span::styled(
                                    format!("{}{}", before_cursor, "█"),
                                    Style::default().fg(Color::White).bg(Color::DarkGray),
                                ));
                                spans.push(Span::raw(after_cursor.to_string()));
                            } else {
                                spans.push(Span::raw(format!("range: {}", widget.input_text)));
                                if is_selected && !widget.enabled {
                                    spans.push(Span::styled(
                                        " [Enter to edit]",
                                        Style::default().fg(Color::Yellow),
                                    ));
                                }
                            }
                        }
                        FilterType::Flag => {
                            let options = ["on", "off", "error", "unavailable"];
                            for opt in &options {
                                if widget.enabled && widget.input_text == *opt {
                                    spans.push(Span::styled(
                                        format!("  {:>12} <-selected", opt),
                                        Style::default().fg(Color::Yellow),
                                    ));
                                } else {
                                    spans.push(Span::raw(format!("  {:>12}", opt)));
                                }
                            }
                        }
                        FilterType::SourceName | FilterType::DestName => {
                            if is_selected && app.input_mode == InputMode::TextInput {
                                let before_cursor = &widget.input_text
                                    [..widget.cursor_pos.min(widget.input_text.len())];
                                let after_cursor = &widget.input_text
                                    [widget.cursor_pos.min(widget.input_text.len())..];
                                spans.push(Span::styled(
                                    format!("{}{}", before_cursor, "█"),
                                    Style::default().fg(Color::White).bg(Color::DarkGray),
                                ));
                                spans.push(Span::raw(after_cursor.to_string()));
                            } else {
                                let name_type = if widget.filter_type == FilterType::SourceName {
                                    "src-name"
                                } else {
                                    "dest-name"
                                };
                                spans.push(Span::raw(format!(
                                    "{}: {}",
                                    name_type, widget.input_text
                                )));
                                if is_selected && !widget.enabled {
                                    spans.push(Span::styled(
                                        " [Enter to edit]",
                                        Style::default().fg(Color::Yellow),
                                    ));
                                }
                            }
                        }
                        FilterType::Regex => {
                            if is_selected && app.input_mode == InputMode::TextInput {
                                let before_cursor = &widget.input_text
                                    [..widget.cursor_pos.min(widget.input_text.len())];
                                let after_cursor = &widget.input_text
                                    [widget.cursor_pos.min(widget.input_text.len())..];
                                spans.push(Span::styled(
                                    format!("{}{}", before_cursor, "█"),
                                    Style::default().fg(Color::White).bg(Color::DarkGray),
                                ));
                                spans.push(Span::raw(after_cursor.to_string()));
                            } else {
                                spans.push(Span::raw(format!("regex: {}", widget.input_text)));
                                if is_selected && !widget.enabled {
                                    spans.push(Span::styled(
                                        " [Enter to edit]",
                                        Style::default().fg(Color::Yellow),
                                    ));
                                }
                            }
                        }
                    }

                    let mut line = Line::from(spans);
                    if is_selected {
                        line = line.style(Style::default().bg(Color::DarkGray).fg(Color::White));
                    }
                    line
                } else {
                    // Minimized - just show name with enabled indicator
                    let indicator = if widget.enabled { "*" } else { " " };
                    let text = format!("  {} {}", indicator, widget.name);
                    if is_selected {
                        Line::from(text).style(Style::default().bg(Color::DarkGray))
                    } else {
                        Line::from(text)
                    }
                }
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));

        frame.render_widget(
            list,
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );
    }

    fn render_main(&self, frame: &mut Frame, area: Rect, app: &mut TuiApp) {
        let is_active = app.focus == Focus::Main;

        let block_style = if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Messages ({}) ", app.message_count()))
            .style(block_style);
        frame.render_widget(block.clone(), area);

        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let viewport_height = inner.height.saturating_sub(1) as usize;

        if app.engine.total_count() == 0 {
            let msg = Paragraph::new("No messages received yet.")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            frame.render_widget(msg, inner);
            return;
        }

        let selected_local = if is_active {
            app.scroll_manager.get_selected_viewport_index()
        } else {
            None
        };

        let enabled_cols: Vec<Column> = app
            .column_config
            .states
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.column)
            .collect();

        let header_cells: Vec<Cell> = enabled_cols.iter().map(|c| Cell::from(c.label())).collect();

        let column_widths: Vec<Constraint> = enabled_cols.iter().map(|c| c.constraint()).collect();

        let global_start_time = app.global_start_time;
        let visible_msgs = app.get_visible_messages(viewport_height);

        let total_width = inner.width;
        let fixed_width: u16 = enabled_cols
            .iter()
            .filter(|c| **c != Column::Detail)
            .map(|c| c.base_width())
            .sum();
        let num_gaps = if enabled_cols.len() > 1 {
            (enabled_cols.len() - 1) as u16
        } else {
            0
        };
        let detail_available = total_width
            .saturating_sub(fixed_width)
            .saturating_sub(num_gaps);

        let rows: Vec<Row> = visible_msgs
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                let is_selected = selected_local == Some(i);

                let mut cells = Vec::new();

                for col in &enabled_cols {
                    let cell_text = col.format(
                        msg,
                        detail_available.max(col.base_width()),
                        global_start_time,
                    );
                    let cell = Cell::new(cell_text);
                    cells.push(cell);
                }

                if is_selected {
                    Row::new(cells).style(Style::default().bg(Color::DarkGray).fg(Color::White))
                } else {
                    Row::new(cells).style(Style::default().fg(Color::White))
                }
            })
            .collect();

        let header_row =
            Row::new(header_cells).style(Style::default().fg(Color::White).bg(Color::DarkGray));

        let table = Table::new(rows, column_widths)
            .header(header_row)
            .column_spacing(1);

        frame.render_widget(table, inner);
    }

    fn render_rhs(&self, frame: &mut Frame, area: Rect, app: &mut TuiApp) {
        let is_active = app.focus == Focus::Rhs;

        let block_style = if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Details ")
            .style(block_style);
        frame.render_widget(block.clone(), area);

        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });

        if let Some(msg) = app.get_selected_message() {
            let mut paragraphs = Vec::new();

            // Extract owned data from msg before accessing device_manager
            let source_addr = msg.source_address();
            let dest_addr = msg.dest_address();
            let pgn_val = msg.pgn();
            let title_str = msg.title.clone();
            let can_id = msg.assembled_message.id;
            let timestamp_val = msg.timestamp();
            let data_hex: String = msg
                .data_bytes()
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let outputs_clone = msg.outputs.clone();
            let updates_clone = msg.updates.clone();

            // Header section
            paragraphs.push(Line::from("").style(Style::default().fg(Color::Yellow).bold()));
            paragraphs.push(
                Line::from(format!(" {}", title_str))
                    .style(Style::default().fg(Color::Yellow).bold()),
            );

            // Metadata section
            paragraphs.push(Line::from("").style(Style::default()));
            paragraphs.push(Line::from("Metadata:").style(Style::default().fg(Color::Cyan).bold()));

            let timestamp = formats::format_timestamp(timestamp_val);
            paragraphs.push(Line::from(format!("  Time:    {}", timestamp)));
            paragraphs.push(Line::from(format!(
                "  PGN:     {:X} ({})",
                pgn_val, title_str
            )));
            paragraphs.push(Line::from(format!("  CAN ID:  {:08X}", can_id)));
            paragraphs.push(Line::from(format!("  Source:  {:02X}h", source_addr)));
            paragraphs.push(Line::from(format!("  Dest:    {:02X}h", dest_addr)));

            // Try to resolve device name (msg borrow is now dropped)
            let src_name = app.device_manager.get_device(source_addr);
            if let Some(device) = src_name {
                if let Some(ref name) = device.name {
                    paragraphs.push(Line::from(format!("  DevName: {}", name)));
                }
            }

            // Raw data
            paragraphs.push(Line::from(format!("  Data:    {}", data_hex)));

            // Decoded fields section
            if !outputs_clone.is_empty() {
                paragraphs.push(Line::from("").style(Style::default()));
                paragraphs.push(
                    Line::from("Decoded Fields:").style(Style::default().fg(Color::Cyan).bold()),
                );

                for output in &outputs_clone {
                    match output {
                        DecodedField::Value {
                            title,
                            value,
                            unit,
                            decimal_places,
                        } => {
                            if let Numeric::Flag(flag_value) = &value {
                                paragraphs.push(Line::from("").style(Style::default()));
                                let flag_color = match flag_value {
                                    FlagValue::Off => Color::Red,
                                    FlagValue::On => Color::Green,
                                    FlagValue::Error => Color::Magenta,
                                    FlagValue::Unavailable => Color::Gray,
                                };
                                let flag_str = match flag_value {
                                    FlagValue::Off => "OFF",
                                    FlagValue::On => "ON",
                                    FlagValue::Error => "ERROR",
                                    FlagValue::Unavailable => "UNAVAILABLE",
                                };
                                paragraphs.push(
                                    Line::from(format!("  {} = {}", title, flag_str))
                                        .style(Style::default().fg(flag_color)),
                                );
                            } else {
                                paragraphs.push(Line::from("").style(Style::default()));
                                paragraphs.push(
                                    Line::from(format!("  {}", title))
                                        .style(Style::default().fg(Color::Green).bold()),
                                );

                                let val_str = formats::format_value(&value);
                                paragraphs.push(Line::from(format!("    Value:   {}", val_str)));

                                if let Some(u) = unit {
                                    paragraphs.push(Line::from(format!("    Unit:    {}", u)));
                                }
                                if let Some(dp) = decimal_places {
                                    let prec = if *dp > 0 {
                                        format!(".{}", "0".repeat(*dp as usize))
                                    } else {
                                        String::new()
                                    };
                                    paragraphs
                                        .push(Line::from(format!("    Precision:{} digits", prec)));
                                }

                                // Raw hex representation
                                let raw_hex = match value {
                                    Numeric::Int(i) => format!("{:#018X}", *i as u64),
                                    Numeric::Float(f) => {
                                        let bytes = f.to_bits();
                                        format!("{:#018X}", bytes)
                                    }
                                    Numeric::Hex(h) => format!("0x{:X}", h),
                                    Numeric::Bool(b) => format!("{}", if *b { 1u64 } else { 0u64 }),
                                    Numeric::Flag(..) => "N/A".to_string(),
                                };
                                paragraphs.push(Line::from(format!("    Raw:     {}", raw_hex)));
                            }
                        }
                        DecodedField::StringMessage { severity, text } => {
                            paragraphs.push(Line::from("").style(Style::default()));
                            let sev_color = match severity {
                                Severity::Info => Color::White,
                                Severity::Warning => Color::Yellow,
                                Severity::Error => Color::Red,
                            };
                            paragraphs.push(
                                Line::from(format!("  [{:?}] {}", severity, text))
                                    .style(Style::default().fg(sev_color)),
                            );
                        }
                    }
                }
            }

            // Updates section
            if !updates_clone.is_empty() {
                paragraphs.push(Line::from("").style(Style::default()));
                paragraphs.push(
                    Line::from("Device Updates:").style(Style::default().fg(Color::Cyan).bold()),
                );
                for update in &updates_clone {
                    paragraphs.push(Line::from(format!(
                        "  target_name={:#018X} param_id={} value={:?}",
                        update.target_name, update.param_id, update.value
                    )));
                }
            }

            let content = Paragraph::new(paragraphs)
                .style(Style::default())
                .wrap(Wrap { trim: true });
            frame.render_widget(content, inner);
        } else {
            let msg = Paragraph::new("No message selected.")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            frame.render_widget(msg, inner);
        }
    }

    fn error_log_popup(&self, area: Rect, app: &TuiApp) -> (Rect, Paragraph<'static>) {
        let width = 70u16;
        let height = 20u16;

        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;

        let popup_area = Rect {
            x,
            y,
            width,
            height,
        };

        let mut lines = Vec::new();
        lines.push(
            Line::from(" Error/Warning Log [Esc to close] ")
                .style(Style::default().fg(Color::Yellow).bold()),
        );
        lines.push(Line::from("").style(Style::default()));

        let log_entries: Vec<&str> = if app.error_log.is_empty() {
            vec!["No errors or warnings."]
        } else {
            let start = app.error_log.len().saturating_sub(height as usize - 4);
            app.error_log
                .iter()
                .map(|s| s.as_str())
                .skip(start)
                .take((height - 4) as usize)
                .collect()
        };

        for entry in log_entries {
            lines.push(Line::from(format!("  {}", entry)));
        }

        let content = Paragraph::new(lines)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true });

        (popup_area, content)
    }
}
