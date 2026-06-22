use can_decoder::pipeline::{Pipeline, NullDecoder};
use can_decoder::traits::{Decoder, Filter, Renderer, Source};
use can_decoder::types::{PrettyOutput, RawFrame, Severity};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::pin::Pin;
use std::future::Future;
use std::error::Error;

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
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static>> {
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
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PrettyOutput>, Box<dyn Error + Send + Sync>>> + Send + 'static>> {
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
        output: PrettyOutput,
    ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let pattern = self.pattern.clone();
        Box::pin(async move {
            if let PrettyOutput::StringMessage { text, .. } = output {
                text.contains(&pattern)
            } else {
                false
            }
        })
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
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send + 'static>> {
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
        RawFrame { can_id: 0x123, timestamp: 100, data: vec![0x01] },
        RawFrame { can_id: 0x456, timestamp: 101, data: vec![0x02] },
    ];
    let source = Arc::new(MockSource { frames });
    
    let decoder = Box::new(MockDecoder);
    pipeline.spawn_decoder(decoder);
    
    let filter = Arc::new(Mutex::new(MockFilter { pattern: "00000123".to_string() }));
    let (filter_rx, _filter_handle) = pipeline.spawn_filter(filter);
    
    let received = Arc::new(Mutex::new(Vec::new()));
    let renderer = Box::new(MockRenderer { received: received.clone() });
    let renderer_handle = Pipeline::spawn_renderer(filter_rx, renderer);
    
    pipeline.spawn_source(source).await.unwrap();
    
    // Wait a bit for the pipeline to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let results = received.lock().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], "Frame: 00000123");
    
    // Drop the renderer handle to stop the renderer task
    drop(renderer_handle);
}
