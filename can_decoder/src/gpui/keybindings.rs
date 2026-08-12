//! Keymap configuration for GPUI frontend.
//!
//! Defines actions and their keystroke bindings using GPUI's action/keymap system.
//! Mirrors src/tui/keybindings.rs but uses GPUI Actions instead of manual KeyEvent mapping.

// Import the actions macro from gpui prelude
use gpui::actions;

// Define all actions using the actions! macro (GPUI 0.2.x pattern)
actions!(
    can_decoder_gpui,
    [ToggleFilter, NextScreenMode, ScrollUp, ScrollDown]
);
actions!(
    can_decoder_gpui,
    [SelectRow, EditFilterInput, CancelFilterEdit]
);
actions!(
    can_decoder_gpui,
    [ToggleLayout, ClearMessages, Quit, ToggleColumns]
);

use gpui::{App, KeyBinding};

/// Configure default keybindings for the GPUI frontend.
pub fn configure_keybindings(cx: &mut App) {
    cx.bind_keys(vec![
        KeyBinding::new("up", ScrollUp, Some("App")),
        KeyBinding::new("down", ScrollDown, Some("App")),
        KeyBinding::new("page_up", ScrollUp, Some("App")),
        KeyBinding::new("page_down", ScrollDown, Some("App")),
        KeyBinding::new("enter", SelectRow, Some("App")),
        KeyBinding::new("space", SelectRow, Some("App")),
        KeyBinding::new("ctrl+c", Quit, Some("App")),
        KeyBinding::new("ctrl+l", ClearMessages, Some("App")),
        KeyBinding::new("ctrl+shift+c", ToggleColumns, Some("App")),
    ])
}
