/// GUI-independent filter editor logic.
///
/// This module contains all filter editor state management and data transformation
/// with zero TUI dependencies. It is designed to be unit tested and reusable by
/// other GUIs (e.g., GPUI).

use std::rc::Rc;

// ─── Field Types ─────────────────────────────────────────────────────────────

/// Represents the type of filter field being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    SourceAddr,   // u8 addresses (0-255)
    DestAddr,     // u8 addresses (0-255)
    SrcName,      // u64 NAME values
    DstName,      // u64 NAME values
    Pgn,          // u32 PGN values
    Title,        // String titles (interned via Rc<str>)
}

impl FieldType {
    /// Human-readable label for display in modal title.
    pub fn label(&self) -> &'static str {
        match self {
            Self::SourceAddr => "Source Addr",
            Self::DestAddr => "Dest Addr",
            Self::SrcName => "Source NAME",
            Self::DstName => "Dest NAME",
            Self::Pgn => "PGN",
            Self::Title => "Title",
        }
    }

    /// Parse a single value string into the appropriate RawValue variant.
    pub fn parse_value(&self, s: &str) -> Result<RawValue, String> {
        match self {
            Self::SourceAddr | Self::DestAddr => {
                let v = parse_hex_or_dec_u8(s).map_err(|e| format!("invalid address: {}", e))?;
                Ok(RawValue::U8(v))
            }
            Self::SrcName | Self::DstName => {
                let v = parse_hex_or_dec_u64(s).map_err(|e| format!("invalid NAME: {}", e))?;
                Ok(RawValue::U64(v))
            }
            Self::Pgn => {
                let v = parse_hex_or_dec_u32(s).map_err(|e| format!("invalid PGN: {}", e))?;
                Ok(RawValue::U32(v))
            }
            Self::Title => {
                let rc: Rc<str> = s.into();
                Ok(RawValue::Title(rc))
            }
        }
    }

    /// Format a RawValue back into its display string representation.
    pub fn format_value(&self, value: &RawValue) -> String {
        match (self, value) {
            (Self::SourceAddr | Self::DestAddr, RawValue::U8(v)) => format!("{}", v),
            (Self::SrcName | Self::DstName, RawValue::U64(v)) => format!("{:016X}", v),
            (Self::Pgn, RawValue::U32(v)) => format!("0x{:05X}", v),
            (Self::Title, RawValue::Title(s)) => s.to_string(),
            _ => value.display_str(),
        }
    }

    /// Get the display string for a RawValue without field-type-specific formatting.
    pub fn raw_display(&self, value: &RawValue) -> String {
        match value {
            RawValue::U8(v) => format!("{}", v),
            RawValue::U64(v) => format!("0x{:016X}", v),
            RawValue::U32(v) => format!("0x{:05X}", v),
            RawValue::Title(s) => s.to_string(),
        }
    }

    /// Get the display string for a FilterOption's raw_value.
    pub fn option_display(&self, opt: &FilterOption) -> String {
        self.raw_display(&opt.raw_value)
    }
}

// ─── Raw Value ───────────────────────────────────────────────────────────────

/// The raw parsed value stored in a FilterOption.
#[derive(Debug, Clone, PartialEq)]
pub enum RawValue {
    U8(u8),
    U64(u64),
    U32(u32),
    Title(Rc<str>),
}

impl RawValue {
    /// Display string without field-type-specific formatting.
    pub fn display_str(&self) -> String {
        match self {
            Self::U8(v) => format!("{}", v),
            Self::U64(v) => format!("{:016X}", v),
            Self::U32(v) => format!("0x{:05X}", v),
            Self::Title(s) => s.to_string(),
        }
    }

    /// Get the u8 value if this is a U8 variant.
    pub fn as_u8(&self) -> Option<u8> {
        match self {
            Self::U8(v) => Some(*v),
            _ => None,
        }
    }

    /// Get the u64 value if this is a U64 variant.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(v) => Some(*v),
            _ => None,
        }
    }

    /// Get the u32 value if this is a U32 variant.
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Self::U32(v) => Some(*v),
            _ => None,
        }
    }

    /// Get the Rc<str> value if this is a Title variant.
    pub fn as_title(&self) -> Option<&Rc<str>> {
        match self {
            Self::Title(s) => Some(s),
            _ => None,
        }
    }
}

// ─── Filter Option ───────────────────────────────────────────────────────────

/// A single option in the selection list.
#[derive(Debug, Clone)]
pub struct FilterOption {
    /// Stable unique ID for comparison across sorts and state changes.
    pub id: u32,
    /// Human-readable label shown in the list (e.g., "EngineECU" for a NAME).
    pub display: String,
    /// The actual parsed value used for matching/filtering.
    pub raw_value: RawValue,
}

