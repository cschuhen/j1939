use can_decoder::pipeline::{ConsoleRenderer, Pipeline};
use can_decoder::traits::{Decoder, Filter, Renderer, Source};
use can_decoder::types::{FlagValue, Numeric, PrettyOutput, RawFrame, Severity};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[tokio::test]
async fn test_console_renderer() {
    let mut renderer = ConsoleRenderer;

    // Test Value
    let output_val = PrettyOutput::Value {
        title: "Test Title".to_string(),
        value: Numeric::Int(42),
        unit: Some("unit".to_string()),
        decimal_places: None,
    };
    let res_val = renderer.render(output_val).await.unwrap();
    assert!(res_val.contains("Test Title"));
    assert!(res_val.contains("42"));
    assert!(res_val.contains("unit"));

    // Test StringMessage
    let output_str = PrettyOutput::StringMessage {
        severity: Severity::Error,
        text: "Error message".to_string(),
    };
    let res_str = renderer.render(output_str).await.unwrap();
    assert!(res_str.contains("ERROR"));
    assert!(res_str.contains("Error message"));

    // Test Flag
    let output_flag = PrettyOutput::Flag {
        title: "Flag Title".to_string(),
        value: FlagValue::On,
    };
    let res_flag = renderer.render(output_flag).await.unwrap();
    assert!(res_flag.contains("Flag Title"));
    assert!(res_flag.contains("ON"));
}

struct MockSource {
    frames: Vec<RawFrame>,
}

impl Source for MockSource {
    fn name(&self) -> &str {
        "mock_source"
    }

    fn start(
        self: Arc<Self>,
        tx: mpsc::UnboundedSender<RawFrame>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static>>
    {
        Box::pin(async move {
            for frame in self.frames.iter() {
                tx.send(frame.clone()).map_err(|e| e.to_string())?;
            }
            Ok(())
        })
    }
}

struct MockDecoder;

impl Decoder for MockDecoder {
    fn name(&self) -> &str {
        "mock_decoder"
    }

    fn decode(
        &mut self,
        frame: RawFrame,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<PrettyOutput>, Box<dyn Error + Send + Sync>>>
                + Send
                + 'static,
        >,
    > {
        Box::pin(async move {
            Ok(vec![PrettyOutput::StringMessage {
                severity: Severity::Info,
                text: format!("Frame: {:08X}", frame.can_id),
            }])
        })
    }
}

struct MockFilter {
    pattern: String,
}

impl Filter for MockFilter {
    fn name(&self) -> &str {
        "mock_filter"
    }

    fn matches(
        &self,
        output: &PrettyOutput,
    ) -> Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        let pattern = self.pattern.clone();
        let is_match = match output {
            PrettyOutput::StringMessage { text, .. } => text.contains(&pattern),
            _ => false,
        };
        Box::pin(async move { is_match })
    }
}

struct MockRenderer {
    received: Arc<Mutex<Vec<String>>>,
}

impl Renderer for MockRenderer {
    fn name(&self) -> &str {
        "mock_renderer"
    }

    fn render(
        &mut self,
        output: PrettyOutput,
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send + 'static>>
    {
        let received = self.received.clone();
        Box::pin(async move {
            let text = match output {
                PrettyOutput::StringMessage { text, .. } => text,
                _ => "other".to_string(),
            };
            received.lock().await.push(text);
            Ok("ok".to_string())
        })
    }
}

#[tokio::test]
async fn test_pipeline_flow() {
    let mut pipeline = Pipeline::new();

    let frames = vec![
        RawFrame {
            can_id: 0x123,
            timestamp: 100,
            data: vec![0x01],
        },
        RawFrame {
            can_id: 0x456,
            timestamp: 101,
            data: vec![0x02],
        },
    ];
    let source = Arc::new(MockSource { frames });

    let decoder = Box::new(MockDecoder);
    let _decoder_handle = pipeline.spawn_decoder(decoder);

    let filter = Arc::new(Mutex::new(MockFilter {
        pattern: "00000123".to_string(),
    }));
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);

    let received = Arc::new(Mutex::new(Vec::new()));
    let renderer = Box::new(MockRenderer {
        received: received.clone(),
    });
    let renderer_handle = Pipeline::spawn_renderer(filter_rx, renderer);

    let _ = pipeline.spawn_source(source).await.unwrap();

    // Wait a bit for the pipeline to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let results = received.lock().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], "Frame: 00000123");

    // Drop the renderer handle to stop the renderer task
    drop(renderer_handle);
}
