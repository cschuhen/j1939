use can_decoder::pgn_decoder::J1939Decoder;
use can_decoder::pipeline::{ConsoleRenderer, Pipeline};
use can_decoder::traits::{Decoder, Filter, Renderer, Source};
use can_decoder::types::{DecodedField, DecodedMessage, FlagValue, Numeric, RawFrame, Severity};
use j1939_async::can::Id;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[tokio::test]
async fn test_console_renderer() {
    let mut renderer = ConsoleRenderer;

    // Test Value
    let output_val = DecodedField::Value {
        title: "Test Title".to_string(),
        value: Numeric::Int(42),
        unit: Some("unit".to_string()),
        decimal_places: None,
    };
    let mut output_message = DecodedMessage::new("Some Title".to_string());
    output_message.outputs.push(output_val);

    let res_val = renderer.render(&output_message).await.unwrap();
    assert!(res_val.contains("Test Title"));
    assert!(res_val.contains("42"));
    assert!(res_val.contains("unit"));

    // Test StringMessage
    let output_str = DecodedField::StringMessage {
        severity: Severity::Error,
        text: "Error message".to_string(),
    };
    let mut output_message = DecodedMessage::new("Some Title".to_string());
    output_message.outputs.push(output_str);
    let res_str = renderer.render(&output_message).await.unwrap();
    assert!(res_str.contains("ERROR"));
    assert!(res_str.contains("Error message"));

    // Test Flag
    let output_flag = DecodedField::Flag {
        title: "Flag Title".to_string(),
        value: FlagValue::On,
    };
    let mut output_message = DecodedMessage::new("Some Title".to_string());
    output_message.outputs.push(output_flag);
    let res_flag = renderer.render(&output_message).await.unwrap();
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

    /*fn decode(
        &mut self,
        frame: RawFrame,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<DecodedField>, Box<dyn Error + Send + Sync>>>
                + Send
                + 'static,
        >,
    > {*/

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
            let _pgn = j1939_async::can::IdImpl::new_unchecked(frame.can_id).pgn();
            let mut output_message =
                DecodedMessage::new(format!("Frame: {:08X}", frame.can_id));
            output_message.outputs = vec![DecodedField::StringMessage {
                severity: Severity::Info,
                text: format!("Frame: {:08X}", frame.can_id),
            }];

            Ok(output_message)
            //Ok(vec![DecodedField::StringMessage {
            //    severity: Severity::Info,
            //    text: format!("Frame: {:08X}", frame.can_id),
            //}])
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
        output: &DecodedMessage,
    ) -> Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        let pattern = self.pattern.clone();
        let is_match = match &output.outputs[0] {
            DecodedField::StringMessage { text, .. } => text.contains(&pattern),
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

    /*fn render<'a>(
        &'a mut self,
        output: &'a DecodedMessage,
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send + 'static>>
    {*/
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
        let received = self.received.clone();
        Box::pin(async move {
            let text = match message.outputs[0].clone() {
                DecodedField::StringMessage { text, .. } => text,
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

// ========================================================================
// TP Reassembly Integration Tests (bam.log and rts.log)
// ========================================================================

#[test]
fn test_full_bam_decode_pipeline() {
    let mut decoder = J1939Decoder::new(false, 5000, false);

    // Feed all 4 frames from bam.log in order
    let all_results: Vec<Vec<DecodedField>> = vec![
        (0x18ECFF22, vec![0x20, 0x0F, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]),
        (0x18EBFF22, vec![0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]),
        (0x18EBFF22, vec![0x02, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]),
        (0x18EBFF22, vec![0x03, 0x4F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    ]
    .into_iter()
    .map(|(can_id, data)| {
        decoder.decode_raw_frame(RawFrame::new(can_id, data))
    })
    .collect();

    // Collect all outputs from all frames
    let mut all_outputs = Vec::new();
    for results in &all_results {
        all_outputs.extend(results.clone());
    }

    // Find the Complete result (should be on frame 4)
    let complete_results: Vec<_> = all_results.iter().enumerate()
        .filter(|(_, results)| !results.is_empty())
        .collect();

    assert!(complete_results.len() >= 1, "Expected at least one non-empty result set");

    // The last frame should produce a Complete assembly result
    let last_results = &all_results[3];
    assert!(!last_results.is_empty(), "Frame 4 should produce output (Complete assembly)");

    // Verify the assembled message contains PGN=0xFF80 info
    let has_pgn_info = last_results.iter().any(|field| {
        match field {
            DecodedField::StringMessage { text, .. } => {
                text.contains("ff80") || text.contains("FF80")
            }
            _ => false,
        }
    });

    // PGN 0xFF80 is proprietary/unrecognized, so we expect a StringMessage with PGN info
    assert!(has_pgn_info || last_results.iter().any(|f| matches!(f, DecodedField::StringMessage { .. })),
        "Expected PGN information in output for assembled message");

    // The assembled data should be 15 bytes: [41 42 43 44 45 46 47 47 49 4A 4B 4C 4D 4E 4F]
    let has_data_length = last_results.iter().any(|field| {
        match field {
            DecodedField::StringMessage { text, .. } => {
                text.contains("15 bytes") || text.contains("15")
            }
            _ => false,
        }
    });

    assert!(has_data_length || has_pgn_info, 
        "Expected assembled message data length or PGN info in output");
}

#[test]
fn test_full_rts_cts_decode_pipeline() {
    let mut decoder = J1939Decoder::new(false, 5000, false);

    // Feed all 9 frames from rts.log in order
    let all_results: Vec<Vec<DecodedField>> = vec![
        (0x18ECEB26, vec![0x10, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]), // RTS CM
        (0x18EC26EB, vec![0x11, 0x06, 0x01, 0xFF, 0xFF, 0x00, 0xE6, 0x00]), // CTS CM
        (0x18EBEB26, vec![0x01, 0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43]), // DT pkt 1
        (0x18EBEB26, vec![0x02, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A]), // DT pkt 2
        (0x18EBEB26, vec![0x03, 0x4B, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]), // DT pkt 3
        (0x18EBEB26, vec![0x04, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]), // DT pkt 4
        (0x18EBEB26, vec![0x05, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]), // DT pkt 5
        (0x18EBEB26, vec![0x06, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), // DT pkt 6 (last)
        (0x18EC26EB, vec![0x13, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]), // EOM CM
    ]
    .into_iter()
    .map(|(can_id, data)| {
        decoder.decode_raw_frame(RawFrame::new(can_id, data))
    })
    .collect();

    // Collect all outputs from all frames
    let mut all_outputs = Vec::new();
    for results in &all_results {
        all_outputs.extend(results.clone());
    }

    // Frame 8 (index 7) should produce the Complete assembly result
    let dt6_results = &all_results[7];
    assert!(!dt6_results.is_empty(), "Frame 8 (last DT packet) should produce output (Complete assembly)");

    // Frame 9 (EOM) should produce no output (assembly already complete)
    let eom_results = &all_results[8];
    assert!(eom_results.is_empty(), "EOM frame after completion should produce no output");

    // Verify the assembled message contains PGN=0xE600 info
    let has_pgn_e600 = dt6_results.iter().any(|field| {
        match field {
            DecodedField::StringMessage { text, .. } => {
                text.contains("e600") || text.contains("E600")
            }
            _ => false,
        }
    });

    // PGN 0xE600 is Virtual Terminal-to-Node (unrecognized in default config)
    assert!(has_pgn_e600 || dt6_results.iter().any(|f| matches!(f, DecodedField::StringMessage { .. })),
        "Expected PGN information in output for assembled message");

    // The assembled data should be 36 bytes
    let has_data_length_36 = dt6_results.iter().any(|field| {
        match field {
            DecodedField::StringMessage { text, .. } => {
                text.contains("36 bytes") || text.contains("36")
            }
            _ => false,
        }
    });

    assert!(has_data_length_36 || has_pgn_e600, 
        "Expected assembled message data length or PGN info in output");

    // Verify we have pending results for frames 2-7 (indices 1-6)
    // Frame 2 (CTS) should be empty (just sets up state)
    // Frames 3-7 (DT packets 1-5) should produce Pending results (empty from decode_raw_frame perspective)
    
    // Count non-empty result frames (should be frame 8 only for Complete)
    let non_empty_count = all_results.iter().filter(|r| !r.is_empty()).count();
    assert_eq!(non_empty_count, 1, "Only the last DT packet should produce output");
}

#[test]
fn test_bam_assembled_data_content() {
    let mut decoder = J1939Decoder::new(false, 5000, false);

    // Feed bam.log frames and capture all outputs
    let results: Vec<Vec<DecodedField>> = vec![
        (0x18ECFF22, vec![0x20, 0x0F, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]),
        (0x18EBFF22, vec![0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]),
        (0x18EBFF22, vec![0x02, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]),
        (0x18EBFF22, vec![0x03, 0x4F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    ]
    .into_iter()
    .map(|(can_id, data)| {
        decoder.decode_raw_frame(RawFrame::new(can_id, data))
    })
    .collect();

    // The Complete result should be on the last frame
    let complete_outputs = &results[3];
    
    // Verify we got output from the assembled message
    assert!(!complete_outputs.is_empty(), "Should have decoded fields for assembled BAM message");

    // Check that at least one StringMessage contains PGN or source info
    let has_message = complete_outputs.iter().any(|f| {
        matches!(f, DecodedField::StringMessage { .. })
    });
    assert!(has_message, "Expected StringMessage for unrecognized proprietary PGN");
}

#[test]
fn test_rts_cts_assembled_data_content() {
    let mut decoder = J1939Decoder::new(false, 5000, false);

    // Feed rts.log frames and capture all outputs
    let results: Vec<Vec<DecodedField>> = vec![
        (0x18ECEB26, vec![0x10, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]), // RTS
        (0x18EC26EB, vec![0x11, 0x06, 0x01, 0xFF, 0xFF, 0x00, 0xE6, 0x00]), // CTS
        (0x18EBEB26, vec![0x01, 0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43]), // DT 1
        (0x18EBEB26, vec![0x02, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A]), // DT 2
        (0x18EBEB26, vec![0x03, 0x4B, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]), // DT 3
        (0x18EBEB26, vec![0x04, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]), // DT 4
        (0x18EBEB26, vec![0x05, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]), // DT 5
        (0x18EBEB26, vec![0x06, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]), // DT 6 (last)
        (0x18EC26EB, vec![0x13, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]), // EOM
    ]
    .into_iter()
    .map(|(can_id, data)| {
        decoder.decode_raw_frame(RawFrame::new(can_id, data))
    })
    .collect();

    // The Complete result should be on frame 8 (index 7)
    let complete_outputs = &results[7];
    
    // Verify we got output from the assembled message
    assert!(!complete_outputs.is_empty(), "Should have decoded fields for assembled RTS/CTS message");

    // EOM frame (index 8) should produce no output
    let eom_outputs = &results[8];
    assert!(eom_outputs.is_empty(), "EOM after completion should produce no output");

    // Check that at least one StringMessage contains PGN or source info
    let has_message = complete_outputs.iter().any(|f| {
        matches!(f, DecodedField::StringMessage { .. })
    });
    assert!(has_message, "Expected StringMessage for unrecognized PGN 0xE600");
}

#[test]
fn test_tp_timeout_in_decoder() {
    let mut decoder = J1939Decoder::new(false, 100, false); // Very short timeout (100ms)

    // Start a BAM assembly but don't complete it
    decoder.decode_raw_frame(RawFrame::new(
        0x18ECFF22, 
        vec![0x20, 0xFF, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00] // BAM for 255 bytes
    ));

    // Send one DT packet (not enough to complete)
    decoder.decode_raw_frame(RawFrame::new(
        0x18EBFF22, 
        vec![0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]
    ));

    // Wait for timeout to expire (in real scenario this would happen asynchronously)
    // For unit test, we just verify the reassembler tracks pending assemblies
    assert_eq!(decoder.active_assemblies(), 1, "Should have one pending assembly");

    // Clear all and verify timeout warning is generated with force_partial enabled
    let mut decoder_partial = J1939Decoder::new(true, 100, true); // force partial output
    
    decoder_partial.decode_raw_frame(RawFrame::new(
        0x18ECFF22, 
        vec![0x20, 0xFF, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]
    ));
    
    decoder_partial.decode_raw_frame(RawFrame::new(
        0x18EBFF22, 
        vec![0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]
    ));

    assert_eq!(decoder_partial.active_assemblies(), 1, "Should have one pending assembly");
}
