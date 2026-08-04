# Design Document: TUI CAN Decoder (Final Refinement)

## 1. Overview
A high-performance terminal user interface (TUI) for real-time J1939 CAN message monitoring, featuring a three-pane layout with specialized navigation modes for rapid filtering and data inspection.

**Build**: `cargo build --bin can_decoder_tui`
**Run**: `./target/release/can_decoder_tui [options]`

## 2. UI Layout & Interaction

### 2.1 Main Central Viewport (Message List)
*   **Function**: Displays a condensed, scrollable list of `DecodedMessage` entries.
*   **Scrolling Modes**:
    *   **Live Stream Mode (Default)**: The view is "locked" to the bottom; as new messages arrive, the list scrolls up automatically.
    *   **Manual Scroll Mode**: Triggered by Space; allows scrolling up through message history without auto-scroll.
*   **Performance**: Virtualized rendering — only visible messages are drawn within viewport height.
*   **Navigation**:
    *   `Up` / `Down` Arrows: Incremental scroll through the message list.
    *   `Shift` + `Up` / `Down`: Page Up / Page Down jumps (3x viewport).

### 2.2 Left-Hand Side (LHS) Panel: Filter Stack
*   **Function**: A vertical stack of "Filter Widgets" (Title, PGN, Severity, Source, Dest, RPM/Speed Numeric, Engine Flag).
*   **Layout**:
    *   **Expanded**: Shows full input controls (text boxes, option lists for severity/flag).
    *   **Minimized**: Single-line summary — `* name` if enabled, `  name` if disabled.
*   **Navigation & Input**:
    *   `Tab`: Move focus from LHS to Main panel.
    *   `Shift` + `Up` / `Down`: Move focus between filter widgets in the stack.
    *   `Arrows` (without Shift): Navigate items within a widget (e.g., severity options).
    *   `Space`: Toggle enabled/disabled on active widget; toggle stream mode when focused on Main.
    *   `Enter`: Enter text input mode for text-based filters.

### 2.3 Right-Hand Side (RHS) Panel: Detail Inspector
*   **Function**: "Deep Inspection" of the message currently selected in the Main Viewport.
*   **Content**: Every `DecodedField` in full detail including Numeric Values (precision, units, raw hex), String Messages (severity color-coding), Flags (On/Off/Error/Unavailable states), Metadata (timestamp, source/dest address, DeviceName from DeviceManager), and raw data bytes.
*   **Navigation**:
    *   `Tab` (from Main): Move focus to RHS panel when LHS is hidden; otherwise cycles LHS → Main → RHS.

### 2.4 Status Bar & Overlays
*   **Status Bar**: Fixed bar at bottom showing: focus panel, input mode, stream mode, active filter count, message count, connection status.
*   **Error Log Popup**: F3 overlay showing recent filtered-out messages and warnings (max 100 entries).

## 3. Keyboard Navigation

| Key Combination | Action |
| :--- | :--- |
| **LHS Panel** | |
| `Shift` + `Up` / `Down` | Move focus between filter widgets |
| `Arrows` (no Shift) | Navigate items within a widget |
| `Space` | Toggle enabled/disabled on active widget |
| `Enter` | Enter text input mode |
| **Main Viewport** | |
| `Up` / `Down` | Incremental scroll |
| `Shift` + `Up` / `Down` | Page Up / Page Down (3x viewport) |
| `Space` | Toggle Live Stream / Manual Scroll |
| **Global Panel Cycling** | |
| `Tab` | Cycle focus: LHS → Main → RHS → LHS (loops; skips hidden panels) |
| `Shift` + `Tab` | Reverse cycle: RHS → Main → LHS → RHS (loops; skips hidden panels) |
| **Panel Visibility** | |
| `F1` | Toggle LHS Panel visibility |
| `F2` | Toggle RHS Panel visibility |
| **Overlays & Exit** | |
| `F3` | Open/Close Error Log Popup |
| `Esc` | Close popup / exit text mode / return to Main focus |
| `Ctrl+C` | Graceful shutdown (SIGINT handler) |
| `q` | Quit TUI |

## 4. Layout Orientation

### 4.1 Horizontal Layout (Default)
```
┌──────────┬──────────────────────┬──────────────────┐
│ Filters  │   Messages           │  Details         │
│ (LHS)    │   (Main)             │  (RHS)           │
│          │                      │                  │
├──────────┴──────────────────────┴──────────────────┤
│ Status Bar                                          │
└─────────────────────────────────────────────────────┘
```

