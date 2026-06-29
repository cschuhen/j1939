#[cfg(test)]
mod tests {
    use can_decoder::types::{
        DecodeContext, DecodeError, DecodedMessage, DeviceUpdate, FlagValue, Numeric, PrettyOutput,
        Severity, PGN,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_decode_context_construction() {
        let pgn = PGN::from_can_id(0x18EF4000);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let context = DecodeContext {
            pgn,
            priority: 6,
            src_addr: 0x20,
            dest_addr: 0x01,
            src_name: None,
            dest_name: None,
            timestamp: now,
        };

        assert_eq!(context.pgn.priority, 6);
        assert_eq!(context.src_addr, 0x20);
        assert_eq!(context.timestamp, now);
    }

    #[test]
    fn test_decoded_message_construction() {
        let title = "Engine Speed".to_string();
        let outputs = vec![PrettyOutput::Value {
            title: "RPM".to_string(),
            value: Numeric::Int(1500),
            unit: Some("rpm".to_string()),
            decimal_places: None,
        }];
        let updates = vec![DeviceUpdate {
            target_name: 0x20,
            param_id: 0x00FF,
            value: Numeric::Int(1500),
        }];

        let msg = DecodedMessage {
            title,
            outputs,
            updates,
        };

        assert_eq!(msg.title, "Engine Speed");
        assert_eq!(msg.outputs.len(), 1);
        assert_eq!(msg.updates.len(), 1);
    }

    #[test]
    fn test_decode_error_variants() {
        let err = DecodeError::InvalidLength {
            expected: 8,
            found: 4,
        };
        assert!(err.to_string().contains("expected 8, found 4"));

        let err_malformed = DecodeError::MalformedData("invalid byte".to_string());
        assert_eq!(err_malformed.to_string(), "Malformed data: invalid byte");
    }
}
