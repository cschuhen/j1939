//! Keymap configuration for GPUI frontend.
//!
//! Defines actions and their keystroke bindings using GPUI's action/keymap system.
//! Mirrors src/tui/keybindings.rs but uses GPUI Actions instead of manual KeyEvent mapping.

// Import the actions macro from gpui prelude
use gpui::actions;

// Define all actions using the actions! macro (GPUI 0.2.x pattern)
actions!(
    can_decoder_gpui,
    [
        ToggleFilter,
        NextScreenMode,
        ScrollUp,
        ScrollDown,
        PageUp,
        PageDown
    ]
);
actions!(
    can_decoder_gpui,
    [SelectRow, EditFilterInput, CancelFilterEdit]
);
actions!(
    can_decoder_gpui,
    [
        ToggleLayout,
        ClearMessages,
        Quit,
        ToggleColumns,
        OpenColumns,
        CloseColumns
    ]
);

use gpui::{App, KeyBinding};

/// Configure default keybindings for the GPUI frontend.
pub fn configure_keybindings(cx: &mut App) {
    cx.bind_keys(vec![
        KeyBinding::new("up", ScrollUp, None),
        KeyBinding::new("down", ScrollDown, None),
        KeyBinding::new("pageup", PageUp, None),
        KeyBinding::new("pagedown", PageDown, None),
        KeyBinding::new("enter", SelectRow, None),
        KeyBinding::new("space", SelectRow, None),
        KeyBinding::new("escape", CancelFilterEdit, None),
        KeyBinding::new("ctrl+c", Quit, None),
        KeyBinding::new("ctrl+l", ClearMessages, None),
        KeyBinding::new("ctrl+shift+c", ToggleColumns, None),
    ])
}
