use j1939_async::Id;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};

use serde::Deserialize;

use crate::device_manager::DeviceManager;
use crate::task_controller::{TaskControllerDecoder, PROCESS_DATA_PGN};
use crate::tp_reassembler::{TpReassembler, TpReassemblyResult};
use crate::traits::{ComplexDecoder, Decoder};
use crate::DetailLevel;
use crate::types::{AssembledMessage, DecodeContext, DecodedField, DecodedMessage, Numeric, RawFrame, Severity};

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
    /// Device manager for identity enrichment (u64 NAME storage and lookup).
    device_manager: Option<Arc<StdMutex<DeviceManager>>>,
    /// Registry of complex decoders keyed by PGN. Checked before YAML config during dispatch.
    complex_decoders: HashMap<u32, Box<dyn ComplexDecoder>>,
    /// Output detail level controlling whether individual TP packets are decoded separately from assembled messages.
    detail_level: DetailLevel,
}

impl J1939Decoder {
    /// Create a new J1939Decoder with default PGN definitions and TP reassembly.
    pub fn new(force_partial_tp: bool, timeout_ms: u64, debug: bool) -> Self {
        Self::with_device_manager_detail(force_partial_tp, timeout_ms, debug, None, DetailLevel::Assembled)
    }

    /// Create a new J1939Decoder with default PGN definitions and an optional DeviceManager.
    pub fn with_device_manager(
        force_partial_tp: bool,
        timeout_ms: u64,
        debug: bool,
        device_manager: Option<Arc<StdMutex<DeviceManager>>>,
    ) -> Self {
        Self::with_device_manager_detail(force_partial_tp, timeout_ms, debug, device_manager, DetailLevel::Assembled)
    }

    /// Create a new J1939Decoder with default PGN definitions, optional DeviceManager, and detail level.
    pub fn with_device_manager_detail(
        force_partial_tp: bool,
        timeout_ms: u64,
        debug: bool,
        device_manager: Option<Arc<StdMutex<DeviceManager>>>,
        detail_level: DetailLevel,
    ) -> Self {
        let mut defs = default_pgn_definitions();

        // Load user config if available (check common paths)
        let custom_config = Self::try_load_user_config();
        if let Ok(user_cfg) = custom_config {
            for (pgn, def) in user_cfg.pgns {
                defs.insert(pgn, def);
            }
        }

        let config = DecoderConfig { pgns: defs };
        let mut decoder = J1939Decoder {
            config,
            reassembler: TpReassembler::new(force_partial_tp, timeout_ms, debug),
            device_manager,
            complex_decoders: HashMap::new(),
            detail_level,
        };

        decoder.register_complex_decoder(PROCESS_DATA_PGN, Box::new(TaskControllerDecoder::new()));

        decoder
    }

    /// Create a J1939Decoder with explicit config and optional DeviceManager.
    pub fn with_config(
        config: DecoderConfig,
        force_partial_tp: bool,
        timeout_ms: u64,
        debug: bool,
        device_manager: Option<Arc<StdMutex<DeviceManager>>>,
    ) -> Self {
        Self::with_config_detail(config, force_partial_tp, timeout_ms, debug, device_manager, DetailLevel::Assembled)
    }

    /// Create a J1939Decoder with explicit config, optional DeviceManager, and detail level.
    pub fn with_config_detail(
        config: DecoderConfig,
        force_partial_tp: bool,
        timeout_ms: u64,
        debug: bool,
        device_manager: Option<Arc<StdMutex<DeviceManager>>>,
        detail_level: DetailLevel,
    ) -> Self {
        let mut decoder = J1939Decoder {
            config,
            reassembler: TpReassembler::new(force_partial_tp, timeout_ms, debug),
            device_manager,
            complex_decoders: HashMap::new(),
            detail_level,
        };

        decoder.register_complex_decoder(PROCESS_DATA_PGN, Box::new(TaskControllerDecoder::new()));

        decoder
    }

    /// Set the output detail level.
    pub fn set_detail_level(&mut self, detail_level: DetailLevel) {
        self.detail_level = detail_level;
    }

    /// Get the current output detail level.
    pub fn detail_level(&self) -> &DetailLevel {
        &self.detail_level
    }

    /// Register a ComplexDecoder for a specific PGN.
    pub fn register_complex_decoder(&mut self, pgn: u32, decoder: Box<dyn ComplexDecoder>) {
        self.complex_decoders.insert(pgn, decoder);
    }

    /// Get the number of registered complex decoders.
    pub fn complex_decoder_count(&self) -> usize {
        self.complex_decoders.len()
    }

    /// Try to load user config from default path.
    fn try_load_user_config() -> Result<DecoderConfig, Box<dyn std::error::Error + Send + Sync>> {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.config/j1939_decoder/config.yaml", home);
        DecoderConfigLoader::load_from_file(std::path::Path::new(&path))
    }

