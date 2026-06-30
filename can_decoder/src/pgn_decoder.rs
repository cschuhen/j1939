use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use serde::Deserialize;

use crate::tp_reassembler::{TpReassembler, TpReassemblyResult};
use crate::traits::Decoder;
use crate::types::{
    AssembledMessage, DecodedField, DecodedMessage, Numeric, RawFrame, Severity, PGN,
};

/// YAML configuration for the PGN decoder engine.
#[derive(Debug, Clone, Deserialize)]
pub struct DecoderConfig {
    pub pgns: HashMap<u32, PgnDefinition>,
}

/// Definition of a single PGN's structure and decoding rules.
#[derive(Debug, Clone, Deserialize)]
pub struct PgnDefinition {
    pub title: String,
    #[serde(default)]
    pub components: Vec<ComponentDefinition>,
    #[serde(default)]
    pub decoder_fn: Option<String>,
}

/// Definition of a single field/component within a PGN.
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentDefinition {
    pub name: String,
    pub offset: usize,
    pub length: u8,
    #[serde(rename = "type")]
    pub data_type: ValueType,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub decimal_places: Option<u8>,
    #[serde(default)]
    pub flags: Option<Vec<FlagDefinition>>,
}

/// Supported data types for component decoding.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum ValueType {
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Float32,
    Hex,
    Bool,
}

fn default_scale() -> f64 {
    1.0
}

/// Definition of a bit-field flag within a component.
#[derive(Debug, Clone, Deserialize)]
pub struct FlagDefinition {
    pub bit: u8,
    pub name: String,
    #[serde(default = "default_on")]
    pub on_value: u8,
}

fn default_on() -> u8 {
    1
}

/// YAML configuration loader for PGN decoder configs.
pub struct DecoderConfigLoader;

impl DecoderConfigLoader {
    /// Load config from a YAML file path.
    pub fn load_from_file(
        path: &std::path::Path,
    ) -> Result<DecoderConfig, Box<dyn std::error::Error + Send + Sync>> {
        let yaml = std::fs::read_to_string(path)?;
        Self::load_from_str(&yaml)
    }

    /// Load config from a YAML string.
    pub fn load_from_str(
        yaml: &str,
    ) -> Result<DecoderConfig, Box<dyn std::error::Error + Send + Sync>> {
        let config: DecoderConfig = serde_yaml::from_str(yaml)?;
        Ok(config)
    }
}

