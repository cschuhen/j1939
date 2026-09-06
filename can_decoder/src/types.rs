use j1939_async::Id;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a single raw CAN frame received from the bus.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

    /// Extract source address from a 29-bit J1939 CAN ID.
    pub fn source_address(&self) -> u8 {
        self.source()
    }

    /// Extract destination address from a CAN ID.
    /// For j1939-async format: (priority << 26) | (pgn << 8) | source
    /// Destination defaults to broadcast (0xFF).
    pub fn destination_address(&self) -> u8 {
        self.destination()
    }
}

impl j1939_async::Id for RawFrame {
    fn as_raw(&self) -> u32 {
        self.can_id
    }
}

/// Represents a fully assembled logical CAN message after multi-frame reassembly.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssembledMessage {
    /// CAN ID of the message. For TP messages, this may not match actual on-bus ID's
    pub id: u32,

    /// Protocol Group Number extracted from the payload PGN bytes (not from CAN ID).
    pub pgn: u32,

    /// Assembled payload data (may exceed 8 bytes for TP messages).
    pub data: Vec<u8>,
    /// Microseconds since Unix epoch when this message was assembled.
    pub timestamp: u64,

    /// J1939 NAME of the source device (if known via Address Claim or other means).
    pub source_name: Option<u64>,

    /// J1939 NAME of the destination device (if known and not broadcast).
    pub dest_name: Option<u64>,
}

impl AssembledMessage {
    /// Create a new AssembledMessage with the given CAN ID, data, and timestamp.
    /// Extracts PGN using j1939_async::IdImpl which handles PDU1/PDU2 format correctly.
    pub fn new(id: u32, data: Vec<u8>, timestamp: u64) -> Self {
        let id_impl = j1939_async::can::IdImpl::new_unchecked(id);
        let pgn = id_impl.pgn();
        AssembledMessage {
            id,
            pgn,
            data,
            timestamp,
            source_name: None,
            dest_name: None,
        }
    }

    /// Create a new AssembledMessage with an explicit PGN value and timestamp.
    pub fn with_pgn(id: u32, pgn: u32, data: Vec<u8>, timestamp: u64) -> Self {
        // In testing, check that the PGN supplied is actually valid. Agents
        // keep wanting to use invalid made-up PGN's (like 0xEF40) that are
        // TOTALLY INVALID! They can not be represented in the J1939 address
        // fields.
        #[cfg(test)]
        assert!(j1939_async::can::is_pgn_valid(pgn));
        AssembledMessage {
            id,
            pgn,
            data,
            timestamp,
            source_name: None,
            dest_name: None,
        }
    }

    /// Returns the Protocol Group Number.
    pub fn pgn(&self) -> u32 {
        self.pgn
    }
}

impl j1939_async::Id for AssembledMessage {
    fn as_raw(&self) -> u32 {
        self.id
    }
}

/// J1939 Protocol Group Number parsed from a 29-bit CAN ID.
/// Format matches j1939-async: [priority:3 bits@26][PGN:18 bits@8][source:8 bits@0]
/*#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
}*/

/// The primary output type emitted by the decoder pipeline.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
}

/// Numeric values that can be decoded from CAN data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Numeric {
    /// Signed integer.
    Int(i64),
    /// Floating point number.
    Float(f64),
    /// Raw hex bytes (e.g., VIN, serial number).
    Hex(u64),
    /// Boolean value.
    Bool(bool),
    /// A flag/switch state (on/off/error/unavailable).
    Flag(FlagValue),
}

/// Severity levels for diagnostic and status messages.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Standard flag states per J1939 conventions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FlagValue {
    #[serde(rename = "off")]
    Off = 0,
    #[serde(rename = "on")]
    On = 1,
    #[serde(rename = "error")]
    Error = 2,
    #[serde(rename = "unavailable")]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecodeContext {
    pub pgn: u32,
    pub priority: u8,
    pub src_addr: u8,
    pub dest_addr: u8,
    pub src_name: Option<u64>,  // Optional: 64-bit J1939 NAME
    pub dest_name: Option<u64>, // Optional: 64-bit J1939 NAME
    pub timestamp: u64,
}

/// A command returned by a decoder to tell the DeviceManager
/// what to change in the system state.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeviceUpdate {
    pub target_name: u64,
    pub param_id: u16,
    pub value: Numeric,
}

