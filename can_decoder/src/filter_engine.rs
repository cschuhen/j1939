use crate::traits::Filter;
use crate::types::DecodedMessage;
use std::collections::VecDeque;

/// Engine that maintains all messages and computes which ones pass active filters.
///
/// Filtering is a soft layer: every message is stored regardless of filter state.
/// `filtered_indices` is computed lazily via dirty-flag optimization.
pub struct FilterEngine {
    /// All messages ever received — never dropped.
    all_messages: VecDeque<DecodedMessage>,

    /// Currently active filters (AND logic).
    active_filters: Vec<Box<dyn Filter>>,

    /// Indices into `all_messages` that pass all active filters.
    filtered_indices: Vec<usize>,

    /// True when `filtered_indices` is stale and needs recomputation.
    indices_dirty: bool,
}

impl FilterEngine {
    /// Create a new empty filter engine with no filters (all messages pass).
    pub fn new() -> Self {
        Self {
            all_messages: VecDeque::new(),
            active_filters: Vec::new(),
            filtered_indices: Vec::new(),
            indices_dirty: true,
        }
    }

    /// Append a message to the engine. Marks indices dirty since a new message
    /// may or may not pass current filters.
    pub fn add_message(&mut self, msg: DecodedMessage) {
        let idx = self.all_messages.len();
        self.all_messages.push_back(msg);
        // Mark dirty so the new index gets evaluated on next recompute
        self.indices_dirty = true;

        // Optimization: if no filters are active, immediately include this index
        if self.active_filters.is_empty() {
            self.filtered_indices.push(idx);
            self.indices_dirty = false;
        }
    }

    /// Replace all active filters with a new set. Marks indices dirty.
    pub fn set_filters(&mut self, filters: Vec<Box<dyn Filter>>) {
        self.active_filters = filters;
        self.indices_dirty = true;
    }

    /// Add one filter to the existing set. Marks indices dirty.
    pub fn add_filter(&mut self, filter: Box<dyn Filter>) {
        self.active_filters.push(filter);
        self.indices_dirty = true;
    }

    /// Remove the filter at the given index. Marks indices dirty.
    pub fn remove_filter_at(&mut self, idx: usize) {
        if idx < self.active_filters.len() {
            self.active_filters.remove(idx);
            self.indices_dirty = true;
        }
    }

    /// Run all active filters against every message and populate `filtered_indices`.
    pub fn recompute(&mut self) {
        self.filtered_indices.clear();

        if self.all_messages.is_empty() {
            self.indices_dirty = false;
            return;
        }

        for (i, msg) in self.all_messages.iter().enumerate() {
            if self.message_passes_all_filters(msg) {
                self.filtered_indices.push(i);
            }
        }

        self.indices_dirty = false;
    }

    /// Return a slice of indices that pass all active filters.
    /// Automatically recomputes if dirty.
    pub fn get_filtered_indices(&mut self) -> &[usize] {
        if self.indices_dirty {
            self.recompute();
        }
        &self.filtered_indices
    }

    /// Get a message by its position in the filtered index list.
    /// Returns None if `filtered_idx` is out of bounds.
    pub fn get_message_at(&self, filtered_idx: usize) -> Option<&DecodedMessage> {
        let global_idx = *self.filtered_indices.get(filtered_idx)?;
        self.all_messages.get(global_idx)
    }

    /// Total number of messages ever received (never drops any).
    pub fn total_count(&self) -> usize {
        self.all_messages.len()
    }

    /// Number of messages that pass all active filters.
    pub fn filtered_count(&mut self) -> usize {
        if self.indices_dirty {
            self.recompute();
        }
        self.filtered_indices.len()
    }

    /// Check if a single message passes all active filters.
    fn message_passes_all_filters(&self, msg: &DecodedMessage) -> bool {
        for filter_ref in &self.active_filters {
            if !filter_matches_sync(&**filter_ref, msg) {
                return false;
            }
        }
        true
    }

    /// Clear all messages and reset state.
    pub fn clear(&mut self) {
        self.all_messages.clear();
        self.filtered_indices.clear();
        self.indices_dirty = true;
    }
}

