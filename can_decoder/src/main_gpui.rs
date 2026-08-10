mod gpui;

use std::sync::Arc;

use anyhow::Result;
use can_decoder::config::validate_proprietary_definitions;
use can_decoder::Cli;
use can_decoder::device_manager::DeviceManager;
use can_decoder::filters::CompositeFilter;
use can_decoder::pipeline::Pipeline;
use can_decoder::pgn_decoder::J1939Decoder;
use can_decoder::sources::{CandumpFileSource, SocketCanSource};
use can_decoder::traits::Source;
use can_decoder::types::DecodedMessage;
use clap::Parser;
use ::gpui::WindowOptions;
use ::gpui::{Application, AppContext, Bounds, Size, WindowBounds};
use tokio::sync::mpsc;

fn build_pipeline(cli: &Cli) -> Result<(Pipeline, mpsc::UnboundedReceiver<DecodedMessage>)> {
    let proprietary_defs = validate_proprietary_definitions(&cli.shared.use_proprietary_ddi_definitions);

    let mut pipeline = Pipeline::new();

    match cli.shared.source {
        can_decoder::SourceType::Socketcan => {
            if let Some(ref iface) = cli.shared.interface {
                let source = Arc::new(SocketCanSource::new(iface));
                eprintln!("[{}] Starting live source...", source.name());
                pipeline.spawn_source(source);
            } else {
                anyhow::bail!("Error: --interface required for socketcan source");
            }
        }
        can_decoder::SourceType::Candump => {
            if let Some(ref file) = cli.shared.input_file {
                let source = Arc::new(CandumpFileSource::new(file));
                eprintln!("[{}] Starting candump source...", source.name());
                pipeline.spawn_source(source);
            } else {
                anyhow::bail!("Error: --input-file required for candump source");
            }
        }
    }

    let device_manager = Arc::new(std::sync::Mutex::new(DeviceManager::new(60)));
    let decoder = J1939Decoder::with_device_manager_detail(
        cli.shared.force_output_partial_tp,
        5000,
        cli.shared.debug,
        Some(device_manager),
        cli.shared.detail_level.clone(),
        &proprietary_defs,
    );
    pipeline.spawn_decoder(Box::new(decoder));

    let filter = Arc::new(tokio::sync::Mutex::new(CompositeFilter::new(vec![])));
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);

    let (msg_tx, msg_rx) = mpsc::unbounded_channel::<DecodedMessage>();

    tokio::spawn(async move {
        let mut rx = filter_rx;
        while let Some(message) = rx.recv().await {
            if msg_tx.send(message).is_err() {
                break;
            }
        }
    });

    Ok((pipeline, msg_rx))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    eprintln!("can_decoder GPUI starting...");
    eprintln!("Source: {:?}", cli.shared.source);
    if let Some(ref iface) = cli.shared.interface {
        eprintln!("Interface: {}", iface);
    }
    if let Some(ref file) = cli.shared.input_file {
        eprintln!("Input file: {}", file.display());
    }

    let rt = tokio::runtime::Runtime::new()?;

    let msg_rx = rt.block_on(async {
        build_pipeline(&cli).map(|(_, rx)| rx)
    })?;

    let _rt = Box::leak(Box::new(rt));

    Application::new().run(move |cx| {
        let app_state = gpui::app_state::AppState::new(&Default::default());
        let main_view = gpui::renderer::MainView::new(app_state, msg_rx);

        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: Size { width: 1200.0.into(), height: 800.0.into() },
                })),
                titlebar: Some(::gpui::TitlebarOptions {
                    title: Some("can_decoder GPUI".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            move |_window, cx| cx.new(|_| main_view),
        );
    });

    eprintln!("\nShutting down...");
    Ok(())
}
