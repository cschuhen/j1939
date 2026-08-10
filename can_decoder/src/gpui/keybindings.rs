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
actions!(can_decoder_gpui, [ToggleLayout, ClearMessages, Quit]);
