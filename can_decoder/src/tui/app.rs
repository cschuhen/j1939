use crate::device_manager::DeviceManager;
use crate::filters::{DestFilter, FlagFilter, NumericFilter, PgnFilter, SourceFilter, SeverityFilter, TitleFilter};
use crate::types::{DecodedMessage, Severity, FlagValue};
use ratatui::layout::Rect;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Lhs,
    Main,
    Rhs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Navigation,
    TextInput,
}

#[derive(Debug, Clone)]
pub struct FilterWidget {
    pub name: String,
    pub enabled: bool,
    pub input_text: String,
    pub cursor_pos: usize,
    pub expanded: bool,
    pub filter_type: FilterType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterType {
    Title,
    Pgn,
    Severity,
    Source,
    Dest,
    Numeric,
    Flag,
}

impl FilterWidget {
    pub fn new(name: &str, filter_type: FilterType) -> Self {
        Self {
            name: name.to_string(),
            enabled: false,
            input_text: String::new(),
            cursor_pos: 0,
            expanded: true,
            filter_type,
        }
    }

    pub fn summary(&self) -> String {
        if !self.enabled {
            return self.name.clone();
        }
        match &self.filter_type {
            FilterType::Title => format!("title:{}", self.input_text),
            FilterType::Pgn => format!("pgn:{}", self.input_text),
            FilterType::Severity => format!("severity:{}", self.input_text),
            FilterType::Source => format!("source:{}", self.input_text),
            FilterType::Dest => format!("dest:{}", self.input_text),
            FilterType::Numeric => format!("numeric:{}:{}", self.name, self.input_text),
            FilterType::Flag => format!("flag:{}={}", self.name, self.input_text),
        }
    }

    pub fn build_filter(&self) -> Option<Box<dyn crate::traits::Filter>> {
        if !self.enabled || self.input_text.is_empty() {
            return None;
        }
        match &self.filter_type {
            FilterType::Title => Some(Box::new(TitleFilter::new(&self.input_text))),
            FilterType::Pgn => {
                let pgn = parse_hex_or_dec_u32(&self.input_text).ok()?;
                Some(Box::new(PgnFilter::new(pgn)))
            }
            FilterType::Severity => {
                let severity = match self.input_text.to_lowercase().as_str() {
                    "info" => Severity::Info,
                    "warning" => Severity::Warning,
                    "error" => Severity::Error,
                    _ => return None,
                };
                Some(Box::new(SeverityFilter { severity }))
            }
            FilterType::Source => {
                let source: u8 = self.input_text.parse().ok()?;
                Some(Box::new(SourceFilter::new(source)))
            }
            FilterType::Dest => {
                let dest: u8 = self.input_text.parse().ok()?;
                Some(Box::new(DestFilter::new(dest)))
            }
            FilterType::Numeric => {
                let title = self.name.clone();
                let (min, max) = parse_range(&self.input_text);
                Some(Box::new(NumericFilter { title, min, max }))
            }
            FilterType::Flag => {
                let title = self.name.clone();
                let value = match self.input_text.to_lowercase().as_str() {
                    "off" => FlagValue::Off,
                    "on" => FlagValue::On,
                    "error" => FlagValue::Error,
                    "unavailable" => FlagValue::Unavailable,
                    _ => return None,
                };
                Some(Box::new(FlagFilter { title, value }))
            }
        }
    }

