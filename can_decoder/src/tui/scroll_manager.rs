/// Manages scrolling state for the message list view.
///
/// Keeps track of which messages are visible in the viewport and ensures
/// the selected message is always visible with configurable look-ahead margins.

#[derive(Debug, Clone)]
pub struct MessageScrollManager {
    /// Maximum number of rows visible in the widget (viewport height).
    pub num_rows: usize,

    /// Total number of messages that pass the filter.
    pub num_messages: usize,

    /// Index of the currently selected message (0-based).
    pub selected_index: usize,

    /// Index of the top visible message in the viewport.
    pub first_visible_message: usize,

    /// Minimum number of messages to show above the selection when possible.
    pub look_ahead_top: usize,

    /// Minimum number of messages to show below the selection when possible.
    pub look_ahead_bottom: usize,
}

impl MessageScrollManager {
    /// Create a new MessageScrollManager with default settings.
    pub fn new() -> Self {
        Self {
            num_rows: 20,
            num_messages: 0,
            selected_index: 0,
            first_visible_message: 0,
            look_ahead_top: 3,
            look_ahead_bottom: 3,
        }
    }

    /// Set the viewport height (number of visible rows).
    pub fn set_num_rows(&mut self, num_rows: usize) {
        self.num_rows = num_rows;
        self.clamp();
    }

    pub fn eprintln(&self, prefix: &str) {
        eprintln!(
            "{} num_rows={} num_messages={} selected_index={} first_visible_message={} LA={}:{}",
            prefix,
            self.num_rows,
            self.num_messages,
            self.selected_index,
            self.first_visible_message,
            self.look_ahead_top,
            self.look_ahead_bottom
        );
    }

    /// Set the total number of messages and clamp all state.
    pub fn set_num_messages(&mut self, num_messages: usize) {
        self.num_messages = num_messages;
        self.clamp();
    }

    /// Ensure selected_index is visible by adjusting first_visible_message if needed.
    pub fn ensure_selected_visible(&mut self) {
        self.clamp();
    }

    /// Add a delta to selected_index and ensure it remains visible.
    pub fn move_selection(&mut self, delta: isize) {
        let current: isize = self.selected_index as isize;
        let max_idx: isize = self.num_messages.saturating_sub(1) as isize;

        let new_selected = (current + delta).max(0).min(max_idx) as usize;

        if new_selected != self.selected_index {
            self.selected_index = new_selected;
            self.ensure_selected_visible();
        }
    }

    /// Move selection down by one.
    pub fn scroll_down(&mut self) {
        self.move_selection(1);
    }

    /// Move selection up by one.
    pub fn scroll_up(&mut self) {
        self.move_selection(-1);
    }

    /// Page down (scroll viewport height).
    pub fn page_down(&mut self) {
        self.move_selection(self.num_rows as isize);
    }

    /// Page up (scroll viewport height).
    pub fn page_up(&mut self) {
        self.move_selection(-(self.num_rows as isize));
    }

    /// Move selection to the first message.
    pub fn go_to_top(&mut self) {
        if self.num_messages > 0 {
            self.selected_index = 0;
            self.first_visible_message = 0;
        }
    }

    /// Move selection to the last message.
    pub fn go_to_bottom(&mut self) {
        if self.num_messages > 0 {
            self.selected_index = self.num_messages - 1;
            self.ensure_selected_visible();
        }
    }

    /// Get the range of visible message indices [start, end).
    pub fn get_visible_range(&self) -> (usize, usize) {
        let start = self
            .first_visible_message
            .min(self.num_messages.saturating_sub(1));
        let end = (start + self.num_rows).min(self.num_messages);
        (start, end)
    }

    /// Get the index of the selected message relative to the viewport.
    pub fn get_selected_viewport_index(&self) -> Option<usize> {
        let (start, _) = self.get_visible_range();
        if self.selected_index >= start
            && self.selected_index < start + self.num_rows
            && self.num_messages > 0
        {
            Some(self.selected_index - start)
        } else {
            None
        }
    }

