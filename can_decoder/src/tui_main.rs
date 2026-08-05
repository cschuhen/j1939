use can_decoder::config::{validate_proprietary_definitions, LayoutOrientation, TuiCli};
use can_decoder::traits::Source;
use can_decoder::tui::app::{TuiApp, TuiKey};
use can_decoder::tui::renderer::TuiRenderer;
use can_decoder::types::DecodedMessage;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use clap::Parser;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let cli = TuiCli::parse();
    let config = &cli.shared;

    println!("can_decoder TUI starting...");
    println!("Source: {:?}", config.source);
    if let Some(ref iface) = config.interface {
        println!("Interface: {}", iface);
    }
    if let Some(ref file) = config.input_file {
        println!("Input file: {}", file.display());
    }
    println!("Layout: {:?}", cli.layout);

    // Validate proprietary DDI definitions
    let proprietary_defs =
        validate_proprietary_definitions(&config.use_proprietary_ddi_definitions);

    // Build pipeline
    let mut pipeline = can_decoder::pipeline::Pipeline::new();

    match config.source {
        can_decoder::SourceType::Socketcan => {
            if let Some(ref iface) = config.interface {
                let source = std::sync::Arc::new(can_decoder::sources::SocketCanSource::new(iface));
                println!("[{}] Starting live source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --interface required for socketcan source");
                std::process::exit(1);
            }
        }
        can_decoder::SourceType::Candump => {
            if let Some(ref file) = config.input_file {
                let source =
                    std::sync::Arc::new(can_decoder::sources::CandumpFileSource::new(file));
                println!("[{}] Starting candump source...", source.name());
                pipeline.spawn_source(source);
            } else {
                eprintln!("Error: --input-file required for candump source");
                std::process::exit(1);
            }
        }
    }

    // Decoder with device manager
    let device_manager = std::sync::Arc::new(std::sync::Mutex::new(
        can_decoder::device_manager::DeviceManager::new(60),
    ));
    let decoder = can_decoder::pgn_decoder::J1939Decoder::with_device_manager_detail(
        config.force_output_partial_tp,
        5000,
        config.debug,
        Some(device_manager),
        config.detail_level.clone(),
        &proprietary_defs,
    );
    pipeline.spawn_decoder(Box::new(decoder));

    // No CLI filters for TUI - they're done via UI widgets (pass-through)
    let filter = std::sync::Arc::new(tokio::sync::Mutex::new(
        can_decoder::filters::CompositeFilter::new(vec![]),
    ));
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);

    // Bridge: decoder output -> mpsc channel for TUI event loop
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<DecodedMessage>();

    tokio::spawn(async move {
        let mut rx = filter_rx;
        while let Some(message) = rx.recv().await {
            if msg_tx.send(message).is_err() {
                break;
            }
        }
    });

    println!("Pipeline ready. Press Ctrl+C or q to quit.");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state with layout option
    let mut app = TuiApp::new();
    app.layout_vertical = cli.layout == LayoutOrientation::Vertical;
    app.connection_status = can_decoder::tui::app::ConnectionStatus::Connected;

    let renderer = TuiRenderer::new();

    // Setup Ctrl+C signal handler via broadcast channel
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
    tokio::spawn(async move {
        if let Err(_) = tokio::signal::ctrl_c().await {
            return;
        }
        let _ = shutdown_tx.send(());
    });

    // Main event loop
    let mut last_frame_time = std::time::Instant::now();
    let mut running = true;

    while running {
        // Drain messages from pipeline channel
        while let Ok(message) = msg_rx.try_recv() {
            app.add_message(message);
        }

        // Render at ~60fps
        if last_frame_time.elapsed() >= std::time::Duration::from_millis(16) {
            terminal.draw(|frame| {
                renderer.render(frame, &mut app);
            })?;
            last_frame_time = std::time::Instant::now();
        }

        // Check for Ctrl+C signal (non-blocking)
        match shutdown_rx.try_recv() {
            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                running = false;
            }
            _ => {}
        }

        // Handle input with short poll to avoid busy-waiting
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    // Direct Ctrl+C check - crossterm may represent it as Char('\u{3}') or Char('\u{0}') with CONTROL
                    if (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('C'))
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        running = false;
                    } else if key.code == KeyCode::Char('q') {
                        running = false;
                    } else if key.code == KeyCode::F(4) {
                        app.toggle_layout();
                    } else {
                        let tui_key = convert_key(key.code, key.modifiers);
                        app.handle_key(tui_key);
                    }
                }
            }
        }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("\nShutting down...");
    Ok(())
}

fn convert_key(code: KeyCode, modifiers: KeyModifiers) -> TuiKey {
    match code {
        KeyCode::Char('\u{3}') if modifiers.contains(KeyModifiers::CONTROL) => TuiKey::CtrlC,
        KeyCode::Char('\u{0}') if modifiers.contains(KeyModifiers::CONTROL) => TuiKey::CtrlC,
        KeyCode::Char(' ') => TuiKey::Space,
        KeyCode::Char('q') => TuiKey::Char('q'),
        KeyCode::Char(c) => TuiKey::Char(c),
        KeyCode::F(n) => TuiKey::F(n),
        KeyCode::Up => {
            if modifiers == KeyModifiers::SHIFT {
                TuiKey::ShiftUp
            } else {
                TuiKey::Up
            }
        }
        KeyCode::Down => {
            if modifiers == KeyModifiers::SHIFT {
                TuiKey::ShiftDown
            } else {
                TuiKey::Down
            }
        }
        KeyCode::Left => TuiKey::Left,
        KeyCode::Right => TuiKey::Right,
        KeyCode::PageUp => TuiKey::PageUp,
        KeyCode::PageDown => TuiKey::PageDown,
        KeyCode::Enter => TuiKey::Enter,
        KeyCode::Esc => TuiKey::Esc,
        KeyCode::Backspace | KeyCode::Delete => TuiKey::Char('\u{7F}'),
        KeyCode::Tab => TuiKey::Tab,
        KeyCode::BackTab => TuiKey::ShiftTab,
        _ => TuiKey::Char(' '),
    }
}
