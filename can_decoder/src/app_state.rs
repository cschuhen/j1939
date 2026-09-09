use crate::columns::ColumnConfig;
use crate::device_manager::DeviceManager;
use crate::filter_editor::{FieldType, FilterEditor, FilterEditorState};
use crate::filter_engine::FilterEngine;
use crate::filters::{
    DestFilter, DestNameFilter, FlagFilter, NumericFilter, PgnFilter, RegexFilter, SeverityFilter,
    SourceFilter, SourceNameFilter, TitleFilter,
};
use crate::filter_state::FilterState;
use crate::latest_index::LatestKey;
use crate::scroll_manager::MessageScrollManager;
use crate::types::{DecodedMessage, FlagValue, Severity};

use serde::{Deserialize, Serialize};

pub struct FilterEditorModal {
    pub state: FilterEditorState,
    pub editor: FilterEditor,
}

pub struct ColumnEditorModal {
    pub config: ColumnConfig,
    pub selected_index: usize,
    pub confirmed: bool,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    Title,
    Pgn,
    Severity,
    Source,
    Dest,
    Numeric,
    Flag,
    SourceName,
    DestName,
    Regex,
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
            FilterType::SourceName => format!(
                "src-name:{:016X}",
                parse_hex_or_dec_u64(&self.input_text).unwrap_or(0)
            ),
            FilterType::DestName => format!(
                "dest-name:{:016X}",
                parse_hex_or_dec_u64(&self.input_text).unwrap_or(0)
            ),
            FilterType::Regex => format!("regex:{}", self.input_text),
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
                let sources: Vec<u8> = self
                    .input_text
                    .split(',')
                    .filter_map(|s| parse_hex_or_dec_u8(s.trim()).ok())
                    .collect();
                if sources.is_empty() {
                    return None;
                }
                Some(Box::new(SourceFilter::from_list(sources)))
            }
            FilterType::Dest => {
                let dests: Vec<u8> = self
                    .input_text
                    .split(',')
                    .filter_map(|s| parse_hex_or_dec_u8(s.trim()).ok())
                    .collect();
                if dests.is_empty() {
                    return None;
                }
                Some(Box::new(DestFilter::from_list(dests)))
            }
            FilterType::Numeric => {
                let title = self.name.clone();
                if self.input_text.contains(',') {
                    let exact_values: Vec<f64> = self
                        .input_text
                        .split(',')
                        .filter_map(|s| parse_hex_or_dec_f64(s.trim()).ok())
                        .collect();
                    if exact_values.is_empty() {
                        return None;
                    }
                    Some(Box::new(NumericFilter {
                        title,
                        min: None,
                        max: None,
                        exact_values,
                    }))
                } else {
                    let (min, max) = parse_range(&self.input_text);
                    Some(Box::new(NumericFilter {
                        title,
                        min,
                        max,
                        exact_values: vec![],
                    }))
                }
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
            FilterType::SourceName => {
                let source_name = parse_hex_or_dec_u64(&self.input_text).ok()?;
                Some(Box::new(SourceNameFilter::new(source_name)))
            }
            FilterType::DestName => {
                let dest_name = parse_hex_or_dec_u64(&self.input_text).ok()?;
                Some(Box::new(DestNameFilter::new(dest_name)))
            }
            FilterType::Regex => {
                let regex = RegexFilter::new(&self.input_text).ok()?;
                Some(Box::new(regex))
            }
        }
    }

    pub fn active_filter_count(&self) -> bool {
        self.enabled && !self.input_text.is_empty()
    }

    /// Convert this widget's FilterType to a FieldType for the modal editor.
    /// Returns None for filter types that don't use the modal (Flag, Severity, Numeric, Regex).
    pub fn to_field_type(&self) -> Option<FieldType> {
        match self.filter_type {
            FilterType::Source => Some(FieldType::SourceAddr),
            FilterType::Dest => Some(FieldType::DestAddr),
            FilterType::SourceName => Some(FieldType::SrcName),
            FilterType::DestName => Some(FieldType::DstName),
            FilterType::Pgn => Some(FieldType::Pgn),
            FilterType::Title => Some(FieldType::Title),
            FilterType::Severity | FilterType::Numeric | FilterType::Flag | FilterType::Regex => {
                None
            }
        }
    }
}

fn parse_hex_or_dec_u32(s: &str) -> Result<u32, ()> {
    s.parse().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        u32::from_str_radix(stripped, 16).map_err(|_| ())
    })
}

fn parse_hex_or_dec_u64(s: &str) -> Result<u64, ()> {
    s.parse().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        u64::from_str_radix(stripped, 16).map_err(|_| ())
    })
}

fn parse_range(s: &str) -> (Option<f64>, Option<f64>) {
    if let Some(stripped) = s.strip_prefix(">=") {
        (
            Some(parse_hex_or_dec_f64(stripped).unwrap_or(f64::MIN)),
            None,
        )
    } else if let Some(stripped) = s.strip_prefix("<=") {
        (
            None,
            Some(parse_hex_or_dec_f64(stripped).unwrap_or(f64::MAX)),
        )
    } else if let Some(pos) = s.find('-') {
        let min_val: f64 = parse_hex_or_dec_f64(&s[..pos]).unwrap_or(f64::MIN);
        let max_val: f64 = parse_hex_or_dec_f64(&s[pos + 1..]).unwrap_or(f64::MAX);
        (Some(min_val), Some(max_val))
    } else {
        let val: f64 = parse_hex_or_dec_f64(s).unwrap_or(f64::MIN);
        (Some(val), Some(val))
    }
}

fn parse_hex_or_dec_f64(s: &str) -> Result<f64, ()> {
    s.parse::<f64>().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        u64::from_str_radix(stripped, 16)
            .map(|v| v as f64)
            .map_err(|_| ())
    })
}

fn parse_hex_or_dec_u8(s: &str) -> Result<u8, ()> {
    s.parse::<u8>().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        u8::from_str_radix(stripped, 16).map_err(|_| ())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Log,
    Latest,
}

