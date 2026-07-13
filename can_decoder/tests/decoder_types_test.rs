#[cfg(test)]
mod tests {
    use can_decoder::types::{
        AssembledMessage, DecodeContext, DecodeError, DecodedField, DecodedMessage, DeviceUpdate, Numeric,
    };
    use j1939_async::can::Id;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_dummy_assembled(pgn: u32, source: u8) -> AssembledMessage {
        let can_id = (3u32 << 26) | (pgn << 8) | source as u32;
        AssembledMessage {
            id: can_id,
            pgn,
            data: vec![],
            timestamp: 1_000_000,
        }
    }

    fn make_dummy_assembled_with_data(pgn: u32, source: u8, data: Vec<u8>) -> AssembledMessage {
        let can_id = (3u32 << 26) | (pgn << 8) | source as u32;
        AssembledMessage {
            id: can_id,
            pgn,
            data,
            timestamp: 1_000_000,
        }
    }

    fn make_dummy_assembled_with_dest(pgn: u32, source: u8, dest: u8) -> AssembledMessage {
        let can_id = (3u32 << 26) | (pgn << 16) | ((dest as u32) << 8) | source as u32;
        AssembledMessage {
            id: can_id,
            pgn,
            data: vec![],
            timestamp: 1_000_000,
        }
    }

    fn make_dummy_assembled_with_dest_data(pgn: u32, source: u8, dest: u8, data: Vec<u8>) -> AssembledMessage {
        let can_id = (3u32 << 26) | (pgn << 16) | ((dest as u32) << 8) | source as u32;
        AssembledMessage {
            id: can_id,
            pgn,
            data,
            timestamp: 1_000_000,
        }
    }

    #[test]
    fn test_decode_context_construction() {
        let id = j1939_async::can::IdImpl::new_unchecked(0x18EF4000);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let context = DecodeContext {
            pgn: id.pgn(),
            priority: 6,
            src_addr: 0x20,
            dest_addr: 0x01,
            src_name: None,
            dest_name: None,
            timestamp: now,
        };

        assert_eq!(context.priority, 6);
        assert_eq!(context.src_addr, 0x20);
        assert_eq!(context.timestamp, now);
    }