impl Default for FilterEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Synchronously evaluate a filter against a message using an embedded Tokio runtime.
/// This mirrors the approach used in `tui/app.rs` for sync filter evaluation.
fn filter_matches_sync(filter: &dyn Filter, message: &DecodedMessage) -> bool {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    rt.block_on(filter.matches(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::{
        FlagFilter, NumericFilter, PgnFilter, RegexFilter, SeverityFilter, SourceFilter,
        TitleFilter,
    };
    use crate::types::{DecodedField, FlagValue, Numeric, Severity};

    // ─── Helpers ───────────────────────────────────────────────────────

    fn make_message(pgn: u32, source: u8, title: &str) -> DecodedMessage {
        let id = pgn << 8 | source as u32;
        let id = (7u32 << 26) | id; // priority 7
        let assembled = crate::types::AssembledMessage::new(id, vec![], 1000);
        DecodedMessage {
            title: title.to_string(),
            outputs: Vec::new(),
            updates: Vec::new(),
            assembled_message: assembled,
        }
    }

    fn make_message_with_outputs(
        pgn: u32,
        source: u8,
        title: &str,
        outputs: Vec<DecodedField>,
    ) -> DecodedMessage {
        let id = pgn << 8 | source as u32;
        let id = (7u32 << 26) | id;
        let assembled = crate::types::AssembledMessage::new(id, vec![], 1000);
        DecodedMessage {
            title: title.to_string(),
            outputs,
            updates: Vec::new(),
            assembled_message: assembled,
        }
    }

    // ─── Test: add_message_stores_all ──────────────────────────────────

    #[test]
    fn test_add_message_stores_all() {
        let mut engine = FilterEngine::new();

        for i in 0..20u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        assert_eq!(engine.total_count(), 20);
        // No filters → all indices should pass
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 20);
    }

    #[test]
    fn test_add_message_with_filters_drops_non_matching() {
        let mut engine = FilterEngine::new();

        // Add a filter for source address 5
        let filter: Box<dyn Filter> = Box::new(SourceFilter::new(5));
        engine.set_filters(vec![filter]);

        for i in 0..10u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        // Only source=5 should pass
        assert_eq!(engine.total_count(), 10);
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 5); // index 5 is the message with source=5
    }

    // ─── Test: no_filters_returns_all_indices ──────────────────────────

    #[test]
    fn test_no_filters_returns_all_indices() {
        let mut engine = FilterEngine::new();

        for i in 0..15u8 {
            engine.add_message(make_message(0xABC000, i, &format!("msg {}", i)));
        }

        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 15);

        // Verify all indices are present in order
        for (i, &idx) in indices.iter().enumerate() {
            assert_eq!(idx, i);
        }
    }

    #[test]
    fn test_empty_engine_no_filters() {
        let mut engine = FilterEngine::new();
        let indices = engine.get_filtered_indices();
        assert!(indices.is_empty());
        assert_eq!(engine.total_count(), 0);
        assert_eq!(engine.filtered_count(), 0);
    }

    // ─── Test: pgn_filter_selects_correct_messages ─────────────────────

    #[test]
    fn test_pgn_filter_selects_correct_messages() {
        let mut engine = FilterEngine::new();

        // Add messages with mixed PGNs
        for i in 0..10u8 {
            let pgn = if i % 3 == 0 { 0xEF40 } else { 0xEC00 };
            engine.add_message(make_message(pgn, 5, &format!("msg {}", i)));
        }

        // Filter by PGN 0xEF40
        let filter: Box<dyn Filter> = Box::new(PgnFilter::new(0xEF40));
        engine.set_filters(vec![filter]);

        engine.recompute();
        let indices = engine.get_filtered_indices();

        // Messages at indices 0, 3, 6, 9 have PGN 0x18EF4000
        assert_eq!(indices.len(), 4);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[1], 3);
        assert_eq!(indices[2], 6);
        assert_eq!(indices[3], 9);
    }

    // ─── Test: title_filter_case_insensitive ───────────────────────────

    #[test]
    fn test_title_filter_case_insensitive() {
        let mut engine = FilterEngine::new();

        for title in &["Vehicle Speed", "ENGINE SPEED", "coolant temp", "Brake Pressure"] {
            engine.add_message(make_message(0xCF00, 1, title));
        }

        // Filter by "speed" (should match "Vehicle Speed" and "ENGINE SPEED")
        let filter: Box<dyn Filter> = Box::new(TitleFilter::new("speed"));
        engine.set_filters(vec![filter]);

        engine.recompute();
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 2);
    }

    #[test]
    fn test_title_filter_no_match() {
        let mut engine = FilterEngine::new();

        for title in &["Vehicle Speed", "Engine RPM"] {
            engine.add_message(make_message(0xEF40, 1, title));
        }

        let filter: Box<dyn Filter> = Box::new(TitleFilter::new("brake"));
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 0);
    }

    // ─── Test: numeric_range_filter ────────────────────────────────────

    #[test]
    fn test_numeric_range_filter() {
        let mut engine = FilterEngine::new();

        for rpm in [500, 1500, 3000, 5500, 8000] {
            let outputs = vec![DecodedField::Value {
                title: "RPM".to_string(),
                value: Numeric::Int(rpm as i64),
                unit: Some("rpm".to_string()),
                decimal_places: None,
            }];
            engine.add_message(make_message_with_outputs(0xEF40, 1, "Engine Speed", outputs));
        }

        // Filter RPM in range 1000-6000 (should match 1500, 3000, and 5500)
        let filter: Box<dyn Filter> = Box::new(NumericFilter {
            title: "RPM".to_string(),
            min: Some(1000.0),
            max: Some(6000.0),
        });
        engine.set_filters(vec![filter]);

        engine.recompute();
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn test_numeric_exact_value_filter() {
        let mut engine = FilterEngine::new();

        for element in [5, 10, 15, 20] {
            let outputs = vec![DecodedField::Value {
                title: "Element".to_string(),
                value: Numeric::Int(element as i64),
                unit: None,
                decimal_places: None,
            }];
            engine.add_message(make_message_with_outputs(0xEF40, 1, "Test", outputs));
        }

        // Filter RPM == 10 (exact)
        let filter: Box<dyn Filter> = Box::new(NumericFilter {
            title: "Element".to_string(),
            min: Some(10.0),
            max: Some(10.0),
        });
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    #[test]
    fn test_numeric_no_outputs_does_not_panic() {
        let mut engine = FilterEngine::new();

        // Add messages with no outputs at all
        for i in 0..5u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        let filter: Box<dyn Filter> = Box::new(NumericFilter {
            title: "RPM".to_string(),
            min: Some(0.0),
            max: Some(5000.0),
        });
        engine.set_filters(vec![filter]);

        // Should not panic, should return 0 matches
        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 0);
    }

    // ─── Test: severity_filter ─────────────────────────────────────────

    #[test]
    fn test_severity_filter() {
        let mut engine = FilterEngine::new();

        for sev in &[Severity::Info, Severity::Warning, Severity::Error, Severity::Warning] {
            let outputs = vec![DecodedField::StringMessage {
                severity: sev.clone(),
                text: "test message".to_string(),
            }];
            engine.add_message(make_message_with_outputs(0xFF00, 1, "Diagnostic", outputs));
        }

        // Filter for errors only
        let filter: Box<dyn Filter> = Box::new(SeverityFilter {
            severity: Severity::Error,
        });
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    #[test]
    fn test_severity_filter_warning() {
        let mut engine = FilterEngine::new();

        for sev in &[Severity::Info, Severity::Warning, Severity::Error] {
            let outputs = vec![DecodedField::StringMessage {
                severity: sev.clone(),
                text: "test".to_string(),
            }];
            engine.add_message(make_message_with_outputs(0xFF00, 1, "Test", outputs));
        }

        let filter: Box<dyn Filter> = Box::new(SeverityFilter {
            severity: Severity::Warning,
        });
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    // ─── Test: source_filter ───────────────────────────────────────────

    #[test]
    fn test_source_filter() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10, 144, 255] {
            engine.add_message(make_message(0xEF40, src, &format!("src {}", src)));
        }

        let filter: Box<dyn Filter> = Box::new(SourceFilter::new(144));
        engine.set_filters(vec![filter]);

        engine.recompute();
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 3); // index 3 has source=144
    }

    #[test]
    fn test_source_filter_broadcast() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 255] {
            engine.add_message(make_message(0xEF40, src, &format!("src {}", src)));
        }

        let filter: Box<dyn Filter> = Box::new(SourceFilter::new(255));
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    // ─── Test: composite_and_filter ────────────────────────────────────

    #[test]
    fn test_composite_and_filter() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10, 144] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // Composite: source=5 AND pgn=0x18EF4000
        let filters = vec![
            Box::new(SourceFilter::new(5)) as Box<dyn Filter>,
            Box::new(PgnFilter::new(0xEF40)),
        ];
        engine.set_filters(filters);

        engine.recompute();
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 1); // index 1 has source=5 and PGN=0x18EF4000
    }

    #[test]
    fn test_composite_filter_no_match() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // Composite: source=99 AND pgn=0x18EF4000 — no message has source=99
        let filters = vec![
            Box::new(SourceFilter::new(99)) as Box<dyn Filter>,
            Box::new(PgnFilter::new(0xEF40)),
        ];
        engine.set_filters(filters);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 0);
    }

    // ─── Test: recompute_on_filter_change ──────────────────────────────

    #[test]
    fn test_recompute_on_filter_change() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10, 144] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // First filter: source=5
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);
        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);

        // Change filter: source=144
        engine.set_filters(vec![Box::new(SourceFilter::new(144))]);
        engine.recompute();
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 3); // index 3 has source=144
    }

    #[test]
    fn test_recompute_auto_on_get_filtered_indices() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // Set filter but don't call recompute manually
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);

        // get_filtered_indices should auto-recompute
        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 1);
    }

    // ─── Test: remove_filter_restores_indices ──────────────────────────

    #[test]
    fn test_remove_filter_restores_indices() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10, 144] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // Two filters: source=5 AND pgn=0x18EF4000 (both must match)
        let filters = vec![
            Box::new(SourceFilter::new(5)) as Box<dyn Filter>,
            Box::new(PgnFilter::new(0xEF40)),
        ];
        engine.set_filters(filters);
        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);

        // Remove the PGN filter — now only source=5 is required, but all messages have that PGN
        // So we should still get 1 match (source=5)
        engine.remove_filter_at(1);
        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);

        // Now add a message with different source and remove the source filter too
        engine.add_message(make_message(0xEF40, 99, "msg 99"));
        engine.remove_filter_at(0);
        engine.recompute();
        // All messages should pass now (no filters)
        assert_eq!(engine.get_filtered_indices().len(), 5);
    }

    // ─── Test: dirty_flag_avoids_unnecessary_recompute ─────────────────

    #[test]
    fn test_dirty_flag_behavior() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // No filters — indices should NOT be dirty (optimized path)
        assert!(!engine.indices_dirty);

        // Set a filter — marks dirty
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);
        assert!(engine.indices_dirty);

        // First call to get_filtered_indices recomputes and clears dirty flag
        let _ = engine.get_filtered_indices();
        assert!(!engine.indices_dirty);

        // Second call should NOT recompute (dirty is still false)
        let len1 = engine.get_filtered_indices().len();
        let len2 = engine.get_filtered_indices().len();
        assert_eq!(len1, len2);
    }

    #[test]
    fn test_add_message_marks_dirty_with_filters() {
        let mut engine = FilterEngine::new();

        for src in [1, 5] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // Set filter after adding messages
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);
        assert!(engine.indices_dirty);

        // Add another message — should remain dirty
        engine.add_message(make_message(0xEF40, 10, "msg 10"));
        assert!(engine.indices_dirty);
    }

    // ─── Test: get_message_at_bounds ───────────────────────────────────

    #[test]
    fn test_get_message_at_valid() {
        let mut engine = FilterEngine::new();

        for i in 0..5u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        // No filters — all messages pass
        let msg = engine.get_message_at(0).unwrap();
        assert_eq!(msg.title, "msg 0");

        let msg = engine.get_message_at(4).unwrap();
        assert_eq!(msg.title, "msg 4");
    }

    #[test]
    fn test_get_message_at_out_of_bounds() {
        let mut engine = FilterEngine::new();

        for i in 0..3u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        // No filters — only 3 messages
        assert!(engine.get_message_at(3).is_none());
        assert!(engine.get_message_at(100).is_none());
    }

    #[test]
    fn test_get_message_at_with_filter() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10, 144] {
            engine.add_message(make_message(0xEF40, src, &format!("src {}", src)));
        }

        // Filter: source=5 only
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);
        engine.recompute();

        // Only one filtered message at index 0 of filtered list
        let msg = engine.get_message_at(0).unwrap();
        assert_eq!(msg.title, "src 5");

        // Out of bounds in filtered list
        assert!(engine.get_message_at(1).is_none());
    }

    // ─── Test: empty_engine ────────────────────────────────────────────

    #[test]
    fn test_empty_engine_all_methods() {
        let mut engine = FilterEngine::new();

        assert_eq!(engine.total_count(), 0);
        assert_eq!(engine.filtered_count(), 0);
        assert!(engine.get_filtered_indices().is_empty());
        assert!(engine.get_message_at(0).is_none());

        // Set a filter on empty engine
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);
        engine.recompute();
        assert_eq!(engine.filtered_count(), 0);
    }

    // ─── Test: clear ───────────────────────────────────────────────────

    #[test]
    fn test_clear() {
        let mut engine = FilterEngine::new();

        for i in 0..10u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        assert_eq!(engine.total_count(), 10);
        engine.clear();
        assert_eq!(engine.total_count(), 0);
        assert_eq!(engine.filtered_count(), 0);
    }

    // ─── Test: regex_filter ────────────────────────────────────────────

    #[test]
    fn test_regex_filter() {
        let mut engine = FilterEngine::new();

        for text in &["Engine fault", "Coolant low", "Oil pressure warning"] {
            let outputs = vec![DecodedField::StringMessage {
                severity: Severity::Warning,
                text: text.to_string(),
            }];
            engine.add_message(make_message_with_outputs(0xFF00, 1, "Diagnostic", outputs));
        }

        // Filter for messages containing "engine" (case-sensitive regex)
        let filter: Box<dyn Filter> = Box::new(RegexFilter::new("(?i)engine").unwrap());
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    #[test]
    fn test_regex_filter_no_match() {
        let mut engine = FilterEngine::new();

        for text in &["Engine fault", "Coolant low"] {
            let outputs = vec![DecodedField::StringMessage {
                severity: Severity::Info,
                text: text.to_string(),
            }];
            engine.add_message(make_message_with_outputs(0xFF00, 1, "Test", outputs));
        }

        let filter: Box<dyn Filter> = Box::new(RegexFilter::new("transmission").unwrap());
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 0);
    }

    // ─── Test: flag_filter ─────────────────────────────────────────────

    #[test]
    fn test_flag_filter() {
        let mut engine = FilterEngine::new();

        for flag_val in &[FlagValue::Off, FlagValue::On, FlagValue::Error, FlagValue::On] {
            let outputs = vec![DecodedField::Flag {
                title: "Engine".to_string(),
                value: flag_val.clone(),
            }];
            engine.add_message(make_message_with_outputs(0xCF00, 1, "Status", outputs));
        }

        // Filter for Engine=On
        let filter: Box<dyn Filter> = Box::new(FlagFilter {
            title: "Engine".to_string(),
            value: FlagValue::On,
        });
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 2);
    }

    // ─── Test: dest_filter ─────────────────────────────────────────────

    #[test]
    fn test_dest_filter() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 255, 10] {
            let id = (7u32 << 26) | (0xEF40 << 8) | src as u32;
            let assembled = crate::types::AssembledMessage::new(id, vec![], 1000);
            engine.add_message(DecodedMessage {
                title: format!("src {}", src),
                outputs: Vec::new(),
                updates: Vec::new(),
                assembled_message: assembled,
            });
        }

        // Use SourceFilter since J1939 extended CAN IDs only encode source in bits 0-7
        let filter: Box<dyn Filter> = Box::new(SourceFilter::new(255));
        engine.set_filters(vec![filter]);

        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    // ─── Test: filtered_count_accuracy ─────────────────────────────────

    #[test]
    fn test_filtered_count_with_dirty_flag() {
        let mut engine = FilterEngine::new();

        for src in [1, 5, 10, 144] {
            engine.add_message(make_message(0xEF40, src, &format!("msg {}", src)));
        }

        // Set filter — marks dirty
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);

        // filtered_count should return accurate count even when dirty
        assert_eq!(engine.filtered_count(), 1);

        // get_filtered_indices should also be accurate
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    #[test]
    fn test_filtered_count_no_filters() {
        let mut engine = FilterEngine::new();

        for i in 0..25u8 {
            engine.add_message(make_message(0xEF40, i, &format!("msg {}", i)));
        }

        assert_eq!(engine.filtered_count(), 25);
    }

    // ─── Test: multiple messages same source different PGNs ────────────

    #[test]
    fn test_multiple_messages_same_source_different_pgns() {
        let mut engine = FilterEngine::new();

        for pgn in [0xEF40, 0xEC00, 0xCF00, 0xFF00] {
            engine.add_message(make_message(pgn, 5, &format!("pgn {:X}", pgn)));
        }

        // Filter by source=5 — all should pass (all have source=5)
        engine.set_filters(vec![Box::new(SourceFilter::new(5))]);
        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 4);

        // Change filter to PGN 0xEC00 — only one should pass
        engine.set_filters(vec![Box::new(PgnFilter::new(0xEC00))]);
        engine.recompute();
        assert_eq!(engine.get_filtered_indices().len(), 1);
    }

    // ─── Test: large message set performance (no panic) ────────────────

    #[test]
    fn test_large_message_set_no_panic() {
        let mut engine = FilterEngine::new();

        for i in 0..500u16 {
            let pgn = if i % 3 == 0 { 0xEF40 } else { 0xEC00 };
            let src = (i % 256) as u8;
            engine.add_message(make_message(pgn, src, &format!("msg {}", i)));
        }

        // Filter by PGN
        engine.set_filters(vec![Box::new(PgnFilter::new(0xEF40))]);
        engine.recompute();

        let indices = engine.get_filtered_indices();
        assert_eq!(indices.len(), 167); // ~500/3 messages have PGN 0xEF40
    }
}
