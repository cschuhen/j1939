use std::sync::Arc;

use j1939_async::can::Id;

use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::traits::{Decoder, Filter, Renderer, Source};
use crate::types::{AssembledMessage, DecodedField, DecodedMessage, RawFrame};

/// Manages the async pipeline: Source → Decoder → Filter → Renderer.
///
/// Uses unbounded channels so no frames are ever dropped (per requirements).
/// Receivers are consumed via `take()` on first use, ensuring each stage
/// gets exclusive ownership of its input channel.
pub struct Pipeline {
    /// Sender for pushing raw CAN frames into the pipeline.
    source_tx: mpsc::UnboundedSender<RawFrame>,
    /// Receiver for raw CAN frames (consumed by decoder).
    decoder_rx: Option<mpsc::UnboundedReceiver<RawFrame>>,
    /// Sender from Decoder, input to Filter.
    output_tx: mpsc::UnboundedSender<DecodedMessage>,
    /// Receiver from Decoder / input to Filter (consumed by spawn_filter).
    output_rx: Option<mpsc::UnboundedReceiver<DecodedMessage>>,
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

    /// Spawn a Decoder task that reads RawFrames and emits DecodedMessage items.
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
                    Ok(message) => {
                        if tx.send(message).is_err() {
                            return Ok(());
                        }
                    }
                    Err(e) => eprintln!("Decoder error: {}", e),
                }
            }
            Ok(())
        })
    }

    /// Spawn a Filter task that passes matching DecodedMessage items downstream.
    ///
    /// Consumes the output_rx channel and returns a new receiver for filtered output.
    pub fn spawn_filter(
        &mut self,
        filter: Arc<Mutex<dyn Filter>>,
    ) -> (mpsc::UnboundedReceiver<DecodedMessage>, JoinHandle<()>) {
        let mut rx = self.output_rx.take().expect("output_rx already consumed");
        let (filter_tx, filter_rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                let f = filter.lock().await;
                if f.matches(&message).await && filter_tx.send(message).is_err() {
                    break;
                }
            }
        });
        (filter_rx, handle)
    }

    /// Spawn a Renderer task that formats and prints DecodedMessage items.
    ///
    /// Takes ownership of both the receiver and renderer instance.
    pub fn spawn_renderer(
        mut rx: mpsc::UnboundedReceiver<DecodedMessage>,
        mut renderer: Box<dyn Renderer>,
    ) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match renderer.render(&message).await {
                    Ok(line) => println!("{}", line),
                    Err(e) => eprintln!("Render error: {}", e),
                }
            }
            Ok(())
        })
    }
}

/// Stub decoder that echoes DecodedField items without modification.
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
                    Output = Result<DecodedMessage, Box<dyn std::error::Error + Send + Sync>>,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            if self.debug {
                let id = j1939_async::can::IdImpl::new_unchecked(frame.can_id);
                if id.pgn() == 0xEE00 {
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
            let assembled = AssembledMessage {
                id: frame.can_id,
                pgn: frame.pgn(),
                data: frame.data.clone(),
                timestamp: frame.timestamp,
            };
            Ok(DecodedMessage {
                title: format!("Raw Frame {:08X}", frame.can_id),
                outputs: vec![DecodedField::StringMessage {
                    severity: crate::types::Severity::Info,
                    text,
                }],
                updates: vec![],
                assembled_message: assembled,
            })
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
        _message: &DecodedMessage,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { true })
    }
}

/// Stub renderer that prints DecodedField as plain text.
pub struct ConsoleRenderer;

impl Renderer for ConsoleRenderer {
    fn name(&self) -> &str {
        "console"
    }

    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<String, Box<dyn std::error::Error + Send + Sync>>,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            use owo_colors::OwoColorize;
            let mut lines = Vec::new();
            lines.push(format!(
                "MSG: --- {} ---",
                message.title.clone().bold().cyan()
            ));
            for output in &message.outputs {
                lines.push(format_output(output));
            }
            Ok(lines.join("\n"))
        })
    }
}

/// Format a DecodedField item into a human-readable string.
fn format_output(output: &DecodedField) -> String {
    use owo_colors::OwoColorize;
    let w1 = 20;
    let w2 = 15;

    match output {
        DecodedField::Value {
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
        DecodedField::StringMessage { severity, text } => {
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
        DecodedField::Flag { title, value } => {
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
