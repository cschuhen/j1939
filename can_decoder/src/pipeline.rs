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
            let synthetic_id =
                crate::tp_reassembler::build_assembled_can_id(frame.pgn(), 0x20, 0xFF, 6);

            let assembled = AssembledMessage {
                id: synthetic_id,
                pgn: frame.pgn(),
                data: frame.data.clone(),
                timestamp: frame.timestamp,
                source_name: None,
                dest_name: None,
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
pub use crate::renderers::ConsoleRenderer;

/// JSON renderer that serializes DecodedMessage to JSON format.
pub use crate::renderers::JsonRenderer;

/// CSV renderer that outputs DecodedMessage in CSV format with dynamic columns.
pub use crate::renderers::CsvRenderer;

/// Condensed renderer that outputs a single-line summary per message.
pub use crate::renderers::CondensedRenderer;

/// FullCondensed renderer that outputs a single-line summary per message.
pub use crate::renderers::FullCondensedRenderer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DecodedField, Numeric};

    fn make_test_message() -> DecodedMessage {
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![0x01, 0x02], 0);
        DecodedMessage {
            title: "Engine Speed".to_string(),
            outputs: vec![
                DecodedField::Value {
                    title: "RPM".to_string(),
                    value: Numeric::Int(1500),
                    unit: Some("rpm".to_string()),
                    decimal_places: None,
                },
                DecodedField::StringMessage {
                    severity: crate::types::Severity::Info,
                    text: "Engine running".to_string(),
                },
            ],
            updates: vec![],
            assembled_message: assembled,
        }
    }

    #[tokio::test]
    async fn test_json_renderer_name() {
        let renderer = JsonRenderer;
        assert_eq!(renderer.name(), "json");
    }

    #[tokio::test]
    async fn test_json_renderer_valid_output() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        assert!(!result.is_empty());
        assert!(result.contains("\"title\": \"Engine Speed\""));
    }

    #[tokio::test]
    async fn test_json_renderer_contains_outputs() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"title\": \"RPM\""));
        assert!(result.contains("1500"));
        assert!(result.contains("\"text\": \"Engine running\""));
    }

    #[tokio::test]
    async fn test_json_renderer_output_structure() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let outputs = parsed["outputs"].as_array().unwrap();
        assert_eq!(outputs.len(), 2);

        let first_output = &outputs[0];
        assert!(first_output.get("Value").is_some());
        let value_obj = &first_output["Value"];
        assert_eq!(value_obj["title"], "RPM");
        assert_eq!(value_obj["value"]["Int"], 1500);

        let second_output = &outputs[1];
        assert!(second_output.get("StringMessage").is_some());
        let msg_obj = &second_output["StringMessage"];
        assert_eq!(msg_obj["text"], "Engine running");
    }

    #[tokio::test]
    async fn test_json_renderer_contains_pgn() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"pgn\": 61184"));
    }

    #[tokio::test]
    async fn test_json_renderer_contains_timestamp() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"timestamp\":"));
    }

    #[tokio::test]
    async fn test_json_renderer_parseable() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "Engine Speed");
        assert_eq!(parsed["outputs"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_json_renderer_empty_outputs() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Empty Message".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"outputs\": []"));
    }

    #[tokio::test]
    async fn test_json_renderer_flag_value() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Status".to_string(),
            outputs: vec![DecodedField::Flag {
                title: "Engine".to_string(),
                value: crate::types::FlagValue::On,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"value\": \"on\""));
    }

    #[tokio::test]
    async fn test_json_renderer_hex_value() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "VIN".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Data".to_string(),
                value: Numeric::Hex(vec![0x12, 0x34, 0xAB]),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"Hex\""));
        assert!(result.contains("18"));
        assert!(result.contains("52"));
        assert!(result.contains("171"));
    }

    #[tokio::test]
    async fn test_json_renderer_bool_value() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Flag".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Active".to_string(),
                value: Numeric::Bool(true),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"Bool\": true"));
    }

    #[tokio::test]
    async fn test_json_renderer_float_value() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Temperature".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Coolant".to_string(),
                value: Numeric::Float(92.5),
                unit: Some("C".to_string()),
                decimal_places: Some(1),
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"Float\": 92.5"));
    }

    #[tokio::test]
    async fn test_json_renderer_severity_values() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for severity in [
            crate::types::Severity::Info,
            crate::types::Severity::Warning,
            crate::types::Severity::Error,
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::StringMessage {
                    severity: severity.clone(),
                    text: "test".to_string(),
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            let expected = format!("\"severity\": \"{:?}\"", severity);
            assert!(result.contains(&expected));
        }
    }

    #[tokio::test]
    async fn test_json_renderer_data_bytes() {
        let mut renderer = JsonRenderer;
        let assembled =
            AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![0xDE, 0xAD, 0xBE, 0xEF], 0);
        let message = DecodedMessage {
            title: "Data".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"data\""));
        assert!(result.contains("222"));
        assert!(result.contains("173"));
    }

    #[tokio::test]
    async fn test_json_renderer_assembled_message_fields() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Address".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"id\":"));
        assert!(result.contains("\"pgn\":"));
        assert!(result.contains("\"timestamp\":"));
    }

    #[tokio::test]
    async fn test_json_renderer_source_dest_name() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Names".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"source_name\": null"));
        assert!(result.contains("\"dest_name\": null"));
    }

    #[tokio::test]
    async fn test_json_renderer_updates() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Updates".to_string(),
            outputs: vec![],
            updates: vec![crate::types::DeviceUpdate {
                target_name: 0x1234567890ABCDEF,
                param_id: 42,
                value: Numeric::Int(100),
            }],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"updates\":"));
        assert!(result.contains("42"));
    }

    #[tokio::test]
    async fn test_json_renderer_can_id() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "ID".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"id\""));
    }

    #[tokio::test]
    async fn test_json_renderer_all_flag_values() {
        let mut renderer = JsonRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for (flag_val, expected_str) in [
            (crate::types::FlagValue::Off, "\"off\""),
            (crate::types::FlagValue::On, "\"on\""),
            (crate::types::FlagValue::Error, "\"error\""),
            (crate::types::FlagValue::Unavailable, "\"unavailable\""),
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::Flag {
                    title: "Status".to_string(),
                    value: flag_val.clone(),
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            assert!(result.contains(expected_str), "Failed for {:?}", flag_val);
        }
    }

    #[tokio::test]
    async fn test_json_renderer_nested_structure() {
        let mut renderer = JsonRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert!(parsed.get("title").is_some());
        assert!(parsed.get("outputs").is_some());
        assert!(parsed.get("updates").is_some());
        assert!(parsed.get("assembled_message").is_some());

        let outputs = parsed["outputs"].as_array().unwrap();
        assert_eq!(outputs.len(), 2);

        let first_output = &outputs[0];
        assert!(first_output.get("Value").is_some());
        let value_obj = &first_output["Value"];
        assert!(value_obj.get("title").is_some());
        assert!(value_obj.get("value").is_some());
    }
}
