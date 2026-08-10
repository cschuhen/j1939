# can_decoder GPUI Design Document

## Status (checked 2026-08-11)

| Phase | Status |
|-------|--------|
| Phase 1 — Foundation & Skeleton | ✅ COMPLETE (module structure, actions, stubs, binary entry point, three-panel layout all working) |
| Phase 2 — Message List & Core Display | ✅ COMPLETE (UniformList, message receiving, row selection, keyboard navigation with FocusHandle fully wired) |
| Phase 3 — Filter Widgets & Dockable LHS | 🚧 IN PROGRESS |
| Phase 4 — Detail Panel & RHS Dock | ⏳ PENDING |
| Phase 5 — Status Bar, Modals & Polish | ⏳ PENDING |
| Phase 6 — Docking UX & Persistence | ⏳ PENDING |

### GPUI Version Note (2026-08-11)
✅ MIGRATED: Now using Zed mainline git dependency at rev `f3fb4e04aa85dbbde6e83d28f231fc452cd8863f` (same as rgitUI). Breaking changes fixed: `Application::new()` → `application()`, `window.focus(handle)` → `window.focus(handle, cx)`. See GPUI Version Evaluation section for full analysis.

---

## Overview

This document outlines the design for a GPU-accelerated native GUI version of `can_decoder` using [GPUI](https://gpui.rs), the UI framework from the Zed editor. The GPUI version will provide the same feature set as the TUI (`can_decoder_tui`) but with a modern, hardware-accelerated interface and dockable panel system.

### Goals

- **Feature parity** with `can_decoder_tui`: same filters, columns, message display, detail view
- **Dockable panels**: LHS filter widgets and RHS details panel use GPUI's native docking system (like Zed's sidebar and inspector)
- **Code sharing**: maximize reuse of core logic (filter engine, PGN decoder, formats, scroll manager) between TUI and GPUI frontends
- **Keyboard navigation**: preserve all F-keys, Tab/Shift+Tab, arrow keys, Space, Enter, Esc — synchronized with TUI behavior

### Non-Goals

- Replacing the existing TUI (both will coexist as separate binaries)
- Web-based rendering (GPUI is native-only)
- Multi-window architecture (single window with dockable panels)

---

## Architecture

### Three-Tier Structure

```
┌─────────────────────────────────────────┐
│           FRONTEND LAYER                │
│  ┌──────────────┐    ┌──────────────┐   │
│  │  TUI (ratatui)│    │  GPUI (gpui) │   │
│  └──────────────┘    └──────────────┘   │
├─────────────────────────────────────────┤
│           SHARED LAYER                  │
│  filter_engine │ filter_editor │ formats│
│  scroll_manager │ columns │ renderers   │
├─────────────────────────────────────────┤
│           CORE / BACKEND                │
│  pipeline │ pgn_decoder │ device_mgr    │
│  tp_reassembler │ sources │ types │ traits│
└─────────────────────────────────────────┘
```

### Shared Modules (src/ root level — used by both TUI and GPUI)

| Module | Purpose | GUI Dependencies |
|--------|---------|------------------|
| `types.rs` | All data types (`RawFrame`, `DecodedMessage`, `Severity`, etc.) | None |
| `traits.rs` | Trait definitions (`Source`, `Decoder`, `ComplexDecoder`, `Renderer`) | None |
| `pipeline.rs` | Async pipeline wiring (tokio channels, stages) | None |
| `pgn_decoder.rs` | J1939 PGN decoding engine + YAML config dispatch | None |
| `device_manager.rs` | Device tracking, NAME resolution, parameter cache | None |
| `tp_reassembler.rs` | Transport Protocol reassembly (BAM/RTS-CTS) | None |
| `sources.rs` | SocketCanSource, CandumpFileSource implementations | None |
| `filter_engine.rs` | Message storage, filter evaluation, dirty-flag optimization | None |
| `filter_editor.rs` | Filter editor state machine (FieldType, options cache) | None |
| `scroll_manager.rs` | Scroll buffer management, viewport tracking | None |
| `columns.rs` | Column enum (AbsTime, Time, Src, Dest, Pgn, etc.) + format methods | None |
| `formats.rs` | Pure formatting functions (`format_timestamp`, `format_data_hex`, etc.) | None |
| `renderers.rs` | Console/Json/Csv/Condensed renderers (swap owo-colors for GPUI colors) | Minimal |

### TUI-Specific Modules (src/tui/)

All ratatui/crossterm-dependent code stays in `src/tui/`:
- `app.rs`, `renderer.rs`, `events.rs`, `keybindings.rs`
- `components/` (status_bar, table, panel, message_row)
- `widgets/` (custom ratatui widgets)
- `theme.rs` (ratatui Color definitions)
- `layout.rs` (ratatui Constraint/Direction)

### GPUI-Specific Modules (src/gpui/)

New module mirroring TUI structure but using GPUI APIs:
- `app.rs` — Application state, screen modes, focus tracking
- `renderer.rs` — GPUI Render trait implementation
- `events.rs` — Keystroke → Action mapping
- `keybindings.rs` — Keymap configuration (F-keys, Tab, arrows)
- `components/` — Filter widgets, message list, detail panel, status bar
- `theme.rs` — GPUI color/theme definitions

### Module Classification Summary