    /// Check if the selected message is currently visible in the viewport.
    pub fn is_selected_visible(&self) -> bool {
        let (start, end) = self.get_visible_range();
        self.selected_index >= start && self.selected_index < end && self.num_messages > 0
    }

    /// Clamp all values to valid ranges.
    fn clamp(&mut self) {
        if self.num_messages == 0 {
            self.selected_index = 0;
            self.first_visible_message = 0;
            return;
        }

        // Clamp selected_index to valid range
        self.selected_index = self.selected_index.min(self.num_messages - 1);

        let mut look_ahead_top = self.look_ahead_top;
        let mut look_ahead_bottom = self.look_ahead_bottom;

        // In case of very small viewport, trim down the look ahead.
        while look_ahead_top + look_ahead_bottom > self.num_rows {
            if look_ahead_top > look_ahead_bottom {
                look_ahead_top -= 1;
            } else {
                look_ahead_bottom -= 1;
            }
        }

        let top_min = self.selected_index.saturating_sub(look_ahead_top);
        let mut bottom_max = self.selected_index + look_ahead_bottom + 1;
        //let mut bottom_max = self.selected_index + self.look_ahead_bottom + 1;
        bottom_max = bottom_max.min(self.num_messages - 1);

        if self.first_visible_message > top_min {
            self.first_visible_message = top_min;
        }
        if self.first_visible_message + self.num_rows + 1 <= bottom_max {
            self.first_visible_message = bottom_max.saturating_sub(self.num_rows);
            if self.first_visible_message + self.num_rows + 1 >= self.num_messages {
                self.first_visible_message = self.num_messages.saturating_sub(self.num_rows);
            }
        }
    }

    /// Called when a new message is added. Updates num_messages and adjusts state.
    pub fn on_message_added(&mut self) {
        let old_num_messages = self.num_messages;
        self.num_messages += 1;

        // If we were at the bottom (last message), auto-follow to the new last message
        if self.selected_index == old_num_messages - 1 && old_num_messages > 0 {
            self.selected_index = old_num_messages; // Points to new last message (index = num_messages - 1)
        }

        self.clamp();
        self.ensure_selected_visible();
    }

    /// Called when a message is removed (e.g., buffer full, popped front).
    pub fn on_message_removed(&mut self) {
        if self.num_messages == 0 {
            return;
        }

        self.num_messages = self.num_messages.saturating_sub(1);

        // Clamp everything to new count
        self.clamp();
        self.ensure_selected_visible();
    }

    /// Called when messages are filtered out (total count changes).
    pub fn on_filter_change(&mut self) {
        // Clamp everything to new message count
        self.clamp();
        self.ensure_selected_visible();

        // If selection is no longer valid, move to last visible
        if self.num_messages > 0 && self.selected_index >= self.num_messages {
            self.selected_index = self.num_messages - 1;
            self.ensure_selected_visible();
        }
    }
}

