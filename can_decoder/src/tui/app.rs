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
            is_live_stream: true,
            max_messages: 10000,
            lhs_widgets: vec![
                FilterWidget::new("Title", FilterType::Title),
                FilterWidget::new("PGN", FilterType::Pgn),
                FilterWidget::new("Severity", FilterType::Severity),
                FilterWidget::new("Source", FilterType::Source),
                FilterWidget::new("Dest", FilterType::Dest),
                FilterWidget::new("RPM", FilterType::Numeric),
                FilterWidget::new("Speed", FilterType::Numeric),
                FilterWidget::new("Engine", FilterType::Flag),
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
            } else if self.selected_index >= self.messages.len() {
                self.selected_index = self.messages.len() - 1;
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

        if self.is_live_stream {
            self.selected_index = self.messages.len() - 1;
            self.scroll_offset = self.messages.len().saturating_sub(1);
        } else {
            let max_scroll = self.messages.len().saturating_sub(viewport_height);
            if self.scroll_offset < max_scroll {
                self.scroll_offset += 1;
            }
            if self.selected_index < self.messages.len() - 1 {
                self.selected_index += 1;
            }
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn page_down(&mut self, viewport_height: usize) {
        let pages = viewport_height * 3;
        for _ in 0..pages {
            self.scroll_down(viewport_height);
        }
    }

    pub fn page_up(&mut self, viewport_height: usize) {
        let pages = viewport_height * 3;
        for _ in 0..pages {
            self.scroll_up();
        }
    }

    pub fn get_visible_messages(&self, viewport_height: usize) -> Vec<&DecodedMessage> {
        if self.messages.is_empty() {
            return vec![];
        }

        let total = self.messages.len();
        let start = if self.is_live_stream {
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
                    self.scroll_up();
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
        match self.focus {
            Focus::Lhs => {
                if self.rhs_visible {
                    self.focus = Focus::Rhs;
                } else {
                    self.focus = Focus::Main;
                }
            }
            Focus::Main => {
                if self.lhs_visible {
                    self.focus = Focus::Lhs;
                } else if self.rhs_visible {
                    self.focus = Focus::Rhs;
                } else {
                    self.focus = Focus::Lhs;
                }
            }
            Focus::Rhs => {
                if self.lhs_visible {
                    self.focus = Focus::Lhs;
                } else {
                    self.focus = Focus::Main;
                }
            }
        }
    }

    pub fn cycle_focus_reverse(&mut self) {
        self.input_mode = InputMode::Navigation;
        match self.focus {
            Focus::Lhs => {
                if self.rhs_visible {
                    self.focus = Focus::Rhs;
                } else {
                    self.focus = Focus::Main;
                }
            }
            Focus::Main => {
                if self.rhs_visible {
                    self.focus = Focus::Rhs;
                } else if self.lhs_visible {
                    self.focus = Focus::Lhs;
                } else {
                    self.focus = Focus::Rhs;
                }
            }
            Focus::Rhs => {
                if self.lhs_visible {
                    self.focus = Focus::Lhs;
                } else {
                    self.focus = Focus::Main;
                }
            }
        }
    }

    pub fn toggle_active_widget(&mut self) {
        if self.focus == Focus::Lhs && self.input_mode == InputMode::Navigation {
            let widget = &mut self.lhs_widgets[self.active_lhs_widget];
            widget.enabled = !widget.enabled;
            if widget.enabled && widget.input_text.is_empty() {
                widget.expanded = true;
            }
        }
    }

    pub fn enter_text_mode(&mut self) {
        if self.focus == Focus::Lhs {
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
            TuiKey::Char('q') => {}
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
                        self.scroll_up();
                    }
                    Focus::Lhs => {}
                    Focus::Rhs => {}
                }
            }
            TuiKey::Down => {
                match self.focus {
                    Focus::Main => {
                        let viewport_height = 20;
                        self.scroll_down(viewport_height);
                    }
                    Focus::Lhs => {}
                    Focus::Rhs => {}
                }
            }
            TuiKey::PageUp => {
                let viewport_height = 20;
                self.page_up(viewport_height);
            }
            TuiKey::PageDown => {
                let viewport_height = 20;
                self.page_down(viewport_height);
            }
            TuiKey::Space => {
                if self.focus == Focus::Lhs && self.input_mode == InputMode::Navigation {
                    self.toggle_active_widget();
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
        format!(
            " {} | {} | {} | Filters:{} | Msgs:{} | [Space:Stream] [q:Quit]",
            focus_str,
            mode_str,
            stream_str,
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
}

fn filter_matches_sync(filter: &dyn crate::traits::Filter, message: &DecodedMessage) -> bool {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    rt.block_on(filter.matches(message))
}