/// Return built-in standard PGN definitions for common J1939 protocols.
pub fn default_pgn_definitions() -> HashMap<u32, PgnDefinition> {
    let mut defs = HashMap::new();

    // PGN 0x0EC00 - Address Claim (ISO 11783-5)
    defs.insert(
        0x0EC00,
        PgnDefinition {
            title: "Address Claim".to_string(),
            components: vec![ComponentDefinition {
                name: "NAME".to_string(),
                offset: 0,
                length: 8,
                data_type: ValueType::Hex,
                scale: 1.0,
                unit: None,
                decimal_places: None,
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0FEF4 - Engine Speed (SAE J1939-71)
    defs.insert(
        0x0FEF4,
        PgnDefinition {
            title: "Engine Speed".to_string(),
            components: vec![ComponentDefinition {
                name: "RPM".to_string(),
                offset: 0,
                length: 2,
                data_type: ValueType::UInt16,
                scale: 0.25,
                unit: Some("rpm".to_string()),
                decimal_places: Some(2),
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0FEF8 - Engine Coolant Temperature (SAE J1939-71)
    defs.insert(
        0x0FEF8,
        PgnDefinition {
            title: "Coolant Temperature".to_string(),
            components: vec![ComponentDefinition {
                name: "Temperature".to_string(),
                offset: 0,
                length: 1,
                data_type: ValueType::Int8,
                scale: 1.0,
                unit: Some("\u{00B0}C".to_string()),
                decimal_places: Some(0),
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0EF00 - Pressure (SAE J1939-71)
    defs.insert(
        0x0EF00,
        PgnDefinition {
            title: "Pressure".to_string(),
            components: vec![ComponentDefinition {
                name: "Pressure".to_string(),
                offset: 0,
                length: 2,
                data_type: ValueType::UInt16,
                scale: 0.1,
                unit: Some("kPa".to_string()),
                decimal_places: Some(1),
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0CF00 - Vehicle Speed (SAE J1939-71)
    defs.insert(
        0x0CF00,
        PgnDefinition {
            title: "Vehicle Speed".to_string(),
            components: vec![ComponentDefinition {
                name: "Speed".to_string(),
                offset: 0,
                length: 1,
                data_type: ValueType::UInt8,
                scale: 1.0,
                unit: Some("km/h".to_string()),
                decimal_places: Some(0),
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0FECC - Engine Oil Pressure (SAE J1939-71)
    defs.insert(
        0x0FECC,
        PgnDefinition {
            title: "Engine Oil Pressure".to_string(),
            components: vec![ComponentDefinition {
                name: "Oil Pressure".to_string(),
                offset: 0,
                length: 2,
                data_type: ValueType::UInt16,
                scale: 0.1,
                unit: Some("kPa".to_string()),
                decimal_places: Some(1),
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0FECE - Engine Oil Temperature (SAE J1939-71)
    defs.insert(
        0x0FECE,
        PgnDefinition {
            title: "Engine Oil Temperature".to_string(),
            components: vec![ComponentDefinition {
                name: "Temperature".to_string(),
                offset: 0,
                length: 1,
                data_type: ValueType::Int8,
                scale: 1.0,
                unit: Some("\u{00B0}C".to_string()),
                decimal_places: Some(0),
                flags: None,
            }],
            decoder_fn: None,
        },
    );

    // PGN 0x0FF00 - GPS Location (SAE J1939-74)
    defs.insert(
        0x0FF00,
        PgnDefinition {
            title: "GPS Location".to_string(),
            components: vec![
                ComponentDefinition {
                    name: "Latitude".to_string(),
                    offset: 0,
                    length: 4,
                    data_type: ValueType::Float32,
                    scale: 1.0,
                    unit: Some("degrees".to_string()),
                    decimal_places: Some(6),
                    flags: None,
                },
                ComponentDefinition {
                    name: "Longitude".to_string(),
                    offset: 4,
                    length: 4,
                    data_type: ValueType::Float32,
                    scale: 1.0,
                    unit: Some("degrees".to_string()),
                    decimal_places: Some(6),
                    flags: None,
                },
            ],
            decoder_fn: None,
        },
    );

    defs
}

/// J1939 decoder that uses YAML configuration to decode PGNs into DecodedField items.
pub struct J1939Decoder {
    /// Combined config (built-in + user-provided).
    pub config: DecoderConfig,
    /// Transport protocol reassembler for multi-frame messages.
    reassembler: TpReassembler,
}

impl J1939Decoder {
    /// Create a new J1939Decoder with default PGN definitions and TP reassembly.
    pub fn new(force_partial_tp: bool, timeout_ms: u64) -> Self {
        let mut defs = default_pgn_definitions();

        // Load user config if available (check common paths)
        let custom_config = Self::try_load_user_config();
        if let Ok(user_cfg) = custom_config {
            for (pgn, def) in user_cfg.pgns {
                defs.insert(pgn, def);
            }
        }

        let config = DecoderConfig { pgns: defs };
        J1939Decoder {
            config,
            reassembler: TpReassembler::new(force_partial_tp, timeout_ms),
        }
    }

    /// Create a J1939Decoder with explicit config.
    pub fn with_config(config: DecoderConfig, force_partial_tp: bool, timeout_ms: u64) -> Self {
        J1939Decoder {
            config,
            reassembler: TpReassembler::new(force_partial_tp, timeout_ms),
        }
    }

    /// Try to load user config from default path.
    fn try_load_user_config() -> Result<DecoderConfig, Box<dyn std::error::Error + Send + Sync>> {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.config/j1939_decoder/config.yaml", home);
        DecoderConfigLoader::load_from_file(std::path::Path::new(&path))
    }

    /// Decode an assembled message into DecodedField items using config definitions.
    pub fn decode_assembled(&self, msg: &AssembledMessage) -> Vec<DecodedField> {
        let pgn_key = msg.pgn.pgn;

        if let Some(pgn_def) = self.config.pgns.get(&pgn_key) {
            return self.decode_components(msg, pgn_def);
        }

        // No definition found - emit raw hex as info message
        vec![DecodedField::StringMessage {
            severity: Severity::Info,
            text: format!(
                "Unrecognized PGN={:#06X} source={:#04X}: {} bytes",
                pgn_key,
                msg.source_address,
                msg.data.len()
            ),
        }]
    }

    /// Decode components of a message based on a PGN definition.
    fn decode_components(
        &self,
        msg: &AssembledMessage,
        pgn_def: &PgnDefinition,
    ) -> Vec<DecodedField> {
        let mut outputs = Vec::new();

        for comp in &pgn_def.components {
            if let Some(output) = Self::decode_component(&msg.data, comp) {
                outputs.push(output);
            }
        }

        // If no components decoded successfully, emit raw data as hex
        if outputs.is_empty() && !msg.data.is_empty() {
            outputs.push(DecodedField::StringMessage {
                severity: Severity::Warning,
                text: format!(
                    "{} PGN={:#06X}: insufficient data for {} components",
                    pgn_def.title,
                    msg.pgn.pgn,
                    pgn_def.components.len()
                ),
            });
        }

        outputs
    }

    /// Decode a single component from raw data bytes.
    pub fn decode_component(data: &[u8], comp: &ComponentDefinition) -> Option<DecodedField> {
        let end = comp.offset + comp.length as usize;

        // Bounds check - return None if data is too short (don't panic)
        if end > data.len() {
            eprintln!(
                "[DECODER] Component '{}' offset={}+length={} exceeds data length={}",
                comp.name,
                comp.offset,
                comp.length,
                data.len()
            );
            return Some(DecodedField::StringMessage {
                severity: Severity::Warning,
                text: format!(
                    "Component '{}': data too short (need {} bytes at offset {}, have {})",
                    comp.name,
                    comp.length,
                    comp.offset,
                    data.len()
                ),
            });
        }

        let slice = &data[comp.offset..end];

        match comp.data_type {
            ValueType::Int8 => {
                let val = slice[0] as i8 as i64;
                let scaled = (val as f64) * comp.scale;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(scaled),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::UInt8 => {
                let val = slice[0] as i64;
                let scaled = (val as f64) * comp.scale;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(scaled),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::Int16 => {
                let val = i16::from_le_bytes([slice[0], slice[1]]) as i64;
                let scaled = (val as f64) * comp.scale;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(scaled),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::UInt16 => {
                let val = u16::from_le_bytes([slice[0], slice[1]]) as i64;
                let scaled = (val as f64) * comp.scale;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(scaled),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::Int32 => {
                let val = i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as i64;
                let scaled = (val as f64) * comp.scale;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(scaled),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::UInt32 => {
                let val = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as i64;
                let scaled = (val as f64) * comp.scale;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(scaled),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::Float32 => {
                let bytes = [slice[0], slice[1], slice[2], slice[3]];
                let bits = u32::from_le_bytes(bytes);
                let val = f32::from_bits(bits) * comp.scale as f32;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Float(val as f64),
                    unit: comp.unit.clone(),
                    decimal_places: comp.decimal_places,
                })
            }
            ValueType::Hex => Some(DecodedField::Value {
                title: comp.name.clone(),
                value: Numeric::Hex(slice.to_vec()),
                unit: None,
                decimal_places: None,
            }),
            ValueType::Bool => {
                let val = slice[0] != 0;
                Some(DecodedField::Value {
                    title: comp.name.clone(),
                    value: Numeric::Bool(val),
                    unit: None,
                    decimal_places: None,
                })
            }
        }
    }

    /// Decode a raw frame directly (for single-frame/broadcast compressed messages).
    pub fn decode_raw_frame(&mut self, frame: RawFrame) -> Vec<DecodedField> {
        let pgn = PGN::from_can_id(frame.can_id);

        if pgn.pgn < 0xEF00 {
            // Broadcast compressed - single frame message
            return self.decode_single_frame(&frame, &pgn);
        }

        // Multi-frame TP - feed to reassembler
        let results = self.reassembler.process_frame(&frame);
        let mut outputs = Vec::new();

        for result in results {
            match result {
                TpReassemblyResult::Complete(assembled) => {
                    outputs.extend(self.decode_assembled(&assembled));
                }
                TpReassemblyResult::Pending => {}
                TpReassemblyResult::Timeout(partial) => {
                    outputs.push(DecodedField::StringMessage {
                        severity: Severity::Warning,
                        text: format!(
                            "TP timeout for PGN={:#06X} source={:#04X}: received {} of {} bytes",
                            partial.pgn.pgn,
                            partial.source_address,
                            partial.data.len(),
                            partial.total_expected
                        ),
                    });
                }
            }
        }

        outputs
    }

    /// Decode a single frame using config definitions.
    fn decode_single_frame(&self, frame: &RawFrame, pgn: &PGN) -> Vec<DecodedField> {
        let pgn_key = pgn.pgn;

        if let Some(_pgn_def) = self.config.pgns.get(&pgn_key) {
            let source_addr = (frame.can_id & 0xFF) as u8;
            let assembled = AssembledMessage {
                pgn: pgn.clone(),
                source_address: source_addr,
                destination_address: 0xFF,
                data: frame.data.clone(),
                timestamp: frame.timestamp,
            };
            return self.decode_assembled(&assembled);
        }

        // No definition - emit raw info
        vec![DecodedField::StringMessage {
            severity: Severity::Info,
            text: format!(
                "PGN={:#08X} source={:#04X}: {} bytes",
                pgn_key,
                (frame.can_id & 0xFF),
                frame.data.len()
            ),
        }]
    }

    /// Get the number of active TP assemblies.
    pub fn active_assemblies(&self) -> usize {
        self.reassembler.active_assemblies_count()
    }
}

impl Decoder for J1939Decoder {
    fn name(&self) -> &str {
        "j1939"
    }

    fn decode(
        &mut self,
        frame: RawFrame,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<DecodedMessage, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let can_id = frame.can_id;
            let outputs = self.decode_raw_frame(frame);
            Ok(DecodedMessage {
                title: format!(
                    "PGN {:X} from {:X}",
                    crate::types::PGN::from_can_id(can_id).pgn,
                    can_id
                ),
                outputs,
                updates: vec![],
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait NumericExt {
        fn unwrap_float(&self) -> f64;
    }

    impl NumericExt for Numeric {
        fn unwrap_float(&self) -> f64 {
            match self {
                Numeric::Float(v) => *v,
                _ => panic!("Expected Float"),
            }
        }
    }

    fn make_assembled(pgn_val: u32, source: u8, data: Vec<u8>) -> AssembledMessage {
        let pgn = PGN::from_can_id((3u32 << 26) | (pgn_val << 8) | source as u32);
        AssembledMessage {
            pgn,
            source_address: source,
            destination_address: 0xFF,
            data,
            timestamp: 1_000_000,
        }
    }

    fn make_frame(can_id: u32, data: &[u8]) -> RawFrame {
        RawFrame {
            timestamp: 1_000_000,
            can_id,
            data: data.to_vec(),
        }
    }

    // ========================================================================
    // YAML Config Loading Tests
    // ========================================================================

    #[test]
    fn test_load_valid_config_from_string() {
        let yaml = r#"
pgns:
  0x1000:
    title: "Test Message"
    components:
      - name: "Field A"
        offset: 0
        length: 2
        type: UInt16
        scale: 0.5
        unit: "volts"
        decimal_places: 1
"#;
        let config = DecoderConfigLoader::load_from_str(yaml).unwrap();
        assert_eq!(config.pgns.len(), 1);
        assert!(config.pgns.contains_key(&0x1000));

        let def = config.pgns.get(&0x1000).unwrap();
        assert_eq!(def.title, "Test Message");
        assert_eq!(def.components.len(), 1);
        assert_eq!(def.components[0].name, "Field A");
        assert_eq!(def.components[0].data_type, ValueType::UInt16);
    }

    #[test]
    fn test_load_invalid_yaml_fails() {
        let yaml = "this is not valid yaml: [[[";
        let result = DecoderConfigLoader::load_from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_empty_config() {
        let yaml = "pgns: {}";
        let config = DecoderConfigLoader::load_from_str(yaml).unwrap();
        assert_eq!(config.pgns.len(), 0);
    }

    // ========================================================================
    // Built-in PGN Definitions Tests
    // ========================================================================

    #[test]
    fn test_default_pgn_definitions_count() {
        let defs = default_pgn_definitions();
        assert!(
            defs.len() >= 8,
            "Should have at least 8 built-in definitions"
        );
    }

    #[test]
    fn test_engine_speed_definition() {
        let defs = default_pgn_definitions();
        let eng_speed = defs.get(&0x0FEF4).unwrap();
        assert_eq!(eng_speed.title, "Engine Speed");
        assert_eq!(eng_speed.components.len(), 1);
        assert_eq!(eng_speed.components[0].name, "RPM");
        assert_eq!(eng_speed.components[0].scale, 0.25);
    }

    #[test]
    fn test_coolant_temp_definition() {
        let defs = default_pgn_definitions();
        let coolant = defs.get(&0x0FEF8).unwrap();
        assert_eq!(coolant.title, "Coolant Temperature");
        assert_eq!(coolant.components[0].data_type, ValueType::Int8);
    }

    #[test]
    fn test_gps_location_definition() {
        let defs = default_pgn_definitions();
        let gps = defs.get(&0x0FF00).unwrap();
        assert_eq!(gps.title, "GPS Location");
        assert_eq!(gps.components.len(), 2);
        assert_eq!(gps.components[0].name, "Latitude");
        assert_eq!(gps.components[1].name, "Longitude");
    }

    // ========================================================================
    // Component Decoding Tests
    // ========================================================================

    #[test]
    fn test_decode_uint8() {
        let data = vec![42u8];
        let comp = ComponentDefinition {
            name: "Test".to_string(),
            offset: 0,
            length: 1,
            data_type: ValueType::UInt8,
            scale: 1.0,
            unit: Some("units".to_string()),
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => assert_eq!(value, Numeric::Float(42.0)),
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_uint16() {
        let data = vec![0x00u8, 0x01]; // 256 in little-endian
        let comp = ComponentDefinition {
            name: "Test".to_string(),
            offset: 0,
            length: 2,
            data_type: ValueType::UInt16,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => assert_eq!(value, Numeric::Float(256.0)),
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_int8_negative() {
        let data = vec![0xF0u8]; // -16 in signed 8-bit
        let comp = ComponentDefinition {
            name: "Temp".to_string(),
            offset: 0,
            length: 1,
            data_type: ValueType::Int8,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => assert_eq!(value, Numeric::Float(-16.0)),
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_float32() {
        // 1.5f32 in IEEE 754 little-endian: 0x00, 0x00, 0xC0, 0x3F (0x3FC00000)
        let data = vec![0x00u8, 0x00, 0xC0, 0x3F];
        let comp = ComponentDefinition {
            name: "Value".to_string(),
            offset: 0,
            length: 4,
            data_type: ValueType::Float32,
            scale: 1.0,
            unit: None,
            decimal_places: Some(1),
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => {
                assert!((value.unwrap_float() - 1.5).abs() < 0.001);
            }
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_hex() {
        let data = vec![0xAAu8, 0xBB, 0xCC];
        let comp = ComponentDefinition {
            name: "Data".to_string(),
            offset: 0,
            length: 3,
            data_type: ValueType::Hex,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => {
                assert_eq!(value, Numeric::Hex(vec![0xAA, 0xBB, 0xCC]))
            }
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_bool_true() {
        let data = vec![0x01u8];
        let comp = ComponentDefinition {
            name: "Flag".to_string(),
            offset: 0,
            length: 1,
            data_type: ValueType::Bool,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => assert_eq!(value, Numeric::Bool(true)),
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_bool_false() {
        let data = vec![0x00u8];
        let comp = ComponentDefinition {
            name: "Flag".to_string(),
            offset: 0,
            length: 1,
            data_type: ValueType::Bool,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, .. } => assert_eq!(value, Numeric::Bool(false)),
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_with_scale() {
        let data = vec![0x64u8, 0x00]; // 100 in little-endian (0x0064)
        let comp = ComponentDefinition {
            name: "Pressure".to_string(),
            offset: 0,
            length: 2,
            data_type: ValueType::UInt16,
            scale: 0.1,
            unit: Some("kPa".to_string()),
            decimal_places: Some(1),
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp).unwrap();
        match result {
            DecodedField::Value { value, unit, .. } => {
                assert_eq!(value, Numeric::Float(10.0));
                assert_eq!(unit, Some("kPa".to_string()));
            }
            _ => panic!("Expected Value"),
        }
    }

    // ========================================================================
    // Bounds Checking Tests
    // ========================================================================

    #[test]
    fn test_decode_short_data_returns_warning() {
        let data = vec![0x01u8]; // Only 1 byte
        let comp = ComponentDefinition {
            name: "LongField".to_string(),
            offset: 0,
            length: 4,
            data_type: ValueType::UInt32,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp);
        assert!(result.is_some());
        match result.unwrap() {
            DecodedField::StringMessage { severity, .. } => assert_eq!(severity, Severity::Warning),
            _ => panic!("Expected StringMessage warning"),
        }
    }

    #[test]
    fn test_decode_offset_beyond_data() {
        let data = vec![0x01u8]; // Only 1 byte
        let comp = ComponentDefinition {
            name: "FarField".to_string(),
            offset: 5,
            length: 2,
            data_type: ValueType::UInt16,
            scale: 1.0,
            unit: None,
            decimal_places: None,
            flags: None,
        };

        let result = J1939Decoder::decode_component(&data, &comp);
        assert!(result.is_some());
    }

    // ========================================================================
    // Assembled Message Decoding Tests
    // ========================================================================

    #[test]
    fn test_decode_engine_speed() {
        let decoder = J1939Decoder::new(false, 1000);

        // RPM = 2500 -> raw value = 2500 / 0.25 = 10000 = 0x2710
        let data = vec![0x10u8, 0x27]; // Little-endian: 10000
        let assembled = make_assembled(0x0FEF4, 0x20, data);

        let outputs = decoder.decode_assembled(&assembled);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::Value {
                title, value, unit, ..
            } => {
                assert_eq!(title, "RPM");
                assert_eq!(*value, Numeric::Float(2500.0));
                assert_eq!(unit.as_ref(), Some(&"rpm".to_string()));
            }
            _ => panic!("Expected Value for RPM"),
        }
    }

    #[test]
    fn test_decode_vehicle_speed() {
        let decoder = J1939Decoder::new(false, 1000);

        // Speed = 60 km/h -> raw value = 60
        let data = vec![0x3Cu8]; // 60 in decimal
        let assembled = make_assembled(0x0CF00, 0xF8, data);

        let outputs = decoder.decode_assembled(&assembled);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::Value {
                title, value, unit, ..
            } => {
                assert_eq!(title, "Speed");
                assert_eq!(*value, Numeric::Float(60.0));
                assert_eq!(unit.as_ref(), Some(&"km/h".to_string()));
            }
            _ => panic!("Expected Value for Speed"),
        }
    }

    #[test]
    fn test_decode_unrecognized_pgn() {
        let decoder = J1939Decoder::new(false, 1000);

        let data = vec![0x01u8, 0x02, 0x03];
        let assembled = make_assembled(0xDEADBEEF & 0x3FFFF, 0x40, data);

        let outputs = decoder.decode_assembled(&assembled);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::StringMessage { text, .. } => assert!(text.contains("Unrecognized PGN")),
            _ => panic!("Expected StringMessage for unrecognized PGN"),
        }
    }

    #[test]
    fn test_decode_empty_data() {
        let decoder = J1939Decoder::new(false, 1000);

        let assembled = make_assembled(0x0FEF4, 0x20, vec![]);
        let outputs = decoder.decode_assembled(&assembled);

        assert!(!outputs.is_empty());
        match &outputs[0] {
            DecodedField::StringMessage { severity, .. } => {
                assert_eq!(*severity, Severity::Warning)
            }
            _ => panic!("Expected StringMessage"),
        }
    }

    // ========================================================================
    // Raw Frame Decoding Tests
    // ========================================================================

    #[test]
    fn test_decode_single_frame_with_config() {
        let mut decoder = J1939Decoder::new(false, 1000);

        // Vehicle speed: 45 km/h
        let can_id = (3u32 << 26) | (0x0CF00 << 8) | 0xF8;
        let frame = make_frame(can_id, &[0x2Du8]); // 45 in decimal

        let outputs = decoder.decode_raw_frame(frame);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Speed");
                assert_eq!(*value, Numeric::Float(45.0));
            }
            _ => panic!("Expected Value"),
        }
    }

    #[test]
    fn test_decode_single_frame_without_config() {
        let mut decoder = J1939Decoder::new(false, 1000);

        // Custom PGN not in built-in definitions
        let can_id = (3u32 << 26) | (0x1234 << 8) | 0xF8;
        let frame = make_frame(can_id, &[0xAA, 0xBB]);

        let outputs = decoder.decode_raw_frame(frame);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::StringMessage { text, .. } => assert!(text.contains("PGN=0x001234")),
            _ => panic!("Expected StringMessage"),
        }
    }

    // ========================================================================
    // Config Merging Tests
    // ========================================================================

    #[test]
    fn test_config_with_custom_override() {
        let yaml = r#"
pgns:
  0x0FEF4:
    title: "Custom Engine Speed"
    components:
      - name: "Custom RPM"
        offset: 0
        length: 2
        type: UInt16
        scale: 1.0
"#;
        let user_config = DecoderConfigLoader::load_from_str(yaml).unwrap();

        let mut defs = default_pgn_definitions();
        for (pgn, def) in user_config.pgns {
            defs.insert(pgn, def);
        }

        let config = DecoderConfig { pgns: defs };
        let decoder = J1939Decoder::with_config(config, false, 1000);

        // The custom definition should override the built-in one
        let data = vec![0x64u8, 0x00]; // 100 in little-endian (0x0064)
        let assembled = make_assembled(0x0FEF4, 0x20, data);
        let outputs = decoder.decode_assembled(&assembled);

        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Custom RPM"); // Custom name overrides built-in
                assert_eq!(*value, Numeric::Float(100.0)); // Scale 1.0 instead of 0.25
            }
            _ => panic!("Expected Value"),
        }
    }

    // ========================================================================
    // J1939Decoder Trait Implementation Tests
    // ========================================================================

    #[test]
    fn test_decoder_name() {
        let decoder = J1939Decoder::new(false, 1000);
        assert_eq!(decoder.name(), "j1939");
    }

    #[test]
    fn test_new_decoder_with_defaults() {
        let decoder = J1939Decoder::new(false, 1000);
        assert!(decoder.config.pgns.len() >= 8);
        assert_eq!(decoder.active_assemblies(), 0);
    }

    #[test]
    fn test_new_decoder_with_force_partial() {
        let decoder = J1939Decoder::new(true, 5000);
        assert!(decoder.config.pgns.len() >= 8);
    }
}
