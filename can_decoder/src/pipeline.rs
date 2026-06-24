use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::traits::{Decoder, Filter, Renderer, Source};
use crate::types::{PrettyOutput, RawFrame};

/// Manages the async pipeline: Source → Decoder → Filter → Renderer.
///
/// Uses unbounded channels so no frames are ever dropped (per requirements).
/// Receivers are consumed via `take()` on first use, ensuring each stage
/// gets exclusive ownership of its input channel.
pub struct Pipeline {
    /// Sender for pushing raw CAN frames into the pipeline.
    source_tx: mpsc::UnboundedSender<RawFrame>,
    /// Receiver for decoded PrettyOutput items (consumed by filter/renderer).
    decoder_rx: Option<mpsc::UnboundedReceiver<RawFrame>>,
    /// Sender from Decoder, input to Filter.
    output_tx: mpsc::UnboundedSender<PrettyOutput>,
    /// Receiver from Decoder / input to Filter (consumed by spawn_filter).
    output_rx: Option<mpsc::UnboundedReceiver<PrettyOutput>>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    pub fn new() -> Self {
        let (source_tx, decoder_rx) = mpsc::unbounded_channel();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        Pipeline {
            source_tx,
            decoder_rx: Some(decoder_rx),
            output_tx,
            output_rx: Some(output_rx),
        }
    }

    /// Get the sender to push raw frames into the pipeline.
    pub fn source_sender(&self) -> mpsc::UnboundedSender<RawFrame> {
        self.source_tx.clone()
    }

    /// Spawn a Source task that feeds RawFrames into this pipeline.
    ///
    /// The Source implementation runs in its own Tokio task and calls
    /// `start()` to begin producing frames until it errors or is stopped.
    pub fn spawn_source(
        &self,
        source: Arc<dyn Source>,
    ) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        let tx = self.source_tx.clone();
        tokio::spawn(async move { source.start(tx).await })
    }

    /// Spawn a Decoder task that reads RawFrames and emits PrettyOutput items.
    ///
    /// Consumes the decoder_rx channel, so this method can only be called once.
    pub fn spawn_decoder(
        &mut self,
        mut decoder: Box<dyn Decoder>,
    ) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        let mut rx = self.decoder_rx.take().expect("decoder_rx already consumed");
        let tx = self.output_tx.clone();
        tokio::spawn(async move {
            while let Some(frame) = rx.recv().await {
                match decoder.decode(frame).await {
                    Ok(outputs) => {
                        for output in outputs {
                            if tx.send(output).is_err() {
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => eprintln!("Decoder error: {}", e),
                }
            }
            Ok(())
        })
    }

    /// Spawn a Filter task that passes matching PrettyOutput items downstream.
    ///
    /// Consumes the output_rx channel and returns a new receiver for filtered output.
    pub fn spawn_filter(
        &mut self,
        filter: Arc<Mutex<dyn Filter>>,
    ) -> (mpsc::UnboundedReceiver<PrettyOutput>, JoinHandle<()>) {
        let mut rx = self.output_rx.take().expect("output_rx already consumed");
        let (filter_tx, filter_rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(output) = rx.recv().await {
                let f = filter.lock().await;
                if f.matches(&output).await && filter_tx.send(output).is_err() {
                    break;
                }
            }
        });
        (filter_rx, handle)
    }

    /// Spawn a Renderer task that formats and prints PrettyOutput items.
    ///
    /// Takes ownership of both the receiver and renderer instance.
    pub fn spawn_renderer(
        mut rx: mpsc::UnboundedReceiver<PrettyOutput>,
        mut renderer: Box<dyn Renderer>,
    ) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        tokio::spawn(async move {
            while let Some(output) = rx.recv().await {
                match renderer.render(output).await {
                    Ok(line) => println!("{}", line),
                    Err(e) => eprintln!("Render error: {}", e),
                }
            }
            Ok(())
        })
    }
}

/// Stub decoder that echoes PrettyOutput items without modification.
pub struct NullDecoder {
    pub debug: bool,
}

