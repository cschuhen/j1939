//! Regression test for FilterPanel wheel scrolling.
//!
//! Root cause (fixed 2026-08): the panel root div was missing `.flex()`, so the
//! list's `flex_1()` had no effect and the overflow container was sized to its
//! full content height instead of the remaining panel space. The visible area
//! then equalled the content size, giving max_offset == 0 — wheel events were
//! accepted but nothing moved, while rows rendered past the clipped bottom edge.
//!
//! This test mirrors the production layout (panel embedded in a flex row like
//! MainView in `src/gpui/renderer.rs`, painted at window size) and asserts on
//! the LAYOUT — which is exactly what determines scrollability:
//!   1. the list's visible height is clamped below the window (i.e. to the
//!      remaining panel space), NOT sized to its multi-thousand-px content,
//!   2. the content still overflows that area, so scrolling has something to do.
//! Removing `.flex()` from the panel root makes assertion 1 fail
//! (the list would measure N_ROWS * ROW_HEIGHT instead).
//!
//! Note: every row carries the same debug id "filter-row"; `debug_bounds` is a
//! last-write-wins map, so querying it yields the bounds of the last painted row.

use gpui::{
    div, point, px, size, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, TestAppContext, Window,
};

const ROW_HEIGHT: f32 = 20.0;
const N_ROWS: usize = 200;
const WINDOW_W: f32 = 1600.0;
const WINDOW_H: f32 = 900.0;

struct PanelMirror {
    _private: (),
}

impl Render for PanelMirror {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Mirrors MainView (renderer.rs): a flex row holding the panel.
        div().flex().flex_row().w_full().h_full().child(
            // Mirrors FilterPanel::render (filter_panel.rs) — note the `.flex()` on this
            // root: without it, `flex_1()` on the list below has no effect.
            div()
                .debug_selector(|| "panel".to_string())
                .flex()
                .h_full()
                .w_72()
                .bg(gpui::rgb(0x1a1a2e))
                .border_r_1()
                .border_color(gpui::rgb(0x333355))
                .flex_col()
                // "Filters" header, same chain as production (incl. the fix's `.flex()`).
                .child(
                    div()
                        .debug_selector(|| "header".to_string())
                        .flex()
                        .h_6()
                        .w_full(),
                )
                .child(
                    div()
                        .debug_selector(|| "filter-list".to_string())
                        .flex_1()
                        .id("filter-panel-list")
                        .overflow_y_scroll()
                        .p_2()
                        .children((0..N_ROWS).map(|_| {
                            // FilterOption rows: uniform 20px height as in production.
                            div()
                                .debug_selector(|| "filter-row".to_string())
                                .h(px(ROW_HEIGHT))
                                .w_full()
                        })),
                ),
        )
    }
}

#[gpui::test]
fn filter_list_is_clamped_to_panel_height(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();

    // Paint the panel at window size; debug_bounds are recorded during this frame.
    cx.draw(
        point(px(0.), px(0.)),
        size(px(WINDOW_W), px(WINDOW_H)),
        |_w, cx| cx.new(|_| PanelMirror { _private: () }).into_any_element(),
    );

    let panel = cx.debug_bounds("panel").expect("panel should be painted");
    assert!(
        (panel.size.height - px(WINDOW_H)).abs() < px(1.0),
        "panel fills window height: expected {}, got {}",
        WINDOW_H,
        panel.size.height
    );

    let list = cx
        .debug_bounds("filter-list")
        .expect("list should be painted");

    // THE regression assertion: with `.flex()` on the panel root, `flex_1()`
    // clamps the list to the remaining panel height (≈ window minus header rows).
    // Without it, taffy sizes the list to its full content (N_ROWS * ROW_HEIGHT px)
    // and this fails by a wide margin.
    let content_height = px(N_ROWS as f32 * ROW_HEIGHT);
    assert!(
        list.size.height < px(WINDOW_H),
        "list must be clamped below the window, not sized to its {}px content; got {}",
        N_ROWS as f32 * ROW_HEIGHT,
        list.size.height
    );
    assert!(
        list.size.height + px(1.0) < content_height,
        "list ({}) must be smaller than its content ({}), otherwise there is nothing to scroll",
        list.size.height,
        content_height
    );

    // The last painted row overflows the visible area — content extends past the panel bottom,
    // which is what makes wheel scrolling meaningful in the first place.
    let last_row = cx
        .debug_bounds("filter-row")
        .expect("rows should be painted");
    assert!(
        last_row.origin.y + px(ROW_HEIGHT) > px(WINDOW_H),
        "content must overflow the window: last row bottom {} exceeds {}",
        last_row.origin.y + px(ROW_HEIGHT),
        WINDOW_H
    );

    // Sanity: header has nonzero height and sits above the list.
    let header = cx.debug_bounds("header").expect("header should be painted");
    assert!(header.size.height > px(1.0));
    assert!((list.origin.y - panel.origin.y) >= (header.origin.y + header.size.height) - px(2.0));
}
