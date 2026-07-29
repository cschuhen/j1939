use ratatui::{prelude::*, widgets::*};
use crate::tui::app::{TuiApp, Focus, FilterType, InputMode};
use crate::types::{DecodedField, Numeric, Severity, FlagValue};

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

    pub fn render(&self, frame: &mut Frame, app: &TuiApp) {
        let area = frame.area();

        // Handle error log overlay
        if app.error_log_visible {
            let (popup_area, popup) = self.error_log_popup(area, app);
            frame.render_widget(popup, popup_area);
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
            self.render_horizontal(content_area, frame, app);
            
            // Status bar (horizontal mode)
            let status_text = app.status_text();
            let block = Paragraph::new(status_text)
                .block(Block::default().borders(Borders::TOP))
                .style(Style::default().bg(Color::Blue).fg(Color::White));
            frame.render_widget(block, status_area);
        }
    }

    fn render_horizontal(&self, content_area: Rect, frame: &mut Frame, app: &TuiApp) {
        let lhs_width = if app.lhs_visible { 30 } else { 0 };
        let rhs_width = if app.rhs_visible { 45 } else { 0 };
        
        let main_width = content_area.width.saturating_sub(lhs_width + rhs_width);

        let constraints = vec![
            Constraint::Length(lhs_width),
            Constraint::Length(main_width),
            Constraint::Length(rhs_width),
        ];
        let areas = Layout::default()
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

    fn render_vertical(&self, content_area: Rect, frame: &mut Frame, app: &TuiApp) {
        // Split vertically: top = LHS (full width), bottom = Main | RHS
        let vertical_chunks = Layout::default()
            .constraints([Constraint::Length(15), Constraint::Min(0)])
            .split(content_area);

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
        let status_area = Layout::default()
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(vertical_chunks[1]);

        let status_text = app.status_text();
        let block = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::TOP))
            .style(Style::default().bg(Color::Blue).fg(Color::White));
        frame.render_widget(block, status_area[1]);
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
        let items: Vec<Line> = app.lhs_widgets.iter().enumerate().map(|(i, widget)| {
            let is_selected = i == app.active_lhs_widget && is_active;
            
            if widget.enabled || widget.expanded {
                // Expanded view - build as string with spans
                let mut spans = vec![Span::styled(
                    format!("{} ", widget.name),
                    Style::default().fg(Color::Green).bold()
                )];

                match widget.filter_type {
                    FilterType::Title => {
                        spans.push(Span::raw(format!("input: {}", widget.input_text)));
                    }
                    FilterType::Pgn => {
                        spans.push(Span::raw(format!("pgn: {}", widget.input_text)));
                    }
                    FilterType::Severity => {
                        let options = ["info", "warning", "error"];
                        for opt in &options {
                            if widget.enabled && widget.input_text == *opt {
                                spans.push(Span::styled(
                                    format!("  {:>10} <-selected", opt),
                                    Style::default().fg(Color::Yellow)
                                ));
                            } else {
                                spans.push(Span::raw(format!("  {:>10}", opt)));
                            }
                        }
                    }
                    FilterType::Source | FilterType::Dest => {
                        spans.push(Span::raw(format!("addr: {}", widget.input_text)));
                    }
                    FilterType::Numeric => {
                        spans.push(Span::raw(format!("range: {}", widget.input_text)));
                    }
                    FilterType::Flag => {
                        let options = ["on", "off", "error", "unavailable"];
                        for opt in &options {
                            if widget.enabled && widget.input_text == *opt {
                                spans.push(Span::styled(
                                    format!("  {:>12} <-selected", opt),
                                    Style::default().fg(Color::Yellow)
                                ));
                            } else {
                                spans.push(Span::raw(format!("  {:>12}", opt)));
                            }
                        }
                    }
                }
                
                if is_selected && app.input_mode == InputMode::TextInput {
                    spans.push(Span::styled(
                        format!("[CURSOR:{}]", widget.cursor_pos),
                        Style::default().fg(Color::Yellow)
                    ));
                }
                
                Line::from(spans)
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
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White));

        frame.render_widget(list, area.inner(Margin { vertical: 1, horizontal: 1 }));
    }

    fn render_main(&self, frame: &mut Frame, area: Rect, app: &TuiApp) {
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

        let inner = area.inner(Margin { vertical: 1, horizontal: 1 });
        let viewport_height = inner.height as usize;

        if app.messages.is_empty() {
            let msg = Paragraph::new("No messages received yet.")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            frame.render_widget(msg, inner);
            return;
        }

        let visible_msgs = app.get_visible_messages(viewport_height);
        
        let items: Vec<Line> = visible_msgs.iter().enumerate().map(|(i, msg)| {
            let global_idx = app.scroll_offset + i;
            let is_selected = global_idx == app.selected_index && is_active;
            
            // Build a condensed single-line representation
            let pgn_hex = format!("{:X}", msg.pgn());
            let src_addr = format!("{:X}", msg.source_address());
            let title = &msg.title;
            
            // Extract first meaningful output for display
            let preview = if let Some(output) = msg.outputs.first() {
                match output {
                    DecodedField::Value { title, value, unit, .. } => {
                        let val_str = format_value(value);
                        if let Some(u) = unit {
                            format!("{}={} {}", title, val_str, u)
                        } else {
                            format!("{}={}", title, val_str)
                        }
                    }
                    DecodedField::StringMessage { text, severity } => {
                        let sev_char = match severity {
                            Severity::Info => "I",
                            Severity::Warning => "W",
                            Severity::Error => "E",
                        };
                        format!("[{}] {}", sev_char, text)
                    }
                    DecodedField::Flag { title, value } => {
                        let flag_str = match value {
                            FlagValue::Off => "OFF",
                            FlagValue::On => "ON",
                            FlagValue::Error => "ERR",
                            FlagValue::Unavailable => "N/A",
                        };
                        format!("{}={}", title, flag_str)
                    }
                }
            } else {
                String::new()
            };

            let timestamp = format_timestamp(msg.timestamp());

            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            // Format: [time] PGN src | title | preview
            let line_text = format!("[{}] {} {} | {} {}", 
                timestamp, pgn_hex, src_addr, title, preview);
            
            Line::from(line_text).style(style)
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(Style::default());

        frame.render_widget(list, inner);
    }

    fn render_rhs(&self, frame: &mut Frame, area: Rect, app: &TuiApp) {
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

        let inner = area.inner(Margin { vertical: 1, horizontal: 1 });

        if let Some(msg) = app.get_selected_message() {
            let mut paragraphs = Vec::new();

            // Header section
            paragraphs.push(Line::from("").style(Style::default().fg(Color::Yellow).bold()));
            paragraphs.push(Line::from(format!(" {}", msg.title)).style(Style::default().fg(Color::Yellow).bold()));
            
            // Metadata section
            paragraphs.push(Line::from("").style(Style::default()));
            paragraphs.push(Line::from("Metadata:").style(Style::default().fg(Color::Cyan).bold()));
            
            let timestamp = format_timestamp(msg.timestamp());
            paragraphs.push(Line::from(format!("  Time:    {}", timestamp)));
            paragraphs.push(Line::from(format!("  PGN:     {:X} ({})", msg.pgn(), msg.title)));
            paragraphs.push(Line::from(format!("  CAN ID:  {:08X}", msg.assembled_message.id)));
            paragraphs.push(Line::from(format!("  Source:  {:02X}h", msg.source_address())));
            paragraphs.push(Line::from(format!("  Dest:    {:02X}h", msg.dest_address())));

            // Try to resolve device name
            let src_name = app.device_manager.get_device(msg.source_address());
            if let Some(device) = src_name {
                if let Some(ref name) = device.name {
                    paragraphs.push(Line::from(format!("  DevName: {}", name)));
                }
            }

            // Raw data
            let data_hex: String = msg.data_bytes()
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            paragraphs.push(Line::from(format!("  Data:    {}", data_hex)));

            // Decoded fields section
            if !msg.outputs.is_empty() {
                paragraphs.push(Line::from("").style(Style::default()));
                paragraphs.push(Line::from("Decoded Fields:").style(Style::default().fg(Color::Cyan).bold()));

                for output in &msg.outputs {
                    match output {
                        DecodedField::Value { title, value, unit, decimal_places } => {
                            paragraphs.push(Line::from("").style(Style::default()));
                            paragraphs.push(Line::from(format!("  {}", title)).style(Style::default().fg(Color::Green).bold()));
                            
                            let val_str = format_value(value);
                            paragraphs.push(Line::from(format!("    Value:   {}", val_str)));
                            
                            if let Some(u) = unit {
                                paragraphs.push(Line::from(format!("    Unit:    {}", u)));
                            }
                            if let Some(dp) = decimal_places {
                                let prec = if *dp > 0 { format!(".{}", "0".repeat(*dp as usize)) } else { String::new() };
                                paragraphs.push(Line::from(format!("    Precision:{} digits", prec)));
                            }

                            // Raw hex representation
                            let raw_hex = match value {
                                Numeric::Int(i) => format!("{:#018X}", *i as u64),
                                Numeric::Float(f) => {
                                    let bytes = f.to_bits();
                                    format!("{:#018X}", bytes)
                                }
                                Numeric::Hex(h) => format!("0x{}", h.iter().map(|b| format!("{:02X}", b)).collect::<String>()),
                                Numeric::Bool(b) => format!("{}", if *b { 1u64 } else { 0u64 }),
                            };
                            paragraphs.push(Line::from(format!("    Raw:     {}", raw_hex)));
                        }
                        DecodedField::StringMessage { severity, text } => {
                            paragraphs.push(Line::from("").style(Style::default()));
                            let sev_color = match severity {
                                Severity::Info => Color::White,
                                Severity::Warning => Color::Yellow,
                                Severity::Error => Color::Red,
                            };
                            paragraphs.push(Line::from(format!("  [{:?}] {}", severity, text)).style(Style::default().fg(sev_color)));
                        }
                        DecodedField::Flag { title, value } => {
                            paragraphs.push(Line::from("").style(Style::default()));
                            let flag_color = match value {
                                FlagValue::Off => Color::Red,
                                FlagValue::On => Color::Green,
                                FlagValue::Error => Color::Magenta,
                                FlagValue::Unavailable => Color::Gray,
                            };
                            let flag_str = match value {
                                FlagValue::Off => "OFF",
                                FlagValue::On => "ON",
                                FlagValue::Error => "ERROR",
                                FlagValue::Unavailable => "UNAVAILABLE",
                            };
                            paragraphs.push(Line::from(format!("  {} = {}", title, flag_str)).style(Style::default().fg(flag_color)));
                        }
                    }
                }
            }

            // Updates section
            if !msg.updates.is_empty() {
                paragraphs.push(Line::from("").style(Style::default()));
                paragraphs.push(Line::from("Device Updates:").style(Style::default().fg(Color::Cyan).bold()));
                for update in &msg.updates {
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
        
        let popup_area = Rect { x, y, width, height };

        let mut lines = Vec::new();
        lines.push(Line::from(" Error/Warning Log [Esc to close] ").style(Style::default().fg(Color::Yellow).bold()));
        lines.push(Line::from("").style(Style::default()));

        let log_entries: Vec<&str> = if app.error_log.is_empty() {
            vec!["No errors or warnings."]
        } else {
            let start = app.error_log.len().saturating_sub(height as usize - 4);
            app.error_log.iter().map(|s| s.as_str()).skip(start).take((height - 4) as usize).collect()
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

fn format_value(value: &Numeric) -> String {
    match value {
        Numeric::Int(i) => format!("{}", i),
        Numeric::Float(f) => format!("{:.4}", f),
        Numeric::Hex(h) => format!("0x{}", h.iter().map(|b| format!("{:02X}", b)).collect::<String>()),
        Numeric::Bool(b) => format!("{}", b),
    }
}

fn format_timestamp(timestamp: u64) -> String {
    let seconds = timestamp / 1_000_000;
    let millis = (timestamp % 1_000_000) / 1_000;
    format!("{}.{:03}", seconds, millis)
}