/// The final result of a decoding operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecodedInfo {
    pub title: String,
    pub topic_id: TopicId,
    pub outputs: Vec<DecodedField>,
    pub updates: Vec<DeviceUpdate>,
}

impl DecodedInfo {
    pub fn new(title: String, topic_id: TopicId) -> Self {
        DecodedInfo {
            title,
            topic_id,
            outputs: Vec::new(),
            updates: Vec::new(),
        }
    }
}

// Topic Id (TopicId)
// Often messages have extra addressing or targt specification in addition to
// the

pub type TopicId = u64;
pub type Pgn = u32;
pub fn topic_id_from_pgn(pgn: Pgn) -> u64 {
    (pgn as TopicId) << 40
}

pub fn topic_id_from_message(am: &AssembledMessage) -> u64 {
    topic_id_from_pgn(am.pgn)
}

pub fn create_topic_id(pgn: u32, sub_topic: u32) -> u64 {
    topic_id_from_pgn(pgn) | sub_topic as u64
}

pub fn create_topic_id_from_message(am: &AssembledMessage, sub_topic: u32) -> u64 {
    create_topic_id(am.pgn, sub_topic)
}

pub fn null_topic_id() -> u64 {
    0u64
}

/// The final result of a decoding operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecodedMessage {
    /// Assembled message containing raw data, timestamp, source/dest addresses, and PGN.
    pub assembled_message: AssembledMessage,
    pub title: String,
    pub topic_id: TopicId,
    pub outputs: Vec<DecodedField>,
    pub updates: Vec<DeviceUpdate>,
}

impl DecodedMessage {
    pub fn new(title: String) -> Self {
        DecodedMessage {
            title,
            topic_id: 0,
            outputs: Vec::new(),
            updates: Vec::new(),
            assembled_message: AssembledMessage::new(0, vec![], 0),
        }
    }

    pub fn create(msg: AssembledMessage, info: DecodedInfo) -> Self {
        DecodedMessage {
            title: info.title,
            outputs: info.outputs,
            topic_id: topic_id_from_pgn(msg.pgn),
            updates: info.updates,
            assembled_message: msg,
        }
    }

    /// Create a DecodedMessage with an assembled message attached.
    pub fn with_assembled(title: String, assembled: AssembledMessage) -> Self {
        DecodedMessage {
            title,
            topic_id: topic_id_from_message(&assembled),
            outputs: Vec::new(),
            updates: Vec::new(),
            assembled_message: assembled,
        }
    }

    /// Get the PGN from the attached assembled message.
    pub fn pgn(&self) -> u32 {
        self.assembled_message.pgn
    }

    /// Get the source address from the attached assembled message.
    pub fn source_address(&self) -> u8 {
        self.assembled_message.source()
    }

    /// Get the destination address from the attached assembled message.
    pub fn dest_address(&self) -> u8 {
        self.assembled_message.destination()
    }

    /// Get the timestamp from the attached assembled message.
    pub fn timestamp(&self) -> u64 {
        self.assembled_message.timestamp
    }

    /// Get the raw data bytes from the attached assembled message.
    pub fn data_bytes(&self) -> &Vec<u8> {
        &self.assembled_message.data
    }

    /// Get the source device NAME if known (from Address Claim or other means).
    pub fn source_name(&self) -> Option<u64> {
        self.assembled_message.source_name
    }

    /// Get the destination device NAME if known and not broadcast.
    pub fn dest_name(&self) -> Option<u64> {
        self.assembled_message.dest_name
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

        let h = Numeric::Hex(0xAABB);
        let cloned_h = h.clone();
        assert_eq!(cloned_h, Numeric::Hex(0xAABB));

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
        let output = DecodedField::Value {
            title: "Status".to_string(),
            value: Numeric::Flag(FlagValue::On),
            unit: None,
            decimal_places: None,
        };
        match output {
            DecodedField::Value {
                ref title,
                value: Numeric::Flag(value),
                ..
            } => {
                assert_eq!(title, "Status");
                assert_eq!(value, FlagValue::On);
            }
            _ => panic!("Expected Value with Numeric::Flag variant"),
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
        let can_id = 0x18EF4000;
        let ts: u64 = 1234567890;
        let msg = AssembledMessage::new(can_id, vec![0x01], ts);

        assert_eq!(msg.timestamp, ts);
    }

    #[test]
    fn flagvalue_unknown_defaults_to_unavailable() {
        for i in 4..=255u8 {
            assert_eq!(FlagValue::from(i), FlagValue::Unavailable);
        }
    }
}
