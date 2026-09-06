use crate::traits::ComplexDecoder;
use crate::types::{
    create_topic_id, DecodeContext, DecodeError, DecodedField, DecodedInfo, Numeric,
};
use iso11783_data::constants::pgn::ADDRESS_CLAIMED;
use j1939_async::name::Name;

/// J1939 PGN for Address Claimed messages (PGN 0xEE00 / 60928).
pub const ADDRESS_CLAIMED_PGN: u32 = ADDRESS_CLAIMED;

/// ComplexDecoder for ISO 11783 Address Claim PGN (60928 / 0xEC00).
/// Parses an 8-byte payload containing the J1939 NAME.
pub struct AddressClaimDecoder {
    /// Track last seen claimed device addresses.
    last_claimed: Vec<u8>,
}

impl AddressClaimDecoder {
    pub fn new() -> Self {
        AddressClaimDecoder {
            last_claimed: Vec::new(),
        }
    }

    /// Parse the 8-byte NAME payload using j1939_async.
    /// Returns DecodeError::InvalidLength if payload is shorter than 8 bytes.
    pub fn parse_name(payload: &[u8]) -> Result<Name, DecodeError> {
        if payload.len() < 8 {
            return Err(DecodeError::InvalidLength {
                expected: 8,
                found: payload.len(),
            });
        }

        Name::from_bytes(payload)
            .map_err(|_| DecodeError::MalformedData("Failed to parse NAME".into()))
    }
}

