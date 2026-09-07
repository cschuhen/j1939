pub mod app;
pub mod column_editor_widget;
pub mod columns;
pub mod filter_editor_widget;
pub mod renderer;

// Re-exports for external access
pub use self::app::ViewMode;
pub use self::columns::{Column, ColumnConfig};
// Re-export the shared scroll manager for backward compatibility
pub use crate::scroll_manager::MessageScrollManager;