### 4.2 Vertical Layout (`--layout vertical`)
```
┌─────────────────────────────────────────────────────┐
│ Filters (LHS)                                       │
│ ┌──────────┬──────────┬──────────────────┐          │
│ │ Messages │ Details  │                  │          │
│ │ (Main)   │ (RHS)    │                  │          │
│ ├──────────┴──────────┴──────────────────┤          │
│ │ Status Bar                              │          │
│ └─────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────┘
```

### 4.3 Toggle at Runtime
*   `F4`: Toggle between horizontal and vertical layout without restarting.
*   In vertical mode, panel cycling (`Tab`/`Shift+Tab`) cycles: Top → Bottom-Left → Bottom-Right → Top.

## 5. Technical Implementation Details

### 5.1 Shared CLI Configuration Module
A new `config.rs` module provides shared types and argument parsing used by **both** the CLI app (`main.rs`) and TUI app (`tui_main.rs`). This eliminates duplication and ensures consistency.

```rust
// src/config.rs — shared between CLI and TUI

#[derive(Parser, Debug, Clone)]
pub struct SharedConfig {
    /// SocketCAN interface for live mode (e.g., can0)
    #[arg(short, long)]
    pub interface: Option<String>,

    /// Input source type (socketcan, candump)
    #[arg(short, long, default_value = "socketcan")]
    pub source: SourceType,

    /// Path to candump-style input file (used with --source candump)
    #[arg(long)]
    pub input_file: Option<PathBuf>,

    /// Path to YAML configuration file defining PGN interpretations
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Output detail level (raw frame bytes vs assembled/decoded output)
    #[arg(long, default_value = "assembled")]
    pub detail_level: DetailLevel,

    /// Emit partially reassembled Transport Protocol messages on timeout
    #[arg(long)]
    pub force_output_partial_tp: bool,

    /// Enable extra debugging information
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// Use proprietary DDI definitions by name (repeatable)
    #[arg(long)]
    pub use_proprietary_ddi_definitions: Vec<String>,
}

// TUI-specific CLI args (extends SharedConfig)
#[derive(Parser, Debug, Clone)]
pub struct TuiCli {
    #[command(flatten)]
    pub shared: SharedConfig,

    /// Layout orientation: horizontal | vertical
    #[arg(long, default_value = "horizontal")]
    pub layout: LayoutOrientation,
}
```

**SourceType**, **DetailLevel**, and **OutputFormat** enums move from `lib.rs` to `config.rs`. The CLI app (`main.rs`) uses `SharedConfig`; the TUI app uses `TuiCli`. Both construct a `J1939Decoder` with identical parameters.

### 5.2 Pipeline Construction Helper
A shared function in `config.rs` or `lib.rs` builds the full pipeline (Source → Decoder → Filter → Renderer) from config, used by both CLI and TUI:

```rust
pub fn build_pipeline(
    config: &SharedConfig,
    proprietary_defs: &[String],
) -> Result<(Pipeline, Arc<tokio::sync::Mutex<dyn Filter>>), anyhow::Error>
```

The TUI replaces the final Renderer stage with its own `mpsc` channel — messages flow through Decoder → Filter → channel → TUI event loop.

### 5.3 TUI Component Architecture
*   **Widget-Based LHS**: The LHS is a `List<FilterWidget>` where each widget manages its own internal state (enabled, input_text, cursor_pos, expanded).
*   **Focus Management**: A global `FocusManager` tracks the current active window and interaction mode (Navigation vs. Text Input). Panel cycling via `Tab`/`Shift+Tab` loops: LHS → Main → RHS → LHS, skipping hidden panels.
*   **Data Buffering**: Messages stored in an unbounded `VecDeque<DecodedMessage>` with configurable max size (default 10,000). Passed to the UI via `mpsc` channel from pipeline.

### 5.4 Data Flow
1.  **Input**: `RawFrame` → `TpReassembler` → `PgnDecoder` → `DecodedMessage`.
2.  **Filtering**: The `DecodedMessage` is passed through the `Filter` logic defined by active LHS widgets (sync-evaluated via a shared tokio runtime).
3.  **Display**: Filtered messages pushed into the `Main Viewport` buffer for rendering.

### 5.5 Signal Handling
*   `Ctrl+C` (SIGINT): Caught via `tokio::signal::ctrl_c()`. Sets a shutdown flag, drains remaining messages, restores terminal state (raw mode off, alternate screen off, cursor visible), then exits cleanly.
*   `q` key: Immediate quit without draining — same cleanup sequence.