pub struct TuiApp {
    pub focus: Focus,
    pub input_mode: InputMode,
    pub lhs_visible: bool,
    pub rhs_visible: bool,
    pub error_log_visible: bool,
    pub engine: FilterEngine,
    pub selected_index: usize,
    pub scroll_manager: MessageScrollManager,
    pub is_live_stream: bool,
    pub max_messages: usize,
    pub lhs_widgets: Vec<FilterWidget>,
    pub active_lhs_widget: usize,
    pub device_manager: DeviceManager,
    pub error_log: Vec<String>,
    pub connection_status: ConnectionStatus,
    pub layout_vertical: bool,

    /// Modal editor state (None = no modal open).
    pub filter_editor: Option<FilterEditorModal>,

    /// Which LHS widget index is being edited by the modal.
    pub editing_widget_index: usize,

    /// Column configuration popup (None = not open).
    pub column_editor: Option<ColumnEditorModal>,

    /// Persistent column configuration for the main message view.
    pub column_config: ColumnConfig,

    /// Timestamp of the very first unfiltered message (microseconds since epoch).
    pub global_start_time: Option<u64>,

    /// Current view mode: full log or latest-per-key.
    pub view_mode: ViewMode,

    /// Stable identity of the selected row in Latest mode (None = nothing selected).
    pub selected_latest_key: Option<LatestKey>,

    /// Space in Latest mode freezes/unfreezes the snapshot instead of toggling live stream.
    pub latest_frozen: bool,

    /// (key, global_idx) rows captured when the Latest view is frozen.
    pub frozen_snapshot: Vec<(LatestKey, usize)>,
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
            engine: FilterEngine::new(),
            selected_index: 0,
            scroll_manager: MessageScrollManager::new(),
            is_live_stream: false,
            max_messages: 10000,
            lhs_widgets: vec![
                FilterWidget::new("Title", FilterType::Title),
                FilterWidget::new("PGN", FilterType::Pgn),
                FilterWidget::new("Severity", FilterType::Severity),
                FilterWidget::new("Source Addr", FilterType::Source),
                FilterWidget::new("Dest Addr", FilterType::Dest),
                FilterWidget::new("Src Name", FilterType::SourceName),
                FilterWidget::new("Dst Name", FilterType::DestName),
                FilterWidget::new("Regex", FilterType::Regex),
            ],
            active_lhs_widget: 0,
            device_manager: DeviceManager::new(60),
            error_log: Vec::new(),
            connection_status: ConnectionStatus::Disconnected,
            layout_vertical: false,
            filter_editor: None,
            editing_widget_index: 0,
            column_editor: None,
            column_config: ColumnConfig::new(),
            global_start_time: None,
            view_mode: ViewMode::Log,
            selected_latest_key: None,
            latest_frozen: false,
            frozen_snapshot: Vec::new(),
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

    pub fn toggle_view_mode(&mut self) {
        match self.view_mode {
            ViewMode::Log => {
                self.view_mode = ViewMode::Latest;
                // Select first row (smallest key); None if the filtered set is empty
                self.selected_latest_key = self.engine.get_latest_map().keys().next().copied();
                self.sync_latest_scroll();
            }
            ViewMode::Latest => {
                self.view_mode = ViewMode::Log;
                self.latest_frozen = false;
                self.frozen_snapshot.clear();
                // selected_index is already valid for Log mode (clamped by apply_filters)
            }
        }
    }

    /// Re-resolve the selected key to its current row position and let the scroll manager
    /// keep it visible. Called only on mutation (message add / filter change / toggle),
    /// never per frame. O(k) scan with k <= 10000 is fine at that frequency.
    fn sync_latest_scroll(&mut self) {
        let rows = if self.latest_frozen {
            self.frozen_snapshot.len()
        } else {
            self.engine.latest_count()
        };
        self.scroll_manager.set_num_messages(rows);

        match self.selected_latest_key {
            Some(key) => {
                let map = self.engine.get_latest_map();
                if let Some(row) = map.iter().position(|(k, _)| *k == key) {
                    self.scroll_manager.selected_index = row;
                    self.scroll_manager.ensure_selected_visible();
                } else {
                    self.selected_latest_key = None; // key lost all passing messages
                }
            }
            None => {}
        }
    }

    pub fn add_message(&mut self, message: DecodedMessage) {
        let was_at_bottom = self.is_live_stream && self.selected_index >= self.engine.total_count();

        if self.engine.total_count() >= self.max_messages {
            // Engine doesn't have a pop_front, so we clear and rebuild without the oldest
            // For simplicity, just skip adding when at capacity (oldest messages are less relevant)
            return;
        }

        if self.global_start_time.is_none() {
            self.global_start_time = Some(message.timestamp());
        }

        let latest_live = self.view_mode == ViewMode::Latest && !self.latest_frozen;
        self.engine.add_message(message);

        if latest_live {
            // Same logical row stays selected even though its index may have shifted (R6)
            self.sync_latest_scroll();
        } else if self.view_mode == ViewMode::Log {
            if self.is_live_stream || was_at_bottom {
                let total = self.engine.total_count();
                if total > 0 {
                    self.selected_index = total - 1;
                }
            }

            // Update scroll manager with filtered count for viewport calculations
            self.scroll_manager
                .set_num_messages(self.engine.filtered_count());
        }
    }

    pub fn apply_filters(&mut self) {
        let filters: Vec<Box<dyn crate::traits::Filter>> = self
            .lhs_widgets
            .iter()
            .filter_map(|w| w.build_filter())
            .collect();
        for widget in &mut self.lhs_widgets {
            if widget.enabled && widget.build_filter().is_none() {
                widget.enabled = false;
            }
        }
        self.engine.set_filters(filters);

        // Clamp selection to new filtered set
        let filtered = self.engine.filtered_count();
        if filtered == 0 {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(filtered - 1);
        }

        if self.view_mode == ViewMode::Latest && !self.latest_frozen {
            // A filter change can remove keys whose newest message no longer passes
            let still_present = self
                .selected_latest_key
                .is_some_and(|k| self.engine.get_latest_map().contains_key(&k));
            if !still_present {
                self.selected_latest_key = None;
            }
            self.sync_latest_scroll();
        } else {
            self.scroll_manager
                .set_num_messages(self.engine.filtered_count());
        }
    }