| Category | Modules | Reuse Strategy |
|----------|---------|----------------|
| **CORE** (no changes needed) | `types.rs`, `traits.rs`, `pipeline.rs`, `pgn_decoder.rs`, `device_manager.rs`, `tp_reassembler.rs`, `sources.rs` | Shared identically by TUI and GPUI. Both frontends create their own pipeline instance wired to the same CAN source. |
| **SHARED** (minor adaptation) | `filter_engine.rs`, `filter_editor.rs`, `scroll_manager.rs`, `columns.rs`, `formats.rs` | Pure logic, zero GUI imports. Shared identically. GPUI uses same structs/methods. |
| **SHARED** (color system swap) | `renderers.rs` | Uses `owo-colors::OwoColorize`. GPUI version strips ANSI codes and returns plain strings; styling handled by GPUI components. |
| **TUI-SPECIFIC** | All of `src/tui/` | Ratatui/Crossterm dependencies. Stays untouched. |
| **GPUI-SPECIFIC** | New `src/gpui/` | GPUI Entity system, Views, Elements, Actions, Keymap. Mirrors TUI structure. |

---

## Dockable Panel Layout

### Design Philosophy

The LHS filter widgets and RHS details panel use GPUI's native dockable panel system (the same mechanism Zed uses for its sidebar, project outline, and inspector panels). This provides:

- **Drag-to-reposition**: Users can drag the LHS or RHS panel to detach it, move it to a different edge, or float it as a separate window
- **Resizable splitters**: Panel widths/heights adjustable via drag handles
- **Persisted layout**: Dock positions saved between sessions (via GPUI's state persistence)

### Default Layout

```
┌─────────────────────────────────────────────────────┐
│  Status Bar (full width, top)                        │
├──────────┬──────────────────────────┬───────────────┤
│          │                          │               │
│  LHS     │       MAIN AREA          │    RHS        │
│  Dock    │   Message List           │   Detail      │
│  (Filters│   UniformList            │   Panel       │
│   Widget)│   + ScrollManager        │   (selected msg│
│          │                          │    details,   │
│          │                          │    hex dump)  │
│          │                          │               │
├──────────┴──────────────────────────┴───────────────┤
│  Status Bar (full width, bottom)                     │
└─────────────────────────────────────────────────────┘
```

### GPUI Dock Implementation

GPUI provides dockable panels through the `ManagedView` trait and its docking system. Each dock area is a `ManagedView` that can be dragged to reposition or detached into a floating window.

**LHS Filter Panel (`FilterDock`):**
- Implements `ManagedView` for GPUI's docking system
- Contains filter widgets (Title, PGN, Severity, Source, Dest, Numeric, Flag, SourceName, DestName, Regex)
- Each widget is an entity with enabled/disabled state, input text, cursor position, expanded/collapsed state
- Panel can be dragged to left/right edge or detached as floating window

**RHS Detail Panel (`DetailDock`):**
- Implements `ManagedView` for GPUI's docking system
- Shows decoded message details when a row is selected
- Includes hex dump view, raw data display, device info from DeviceManager
- Panel can be dragged to left/right edge or detached as floating window

**Main Area:**
- Not dockable (fixed center area)
- Contains the `MessageList` component (GPUI UniformList for efficient rendering)
- Status bar at top and bottom

### Layout Toggle

Same as TUI: `Ctrl+L` toggles between horizontal layout (LHS/Main/RHS side-by-side) and vertical layout (Top/Bottom stacked). GPUI's flexbox/grid layouts make this a simple style change on the root container.

---

## Entity Model for State Management

GPUI uses an `Entity` system — state is owned by the `App` and accessed through smart pointers similar to `Rc`. This replaces the TUI's mutable struct approach with a more structured ownership model.

### Core Entities

| GPUI Entity | TUI Equivalent | Purpose |
|-------------|----------------|---------|
| `AppState` | `tui::app.rs::App` | Root application state: screen mode, focus handle, scroll manager, filter widgets, column config |
| `FilterEngineState` | `filter_engine.rs::FilterEngine` | Message buffer, dirty flags, scroll position (shared logic) |
| `DeviceManager` | `device_manager.rs::DeviceManager` | Device tracking, NAME resolution, parameter cache (shared logic) |
| `FilterWidgetEntity` | `tui::app.rs::FilterWidget` | Individual filter widget state (enabled, input text, cursor, expanded) |
| `ColumnConfig` | `tui/columns.rs::ColumnConfig` | Enabled columns, widths, visibility toggles |

### State Flow

```
App (root view)
├── Entity<AppState>          ← root entity holding all app state
│   ├── Entity<FilterEngineState>  ← shared filter engine (message buffer + evaluation)
│   ├── Entity<DeviceManager>      ← shared device tracking
│   ├── Vec<Entity<FilterWidget>>  ← one entity per filter widget type
│   ├── Entity<ColumnConfig>       ← column visibility/width config
│   └── Entity<MessageScrollManager> ← scroll position tracking (shared)
├── MessageList View          ← GPUI UniformList rendering filtered messages
├── DetailPanel View          ← RHS detail view for selected message
└── FilterDock ManagedView    ← LHS dockable filter panel
```

### Key Design Decisions

1. **Shared entities for core logic**: `FilterEngine`, `DeviceManager`, `ScrollManager` are shared between TUI and GPUI as regular structs (not GPUI entities). They're wrapped in `Entity<>` handles within the GPUI app.

2. **One entity per filter widget**: Each of the 10 filter types (Title, PGN, Severity, etc.) is its own `Entity<FilterWidget>` for independent state management and easy drag-and-drop reordering in dock panels.

3. **Message buffer lives in FilterEngine**: The TUI's message storage (`Vec<DecodedMessage>`) stays in the shared `FilterEngine` struct. GPUI views read from this via entity references — no duplication of message data.

4. **Focus management**: GPUI's `FocusHandle` system replaces the TUI's manual cursor tracking. Focus can be on:
   - Main message list (arrow key navigation)
   - A filter widget input field (text editing mode)
   - Status bar (for future command palette)

---

## Actions & Keymap System

GPUI's action system converts keystrokes into logical operations via a keymap. This replaces the TUI's manual `KeyEvent` → action mapping in `events.rs`.

### Action Definitions

Actions are unit structs registered with GPUI's runtime using the `actions!` macro or `Action` derive. Each action corresponds to one user operation:

```rust
// Example action definitions (src/gpui/actions.rs)
actions![
    ToggleFilter,           // F1 — toggle first filter widget
    NextScreenMode,         // F2 — switch screen mode
    ScrollUp,               // Arrow Up / Page Up
    ScrollDown,             // Arrow Down / Page Down
    SelectRow,              // Enter — select message row
    EditFilterInput,        // Tab — move focus to filter input
    ConfirmFilterEdit,      // Enter — confirm filter text entry
    CancelFilterEdit,       // Esc — cancel/exit edit mode
    ToggleLayout,           // Ctrl+L — horizontal/vertical layout toggle
    ClearMessages,          // Ctrl+C — clear message buffer
    Quit,                   // Ctrl+Q — quit application
]
```

### Keystroke → Action Mapping

| Keystroke | TUI Keybinding | GPUI Action | Notes |
|-----------|----------------|-------------|-------|
| `F1` | Toggle first filter | `ToggleFilter` | First filter = Title |
| `F2` | Next screen mode | `NextScreenMode` | Cycles Main → Filter → ErrorLog |
| `F3` | Third action | TBD | Currently unused in TUI |
| `F4` | Fourth action | TBD | Currently unused in TUI |
| `F5` | Fifth action | TBD | Currently unused in TUI |
| `Tab` | Next focus target | `EditFilterInput` | Cycles: list → filter widget → status bar |
| `Shift+Tab` | Previous focus target | `EditFilterInput` (prev) | Reverse cycle |
| `↑` / `↓` | Scroll / navigate | `ScrollUp` / `ScrollDown` | Arrow key navigation in message list |
| `Page Up/Down` | Page scroll | `ScrollUp` / `ScrollDown` | Larger scroll increments |
| `Space` | Toggle selection | `SelectRow` | Select/deselect current row |
| `Enter` | Confirm / open detail | `SelectRow` | Same as Space for message selection |
| `Esc` | Cancel / exit modal | `CancelFilterEdit` | Exit filter edit mode, close modals |
| `Ctrl+L` | Toggle layout | `ToggleLayout` | Horizontal ↔ Vertical |
| `Ctrl+C` | Clear messages | `ClearMessages` | Clear message buffer |
| `Ctrl+Q` | Quit | `Quit` | Close application |

### Keymap Configuration

GPUI's keymap system uses JSON configuration files (like VS Code / Zed). The default keymap for can_decoder GPUI:

```jsonc
// src/gpui/keymaps/default.json
[
  {
    "context": "MessageList",
    "bindings": {
      "up": ["ScrollUp"],
      "down": ["ScrollDown"],
      "pageup": ["ScrollUp", "PageUp"],
      "pagedown": ["ScrollDown", "PageDown"],
      "enter": ["SelectRow"],
      "space": ["SelectRow"]
    }
  },
  {
    "context": "FilterInput",
    "bindings": {
      "enter": ["ConfirmFilterEdit"],
      "escape": ["CancelFilterEdit"],
      "backspace": ["Backspace"],
      "delete": ["Delete"],
      "left": ["MoveLeft"],
      "right": ["MoveRight"]
    }
  },
  {
    "context": "App",
    "bindings": {
      "f1": ["ToggleFilter"],
      "f2": ["NextScreenMode"],
      "tab": ["EditFilterInput"],
      "shift+tab": ["EditFilterInputPrev"],
      "ctrl+l": ["ToggleLayout"],
      "ctrl+c": ["ClearMessages"],
      "ctrl+q": ["Quit"]
    }
  }
]
```

### Context Resolution

GPUI's `KeyContext` resolves which bindings apply based on the current focus handle. This replaces the TUI's manual context tracking:

- When focus is on the message list → `MessageList` context active
- When focus is on a filter widget input → `FilterInput` context active
- Global keybindings (F-keys, Ctrl+Q) always apply regardless of focus (`App` context)

### Input Mode Handling

GPUI handles input mode transitions via focus changes:
- Clicking a filter widget input field → switches to text input mode automatically
- Pressing Tab → cycles focus between list and widgets
- Pressing Esc → exits text input, returns to navigation mode

---

## Component Mapping: TUI → GPUI

This section maps each TUI component to its GPUI equivalent. The goal is feature parity while leveraging GPUI's native capabilities.

### Layout Components

| TUI Component | File | GPUI Equivalent | GPUI API Used |
|---------------|------|-----------------|---------------|
| Terminal layout | `tui/layout.rs` | Root container with flexbox | `Div::flex()` + `Div::flex_col()` / `flex_row()` |
| LHS panel (filters) | `tui/components/panel.rs` | Dockable filter panel | `ManagedView` trait + dock system |
| Main area (message list) | `tui/components/table.rs` | Message list view | `UniformList` for efficient rendering |
| RHS panel (details) | `tui/components/panel.rs` | Dockable detail panel | `ManagedView` trait + dock system |
| Status bar | `tui/components/status_bar.rs` | Top/bottom status bars | Simple `Div` with styled text |

### Message List Components

| TUI Component | File | GPUI Equivalent | GPUI API Used |
|---------------|------|-----------------|---------------|
| Table widget | `tui/components/table.rs` | `MessageList` view | `UniformList<TableRowEntity>` |
| Row rendering | `tui/components/message_row.rs` | `TableRow` entity + Render impl | Entity with styled text runs |
| Column headers | `tui/columns.rs` (TUI ext) | Header row in list | Fixed header above UniformList |
| Selection highlight | TUI table styling | Selected row background color | GPUI style cascade |
| Scroll handling | `tui/scrolling.rs` | `UniformListScrollHandle` | Built-in scroll management |

### Filter Widget Components

| TUI Component | File | GPUI Equivalent | GPUI API Used |
|---------------|------|-----------------|---------------|
| Filter widget row | `tui/components/panel.rs` | `FilterWidgetView` entity | Entity with toggle button + input field |
| Filter editor modal | `filter_editor.rs` (shared) | Modal overlay / anchored popup | GPUI `Anchored` element or modal view |
| Toggle checkbox | TUI custom widget | Toggle button | `Button` with selected state |
| Text input | TUI custom widget | Input field | GPUI text input element |

### Detail Panel Components

| TUI Component | File | GPUI Equivalent | GPUI API Used |
|---------------|------|-----------------|---------------|
| Message details view | `tui/components/panel.rs` | `DetailPanel` view | Scrollable `Div` with styled content |
| Hex dump display | TUI table formatting | Hex viewer component | Monospace text with color runs |
| Raw data display | TUI formatting | Raw data section | Styled text using `formats::build_detail_string()` |

### Modal / Overlay Components

| TUI Component | File | GPUI Equivalent | GPUI API Used |
|---------------|------|-----------------|---------------|
| Filter editor modal | N/A (inline) | Anchored popup panel | GPUI `Anchored` element |
| Column config popup | `tui/components/panel.rs` | Modal overlay | GPUI anchored/modal system |
| Error log overlay | TUI screen mode | Separate view / tab | GPUI managed view or tab bar |

### Rendering Differences

**TUI rendering approach:**
- Uses ratatui widgets (`Table`, `Row`, `Cell`, `Block`)
- Colors via `owo-colors` → ANSI escape codes
- Text layout via ratatui's text system
- Full terminal redraw each frame (~60fps target)

**GPUI rendering approach:**
- Uses GPUI elements (`Div`, `UniformList`, styled text runs)
- Colors via GPUI's color system (Hsla, Rgba)
- Text layout via GPUI's text system (cosmic-text backend)
- GPU-accelerated rendering, only changed elements redrawn

### Shared Rendering Logic

The following rendering logic is shared between TUI and GPUI:
- `formats.rs` — timestamp, elapsed time, hex data formatting
- `columns.rs::Column::format()` — column-specific value formatting
- `renderers.rs` — Console/Json/Csv renderers (GPUI strips ANSI codes)
- `filter_engine.rs::matches()` — filter evaluation logic

---

## Implementation Phases

### Phase 1: Foundation & Skeleton (Weeks 1-2) ✅ COMPLETE

**Goal**: Build the GPUI application skeleton with basic window, shared pipeline, and empty layout.

| Task | Status | Details |
|------|--------|---------|
| Create `src/gpui/` module structure | ✅ COMPLETE | Module root with re-exports: `App`, `Application`, `Context`, `Entity`, `IntoElement`, `Render`, `Window`. Submodules declared: `app_state`, `components`, `keybindings`, `renderer`. (13 lines) |
| Add `gpui` dependency to Cargo.toml | ✅ COMPLETE | Added `gpui = "0.2"` dependency and `[[bin]]` target for `can_decoder_gpui` pointing to `src/main_gpui.rs`. |
| Implement action definitions | ✅ COMPLETE | 10 actions defined using `gpui::action!` macro: `ToggleFilter`, `NextScreenMode`, `ScrollUp`, `ScrollDown`, `SelectRow`, `EditFilterInput`, `CancelFilterEdit`, `ToggleLayout`, `ClearMessages`, `Quit`. All use `can_decoder_gpui` namespace. (18 lines) |
| Implement stub components | ✅ COMPLETE | Skeleton structs with `Render` trait implementations: `AppState`, `MainView`, `MessageList`, `StatusBar`. All use placeholder rendering (`div().size_full()`). (~125 lines total across 4 files) |
| Wire shared pipeline to GPUI | ✅ COMPLETE | `main_gpui.rs` builds full pipeline (source → decoder → filter → channel), creates Tokio runtime, leaks it for GPUI event loop lifetime. Window opens with `MainView` entity holding message receiver. |
| Build root container layout | ✅ COMPLETE | Three-panel flexbox layout in `MainView::render()`: LHS FilterPanel (w_72), center MessagePanel (flex_1), RHS DetailPanel (w_80). Each panel has header bar + content area with phase placeholder text. Dark theme colors matching TUI aesthetic. (~165 lines across 3 new panel structs) |

**Current Deliverables**: Compiling and running GPUI binary (`cargo build --bin can_decoder_gpui` succeeds, window opens with three-panel dark-themed layout, processes candump frames). Module structure with action definitions, stub components, working pipeline wiring, and complete three-panel flexbox layout. Build completes in ~2.9s with only unused struct warnings. All 24 existing tests pass.

**Key technical solutions:**
- **Borrow checker**: `open_window` callback receives its own `&mut App` (`cx`) — use `cx.new()` to create entities (pattern from [rgitui](https://github.com/noahbclarkson/rgitui))
- **Tokio runtime**: Create runtime upfront, call `rt.block_on(async { build_pipeline() })`, then leak with `Box::leak(Box::new(rt))` for GPUI event loop lifetime
- **Panel entities**: Each panel (FilterPanel, MessagePanel, DetailPanel) is a separate struct with its own Render impl, created via `cx.new()` in MainView and passed as Entity children

**Next Steps**: Phase 2 — Implement actual message list content (UniformList + scroll handling + row rendering).

### Phase 2: Message List & Core Display (Weeks 3-4) 🚧 IN PROGRESS

**Goal**: Display filtered messages in the main area with keyboard navigation.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Implement `MessageList` view | GPUI UniformList rendering `DecodedMessage` rows | Phase 1 | ✅ COMPLETE — MessageList struct with Vec<DecodedMessage>, selected_index, global_start_time. Uses `make_uniform_list()` wrapper for GPUI's uniform_list function. (~170 lines) |
| Wire FilterEngine to list | Read messages from shared FilterEngine entity | Phase 2 | ✅ COMPLETE — Tokio spawn in MainView receives from pipeline channel and calls `message_list.update(cx, ...)` to add messages. Auto-scrolls to end on new messages. |
| Implement row rendering | Styled text runs per column (using shared `columns.rs`) | Phase 2 | ✅ COMPLETE — `render_row()` function formats Time, Src, Dest, Pgn, Title, Detail columns using shared `formats::format_elapsed_time`, `build_detail_string`. Monospace font, selection highlighting with bg color. |
| Add scroll handling | `UniformListScrollHandle` + keyboard navigation (↑/↓) | Phase 2 | ✅ COMPLETE — `scroll_to_end()` method on MessageList. Keyboard nav methods: `select_prev()`, `select_next()`, `toggle_selection()`. |
| Implement selection | Click/Enter to select a row, highlight selected row | Phase 2 | ✅ COMPLETE — `selected_index: Option<usize>` tracks current selection. Selected rows get blue bg (0x1a3a5f) with white text. `selected_message()` accessor. |
| Add keymap for list navigation | Up/down arrows, page up/down, enter, space | Phase 2 | ✅ COMPLETE — FocusHandle added to MainView, window.focus() called in render(), .on_action() handlers wired for ScrollUp/ScrollDown/SelectRow. |

**Deliverables**: Functional message list with UniformList virtualization, message receiving from pipeline, row selection with highlighting, keyboard navigation fully operational.

### Phase 3: Filter Widgets & Dockable LHS (Weeks 5-6) 🚧 IN PROGRESS

**Goal**: Implement filter widgets in a dockable LHS panel.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Create `FilterWidget` entity model | One entity per filter type with state management | Phase 2 | ✅ COMPLETE — FilterWidget struct with field_type, input_text, enabled, focus_handle, label fields. Methods: new(), toggle_enabled(), is_enabled(), set_input_text(). |
| Implement filter widget views | Toggle button + text input for each filter | Phase 3 | 🚧 IN PROGRESS — Toggle button (ON/OFF) with hover effects, visual enabled/disabled states. Text input functional: click to focus, type characters, Backspace to delete, cursor shown with | character. |
| Build LHS dockable panel | ManagedView implementing dock system | Phase 3 | ⏳ PENDING — FilterPanel exists as simple div container, not yet using GPUI dock system. |
| Wire filters to FilterEngine | Enable/disable filters, update input text → re-evaluate | Phase 3 | ⏳ PENDING |
| Implement filter editor modal | GPUI anchored popup for complex filter editing (Numeric, Flag) | Phase 3 | ⏳ PENDING |
| Add F1 keybinding | Toggle first filter widget focus/enable | Phase 3 | ⏳ PENDING |

**Deliverables**: Filter widgets with toggle functionality and visual state indicators. Dockable panel wiring pending.

### Phase 4: Detail Panel & RHS Dock (Weeks 7-8) ⏳ PENDING

**Goal**: Implement detail view in a dockable RHS panel.

| Task | Details | Status |
|------|---------|--------|
| Create `DetailPanel` view | Shows decoded message details for selected row | ⏳ PENDING |
| Implement hex dump display | Monospace text with color-coded bytes | ⏳ PENDING |
| Show device info from DeviceManager | Source/dest addresses, NAME resolution | ⏳ PENDING |
| Build RHS dockable panel | ManagedView implementing dock system | ⏳ PENDING |
| Wire selection to detail view | Clicking a row updates detail panel content | ⏳ PENDING |

**Deliverables**: Dockable RHS with message details, feature parity with TUI.

### Phase 5: Status Bar, Modals & Polish (Weeks 9-10) ⏳ PENDING

**Goal**: Complete remaining UI elements and polish the experience.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Implement status bars | Top bar (mode indicator) + bottom bar (status info) | Phase 4 | ⏳ PENDING |
| Implement column config modal | Popup for toggling columns / adjusting widths | Phase 4 | ⏳ PENDING |
| Add error log view | Screen mode equivalent for error messages | Phase 4 | ⏳ PENDING |
| Implement layout toggle | Ctrl+L horizontal ↔ vertical switch | Phase 4 | ⏳ PENDING |
| Theme/color system | GPUI-native color definitions matching TUI theme | Phase 5 | ⏳ PENDING |
| Keyboard shortcuts completion | All F-keys, Tab/Shift+Tab, Esc, Ctrl+Q | Phase 5 | ⏳ PENDING |

**Deliverables**: Feature-complete GPUI application matching all TUI functionality.

### Phase 6: Docking UX & Persistence (Weeks 11-12) ⏳ PENDING

**Goal**: Refine dockable panel experience and add layout persistence.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Test drag-to-reposition | Verify LHS/RHS panels can be dragged to different edges | Phase 5 | ⏳ PENDING |
| Test detach-as-floating-window | Verify panels can be detached into separate windows | Phase 5 | ⏳ PENDING |
| Implement layout persistence | Save/restore dock positions between sessions | Phase 6 | ⏳ PENDING |
| Performance optimization | Profile rendering, optimize UniformList if needed | Phase 6 | ⏳ PENDING |
| Edge case handling | Empty message list, malformed data, rapid stream mode | Phase 6 | ⏳ PENDING |

**Deliverables**: Production-ready GPUI application with polished docking UX.

---

## File Structure

### Actual Directory Layout (Phase 1 Progress)

```
can_decoder/
├── Cargo.toml                          — ✅ gpui dependency + can_decoder_gpui binary target added
├── src/
│   ├── main.rs                         — CLI entry point (existing, TUI binary)
│   ├── main_gpui.rs                    — ✅ GPUI binary entry point (full pipeline wiring, Tokio runtime, window with MainView)
│   │
│   │   /* CORE / BACKEND - shared identically */
│   ├── types.rs                        — All data types (no changes needed)
│   ├── traits.rs                       — Trait definitions (no changes needed)
│   ├── pipeline.rs                     — Async pipeline (no changes needed)
│   ├── pgn_decoder.rs                  — J1939 decoder engine (no changes needed)
│   ├── device_manager.rs               — Device tracking (no changes needed)
│   ├── tp_reassembler.rs               — TP reassembly (no changes needed)
│   ├── sources.rs                      — CAN sources (no changes needed)
│   │
│   │   /* SHARED - minor adaptation for GPUI */
│   ├── filter_engine.rs                — Message storage + evaluation (shared, no changes)
│   ├── filter_editor.rs                — Filter editor state machine (shared, no changes)
│   ├── scroll_manager.rs               — Scroll buffer management (shared, no changes)
│   ├── columns.rs                      — Column enum + format methods (shared, no changes)
│   ├── formats.rs                      — Formatting functions (shared, no changes)
│   │
│   │   /* TUI-SPECIFIC - untouched */
│   └── tui/                            — All existing TUI code (no changes)
│       ├── mod.rs
│       ├── app.rs
│       ├── renderer.rs
│       ├── events.rs
│       ├── keybindings.rs
│       ├── layout.rs
│       ├── theme.rs
│       ├── columns.rs                  — TUI-specific ColumnConfig/ColumnState
│       ├── components/
│       │   ├── status_bar.rs
│       │   ├── table.rs
│       │   ├── panel.rs
│       │   └── message_row.rs
│       └── widgets/
│           └── ...
│
│   /* GPUI-SPECIFIC - new module (Phase 1-2 progress) */
├── src/gpui/
│   ├── mod.rs                          — ✅ Module organization, exports (13 lines)
│   ├── app_state.rs                    — 🚧 AppState entity stub + Clone derive (21 lines)
│   ├── renderer.rs                     — ✅ MainView + three panel structs (~220 lines)
│   │                                   — FilterPanel: LHS dockable filter widget container (~40 lines)
│   │                                   — MessagePanel: center message list area header (~35 lines)
│   │                                   — DetailPanel: RHS detail view container (~35 lines)
│   │                                   — Message receiving via cx.spawn() + Tokio task
│   ├── keybindings.rs                  — ✅ Action definitions using gpui::action! macro (18 lines)
│   │
│   └── components/
│       ├── mod.rs                      — ✅ Component module re-exports (6 lines)
│       ├── message_list.rs             — 🚧 MessageList with UniformList rendering (~170 lines)
│       │                               — uniform_list wrapper, row rendering, selection state
│       └── status_bar.rs               — 🚧 StatusBar stub (32 lines)
```

**Total GPUI code**: ~580 lines across 8 files (including main_gpui.rs). Build succeeds with only unused struct warnings. Window opens with three-panel dark-themed layout and processes candump frames successfully. All 24 existing tests pass.

### Cargo.toml Configuration

```toml
[dependencies]
gpui = "0.2"  # Pre-1.0, accept breaking changes

[[bin]]
name = "can_decoder_gpui"
path = "src/main_gpui.rs"
```

**Note**: The `gpui` dependency is pinned to version `"0.2"` (not a specific patch version) to allow minor updates while staying in the 0.x series where breaking changes are expected. Currently tested against gpui 0.2.2.

---

## Risks & Mitigations

### Pre-1.0 API Instability

**Risk**: GPUI is pre-1.0 with frequent breaking changes. The dockable panel system (`ManagedView`) may change significantly between versions.

**Mitigation**:
- Pin to a specific GPUI version (e.g., `gpui = "0.2.2"`) and update manually
- Abstract the docking layer behind a thin wrapper module so migration is localized
- Monitor Zed releases for dock system changes — the API is likely stable once Zed ships

### Learning Curve

**Risk**: GPUI has limited documentation beyond Zed source code. The Entity system, Views/Elements model, and dock system require significant study.

**Mitigation**:
- Study Zed's open-source codebase extensively (it's the primary reference)
- Start with simple components (status bar, basic list) before tackling docks
- Build a minimal GPUI prototype first to validate understanding of core concepts

### Performance Concerns

**Risk**: Large message buffers (10k+ messages) may cause rendering slowdowns if not optimized.

**Mitigation**:
- Use `UniformList` for efficient virtualized rendering (only visible rows rendered)
- Leverage GPUI's entity system for incremental updates (only changed entities re-render)
- Profile early and often; optimize with custom elements only if needed

### Code Duplication Risk

**Risk**: Accidentally duplicating logic that should be shared between TUI and GPUI.

**Mitigation**:
- Strict adherence to the three-tier architecture (CORE → SHARED → FRONTEND)
- Any new logic goes in `src/` root level first, then referenced by both frontends
- Code review checklist: "Is this logic GUI-specific or can it be shared?"

### Dock System Maturity

**Risk**: GPUI's dockable panel system may not have all features needed (e.g., nested docks, specific drag behaviors).

**Mitigation**:
- Start with simple left/right docks; add complexity incrementally
- If docking is too immature, fall back to fixed layout using GPUI flexbox/grid
- The shared message list and filter logic work regardless of panel arrangement

---

## Current Implementation Status (2026-08-10)

### Completed (Phase 1 - Foundation)

| Component | File | Lines | Status |
|-----------|------|-------|--------|
| Module structure | `src/gpui/mod.rs` | 13 | ✅ Complete — re-exports key GPUI types, declares submodules |
| Action definitions | `src/gpui/keybindings.rs` | 18 | ✅ Complete — 10 actions using `gpui::action!` macro |
| Component module | `src/gpui/components/mod.rs` | 6 | ✅ Complete — re-exports component submodules |
| AppState stub | `src/gpui/app_state.rs` | 21 | 🚧 Stub — skeleton struct with Clone derive, TODO: pipeline/device_manager/filter_engine init |
| MainView Render impl | `src/gpui/renderer.rs` | 220 | ✅ Complete — three-panel flexbox layout + message receiving via cx.spawn() |
| FilterPanel | `src/gpui/renderer.rs` | 40 | ✅ Complete — LHS dockable panel skeleton (w_72, dark theme header + content) |
| MessagePanel | `src/gpui/renderer.rs` | 35 | ✅ Complete — center message list area header (~35 lines) |
| DetailPanel | `src/gpui/renderer.rs` | 35 | ✅ Complete — RHS detail panel skeleton (w_80, dark theme header + content) |
| MessageList view | `src/gpui/components/message_list.rs` | 170 | 🚧 Phase 2 — UniformList rendering with row formatting, selection state, scroll handling |
| StatusBar stub | `src/gpui/components/status_bar.rs` | 32 | 🚧 Stub — TODO: mode/device count display |
| Binary entry point | `src/main_gpui.rs` | 119 | ✅ Complete — full pipeline wiring, Tokio runtime, GPUI window with MessageList entity |

**Total GPUI code**: ~580 lines across 8 files (including main_gpui.rs).  
**Build status**: `cargo build --bin can_decoder_gpui` succeeds in ~2.8s with only unused struct warnings. Window opens with three-panel dark-themed layout and processes candump frames successfully. All 24 existing tests pass.

### Next Steps (Phase 1 Completion)

1. ~~**Implement `AppState::new()`**~~ ✅ Done — pipeline wired in `main_gpui.rs` with Tokio runtime
2. ~~**Implement `MainView::render()`**~~ ✅ Done — three-panel flexbox layout: FilterPanel, MessagePanel, DetailPanel as GPUI Entity children
3. ~~**Wire binary entry point**~~ ✅ Done — mirrors TUI structure using GPUI event loop

### Phase 2 Progress (In Progress)

1. ~~**Implement `MessageList` view**~~ ✅ Done — UniformList rendering with shared columns.rs formatting
2. ~~**Wire messages to list**~~ ✅ Done — Tokio spawn in MainView feeds pipeline messages into MessageList entity
3. ~~**Implement row rendering**~~ ✅ Done — render_row() formats Time, Src, Dest, Pgn, Title, Detail columns
4. ~~**Add scroll handling**~~ ✅ Done — scroll_to_end(), select_prev/next(), toggle_selection() methods
5. ~~**Implement selection**~~ ✅ Done — selected_index tracking with blue highlight on selected rows
6. **Keymap wiring** ⏳ Pending — connect ScrollUp/ScrollDown actions to MessageList navigation methods

### Pending Phases

| Phase | Description | Dependency |
|-------|-------------|------------|
| ~~Phase 1~~ | ~~Foundation & Skeleton~~ | — | ✅ COMPLETE |
| Phase 2 | Message List & Core Display | Phase 1 completion | 🚧 IN PROGRESS (keymap wiring pending) |
| Phase 3 | Filter Widgets & Dockable LHS | Phase 2 | ⏳ PENDING |
| Phase 4 | Detail Panel & RHS Dock | Phase 3 | ⏳ PENDING |
| Phase 5 | Status Bar, Modals & Polish | Phase 4 | ⏳ PENDING |
| Phase 6 | Docking UX & Persistence | Phase 5 | ⏳ PENDING |

# Workflow

Implement functionality in managable steps and for each step:
- Implement buildable chunk of code
- Test the implemented code compiles, iterate until `cargo build && cargo test` passes
- Update the status document
- Run `cargo +nightly fmt` to ensure formatting
- Stage and commit all rust .rs files. Running this will invalidate your memory of any files that you wrote, you need to re-read them on the next step.
- Proceed to next step

# Reference works

## RGitGui
- Checked out here: /home/cschuhen/rust/rgitui
- URL: https://github.com/noahbclarkson/rgitui.git
- Uses git hash version of gpui, presumably much newer than the 10 month old released crate

## GitComet
- Checked out here: /home/cschuhen/rust/GitComet
- URL: https://github.com/Auto-Explore/GitComet.git
- Uses a git hash version of gpui-ce (a fork of Zed's gpui)


---

## GPUI Version Evaluation (2026-08-11)

### Current State
- **can_decoder** uses `gpui = "0.2"` from crates.io — released ~10 months ago
- This is the stable pre-1.0 release, which means it will not receive further updates
- Several API limitations encountered: no built-in TextInput, UniformList `'static` closure restrictions, focus handling complexity

### Option A: Zed Mainline (RGitUI's approach)
- **Source**: `https://github.com/zed-industries/zed.git` at rev `f3fb4e04aa85dbbde6e83d28f231fc452cd8863f`
- **Packages**: `gpui` + `gpui_platform` (with features: font-kit, x11, wayland)
- **Pros**:
  - Directly tracks Zed editor development — the primary consumer of GPUI
  - Most up-to-date with official GPUI roadmap and 1.0 preparation
  - Active development with regular commits from Zed team
  - Best documentation alignment with future gpui.rs releases
  - rgitui is a mature, production-quality application using this approach
- **Cons**:
  - Breaking changes between Zed commits (expected for pre-1.0)
  - Must pin to specific git rev for reproducibility
  - Requires `[patch.crates-io]` or direct git dependency in Cargo.toml

### Option B: GPUI-CE Fork (GitComet's approach)
- **Source**: `https://github.com/Havunen/gpui-ce.git` at rev `f5c044833b803206ebc5c91a10361ec28f389c3b`
- **Note**: This is a fork of `gpui-ce/gpui-ce` (GPUI Community Edition)
- **Packages**: `gpui` + `gpui_platform` (with feature: font-kit)
- **Pros**:
  - Community-maintained fork focused on independence from Zed
  - Claims to be a drop-in replacement for mainline GPUI
  - Tracks upstream Zed changes and treats mismatches as bugs
  - Has patch block support for compatibility with crates.io consumers
  - Active community via Discord
- **Cons**:
  - Smaller community and less proven than mainline approach
  - Fork may diverge from Zed's official direction
  - gpui-ce organization has its own ecosystem (gpui-component compatibility)
  - Less documentation alignment with future gpui.rs

### Recommendation: Option A — Zed Mainline (RGitUI's approach)

**Rationale**:
1. **Closer to GPUI 1.0**: The Zed mainline branch is the canonical source for GPUI development. When GPUI 1.0 releases, it will come from this repository.
2. **Proven in production**: rgitui demonstrates that this approach works reliably for a complex Git client application.
3. **Better long-term maintenance**: Following Zed's official track means fewer compatibility surprises when GPUI 1.0 ships.
4. **API maturity**: The git version likely includes fixes and improvements not yet in the 0.2 crates.io release, potentially resolving our current limitations (TextInput, focus handling, UniformList callbacks).

**Migration plan**:
1. Update `Cargo.toml` to use git dependency:
   ```toml
   gpui = { git = "https://github.com/zed-industries/zed.git", rev = "f3fb4e04aa85dbbde6e83d28f231fc452cd8863f" }
   gpui_platform = { git = "https://github.com/zed-industries/zed.git", rev = "f3fb4e04aa85dbbde6e83d28f231fc452cd8863f", features = ["font-kit", "x11", "wayland"] }
   ```
2. Test build and verify all GPUI APIs still work
3. Address any breaking changes (likely minor API signature updates)
4. Re-evaluate focus handling, TextInput alternatives, and UniformList patterns with the newer API

**Risk assessment**:
- **Medium risk**: Pre-1.0 git versions will have breaking changes, but pinning to a specific rev mitigates this
- **Mitigation**: Start with rgitUI's known-working commit; upgrade incrementally if needed
- **Impact**: Requires updating imports and possibly method signatures, but architecture remains compatible
