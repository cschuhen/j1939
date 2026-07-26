use crate::traits::ComplexDecoder;
use crate::types::{DecodeContext, DecodeError, DecodedField, DecodedInfo, Numeric};
use iso11783_data::constants::pgn::REQUEST;

/// J1939 PGN for Request messages (PGN 0xEA00 / 59904).
pub const REQUEST_PGN: u32 = REQUEST;

/// ComplexDecoder for ISO 11783 Request PGN (59904 / 0xEA00).
/// Parses a 3-byte payload containing the requested PGN in little-endian order.
pub struct RequestDecoder {
    /// Track last seen requested PGNs for sequence detection.
    last_requests: Vec<u32>,
}

impl RequestDecoder {
    pub fn new() -> Self {
        RequestDecoder {
            last_requests: Vec::new(),
        }
    }

    /// Parse the 3-byte payload into a requested PGN value.
    /// Returns DecodeError::InvalidLength if payload is shorter than 3 bytes.
    /// Byte order matches TP RTS format: little-endian (byte0=low, byte1=mid, byte2=high).
    pub fn parse_payload(payload: &[u8]) -> Result<u32, DecodeError> {
        if payload.len() < 3 {
            return Err(DecodeError::InvalidLength {
                expected: 3,
                found: payload.len(),
            });
        }

        // Little-endian byte order (same as TP RTS PGN bytes)
        let requested_pgn =
            ((payload[2] as u32) << 16) | ((payload[1] as u32) << 8) | (payload[0] as u32);

        Ok(requested_pgn)
    }
}

