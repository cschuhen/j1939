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
#[derive(Debug, Clone)]
pub struct PGN {
    /// Message priority (0-7).
    pub priority: u8,
    /// PGN-specific bits (18 bits).
    pub pgn_specific: u32,
    /// Data page bit (0 or 1).
    pub data_page: u8,
}

impl PGN {
    /// Extract PGN fields from a 29-bit extended CAN ID.
    /// J1939 format: [3 bits priority][1 bit data page][8 bits reserved][7 bits PGN extension][8 bits PGN base].
    pub fn from_can_id(can_id: u32) -> Self {
        let priority = ((can_id >> 26) & 0x7) as u8;
        let data_page = ((can_id >> 24) & 0x1) as u8;
        let pgn_specific = can_id & 0x1FFFFF;
        PGN {
            priority,
            pgn_specific,
            data_page,
        }
    }

    /// Reconstruct a 29-bit CAN ID from this PGN.
    pub fn to_u32(&self) -> u32 {
        (self.data_page as u32) << 24
            | (self.priority as u32) << 26
            | self.pgn_specific
    }
}

/// The primary output type emitted by the decoder pipeline.
#[derive(Debug, Clone)]
pub enum PrettyOutput {
    /// A numeric value with optional unit and precision info.
    Value {
        title: String,
        value: Numeric,
        unit: Option<String>,
        decimal_places: Option<u8>,
    },
    /// A text message (status, warning, error) from a device.
    StringMessage {
        severity: Severity,
        text: String,
    },
    /// A flag/switch state (on/off/error/unavailable).
    Flag {
        title: String,
        value: FlagValue,
    },
}

/// Numeric values that can be decoded from CAN data.
#[derive(Debug, Clone)]
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
