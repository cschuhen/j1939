use crate::proprietary::ddi::canot;
use crate::traits::ComplexDecoder;
use crate::types::{
    create_topic_id, DecodeContext, DecodeError, DecodedField, DecodedInfo, Numeric, Severity,
    TopicId,
};

/// J1939 ISO-11783-10 Task Controller Process Data PGN (51968 / 0xCB00).
pub const PROCESS_DATA_PGN: u32 = 0x00cb00;

use iso11783_data::strings::task_controller_ddi::{lookup as ddi_lookup, to_physical};

/// Represents a proprietary DDI handler with lookup and conversion functions.
struct ProprietaryHandler {
    lookup_fn: fn(u16) -> Option<&'static iso11783_data::strings::task_controller_ddi::DdiInfo>,
    to_physical_fn: fn(u16, i32) -> Option<f64>,
}

impl ProprietaryHandler {
    fn new_canot() -> Self {
        ProprietaryHandler {
            lookup_fn: canot::lookup,
            to_physical_fn: canot::to_physical,
        }
    }
}

/// Resolve proprietary handler names to actual handlers.
fn resolve_proprietary_handlers(names: &[String]) -> Vec<ProprietaryHandler> {
    let mut handlers = Vec::new();
    for name in names {
        match name.as_str() {
            "canot" => handlers.push(ProprietaryHandler::new_canot()),
            _ => {}
        }
    }
    handlers
}

/// Command values for TaskController Process Data messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCommand {
    TechnicalCapabilities = 0x0,
    DeviceDescriptor = 0x1,
    RequestValue = 0x2,
    Value = 0x3,
    MeasurementTimeInterval = 0x4,
    MeasurementDistanceInterval = 0x5,
    MeasurementMinimumWithinThreshold = 0x6,
    MeasurementMaximumWithinThreshold = 0x7,
    MeasurementChangeThreshold = 0x8,
    PeerControlAssignment = 0x9,
    SetValueAndAcknowledge = 0xA,
    Reserved1 = 0xB,
    Reserved2 = 0xC,
    ProcessDataAcknowledge = 0xD,
    StatusMessage = 0xE,
    ClientTask = 0xF,
}

impl From<u8> for TaskCommand {
    fn from(value: u8) -> Self {
        match value & 0x0F {
            0x0 => TaskCommand::TechnicalCapabilities,
            0x1 => TaskCommand::DeviceDescriptor,
            0x2 => TaskCommand::RequestValue,
            0x3 => TaskCommand::Value,
            0x4 => TaskCommand::MeasurementTimeInterval,
            0x5 => TaskCommand::MeasurementDistanceInterval,
            0x6 => TaskCommand::MeasurementMinimumWithinThreshold,
            0x7 => TaskCommand::MeasurementMaximumWithinThreshold,
            0x8 => TaskCommand::MeasurementChangeThreshold,
            0x9 => TaskCommand::PeerControlAssignment,
            0xA => TaskCommand::SetValueAndAcknowledge,
            0xB => TaskCommand::Reserved1,
            0xC => TaskCommand::Reserved2,
            0xD => TaskCommand::ProcessDataAcknowledge,
            0xE => TaskCommand::StatusMessage,
            0xF => TaskCommand::ClientTask,
            _ => unreachable!(), // Handled by masking with 0x0F
        }
    }
}

/// Parsed TaskController Process Data message fields.
#[derive(Debug, Clone)]
pub struct TaskControllerData {
    pub command: TaskCommand,
    pub element_id: u16,
    pub ddi: u16,
    pub value: i32,
}

impl TaskControllerData {
    fn topic_id(&self) -> TopicId {
        let sub_topic =
            (self.element_id << 4) as u32 | self.command as u32 | (self.ddi as u32) << 16;

        create_topic_id(PROCESS_DATA_PGN, sub_topic)
    }
}

/// ComplexDecoder for ISO-11783-10 Annex B.3 Process Data messages (PGN 51968).
/// Parses Element ID, DDI, and Value fields from 8-byte payloads.
pub struct TaskControllerDecoder {
    /// Track last seen element IDs for sequence detection.
    last_elements: Vec<u16>,
    /// Proprietary DDI handlers to use for DDIs in the proprietary range (0xE000-0xFFFE).
    proprietary_handlers: Vec<ProprietaryHandler>,
}

impl TaskControllerDecoder {
    pub fn new() -> Self {
        TaskControllerDecoder {
            last_elements: Vec::new(),
            proprietary_handlers: Vec::new(),
        }
    }

