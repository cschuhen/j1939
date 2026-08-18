# can_decoder GPUI Design Document

## Status (checked 2026-08-18)

| Phase | Status |
|-------|--------|
| Phase 1 — Foundation & Skeleton | ✅ COMPLETE (module structure, actions, stubs, binary entry point, three-panel layout all working) |
| Phase 2 — Message List & Core Display | ✅ COMPLETE (UniformList, message receiving, row selection; keyboard navigation working via track_focus + key_context("MessageList"); page up/down keys functional; auto-scroll on arrow key nav; shared FilterEngine integrated; ColumnConfig with toggle panel; column headers clickable to open config popup) |
| Phase 3 — Filter Widgets & Dockable LHS | 🚧 IN PROGRESS (FilterPanel with checkbox-based flat list using incremental BTreeSet state tracking; not yet dockable; filters wired to MessageList) |
| Phase 4 — Detail Panel & RHS Dock | 🚧 IN PROGRESS (DetailPanel content rendering implemented: title, PGN/Source/Dest, Outputs, Updates; hex dump + device info pending; not yet dockable) |
| Phase 5 — Status Bar, Modals & Polish | ⏳ PENDING |
| Phase 6 — Docking UX & Persistence | ⏳ PENDING |

### Critical Issues (2026-08-18)

1. **Keyboard bindings** ✅ FIXED: Added `.track_focus(&self.focus_handle)` to root div. Actions moved to MessageList div with `.key_context("MessageList")` so they fire when focus is inside list items. Page keys use correct GPUI names `"pageup"`/`"pagedown"`.
2. **Auto-scroll on navigation** ✅ FIXED: Added `UniformListScrollHandle` to MessageList. Arrow key navigation now auto-scrolls to keep selected item visible in viewport via `ensure_selected_visible()`.
3. ~~**FilterEngine code duplication**~~ ✅ RESOLVED: MessageList uses shared FilterEngine from filter_engine.rs
4. ~~**Hardcoded columns**~~ ✅ FIXED: ColumnConfig with ColumnTogglePanel popup (Ctrl+Shift+C); clickable column headers at top of message list open config popup
5. **Column config screen layout** ✅ FIXED (2026-08-18): Rewrote `column_toggle_panel.rs` from scratch — title bar ("Configure Columns" + close button) at the very top, each option on its own horizontal row (checkbox left, label right, width hint), footer with key hints. Root cause of the previous vertical-stacking bug: in this GPUI version `.flex_row()`/`.flex_col()` only set `flex_direction` and do NOT set `display:flex` — an explicit `.flex()` is required or children stack vertically (default display is Block).
6. **Keybinding mismatches** ⚠️ OPEN: `keybindings.rs` binds `ctrl+c` → Quit and `ctrl+l` → ClearMessages, but the top bar says "Press Ctrl+Q to quit" with no ctrl+q binding; original intent was ctrl+c=ClearMessages, ctrl+l=ToggleLayout.

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

### Phase 2: Message List & Core Display (Weeks 3-4) ✅ COMPLETE

**Goal**: Display filtered messages in the main area with keyboard navigation.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Implement `MessageList` view | GPUI UniformList rendering `DecodedMessage` rows | Phase 1 | ✅ COMPLETE — MessageList struct with FilterEngine, selected_index, global_start_time, ColumnConfig. Uses `make_uniform_list()` wrapper for GPUI's uniform_list function. (~300 lines) |
| Wire FilterEngine to list | Read messages from shared FilterEngine entity | Phase 2 | ✅ COMPLETE — Tokio spawn in MainView receives from pipeline channel and calls `message_list.update(cx, ...)` to add messages. Auto-scrolls to end on new messages. |
| Implement row rendering | Styled text runs per column (using shared `columns.rs`) | Phase 2 | ✅ COMPLETE — `render_row()` formats columns using shared ColumnConfig. Column headers rendered above UniformList with clickable labels that dispatch ToggleColumns action via window.dispatch_action(). |
| Add scroll handling | `UniformListScrollHandle` + keyboard navigation (↑/↓) | Phase 2 | ✅ COMPLETE — `scroll_to_end()` method on MessageList. Keyboard nav methods: `select_prev()`, `select_next()`, `toggle_selection()`. |
| Implement selection | Click/Enter to select a row, highlight selected row | Phase 2 | ✅ COMPLETE — `selected_index: Option<usize>` tracks current selection. Selected rows get blue bg (0x1a3a5f) with white text. `selected_message()` accessor. |
| Add keymap for list navigation | Up/down arrows, page up/down, enter, space | Phase 2 | ✅ COMPLETE — Actions defined in `keybindings.rs`, `.on_action()` handlers on MessageList div with `key_context("MessageList")`. Keyboard bindings functional via track_focus on root div. |