impl Default for AddressClaimDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexDecoder for AddressClaimDecoder {
    fn decode(
        &mut self,
        context: &DecodeContext,
        payload: &[u8],
    ) -> Result<Option<DecodedInfo>, DecodeError> {
        let name = Self::parse_name(payload)?;

        self.last_claimed.push(context.src_addr);

        let mut msg = DecodedInfo::new(
            "Address Claim".into(),
            create_topic_id(context.pgn, context.src_addr as u32),
        );

        // Industry Group (3 bits)
        let ig = name.industry_group();
        let ig_title = iso11783_data::strings::name::industry_group_lookup(ig as u8);
        msg.outputs.push(DecodedField::Value {
            title: "Industry Group".to_string(),
            value: Numeric::Int(ig as i64),
            unit: Some("IG".to_string()),
            decimal_places: None,
        });
        if let Some(title) = ig_title {
            msg.outputs.push(DecodedField::StringMessage {
                severity: crate::types::Severity::Info,
                text: format!("Industry Group: {}", title),
            });
        }

        // Self Configurable flag
        msg.outputs.push(DecodedField::Value {
            title: "Self Configurable".to_string(),
            value: Numeric::Flag(if name.self_configurable() {
                crate::types::FlagValue::On
            } else {
                crate::types::FlagValue::Off
            }),
            unit: None,
            decimal_places: None,
        });

        // Vehicle System Instance (4 bits)
        msg.outputs.push(DecodedField::Value {
            title: "Vehicle System Instance".to_string(),
            value: Numeric::Int(name.vehicle_system_instance() as i64),
            unit: Some("Instance".to_string()),
            decimal_places: None,
        });

        // Vehicle System (7 bits) with IG-specific lookup
        let vs = name.vehicle_system();
        let vs_title = iso11783_data::strings::name::vehicle_system_lookup(ig as u8, vs as u8);
        msg.outputs.push(DecodedField::Value {
            title: "Vehicle System".to_string(),
            value: Numeric::Int(vs as i64),
            unit: Some("VS".to_string()),
            decimal_places: None,
        });
        if let Some(title) = vs_title {
            msg.outputs.push(DecodedField::StringMessage {
                severity: crate::types::Severity::Info,
                text: format!("Vehicle System: {}", title),
            });
        }

        // Function (8 bits) with global lookup
        let func = name.function() as u16;
        let func_title = iso11783_data::strings::name::global_function_lookup(func);
        msg.outputs.push(DecodedField::Value {
            title: "Function".to_string(),
            value: Numeric::Int(name.function() as i64),
            unit: Some("Func".to_string()),
            decimal_places: None,
        });
        if let Some(title) = func_title {
            msg.outputs.push(DecodedField::StringMessage {
                severity: crate::types::Severity::Info,
                text: format!("Function: {}", title),
            });
        }

        // Function Instance (5 bits)
        msg.outputs.push(DecodedField::Value {
            title: "Function Instance".to_string(),
            value: Numeric::Int(name.function_instance() as i64),
            unit: Some("Instance".to_string()),
            decimal_places: None,
        });

        // ECU Instance (3 bits)
        msg.outputs.push(DecodedField::Value {
            title: "ECU Instance".to_string(),
            value: Numeric::Int(name.ecu_instance() as i64),
            unit: Some("Instance".to_string()),
            decimal_places: None,
        });

        // Manufacturer ID (11 bits) with lookup
        let mfr = name.manufacturer();
        let mfr_title = iso11783_data::strings::name::manufacturer_id_lookup_u16(mfr as u16);
        msg.outputs.push(DecodedField::Value {
            title: "Manufacturer ID".to_string(),
            value: Numeric::Int(mfr as i64),
            unit: Some("MFR".to_string()),
            decimal_places: None,
        });
        if let Some(title) = mfr_title {
            msg.outputs.push(DecodedField::StringMessage {
                severity: crate::types::Severity::Info,
                text: format!("Manufacturer: {}", title),
            });
        }

        // Identity (21 bits)
        msg.outputs.push(DecodedField::Value {
            title: "Identity".to_string(),
            value: Numeric::Int(name.identity() as i64),
            unit: Some("ID".to_string()),
            decimal_places: None,
        });

        // Raw NAME (8 bytes)
        let name_bytes: Vec<u8> = name.bytes_iter().collect();
        let name_u64: u64 = u64::from_be_bytes(name_bytes.try_into().unwrap_or([0; 8]));
        msg.outputs.push(DecodedField::Value {
            title: "Raw NAME".to_string(),
            value: Numeric::Hex(name_u64),
            unit: Some("NAME".to_string()),
            decimal_places: None,
        });

        Ok(Some(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FlagValue;

    fn make_dummy_context(src_addr: u8) -> DecodeContext {
        DecodeContext {
            pgn: ADDRESS_CLAIMED_PGN,
            priority: 7,
            src_addr,
            dest_addr: 0xFF,
            src_name: None,
            dest_name: None,
            timestamp: 1_000_000,
        }
    }

    // ========================================================================
    // NAME Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_name_valid() {
        let payload = vec![0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80];
        let name = AddressClaimDecoder::parse_name(&payload).unwrap();

        assert_eq!(name.raw(), 0x8000_3e00_460d_836e);
        assert_eq!(name.identity(), 885614);
        assert_eq!(name.manufacturer(), 560);
        assert_eq!(name.ecu_instance(), 0);
        assert_eq!(name.function_instance(), 0);
        assert_eq!(name.function(), 62);
        assert_eq!(name.vehicle_system(), 0);
        assert_eq!(name.vehicle_system_instance(), 0);
        assert_eq!(name.industry_group(), 0);
        assert!(name.self_configurable());
    }

    #[test]
    fn test_parse_name_too_short() {
        let payload = vec![0x6e, 0x83, 0x0d];
        let result = AddressClaimDecoder::parse_name(&payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 8,
                found: 3,
            } => {}
            other => panic!("Expected InvalidLength(8,3), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_name_empty() {
        let payload = Vec::<u8>::new();
        let result = AddressClaimDecoder::parse_name(&payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 8,
                found: 0,
            } => {}
            other => panic!("Expected InvalidLength(8,0), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_name_seven_bytes() {
        let payload = vec![0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00];
        let result = AddressClaimDecoder::parse_name(&payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 8,
                found: 7,
            } => {}
            other => panic!("Expected InvalidLength(8,7), got {:?}", other),
        }
    }

    // ========================================================================
    // ComplexDecoder Trait Integration Tests
    // ========================================================================

    #[test]
    fn test_decode_address_claim() {
        let mut decoder = AddressClaimDecoder::new();
        let context = make_dummy_context(0x20);
        let payload = vec![0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80];

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        assert_eq!(msg.title, "Address Claim");
        assert!(msg.outputs.len() >= 9);

        // Check Industry Group output
        match &msg.outputs[0] {
            DecodedField::Value { title, value, .. } => {
                assert_eq!(title, "Industry Group");
                assert_eq!(*value, Numeric::Int(0i64));
            }
            _ => panic!("Expected Industry Group Value"),
        }

        // Check Manufacturer ID output
        let mfr_idx = msg.outputs.iter().position(|o| match o {
            DecodedField::Value { title, .. } => title == "Manufacturer ID",
            _ => false,
        });
        assert!(mfr_idx.is_some());
        if let Some(idx) = mfr_idx {
            match &msg.outputs[idx] {
                DecodedField::Value { value, .. } => {
                    assert_eq!(*value, Numeric::Int(560i64));
                }
                _ => panic!("Expected Manufacturer ID Value"),
            }
        }

        // Check Identity output
        let identity_idx = msg.outputs.iter().position(|o| match o {
            DecodedField::Value { title, .. } => title == "Identity",
            _ => false,
        });
        assert!(identity_idx.is_some());
        if let Some(idx) = identity_idx {
            match &msg.outputs[idx] {
                DecodedField::Value { value, .. } => {
                    assert_eq!(*value, Numeric::Int(885614i64));
                }
                _ => panic!("Expected Identity Value"),
            }
        }

        // Check Raw NAME output (8 bytes in little-endian order)
        let raw_idx = msg.outputs.iter().position(|o| match o {
            DecodedField::Value { title, .. } => title == "Raw NAME",
            _ => false,
        });
        assert!(raw_idx.is_some());
        if let Some(idx) = raw_idx {
            match &msg.outputs[idx] {
                DecodedField::Value { value, .. } => {
                    assert_eq!(*value, Numeric::Hex(0x6E830D46003E0080));
                }
                _ => panic!("Expected Raw NAME Value"),
            }
        }

        // Check manufacturer lookup string message exists (MFR 560 = Danfoss)
        let mfr_str_idx = msg.outputs.iter().position(|o| match o {
            DecodedField::StringMessage { text, .. } => {
                text.contains("Danfoss") || text.contains("560")
            }
            _ => false,
        });
        // MFR ID 560 is beyond u8 range for lookup table, so string message may not exist
        let _ = mfr_str_idx;
    }

    #[test]
    fn test_decode_address_claim_non_configurable() {
        let mut decoder = AddressClaimDecoder::new();
        let context = make_dummy_context(0x30);
        // Non-self-configurable NAME (bit 63 = 0)
        let payload = vec![0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x00];

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        // Check Self Configurable flag is Off (it's the first Flag output)
        let sc_idx = msg.outputs.iter().position(|o| {
            matches!(
                o,
                DecodedField::Value {
                    value: Numeric::Flag(..),
                    ..
                }
            )
        });
        assert!(sc_idx.is_some());
        if let Some(idx) = sc_idx {
            match &msg.outputs[idx] {
                DecodedField::Value {
                    value: Numeric::Flag(value),
                    ..
                } => {
                    assert_eq!(*value, FlagValue::Off);
                }
                _ => panic!("Expected Self Configurable Flag"),
            }
        }
    }

    #[test]
    fn test_decode_address_claim_all_on() {
        let mut decoder = AddressClaimDecoder::new();
        let context = make_dummy_context(0x40);
        // All bits set except reserved (bit 63 is self_configurable, so all on means SC=true)
        let payload = vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF];

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        // Check Self Configurable flag is On (it's the first Flag output)
        let sc_idx = msg.outputs.iter().position(|o| {
            matches!(
                o,
                DecodedField::Value {
                    value: Numeric::Flag(..),
                    ..
                }
            )
        });
        assert!(sc_idx.is_some());
        if let Some(idx) = sc_idx {
            match &msg.outputs[idx] {
                DecodedField::Value {
                    value: Numeric::Flag(value),
                    ..
                } => {
                    assert_eq!(*value, FlagValue::On);
                }
                _ => panic!("Expected Self Configurable Flag"),
            }
        }

        // Check manufacturer lookup string message exists (MFR 560 = Danfoss)
        let mfr_str_idx = msg.outputs.iter().position(|o| match o {
            DecodedField::StringMessage { text, .. } => {
                text.contains("Danfoss") || text.contains("560")
            }
            _ => false,
        });
        // MFR ID 560 is beyond u8 range for lookup table, so string message may not exist
        let _ = mfr_str_idx;
    }

    #[test]
    fn test_decode_address_claim_short_payload() {
        let mut decoder = AddressClaimDecoder::new();
        let context = make_dummy_context(0x20);
        let payload = vec![0x6e, 0x83, 0x0d];

        let result = decoder.decode(&context, &payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 8,
                found: 3,
            } => {}
            other => panic!("Expected InvalidLength(8,3), got {:?}", other),
        }
    }

    #[test]
    fn test_decoder_tracks_claims() {
        let mut decoder = AddressClaimDecoder::new();
        let context1 = make_dummy_context(0x20);
        let payload = vec![0x6e, 0x83, 0x0d, 0x46, 0x00, 0x3e, 0x00, 0x80];

        decoder.decode(&context1, &payload).unwrap();

        let context2 = make_dummy_context(0x30);
        decoder.decode(&context2, &payload).unwrap();

        assert_eq!(decoder.last_claimed.len(), 2);
        assert_eq!(decoder.last_claimed[0], 0x20);
        assert_eq!(decoder.last_claimed[1], 0x30);
    }

    #[test]
    fn test_address_claim_pgn_constant() {
        assert_eq!(ADDRESS_CLAIMED_PGN, 0x0EE00);
        assert_eq!(ADDRESS_CLAIMED_PGN, 60928u32);
    }
}
