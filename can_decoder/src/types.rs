use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a single raw CAN frame received from the bus.
#[derive(Debug, Clone)]
pub struct RawFrame {
    /// Microseconds since Unix epoch.
    pub timestamp: u64,
    /// 11-bit or 29-bit CAN identifier.
    pub can_id: u32,
    /// CAN data bytes (max 8 for standard frames).
    pub data: Vec<u8>,
}

impl RawFrame {
    /// Create a new RawFrame with the current timestamp.
    pub fn new(can_id: u32, data: Vec<u8>) -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        RawFrame {
            timestamp: duration.as_micros() as u64,
            can_id,
            data,
        }
    }
}

/// Represents a fully assembled logical CAN message after multi-frame reassembly.
#[derive(Debug, Clone)]
pub struct AssembledMessage {
    /// Protocol Group Number extracted from the CAN ID.
    pub pgn: PGN,
    /// Source address of the ECU that sent this message.
    pub source_address: u8,
    /// Destination address (0xFF = broadcast).
    pub destination_address: u8,
    /// Assembled payload data (may exceed 8 bytes for TP messages).
    pub data: Vec<u8>,
    /// Microseconds since Unix epoch when this message was assembled.
    pub timestamp: u64,
}

impl AssembledMessage {
    /// Create a new AssembledMessage with the current timestamp and broadcast destination.
    pub fn new(pgn: PGN, source_address: u8, data: Vec<u8>) -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        AssembledMessage {
            pgn,
            source_address,
            destination_address: 0xFF,
            data,
            timestamp: duration.as_micros() as u64,
        }
    }
}

/// J1939 Protocol Group Number parsed from a 29-bit CAN ID.
/// Format matches j1939-async: [priority:3 bits@26][PGN:18 bits@8][source:8 bits@0]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PGN {
    /// Message priority (0-7), from bits 26-28 of CAN ID.
    pub priority: u8,
    /// Protocol Group Number (18 bits), from bits 8-25 of CAN ID.
    pub pgn: u32,
}

impl PGN {
    /// Extract PGN fields from a 29-bit extended CAN ID.
    /// Format matches j1939-async crate in this workspace.
    pub fn from_can_id(can_id: u32) -> Self {
        let priority = ((can_id >> 26) & 0x7) as u8;
        let pgn = (can_id >> 8) & 0x3FFFF;
        PGN { priority, pgn }
    }

    /// Reconstruct a 29-bit CAN ID from this PGN.
    pub fn to_u32(&self) -> u32 {
        (self.priority as u32) << 26 | self.pgn << 8
    }
}

/// The primary output type emitted by the decoder pipeline.
#[derive(Debug, Clone)]
pub enum DecodedField {
    /// A numeric value with optional unit and precision info.
    Value {
        title: String,
        value: Numeric,
        unit: Option<String>,
        decimal_places: Option<u8>,
    },
    /// A text message (status, warning, error) from a device.
    StringMessage { severity: Severity, text: String },
    /// A flag/switch state (on/off/error/unavailable).
    Flag { title: String, value: FlagValue },
}

/// Numeric values that can be decoded from CAN data.
#[derive(Debug, Clone, PartialEq)]
pub enum Numeric {
    /// Signed integer.
    Int(i64),
    /// Floating point number.
    Float(f64),
    /// Raw hex bytes (e.g., VIN, serial number).
    Hex(Vec<u8>),
    /// Boolean value.
    Bool(bool),
}

/// Severity levels for diagnostic and status messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Standard flag states per J1939 conventions.
#[derive(Debug, Clone, PartialEq)]
pub enum FlagValue {
    Off = 0,
    On = 1,
    Error = 2,
    Unavailable = 3,
}

impl From<u8> for FlagValue {
    /// Convert a raw byte to FlagValue. Unknown values default to Unavailable.
    fn from(value: u8) -> Self {
        match value {
            0 => FlagValue::Off,
            1 => FlagValue::On,
            2 => FlagValue::Error,
            3 => FlagValue::Unavailable,
            _ => FlagValue::Unavailable,
        }
    }
}

/// Errors that can occur during the decoding process.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Invalid payload length: expected {expected}, found {found}")]
    InvalidLength { expected: usize, found: usize },
    #[error("Malformed data: {0}")]
    MalformedData(String),
    #[error("Unknown PGN: {0:#x}")]
    UnknownPgn(u32),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// The context provided to every decoder call.
#[derive(Debug, Clone)]
pub struct DecodeContext {
    pub pgn: PGN,
    pub priority: u8,
    pub src_addr: u8,
    pub dest_addr: u8,
    pub src_name: Option<u64>,  // Optional: 64-bit J1939 NAME
    pub dest_name: Option<u64>, // Optional: 64-bit J1939 NAME
    pub timestamp: u64,
}