// ─── Validation ──────────────────────────────────────────────────────────────

/// Validation state of the text field.
#[derive(Debug, Clone, PartialEq)]
pub enum TextValidation {
    Valid,
    Invalid(String), // Error message (e.g., "invalid address: 999")
}

// ─── Editor State ────────────────────────────────────────────────────────────

/// Complete state of the modal editor — the single source of truth for all editor logic.
#[derive(Debug, Clone)]
pub struct FilterEditorState {
    /// Type of field being edited.
    pub field_type: FieldType,

    /// Current text in the edit field.
    pub text_input: String,

    /// Validation state of the text input.
    pub validation: TextValidation,

    /// IDs of currently selected options (checked checkboxes).
    pub selected_ids: Vec<u32>,

    /// All available options for this field type.
    pub options: Vec<FilterOption>,

    /// Scroll manager for the selection list.
    pub scroll_mgr: crate::scroll_manager::MessageScrollManager,

    /// Whether the text input field currently has focus (-1 = text focused, >=0 = list index).
    /// Kept separate from scroll_mgr because MessageScrollManager doesn't track text-field focus.
    pub focus_index: i32, // -1 means text field is focused, >=0 means list item index

    /// Cursor position within the text input (char index, not byte offset).
    pub cursor_pos: usize,

    /// Whether any option was selected via keyboard navigation (vs manual text edit).
    /// Used to decide whether to auto-apply on Enter.
    pub list_interaction_active: bool,
}

impl FilterEditorState {
    /// Create a new empty state for the given field type and options.
    pub fn new(field_type: FieldType, options: Vec<FilterOption>) -> Self {
        let mut scroll_mgr = crate::scroll_manager::MessageScrollManager::new();
        scroll_mgr.set_num_messages(options.len());

        Self {
            field_type,
            text_input: String::new(),
            validation: TextValidation::Valid,
            selected_ids: Vec::new(),
            options,
            scroll_mgr,
            focus_index: -1,
            cursor_pos: 0,
            list_interaction_active: false,
        }
    }

    /// Re-sync the scroll manager with current option count.
    pub fn sync_scroll_manager(&mut self) {
        self.scroll_mgr.set_num_messages(self.options.len());
    }