    /// Create a new TaskControllerDecoder with the given proprietary handler names.
    pub fn with_proprietary_handlers(handler_names: &[String]) -> Self {
        TaskControllerDecoder {
            last_elements: Vec::new(),
            proprietary_handlers: resolve_proprietary_handlers(handler_names),
        }
    }

    /// Parse the 8-byte payload into TaskControllerData fields.
    /// Returns DecodeError::InvalidLength if payload is shorter than 8 bytes.
    pub fn parse_payload(payload: &[u8]) -> Result<TaskControllerData, DecodeError> {
        if payload.len() < 8 {
            return Err(DecodeError::InvalidLength {
                expected: 8,
                found: payload.len(),
            });
        }

        let command_byte = payload[0];
        let command = TaskCommand::from(command_byte);
        let element_id = (command_byte >> 4) as u16;
        let ddi = u16::from_le_bytes([payload[2], payload[3]]);
        let value = i32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);

        Ok(TaskControllerData {
            command,
            element_id,
            ddi,
            value,
        })
    }

    /// Decode a TaskController Process Data message.
    fn decode_value_command(
        data: &TaskControllerData,
        proprietary_handlers: &[ProprietaryHandler],
    ) -> DecodedInfo {
        let ddi_info = Self::resolve_ddi_info(data.ddi, proprietary_handlers);

        let mut msg = DecodedInfo::new("TC Value".into(), data.topic_id());

        // Field 1: Element = element_id (no unit)
        msg.outputs.push(DecodedField::Value {
            title: "Element".to_string(),
            value: Numeric::Int(data.element_id as i64),
            unit: None,
            decimal_places: None,
        });

        // Field 2: DDI = ddi number (no unit)
        msg.outputs.push(DecodedField::Value {
            title: "DDI".to_string(),
            value: Numeric::Int(data.ddi as i64),
            unit: None,
            decimal_places: None,
        });

        // Field 3: Value converted into physical units (if available)
        if let Some(info) = &ddi_info {
            // Field 4: DDI name (e.g., "Total Charge") = physical_value with unit
            if let Some(physical) =
                Self::resolve_physical(data.ddi, data.value, proprietary_handlers)
            {
                msg.outputs.push(DecodedField::Value {
                    title: info.name.to_string(),
                    value: Numeric::Float(physical),
                    unit: info.unit.map(|u| u.to_string()),
                    decimal_places: Some(2),
                });
            } else {
                msg.outputs.push(DecodedField::Value {
                    title: info.name.to_string(),
                    value: Numeric::Int(data.value as i64),
                    unit: info.unit.map(|u| u.to_string()),
                    decimal_places: None,
                });
            }
        } else if data.ddi >= 0xE000 && data.ddi <= 0xFFFE {
            // Unknown proprietary DDI - RAW is the only value available
            msg.outputs.push(DecodedField::Value {
                title: "Unknown Proprietary".to_string(),
                value: Numeric::Int(data.value as i64),
                unit: None,
                decimal_places: None,
            });
        } else {
            // Unknown DDI - Value is the only value available
            msg.outputs.push(DecodedField::Value {
                title: "Unknown".to_string(),
                value: Numeric::Int(data.value as i64),
                unit: None,
                decimal_places: None,
            });
        }

        // Field 4: RAW = raw i32_value (no unit)
        msg.outputs.push(DecodedField::Value {
            title: "RAW".to_string(),
            value: Numeric::Int(data.value as i64),
            unit: None,
            decimal_places: None,
        });

        msg
    }

    /// Resolve DDI info from proprietary handlers first (if in proprietary range), then standard lookup.
    fn resolve_ddi_info(
        ddi: u16,
        proprietary_handlers: &[ProprietaryHandler],
    ) -> Option<&'static iso11783_data::strings::task_controller_ddi::DdiInfo> {
        if ddi >= 0xE000 && ddi <= 0xFFFE {
            for handler in proprietary_handlers {
                if let Some(info) = (handler.lookup_fn)(ddi) {
                    return Some(info);
                }
            }
        }

        ddi_lookup(ddi)
    }

    /// Resolve physical value using proprietary handlers first (if in proprietary range), then standard lookup.
    fn resolve_physical(
        ddi: u16,
        raw_value: i32,
        proprietary_handlers: &[ProprietaryHandler],
    ) -> Option<f64> {
        if ddi >= 0xE000 && ddi <= 0xFFFE {
            for handler in proprietary_handlers {
                if let Some(physical) = (handler.to_physical_fn)(ddi, raw_value) {
                    return Some(physical);
                }
            }
        }

        to_physical(ddi, raw_value)
    }

    /// Decode an unrecognized command into a warning message.
    fn decode_unknown_command(data: &TaskControllerData) -> DecodedInfo {
        let title = format!("TaskController Unknown Command 0x{:X}", data.command as u8);

        DecodedInfo::new(title.clone(), data.topic_id())
    }
}