    #[test]
    fn test_decoded_message_construction() {
        let assembled = make_dummy_assembled_with_data(0xEF4, 0x20, vec![0x01, 0x02]);
        let title = "Engine Speed".to_string();
        let outputs = vec![DecodedField::Value {
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
            assembled_message: assembled,
        };

        assert_eq!(msg.pgn(), 0xEF4);
        assert_eq!(msg.title, "Engine Speed");
        assert_eq!(msg.outputs.len(), 1);
        assert_eq!(msg.updates.len(), 1);
    }

    #[test]
    fn test_decoded_message_new_constructor() {
        let msg = DecodedMessage::new("Test Message".to_string());

        assert_eq!(msg.pgn(), 0);
        assert_eq!(msg.title, "Test Message");
        assert!(msg.outputs.is_empty());
        assert!(msg.updates.is_empty());
    }

    #[test]
    fn test_decoded_message_pgn_preserved_on_clone() {
        let assembled = make_dummy_assembled(0xFE8D, 0x22);
        let msg = DecodedMessage::with_assembled("Active Faults".to_string(), assembled);
        let cloned = msg.clone();

        assert_eq!(cloned.pgn(), 0xFE8D);
        assert_eq!(cloned.title, "Active Faults");
    }

    #[test]
    fn test_decoded_message_pgn_various_values() {
        let pdu2_id = j1939_async::can::IdImpl::new_unchecked(0x18EF4000);
        let assembled_pdu2 = make_dummy_assembled_with_data(pdu2_id.pgn(), 0x20, vec![0x01]);
        let msg_pdu2 = DecodedMessage::with_assembled("PDU2 Test".to_string(), assembled_pdu2);
        assert_eq!(msg_pdu2.pgn(), pdu2_id.pgn());

        let large_pgn_id = j1939_async::can::IdImpl::new_unchecked(0x18ECFF22);
        let assembled_large = make_dummy_assembled_with_data(large_pgn_id.pgn(), 0x22, vec![0x01]);
        let msg_large = DecodedMessage::with_assembled("Large PGN Test".to_string(), assembled_large);
        assert_eq!(msg_large.pgn(), large_pgn_id.pgn());

        let msg_zero = DecodedMessage::new("Zero PGN Test".to_string());
        assert_eq!(msg_zero.pgn(), 0);

        let max_assembled = make_dummy_assembled_with_data(u32::MAX, 0x20, vec![0x01]);
        let msg_max = DecodedMessage::with_assembled("Max PGN Test".to_string(), max_assembled);
        assert_eq!(msg_max.pgn(), u32::MAX);
    }

    #[test]
    fn test_decoded_message_pgn_with_outputs() {
        let assembled = make_dummy_assembled(0xEF4, 0x20);
        let mut msg = DecodedMessage::with_assembled("Engine Parameters".to_string(), assembled);
        
        msg.outputs.push(DecodedField::Value {
            title: "RPM".to_string(),
            value: Numeric::Int(1500),
            unit: Some("rpm".to_string()),
            decimal_places: None,
        });

        assert_eq!(msg.pgn(), 0xEF4);
        assert_eq!(msg.outputs.len(), 1);
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

    #[test]
    fn test_decoded_message_with_assembled() {
        let assembled = make_dummy_assembled_with_data(0xEF4, 0x20, vec![0x01, 0x02, 0x03, 0x04]);

        let msg = DecodedMessage::with_assembled("Engine Speed".to_string(), assembled.clone());

        assert_eq!(msg.pgn(), 0xEF4);
        assert_eq!(msg.title, "Engine Speed");
        
        assert_eq!(msg.assembled_message.id, assembled.id);
        assert_eq!(msg.assembled_message.pgn, 0xEF4);
        assert_eq!(msg.assembled_message.data, vec![0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_decoded_message_accessor_methods() {
        let assembled = make_dummy_assembled_with_data(0xEF4, 0x20, vec![0xAA, 0xBB]);

        let msg = DecodedMessage::with_assembled("Test".to_string(), assembled);

        assert_eq!(msg.source_address(), 0x20);
        // For PDU1 with ID format (3<<26)|(pgn<<8)|source, dest is in bits 8-15 which overlaps PGN lower byte
        assert_eq!(msg.dest_address(), 0xF4);
        assert_eq!(msg.timestamp(), 1_000_000);
        
        let data = msg.data_bytes();
        assert_eq!(*data, vec![0xAA, 0xBB]);
    }

    #[test]
    fn test_decoded_message_new_constructor_has_empty_assembled() {
        let msg = DecodedMessage::new("No Assembled".to_string());

        assert_eq!(msg.pgn(), 0);
        // ID=0 means PDU format=0, source=0, dest (pdu_specific)=0
        assert_eq!(msg.source_address(), 0x00);
        assert_eq!(msg.dest_address(), 0x00);
        assert_eq!(msg.data_bytes().len(), 0);
    }

    #[test]
    fn test_decoded_message_clone_preserves_assembled() {
        let assembled = make_dummy_assembled_with_data(0xEC00, 0x22, vec![0xFF, 0xFE, 0xFF, 0xFF, 0x03, 0x00, 0x0F, 0x20]);

        let msg = DecodedMessage::with_assembled("Address Claim".to_string(), assembled);
        let cloned = msg.clone();

        assert_eq!(cloned.pgn(), 0xEC00);
        assert_eq!(cloned.title, "Address Claim");
        
        assert_eq!(cloned.assembled_message.id, msg.assembled_message.id);
        assert_eq!(cloned.assembled_message.data.len(), 8);
    }

    #[test]
    fn test_decoded_message_with_empty_data() {
        let assembled = make_dummy_assembled(0, 0x20);

        let msg = DecodedMessage::with_assembled("Empty Data".to_string(), assembled);
        
        assert_eq!(msg.data_bytes().len(), 0);
    }

    #[test]
    fn test_decoded_message_with_large_payload() {
        let large_data: Vec<u8> = (0..250).collect();
        let assembled = make_dummy_assembled_with_data(0xF000, 0x20, large_data);

        let msg = DecodedMessage::with_assembled("Large TP Message".to_string(), assembled);
        
        assert_eq!(msg.data_bytes().len(), 250);
    }

    #[test]
    fn test_decoded_message_source_dest_from_id() {
        // PDU1: source=0x40, dest is in bits 8-15 (overlaps with PGN lower byte = 0x00)
        let assembled = make_dummy_assembled(0x0700, 0x40);

        let msg = DecodedMessage::with_assembled("PDU1 Unicast".to_string(), assembled);
        
        assert_eq!(msg.source_address(), 0x40);
        assert_eq!(msg.dest_address(), 0x00);
    }

    #[test]
    fn test_decoded_message_timestamp_preserved() {
        let mut assembled = make_dummy_assembled(0xEF4, 0x20);
        assembled.timestamp = 9_876_543;

        let msg = DecodedMessage::with_assembled("Timestamp Test".to_string(), assembled);
        
        assert_eq!(msg.timestamp(), 9_876_543);
    }

    #[test]
    fn test_decoded_message_all_accessors() {
        let mut assembled = make_dummy_assembled_with_data(0x1234, 0x55, vec![0xDE, 0xAD]);
        assembled.timestamp = 7_777_777;

        let msg = DecodedMessage::with_assembled("All Accessors".to_string(), assembled);
        
        assert_eq!(msg.pgn(), 0x1234);
        assert_eq!(msg.source_address(), 0x55);
        // PGN lower byte in bits 8-15 = 0x34
        assert_eq!(msg.dest_address(), 0x34);
        assert_eq!(msg.timestamp(), 7_777_777);
        assert_eq!(*msg.data_bytes(), vec![0xDE, 0xAD]);
    }
}