impl Decoder for NullDecoder {
    fn name(&self) -> &str {
        "null"
    }

    fn decode(
        &mut self,
        frame: RawFrame,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<Vec<PrettyOutput>, Box<dyn std::error::Error + Send + Sync>>,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            if self.debug {
                let pgn_info = crate::types::PGN::from_can_id(frame.can_id);
                if pgn_info.pgn == 0xEE00 {
                    println!("[DEBUG] Processing address claim");
                }
                println!(
                    "[DEBUG] Decoding frame: ID={:08X}, Data={:02X?}",
                    frame.can_id, frame.data
                );
            }
            let text = format!(
                "CAN {:08X} len={} data={}",
                frame.can_id,
                frame.data.len(),
                frame
                    .data
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            Ok(vec![PrettyOutput::StringMessage {
                severity: crate::types::Severity::Info,
                text,
            }])
        })
    }
}

/// Stub filter that passes everything through.
pub struct PassThroughFilter;

impl Filter for PassThroughFilter {
    fn name(&self) -> &str {
        "passthrough"
    }

    fn matches(
        &self,
        _output: &PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { true })
    }
}

/// Stub renderer that prints PrettyOutput as plain text.
pub struct ConsoleRenderer;

impl Renderer for ConsoleRenderer {
    fn name(&self) -> &str {
        "console"
    }

    fn render(
        &mut self,
        output: PrettyOutput,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<String, Box<dyn std::error::Error + Send + Sync>>,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move { Ok(format_output(&output)) })
    }
}

/// Format a PrettyOutput item into a human-readable string.
fn format_output(output: &PrettyOutput) -> String {
    use owo_colors::OwoColorize;
    let w1 = 20;
    let w2 = 15;

    match output {
        PrettyOutput::Value {
            title,
            value,
            unit,
            decimal_places,
        } => {
            let val_str = match value {
                crate::types::Numeric::Int(v) => format!("{}", v),
                crate::types::Numeric::Float(v) => {
                    if let Some(dp) = decimal_places {
                        format!("{:.dp$}", v, dp = *dp as usize)
                    } else {
                        format!("{}", v)
                    }
                }
                crate::types::Numeric::Hex(v) => {
                    format!(
                        "0x{}",
                        v.iter().map(|b| format!("{:02X}", b)).collect::<String>()
                    )
                }
                crate::types::Numeric::Bool(v) => format!("{}", v),
            };
            let title_part = format!("[{}]", title.bold().cyan());
            let unit_part = unit
                .as_ref()
                .map(|u| u.dimmed().to_string())
                .unwrap_or_else(|| "".to_string());
            format!(
                "{:<w1$} | {:<w2$} | {}",
                title_part,
                val_str,
                unit_part,
                w1 = w1,
                w2 = w2
            )
        }
        PrettyOutput::StringMessage { severity, text } => {
            let sev_part = match severity {
                crate::types::Severity::Info => "INFO".green().bold().to_string(),
                crate::types::Severity::Warning => "WARN".yellow().bold().to_string(),
                crate::types::Severity::Error => "ERROR".red().bold().to_string(),
            };
            format!(
                "{:<w1$} | {:<w2$} | {}",
                sev_part,
                "",
                text,
                w1 = w1,
                w2 = w2
            )
        }
        PrettyOutput::Flag { title, value } => {
            let flag_text = match value {
                crate::types::FlagValue::Off => "OFF".red().to_string(),
                crate::types::FlagValue::On => "ON".green().to_string(),
                crate::types::FlagValue::Error => "ERR".red().bold().to_string(),
                crate::types::FlagValue::Unavailable => "N/A".dimmed().to_string(),
            };
            let title_part = format!("[{}]", title.bold().cyan());
            format!(
                "{:<w1$} | {:<w2$} | {}",
                title_part,
                "",
                flag_text,
                w1 = w1,
                w2 = w2
            )
        }
    }
}