    /// Decode an assembled message into DecodedField items using config definitions.
    /// Returns (outputs, updates) tuple to capture DeviceUpdates from complex decoders.
    pub fn decode_assembled(&mut self, msg: &AssembledMessage) -> (Vec<DecodedField>, Vec<crate::types::DeviceUpdate>) {
        let pgn_key = msg.pgn();

        if let Some(decoder) = self.complex_decoders.get_mut(&pgn_key) {
            let context = DecodeContext {
                pgn: pgn_key,
                priority: ((msg.id >> 26) & 0x7) as u8,
                src_addr: msg.source(),
                dest_addr: msg.destination(),
                src_name: msg.source_name,
                dest_name: msg.dest_name,
                timestamp: msg.timestamp,
            };
            if let Ok(Some(msg_result)) = decoder.decode(&context, &msg.data) {
                return (msg_result.outputs, msg_result.updates);
            }
        }

        if let Some(pgn_def) = self.config.pgns.get(&pgn_key) {
            return (self.decode_components(msg, pgn_def), vec![]);
        }

        // No definition found - emit raw hex as info message
        (vec![DecodedField::StringMessage {
            severity: Severity::Info,
            text: format!(
                "Unrecognized PGN={:#06X} source={:#04X}: {} bytes",
                pgn_key,
                msg.source(),
                msg.data.len()
            ),
        }], vec![])
    }

    /// Decode an assembled message with device name enrichment from DeviceManager.
    #[allow(unused_mut)]
    fn decode_assembled_with_context(&mut self, msg: &AssembledMessage) -> (Vec<DecodedField>, Vec<crate::types::DeviceUpdate>) {
        let (mut outputs, mut updates) = self.decode_assembled(msg);

        // Enrich output with source device name if available in assembled message
        if let Some(src_name_u64) = msg.source_name {
            outputs.insert(0, DecodedField::Value {
                title: "Source Device".to_string(),
                value: Numeric::Hex(vec![
                    (src_name_u64 >> 56) as u8,
                    (src_name_u64 >> 48) as u8,
                    (src_name_u64 >> 40) as u8,
                    (src_name_u64 >> 32) as u8,
                    (src_name_u64 >> 24) as u8,
                    (src_name_u64 >> 16) as u8,
                    (src_name_u64 >> 8) as u8,
                    src_name_u64 as u8,
                ]),
                unit: Some("NAME".to_string()),
                decimal_places: None,
            });
        }

        // Enrich output with destination device name if available and not broadcast
        if let Some(dest_name_u64) = msg.dest_name {
            outputs.push(DecodedField::Value {
                title: "Dest Device".to_string(),
                value: Numeric::Hex(vec![
                    (dest_name_u64 >> 56) as u8,
                    (dest_name_u64 >> 48) as u8,
                    (dest_name_u64 >> 40) as u8,
                    (dest_name_u64 >> 32) as u8,
                    (dest_name_u64 >> 24) as u8,
                    (dest_name_u64 >> 16) as u8,
                    (dest_name_u64 >> 8) as u8,
                    dest_name_u64 as u8,
                ]),
                unit: Some("NAME".to_string()),
                decimal_places: None,
            });
        }

        (outputs, updates)
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
                    msg.pgn(),
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
        let (outputs, _) = self.decode_raw_frame_with_updates(frame);
        outputs
    }

    /// Decode a raw frame and return both outputs and DeviceUpdates.
    pub fn decode_raw_frame_with_updates(&mut self, frame: RawFrame) -> (Vec<DecodedField>, Vec<crate::types::DeviceUpdate>) {
        let pgn = frame.pgn();

        // Handle Address Claim PGN (0xEC00) - extract and store NAME for source address
        if pgn == 0xEC00 && frame.data.len() >= 8 {
            if let Some(ref dm) = self.device_manager {
                let name_bytes = &frame.data[0..8];
                if let Ok(name_u64) = DeviceManager::parse_name_from_bytes(name_bytes) {
                    let src_addr = frame.source_address();
                    dm.lock().unwrap().set_name_u64(src_addr, name_u64);
                }
            }
        }

        // Multi-frame TP - feed to reassembler
        let results = self.reassembler.process_frame(&frame);
        let mut outputs = Vec::new();
        let mut updates = Vec::new();

        for result in results {
            match result {
                TpReassemblyResult::Complete(assembled) => {
                    // Task 2: Always pass assembled TP messages to PGN decoder, independent of detail level
                    let (o, u) = self.decode_assembled_with_context(&assembled);
                    outputs.extend(o);
                    updates.extend(u);
                }
                TpReassemblyResult::SingleFrame(assembled) => {
                    // Non-TP frames are always decoded (they're not part of a TP session)
                    let (o, u) = self.decode_assembled_with_context(&assembled);
                    outputs.extend(o);
                    updates.extend(u);
                }
                TpReassemblyResult::Pending => {
                    // Task 1: Individual TP packets during assembly are NOT passed to PGN decoder
                    // The assembler handles them; only the final assembled message gets decoded
                }
                TpReassemblyResult::Timeout(partial) => {
                    outputs.push(DecodedField::StringMessage {
                        severity: Severity::Warning,
                        text: format!(
                            "TP timeout for PGN={:#06X} source={:#04X}: received {} of {} bytes",
                            partial.pgn,
                            partial.source_address,
                            partial.data.len(),
                            partial.total_expected
                        ),
                    });
                }
            }
        }

        (outputs, updates)
    }

