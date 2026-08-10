//! GPUI-native GUI frontend for can_decoder.
//!
//! This module provides a GPU-accelerated native GUI using the GPUI framework,
//! mirroring the functionality of the TUI frontend (src/tui/) while leveraging
//! GPUI's Entity system, dockable panels, and hardware-accelerated rendering.

// Re-export key GPUI types for convenience
pub use gpui::{App, Application, Context, Entity, IntoElement, Render, Window};

pub mod app_state;
pub mod components;
pub mod keybindings;
pub mod renderer;