    /// Snapshot the current filter widgets for persistence.
    pub fn save_filter_state(&self) -> FilterState {
        FilterState::from_widgets(&self.lhs_widgets)
    }

    /// Restore persisted filter state, replacing the default widget set and re-applying.
    pub fn load_filter_state(&mut self, state: FilterState) {
        self.lhs_widgets = state.to_widgets();
        if !self.lhs_widgets.is_empty() {
            self.active_lhs_widget = 0;
        }
        self.apply_filters();
    }

    /// Ordered (key, global_idx) rows for the Latest view: live map or frozen snapshot.
    fn latest_rows(&mut self) -> Vec<(LatestKey, usize)> {
        if self.latest_frozen {
            self.frozen_snapshot.clone()
        } else {
            self.engine
                .get_latest_map()
                .iter()
                .map(|(k, &gi)| (*k, gi))
                .collect()
        }
    }

    pub fn scroll_down(&mut self, steps: usize) {
        if self.view_mode == ViewMode::Latest {
            let rows = self.latest_rows();
            if rows.is_empty() {
                return;
            }

            self.scroll_manager.set_num_messages(rows.len());

            // No live-follow in Latest mode: bottom is just the largest key, not "newest"
            let mut row = self.scroll_manager.selected_index + steps;
            if row >= rows.len() {
                row = rows.len() - 1;
            }
            self.scroll_manager.selected_index = row;
            self.selected_latest_key = Some(rows[row].0);
            self.scroll_manager.ensure_selected_visible();
            return;
        }

        if self.engine.filtered_count() == 0 {
            return;
        }

        let filtered = self.engine.filtered_count();
        self.scroll_manager.set_num_messages(filtered);

        // In live mode at bottom, auto-follow new messages
        if self.is_live_stream && self.selected_index >= filtered - 1 {
            self.selected_index = filtered - 1;
            return;
        }

        self.selected_index += steps;
        if self.selected_index >= filtered {
            self.selected_index = filtered - 1;
        }

        self.scroll_manager.selected_index = self.selected_index;
        self.scroll_manager.ensure_selected_visible();
    }

    pub fn scroll_up(&mut self, steps: usize) {
        if self.view_mode == ViewMode::Latest {
            let rows = self.latest_rows();
            if rows.is_empty() {
                return;
            }

            self.scroll_manager.set_num_messages(rows.len());

            let row = self.scroll_manager.selected_index.saturating_sub(steps);
            self.scroll_manager.selected_index = row;
            self.selected_latest_key = Some(rows[row].0);
            self.scroll_manager.ensure_selected_visible();
            return;
        }

        // Switch from live to manual when scrolling up from bottom
        let filtered = self.engine.filtered_count();
        if self.is_live_stream && self.selected_index >= filtered - 1 {
            self.is_live_stream = false;
        }

        if filtered == 0 {
            return;
        }

        self.scroll_manager.set_num_messages(filtered);

        self.selected_index = self.selected_index.saturating_sub(steps);

        self.scroll_manager.selected_index = self.selected_index;
        self.scroll_manager.ensure_selected_visible();
    }

    pub fn page_down(&mut self) {
        if self.engine.filtered_count() == 0 {
            return;
        }

        self.scroll_manager
            .set_num_messages(self.engine.filtered_count());

        self.scroll_down(self.scroll_manager.num_rows - self.scroll_manager.look_ahead_bottom);
    }

    pub fn page_up(&mut self) {
        if self.engine.filtered_count() == 0 {
            return;
        }

        // Switch from live to manual when scrolling up from bottom
        let filtered = self.engine.filtered_count();
        if self.is_live_stream && self.selected_index >= filtered - 1 {
            self.is_live_stream = false;
        }

        self.scroll_manager.set_num_messages(filtered);

        self.scroll_up(self.scroll_manager.num_rows - self.scroll_manager.look_ahead_top);
    }

    pub fn get_visible_messages(&mut self, viewport_height: usize) -> Vec<&DecodedMessage> {
        self.scroll_manager.set_num_rows(viewport_height);

        if self.view_mode == ViewMode::Latest {
            let rows = self.latest_rows();
            self.scroll_manager.set_num_messages(rows.len());

            if rows.is_empty() {
                return vec![];
            }

            let (start, end) = self.scroll_manager.get_visible_range();
            return rows[start.min(rows.len())..end.min(rows.len())]
                .iter()
                .filter_map(|&(_, gi)| self.engine.get_message_by_global_index(gi))
                .collect();
        }

        let filtered = self.engine.filtered_count();
        self.scroll_manager.set_num_messages(filtered);

        if filtered == 0 {
            return vec![];
        }

        // Collect indices first to avoid borrow conflicts
        let indices = self.engine.get_filtered_indices().to_vec();

        let (start, end) = self.scroll_manager.get_visible_range();
        let start = start.min(indices.len());
        let end = end.min(indices.len());

        let mut result = Vec::new();
        for &global_idx in &indices[start..end] {
            if let Some(msg) = self.engine.get_message_by_global_index(global_idx) {
                result.push(msg);
            }
        }
        result
    }