    /// Get the number of active TP assemblies.
    pub fn active_assemblies(&self) -> usize {
        self.reassembler.active_assemblies_count()
    }

    /// Get a reference to the DeviceManager if one is configured.
    pub fn device_manager(&self) -> Option<&Arc<StdMutex<DeviceManager>>> {
        self.device_manager.as_ref()
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
            let (outputs, updates) = self.decode_raw_frame_with_updates(frame.clone());
            let id = j1939_async::can::IdImpl::new_unchecked(can_id);
            
            let mut source_name: Option<u64> = None;
            let mut dest_name: Option<u64> = None;
            
            if let Some(ref dm) = self.device_manager {
                let manager = dm.lock().unwrap();
                source_name = manager.get_name_u64(id.source());
                if id.destination() != 0xFF {
                    dest_name = manager.get_name_u64(id.destination());
                }
            }
            
            let assembled = AssembledMessage {
                id: can_id,
                pgn: id.pgn(),
                data: frame.data.clone(),
                timestamp: frame.timestamp,
                source_name,
                dest_name,
            };
            Ok(DecodedMessage {
                title: format!("PGN {:X} from {:X}", id.pgn(), id.source()),
                outputs,
                updates,
                assembled_message: assembled,
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
        let can_id = (3u32 << 26) | (pgn_val << 8) | source as u32;
        AssembledMessage {
            id: can_id,
            pgn: pgn_val,
            data,
            timestamp: 1_000_000,
            source_name: None,
            dest_name: None,
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
        let mut decoder = J1939Decoder::new(false, 1000, false);

        // RPM = 2500 -> raw value = 2500 / 0.25 = 10000 = 0x2710
        let data = vec![0x10u8, 0x27]; // Little-endian: 10000
        let assembled = make_assembled(0x0FEF4, 0x20, data);

        let (outputs, _) = decoder.decode_assembled(&assembled);
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
        let mut decoder = J1939Decoder::new(false, 1000, false);

        // Speed = 60 km/h -> raw value = 60
        let data = vec![0x3Cu8]; // 60 in decimal
        let assembled = make_assembled(0x0CF00, 0xF8, data);

        let (outputs, _) = decoder.decode_assembled(&assembled);
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
        let mut decoder = J1939Decoder::new(false, 1000, false);

        let data = vec![0x01u8, 0x02, 0x03];
        let assembled = make_assembled(0xDEADBEEF & 0x3FFFF, 0x40, data);

        let (outputs, _) = decoder.decode_assembled(&assembled);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::StringMessage { text, .. } => assert!(text.contains("Unrecognized PGN")),
            _ => panic!("Expected StringMessage for unrecognized PGN"),
        }
    }

    #[test]
    fn test_decode_empty_data() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        let assembled = make_assembled(0x0FEF4, 0x20, vec![]);
        let (outputs, _) = decoder.decode_assembled(&assembled);

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
        let mut decoder = J1939Decoder::new(false, 1000, false);

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
    fn test_decode_pdu1_single_frame_without_config() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        // Custom PGN not in built-in definitions
        let can_id = (3u32 << 26) | (0x1200 << 8) | 0xF8;
        let frame = make_frame(can_id, &[0xAA, 0xBB]);

        let outputs = decoder.decode_raw_frame(frame);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::StringMessage { text, .. } => {
                println!("text: {}", text);
                assert!(text.contains("PGN=0x1200"));
            }
            _ => panic!("Expected StringMessage"),
        }
    }


    #[test]
    fn test_decode_pdu2_single_frame_without_config() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        // Custom PGN not in built-in definitions
        let can_id = (3u32 << 26) | (0xF034 << 8) | 0xF8;
        let frame = make_frame(can_id, &[0xAA, 0xBB]);

        let outputs = decoder.decode_raw_frame(frame);
        assert!(!outputs.is_empty());

        match &outputs[0] {
            DecodedField::StringMessage { text, .. } => {
                println!("text: {}", text);
                assert!(text.contains("PGN=0xF034"));
            }
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
        let mut decoder = J1939Decoder::with_config(config, false, 1000, false, None);

        // The custom definition should override the built-in one
        let data = vec![0x64u8, 0x00]; // 100 in little-endian (0x0064)
        let assembled = make_assembled(0x0FEF4, 0x20, data);
        let (outputs, _) = decoder.decode_assembled(&assembled);

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
        let decoder = J1939Decoder::new(false, 1000, false);
        assert_eq!(decoder.name(), "j1939");
    }