impl Default for RequestDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexDecoder for RequestDecoder {
    fn decode(
        &mut self,
        _context: &DecodeContext,
        payload: &[u8],
    ) -> Result<Option<DecodedInfo>, DecodeError> {
        let requested_pgn = Self::parse_payload(payload)?;

        self.last_requests.push(requested_pgn);

        let pgn_title: String = {
            let title = iso11783_data::strings::pgn::lookup(requested_pgn);
            match title {
                None => format!("PGN 0x{:X}", requested_pgn),
                Some(t) => t.to_string(),
            }
        };

        let mut msg = DecodedInfo::new("Request".into());

        msg.outputs.push(DecodedField::Value {
            title: "Requested PGN".to_string(),
            value: Numeric::Int(requested_pgn as i64),
            unit: Some("PGN".to_string()),
            decimal_places: None,
        });

        msg.outputs.push(DecodedField::StringMessage {
            severity: crate::types::Severity::Info,
            text: pgn_title,
        });

        Ok(Some(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ComplexDecoder;

    fn make_dummy_context() -> DecodeContext {
        DecodeContext {
            pgn: REQUEST_PGN,
            priority: 6,
            src_addr: 0x20,
            dest_addr: 0xFF,
            src_name: None,
            dest_name: None,
            timestamp: 1_000_000,
        }
    }

    // ========================================================================
    // Payload Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_payload_pgn_0ea00() {
        let payload = vec![0x00u8, 0xEA, 0x00]; // PGN 0xEA00 in little-endian
        let pgn = RequestDecoder::parse_payload(&payload).unwrap();
        assert_eq!(pgn, 0xEA00);
    }

    #[test]
    fn test_parse_payload_pgn_0ec00() {
        let payload = vec![0x00u8, 0xEC, 0x00]; // PGN 0xEC00 in little-endian
        let pgn = RequestDecoder::parse_payload(&payload).unwrap();
        assert_eq!(pgn, 0xEC00);
    }

    #[test]
    fn test_parse_payload_pgn_0cf00() {
        let payload = vec![0x00u8, 0xCF, 0x00]; // PGN 0xCF00 (Vehicle Speed)
        let pgn = RequestDecoder::parse_payload(&payload).unwrap();
        assert_eq!(pgn, 0xCF00);
    }

    #[test]
    fn test_parse_payload_pgn_0000() {
        let payload = vec![0x00u8, 0x00, 0x00];
        let pgn = RequestDecoder::parse_payload(&payload).unwrap();
        assert_eq!(pgn, 0x0000);
    }

    #[test]
    fn test_parse_payload_pgn_max_18bit() {
        // Max 18-bit PGN: 0x3FFFF = 0xFF, 0xFF, 0x03
        let payload = vec![0xFFu8, 0xFF, 0x03];
        let pgn = RequestDecoder::parse_payload(&payload).unwrap();
        assert_eq!(pgn, 0x3FFFF);
    }

    #[test]
    fn test_parse_payload_too_short() {
        let payload = vec![0x00u8, 0xEA];
        let result = RequestDecoder::parse_payload(&payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 3,
                found: 2,
            } => {}
            other => panic!("Expected InvalidLength(3,2), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_payload_empty() {
        let payload = Vec::<u8>::new();
        let result = RequestDecoder::parse_payload(&payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 3,
                found: 0,
            } => {}
            other => panic!("Expected InvalidLength(3,0), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_payload_one_byte() {
        let payload = vec![0x00u8];
        let result = RequestDecoder::parse_payload(&payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 3,
                found: 1,
            } => {}
            other => panic!("Expected InvalidLength(3,1), got {:?}", other),
        }
    }

    // ========================================================================
    // ComplexDecoder Trait Integration Tests
    // ========================================================================

    #[test]
    fn test_decode_request_pgn() {
        let mut decoder = RequestDecoder::new();
        let context = make_dummy_context();
        let payload = vec![0x00u8, 0xEA, 0x00]; // PGN 0xEA00

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        assert_eq!(msg.title, "Request");
        assert_eq!(msg.outputs.len(), 2);

        match &msg.outputs[0] {
            DecodedField::Value {
                title, value, unit, ..
            } => {
                assert_eq!(title, "Requested PGN");
                assert_eq!(*value, Numeric::Int(0xEA00i64));
                assert_eq!(unit.as_ref(), Some(&"PGN".to_string()));
            }
            _ => panic!("Expected Value for Requested PGN"),
        }

        match &msg.outputs[1] {
            DecodedField::StringMessage { text, .. } => {
                // Should contain the PGN title lookup or hex representation
                assert!(text.contains("0xEA00") || text.len() > 0);
            }
            _ => panic!("Expected StringMessage for PGN title"),
        }
    }

    #[test]
    fn test_decode_request_address_claim_pgn() {
        let mut decoder = RequestDecoder::new();
        let context = make_dummy_context();
        let payload = vec![0x00u8, 0xEC, 0x00]; // PGN 0xEC00 (Address Claim)

        let result = decoder.decode(&context, &payload).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();

        match &msg.outputs[1] {
            DecodedField::StringMessage { text, .. } => {
                // Address Claim should be looked up by iso11783_data
                assert!(!text.is_empty());
            }
            _ => panic!("Expected StringMessage"),
        }
    }

    #[test]
    fn test_decode_short_payload_returns_error() {
        let mut decoder = RequestDecoder::new();
        let context = make_dummy_context();
        let payload = vec![0x00u8, 0xEA];

        let result = decoder.decode(&context, &payload);
        assert!(result.is_err());
        match result.unwrap_err() {
            DecodeError::InvalidLength {
                expected: 3,
                found: 2,
            } => {}
            other => panic!("Expected InvalidLength(3,2), got {:?}", other),
        }
    }

    #[test]
    fn test_decoder_tracks_requests() {
        let mut decoder = RequestDecoder::new();
        let context = make_dummy_context();

        // Decode request for PGN 0xEC00
        let payload_1 = vec![0x00u8, 0xEC, 0x00];
        decoder.decode(&context, &payload_1).unwrap();

        // Decode request for PGN 0xEF00
        let payload_2 = vec![0x00u8, 0xEF, 0x00];
        decoder.decode(&context, &payload_2).unwrap();

        assert_eq!(decoder.last_requests.len(), 2);
        assert_eq!(decoder.last_requests[0], 0xEC00);
        assert_eq!(decoder.last_requests[1], 0xEF00);
    }

    #[test]
    fn test_request_pgn_constant() {
        assert_eq!(REQUEST_PGN, 0x0EA00);
        assert_eq!(REQUEST_PGN, 59904u32);
    }
}