**Deliverables**: Functional message list with UniformList virtualization, message receiving from pipeline, row selection with highlighting, keyboard navigation (arrow keys + page up/down), clickable column headers that dispatch ToggleColumns action to open ColumnTogglePanel popup for column configuration. Popup uses .occlude() and stop_propagation() for proper click event handling (RGitUI pattern).

### Phase 3: Filter Widgets & Dockable LHS (Weeks 5-6) 🚧 IN PROGRESS

**Goal**: Implement filter widgets in a dockable LHS panel.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Create `FilterWidget` entity model | One entity per filter type with state management | Phase 2 | ✅ COMPLETE — FilterPanel struct with BTreeSet-based incremental state tracking (source_addrs, dest_addrs, pgns, titles, source_names, dest_names). Methods: add_message(), rebuild_items(), update_from_messages(). (~130 lines) |
| Implement filter widget views | Checkbox-based flat list with section headers | Phase 3 | ✅ COMPLETE — FilterPanel renders as flat list with section headers (SourceAddr, DestAddr, PGN, Title, SourceName, DestName). Each option has checkbox indicator. Click-to-expand for NAME details. Incremental state updates via BTreeSet insert per message instead of rescanning all messages. (~200 lines) |
| Build LHS dockable panel | ManagedView implementing dock system | Phase 3 | ⏳ PENDING — FilterPanel exists as simple div container (w_72), not yet using GPUI dock system. |
| Wire filters to FilterEngine | Enable/disable filters, update input text → re-evaluate | Phase 3 | ❌ NOT DONE — GPUI has its own `MessageFilter` struct in `message_list.rs` with inline filter evaluation (`matches()` method). Does NOT use shared `filter_engine.rs::FilterEngine`. This duplicates message storage, filtering logic, and unique value tracking. |
| Implement filter editor modal | GPUI anchored popup for complex filter editing (Numeric, Flag) | Phase 3 | ⏳ PENDING |
| Add F1 keybinding | Toggle first filter widget focus/enable | Phase 3 | ❌ NOT WORKING — Same context/keymap issue as keyboard navigation. |

**Deliverables**: FilterPanel with checkbox-based flat list using incremental BTreeSet state tracking (O(1) per-message updates instead of O(n²) rescans). Dockable panel wiring pending. **Critical**: Needs to use shared `filter_engine.rs` instead of duplicating logic in `MessageFilter`.

### Phase 4: Detail Panel & RHS Dock (Weeks 7-8) 🚧 IN PROGRESS

**Goal**: Implement detail view in a dockable RHS panel.

| Task | Details | Status |
|------|---------|--------|
| Create `DetailPanel` view | Shows decoded message details for selected row | ✅ DONE — title, PGN/Source/Dest rows, Outputs (severity-colored), Updates sections rendered via `render_detail()` in renderer.rs |
| Wire selection to detail view | Clicking a row updates detail panel content | ✅ DONE — MainView copies `selected_message` into DetailPanel on every render (note: entity is recreated each render; should be stored as a field) |
| Implement hex dump display | Monospace text with color-coded bytes | ⏳ PENDING |
| Show device info from DeviceManager | Source/dest addresses, NAME resolution | 🚧 PARTIAL — raw src/dst addresses shown; NAME resolution pending |
| Build RHS dockable panel | ManagedView implementing dock system | ⏳ PENDING |

**Deliverables**: Dockable RHS with message details, feature parity with TUI.