## 6. Implementation Plan

### Phase 1: TUI Skeleton & Layout ✅ COMPLETE
*   [x] **Add Dependencies**: Added `ratatui` and `crossterm` to `Cargo.toml`.
*   [x] **Define App State**: Created `TuiApp` struct with focus, panel visibility, scroll offset.
*   [x] **Basic Layout**: Three-column horizontal layout (LHS | Main | RHS) + status bar using `ratatui::Layout`.
*   [x] **Event Loop**: Basic crossterm event loop handling terminal input and UI rendering at ~60fps.

### Phase 2: Data Plumbing & Main Message List ✅ COMPLETE
*   [x] **Async/Sync Bridge**: `mpsc` channel in `tui_main.rs` receives `DecodedMessage` from pipeline to TUI event loop.
*   [x] **TuiRenderer Implementation**: Full renderer with all three panels + status bar via `ratatui`.
*   [x] **Main Viewport Widget**: Virtualized list rendering only visible messages within viewport height.
*   [x] **Scrolling Logic**: "Live Stream" (auto-scroll to bottom) and "Manual Scroll" modes toggled with Space.
*   [x] **Main Navigation**: `Up`/`Down` (incremental), `Shift`+`Up`/`Down` (page jump), `PageUp`/`PageDown`.

### Phase 3: Detail Inspector (RHS) ✅ COMPLETE
*   [x] **Detail Widget**: Displays all `DecodedField`s with Value (precision, units, raw hex), StringMessage (severity colors), Flag (On/Off/Error/Unavailable).
*   [x] **Metadata Integration**: Timestamp, PGN, CAN ID, source/dest addresses, DeviceName from `DeviceManager`, raw data bytes.
*   [x] **RHS Navigation**: Panel cycling via `Tab`/`Shift+Tab`.

### Phase 4: Filter Stack (LHS) ✅ COMPLETE
*   [x] **Filter Widget Stack**: Vertical stack of 8 filter widgets (Title, PGN, Severity, Source, Dest, RPM/Speed Numeric, Engine Flag).
*   [x] **Widget States**: Expanded view shows input controls and options; Minimized shows `* name` or `  name`.
*   [x] **Input Modes**: "Navigation Mode" (Space toggles enabled) and "Text Input Mode".
*   [x] **Filter Engine Integration**: Each widget builds a `Box<dyn Filter>` via `build_filter()`; `passes_all_filters()` sync-evaluates all active filters.

### Phase 5: Polish & Extras ✅ COMPLETE
*   [x] **Status Bar**: Real-time display of focus panel, input mode, stream mode, active filter count, message count.
*   [x] **Error Log Popup**: F3 overlay showing recent filtered-out messages and warnings (max 100 entries).
*   [x] **Focus Management**: Keyboard handling: arrows, Space, Enter, Esc, h/l for panel switching, q to quit.
*   [x] **Styling**: Colorization across all panels — Cyan borders for active focus, Yellow highlights, Green/Red/Yellow/Magenta/Gray for field values and severities.

### Phase 6: Navigation & Layout Refinements ✅ COMPLETE
*   [x] **Tab/Shift+Tab Panel Cycling**: Replaced `Shift`+`Left`/`Right` with `Tab` (forward) and `Shift`+`Tab` (reverse). Loops on overflow/underflow. Skips hidden panels. Added `Tab` and `ShiftTab` variants to `TuiKey` enum.
*   [x] **Ctrl+C Exit Handler**: Added `tokio::signal::ctrl_c()` in `tui_main.rs`. On SIGINT: sets shutdown flag, restores terminal state (disable raw mode, leave alternate screen, show cursor), then exits cleanly.
*   [x] **Vertical Layout Support**: Added `--layout horizontal|vertical` CLI option. In vertical mode: split content area into top (LHS full-width) and bottom row (Main | RHS side-by-side). Toggle at runtime with F4 key.