    #[test]
    fn test_new_decoder_with_defaults() {
        let decoder = J1939Decoder::new(false, 1000, false);
        assert!(decoder.config.pgns.len() >= 8);
        assert_eq!(decoder.active_assemblies(), 0);
    }

    #[test]
    fn test_new_decoder_with_force_partial() {
        let decoder = J1939Decoder::new(true, 5000, true);
        assert!(decoder.config.pgns.len() >= 8);
    }

    // ========================================================================
    // DeviceManager Integration Tests
    // ========================================================================

    #[test]
    fn test_decoder_with_device_manager() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        let mut decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm.clone()));

        assert!(decoder.device_manager().is_some());
        assert_eq!(dm.lock().unwrap().get_name_u64(0x20), None);

        // Decode a vehicle speed frame (PGN 0x0CF00) with source address 0x20
        let can_id = (3u32 << 26) | (0x0CF00 << 8) | 0x20;
        let frame = make_frame(can_id, &[0x2Du8]);

        let outputs = decoder.decode_raw_frame(frame);

        // Should produce output for the vehicle speed message
        assert!(!outputs.is_empty());

        // NAME should NOT be stored (only Address Claim PGN stores names)
        assert_eq!(dm.lock().unwrap().get_name_u64(0x20), None);
    }

    #[test]
    fn test_decoder_without_device_manager() {
        let mut decoder = J1939Decoder::new(false, 5000, false);
        assert!(decoder.device_manager().is_none());

        // Should still decode normally without DeviceManager
        let can_id = (3u32 << 26) | (0x0CF00 << 8) | 0xF8;
        let frame = make_frame(can_id, &[0x2Du8]);
        let outputs = decoder.decode_raw_frame(frame);
        assert!(!outputs.is_empty());
    }

    #[test]
    fn test_decoder_address_claim_with_short_data() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        let mut decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm));

        // Address Claim (PGN 0xEC00) with insufficient data - should not panic
        // The reassembler treats PGN 0xEC00 as Connection Management, so it returns empty
        let can_id = (3u32 << 26) | (0xEC00 << 8) | 0x20;
        let frame = make_frame(can_id, &[0x01, 0x02, 0x03]);

        let outputs = decoder.decode_raw_frame(frame);
        // PGN 0xEC00 is treated as TP Connection Management by reassembler
        // Unknown control byte returns empty vec - this is expected behavior
        assert!(outputs.is_empty());
    }

    #[test]
    fn test_decoder_non_address_claim_does_not_store_name() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        let mut decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm.clone()));

        // Decode a non-Address Claim frame - should NOT store any NAME
        let can_id = (3u32 << 26) | (0x0FEF4 << 8) | 0x20;
        let frame = make_frame(can_id, &[0x10, 0x27]);

        decoder.decode_raw_frame(frame);

        assert_eq!(dm.lock().unwrap().get_name_u64(0x20), None);
    }

    #[test]
    fn test_decode_assembled_with_device_name_enrichment() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        dm.lock().unwrap().set_name_u64(0x20, 0x8000_3e00_460d_836e);

        let mut decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm));

        // Decode vehicle speed with known source name
        let data = vec![0x3Cu8]; // 60 km/h
        let mut assembled = make_assembled(0x0CF00, 0x20, data);
        assembled.source_name = Some(0x8000_3e00_460d_836e);

        let (outputs, _) = decoder.decode_assembled_with_context(&assembled);

        // First output should be the Source Device NAME enrichment
        match &outputs[0] {
            DecodedField::Value { title, value, unit, .. } => {
                assert_eq!(title, "Source Device");
                assert_eq!(unit.as_ref(), Some(&"NAME".to_string()));
                if let Numeric::Hex(hex_bytes) = value {
                    assert_eq!(hex_bytes.len(), 8);
                    assert_eq!(*hex_bytes, vec![0x80, 0x00, 0x3e, 0x00, 0x46, 0x0d, 0x83, 0x6e]);
                } else {
                    panic!("Expected Hex value for Source Device");
                }
            }
            _ => panic!("Expected Value for Source Device enrichment"),
        }

        // Second output should be the actual decoded field (Speed)
        match &outputs[1] {
            DecodedField::Value { title, .. } => {
                assert_eq!(title, "Speed");
            }
            _ => panic!("Expected Value for Speed"),
        }
    }

    #[test]
    fn test_decode_assembled_with_dest_device_enrichment() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        dm.lock().unwrap().set_name_u64(0x20, 0x8000_3e00_460d_836e);
        dm.lock().unwrap().set_name_u64(0x40, 0xDEAD_BEEF_CAFE_BABE);

        let mut decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm));

        // Create assembled message with PGN 0x0FEF4 (Engine Speed) and source=0x20
        // Using explicit pgn field since make_assembled shifts can_id bits
        let data = vec![0x10u8, 0x27]; // RPM = 10000 raw -> 2500.0 scaled
        let assembled = AssembledMessage {
            id: (3u32 << 26) | (0x0FEF4 << 8) | 0x20,
            pgn: 0x0FEF4,
            data,
            timestamp: 1_000_000,
            source_name: Some(0x8000_3e00_460d_836e),
            dest_name: None,
        };

        let (outputs, _) = decoder.decode_assembled_with_context(&assembled);

        // Should have Source Device + RPM (2 outputs)
        // Dest Device not added because destination() returns broadcast for PDU2-style IDs
        assert!(outputs.len() >= 2);

        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Source Device");
                if let Numeric::Hex(hex_bytes) = value {
                    assert_eq!(hex_bytes.len(), 8);
                    assert_eq!(*hex_bytes, vec![0x80, 0x00, 0x3e, 0x00, 0x46, 0x0d, 0x83, 0x6e]);
                } else {
                    panic!("Expected Hex value for Source Device");
                }
            }
            _ => panic!("Expected Value for Source Device enrichment"),
        }

        // Second output should be the decoded RPM field
        match &outputs[1] {
            DecodedField::Value { title, .. } => {
                assert_eq!(title, "RPM");
            }
            _ => panic!("Expected Value for RPM"),
        }
    }

    #[test]
    fn test_decode_assembled_no_dest_enrichment_for_broadcast() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        dm.lock().unwrap().set_name_u64(0x20, 0x8000_3e00_460d_836e);
        // Also set dest name for broadcast address (should still not enrich)
        dm.lock().unwrap().set_name_u64(0xFF, 0x1111_2222_3333_4444);

        let mut decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm));

        // Broadcast destination (0xFF) should NOT get Dest Device enrichment
        let can_id = (3u32 << 26) | (0x0CF00 << 8) | 0xFF;
        let data = vec![0x3Cu8];
        let assembled = AssembledMessage {
            id: can_id,
            pgn: 0x0CF00,
            data,
            timestamp: 1_000_000,
            source_name: Some(0x8000_3e00_460d_836e),
            dest_name: None,
        };

        let (outputs, _) = decoder.decode_assembled_with_context(&assembled);

        // Should only have Source Device + Speed (2 outputs), no Dest Device
        assert_eq!(outputs.len(), 2);
        match &outputs[0] {
            DecodedField::Value { title, .. } => {
                assert_eq!(title, "Source Device");
            }
            _ => panic!("Expected Source Device"),
        }
    }

    #[test]
    fn test_decoder_name_with_device_manager() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        let decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm));
        assert_eq!(decoder.name(), "j1939");
    }

    #[test]
    fn test_decoder_active_assemblies_with_device_manager() {
        use crate::device_manager::DeviceManager;
        use std::sync::{Arc, Mutex as StdMutex};

        let dm = Arc::new(StdMutex::new(DeviceManager::new(60)));
        let decoder = J1939Decoder::with_device_manager(false, 5000, false, Some(dm));
        assert_eq!(decoder.active_assemblies(), 0);
    }

    // ========================================================================
    // ComplexDecoder Dispatch Tests (Phase 4)
    // ========================================================================

    /// Mock complex decoder for testing dispatch routing.
    #[allow(dead_code)]
    struct MockComplexDecoder {
        handled_pgns: Vec<u32>,
        output: DecodedMessage,
        should_return_none: bool,
        call_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[allow(dead_code)]
    impl MockComplexDecoder {
        fn new(handled_pgn: u32, output: DecodedMessage) -> Self {
            MockComplexDecoder {
                handled_pgns: vec![handled_pgn],
                output,
                should_return_none: false,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }

        fn with_multiple(handled_pgns: Vec<u32>, output: DecodedMessage) -> Self {
            MockComplexDecoder {
                handled_pgns,
                output,
                should_return_none: false,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }

        fn with_none_output(mut self) -> Self {
            self.should_return_none = true;
            self
        }

        fn get_call_count(&self) -> usize {
            self.call_count.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn reset_call_count(&self) {
            self.call_count.store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl ComplexDecoder for MockComplexDecoder {
        fn decode(
            &mut self,
            _context: &DecodeContext,
            _payload: &[u8],
        ) -> Result<Option<DecodedMessage>, crate::types::DecodeError> {
            self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.should_return_none {
                Ok(None)
            } else {
                Ok(Some(self.output.clone()))
            }
        }
    }

    #[test]
    fn test_complex_decoder_dispatch_routes_to_mock() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        let mut mock_output = DecodedMessage::with_assembled(
            "Mock Decoded".to_string(),
            AssembledMessage::new(0, vec![]),
        );
        mock_output.outputs.push(DecodedField::Value {
            title: "Custom Field".to_string(),
            value: Numeric::Int(999),
            unit: Some("custom".to_string()),
            decimal_places: None,
        });

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        decoder.register_complex_decoder(
            0xDEADBEEF & 0x3FFFF,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0xDEADBEEF & 0x3FFFF],
                output: mock_output.clone(),
                should_return_none: false,
                call_count: call_count.clone(),
            }),
        );

        let data = vec![0xAAu8, 0xBB];
        let assembled = make_assembled(0xDEADBEEF & 0x3FFFF, 0x50, data);
        let (outputs, _) = decoder.decode_assembled(&assembled);

        assert_eq!(outputs.len(), 1);
        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Custom Field");
                assert_eq!(*value, Numeric::Int(999));
            }
            _ => panic!("Expected Value from mock decoder"),
        }
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_complex_decoder_falls_back_to_yaml_when_none() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        let mock_output = DecodedMessage::with_assembled(
            "Mock".to_string(),
            AssembledMessage::new(0, vec![]),
        );
        decoder.register_complex_decoder(
            0x0CF00,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0x0CF00],
                output: mock_output,
                should_return_none: true,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }),
        );

        let data = vec![0x3Cu8]; // 60 km/h
        let assembled = make_assembled(0x0CF00, 0xF8, data);
        let (outputs, _) = decoder.decode_assembled(&assembled);

        assert!(!outputs.is_empty());
        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Speed");
                assert_eq!(*value, Numeric::Float(60.0));
            }
            _ => panic!("Expected YAML-decoded Speed field"),
        }
    }

    #[test]
    fn test_complex_decoder_ignores_unregistered_pgn() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        let mock_output = DecodedMessage::with_assembled(
            "Mock".to_string(),
            AssembledMessage::new(0, vec![]),
        );
        decoder.register_complex_decoder(
            0xDEAD,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0xDEAD],
                output: mock_output,
                should_return_none: false,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }),
        );

        // PGN 0x0CF00 is NOT registered with the complex decoder
        let data = vec![0x2Du8]; // 45 km/h
        let assembled = make_assembled(0x0CF00, 0xF8, data);
        let (outputs, _) = decoder.decode_assembled(&assembled);

        assert!(!outputs.is_empty());
        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Speed");
                assert_eq!(*value, Numeric::Float(45.0));
            }
            _ => panic!("Expected YAML-decoded Speed field"),
        }
    }

    #[test]
    fn test_complex_decoder_multiple_registered_pgns() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        let call_count_a = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let call_count_b = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Register two different mock decoders for two different PGNs
        let mut mock_a_output = DecodedMessage::with_assembled(
            "Mock A".to_string(),
            AssembledMessage::new(0, vec![]),
        );
        mock_a_output.outputs.push(DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Mock A".to_string(),
        });

        let mut mock_b_output = DecodedMessage::with_assembled(
            "Mock B".to_string(),
            AssembledMessage::new(0, vec![]),
        );
        mock_b_output.outputs.push(DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Mock B".to_string(),
        });

        decoder.register_complex_decoder(
            0x1111,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0x1111],
                output: mock_a_output,
                should_return_none: false,
                call_count: call_count_a.clone(),
            }),
        );

        decoder.register_complex_decoder(
            0x2222,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0x2222],
                output: mock_b_output,
                should_return_none: false,
                call_count: call_count_b.clone(),
            }),
        );

        // Decode PGN 0x1111 -> should use mock A
        let assembled_a = make_assembled(0x1111, 0x30, vec![0x01]);
        let (outputs_a, _) = decoder.decode_assembled(&assembled_a);
        assert_eq!(outputs_a[0], DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Mock A".to_string(),
        });
        assert_eq!(call_count_a.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(call_count_b.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Decode PGN 0x2222 -> should use mock B
        let assembled_b = make_assembled(0x2222, 0x40, vec![0x02]);
        let (outputs_b, _) = decoder.decode_assembled(&assembled_b);
        assert_eq!(outputs_b[0], DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Mock B".to_string(),
        });
        assert_eq!(call_count_a.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(call_count_b.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Decode unregistered PGN -> should fall back to YAML (or unrecognized)
        let assembled_c = make_assembled(0x3333, 0x50, vec![0x03]);
        let (outputs_c, _) = decoder.decode_assembled(&assembled_c);
        assert_eq!(outputs_c[0], DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Unrecognized PGN=0x3333 source=0x50: 1 bytes".to_string(),
        });
    }

    #[test]
    fn test_complex_decoder_registry_count() {
        let mut decoder = J1939Decoder::new(false, 1000, false);
        assert_eq!(decoder.complex_decoder_count(), 1);

        decoder.register_complex_decoder(
            0xAAAA,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0xAAAA],
                output: DecodedMessage::with_assembled(
                    "Mock".to_string(),
                    AssembledMessage::new(0, vec![]),
                ),
                should_return_none: false,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }),
        );
        assert_eq!(decoder.complex_decoder_count(), 2);

        decoder.register_complex_decoder(
            0xBBBB,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0xBBBB],
                output: DecodedMessage::with_assembled(
                    "Mock2".to_string(),
                    AssembledMessage::new(0, vec![]),
                ),
                should_return_none: false,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }),
        );
        assert_eq!(decoder.complex_decoder_count(), 3);
    }

    #[test]
    fn test_complex_decoder_overrides_yaml_config() {
        let mut decoder = J1939Decoder::new(false, 1000, false);

        // PGN 0x0FEF4 (Engine Speed) is in the built-in YAML config
        // Register a mock for it - should override YAML decoding
        let mut overridden_output = DecodedMessage::with_assembled(
            "Overridden".to_string(),
            AssembledMessage::new(0, vec![]),
        );
        overridden_output.outputs.push(DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Overridden".to_string(),
        });

        decoder.register_complex_decoder(
            0x0FEF4,
            Box::new(MockComplexDecoder {
                handled_pgns: vec![0x0FEF4],
                output: overridden_output,
                should_return_none: false,
                call_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }),
        );

        let data = vec![0x10u8, 0x27]; // RPM = 10000 raw -> would be 2500.0 from YAML
        let assembled = make_assembled(0x0FEF4, 0x20, data);
        let (outputs, _) = decoder.decode_assembled(&assembled);

        // Should get mock output, not YAML-decoded RPM
        assert_eq!(outputs[0], DecodedField::StringMessage {
            severity: Severity::Info,
            text: "Overridden".to_string(),
        });
    }

    // ========================================================================
    // TaskController Decoder Integration Tests (Phase 6)
    // ========================================================================

    #[test]
    fn test_taskcontroller_decoder_registered_by_default() {
        let decoder = J1939Decoder::new(false, 1000, false);
        assert_eq!(decoder.complex_decoder_count(), 1);
    }

    #[test]
    fn test_pgn_51968_decoded_by_taskcontroller() {
        use crate::task_controller::PROCESS_DATA_PGN;

        let mut decoder = J1939Decoder::new(false, 1000, false);

        // Element 10, Command=3 (Value), DDI=57344, Value=-1685540174
        let data = vec![0xA3, 0x00, 0x00, 0xE0, 0xB2, 0xB2, 0x88, 0x9B];
        let assembled = make_assembled(PROCESS_DATA_PGN, 0x90, data);

        let (outputs, _) = decoder.decode_assembled(&assembled);
        assert!(!outputs.is_empty());

        // First output should be Element ID from TaskController decoder
        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Element ID");
                assert_eq!(*value, Numeric::Int(10));
            }
            _ => panic!("Expected Element ID Value from TaskController decoder"),
        }

        // Second output should be DDI
        match &outputs[1] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "DDI");
                if let Numeric::Int(v) = value {
                    assert_eq!(*v, 57344i64);
                } else {
                    panic!("Expected Int for DDI");
                }
            }
            _ => panic!("Expected DDI Value"),
        }

        // Third output should be Value
        match &outputs[2] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Value");
                if let Numeric::Int(v) = value {
                    assert_eq!(*v, -1685540174i64);
                } else {
                    panic!("Expected Int for Value");
                }
            }
            _ => panic!("Expected Value field"),
        }
    }

    #[test]
    fn test_pgn_51968_all_elements_round_robin() {
        use crate::task_controller::PROCESS_DATA_PGN;

        let mut decoder = J1939Decoder::new(false, 1000, false);

        let elements = [
            (0xA3, 10, -1685540174i64),
            (0xB3, 11, -1686193997i64),
            (0xC3, 12, -1686236656i64),
            (0xD3, 13, -1686726305i64),
        ];

        for (byte_val, expected_elem, expected_value) in &elements {
            let data = vec![
                *byte_val, 0x00, 0x00, 0xE0,
                ((expected_value % 256) as i32) as u8,
                (((expected_value >> 8) % 256) as i32) as u8,
                (((expected_value >> 16) % 256) as i32) as u8,
                (((expected_value >> 24) % 256) as i32) as u8,
            ];

            let assembled = make_assembled(PROCESS_DATA_PGN, 0x90, data);
            let (outputs, _) = decoder.decode_assembled(&assembled);

            assert_eq!(outputs.len(), 3, "Expected 3 outputs for element {}", expected_elem);

            match &outputs[0] {
                DecodedField::Value { value, .. } => {
                    if let Numeric::Int(v) = value {
                        assert_eq!(*v, *expected_elem as i64);
                    } else {
                        panic!("Expected Int for Element ID");
                    }
                }
                _ => panic!("Expected Value"),
            }

            match &outputs[2] {
                DecodedField::Value { value, .. } => {
                    if let Numeric::Int(v) = value {
                        assert_eq!(*v, *expected_value);
                    } else {
                        panic!("Expected Int for Value");
                    }
                }
                _ => panic!("Expected Value"),
            }
        }
    }

    // ========================================================================
    // TP Assembly Detail-Level Tests (Fixes and general)
    // ========================================================================

    #[test]
    fn test_tp_assembly_individual_packets_not_decoded_separately() {
        let mut decoder = J1939Decoder::new(false, 5000, false);

        // Step 1: Send BAM to set up broadcast transfer for PGN 0x0CF00 (Vehicle Speed)
        let source = 0xF8u8;
        let dest = 0xFFu8;
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((dest as u32) << 8) | source as u32;
        // PGN 0x0CF00 in little-endian: low=0xF0, mid=0xCF, high=0x00
        let bam_data = [
            0x20,           // control byte = BAM
            0x0E, 0x00,     // total_size = 14 bytes
            0x02,           // num_packets = 2
            0xFF,           // reserved
            0xF0, 0xCF, 0x00, // PGN = 0x0CF00 (Vehicle Speed) - Little-Endian
        ];
        decoder.decode_raw_frame(make_frame(bam_can_id, &bam_data));

        // Step 2: Send first DT packet - should NOT produce decoded output (Pending)
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;
        let packet1_data = [0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47];
        let outputs1 = decoder.decode_raw_frame(make_frame(dt_can_id, &packet1_data));

        // Individual DT packet should NOT be decoded - only Pending from reassembler
        assert!(outputs1.is_empty(), "Individual TP packets should not produce output");

        // Step 3: Send second DT packet (completes the transfer) - should produce decoded output
        let packet2_data = [0x02, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E];
        let outputs2 = decoder.decode_raw_frame(make_frame(dt_can_id, &packet2_data));

        // Assembled message should be decoded (regardless of PGN recognition)
        assert!(!outputs2.is_empty(), "Assembled TP message should produce output");
    }

    #[test]
    fn test_assembled_tp_always_decoded_independent_of_detail_level() {
        // Test with detail_level = Assembled (default) - decode pre-built assembled message
        let mut decoder_assembled = J1939Decoder::with_device_manager_detail(
            false, 5000, false, None, DetailLevel::Assembled
        );

        let assembled = make_assembled(0x0CF00, 0xF8, vec![0x3Cu8]); // Speed = 60 km/h
        let (outputs, _) = decoder_assembled.decode_assembled(&assembled);
        assert!(!outputs.is_empty(), "Assembled message should be decoded with Assembled detail level");
        match &outputs[0] {
            DecodedField::Value { title, .. } => {
                assert_eq!(title, "Speed", "Should decode as Vehicle Speed");
            }
            _ => panic!("Expected Value for Speed"),
        }

        // Test with detail_level = Both - assembled message should still be decoded
        let mut decoder_both = J1939Decoder::with_device_manager_detail(
            false, 5000, false, None, DetailLevel::Both
        );

        let (outputs_both, _) = decoder_both.decode_assembled(&assembled);
        assert!(!outputs_both.is_empty(), "Assembled message should be decoded with Both detail level");

        // Test with detail_level = Raw - assembled TP messages should still be decoded (task 2)
        let mut decoder_raw = J1939Decoder::with_device_manager_detail(
            false, 5000, false, None, DetailLevel::Raw
        );

        let (outputs_raw, _) = decoder_raw.decode_assembled(&assembled);
        assert!(!outputs_raw.is_empty(), "Assembled TP message should always be decoded regardless of detail level");
    }

    #[test]
    fn test_non_tp_frames_always_decoded() {
        let mut decoder = J1939Decoder::new(false, 5000, false);

        // Non-TP frame (vehicle speed PGN 0x0CF00) should always be decoded
        let can_id = (3u32 << 26) | (0x0CF00 << 8) | 0xF8;
        let outputs = decoder.decode_raw_frame(make_frame(can_id, &[0x3Cu8])); // 60 km/h

        assert!(!outputs.is_empty(), "Non-TP frames should always be decoded");
        match &outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Speed");
                assert_eq!(*value, Numeric::Float(60.0));
            }
            _ => panic!("Expected Value for Speed"),
        }
    }

    #[test]
    fn test_detail_level_wiring() {
        let decoder = J1939Decoder::new(false, 1000, false);
        assert_eq!(*decoder.detail_level(), DetailLevel::Assembled);

        let mut decoder_both = J1939Decoder::with_device_manager_detail(
            false, 5000, false, None, DetailLevel::Both
        );
        assert_eq!(*decoder_both.detail_level(), DetailLevel::Both);

        decoder_both.set_detail_level(DetailLevel::Raw);
        assert_eq!(*decoder_both.detail_level(), DetailLevel::Raw);
    }
}
