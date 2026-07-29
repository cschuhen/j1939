# Implementation Plan: TUI CAN Decoder

## Phase 1: TUI Skeleton & Layout
*   [ ] **Add Dependencies**: Add `ratatui` and `crossterm` to `Cargo.toml`.
*   [ ] **Define App State**: Create a `TuiApp` struct to manage focus (LHS, Main, RHS), panel visibility (F-keys), and scrolling position.
*   [ ] **Basic Layout**: Implement the three-column layout (LHS, Main, RHS) and the bottom Status Bar using `ratatui`'s `Layout`.
*   [ ] **Event Loop**: Set up the main `tokio` event loop to handle terminal input (crossterm) and UI rendering.

## Phase 2: Data Plumbing & Main Message List
*   [ ] **Async/Sync Bridge**: Implement an `mpsc` channel to send `DecodedMessage` from the existing pipeline to the TUI event loop.
*   [ ] **TuiRenderer Implementation**: Implement the `Renderer` trait for a new `TuiRenderer` that interacts with the `TuiApp` state.
*   [ ] **Main Viewport Widget**: Implement a virtualized list widget that renders only the messages currently in the viewport.
*   [ ] **Scrolling Logic**: Implement "Live Stream" (auto-scroll to bottom) and "Manual Scroll" modes.
*   [ ] **Main Navigation**: Implement `Up`/`Down` (incremental) and `Shift`+`Up`/`Down` (page jump) for the list.

## Phase 3: Detail Inspector (RHS)
*   [ ] **Detail Widget**: Implement a widget to display all `DecodedField`s of a selected message.
*   [ ] **Metadata Integration**: Ensure `DeviceName` from `DeviceManager` is displayed in the details.
*   [ ] **RHS Navigation**: Implement `Shift` + `Left` to focus the Detail Panel.

## Phase 4: Filter Stack (LHS)
*   [ ] **Filter Widget Stack**: Implement the vertical stack of "Filter Widgets" in the LHS.
*   [ ] **Widget States**: Implement the "Expanded" (full controls) and "Minimized" (single line) states for widgets.
*   [ ] **LHS Navigation**: Implement `Shift` + `Up`/`Down` (focus) and `Shift` + `Right` (exit to Main).
*   [ ] **Input Modes**: Implement "Navigation Mode" (arrows/space) and "Text Input Mode" (for text filters).
*   [ ] **Filter Engine Integration**: Link LHS widget states to the active `Filter` logic in the processing pipeline.

## Phase 5: Polish & Extras
*   [ ] **Status Bar**: Implement real-time status updates (connection status, active filter count).
*   [ ] **Error Log Popup**: Implement the F3 overlay for displaying recent errors/warnings.
*   [ ] **Focus Management**: Refine the global `FocusManager` to handle all complex keyboard interactions (Enter, Space, Arrows).
*   [ ] **Styling**: Apply colorization and consistent formatting across all panels.