### Phase 7: Shared CLI Configuration ✅ COMPLETE
*   [x] **Create `src/config.rs`**: Defined `SharedConfig` struct with all common CLI args (`--interface`, `--source`, `--input-file`, `--config`, `--detail-level`, `--force-output-partial-tp`, `--debug`, `--use-proprietary-ddi-definitions`).
*   [x] **Define `TuiCli`**: Extends `SharedConfig` with TUI-specific args (`--layout horizontal|vertical`).
*   [x] **Move Enums to `config.rs`**: Moved `SourceType`, `DetailLevel`, `OutputFormat` from `lib.rs` to shared module. Re-exported from `lib.rs`. Updated imports in `lib.rs`, `main.rs`.
*   [x] **Refactor CLI `main.rs`**: Uses `Cli` struct that flattens `SharedConfig` and adds CLI-only args (`--filter`, `--output-format`).
*   [x] **Refactor TUI `tui_main.rs`**: Parses `TuiCli`, extracts `SharedConfig`, builds source (SocketCanSource or CandumpFileSource), creates J1939Decoder with matching parameters, wires into mpsc channel for TUI consumption.

### Phase 8: Reliability & UX Polish ✅ IN PROGRESS
*   [x] **Ctrl+C Exit Handler**: Added raw Unix signal handler (`libc::sigaction`) alongside tokio signal for reliable Ctrl+C exit on Linux. Fixed Ctrl+C key detection in crossterm event loop — now checks `Char('c')` with CONTROL modifier instead of unreliable ASCII control codes.
*   [x] **Shift+Tab Navigation**: Fixed Shift+Tab to use `KeyCode::BackTab` from crossterm (not Tab + SHIFT modifier). Now properly cycles focus in reverse order.
*   [x] **Filter Widget Editability**: Added visual "[Enter to edit]" hint for disabled widgets when selected. When Space is pressed on a disabled widget, it enables AND enters text input mode immediately. Up/Down arrows now navigate between filter widgets in LHS panel and move cursor within text fields during TextInput mode.
*   [x] **Horizontal Layout Panel Sizing**: Replaced fixed `Constraint::Length` with ratio-based constraints so main panel gets proportional space. Prevents narrow terminal from squeezing panels into vertical-looking layout.
*   [x] **Scroll Overflow Fix**: Fixed subtraction overflow in `scroll_down()` when `new_selected < viewport_height`. Now uses `saturating_sub()` for safe arithmetic.

### Bug Fixes & Improvements ✅ COMPLETE
 *   [x] **Ctrl+C Exit Handler**: Added `TuiKey::CtrlC` variant and crossterm event handling for Ctrl+U (0x00) to reliably exit. Both `q` and Ctrl+C now work in both `tui_main.rs` and `app.rs`.
 *   [x] **Tab Panel Cycling**: Rewrote `cycle_focus_forward()`/`cycle_focus_reverse()` using `get_visible_panel_order()` helper. Now properly cycles Lhs → Main → Rhs → Lhs (and reverse), including hidden panels.
 *   [x] **Message List Scrolling**: Fixed `scroll_up()`/`scroll_down()` to work in live mode when user manually scrolls away from bottom. Up arrow switches from LIVE to MANUAL mode when scrolling up from the latest message. PageUp/PageDown now check focus before scrolling.
 *   [x] **Filter Widgets**: Replaced invalid hardcoded filters (RPM, Speed, Engine) with NAME-based filters: "Src Name" and "Dst Name" (Numeric type). Renamed "Source"/"Dest" to "Source Addr"/"Dest Addr" for clarity.
 *   [x] **F4 Layout Toggle**: F4 now correctly toggles between horizontal and vertical layouts. Status bar shows current layout mode (HORZ/VERT). Added full key hints in status bar: `[F1:LHS] [F2:RHS] [F3:Log] [F4:Layout]`.
 *   [x] **Vertical Layout Status Bar**: Fixed status bar rendering in vertical mode — was incorrectly splitting from bottom row instead of content area, causing it to steal space from Main/RHS panels.
 *   [x] **Ctrl+C Reliability (Phase 8)**: Added raw Unix signal handler (`libc::sigaction`) alongside tokio signal for reliable Ctrl+C exit on Linux. Fixed Ctrl+C key detection in crossterm event loop — now checks `Char('c')` with CONTROL modifier instead of unreliable ASCII control codes.
 *   [x] **Shift+Tab Navigation (Phase 8)**: Fixed Shift+Tab to use `KeyCode::BackTab` from crossterm (not Tab + SHIFT modifier). Now properly cycles focus in reverse order.
 *   [x] **Filter Widget Editability (Phase 8)**: Added visual "[Enter to edit]" hint for disabled widgets when selected. When Space is pressed on a disabled widget, it enables AND enters text input mode immediately. Up/Down arrows now navigate between filter widgets in LHS panel and move cursor within text fields during TextInput mode.
 *   [x] **Horizontal Layout Panel Sizing (Phase 8)**: Replaced fixed `Constraint::Length` with ratio-based constraints so main panel gets proportional space. Prevents narrow terminal from squeezing panels into vertical-looking layout.
 *   [x] **Scroll Overflow Fix (Phase 8)**: Fixed subtraction overflow in `scroll_down()` when `new_selected < viewport_height`. Now uses `saturating_sub()` for safe arithmetic.

