//! Root renderer for GPUI frontend.
//!
//! Implements the main `MainView` with the three-panel layout skeleton.

use gpui::{IntoElement, Render, Window};

/// Root view — three-panel layout skeleton.
pub struct MainView {
    // TODO: Add app_state entity reference (Phase 1 Step 5)
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // Phase 1 stub: empty root container
        // Full layout will be added in Phases 2-5:
        //
        // div().flex_col()
        //     .child(StatusBar::new(/* top */))
        //     .child(
        //         div().flex_row().size_full()
        //             .child(FilterDock::new())      // LHS — Phase 3
        //             .child(MessageList::new())       // Main — Phase 2
        //             .child(DetailPanel::new())       // RHS — Phase 4
        //     )
        //     .child(StatusBar::new(/* bottom */))

        gpui::div()
    }
}