### Phase 5: Status Bar, Modals & Polish (Weeks 9-10) ⏳ PENDING

**Goal**: Complete remaining UI elements and polish the experience.

| Task | Details | Dependencies | Status |
|------|---------|--------------|--------|
| Implement status bars | Top bar (mode indicator) + bottom bar (status info) | Phase 4 | ⏳ PENDING |
| Implement column config modal | Popup for toggling columns / adjusting widths | Phase 4 | 🚧 PARTIAL — toggle popup done (Ctrl+Shift+C / gear button); width adjustment still pending |
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

## Future Work & Refactoring Priorities (2026-08-12)

### 1. Incorporate FilterEngine to Eliminate Code Duplication ✅ COMPLETE

**Status**: Implemented in this session. MessageList now uses shared `FilterEngine` from `filter_engine.rs`. The `engine` field stores all messages and handles filtering via dirty-flag optimization. FilterPanel maintains its own BTreeSet state for UI checkbox display (GPUI constraint — can't access cx during render).

**Files modified**:
- `src/gpui/components/message_list.rs` — Uses shared `FilterEngine` instead of inline message storage + filter logic
- `src/gpui/components/filter_panel.rs` — FilterPanel's BTreeSets serve UI display only; actual filtering runs through MessageList's FilterEngine

### 2. Add Column Selection Using Shared columns.rs ✅ COMPLETE

**Status**: Implemented. Replaced DEFAULT_COLUMNS constant with `ColumnConfig` from shared `columns.rs`. Added `ColumnTogglePanel` popup (Ctrl+Shift+C or ⚙ button) for toggling column visibility; clickable column headers above the message list also open it. Layout was redone 2026-08-18: title bar at top, one horizontal row per option (checkbox left, label right, width hint), footer key hints — see Critical Issue #5 for the `.flex()` gotcha that caused the earlier vertical-stacking bug.

**Files modified**:
- `src/gpui/components/message_list.rs` — Uses shared `ColumnConfig`, clickable headers dispatch ToggleColumns
- `src/gpui/components/column_toggle_panel.rs` — Column config popup (rewritten 2026-08-18)
- `src/gpui/renderer.rs` — Popup entity + modal overlay, gear button, Escape/Ctrl+Shift+C handling

**Remaining**: column width configuration (popup shows read-only `w=NN` hints only).

### 3. Fix Keyboard Bindings ✅ COMPLETE

**Status**: Fixed in this session. Added `.track_focus(&self.focus_handle)` to root div in `MainView::render()`. Actions (ScrollUp, ScrollDown, SelectRow) moved from MainView to MessageList div where `.key_context("MessageList")` is set, so they fire when focus is inside list items. Page keys use correct GPUI names `"pageup"`/`"pagedown"`.

### 4. Auto-Scroll on Navigation ✅ COMPLETE

**Status**: Implemented in this session. Added `UniformListScrollHandle` to MessageList struct. Arrow key navigation (`select_prev`/`select_next`) now calls `ensure_selected_visible()` which uses `scroll_to_item()` to keep the selected item visible in viewport. Page up/down also scrolls to selected item after jumping.

### 5. Implement Detail Panel Content 🚧 IN PROGRESS

**Current State** (as of 2026-08-18):
- `DetailPanel` renders the selected message: title, PGN/Source/Dest rows, Outputs (severity-colored) and Updates sections via `render_detail()` in renderer.rs
- MainView copies `selected_message` from MessageList into DetailPanel on every render — works, but the entity is recreated with `cx.new()` each render; should be stored as a field

**Remaining**:
- Implement hex dump view with color-coded bytes
- Show device info from DeviceManager (NAME resolution)

### 6. Implement Dockable Panel System

**Current State**:
- FilterPanel and DetailPanel are simple div containers with fixed widths (w_72, w_80)
- No drag-to-reposition, no detach-as-floating-window capability
- Layout is static flexbox: LHS/Main/RHS side-by-side only

**Proposed Solution**:
- Implement `ManagedView` trait for FilterPanel and DetailPanel
- Use GPUI's dock system for drag-to-reposition and floating windows
- Add layout toggle (Ctrl+L) for horizontal/vertical switching

**Priority**: MEDIUM — Nice-to-have, doesn't block core functionality.

### 7. Fix Click Event Propagation in ColumnTogglePanel ✅ COMPLETE

**Status**: Implemented in this session. Added `.occlude()` to root div of ColumnTogglePanel, `stop_propagation()` on mouse_down and mouse_move events. Toggle rows use entity.update() for column toggling via .on_mouse_down(). Close button dispatches CloseColumns action via window.dispatch_action(). Popup renders as sibling of message_list in MainView using RGitUI pattern (DOM order hit testing).

**Files modified**:
- `src/gpui/components/column_toggle_panel.rs` — Added .occlude(), stop_propagation() handlers, entity.update() for toggle rows
- `src/gpui/components/message_list.rs` — Header click handler uses window.dispatch_action(ToggleColumns) instead of parent_entity callback; removed unused parent_entity field

**Impact**:
- Popup now properly blocks underlying elements from receiving mouse events (via .occlude())
- Mouse events no longer leak through popup to message list rows below
- Column toggle works reliably via entity.update() pattern (RGitUI style)
- Header click dispatches action through GPUI's keymap system instead of direct parent callback

### 8. Remove Dead Code in filter_widget.rs ⚠️ NEW

**Status**: `src/gpui/components/filter_widget.rs` (263 lines) is declared in `components/mod.rs` but referenced nowhere — leftover from the 2026-08-11 dock-panel attempt. Contains debug `eprintln!` statements and a standalone Focusable text-input widget that predates FilterPanel's checkbox design.

**Proposed Solution**: Delete the file (and its mod declaration), or repurpose it as the input widget for future filter-value editing if that feature is wanted.

### 9. Fix Keybinding Mismatches ⚠️ NEW

**Status**: `keybindings.rs` binds `ctrl+c` → Quit and `ctrl+l` → ClearMessages, but the top bar text says "Press Ctrl+Q to quit" with no ctrl+q binding; original intent was ctrl+c=ClearMessages, ctrl+l=ToggleLayout. Also `ClearMessages` action handler in renderer.rs is a TODO stub (just calls cx.notify()).

**Proposed Solution**: Decide the intended bindings, update `configure_keybindings()` and the top bar text to match, and implement ClearMessages (clear MessageList + FilterPanel state).

### GPUI-Specific Modules (Current State — 2026-08-18)

```
src/gpui/
├── mod.rs                          — ✅ Module organization, exports (10 lines)
├── app_state.rs                    — 🚧 AppState entity stub + Clone derive (21 lines)
├── renderer.rs                     — ✅ MainView + DetailPanel with content rendering (~509 lines)
│                                   — MainView: three-panel flexbox layout, message receiving via cx.spawn()
│                                   — Gear button (⚙) top-right of message list toggles column config popup
│                                   — Modal overlay when popup open: dims background, closes on outside click / Escape
│                                   — DetailPanel: title, PGN/Source/Dest rows, Outputs (severity-colored), Updates
│                                   — Action handlers: ScrollUp/Down, PageUp/Down, SelectRow, ClearMessages (TODO stub)
├── keybindings.rs                  — ✅ 15 actions in three actions! blocks + bind_keys() (53 lines)
│                                   — ⚠️ ctrl+c→Quit and ctrl+l→ClearMessages mismatch with top bar text (see Future Work #9)
│
└── components/
    ├── mod.rs                      — ✅ Component module re-exports (9 lines)
    ├── message_list.rs             — ✅ MessageList with UniformList (~475 lines)
    │                               — ✅ Uses shared FilterEngine from filter_engine.rs
    │                               — ✅ ColumnConfig from columns.rs with toggle panel support
    │                               — ✅ Row rendering with selection highlighting
    │                               — ✅ Clickable column headers dispatch ToggleColumns action via window.dispatch_action()
    │                               — ✅ Keyboard nav: arrow keys + page up/down + auto-scroll via UniformListScrollHandle
    │                               — ✅ Actions wired on MessageList div (key_context "MessageList")
    ├── filter_panel.rs             — 🚧 FilterPanel with checkbox-based flat list (~465 lines)
    │                               — ✅ Incremental BTreeSet state tracking (add_message per message)
    │                               — ✅ Section headers: SourceAddr, DestAddr, PGN, Title, SourceName, DestName
    │                               — ✅ Filters propagate to MessageList via apply_to_message_list()
    │                               — ⚠️ Not yet dockable; row_height() helper exists but UniformList not wired up
    ├── filter_widget.rs            — ⚠️ DEAD CODE (263 lines) — declared in mod.rs, referenced nowhere;
    │                               — leftover from 2026-08-11 dock-panel attempt, has debug eprintln! statements
    ├── status_bar.rs               — 🚧 StatusBar stub (37 lines)
    └── column_toggle_panel.rs      — ✅ Column config popup panel (308 lines, rewritten 2026-08-18)
                                — ✅ Title bar ("Configure Columns" + close button) at very top
                                — ✅ Each option on its own horizontal row: checkbox left, label right, width hint
                                — ✅ Footer with key hints ([↑↓] Navigate [Space] Toggle)
                                — ⚠️ GPUI gotcha: .flex_row()/.flex_col() do NOT set display:flex in this version;
                                   explicit .flex() required or children stack vertically (see Critical Issue #5)
```

**Total GPUI code**: ~2,280 lines across 11 files. Build succeeds with only unused struct warnings. All tests pass.

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
│   /* GPUI-SPECIFIC - new module */
├── src/gpui/
│   ├── mod.rs                          — ✅ Module organization, exports (10 lines)
│   ├── app_state.rs                    — 🚧 AppState entity stub + Clone derive (21 lines)
│   ├── renderer.rs                     — ✅ MainView + DetailPanel with content rendering (~509 lines)
│   │                                   — Three-panel flexbox layout, gear button, modal overlay for column popup
│   │                                   — Message receiving via cx.spawn() + Tokio task
│   ├── keybindings.rs                  — ✅ 15 actions using gpui::actions! macro + bind_keys() (53 lines)
│   │
│   └── components/
│       ├── mod.rs                      — ✅ Component module re-exports (9 lines)
│       ├── message_list.rs             — ✅ MessageList with UniformList rendering (~475 lines)
│       │                               — uniform_list wrapper, row rendering, selection state, clickable headers
│       ├── filter_panel.rs             — 🚧 FilterPanel checkbox-based flat list (~465 lines)
│       ├── filter_widget.rs            — ⚠️ Dead code (263 lines), unreferenced
│       ├── status_bar.rs               — 🚧 StatusBar stub (37 lines)
│       └── column_toggle_panel.rs      — ✅ Column config popup, rewritten 2026-08-18 (308 lines)
```

**Total GPUI code**: ~2,280 lines across 11 files (including main_gpui.rs). Build succeeds with only unused struct warnings. Window opens with three-panel dark-themed layout and processes candump frames successfully. All existing tests pass.

### Cargo.toml Configuration

```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed.git", rev = "f3fb4e04aa85dbbde6e83d28f231fc452cd8863f" }
gpui_platform = { git = "https://github.com/zed-industries/zed.git", rev = "f3fb4e04aa85dbbde6e83d28f231fc452cd8863f", features = ["font-kit", "x11", "wayland"] }

[[bin]]
name = "can_decoder_gpui"
path = "src/main_gpui.rs"
```

**Note**: The `gpui` dependency was migrated from crates.io `"0.2"` to the Zed mainline git rev (same as rgitUI) on 2026-08-11 — see GPUI Version Evaluation section for the full analysis and breaking changes that had to be fixed.

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

## Current Implementation Status (2026-08-18)

### Completed (Phase 1 - Foundation)

| Component | File | Lines | Status |
|-----------|------|-------|--------|
| Module structure | `src/gpui/mod.rs` | 10 | ✅ Complete — re-exports key GPUI types, declares submodules |
| Action definitions | `src/gpui/keybindings.rs` | 53 | ✅ 15 actions defined and functional (track_focus added to root div). ⚠️ ctrl+c/ctrl+l bindings mismatch top bar text. |
| Component module | `src/gpui/components/mod.rs` | 9 | ✅ Complete — re-exports component submodules incl. column_toggle_panel |
| AppState stub | `src/gpui/app_state.rs` | 21 | 🚧 Stub — skeleton struct with Clone derive, TODO: pipeline/device_manager/filter_engine init |
| MainView Render impl | `src/gpui/renderer.rs` | ~509 | ✅ Complete — three-panel flexbox layout + message receiving via cx.spawn() + keyboard bindings working + gear button + modal overlay for column popup. DetailPanel content rendering implemented (title, PGN/Source/Dest, Outputs, Updates). ⚠️ DetailPanel entity recreated each render; ClearMessages handler is a TODO stub. |
| FilterPanel | `src/gpui/components/filter_panel.rs` | ~465 | ✅ Complete — Checkbox-based flat list with incremental BTreeSet state tracking, section headers, click-to-expand for NAME details. Filters propagate to MessageList via apply_to_message_list(). Not yet dockable; row_height() helper exists but UniformList not wired up. |
| MessageList view | `src/gpui/components/message_list.rs` | ~475 | ✅ Uses shared FilterEngine from filter_engine.rs (no duplication). ColumnConfig from columns.rs with toggle panel support. UniformList rendering, selection state, scroll handling. Clickable column headers above list dispatch ToggleColumns action via window.dispatch_action(). Keyboard nav: arrow keys + page up/down + auto-scroll to selected item via UniformListScrollHandle. Actions wired on MessageList div with key_context("MessageList"). |
| StatusBar stub | `src/gpui/components/status_bar.rs` | 37 | 🚧 Stub — TODO: mode/device count display |
| Binary entry point | `src/main_gpui.rs` | ~130 | ✅ Complete — full pipeline wiring, Tokio runtime, GPUI window with MessageList entity. |
| ColumnTogglePanel | `src/gpui/components/column_toggle_panel.rs` | 308 | ✅ Rewritten 2026-08-18: title bar at top ("Configure Columns" + close button), each option on its own horizontal row (checkbox left, label right, width hint), footer key hints. Uses shared ColumnConfig; .flex() required alongside .flex_row() in this GPUI version or rows stack vertically. |
| FilterWidget (dead) | `src/gpui/components/filter_widget.rs` | 263 | ⚠️ Dead code — declared in mod.rs but referenced nowhere; leftover from 2026-08-11 dock-panel attempt with debug eprintln! statements. Candidate for deletion. |

**Total GPUI code**: ~2,280 lines across 11 files (including main_gpui.rs).  
**Build status**: `cargo build --bin can_decoder_gpui` succeeds with only unused struct warnings. All tests pass.

### Critical Issues Status (2026-08-18)

1. **Keyboard bindings** ✅ FIXED: Added `.track_focus(&self.focus_handle)` to root div in MainView render (`renderer.rs`). GPUI's keymap system now properly dispatches ScrollUp, ScrollDown, SelectRow actions.
2. **FilterEngine code duplication** ✅ RESOLVED: MessageList now uses shared `FilterEngine` from `filter_engine.rs` for message storage and filtering. FilterPanel maintains its own BTreeSet state for UI checkbox display (GPUI constraint — can't access cx during render). Actual filter evaluation runs through shared FilterEngine.
3. **Hardcoded columns** ✅ FIXED: Replaced DEFAULT_COLUMNS constant with `ColumnConfig` from shared `columns.rs`. ColumnTogglePanel popup allows users to toggle column visibility. Clickable column headers at top of message list open config popup directly via window.dispatch_action(ToggleColumns).
4. **Click event propagation in ColumnTogglePanel** ✅ FIXED: Added `.occlude()` to root div, `stop_propagation()` on mouse events (both mouse_down and mouse_move), toggle rows use entity.update() for column toggling. Close button dispatches CloseColumns action via window.dispatch_action(). Popup renders as sibling of message_list in MainView using RGitUI pattern (DOM order hit testing).
5. **Column config screen layout** ✅ FIXED (2026-08-18): Rewrote `column_toggle_panel.rs` from scratch — title bar at top, one horizontal row per option (checkbox left, label right), footer key hints. Root cause: `.flex_row()`/`.flex_col()` do not set `display:flex` in this GPUI version; explicit `.flex()` required or children stack vertically.
6. **Keybinding mismatches** ⚠️ OPEN: ctrl+c→Quit and ctrl+l→ClearMessages bound, but top bar says "Press Ctrl+Q to quit" (no such binding); original intent was ctrl+c=ClearMessages, ctrl+l=ToggleLayout. ClearMessages handler is a TODO stub.

### Next Steps (Priority Order)

1. ~~**Fix keyboard bindings**~~ ✅ Done — track_focus + key_context("MessageList") on MessageList div
2. ~~**Incorporate FilterEngine**~~ ✅ Done — MessageList uses shared FilterEngine
3. ~~**Add column selection**~~ ✅ Done — ColumnConfig + ColumnTogglePanel implemented
4. ~~**Page keys + auto-scroll**~~ ✅ Done — pageup/pagedown key names fixed, UniformListScrollHandle with ensure_selected_visible()
5. ~~**Clickable column headers**~~ ✅ Done — Column headers rendered above UniformList, clicking any header opens ColumnTogglePanel popup via window.dispatch_action(ToggleColumns)
6. ~~**Fix click event propagation in ColumnTogglePanel**~~ ✅ Done — Added .occlude() to root div, stop_propagation() on mouse events, toggle rows use entity.update() for column toggling. Close button dispatches CloseColumns action. Popup renders as sibling of message_list in MainView (RGitUI pattern).
7. ~~**Implement `DetailPanel` content**~~ ✅ PARTIAL — basic detail rendering done (title, PGN/Source/Dest, Outputs, Updates); hex dump + device NAME resolution still pending; store DetailPanel as a field instead of recreating each render
8. **Filter list scrolling in FilterPanel** — Use UniformList for filter options when many values exist (row_height() helper already exists)
9. **Fix keybinding mismatches** — align ctrl+c/ctrl+l bindings with top bar text and original intent; implement ClearMessages handler
10. **Remove dead code in filter_widget.rs** — delete or repurpose the unreferenced 263-line module
11. **Resize(width) of filter and detail panels** — Make panel widths adjustable
12. **Dockable panel system** — Implement ManagedView trait for LHS/RHS panels
13. **Status bar polish** — Mode indicator, device count display

### Pending Phases

| Phase | Description | Dependency |
|-------|-------------|------------|
| ~~Phase 1~~ | ~~Foundation & Skeleton~~ | — | ✅ COMPLETE |
| ~~Phase 2~~ | ~~Message List & Core Display~~ | Phase 1 completion | ✅ COMPLETE (keyboard nav working: arrow keys + page up/down + auto-scroll; FilterEngine integrated; ColumnConfig with toggle panel; clickable column headers dispatch ToggleColumns action via window.dispatch_action(); click event propagation fixed with .occlude() and stop_propagation using RGitUI pattern) |
| Phase 3 | Filter Widgets & Dockable LHS | Phase 2 | 🚧 IN PROGRESS (FilterPanel UI complete; dockable panel pending) |
| Phase 4 | Detail Panel & RHS Dock | Phase 3 | 🚧 IN PROGRESS (DetailPanel content rendering implemented: title, PGN/Source/Dest, Outputs, Updates; hex dump + device info + docking pending) |
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

## zed
- Checked out here: /home/cschuhen/rust/zed
- URL: https://github.com/zed-industries/zed.git
- The definitive reference implementation

---

## GPUI Version Evaluation (2026-08-11)

### Current State (as of 2026-08-11)
- **can_decoder** originally used `gpui = "0.2"` from crates.io — released ~10 months ago
- This is the stable pre-1.0 release, which means it will not receive further updates
- Several API limitations encountered: no built-in TextInput, UniformList `'static` closure restrictions, focus handling complexity
- ✅ **Migrated** to Option A (Zed mainline git rev) on 2026-08-11 — see GPUI Version Note at top of document for the breaking changes that had to be fixed

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