## 7. Current File Inventory

| File | Status | Description |
|---|---|---|
| `src/config.rs` | ✅ Complete | Shared CLI config: SharedConfig, TuiCli, SourceType, DetailLevel, OutputFormat, LayoutOrientation enums |
| `src/lib.rs` | ✅ Updated | Re-exports from config module; Cli struct extends SharedConfig with filter + output_format |
| `src/main.rs` | ✅ Updated | Uses SharedConfig via Cli; identical decoder/pipeline construction as TUI path |
| `src/tui/mod.rs` | ✅ Complete | Module re-export |
| `src/tui/app.rs` | ✅ Complete | TuiApp state, FilterWidget, Tab/Shift+Tab cycling, vertical layout toggle, scroll logic, filter evaluation. Up/Down arrows navigate LHS widgets and cursor position in text input mode. Space enables widget + enters edit mode. |
| `src/tui/renderer.rs` | ✅ Complete | Full 3-panel renderer with detail inspector, error log popup, colorization, horizontal + vertical layouts. Ratio-based panel sizing for horizontal mode. Cursor block rendering during text input. "[Enter to edit]" hints. |
| `src/tui_main.rs` | ✅ Complete | TuiCli parsing, Tab navigation, Ctrl+C handler (raw Unix signal + tokio), layout option, pipeline wiring from shared config. Fixed Shift+Tab via BackTab key code. |

## 8. Planned Features

### 8.1 Device Manager Panel

**Goal**: Display real-time device tracking information from the `DeviceManager` in a dedicated panel.

#### 8.1.1 Data Model
The `DeviceManager` tracks:
- Active devices with NAME, source address, TTL expiration
- Parameter cache (DDI values per device)
- Address claim conflicts (duplicate claims from different NAMEs for same address)

**Panel Layout**:
```
┌─────────────────────────────────────────┐
│ Devices (N active)                     │
├─────────────────────────────────────────┤
│ 0x90  EngineECU        [45s]            │
│ 0xA1  DisplayUnit      [12s] ⚠️         │
│ 0xFF  Broadcast        [60s]            │
│                                         │
│ ── Selected Device Details ──────────── │
│ NAME:    0x123456789ABCDEF              │
│ Address: 0x90 (144)                     │
│ TTL:     45s                            │
│ Params:  DDI 0x0E04 = 2500 RPM          │
│          DDI 0x0E08 = 85°C              │
│                                         │
│ Conflicts: 1                            │
│ ⚠️ 0x90 claimed by two NAMEs            │
└─────────────────────────────────────────┘
```

#### 8.1.2 Implementation
- **New RHS panel** (or expand existing RHS to include device tab)
- **Tab switching**: Device list view ↔ Selected device details ↔ Conflicts view
- **Auto-refresh**: TTL countdown updates every second (via timer in event loop)
- **Color coding**: Green = healthy, Yellow = expiring soon (< 10s), Red = expired/conflict
- **Navigation**: `Up`/`Down` to select device, `Enter` for details, `Tab` to switch views
- **Data source**: Access `DeviceManager` state from `TuiApp.device_manager` field

#### 8.1.3 Key Bindings
| Key | Action |
|---|---|
| `F5` | Toggle Device Manager panel visibility (or replace F2) |
| `Up`/`Down` | Navigate device list |
| `Enter` | View selected device details |
| `Tab` | Switch between List / Details / Conflicts views |
| `Esc` | Return to device list from details view |

---

### 8.2 Debug Logging

**Goal**: Separate debug log output independent of pretty-print filtering, for troubleshooting protocol issues.

#### 8.2.1 Log Categories
- **Protocol Issues**: TP reassembly timeouts, malformed frames, abort messages
- **Device Events**: Address claims, conflicts, TTL expiration, parameter cache updates
- **Decoder Warnings**: Unrecognized PGNs, short payloads, scale/offset errors
- **Filter Events**: Messages dropped by filters (optional, can be verbose)