/// A command returned by a decoder to tell the DeviceManager
/// what to change in the system state.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceUpdate {
    pub target_name: u64,
    pub param_id: u16,
    pub value: Numeric,
}

/// The final result of a decoding operation.
#[derive(Debug, Clone)]
pub struct DecodedMessage {
    pub title: String,
    pub outputs: Vec<DecodedField>,
    pub updates: Vec<DeviceUpdate>,
}

impl DecodedMessage {
    pub fn new(title: String) -> Self {
        DecodedMessage {
            title,
            outputs: Vec::new(),
            updates: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn rawframe_new_creates_correct_can_id_and_data() {
        let frame = RawFrame::new(0x18EF4000, vec![0x01, 0x02, 0x03]);
        assert_eq!(frame.can_id, 0x18EF4000);
        assert_eq!(frame.data, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn rawframe_new_has_current_timestamp() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let frame = RawFrame::new(0x123, vec![0xAA]);
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        assert!(frame.timestamp >= before);
        assert!(frame.timestamp <= after + 100_000);
    }

    #[test]
    fn rawframe_new_empty_data() {
        let frame = RawFrame::new(0x0, vec![]);
        assert_eq!(frame.can_id, 0x0);
        assert!(frame.data.is_empty());
    }

    #[test]
    fn pgn_from_can_id_standard_j1939() {
        let can_id = 0x18EF4000;
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, ((can_id >> 26) & 7) as u8);
        assert_eq!(pgn.pgn, (can_id >> 8) & 0x3FFFF);
    }

    #[test]
    fn pgn_from_can_id_priority_0_pgn_0() {
        let can_id = 0x00000000;
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, 0);
        assert_eq!(pgn.pgn, 0);
    }

    #[test]
    fn pgn_from_can_id_priority_7_pgn_max() {
        let can_id = (7u32 << 26) | (0x3FFFF << 8);
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, 7);
        assert_eq!(pgn.pgn, 0x3FFFF);
    }

    #[test]
    fn pgn_from_can_id_various_priorities() {
        let test_cases = [
            ((2u32 << 26) | (0x1000 << 8), 2),
            ((4u32 << 26) | (0x2000 << 8), 4),
            ((6u32 << 26) | (0x3000 << 8), 6),
        ];
        for (can_id, expected_priority) in test_cases {
            let pgn = PGN::from_can_id(can_id);
            assert_eq!(
                pgn.priority, expected_priority,
                "priority mismatch for can_id={:#x}",
                can_id
            );
        }
    }

    #[test]
    fn pgn_to_u32_roundtrip() {
        let test_ids = [
            0x18EF4000u32,
            0x00000000u32,
            (6u32 << 26) | (0xCB00 << 8),
            0x0A000000u32,
        ];
        for &can_id in &test_ids {
            let pgn = PGN::from_can_id(can_id);
            assert_eq!(
                pgn.to_u32(),
                can_id,
                "roundtrip failed for can_id={:#x}",
                can_id
            );
        }
    }

    #[test]
    fn pgn_to_u32_preserves_fields() {
        let pgn = PGN {
            priority: 4,
            pgn: 0xDEADBEEF & 0x3FFFF,
        };
        let reconstructed = PGN::from_can_id(pgn.to_u32());
        assert_eq!(reconstructed.priority, pgn.priority);
        assert_eq!(reconstructed.pgn, pgn.pgn);
    }

    #[test]
    fn pgn_pgn_is_18_bits() {
        let all_set = PGN::from_can_id(0xFFFFFFFF);
        assert!(all_set.pgn <= 0x3FFFF, "pgn should fit in 18 bits");
    }