    pub fn get_selected_message(&mut self) -> Option<&DecodedMessage> {
        if self.view_mode == ViewMode::Latest {
            let gi = if self.latest_frozen {
                self.frozen_snapshot
                    .iter()
                    .find(|(k, _)| Some(*k) == self.selected_latest_key)
                    .map(|(_, gi)| *gi)
            } else {
                self.engine
                    .get_latest_map()
                    .get(self.selected_latest_key.as_ref()?)
                    .copied()
            };
            return gi.and_then(|gi| self.engine.get_message_by_global_index(gi));
        }

        let filtered = self.engine.filtered_count();
        if filtered == 0 {
            return None;
        }
        let idx = self.selected_index.min(filtered - 1);
        self.engine.get_message_at(idx)
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
                    self.scroll_up(1);
                }
            }
            Direction::Down => {
                if self.focus == Focus::Lhs {
                    if self.active_lhs_widget < self.lhs_widgets.len() - 1 {
                        self.active_lhs_widget += 1;
                    }
                } else if self.focus == Focus::Main {
                    self.scroll_down(1);
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
                let next_idx = if idx == 0 {
                    visible_panels.len() - 1
                } else {
                    idx - 1
                };
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

    /// Open the modal filter editor for the currently active LHS widget.
    pub fn open_filter_editor(&mut self) {
        if self.focus != Focus::Lhs || self.input_mode != InputMode::Navigation {
            return;
        }

        let widget = &self.lhs_widgets[self.active_lhs_widget];
        let field_type = match widget.to_field_type() {
            Some(ft) => ft,
            None => return, // This filter type doesn't use the modal
        };

        self.editing_widget_index = self.active_lhs_widget;
        let options = self.engine.get_unique_options(field_type);
        let editor = FilterEditor::new(field_type);
        let state = editor.open(&widget.input_text, options);

        self.filter_editor = Some(FilterEditorModal { state, editor });
    }

    /// Close the modal filter editor, optionally applying changes.
    pub fn close_filter_editor(&mut self, apply: bool) {
        if let Some(modal) = self.filter_editor.take() {
            if apply && !modal.state.selected_ids.is_empty() {
                let csv = modal.editor.get_output_csv(&modal.state);
                if let Some(csv_text) = csv {
                    if let Some(widget) = self.lhs_widgets.get_mut(self.editing_widget_index) {
                        widget.input_text = csv_text;
                        widget.enabled = true;
                        widget.expanded = true;
                        self.apply_filters();
                    }
                }
            }
        }
    }

    /// Open the column configuration popup.
    pub fn open_column_editor(&mut self) {
        if self.column_editor.is_some() {
            return;
        }
        let config = ColumnConfig::new();
        self.column_editor = Some(ColumnEditorModal {
            config,
            selected_index: 0,
            confirmed: false,
        });
    }

    /// Close the column editor popup.
    pub fn close_column_editor(&mut self, apply: bool) {
        if let Some(modal) = self.column_editor.take() {
            if apply {
                self.column_config = modal.config;
            }
        }
    }

    /// Handle a key event while the modal editor is open.
    /// Returns true if the key was consumed by the modal (should not propagate to main handler).
    pub fn handle_modal_key(&mut self, key: &str) -> bool {
        let modal = match &mut self.filter_editor {
            Some(m) => m,
            None => return false,
        };

        let viewport_height = 14;

        match key {
            "\t" | "Tab" => {
                // Toggle focus between text field and list
                if modal.state.is_text_focused() {
                    modal.state.move_down(viewport_height);
                } else {
                    modal.state.focus_index = -1;
                    // Re-sync text from list selection
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                }
                true
            }
            "Enter" | "\n" => {
                if modal.editor.can_accept(&modal.state) {
                    self.close_filter_editor(true);
                } else {
                    // Beep / invalid - just stay open
                }
                true
            }
            "Esc" => {
                self.close_filter_editor(false);
                true
            }
            "Space" => {
                if modal.state.is_list_focused() && !modal.state.options.is_empty() {
                    modal.state.toggle_focused();
                    // Re-sync text from list selection
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                } else if modal.state.is_text_focused() {
                    // Insert space character in text field
                    modal.state.text_input.insert(modal.state.cursor_pos, ' ');
                    modal.state.cursor_pos += 1;
                    let (validation, selected_ids) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                    modal.state.selected_ids = selected_ids;
                }
                true
            }
            "Up" => {
                if modal.state.is_list_focused() {
                    modal.state.move_up(viewport_height);
                    // Re-sync text from list selection
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                } else {
                    // Text field: move cursor left
                    if modal.state.cursor_pos > 0 {
                        modal.state.cursor_pos -= 1;
                    }
                }
                true
            }
            "Down" => {
                if modal.state.is_list_focused() {
                    modal.state.move_down(viewport_height);
                    // Re-sync text from list selection
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                } else {
                    // Text field: move cursor right
                    modal.state.cursor_pos = modal.state.cursor_pos.saturating_add(1);
                }
                true
            }
            "PageUp" => {
                if modal.state.is_list_focused() {
                    modal.state.page_up(viewport_height);
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                }
                true
            }
            "PageDown" => {
                if modal.state.is_list_focused() {
                    modal.state.page_down(viewport_height);
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                }
                true
            }
            "Home" => {
                if modal.state.is_list_focused() {
                    modal.state.go_to_first(viewport_height);
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                } else {
                    modal.state.cursor_pos = 0;
                }
                true
            }
            "End" => {
                if modal.state.is_list_focused() {
                    modal.state.go_to_last(viewport_height);
                    let new_text = modal.editor.reconstruct_text(&modal.state);
                    modal.state.text_input = new_text;
                    let (validation, _) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                } else {
                    modal.state.cursor_pos = modal.state.text_input.chars().count();
                }
                true
            }
            "\u{7F}" | "\x08" => {
                // Backspace in text field
                if modal.state.is_text_focused() && modal.state.cursor_pos > 0 {
                    let byte_idx = modal.state.char_to_byte(modal.state.cursor_pos - 1);
                    modal.state.text_input.remove(byte_idx);
                    modal.state.cursor_pos -= 1;
                    // Re-validate
                    let (validation, selected_ids) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                    modal.state.selected_ids = selected_ids;
                }
                true
            }
            _ => {
                // Character key - append to text input if text field is focused
                if modal.state.is_text_focused() {
                    let byte_idx = modal.state.char_to_byte(modal.state.cursor_pos);
                    let ch = key.chars().next().unwrap_or(' ');
                    modal.state.text_input.insert(byte_idx, ch);
                    modal.state.cursor_pos += 1;
                    let (validation, selected_ids) = modal.editor.update_from_text(&modal.state);
                    modal.state.validation = validation;
                    modal.state.selected_ids = selected_ids;
                }
                true
            }
        }
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
                    self.apply_filters();
                    self.exit_text_mode();
                }
                _ => {
                    widget
                        .input_text
                        .insert(widget.cursor_pos, key.chars().next().unwrap_or(' '));
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

        // Route keys through modal editor first if it's open
        if self.filter_editor.is_some() {
            let consumed = match &key {
                TuiKey::Esc => self.handle_modal_key("Esc"),
                TuiKey::Enter | TuiKey::Char('\n') => self.handle_modal_key("Enter"),
                TuiKey::Tab => self.handle_modal_key("\t"),
                TuiKey::ShiftTab => self.handle_modal_key("\t"),
                TuiKey::Space => self.handle_modal_key("Space"),
                TuiKey::Up => self.handle_modal_key("Up"),
                TuiKey::Down => self.handle_modal_key("Down"),
                TuiKey::PageUp => self.handle_modal_key("PageUp"),
                TuiKey::PageDown => self.handle_modal_key("PageDown"),
                TuiKey::Left => self.handle_modal_key("Home"),
                TuiKey::Right => self.handle_modal_key("End"),
                TuiKey::Char(c) => self.handle_modal_key(&c.to_string()),
                _ => false,
            };
            if consumed {
                return;
            }
        }

        // Route keys through filter editor modal if it's open (only Esc/Enter propagate)
        if self.filter_editor.is_some() {
            match &key {
                TuiKey::Esc | TuiKey::Enter | TuiKey::Char('\n') => {}
                _ => {
                    return;
                }
            }
        }

        match key {
            TuiKey::CtrlC | TuiKey::Char('q') => {}
            TuiKey::F(1) => self.toggle_lhs(),
            TuiKey::F(2) => self.toggle_rhs(),
            TuiKey::F(3) => self.toggle_error_log(),
            TuiKey::F(4) => self.toggle_layout(),
            TuiKey::F(5) => {
                if self.column_editor.is_none() {
                    self.open_column_editor();
                }
            }
            TuiKey::F(6) => self.toggle_view_mode(),
            TuiKey::Esc => {
                if let Some(_) = self.column_editor {
                    self.close_column_editor(false);
                    return;
                }
                if self.input_mode == InputMode::TextInput {
                    self.apply_filters();
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
                if self.input_mode == InputMode::TextInput {
                    self.apply_filters();
                }
                self.cycle_focus_forward();
            }
            TuiKey::ShiftTab => {
                if self.input_mode == InputMode::TextInput {
                    self.apply_filters();
                }
                self.cycle_focus_reverse();
            }
            TuiKey::Up => match self.focus {
                Focus::Main => {
                    if self.column_editor.is_some() {
                        let idx = self.column_editor.as_mut().unwrap().selected_index;
                        if idx > 0 {
                            self.column_editor.as_mut().unwrap().selected_index = idx - 1;
                        }
                    } else {
                        self.scroll_up(1);
                    }
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
            },
            TuiKey::Down => match self.focus {
                Focus::Main => {
                    if let Some(ref mut modal) = self.column_editor {
                        let cols_len = modal.config.states.len();
                        if cols_len > 0 {
                            if modal.selected_index < cols_len - 1 {
                                modal.selected_index += 1;
                            }
                        }
                    } else {
                        self.scroll_down(1);
                    }
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
            },
            TuiKey::PageUp => {
                if self.focus == Focus::Main || self.focus == Focus::Lhs {
                    self.page_up();
                }
            }
            TuiKey::PageDown => {
                if self.focus == Focus::Main || self.focus == Focus::Lhs {
                    self.page_down();
                }
            }
            TuiKey::Space => {
                if let Some(ref mut modal) = self.column_editor {
                    let cols = &mut modal.config.states;
                    if !cols.is_empty() {
                        let idx = modal.selected_index.min(cols.len() - 1);
                        cols[idx].enabled = !cols[idx].enabled;
                    }
                } else if self.focus == Focus::Lhs && self.input_mode == InputMode::Navigation {
                    let widget = &mut self.lhs_widgets[self.active_lhs_widget];
                    if !widget.enabled {
                        widget.enabled = true;
                        widget.expanded = true;
                        self.input_mode = InputMode::TextInput;
                    } else {
                        self.exit_text_mode();
                    }
                } else if self.focus == Focus::Main {
                    if self.view_mode == ViewMode::Latest {
                        if self.latest_frozen {
                            self.latest_frozen = false;
                            self.frozen_snapshot.clear();
                            self.sync_latest_scroll(); // catch up to live data
                        } else {
                            self.latest_frozen = true;
                            self.frozen_snapshot = self
                                .engine
                                .get_latest_map()
                                .iter()
                                .map(|(k, &gi)| (*k, gi))
                                .collect();
                        }
                    } else {
                        self.is_live_stream = !self.is_live_stream;
                    }
                }
            }
            TuiKey::Enter => {
                if self.column_editor.is_some() {
                    self.close_column_editor(true);
                    return;
                }
                if self.focus == Focus::Lhs && self.input_mode == InputMode::TextInput {
                    // Check if this widget supports the modal editor
                    let widget = &self.lhs_widgets[self.active_lhs_widget];
                    if widget.to_field_type().is_some() {
                        self.open_filter_editor();
                    } else {
                        self.apply_filters();
                        self.exit_text_mode();
                    }
                } else if self.focus == Focus::Lhs {
                    let widget = &mut self.lhs_widgets[self.active_lhs_widget];
                    if !widget.enabled {
                        widget.enabled = true;
                        widget.expanded = true;
                    }
                    // Check if this widget supports the modal editor
                    if widget.to_field_type().is_some() {
                        self.open_filter_editor();
                    } else {
                        self.enter_text_mode();
                    }
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
        self.lhs_widgets
            .iter()
            .filter(|w| w.active_filter_count())
            .count()
    }

    pub fn message_count(&self) -> usize {
        self.engine.total_count()
    }

    /// Row count for the currently active view (Log: total, Latest: rows per key).
    pub fn view_row_count(&mut self) -> usize {
        if self.view_mode == ViewMode::Latest {
            if self.latest_frozen {
                self.frozen_snapshot.len()
            } else {
                self.engine.latest_count()
            }
        } else {
            self.message_count()
        }
    }

    pub fn status_text(&mut self) -> String {
        let focus_str = match self.focus {
            Focus::Lhs => "LHS",
            Focus::Main => "MAIN",
            Focus::Rhs => "RHS",
        };
        let mode_str = match self.input_mode {
            InputMode::Navigation => "NAV",
            InputMode::TextInput => "INPUT",
        };
        let stream_str = if self.is_live_stream {
            "LIVE"
        } else {
            "MANUAL"
        };
        let layout_hint = if self.layout_vertical { "VERT" } else { "HORZ" };
        let view_str = match (self.view_mode, self.latest_frozen) {
            (ViewMode::Log, _) => "LOG",
            (ViewMode::Latest, true) => "LATEST-FROZEN",
            (ViewMode::Latest, false) => "LATEST-LIVE",
        };
        let msgs = if self.view_mode == ViewMode::Latest {
            self.view_row_count()
        } else {
            self.engine.filtered_count()
        };
        format!(
            " {} | {} | {} | View:{} | {} | Filters:{} | Msgs:{}/{} | [F1:LHS] [F2:RHS] [F3:Log] [F4:Layout] [F5:Cols] [F6:Mode] [Tab:Nxt] [Space:Stream] [Ctrl+C/q:Quit]",
            focus_str,
            mode_str,
            stream_str,
            view_str,
            layout_hint,
            self.active_filter_count(),
            msgs,
            self.engine.total_count()
        )
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
    fn test_toggle_view_mode_selects_first_key() {
        let mut app = TuiApp::new();
        assert_eq!(app.view_mode, ViewMode::Log);

        for topic in [5u64, 3, 8] {
            let mut msg = make_test_message(0x600, "msg");
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode();

        assert_eq!(app.view_mode, ViewMode::Latest);
        // Keys are sorted by (src, dst, topic_id) -> smallest topic first
        let expected_key = LatestKey {
            source_address: 0,
            destination_address: 0,
            topic_id: 3,
        };
        assert_eq!(app.selected_latest_key, Some(expected_key));
        assert_eq!(app.scroll_manager.num_messages, 3);
    }

    #[test]
    fn test_toggle_view_mode_empty_engine_selects_none() {
        let mut app = TuiApp::new();
        app.toggle_view_mode();

        assert_eq!(app.view_mode, ViewMode::Latest);
        assert_eq!(app.selected_latest_key, None);
        assert_eq!(app.scroll_manager.num_messages, 0);
    }

    #[test]
    fn test_toggle_back_to_log_clears_freeze_state() {
        let mut app = TuiApp::new();
        let mut msg = make_test_message(0x600, "msg");
        msg.topic_id = 1;
        app.add_message(msg);

        app.toggle_view_mode(); // -> Latest
        assert_eq!(app.view_mode, ViewMode::Latest);

        app.latest_frozen = true;
        let key = app.selected_latest_key.unwrap();
        app.frozen_snapshot.push((key, 0));

        app.toggle_view_mode(); // -> Log

        assert_eq!(app.view_mode, ViewMode::Log);
        assert!(!app.latest_frozen);
        assert!(app.frozen_snapshot.is_empty());
    }

    #[test]
    fn test_latest_selection_stable_when_rows_shift() {
        let mut app = TuiApp::new();
        for topic in [10u64, 20] {
            let mut msg = make_test_message(0x600, "msg");
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode(); // selects key(topic=10) at row 0
        assert_eq!(app.scroll_manager.selected_index, 0);

        // Select the second key (topic=20), currently at row 1
        let key_20 = LatestKey {
            source_address: 0,
            destination_address: 0,
            topic_id: 20,
        };
        app.selected_latest_key = Some(key_20);
        app.sync_latest_scroll();
        assert_eq!(app.scroll_manager.selected_index, 1);

        // A new key sorts before the selected one -> rows shift down by one
        let mut msg = make_test_message(0x600, "msg");
        msg.topic_id = 5;
        app.add_message(msg);
        app.sync_latest_scroll();

        assert_eq!(app.selected_latest_key, Some(key_20));
        assert_eq!(app.scroll_manager.selected_index, 2);
    }

    #[test]
    fn test_latest_selection_cleared_when_key_disappears() {
        let mut app = TuiApp::new();
        for (topic, title) in [(1u64, "alpha"), (2, "beta")] {
            let mut msg = make_test_message(0x600, title);
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode(); // selects key(topic=1)
        assert!(app.selected_latest_key.is_some());

        // Filter out the selected key's message -> it leaves the latest map
        app.engine
            .set_filters(vec![Box::new(TitleFilter::new("beta"))]);
        app.sync_latest_scroll();

        assert_eq!(app.selected_latest_key, None);
    }

    #[test]
    fn test_f6_toggles_view_mode() {
        let mut app = TuiApp::new();
        app.handle_key(TuiKey::F(6));
        assert_eq!(app.view_mode, ViewMode::Latest);
        app.handle_key(TuiKey::F(6));
        assert_eq!(app.view_mode, ViewMode::Log);
    }

    #[test]
    fn test_latest_freeze_snapshot_isolated_from_new_messages() {
        let mut app = TuiApp::new();
        let mut msg = make_test_message(0x600, "msg");
        msg.topic_id = 1;
        app.add_message(msg);

        app.toggle_view_mode(); // -> Latest, selects key(topic=1)
        assert!(!app.latest_frozen);

        // Space in Main focus freezes the snapshot
        app.handle_key(TuiKey::Space);
        assert!(app.latest_frozen);
        assert_eq!(app.frozen_snapshot.len(), 1);

        // New message arrives while frozen: view must not change
        let mut msg = make_test_message(0x600, "msg");
        msg.topic_id = 2;
        app.add_message(msg);

        let visible = app.get_visible_messages(10);
        assert_eq!(visible.len(), 1); // still only the frozen row
        assert_eq!(
            app.selected_latest_key,
            Some(LatestKey {
                source_address: 0,
                destination_address: 0,
                topic_id: 1
            })
        );

        // Space again unfreezes and catches up to live data
        app.handle_key(TuiKey::Space);
        assert!(!app.latest_frozen);
        assert!(app.frozen_snapshot.is_empty());
        assert_eq!(app.scroll_manager.num_messages, 2);
    }

    #[test]
    fn test_space_in_log_mode_still_toggles_live_stream() {
        let mut app = TuiApp::new();
        assert!(!app.is_live_stream);
        app.handle_key(TuiKey::Space);
        assert!(app.is_live_stream);
        app.handle_key(TuiKey::Space);
        assert!(!app.is_live_stream);
    }

    #[test]
    fn test_apply_filters_in_latest_mode_clears_removed_key() {
        let mut app = TuiApp::new();
        for (topic, title) in [(1u64, "alpha"), (2, "beta")] {
            let mut msg = make_test_message(0x600, title);
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode(); // selects key(topic=1) / "alpha"
        assert_eq!(app.selected_latest_key.map(|k| k.topic_id), Some(1));

        // Enable a Title filter that excludes the selected message
        let widget = &mut app.lhs_widgets[0]; // Title widget
        widget.enabled = true;
        widget.input_text = "beta".to_string();
        app.apply_filters();

        assert_eq!(app.selected_latest_key, None);
        assert_eq!(app.scroll_manager.num_messages, 1);
    }

    #[test]
    fn test_apply_filters_in_latest_mode_keeps_surviving_key() {
        let mut app = TuiApp::new();
        for (topic, title) in [(1u64, "alpha"), (2, "beta")] {
            let mut msg = make_test_message(0x600, title);
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode(); // selects key(topic=1) / "alpha"

        // Filter that keeps the selected message
        let widget = &mut app.lhs_widgets[0];
        widget.enabled = true;
        widget.input_text = "alpha".to_string();
        app.apply_filters();

        assert_eq!(app.selected_latest_key.map(|k| k.topic_id), Some(1));
        assert_eq!(app.scroll_manager.num_messages, 1);
        assert_eq!(app.scroll_manager.selected_index, 0);
    }

    #[test]
    fn test_get_visible_messages_latest_shows_newest_passing_per_key() {
        let mut app = TuiApp::new();
        // Key topic=1 gets two messages; key topic=2 gets one
        for (topic, title) in [(1u64, "a-old"), (2, "b"), (1, "a-new")] {
            let mut msg = make_test_message(0x600, title);
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode();

        let visible = app.get_visible_messages(10);
        assert_eq!(visible.len(), 2);
        // Sorted by key: topic=1 first (its newest passing message), then topic=2
        assert_eq!(visible[0].title, "a-new");
        assert_eq!(visible[1].title, "b");
    }

    #[test]
    fn test_get_selected_message_latest_resolves_key() {
        let mut app = TuiApp::new();
        for (topic, title) in [(1u64, "alpha"), (2, "beta")] {
            let mut msg = make_test_message(0x600, title);
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode(); // selects key(topic=1)
        let sel = app.get_selected_message().unwrap();
        assert_eq!(sel.title, "alpha");

        // Scroll to the next row -> selection follows the key
        app.scroll_down(1);
        assert_eq!(app.selected_latest_key.map(|k| k.topic_id), Some(2));
        let sel = app.get_selected_message().unwrap();
        assert_eq!(sel.title, "beta");

        // Scroll back up past the top -> clamps to first key
        app.scroll_up(5);
        assert_eq!(app.selected_latest_key.map(|k| k.topic_id), Some(1));
    }

    #[test]
    fn test_latest_scroll_clamps_at_bounds() {
        let mut app = TuiApp::new();
        for topic in [1u64, 2, 3] {
            let mut msg = make_test_message(0x600, "msg");
            msg.topic_id = topic;
            app.add_message(msg);
        }

        app.toggle_view_mode(); // selects row 0 (topic=1)

        app.scroll_up(10); // stays at top
        assert_eq!(app.selected_latest_key.map(|k| k.topic_id), Some(1));
        assert_eq!(app.scroll_manager.selected_index, 0);

        for _ in 0..5 {
            app.scroll_down(2);
        } // clamps at bottom
        assert_eq!(app.selected_latest_key.map(|k| k.topic_id), Some(3));
        assert_eq!(app.scroll_manager.selected_index, 2);
    }

    #[test]
    fn test_scroll_down_keeps_selection_visible() {
        let mut app = TuiApp::new();

        for i in 0..100 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Start at top
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.scroll_manager.first_visible_message, 0);

        // Scroll down many times - selection should stay visible
        for _ in 0..60 {
            app.scroll_down(20);
        }

        let viewport_height = 20;
        assert!(app.selected_index < app.engine.total_count());
        assert!(app.selected_index >= app.scroll_manager.first_visible_message);
        assert!(
            app.selected_index
                < app.scroll_manager.first_visible_message + viewport_height as usize
        );
    }

    #[test]
    fn test_scroll_up_keeps_selection_visible() {
        let mut app = TuiApp::new();

        for i in 0..100 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Jump to bottom manually
        app.selected_index = 99;
        app.scroll_manager.first_visible_message = 80;

        // Scroll up - selection should stay visible
        for _ in 0..50 {
            app.scroll_up(20);
        }

        let viewport_height = 20;
        assert!(app.selected_index >= app.scroll_manager.first_visible_message);
        assert!(
            app.selected_index
                < app.scroll_manager.first_visible_message + viewport_height as usize
        );
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
        assert_eq!(app.scroll_manager.first_visible_message, 0);
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
        app.scroll_manager.first_visible_message = 30;
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
        let mut app = TuiApp::new();
        let visible = app.get_visible_messages(20);
        assert!(visible.is_empty());
    }

    #[test]
    fn test_add_message_never_drops_filtered() {
        // Phase 2: Messages are never dropped due to filtering.
        // All messages should be stored in the engine regardless of filter state.
        let mut app = TuiApp::new();

        for i in 0..10 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // All 10 messages should be stored (total_count)
        assert_eq!(app.engine.total_count(), 10);
        // Without filters, all pass
        assert_eq!(app.engine.filtered_count(), 10);
    }

    #[test]
    fn test_apply_filters_clamps_selection() {
        let mut app = TuiApp::new();

        for i in 0..20 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Set selection near the end
        app.selected_index = 15;

        // Apply a title filter that matches only "msg 0" (substring match)
        let widget = &mut app.lhs_widgets[0]; // TitleFilter widget at index 0
        widget.enabled = true;
        widget.input_text = "msg 0".to_string();

        assert!(widget.build_filter().is_some());

        app.apply_filters();

        // Selection should be clamped to the filtered set (1 message matching "msg 0")
        let filtered = app.engine.filtered_count();
        assert!(
            filtered > 0,
            "filtered_count should be > 0 but was {}",
            filtered
        );
        assert!(
            app.selected_index < filtered,
            "selected_index {} >= filtered {}",
            app.selected_index,
            filtered
        );
    }

    #[test]
    fn test_status_bar_shows_pass_total() {
        let mut app = TuiApp::new();

        for i in 0..10 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Without filters: pass == total
        let status = app.status_text();
        assert!(status.contains("Msgs:10/10"));
    }

    #[test]
    fn test_status_bar_shows_view_mode() {
        let mut app = TuiApp::new();

        for i in 0..10 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Log mode: filtered count shown
        assert!(app.status_text().contains("View:LOG"));

        app.toggle_view_mode();
        let status = app.status_text();
        assert!(status.contains("View:LATEST-LIVE"));
        assert!(status.contains("[F6:Mode]"));
        // All test messages share one key -> 1 latest row out of 10 total
        assert!(status.contains(&format!("Msgs:{}/", app.view_row_count())));

        app.latest_frozen = true;
        assert!(app.status_text().contains("View:LATEST-FROZEN"));
    }

    #[test]
    fn test_scroll_with_filters() {
        let mut app = TuiApp::new();

        for i in 0..20 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Apply a title filter that matches "msg 1" (messages msg 1, msg 10-19)
        let widget = &mut app.lhs_widgets[0]; // TitleFilter widget at index 0
        widget.enabled = true;
        widget.input_text = "msg 1".to_string();

        app.apply_filters();

        assert_eq!(app.engine.total_count(), 20);
        // "msg 1" matches: msg 1, msg 10, msg 11, msg 12, msg 13, msg 14, msg 15, msg 16, msg 17, msg 18, msg 19 = 11
        assert!(app.engine.filtered_count() > 0);

        // Scrolling should work within the filtered set
        app.scroll_down(5);
        let filtered = app.engine.filtered_count();
        assert!(app.selected_index < filtered);
    }

    #[test]
    fn test_get_visible_messages_with_filters() {
        let mut app = TuiApp::new();

        for i in 0..20 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Apply a title filter that matches "msg 5" (only msg 5)
        let widget = &mut app.lhs_widgets[0]; // TitleFilter widget at index 0
        widget.enabled = true;
        widget.input_text = "msg 5".to_string();

        app.apply_filters();

        assert_eq!(app.engine.total_count(), 20);
        assert_eq!(app.engine.filtered_count(), 1);

        let indices = app.engine.get_filtered_indices().to_vec();
        println!("indices: {:?}", indices);
        println!("selected_index: {}", app.selected_index);
        println!(
            "first_visible_message: {}",
            app.scroll_manager.first_visible_message
        );
        println!("num_messages: {}", app.scroll_manager.num_messages);

        let visible = app.get_visible_messages(10);
        assert_eq!(
            visible.len(),
            1,
            "expected 1 visible message but got {}, indices={:?}",
            visible.len(),
            indices
        );
    }

    #[test]
    fn test_selected_message_with_filters() {
        let mut app = TuiApp::new();

        for i in 0..20 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Apply a title filter that matches "msg 1" (messages msg 1, msg 10-19)
        let widget = &mut app.lhs_widgets[0]; // TitleFilter widget at index 0
        widget.enabled = true;
        widget.input_text = "msg 1".to_string();

        app.apply_filters();

        assert_eq!(app.engine.total_count(), 20);
        assert!(app.engine.filtered_count() > 0);

        let selected = app.get_selected_message();
        assert!(selected.is_some());
    }

    #[test]
    fn test_scroll_at_bottom_with_filters() {
        let mut app = TuiApp::new();
        app.is_live_stream = true;

        for i in 0..20 {
            app.add_message(make_test_message(0x600 + i, &format!("msg {}", i)));
        }

        // Apply a title filter that matches "msg 5" (only msg 5)
        let widget = &mut app.lhs_widgets[0]; // TitleFilter widget at index 0
        widget.enabled = true;
        widget.input_text = "msg 5".to_string();

        app.apply_filters();

        assert_eq!(app.engine.filtered_count(), 1);

        // Scroll past the end - should not panic or overflow
        for _ in 0..100 {
            app.scroll_down(20);
        }

        assert_eq!(app.selected_index, 0); // Only 1 filtered message
    }
}