#### 8.2.2 Output Targets
- **stderr** (default): Always available, no file management needed
- **File** (`--debug-log <path>`): Optional persistent log for post-session analysis
- **TUI Error Log** (existing F3 popup): Already captures some debug info; extend with structured categories

#### 8.2.3 Implementation
```rust
pub enum DebugLevel {
    Off,
    Basic,      // Errors and warnings only
    Verbose,    // Include protocol events, device lifecycle
    Trace,      // Every frame, reassembly state transitions
}
```

- **Log format**: `[HH:MM:SS.mmm] [CATEGORY] message` (colorized in TUI, plain text to file)
- **Categories**: `TP`, `DEVICE`, `DECODER`, `FILTER`, `SOURCE`
- **TUI integration**: F3 error log popup shows categorized entries with color-coded category prefixes
- **File rotation**: Optional max size (e.g., 10MB) with rotation to `.log.1`, `.log.2`

#### 8.2.4 Key Bindings / CLI
| Flag | Action |
|---|---|
| `--debug-log stderr` | Log to stderr (default when --debug is set) |
| `--debug-log <path>` | Log to file |
| `--debug-level basic\|verbose\|trace` | Control verbosity (default: basic) |

---

### 8.3 Save Capability

**Goal**: Export messages from the TUI to disk in multiple formats, supporting both all messages and filtered-only subsets.

#### 8.3.1 Supported Formats
| Format | Extension | Description |
|---|---|---|
| **JSON** | `.json` | Structured `DecodedMessage` array with full decoded fields, metadata, assembled payloads |
| **CSV** | `.csv` | Tabular format with fixed columns (timestamp, CAN ID, PGN, src/dest, data bytes, outputs) |
| **Condensed** | `.txt` | Single-line-per-message format with key=value pairs and pipe separators |
| **RAW candump** | `.log` | Standard `candump` format: `[timestamp] can0  DLC <len> <hex bytes>` — compatible with `cansniffer`, `canplayer` |

#### 8.3.2 Save Modes
| Mode | Description |
|---|---|
| **All Messages** | Export the complete message history (up to max_messages limit) |
| **Filtered Only** | Export only messages passing all active filter rules |
| **Selected Range** | Export a user-defined range of visible messages (from selected index, N messages) |

#### 8.3.3 UI Interaction
```
┌─────────────────────────────────────────┐
│ Save Dialog                             │
├─────────────────────────────────────────┤
│ Format: [JSON ▼]                        │
│                                         │
│ Scope:                                  │
│   (•) All Messages                      │
│   ( ) Filtered Only                     │
│   ( ) Selected Range                    │
│                                         │
│ Path: /tmp/capture.json                 │
│                                         │
│ [ Cancel ]  [ Save ]                    │
└─────────────────────────────────────────┘
```

#### 8.3.4 Key Bindings
| Key | Action |
|---|---|
| `F6` | Open save dialog (when Main panel has focus) |
| `Tab` | Navigate between format, scope, path fields |
| `Enter` | Confirm save |
| `Esc` / `Ctrl+C` | Cancel and close dialog |

#### 8.3.5 Implementation Details
- **Dialog widget**: New overlay modal in `renderer.rs`, similar to error log popup but with form inputs
- **Format selection**: Dropdown using FlagFilter-style option rendering (JSON, CSV, Condensed, RAW candump)
- **Scope selection**: Radio buttons implemented as selectable options
- **Path input**: Text field with basic path validation (directory exists, writable)
- **Background save**: Use `tokio::task::spawn_blocking` to write files without blocking the event loop
- **Progress feedback**: Status bar shows "Saving..." during write; toast notification on completion/error
- **RAW candump format**: Convert each `RawFrame` to standard candump line: `[timestamp_sec.usec] can0  DLC <len> byte0 byte1 ...`

#### 8.3.6 RAW Candump Format Specification
Each message's underlying `RawFrame` data is written as:
```
[1697452800.123456] can0  8  01 23 45 67 89 AB CD EF
```
Fields: `[seconds.microseconds] interface  DLC <data_length> <hex_bytes>`

For assembled TP messages, write all constituent DT frames individually (preserving the multi-frame sequence).

---

## 9. Execution Order for Planned Features

1. **Save Capability** — High value, self-contained. Adds `F6` key, save dialog widget, format writers. No dependencies on other features.
2. **Device Manager Panel** — Completes TUI feature set. Reuses existing `DeviceManager` state; adds panel rendering and navigation.
3. **Debug Logging** — Improves troubleshooting. Requires log infrastructure in pipeline + TUI error log integration.