    #[test]
    fn pgn_from_can_id_clears_upper_bits() {
        let can_id = 0x29EF4000;
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, ((can_id >> 26) & 0x7) as u8);
    }

    #[test]
    fn pgn_from_can_id_priority_3() {
        let can_id = (3u32 << 26) | (0x1000 << 8);
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, 3);
    }

    #[test]
    fn pgn_from_can_id_all_zero() {
        let can_id = 0x0;
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, 0);
        assert_eq!(pgn.pgn, 0);
    }

    #[test]
    fn pgn_to_u32_zero() {
        let pgn = PGN {
            priority: 0,
            pgn: 0,
        };
        assert_eq!(pgn.to_u32(), 0);
    }

    #[test]
    fn pgn_to_u32_max_priority_pgn() {
        let pgn = PGN {
            priority: 7,
            pgn: 0x1FFFFF,
        };
        assert_eq!(pgn.to_u32(), (7 << 26) | (0x1FFFFF << 8));
    }

    #[test]
    fn flagvalue_clone_preserves_value() {
        let fv = FlagValue::Error;
        let cloned = fv.clone();
        assert_eq!(cloned, FlagValue::Error);
    }

    #[test]
    fn flagvalue_partial_eq() {
        assert_eq!(FlagValue::Off, FlagValue::from(0));
        assert_eq!(FlagValue::On, FlagValue::from(1));
        assert_ne!(FlagValue::Off, FlagValue::On);
    }

    #[test]
    fn numeric_clone_preserves_value() {
        let v = Numeric::Int(42);
        let cloned = v.clone();
        assert_eq!(cloned, Numeric::Int(42));

        let f = Numeric::Float(3.14);
        let cloned_f = f.clone();
        assert_eq!(cloned_f, Numeric::Float(3.14));

        let h = Numeric::Hex(vec![0xAA, 0xBB]);
        let cloned_h = h.clone();
        assert_eq!(cloned_h, Numeric::Hex(vec![0xAA, 0xBB]));

        let b = Numeric::Bool(true);
        let cloned_b = b.clone();
        assert_eq!(cloned_b, Numeric::Bool(true));
    }

    #[test]
    fn decoded_field_value_variant() {
        let output = DecodedField::Value {
            title: "Speed".to_string(),
            value: Numeric::Float(55.5),
            unit: Some("km/h".to_string()),
            decimal_places: Some(1),
        };
        match output {
            DecodedField::Value {
                ref title,
                ref value,
                ref unit,
                decimal_places,
            } => {
                assert_eq!(title, "Speed");
                assert_eq!(value, &Numeric::Float(55.5));
                assert_eq!(unit.as_ref(), Some(&"km/h".to_string()));
                assert_eq!(decimal_places, Some(1));
            }
            _ => panic!("Expected Value variant"),
        }
    }

    #[test]
    fn decoded_field_string_message_variant() {
        let output = DecodedField::StringMessage {
            severity: Severity::Warning,
            text: "Engine fault".to_string(),
        };
        match output {
            DecodedField::StringMessage { severity, ref text } => {
                assert_eq!(severity, Severity::Warning);
                assert_eq!(text, "Engine fault");
            }
            _ => panic!("Expected StringMessage variant"),
        }
    }

    #[test]
    fn decoded_field_flag_variant() {
        let output = DecodedField::Flag {
            title: "Status".to_string(),
            value: FlagValue::On,
        };
        match output {
            DecodedField::Flag { ref title, value } => {
                assert_eq!(title, "Status");
                assert_eq!(value, FlagValue::On);
            }
            _ => panic!("Expected Flag variant"),
        }
    }

    #[test]
    fn severity_partial_eq() {
        assert_eq!(Severity::Info, Severity::Info);
        assert_eq!(Severity::Warning, Severity::Warning);
        assert_eq!(Severity::Error, Severity::Error);
        assert_ne!(Severity::Info, Severity::Warning);
        assert_ne!(Severity::Warning, Severity::Error);
    }

    #[test]
    fn rawframe_new_data_sized() {
        let frame = RawFrame::new(0x123, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(frame.data.len(), 8);

        let frame_short = RawFrame::new(0x123, vec![0xFF]);
        assert_eq!(frame_short.data.len(), 1);
    }

    #[test]
    fn assembled_message_new_timestamp() {
        let pgn = PGN::from_can_id(0x18EF4000);
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let msg = AssembledMessage::new(pgn, 0x20, vec![0x01]);
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        assert!(msg.timestamp >= before.as_micros() as u64);
        assert!(msg.timestamp <= after.as_micros() as u64 + 100_000);
    }

    #[test]
    fn pgn_matches_j1939_async_format() {
        // Verify format matches j1939-async construction: (priority<<26)|(pgn<<8)|source
        let priority = 4u32;
        let pgn_val = 0xEE00u32;
        let source = 0xF8u32;

        // Build CAN ID the same way j1939-async does
        let can_id = (priority << 26) | (pgn_val << 8) | source;

        // Extract should give back original values
        let pgn = PGN::from_can_id(can_id);
        assert_eq!(pgn.priority, priority as u8);
        assert_eq!(pgn.pgn, pgn_val);
    }

    #[test]
    fn flagvalue_unknown_defaults_to_unavailable() {
        for i in 4..=255u8 {
            assert_eq!(FlagValue::from(i), FlagValue::Unavailable);
        }
    }
}