    pub fn active_filter_count(&self) -> bool {
        self.enabled && !self.input_text.is_empty()
    }
}

fn parse_hex_or_dec_u32(s: &str) -> Result<u32, ()> {
    s.parse().or_else(|_| {
        let stripped = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
        u32::from_str_radix(stripped, 16).map_err(|_| ())
    })
}

fn parse_range(s: &str) -> (Option<f64>, Option<f64>) {
    if let Some(stripped) = s.strip_prefix(">=") {
        (Some(stripped.parse().unwrap_or(f64::MIN)), None)
    } else if let Some(stripped) = s.strip_prefix("<=") {
        (None, Some(stripped.parse().unwrap_or(f64::MAX)))
    } else if let Some(pos) = s.find('-') {
        let min_val: f64 = s[..pos].parse().unwrap_or(f64::MIN);
        let max_val: f64 = s[pos + 1..].parse().unwrap_or(f64::MAX);
        (Some(min_val), Some(max_val))
    } else {
        let val: f64 = s.parse().unwrap_or(f64::MIN);
        (Some(val), Some(val))
    }
}

pub struct TuiApp {
    pub focus: Focus,
    pub input_mode: InputMode,
    pub lhs_visible: bool,
    pub rhs_visible: bool,
    pub error_log_visible: bool,
    pub messages: VecDeque<DecodedMessage>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub is_live_stream: bool,
    pub max_messages: usize,
    pub lhs_widgets: Vec<FilterWidget>,
    pub active_lhs_widget: usize,
    pub device_manager: DeviceManager,
    pub error_log: Vec<String>,
    pub connection_status: ConnectionStatus,
    pub layout_vertical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connected,
    Buffering,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            focus: Focus::Main,
            input_mode: InputMode::Navigation,
            lhs_visible: true,
            rhs_visible: true,
            error_log_visible: false,
            messages: VecDeque::new(),
            selected_index: 0,
            scroll_offset: 0,
            is_live_stream: false,
            max_messages: 10000,
            lhs_widgets: vec![
                FilterWidget::new("Title", FilterType::Title),
                FilterWidget::new("PGN", FilterType::Pgn),
                FilterWidget::new("Severity", FilterType::Severity),
                FilterWidget::new("Source Addr", FilterType::Source),
                FilterWidget::new("Dest Addr", FilterType::Dest),
                FilterWidget::new("Src Name", FilterType::Numeric),
                FilterWidget::new("Dst Name", FilterType::Numeric),
            ],
            active_lhs_widget: 0,
            device_manager: DeviceManager::new(60),
            error_log: Vec::new(),
            connection_status: ConnectionStatus::Disconnected,
            layout_vertical: false,
        }
    }

    pub fn toggle_layout(&mut self) {
        self.layout_vertical = !self.layout_vertical;
    }

    pub fn toggle_lhs(&mut self) {
        self.lhs_visible = !self.lhs_visible;
        if !self.lhs_visible && self.focus == Focus::Lhs {
            self.focus = Focus::Main;
        }
    }

    pub fn toggle_rhs(&mut self) {
        self.rhs_visible = !self.rhs_visible;
        if !self.rhs_visible && self.focus == Focus::Rhs {
            self.focus = Focus::Main;
        }
    }

    pub fn toggle_error_log(&mut self) {
        self.error_log_visible = !self.error_log_visible;
    }

    pub fn add_message(&mut self, mut message: DecodedMessage) {
        let passes_filters = self.passes_all_filters(&message);

        if !passes_filters && !self.lhs_widgets.iter().all(|w| !w.active_filter_count()) {
            let title = std::mem::take(&mut message.title);
            let summary = format!(
                "[FILTERED] PGN={:X} title={} src={:X}",
                message.pgn(),
                title,
                message.source_address()
            );
            self.error_log.push(summary);
            if self.error_log.len() > 100 {
                self.error_log.remove(0);
            }
            return;
        }

        if passes_filters || self.lhs_widgets.iter().all(|w| !w.active_filter_count()) {
            if self.messages.len() >= self.max_messages {
                self.messages.pop_front();
            }
            self.messages.push_back(message);

            if self.is_live_stream {
                self.selected_index = self.messages.len() - 1;
                self.scroll_offset = self.messages.len().saturating_sub(1);
            }
        }
    }

    pub fn passes_all_filters(&self, message: &DecodedMessage) -> bool {
        for widget in &self.lhs_widgets {
            if !widget.active_filter_count() {
                continue;
            }
            if let Some(filter) = widget.build_filter() {
                if !filter_matches_sync(&*filter, message) {
                    return false;
                }
            }
        }
        true
    }

    pub fn scroll_down(&mut self, viewport_height: usize) {
        if self.messages.is_empty() {
            return;
        }

        let max_scroll = self.messages.len().saturating_sub(viewport_height);
        
        // In live mode at bottom, auto-follow new messages
        if self.is_live_stream && self.selected_index >= self.messages.len() - 1 {
            self.selected_index = self.messages.len() - 1;
            self.scroll_offset = max_scroll;
            return;
        }

        // Manual scrolling: move selection down one and keep it visible
        let new_selected = (self.selected_index + 1).min(self.messages.len() - 1);
        self.selected_index = new_selected;
        
        // Adjust scroll_offset to keep selected message in view
        if !self.is_live_stream && new_selected > self.scroll_offset {
            // In manual mode, follow selection after first page
            let max_scroll = self.messages.len().saturating_sub(viewport_height);
            let offset = new_selected.saturating_sub(viewport_height).saturating_add(1);
            self.scroll_offset = offset.max(0).min(max_scroll);
        } else if new_selected >= self.scroll_offset + viewport_height {
            // In live mode, only scroll when selection goes past bottom of view
            let max_scroll = self.messages.len().saturating_sub(viewport_height);
            let offset = new_selected.saturating_sub(viewport_height).saturating_add(1);
            self.scroll_offset = offset.max(0).min(max_scroll);
        }
    }

    pub fn scroll_up(&mut self, viewport_height: usize) {
        // Switch from live to manual when scrolling up from bottom
        if self.is_live_stream && self.selected_index >= self.messages.len() - 1 {
            self.is_live_stream = false;
        }
        
        let new_selected = if self.selected_index > 0 {
            self.selected_index - 1
        } else {
            0
        };
        self.selected_index = new_selected;
        
        // Adjust scroll_offset to keep selected message in view (manual mode)
        if !self.is_live_stream && new_selected < self.scroll_offset {
            let max_scroll = self.messages.len().saturating_sub(viewport_height);
            self.scroll_offset = new_selected.min(max_scroll);
        }
    }

    pub fn page_down(&mut self, viewport_height: usize) {
        let pages = viewport_height * 2;
        for _ in 0..pages {
            self.scroll_down(viewport_height);
        }
    }

    pub fn page_up(&mut self, viewport_height: usize) {
        if self.is_live_stream && self.selected_index >= self.messages.len() - 1 {
            self.is_live_stream = false;
        }
        
        let pages = viewport_height * 2;
        for _ in 0..pages {
            self.scroll_up(viewport_height);
        }
    }

    pub fn get_visible_messages(&self, viewport_height: usize) -> Vec<&DecodedMessage> {
        if self.messages.is_empty() {
            return vec![];
        }

        let total = self.messages.len();
        
        // In live mode at bottom, show last N messages
        // In manual mode, show scroll_offset to scroll_offset+viewport_height
        let start = if self.is_live_stream && self.selected_index >= self.messages.len() - 1 {
            total.saturating_sub(viewport_height)
        } else {
            self.scroll_offset.min(total - 1)
        };
        let end = total.min(start + viewport_height);

        let mut result = Vec::new();
        for (i, msg) in self.messages.iter().enumerate() {
            if i >= start && i < end {
                result.push(msg);
            }
        }
        result
    }

    pub fn get_selected_message(&self) -> Option<&DecodedMessage> {
        if self.messages.is_empty() {
            return None;
        }
        let idx = self.selected_index.min(self.messages.len() - 1);
        self.messages.get(idx)
    }

    pub fn move_focus_next(&mut self, direction: Direction) {
        match direction {
            Direction::Right => {
                if self.focus == Focus::Lhs {
                    self.focus = Focus::Main;
                    self.input_mode = InputMode::Navigation;
                } else if self.focus == Focus::Main && self.rhs_visible {
                    self.focus = Focus::Rhs;
                    self.input_mode = InputMode::Navigation;
                }
            }
            Direction::Left => {
                if self.focus == Focus::Rhs {
                    self.focus = Focus::Main;
                    self.input_mode = InputMode::Navigation;
                } else if self.focus == Focus::Main && self.lhs_visible {
                    self.focus = Focus::Lhs;
                    self.input_mode = InputMode::Navigation;
                }
            }
            Direction::Up => {
                if self.focus == Focus::Lhs {
                    if self.active_lhs_widget > 0 {
                        self.active_lhs_widget -= 1;
                    }
                } else if self.focus == Focus::Main {
                    self.scroll_up(20);
                }
            }
            Direction::Down => {
                if self.focus == Focus::Lhs {
                    if self.active_lhs_widget < self.lhs_widgets.len() - 1 {
                        self.active_lhs_widget += 1;
                    }
                } else if self.focus == Focus::Main {
                    let viewport_height = 20;
                    self.scroll_down(viewport_height);
                }
            }
        }
    }

    pub fn cycle_focus_forward(&mut self) {
        self.input_mode = InputMode::Navigation;
        let visible_panels = self.get_visible_panel_order();
        if visible_panels.is_empty() {
            return;
        }
        let current_idx = visible_panels.iter().position(|&p| p == self.focus);
        match current_idx {
            Some(idx) => {
                let next_idx = (idx + 1) % visible_panels.len();
                self.focus = visible_panels[next_idx];
            }
            None => {
                self.focus = visible_panels[0];
            }
        }
    }

    pub fn cycle_focus_reverse(&mut self) {
        self.input_mode = InputMode::Navigation;
        let visible_panels = self.get_visible_panel_order();
        if visible_panels.is_empty() {
            return;
        }
        let current_idx = visible_panels.iter().position(|&p| p == self.focus);
        match current_idx {
            Some(idx) => {
                let next_idx = if idx == 0 { visible_panels.len() - 1 } else { idx - 1 };
                self.focus = visible_panels[next_idx];
            }
            None => {
                self.focus = visible_panels[0];
            }
        }
    }

    fn get_visible_panel_order(&self) -> Vec<Focus> {
        let mut panels = Vec::new();
        if self.lhs_visible {
            panels.push(Focus::Lhs);
        }
        panels.push(Focus::Main);
        if self.rhs_visible {
            panels.push(Focus::Rhs);
        }
        panels
    }

    pub fn toggle_active_widget(&mut self) {
        if self.focus == Focus::Lhs && self.input_mode == InputMode::Navigation {
            let widget = &mut self.lhs_widgets[self.active_lhs_widget];
            widget.enabled = !widget.enabled;
            widget.expanded = true;
        }
    }

    pub fn enter_text_mode(&mut self) {
        if self.focus == Focus::Lhs && self.input_mode == InputMode::Navigation {
            let widget = &mut self.lhs_widgets[self.active_lhs_widget];
            widget.expanded = true;
            self.input_mode = InputMode::TextInput;
        }
    }

    pub fn exit_text_mode(&mut self) {
        self.input_mode = InputMode::Navigation;
    }

    pub fn handle_text_input(&mut self, key: &str) {
        if self.focus == Focus::Lhs && self.input_mode == InputMode::TextInput {
            let widget = &mut self.lhs_widgets[self.active_lhs_widget];
            match key {
                "\u{7F}" | "\x08" => {
                    if widget.cursor_pos > 0 {
                        let bytes: Vec<u8> = widget.input_text.bytes().collect();
                        bytes[..widget.cursor_pos - 1].to_vec();
                        widget.input_text.remove(widget.cursor_pos - 1);
                        widget.cursor_pos -= 1;
                    }
                }
                "\n" | "\r" => {
                    self.exit_text_mode();
                }
                _ => {
                    widget.input_text.insert(widget.cursor_pos, key.chars().next().unwrap_or(' '));
                    widget.cursor_pos += 1;
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: TuiKey) {
        if self.error_log_visible {
            match key {
                TuiKey::Esc | TuiKey::Char('q') => self.toggle_error_log(),
                _ => {}
            }
            return;
        }

        match key {
            TuiKey::CtrlC | TuiKey::Char('q') => {}
            TuiKey::F(1) => self.toggle_lhs(),
            TuiKey::F(2) => self.toggle_rhs(),
            TuiKey::F(3) => self.toggle_error_log(),
            TuiKey::Esc => {
                if self.input_mode == InputMode::TextInput {
                    self.exit_text_mode();
                } else {
                    self.focus = Focus::Main;
                }
            }
            TuiKey::ShiftUp => {
                if self.focus == Focus::Lhs && self.active_lhs_widget > 0 {
                    self.active_lhs_widget -= 1;
                }
            }
            TuiKey::ShiftDown => {
                if self.focus == Focus::Lhs && self.active_lhs_widget < self.lhs_widgets.len() - 1 {
                    self.active_lhs_widget += 1;
                }
            }
            TuiKey::Tab => {
                self.cycle_focus_forward();
            }
            TuiKey::ShiftTab => {
                self.cycle_focus_reverse();
            }
            TuiKey::Up => {
                match self.focus {
                    Focus::Main => {
                        self.scroll_up(20);
                    }
                    Focus::Lhs => {
                        if self.input_mode == InputMode::TextInput {
                            let widget = &mut self.lhs_widgets[self.active_lhs_widget];
                            if widget.cursor_pos > 0 {
                                widget.cursor_pos -= 1;
                            }
                        } else if self.active_lhs_widget > 0 {
                            self.active_lhs_widget -= 1;
                        }
                    }
                    Focus::Rhs => {}
                }
            }
            TuiKey::Down => {
                match self.focus {
                    Focus::Main => {
                        let viewport_height = 20;
                        self.scroll_down(viewport_height);
                    }
                    Focus::Lhs => {
                        if self.input_mode == InputMode::TextInput {
                            let widget = &mut self.lhs_widgets[self.active_lhs_widget];
                            widget.cursor_pos += 1;
                        } else if self.active_lhs_widget < self.lhs_widgets.len() - 1 {
                            self.active_lhs_widget += 1;
                        }
                    }
                    Focus::Rhs => {}
                }
            }
            TuiKey::PageUp => {
                if self.focus == Focus::Main || self.focus == Focus::Lhs {
                    let viewport_height = 20;
                    self.page_up(viewport_height);
                }
            }
            TuiKey::PageDown => {
                if self.focus == Focus::Main || self.focus == Focus::Lhs {
                    let viewport_height = 20;
                    self.page_down(viewport_height);
                }
            }
            TuiKey::Space => {
                if self.focus == Focus::Lhs && self.input_mode == InputMode::Navigation {
                    let widget = &mut self.lhs_widgets[self.active_lhs_widget];
                    if !widget.enabled {
                        widget.enabled = true;
                        widget.expanded = true;
                        self.input_mode = InputMode::TextInput;
                    } else {
                        self.exit_text_mode();
                    }
                } else if self.focus == Focus::Main {
                    self.is_live_stream = !self.is_live_stream;
                }
            }
            TuiKey::Enter => {
                if self.focus == Focus::Lhs {
                    self.enter_text_mode();
                }
            }
            TuiKey::Char(c) => {
                if self.input_mode == InputMode::TextInput {
                    self.handle_text_input(&c.to_string());
                } else if c == 'l' && self.focus == Focus::Lhs {
                    self.move_focus_next(Direction::Right);
                } else if c == 'h' && (self.focus == Focus::Main || self.focus == Focus::Rhs) {
                    self.move_focus_next(Direction::Left);
                }
            }
            TuiKey::Left => {}
            TuiKey::Right => {}
            TuiKey::F(_) => {}
        }
    }

    pub fn active_filter_count(&self) -> usize {
        self.lhs_widgets.iter().filter(|w| w.active_filter_count()).count()
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn status_text(&self) -> String {
        let focus_str = match self.focus {
            Focus::Lhs => "LHS",
            Focus::Main => "MAIN",
            Focus::Rhs => "RHS",
        };
        let mode_str = match self.input_mode {
            InputMode::Navigation => "NAV",
            InputMode::TextInput => "INPUT",
        };
        let stream_str = if self.is_live_stream { "LIVE" } else { "MANUAL" };
        let layout_hint = if self.layout_vertical { "VERT" } else { "HORZ" };
        format!(
            " {} | {} | {} | {} | Filters:{} | Msgs:{} | [F1:LHS] [F2:RHS] [F3:Log] [F4:Layout] [Tab:Nxt] [Space:Stream] [Ctrl+C/q:Quit]",
            focus_str,
            mode_str,
            stream_str,
            layout_hint,
            self.active_filter_count(),
            self.message_count()
        )
    }

    pub fn area_for_focus(&self, area: Rect) -> Option<Rect> {
        match self.focus {
            Focus::Lhs if self.lhs_visible => Some(area),
            Focus::Main => Some(area),
            Focus::Rhs if self.rhs_visible => Some(area),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum TuiKey {
    Char(char),
    F(u8),
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    ShiftUp,
    ShiftDown,
    Tab,
    ShiftTab,
    Enter,
    Esc,
    Space,
    CtrlC,
}

fn filter_matches_sync(filter: &dyn crate::traits::Filter, message: &DecodedMessage) -> bool {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    rt.block_on(filter.matches(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_message(pgn: u32, title: &str) -> DecodedMessage {
        let mut msg = DecodedMessage::new(title.to_string());
        msg.assembled_message.pgn = pgn;
        msg
    }

    #[test]
    fn test_manual_mode_does_not_auto_select_latest() {
        let mut app = TuiApp::new();
        assert!(!app.is_live_stream);
        
        for i in 0..10 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // In manual mode, selected_index should stay at 0 (not auto-advance)
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_live_mode_auto_selects_latest() {
        let mut app = TuiApp::new();
        app.is_live_stream = true;
        
        for i in 0..10 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        assert_eq!(app.selected_index, 9);
    }

    #[test]
    fn test_scroll_down_keeps_selection_visible() {
        let mut app = TuiApp::new();
        
        for i in 0..100 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // Start at top
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.scroll_offset, 0);
        
        // Scroll down many times - selection should stay visible
        for _ in 0..60 {
            app.scroll_down(20);
        }
        
        let viewport_height = 20;
        assert!(app.selected_index < app.messages.len());
        assert!(app.selected_index >= app.scroll_offset);
        assert!(app.selected_index < app.scroll_offset + viewport_height as usize);
    }

    #[test]
    fn test_scroll_up_keeps_selection_visible() {
        let mut app = TuiApp::new();
        
        for i in 0..100 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // Jump to bottom manually
        app.selected_index = 99;
        app.scroll_offset = 80;
        
        // Scroll up - selection should stay visible
        for _ in 0..50 {
            app.scroll_up(20);
        }
        
        let viewport_height = 20;
        assert!(app.selected_index >= app.scroll_offset);
        assert!(app.selected_index < app.scroll_offset + viewport_height as usize);
    }

    #[test]
    fn test_cycle_focus_forward() {
        let mut app = TuiApp::new();
        app.lhs_visible = true;
        app.rhs_visible = true;
        
        // Visible order: [Lhs, Main, Rhs], forward cycles through them
        // Start at Main -> next is Rhs
        app.focus = Focus::Main;
        app.cycle_focus_forward();
        assert_eq!(app.focus, Focus::Rhs);
        
        app.cycle_focus_forward();
        assert_eq!(app.focus, Focus::Lhs);
        
        app.cycle_focus_forward();
        assert_eq!(app.focus, Focus::Main);
    }

    #[test]
    fn test_cycle_focus_reverse() {
        let mut app = TuiApp::new();
        app.lhs_visible = true;
        app.rhs_visible = true;
        
        // Start at Main
        app.focus = Focus::Main;
        app.cycle_focus_reverse();
        assert_eq!(app.focus, Focus::Lhs);
        
        app.cycle_focus_reverse();
        assert_eq!(app.focus, Focus::Rhs);
        
        app.cycle_focus_reverse();
        assert_eq!(app.focus, Focus::Main);
    }

    #[test]
    fn test_cycle_focus_with_hidden_panels() {
        let mut app = TuiApp::new();
        app.lhs_visible = true;
        app.rhs_visible = false;
        
        app.focus = Focus::Main;
        app.cycle_focus_forward();
        assert_eq!(app.focus, Focus::Lhs);
        
        app.cycle_focus_forward();
        assert_eq!(app.focus, Focus::Main);
    }

    #[test]
    fn test_widget_toggle_expands() {
        let mut app = TuiApp::new();
        app.focus = Focus::Lhs;
        
        let widget_idx = 0;
        assert!(!app.lhs_widgets[widget_idx].enabled);
        
        app.toggle_active_widget();
        
        assert!(app.lhs_widgets[widget_idx].enabled);
        // toggle_active_widget always sets expanded=true
        assert!(app.lhs_widgets[widget_idx].expanded);
    }

    #[test]
    fn test_enter_text_mode_expands() {
        let mut app = TuiApp::new();
        app.focus = Focus::Lhs;
        
        app.enter_text_mode();
        
        assert_eq!(app.input_mode, InputMode::TextInput);
        assert!(app.lhs_widgets[0].expanded);
    }

    #[test]
    fn test_exit_text_mode() {
        let mut app = TuiApp::new();
        app.focus = Focus::Lhs;
        app.input_mode = InputMode::TextInput;
        
        app.exit_text_mode();
        
        assert_eq!(app.input_mode, InputMode::Navigation);
    }

    #[test]
    fn test_toggle_layout() {
        let mut app = TuiApp::new();
        assert!(!app.layout_vertical);
        
        app.toggle_layout();
        assert!(app.layout_vertical);
        
        app.toggle_layout();
        assert!(!app.layout_vertical);
    }

    #[test]
    fn test_toggle_lhs() {
        let mut app = TuiApp::new();
        assert!(app.lhs_visible);
        
        app.toggle_lhs();
        assert!(!app.lhs_visible);
        
        app.toggle_lhs();
        assert!(app.lhs_visible);
    }

    #[test]
    fn test_toggle_rhs() {
        let mut app = TuiApp::new();
        assert!(app.rhs_visible);
        
        app.toggle_rhs();
        assert!(!app.rhs_visible);
        
        app.toggle_rhs();
        assert!(app.rhs_visible);
    }

    #[test]
    fn test_toggle_error_log() {
        let mut app = TuiApp::new();
        assert!(!app.error_log_visible);
        
        app.toggle_error_log();
        assert!(app.error_log_visible);
        
        app.toggle_error_log();
        assert!(!app.error_log_visible);
    }

    #[test]
    fn test_scroll_at_bottom_no_overflow() {
        let mut app = TuiApp::new();
        
        for i in 0..5 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // Scroll past the end - should not panic or overflow
        for _ in 0..100 {
            app.scroll_down(20);
        }
        
        assert_eq!(app.selected_index, 4);
    }

    #[test]
    fn test_scroll_at_top_no_underflow() {
        let mut app = TuiApp::new();
        
        for i in 0..5 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // Scroll up from top - should not panic or underflow
        for _ in 0..100 {
            app.scroll_up(20);
        }
        
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_get_visible_messages_manual_mode() {
        let mut app = TuiApp::new();
        
        for i in 0..50 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // At top - should show first viewport_height messages
        let visible = app.get_visible_messages(20);
        assert_eq!(visible.len(), 20);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_get_visible_messages_scrolled() {
        let mut app = TuiApp::new();
        
        for i in 0..50 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        // Scroll down
        app.scroll_offset = 30;
        app.selected_index = 35;
        
        let visible = app.get_visible_messages(20);
        assert_eq!(visible.len(), 20);
    }

    #[test]
    fn test_get_selected_message() {
        let mut app = TuiApp::new();
        
        for i in 0..5 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }
        
        assert!(app.get_selected_message().is_some());
        assert_eq!(app.get_selected_message().unwrap().title, "msg 0");
        
        app.selected_index = 3;
        assert_eq!(app.get_selected_message().unwrap().title, "msg 3");
    }

    #[test]
    fn test_get_visible_messages_empty() {
        let app = TuiApp::new();
        let visible = app.get_visible_messages(20);
        assert!(visible.is_empty());
    }
}