    /// Convert a char index to a byte offset in the text input string.
    /// Returns 0 if the text is empty or the index is out of bounds.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        if self.text_input.is_empty() || char_idx == 0 {
            return 0;
        }
        let chars: Vec<char> = self.text_input.chars().collect();
        if char_idx >= chars.len() {
            return self.text_input.len();
        }
        let mut byte_count = 0;
        for c in &chars[..char_idx] {
            byte_count += c.len_utf8();
        }
        byte_count
    }

    /// Check if the text field currently has focus.
    pub fn is_text_focused(&self) -> bool {
        self.focus_index == -1
    }

    /// Check if the selection list currently has focus.
    pub fn is_list_focused(&self) -> bool {
        self.focus_index >= 0
    }

    /// Get the number of visible items in the list (for scroll calculations).
    pub fn visible_count(&self, viewport_height: usize) -> usize {
        self.options.len().min(viewport_height)
    }

    /// Update the scroll manager's viewport height and ensure selection is visible.
    pub fn update_viewport(&mut self, viewport_height: usize) {
        self.scroll_mgr.set_num_rows(viewport_height);
        if self.is_list_focused() {
            self.scroll_mgr.ensure_selected_visible();
        }
    }

    /// Page down (scroll list down by one page).
    pub fn page_down(&mut self, viewport_height: usize) {
        if !self.is_list_focused() {
            // Jump to first item when focusing from text field
            if !self.options.is_empty() {
                self.focus_index = 0;
                self.scroll_mgr.selected_index = 0;
                self.scroll_mgr.ensure_selected_visible();
            }
            return;
        }
        self.update_viewport(viewport_height);
        self.scroll_mgr.page_down();
        self.focus_index = self.scroll_mgr.selected_index as i32;
    }

    /// Page up (scroll list up by one page).
    pub fn page_up(&mut self, viewport_height: usize) {
        if !self.is_list_focused() {
            // Jump to last item when focusing from text field
            if !self.options.is_empty() {
                let max = self.options.len().saturating_sub(1);
                self.focus_index = max as i32;
                self.scroll_mgr.selected_index = max;
                self.scroll_mgr.ensure_selected_visible();
            }
            return;
        }
        self.update_viewport(viewport_height);
        self.scroll_mgr.page_up();
        self.focus_index = self.scroll_mgr.selected_index as i32;
    }

    /// Move focus to the first item in the list.
    pub fn go_to_first(&mut self, viewport_height: usize) {
        if !self.options.is_empty() {
            self.focus_index = 0;
            self.scroll_mgr.selected_index = 0;
            self.update_viewport(viewport_height);
        }
    }

    /// Move focus to the last item in the list.
    pub fn go_to_last(&mut self, viewport_height: usize) {
        if !self.options.is_empty() {
            let max = self.options.len().saturating_sub(1);
            self.focus_index = max as i32;
            self.scroll_mgr.selected_index = max;
            self.update_viewport(viewport_height);
        }
    }

    /// Move focus up by one item.
    pub fn move_up(&mut self, viewport_height: usize) {
        if !self.is_list_focused() {
            // If text is focused, go to last item (reverse direction from bottom)
            if !self.options.is_empty() {
                let max = self.options.len().saturating_sub(1);
                self.focus_index = max as i32;
                self.scroll_mgr.selected_index = max;
                self.update_viewport(viewport_height);
            }
        } else {
            self.update_viewport(viewport_height);
            self.scroll_mgr.scroll_up();
            self.focus_index = self.scroll_mgr.selected_index as i32;
        }
    }

    /// Move focus down by one item.
    pub fn move_down(&mut self, viewport_height: usize) {
        if !self.is_list_focused() {
            // If text is focused, go to first item
            if !self.options.is_empty() {
                self.focus_index = 0;
                self.scroll_mgr.selected_index = 0;
                self.update_viewport(viewport_height);
            }
        } else {
            self.update_viewport(viewport_height);
            self.scroll_mgr.scroll_down();
            self.focus_index = self.scroll_mgr.selected_index as i32;
        }
    }

    /// Toggle the checkbox for the currently focused option.
    pub fn toggle_focused(&mut self) {
        if !self.is_list_focused() || self.options.is_empty() {
            return;
        }
        let idx = self.focus_index as usize;
        if idx >= self.options.len() {
            return;
        }
        let option_id = self.options[idx].id;
        if self.selected_ids.contains(&option_id) {
            self.selected_ids.retain(|&id| id != option_id);
        } else {
            self.selected_ids.push(option_id);
        }
    }

    /// Toggle a specific option by ID.
    pub fn toggle_option_by_id(&mut self, option_id: u32) {
        if self.selected_ids.contains(&option_id) {
            self.selected_ids.retain(|&id| id != option_id);
        } else {
            self.selected_ids.push(option_id);
        }
    }

    /// Check if a specific option ID is currently selected.
    pub fn is_selected(&self, option_id: u32) -> bool {
        self.selected_ids.contains(&option_id)
    }

    /// Get the display string for a given option ID.
    pub fn get_option_display(&self, option_id: u32) -> Option<String> {
        self.options.iter().find(|opt| opt.id == option_id).map(|opt| opt.display.clone())
    }

    /// Check if there are more items below the current scroll position.
    pub fn has_more_below(&self, viewport_height: usize) -> bool {
        (self.scroll_mgr.first_visible_message + viewport_height) < self.options.len()
    }

    /// Check if we're scrolled past the beginning (can scroll up).
    pub fn can_scroll_up(&self) -> bool {
        self.scroll_mgr.first_visible_message > 0
    }
}

// ─── Filter Editor ───────────────────────────────────────────────────────────

/// GUI-independent filter editor that manages state transitions.
///
/// All methods are pure functions or state mutations with no GUI side effects.
pub struct FilterEditor {
    field_type: FieldType,
}

impl FilterEditor {
    /// Create a new editor for the given field type.
    pub fn new(field_type: FieldType) -> Self {
        Self { field_type }
    }

    /// Initialize state from an existing filter expression string (e.g., "144,255").
    /// Pre-checks matching options in the selection list.
    pub fn open(&self, current_filter_text: &str, options: Vec<FilterOption>) -> FilterEditorState {
        let mut state = FilterEditorState::new(self.field_type, options);

        // Parse existing filter text and pre-check matching options
        if !current_filter_text.is_empty() {
            let (validation, selected_ids) = self.update_from_text_impl(current_filter_text, &state.options);
            state.validation = validation;
            state.selected_ids = selected_ids;
            state.text_input = current_filter_text.to_string();
            state.cursor_pos = current_filter_text.len();
        }

        state
    }

    /// Process text input and return updated validation + selected_ids.
    /// Called on every keystroke in the text field.
    pub fn update_from_text(&self, state: &FilterEditorState) -> (TextValidation, Vec<u32>) {
        self.update_from_text_impl(&state.text_input, &state.options)
    }