impl Default for TaskControllerDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexDecoder for TaskControllerDecoder {
    fn decode(
        &mut self,
        _context: &DecodeContext,
        payload: &[u8],
    ) -> Result<Option<DecodedInfo>, DecodeError> {
        let data = Self::parse_payload(payload)?;

        match &data.command {
            TaskCommand::Value => {
                let msg = Self::decode_value_command(&data, &self.proprietary_handlers);

                self.last_elements.push(data.element_id);

                Ok(Some(msg))
            }
            _ => {
                let mut msg = Self::decode_unknown_command(&data);

                msg.outputs.push(DecodedField::StringMessage {
                    severity: Severity::Warning,
                    text: format!(
                        "TaskController command=0x{:02X} element={} DDI={} value={}",
                        data.command as u8, data.element_id, data.ddi, data.value
                    ),
                });

                Ok(Some(msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ComplexDecoder;

    fn make_dummy_context() -> DecodeContext {
        DecodeContext {
            pgn: PROCESS_DATA_PGN,
            priority: 7,
            src_addr: 0x90,
            dest_addr: 0xFF,
            src_name: None,
            dest_name: None,
            timestamp: 1_000_000,
        }
    }

    // ========================================================================
    // Payload Parsing Tests (6c.1)
    // ========================================================================

    #[test]
    fn test_parse_payload_element_10() {
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0xB2, 0xB2, 0x88, 0x9B];
        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();

        assert_eq!(data.command, TaskCommand::Value);
        assert_eq!(data.element_id, 10);
        assert_eq!(data.ddi, 0xE000);
        assert_eq!(data.value, -1685540174i32);
    }

    #[test]
    fn test_parse_payload_element_11() {
        let payload = vec![0xB3, 0x00, 0x00, 0xE0, 0xB3, 0xB8, 0x7E, 0x9B];
        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();

        assert_eq!(data.command, TaskCommand::Value);
        assert_eq!(data.element_id, 11);
        assert_eq!(data.ddi, 0xE000);
        assert_eq!(data.value, -1686193997i32);
    }

    #[test]
    fn test_parse_payload_element_12() {
        let payload = vec![0xC3, 0x00, 0x00, 0xE0, 0x10, 0x12, 0x7E, 0x9B];
        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();

        assert_eq!(data.command, TaskCommand::Value);
        assert_eq!(data.element_id, 12);
        assert_eq!(data.ddi, 0xE000);
        assert_eq!(data.value, -1686236656i32);
    }

    #[test]
    fn test_parse_payload_element_13() {
        let payload = vec![0xD3, 0x00, 0x00, 0xE0, 0x5F, 0x99, 0x76, 0x9B];
        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();

        assert_eq!(data.command, TaskCommand::Value);
        assert_eq!(data.element_id, 13);
        assert_eq!(data.ddi, 0xE000);
        assert_eq!(data.value, -1686726305i32);
    }

    #[test]
    fn test_parse_payload_all_candump_frames() {
        let frames = [
            (vec![0xA3, 0x00, 0x00, 0xE0, 0xB2, 0xB2, 0x88, 0x9B], 10),
            (vec![0xB3, 0x00, 0x00, 0xE0, 0xB3, 0xB8, 0x7E, 0x9B], 11),
            (vec![0xC3, 0x00, 0x00, 0xE0, 0x10, 0x12, 0x7E, 0x9B], 12),
            (vec![0xD3, 0x00, 0x00, 0xE0, 0x5F, 0x99, 0x76, 0x9B], 13),
        ];

        for (payload, expected_elem) in &frames {
            let data = TaskControllerDecoder::parse_payload(payload).unwrap();
            assert_eq!(data.element_id, *expected_elem);
            assert_eq!(data.command, TaskCommand::Value);
            assert_eq!(data.ddi, 0xE000);
        }
    }

    // ========================================================================
    // Short Payload Tests (6c.2)
    // ========================================================================

    #[test]
    fn test_parse_payload_too_short() {
        let payload = vec![0xA3, 0x00, 0x00];
        let result = TaskControllerDecoder::parse_payload(&payload);
        assert!(result.is_err());

        match result.unwrap_err() {
            DecodeError::InvalidLength { expected, found } => {
                assert_eq!(expected, 8);
                assert_eq!(found, 3);
            }
            _ => panic!("Expected InvalidLength error"),
        }
    }

    #[test]
    fn test_parse_payload_empty() {
        let payload = vec![];
        let result = TaskControllerDecoder::parse_payload(&payload);
        assert!(result.is_err());

        match result.unwrap_err() {
            DecodeError::InvalidLength { expected, found } => {
                assert_eq!(expected, 8);
                assert_eq!(found, 0);
            }
            _ => panic!("Expected InvalidLength error"),
        }
    }

    #[test]
    fn test_parse_payload_seven_bytes() {
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0xB2, 0xB2, 0x88];
        let result = TaskControllerDecoder::parse_payload(&payload);
        assert!(result.is_err());

        match result.unwrap_err() {
            DecodeError::InvalidLength { expected, found } => {
                assert_eq!(expected, 8);
                assert_eq!(found, 7);
            }
            _ => panic!("Expected InvalidLength error"),
        }
    }

    // ========================================================================
    // Command Dispatch Tests (6c.3)
    // ========================================================================

    #[test]
    fn test_decode_value_command() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0xB2, 0xB2, 0x88, 0x9B];

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        assert_eq!(msg.title, "TC Value");
        assert_eq!(msg.outputs.len(), 4);

        match &msg.outputs[0] {
            DecodedField::Value {
                title, value, unit, ..
            } => {
                assert_eq!(title, "Element");
                assert_eq!(*value, Numeric::Int(10));
                assert!(unit.is_none());
            }
            _ => panic!("Expected Value for Element"),
        }

        match &msg.outputs[1] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "DDI");
                if let Numeric::Int(v) = value {
                    assert_eq!(*v, 0xE000i64);
                } else {
                    panic!("Expected Int for DDI");
                }
            }
            _ => panic!("Expected Value for DDI"),
        }

        // Without proprietary handlers, standard lookup returns "65534 Proprietary DDI Range"
        match &msg.outputs[2] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "65534 Proprietary DDI Range");
                if let Numeric::Float(v) = value {
                    assert!((v - 0.0).abs() < 1.0, "Expected ~0.0, got {}", v);
                } else {
                    panic!("Expected Float for DDI info name");
                }
            }
            _ => panic!("Expected Value for DDI info"),
        }

        match &msg.outputs[3] {
            DecodedField::Value { title, .. } => {
                assert_eq!(title, "RAW");
            }
            _ => panic!("Expected Value for RAW"),
        }
    }

    #[test]
    fn test_decode_unknown_command_zero() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();
        // Command 0x0: element=15, command=0
        let payload = vec![0xF0, 0x00, 0x00, 0xE0, 0x00, 0x00, 0x00, 0x00];

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        assert_eq!(msg.title, "TaskController Unknown Command 0x0");
        assert_eq!(msg.outputs.len(), 1);

        match &msg.outputs[0] {
            DecodedField::StringMessage { severity, text } => {
                assert_eq!(*severity, Severity::Warning);
                assert!(text.contains("command=0x00"));
                assert!(text.contains("element=15"));
            }
            _ => panic!("Expected StringMessage for unknown command"),
        }
    }

    #[test]
    fn test_decode_unknown_command_four() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();
        // Command 0x4: element=12, command=4
        let payload = vec![0xC4, 0x05, 0x00, 0xE0, 0x00, 0x00, 0x00, 0x00];

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        assert_eq!(msg.title, "TaskController Unknown Command 0x4");
        match &msg.outputs[0] {
            DecodedField::StringMessage { text, .. } => {
                assert!(text.contains("command=0x04"));
                assert!(text.contains("element=12"));
            }
            _ => panic!("Expected StringMessage"),
        }
    }

    #[test]
    fn test_decode_all_commands_produce_output() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();

        for cmd in 0..=15u8 {
            let payload = vec![
                (cmd << 4) | cmd, // upper nibble = element, lower nibble = command
                0x00,
                0x00,
                0xE0,
                0x00,
                0x00,
                0x00,
                0x00,
            ];

            let result = decoder.decode(&context, &payload);
            assert!(result.is_ok(), "Command 0x{:X} should not error", cmd);
            let msg = result.unwrap();
            assert!(msg.is_some(), "Command 0x{:X} should produce output", cmd);
        }
    }

    // ========================================================================
    // Element ID Extraction Tests
    // ========================================================================

    #[test]
    fn test_element_id_extraction_all_nibbles() {
        let expected_elements = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

        for (i, &expected) in expected_elements.iter().enumerate() {
            let payload = vec![
                (i as u8) << 4 | 0x03, // element nibble + command=3
                0x00,
                0x00,
                0xE0,
                0x00,
                0x00,
                0x00,
                0x00,
            ];

            let data = TaskControllerDecoder::parse_payload(&payload).unwrap();
            assert_eq!(data.element_id, expected, "Failed for element {}", expected);
        }
    }

    // ========================================================================
    // DDI Value Tests
    // ========================================================================

    #[test]
    fn test_ddi_various_values() {
        let ddis = [0xE000u16, 0xD3D8, 0x9240, 0x0000, 0xFFFF];

        for &ddi in &ddis {
            let payload = vec![
                0xA3, // element=10, command=3
                0x00,
                (ddi & 0xFF) as u8,
                ((ddi >> 8) & 0xFF) as u8,
                0x00,
                0x00,
                0x00,
                0x00,
            ];

            let data = TaskControllerDecoder::parse_payload(&payload).unwrap();
            assert_eq!(data.ddi, ddi, "Failed for DDI {:#06X}", ddi);
        }
    }

    // ========================================================================
    // Value Field Tests (signed i32 LE)
    // ========================================================================

    #[test]
    fn test_value_positive() {
        let payload = vec![
            0xA3, // element=10, command=3
            0x00, 0x00, 0xE0, 0x64, 0x00, 0x00, 0x00, // value = 100 (little-endian)
        ];

        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();
        assert_eq!(data.value, 100i32);
    }

    #[test]
    fn test_value_negative() {
        // -1 in two's complement: 0xFFFFFFFF
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF];

        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();
        assert_eq!(data.value, -1i32);
    }

    #[test]
    fn test_value_max_i32() {
        // i32::MAX = 0x7FFFFFFF
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0x7F];

        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();
        assert_eq!(data.value, i32::MAX);
    }

    #[test]
    fn test_value_min_i32() {
        // i32::MIN = 0x80000000
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0x00, 0x00, 0x00, 0x80];

        let data = TaskControllerDecoder::parse_payload(&payload).unwrap();
        assert_eq!(data.value, i32::MIN);
    }

    // ========================================================================
    // ComplexDecoder Trait Integration Tests
    // ========================================================================

    #[test]
    fn test_complex_decoder_trait_integration() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();
        let payload = vec![0xA3, 0x00, 0x00, 0xE0, 0x64, 0x00, 0x00, 0x00];

        // Should return Ok(Some(msg)) for valid payload
        let result = decoder.decode(&context, &payload);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_complex_decoder_short_payload_returns_error() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();
        let payload = vec![0xA3, 0x00];

        let result = decoder.decode(&context, &payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 8,
                found: 2,
            } => {}
            other => panic!("Expected InvalidLength(8,2), got {:?}", other),
        }
    }

    #[test]
    fn test_decoder_tracks_elements() {
        let mut decoder = TaskControllerDecoder::new();
        let context = make_dummy_context();

        // Decode element 10
        let payload_10 = vec![0xA3, 0x00, 0x00, 0xE0, 0x64, 0x00, 0x00, 0x00];
        decoder.decode(&context, &payload_10).unwrap();

        // Decode element 11
        let payload_11 = vec![0xB3, 0x00, 0x00, 0xE0, 0x65, 0x00, 0x00, 0x00];
        decoder.decode(&context, &payload_11).unwrap();

        // Decode element 12
        let payload_12 = vec![0xC3, 0x00, 0x00, 0xE0, 0x66, 0x00, 0x00, 0x00];
        decoder.decode(&context, &payload_12).unwrap();

        assert_eq!(decoder.last_elements.len(), 3);
        assert_eq!(decoder.last_elements[0], 10);
        assert_eq!(decoder.last_elements[1], 11);
        assert_eq!(decoder.last_elements[2], 12);
    }

    #[test]
    fn test_process_data_pgn_constant() {
        assert_eq!(PROCESS_DATA_PGN, 0x00cb00);
        assert_eq!(PROCESS_DATA_PGN, 51968u32);
    }
}