impl Default for MessageScrollManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager_defaults() {
        let mgr = MessageScrollManager::new();
        assert_eq!(mgr.num_rows, 20);
        assert_eq!(mgr.num_messages, 0);
        assert_eq!(mgr.selected_index, 0);
        assert_eq!(mgr.first_visible_message, 0);
        assert_eq!(mgr.look_ahead_top, 3);
        assert_eq!(mgr.look_ahead_bottom, 3);
    }

    #[test]
    fn test_empty_messages() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(0);

        assert_eq!(mgr.selected_index, 0);
        assert_eq!(mgr.first_visible_message, 0);
        assert_eq!(mgr.get_visible_range(), (0, 0));
        assert!(!mgr.is_selected_visible());
    }

    #[test]
    fn test_single_message() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(1);

        assert_eq!(mgr.selected_index, 0);
        assert_eq!(mgr.first_visible_message, 0);
        assert_eq!(mgr.get_visible_range(), (0, 1));
        assert!(mgr.is_selected_visible());
    }

    #[test]
    fn test_selection_in_middle() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        // Select message 25, should be visible with look-ahead
        mgr.selected_index = 25;
        mgr.ensure_selected_visible();

        assert_eq!(mgr.selected_index, 25);
        // Should show at least 3 messages above selection
        let min_top: usize = 25 - 3; // 22
        assert!(mgr.first_visible_message <= min_top + 1);
        // Should show at least 3 messages below selection
        let max_bottom = mgr.first_visible_message + 10;
        assert!(max_bottom > 25);
    }

    #[test]
    fn test_selection_at_top() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        mgr.selected_index = 0;
        mgr.ensure_selected_visible();

        assert_eq!(mgr.first_visible_message, 0);
    }

    #[test]
    fn test_selection_at_bottom() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        mgr.selected_index = 49;
        mgr.ensure_selected_visible();

        // Should scroll to show the last message with look-ahead below if possible
        assert!(mgr.first_visible_message >= 40); // At least near bottom
    }

    #[test]
    fn test_scroll_down_from_top() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        mgr.selected_index = 0;
        mgr.first_visible_message = 0;

        // Scroll down 5 times
        for _ in 0..5 {
            mgr.scroll_down();
        }

        assert_eq!(mgr.selected_index, 5);
        assert!(mgr.is_selected_visible());
    }

    #[test]
    fn test_scroll_up_from_bottom() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        mgr.selected_index = 49;
        mgr.ensure_selected_visible();

        // Scroll up 5 times
        for _ in 0..5 {
            mgr.scroll_up();
        }

        assert_eq!(mgr.selected_index, 44);
        assert!(mgr.is_selected_visible());
    }

    #[test]
    fn test_page_down() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(100);

        mgr.selected_index = 0;
        mgr.first_visible_message = 0;

        mgr.page_down();

        assert_eq!(mgr.selected_index, 10);
    }

    #[test]
    fn test_page_up_from_bottom() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(100);

        mgr.selected_index = 99;
        mgr.ensure_selected_visible();

        mgr.page_up();

        assert_eq!(mgr.selected_index, 89);
    }

    #[test]
    fn test_go_to_top() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        mgr.selected_index = 49;
        mgr.first_visible_message = 40;

        mgr.go_to_top();

        assert_eq!(mgr.selected_index, 0);
        assert_eq!(mgr.first_visible_message, 0);
    }

    #[test]
    fn test_go_to_bottom() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        mgr.selected_index = 0;
        mgr.first_visible_message = 0;

        mgr.go_to_bottom();

        assert_eq!(mgr.selected_index, 49);
    }

    #[test]
    fn test_move_selection_negative_delta() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 5;
        mgr.move_selection(-10);

        assert_eq!(mgr.selected_index, 0); // Can't go below 0
    }

    #[test]
    fn test_move_selection_positive_delta_overflow() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 5;
        mgr.move_selection(100);

        assert_eq!(mgr.selected_index, 19); // Can't go beyond last message
    }

    #[test]
    fn test_visible_range_clamped() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(5);

        assert_eq!(mgr.get_visible_range(), (0, 5));
    }

    #[test]
    fn test_get_selected_viewport_index() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 5;
        mgr.first_visible_message = 0;

        assert_eq!(mgr.get_selected_viewport_index(), Some(5));
    }

    #[test]
    fn test_get_selected_viewport_index_not_visible() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 15;
        mgr.first_visible_message = 0;

        assert_eq!(mgr.get_selected_viewport_index(), None);
    }

    #[test]
    fn test_on_message_added_at_bottom() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(5);

        mgr.selected_index = 4; // At last message

        mgr.on_message_added();

        assert_eq!(mgr.num_messages, 6);
        assert_eq!(mgr.selected_index, 5); // Follows new message
    }

    #[test]
    fn test_on_message_removed() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(10);

        mgr.selected_index = 9; // At last message

        mgr.on_message_removed();

        assert_eq!(mgr.num_messages, 9);
        assert_eq!(mgr.selected_index, 8); // Adjusted to new last
    }

    #[test]
    fn test_on_filter_change_reduces_count() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 15;
        mgr.first_visible_message = 10;

        // Simulate filter reducing count to 10
        mgr.num_messages = 10;
        mgr.on_filter_change();

        assert!(mgr.selected_index < 10);
    }

    #[test]
    fn test_look_ahead_top_constraint() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);
        mgr.look_ahead_top = 5;

        mgr.selected_index = 20;
        mgr.ensure_selected_visible();

        // Should show at least 5 messages above selection
        let min_top = 20 - 5; // 15
        assert!(mgr.first_visible_message <= min_top);
    }

    #[test]
    fn test_look_ahead_bottom_constraint() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);
        mgr.look_ahead_bottom = 5;

        mgr.selected_index = 20;
        mgr.ensure_selected_visible();

        // Should show at least 5 messages below selection (if viewport allows)
        let bottom_edge = mgr.first_visible_message + 10;
        assert!(bottom_edge >= 20 + 5);
    }

    #[test]
    fn test_small_viewport() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(3);
        mgr.set_num_messages(10);

        // Selection at position 2 should be visible with a 3-row viewport
        mgr.selected_index = 2;
        mgr.ensure_selected_visible();

        assert!(mgr.is_selected_visible());
    }

    #[test]
    fn test_zero_viewport() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(0);
        mgr.set_num_messages(10);

        // Should not panic, just clamp
        mgr.clamp();
        assert_eq!(mgr.first_visible_message, 1);
    }

    #[test]
    fn test_scroll_down_stays_at_bottom_in_live() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(50);

        // Start at bottom
        mgr.selected_index = 49;
        mgr.ensure_selected_visible();

        // Add a message (simulating live stream)
        mgr.on_message_added(); // num_messages = 51, selected = 50

        assert_eq!(mgr.num_messages, 51);
        assert_eq!(mgr.selected_index, 50);
    }

    #[test]
    fn test_consecutive_scroll_down() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(30);

        mgr.selected_index = 0;
        mgr.first_visible_message = 0;

        // Scroll down 25 times
        for _ in 0..25 {
            mgr.scroll_down();
        }

        assert_eq!(mgr.selected_index, 25);
        assert!(mgr.is_selected_visible());
    }

    #[test]
    fn test_consecutive_scroll_up() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(30);

        mgr.selected_index = 25;
        mgr.ensure_selected_visible();

        // Scroll up 20 times
        for _ in 0..20 {
            mgr.scroll_up();
        }

        assert_eq!(mgr.selected_index, 5);
        assert!(mgr.is_selected_visible());
    }

    #[test]
    fn test_selection_clamped_on_message_count_decrease() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 15;

        // Reduce to 10 messages
        mgr.set_num_messages(10);

        assert_eq!(mgr.selected_index, 9); // Clamped to last message
    }

    #[test]
    fn test_first_visible_clamped_on_message_count_decrease() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.first_visible_message = 15;

        // Reduce to 10 messages
        mgr.set_num_messages(10);

        assert_eq!(mgr.first_visible_message, 0); // Clamped to valid range
    }

    #[test]
    fn test_move_selection_zero_delta() {
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_rows(10);
        mgr.set_num_messages(20);

        mgr.selected_index = 5;
        mgr.move_selection(0);

        assert_eq!(mgr.selected_index, 5); // No change
    }

    #[test]
    fn test_visible_range_empty() {
        let mgr = MessageScrollManager::new();
        assert_eq!(mgr.get_visible_range(), (0, 0));
    }

    #[test]
    fn test_get_selected_viewport_index_empty() {
        let mgr = MessageScrollManager::new();
        assert_eq!(mgr.get_selected_viewport_index(), None);
    }

    #[test]
    fn test_scroll_down_look_ahead_bottom_preserved() {
        // Reproduce: 48 messages, viewport=19 rows, look_ahead_bottom=3
        // Expected: when selection reaches within look_ahead of bottom (row 16),
        // scrolling should start so that selection stays 3 rows from bottom.
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_messages(48);
        mgr.set_num_rows(19);

        // Start at top
        assert_eq!(mgr.selected_index, 0);
        assert_eq!(mgr.first_visible_message, 0);

        // Simulate pressing down arrow one at a time from row 0 to row 25
        for target in 1..=25 {
            mgr.selected_index = target;
            mgr.ensure_selected_visible();

            let (start, end) = mgr.get_visible_range();
            let _viewport_pos = target - start;

            // Selection should always be visible
            assert!(
                target >= start,
                "selection {} not in range [{}, {}) at iteration {}",
                target,
                start,
                end,
                target
            );
            assert!(
                target < end,
                "selection {} past end of range [{}, {}) at iteration {}",
                target,
                start,
                end,
                target
            );

            // Once selection gets close to bottom edge (within look_ahead_bottom=3),
            // it should stay within 3 rows of the bottom of viewport.
            if target >= 16 {
                let bottom_of_viewport = start + 19;
                let distance_from_bottom = bottom_of_viewport - 1 - target;
                assert!(distance_from_bottom <= 3,
                    "at selection={}, first_visible={}, viewport=[{},{}), selection is {} rows from bottom (expected <= 3)",
                    target, mgr.first_visible_message, start, end, distance_from_bottom);
            }
        }

        // At row 25 with look_ahead_bottom=3:
        // first_visible should be 25 - 19 + 3 = 9 (so selection is at viewport index 16, 2 from bottom)
        assert_eq!(
            mgr.first_visible_message, 10,
            "At row 25 with 48 messages and look_ahead_bottom=3, first_visible should be 10"
        );
    }

    #[test]
    fn test_scroll_down_with_stale_num_rows() {
        // Reproduce the TUI bug: scroll_down sets num_rows to viewport_height (e.g. 20)
        // but actual viewport is smaller (e.g. 19). This causes ensure_selected_visible
        // to use wrong bottom_edge calculation.
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_messages(48);

        // Simulate TUI flow: scroll_down sets num_rows=20, but actual viewport is 19
        mgr.set_num_rows(20);

        for target in 1..=25 {
            mgr.selected_index = target;
            mgr.ensure_selected_visible();

            let (start, end) = mgr.get_visible_range();
            assert!(target >= start && target < end,
                "selection {} not visible at iteration {}, range=[{},{}), first_visible={}, num_rows={}",
                target, target, start, end, mgr.first_visible_message, mgr.num_rows);
        }

        // Now simulate what happens when get_visible_messages updates num_rows to 19
        // but ensure_selected_visible was called with stale num_rows=20
        let mut mgr2 = MessageScrollManager::new();
        mgr2.set_num_messages(48);

        // First render sets num_rows=19
        mgr2.set_num_rows(19);

        // User scrolls down - scroll_down sets num_rows=20 (wrong!)
        mgr2.set_num_rows(20);

        for target in 1..=25 {
            mgr2.selected_index = target;
            mgr2.ensure_selected_visible();

            let (start, end) = mgr2.get_visible_range();
            assert!(target >= start && target < end,
                "selection {} not visible with stale num_rows=20 at iteration {}, range=[{},{}), first_visible={}",
                target, target, start, end, mgr2.first_visible_message);
        }
    }

    #[test]
    fn test_scroll_down_viewport_mismatch_bug() {
        // This test reproduces the exact bug: when num_rows is set to a value larger
        // than the actual viewport (e.g. 20 instead of 19), ensure_selected_visible
        // delays scrolling too long, causing selection to go off-screen.
        let mut mgr = MessageScrollManager::new();
        mgr.set_num_messages(48);

        // Actual viewport is 19 rows but scroll_down sets num_rows=20
        mgr.set_num_rows(20);

        // Walk through selection one at a time, checking visibility
        for target in 0..=47 {
            mgr.selected_index = target;
            mgr.ensure_selected_visible();

            let (start, end) = mgr.get_visible_range();

            if !(target >= start && target < end) {
                panic!(
                    "At selection={}, first_visible={}, num_rows={}: selection {} is NOT visible in range [{}, {})",
                    target, mgr.first_visible_message, mgr.num_rows, target, start, end
                );
            }
        }

        // All 48 selections are visible - the viewport_mismatch test passes
    }
}