    /// Internal implementation of text → list sync.
    fn update_from_text_impl(&self, text_input: &str, options: &[FilterOption]) -> (TextValidation, Vec<u32>) {
        if text_input.is_empty() {
            return (TextValidation::Valid, Vec::new());
        }

        let mut selected_ids = Vec::new();
        let mut has_error = false;
        let mut error_msg = String::new();

        for segment in text_input.split(',') {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                continue;
            }

            match self.field_type.parse_value(trimmed) {
                Ok(raw_value) => {
                    // Find matching option by raw value comparison
                    if let Some(opt_id) = self.find_option_by_raw_value(options, &raw_value) {
                        selected_ids.push(opt_id);
                    } else {
                        // Value parsed OK but not in available options — still accept it
                        // (user may have typed a valid value that hasn't been seen yet)
                        has_error = true;
                        if error_msg.is_empty() {
                            error_msg = format!("unknown value: {}", trimmed);
                        }
                    }
                }
                Err(e) => {
                    has_error = true;
                    if error_msg.is_empty() {
                        error_msg = e;
                    }
                }
            }
        }

        // Deduplicate selected IDs while preserving order
        let mut seen = std::collections::HashSet::new();
        selected_ids.retain(|id| seen.insert(*id));

        if has_error {
            (TextValidation::Invalid(error_msg), selected_ids)
        } else {
            (TextValidation::Valid, selected_ids)
        }
    }

    /// Reconstruct CSV text from currently selected option IDs.
    /// Called when list focus changes or options are toggled.
    pub fn reconstruct_text(&self, state: &FilterEditorState) -> String {
        if state.selected_ids.is_empty() {
            return String::new();
        }

        let mut parts = Vec::with_capacity(state.selected_ids.len());
        for id in &state.selected_ids {
            if let Some(opt) = state.options.iter().find(|o| o.id == *id) {
                parts.push(self.field_type.raw_display(&opt.raw_value));
            }
        }

        parts.join(", ")
    }

    /// Toggle an option by ID. Returns updated selected_ids.
    pub fn toggle_option(&self, state: &FilterEditorState, option_id: u32) -> Vec<u32> {
        let mut new_selected = state.selected_ids.clone();
        if new_selected.contains(&option_id) {
            new_selected.retain(|&id| id != option_id);
        } else {
            new_selected.push(option_id);
        }
        new_selected
    }

    /// Validate a single value string against the field type's constraints.
    pub fn validate_value(&self, value_str: &str) -> Result<RawValue, String> {
        self.field_type.parse_value(value_str.trim())
    }

    /// Get the final CSV output string from state (for applying to filter).
    pub fn get_output_csv(&self, state: &FilterEditorState) -> Option<String> {
        if !self.can_accept(state) {
            return None;
        }

        let mut parts = Vec::with_capacity(state.selected_ids.len());
        for id in &state.selected_ids {
            if let Some(opt) = state.options.iter().find(|o| o.id == *id) {
                parts.push(self.field_type.raw_display(&opt.raw_value));
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }

    /// Check if current state is valid and ready to accept.
    pub fn can_accept(&self, state: &FilterEditorState) -> bool {
        matches!(state.validation, TextValidation::Valid) && !state.selected_ids.is_empty()
    }

    /// Find an option ID by comparing raw values against the given options list.
    fn find_option_by_raw_value(&self, options: &[FilterOption], raw_value: &RawValue) -> Option<u32> {
        for opt in options {
            if Self::raw_values_match(&opt.raw_value, raw_value) {
                return Some(opt.id);
            }
        }
        None
    }

    /// Check if two RawValues match (accounting for field type).
    fn raw_values_match(a: &RawValue, b: &RawValue) -> bool {
        match (a, b) {
            (RawValue::U8(x), RawValue::U8(y)) => x == y,
            (RawValue::U64(x), RawValue::U64(y)) => x == y,
            (RawValue::U32(x), RawValue::U32(y)) => x == y,
            (RawValue::Title(x), RawValue::Title(y)) => x == y,
            _ => false,
        }
    }

    /// Check if two RawValues match (accounting for field type).
    fn matches_raw_value(&self, a: &RawValue, b: &RawValue) -> bool {
        Self::raw_values_match(a, b)
    }
}

// ─── Numeric Parsing Helpers ─────────────────────────────────────────────────

/// Parse a decimal or hex string into u8.
fn parse_hex_or_dec_u8(s: &str) -> Result<u8, String> {
    s.trim().parse::<u8>().map_err(|_| format!("'{}'", s.trim()))
        .or_else(|_| {
            let stripped = s.trim()
                .strip_prefix("0x")
                .or_else(|| s.trim().strip_prefix("0X"))
                .ok_or_else(|| format!("'{}'", s.trim()))?;
            u8::from_str_radix(stripped, 16)
                .map_err(|_| format!("'{}'", s.trim()))
        })
}

/// Parse a decimal or hex string into u32.
fn parse_hex_or_dec_u32(s: &str) -> Result<u32, String> {
    s.trim().parse::<u32>().map_err(|_| format!("'{}'", s.trim()))
        .or_else(|_| {
            let stripped = s.trim()
                .strip_prefix("0x")
                .or_else(|| s.trim().strip_prefix("0X"))
                .ok_or_else(|| format!("'{}'", s.trim()))?;
            u32::from_str_radix(stripped, 16)
                .map_err(|_| format!("'{}'", s.trim()))
        })
}

/// Parse a decimal or hex string into u64.
fn parse_hex_or_dec_u64(s: &str) -> Result<u64, String> {
    s.trim().parse::<u64>().map_err(|_| format!("'{}'", s.trim()))
        .or_else(|_| {
            let stripped = s.trim()
                .strip_prefix("0x")
                .or_else(|| s.trim().strip_prefix("0X"))
                .ok_or_else(|| format!("'{}'", s.trim()))?;
            u64::from_str_radix(stripped, 16)
                .map_err(|_| format!("'{}'", s.trim()))
        })
}

// ─── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── FieldType ─────────────────────────────────────────────────────────

    #[test]
    fn test_field_type_label() {
        assert_eq!(FieldType::SourceAddr.label(), "Source Addr");
        assert_eq!(FieldType::DestAddr.label(), "Dest Addr");
        assert_eq!(FieldType::SrcName.label(), "Source NAME");
        assert_eq!(FieldType::DstName.label(), "Dest NAME");
        assert_eq!(FieldType::Pgn.label(), "PGN");
        assert_eq!(FieldType::Title.label(), "Title");
    }

    #[test]
    fn test_parse_value_u8_decimal() {
        assert_eq!(FieldType::SourceAddr.parse_value("144"), Ok(RawValue::U8(144)));
        assert_eq!(FieldType::DestAddr.parse_value("255"), Ok(RawValue::U8(255)));
    }

    #[test]
    fn test_parse_value_u8_hex() {
        assert_eq!(FieldType::SourceAddr.parse_value("0x90"), Ok(RawValue::U8(144)));
        assert_eq!(FieldType::DestAddr.parse_value("0xfe"), Ok(RawValue::U8(254)));
    }

    #[test]
    fn test_parse_value_u8_invalid() {
        assert!(FieldType::SourceAddr.parse_value("300").is_err());
        assert!(FieldType::SourceAddr.parse_value("abc").is_err());
    }

    #[test]
    fn test_parse_value_u64_hex() {
        let result = FieldType::SrcName.parse_value("0x80000000000F2EEC");
        assert!(result.is_ok());
        if let Ok(RawValue::U64(v)) = result {
            assert_eq!(v, 0x80000000000F2EEC);
        } else {
            panic!("Expected U64");
        }
    }

    #[test]
    fn test_parse_value_u32() {
        assert_eq!(FieldType::Pgn.parse_value("51968"), Ok(RawValue::U32(51968)));
        assert_eq!(FieldType::Pgn.parse_value("0xCAF00"), Ok(RawValue::U32(0xCAF00)));
    }

    #[test]
    fn test_parse_value_title() {
        let result = FieldType::Title.parse_value("Engine Speed");
        assert!(result.is_ok());
        if let Ok(RawValue::Title(s)) = result {
            assert_eq!(s.as_ref(), "Engine Speed");
        } else {
            panic!("Expected Title");
        }
    }

    #[test]
    fn test_format_value_u8() {
        assert_eq!(FieldType::SourceAddr.format_value(&RawValue::U8(144)), "144");
    }

    #[test]
    fn test_format_value_u64() {
        let formatted = FieldType::SrcName.format_value(&RawValue::U64(0x80000000000F2EEC));
        assert_eq!(formatted, "80000000000F2EEC");
    }

    #[test]
    fn test_format_value_u32() {
        let formatted = FieldType::Pgn.format_value(&RawValue::U32(0xCAF00));
        assert_eq!(formatted, "0xCAF00");
    }

    // ─── RawValue ──────────────────────────────────────────────────────────

    #[test]
    fn test_raw_value_display_str() {
        assert_eq!(RawValue::U8(144).display_str(), "144");
        assert_eq!(RawValue::U64(0xFF).display_str(), "00000000000000FF");
        assert_eq!(RawValue::U32(0xCAF00).display_str(), "0xCAF00");
    }

    #[test]
    fn test_raw_value_as_u8() {
        assert_eq!(RawValue::U8(144).as_u8(), Some(144));
        assert_eq!(RawValue::U64(144).as_u8(), None);
    }

    #[test]
    fn test_raw_value_as_u64() {
        assert_eq!(RawValue::U64(0xFF).as_u64(), Some(0xFF));
        assert_eq!(RawValue::U8(255).as_u64(), None);
    }

    #[test]
    fn test_raw_value_as_title() {
        let rc: Rc<str> = "Engine Speed".into();
        assert_eq!(RawValue::Title(rc.clone()).as_title(), Some(&rc));
        assert_eq!(RawValue::U8(144).as_title(), None);
    }

    // ─── FilterEditorState ─────────────────────────────────────────────────

    fn make_test_options() -> Vec<FilterOption> {
        vec![
            FilterOption { id: 0, display: "EngineECU".to_string(), raw_value: RawValue::U8(144) },
            FilterOption { id: 1, display: "DisplayUnit".to_string(), raw_value: RawValue::U8(254) },
            FilterOption { id: 2, display: "GPSModule".to_string(), raw_value: RawValue::U8(0) },
        ]
    }

    #[test]
    fn test_state_new() {
        let options = make_test_options();
        let state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert_eq!(state.field_type, FieldType::SourceAddr);
        assert_eq!(state.text_input, "");
        assert!(matches!(state.validation, TextValidation::Valid));
        assert!(state.selected_ids.is_empty());
        assert_eq!(state.options.len(), 3);
        assert_eq!(state.focus_index, -1);
    }

    #[test]
    fn test_state_is_text_focused() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert!(state.is_text_focused());
        state.focus_index = 0;
        assert!(!state.is_text_focused());
    }

    #[test]
    fn test_state_is_list_focused() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert!(!state.is_list_focused());
        state.focus_index = 0;
        assert!(state.is_list_focused());
    }

    #[test]
    fn test_state_page_down_from_text() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        // From text focus, page_down jumps to first item
        state.page_down(15);
        assert_eq!(state.focus_index, 0);
    }

    #[test]
    fn test_state_move_up_from_text() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert!(state.is_text_focused());
        state.move_up(15);
        // Goes to last item when moving up from text focus
        assert_eq!(state.focus_index, 2);
    }

    #[test]
    fn test_state_move_down_from_text() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert!(state.is_text_focused());
        state.move_down(15);
        // Goes to first item when moving down from text focus
        assert_eq!(state.focus_index, 0);
    }

    #[test]
    fn test_state_toggle_focused() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        state.focus_index = 0;
        assert!(!state.is_selected(0));
        state.toggle_focused();
        assert!(state.is_selected(0));

        state.toggle_focused();
        assert!(!state.is_selected(0));
    }

    #[test]
    fn test_state_toggle_option_by_id() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        state.toggle_option_by_id(1);
        assert!(state.is_selected(1));
        state.toggle_option_by_id(1);
        assert!(!state.is_selected(1));
    }

    #[test]
    fn test_state_get_option_display() {
        let options = make_test_options();
        let state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert_eq!(state.get_option_display(0), Some("EngineECU".to_string()));
        assert_eq!(state.get_option_display(99), None);
    }

    #[test]
    fn test_state_has_more_below() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        // 3 items, viewport 2 → has more below at position 0
        assert!(state.has_more_below(2));

        state.scroll_mgr.first_visible_message = 1;
        assert!(!state.has_more_below(2));
    }

    #[test]
    fn test_state_can_scroll_up() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        assert!(!state.can_scroll_up());
        state.scroll_mgr.first_visible_message = 1;
        assert!(state.can_scroll_up());
    }

    // ─── FilterEditor ──────────────────────────────────────────────────────

    #[test]
    fn test_editor_new() {
        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert_eq!(editor.field_type, FieldType::SourceAddr);
    }

    #[test]
    fn test_open_with_empty_filter() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);
        let state = editor.open("", options);
        assert!(matches!(state.validation, TextValidation::Valid));
        assert!(state.selected_ids.is_empty());
    }

    #[test]
    fn test_open_with_csv_filter() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);
        // "144,254" should check IDs 0 and 1
        let state = editor.open("144,254", options);
        assert!(matches!(state.validation, TextValidation::Valid));
        assert_eq!(state.selected_ids.len(), 2);
        assert!(state.selected_ids.contains(&0)); // 144
        assert!(state.selected_ids.contains(&1)); // 254
    }

    #[test]
    fn test_open_with_hex_filter() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);
        // "0x90,0xfe" should check IDs 0 and 1 (same as 144,254)
        let state = editor.open("0x90,0xfe", options);
        assert!(matches!(state.validation, TextValidation::Valid));
        assert_eq!(state.selected_ids.len(), 2);
    }

    #[test]
    fn test_update_from_text_valid() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        state.text_input = "144,254".to_string();
        let editor = FilterEditor::new(FieldType::SourceAddr);
        let (validation, selected_ids) = editor.update_from_text(&state);
        assert!(matches!(validation, TextValidation::Valid));
        assert_eq!(selected_ids.len(), 2);
    }

    #[test]
    fn test_update_from_text_invalid() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);
        // "300" is out of range for u8
        let (validation, selected_ids) = editor.update_from_text_impl("300", &options);
        assert!(matches!(validation, TextValidation::Invalid(_)));
    }

    #[test]
    fn test_update_from_text_mixed() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);
        // "144,abc" — 144 is valid, abc is invalid
        let (validation, selected_ids) = editor.update_from_text_impl("144,abc", &options);
        assert!(matches!(validation, TextValidation::Invalid(_)));
        assert_eq!(selected_ids.len(), 1); // only 144 matched
    }

    #[test]
    fn test_reconstruct_text_empty() {
        let options = make_test_options();
        let state = FilterEditorState::new(FieldType::SourceAddr, options);
        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert_eq!(editor.reconstruct_text(&state), "");
    }

    #[test]
    fn test_reconstruct_text_selected() {
        let options = make_test_options();
        // Manually set selected_ids
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options.clone());
        state.selected_ids = vec![0, 2];

        let editor = FilterEditor::new(FieldType::SourceAddr);
        let text = editor.reconstruct_text(&state);
        assert_eq!(text, "144, 0"); // IDs 0 and 2 → values 144 and 0
    }

    #[test]
    fn test_toggle_option_adds() {
        let options = make_test_options();
        let state = FilterEditorState::new(FieldType::SourceAddr, options);
        let editor = FilterEditor::new(FieldType::SourceAddr);
        let new_selected = editor.toggle_option(&state, 1);
        assert_eq!(new_selected.len(), 1);
        assert!(new_selected.contains(&1));
    }

    #[test]
    fn test_toggle_option_removes() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options.clone());
        state.selected_ids = vec![0, 1];

        let editor = FilterEditor::new(FieldType::SourceAddr);
        let new_selected = editor.toggle_option(&state, 1);
        assert_eq!(new_selected.len(), 1);
        assert!(!new_selected.contains(&1));
    }

    #[test]
    fn test_validate_value_u8_hex() {
        let editor = FilterEditor::new(FieldType::SourceAddr);
        let result = editor.validate_value("0x90");
        assert!(result.is_ok());
        if let Ok(RawValue::U8(v)) = result {
            assert_eq!(v, 144);
        } else {
            panic!("Expected U8");
        }
    }

    #[test]
    fn test_validate_value_u8_decimal() {
        let editor = FilterEditor::new(FieldType::SourceAddr);
        let result = editor.validate_value("255");
        assert!(result.is_ok());
        if let Ok(RawValue::U8(v)) = result {
            assert_eq!(v, 255);
        } else {
            panic!("Expected U8");
        }
    }

    #[test]
    fn test_validate_value_u8_invalid() {
        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert!(editor.validate_value("300").is_err());
        assert!(editor.validate_value("abc").is_err());
    }

    #[test]
    fn test_validate_value_name_hex() {
        let editor = FilterEditor::new(FieldType::SrcName);
        let result = editor.validate_value("0x80000000000F2EEC");
        assert!(result.is_ok());
        if let Ok(RawValue::U64(v)) = result {
            assert_eq!(v, 0x80000000000F2EEC);
        } else {
            panic!("Expected U64");
        }
    }

    #[test]
    fn test_validate_value_title() {
        let editor = FilterEditor::new(FieldType::Title);
        let result = editor.validate_value("Engine Speed");
        assert!(result.is_ok());
        if let Ok(RawValue::Title(s)) = result {
            assert_eq!(s.as_ref(), "Engine Speed");
        } else {
            panic!("Expected Title");
        }
    }

    #[test]
    fn test_can_accept_valid() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options.clone());
        state.selected_ids = vec![0];
        state.validation = TextValidation::Valid;

        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert!(editor.can_accept(&state));
    }

    #[test]
    fn test_can_accept_invalid() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options.clone());
        state.selected_ids = vec![0];
        state.validation = TextValidation::Invalid("bad value".to_string());

        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert!(!editor.can_accept(&state));
    }

    #[test]
    fn test_can_accept_empty() {
        let options = make_test_options();
        let state = FilterEditorState::new(FieldType::SourceAddr, options);

        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert!(!editor.can_accept(&state));
    }

    #[test]
    fn test_get_output_csv() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options.clone());
        state.selected_ids = vec![0, 1];
        state.validation = TextValidation::Valid;

        let editor = FilterEditor::new(FieldType::SourceAddr);
        let csv = editor.get_output_csv(&state);
        assert_eq!(csv, Some("144, 254".to_string()));
    }

    #[test]
    fn test_get_output_csv_invalid() {
        let options = make_test_options();
        let mut state = FilterEditorState::new(FieldType::SourceAddr, options.clone());
        state.selected_ids = vec![0];
        state.validation = TextValidation::Invalid("bad".to_string());

        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert!(editor.get_output_csv(&state).is_none());
    }

    #[test]
    fn test_get_output_csv_empty() {
        let options = make_test_options();
        let state = FilterEditorState::new(FieldType::SourceAddr, options);

        let editor = FilterEditor::new(FieldType::SourceAddr);
        assert!(editor.get_output_csv(&state).is_none());
    }

    // ─── Numeric Parsing Helpers ───────────────────────────────────────────

    #[test]
    fn test_parse_hex_or_dec_u8_decimal() {
        assert_eq!(parse_hex_or_dec_u8("144").unwrap(), 144);
        assert_eq!(parse_hex_or_dec_u8("255").unwrap(), 255);
    }

    #[test]
    fn test_parse_hex_or_dec_u8_hex() {
        assert_eq!(parse_hex_or_dec_u8("0x90").unwrap(), 144);
        assert_eq!(parse_hex_or_dec_u8("0xfe").unwrap(), 254);
        assert_eq!(parse_hex_or_dec_u8("0XFF").unwrap(), 255);
    }

    #[test]
    fn test_parse_hex_or_dec_u8_invalid() {
        assert!(parse_hex_or_dec_u8("300").is_err());
        assert!(parse_hex_or_dec_u8("abc").is_err());
    }

    #[test]
    fn test_parse_hex_or_dec_u32_decimal() {
        assert_eq!(parse_hex_or_dec_u32("51968").unwrap(), 51968);
    }

    #[test]
    fn test_parse_hex_or_dec_u32_hex() {
        assert_eq!(parse_hex_or_dec_u32("0xCAF00").unwrap(), 0xCAF00);
    }

    #[test]
    fn test_parse_hex_or_dec_u64_decimal() {
        assert_eq!(parse_hex_or_dec_u64("144").unwrap(), 144);
    }

    #[test]
    fn test_parse_hex_or_dec_u64_hex() {
        assert_eq!(parse_hex_or_dec_u64("0x80000000000F2EEC").unwrap(), 0x80000000000F2EEC);
    }

    // ─── Integration: open → update_from_text → reconstruct_text cycle ─────

    #[test]
    fn test_full_cycle_open_update_reconstruct() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);

        // Open with existing filter "144,254"
        let state = editor.open("144,254", options.clone());
        assert!(matches!(state.validation, TextValidation::Valid));
        assert_eq!(state.selected_ids.len(), 2);

        // Update from text (should be same result)
        let (validation, selected_ids) = editor.update_from_text(&state);
        assert!(matches!(validation, TextValidation::Valid));
        assert_eq!(selected_ids.len(), 2);

        // Reconstruct text from selection
        let text = editor.reconstruct_text(&state);
        assert!(!text.is_empty());
    }

    #[test]
    fn test_full_cycle_with_hex_input() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);

        // Open with hex values "0x90,0xfe" (same as 144,254)
        let state = editor.open("0x90,0xfe", options.clone());
        assert!(matches!(state.validation, TextValidation::Valid));
        assert_eq!(state.selected_ids.len(), 2);

        // Reconstruct should produce decimal representation
        let text = editor.reconstruct_text(&state);
        assert!(!text.is_empty());
    }

    #[test]
    fn test_full_cycle_invalid_then_valid() {
        let options = make_test_options();
        let editor = FilterEditor::new(FieldType::SourceAddr);

        // Start with invalid input
        let (validation, _) = editor.update_from_text_impl("300", &options);
        assert!(matches!(validation, TextValidation::Invalid(_)));

        // Then valid input
        let (validation, selected_ids) = editor.update_from_text_impl("144", &options);
        assert!(matches!(validation, TextValidation::Valid));
        assert_eq!(selected_ids.len(), 1);
    }

    #[test]
    fn test_state_navigation_with_viewport() {
        // Create a larger options list for navigation testing
        let mut options = Vec::new();
        for i in 0..20u8 {
            options.push(FilterOption {
                id: i as u32,
                display: format!("Addr{}", i),
                raw_value: RawValue::U8(i),
            });
        }

        let mut state = FilterEditorState::new(FieldType::SourceAddr, options);
        // Viewport of 5 items
        assert_eq!(state.visible_count(5), 5);

        // From text focus, move_down jumps to first item
        state.move_down(5);
        assert_eq!(state.focus_index, 0);

        // Move down a few times via scroll_mgr
        for _ in 0..3 {
            state.scroll_mgr.scroll_down();
            state.focus_index = state.scroll_mgr.selected_index as i32;
        }
        assert_eq!(state.focus_index, 3);

        // Toggle focused item
        state.toggle_focused();
        assert!(state.is_selected(3));
    }
}
