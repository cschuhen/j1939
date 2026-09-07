use crate::types::DecodedMessage;

/// Composite identity of a message stream: who talks to whom about what.
/// Field order defines the Latest-mode sort order (source, then destination, then topic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LatestKey {
    pub source_address: u8,
    pub destination_address: u8,
    pub topic_id: u64,
}

impl LatestKey {
    /// Build a key from a decoded message's addressing and topic.
    pub fn from_message(msg: &DecodedMessage) -> Self {
        Self {
            source_address: msg.source_address(),
            destination_address: msg.dest_address(),
            topic_id: msg.topic_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssembledMessage, DecodedMessage};

    /// Build a message with the given source address and PF/PS bytes.
    fn make_msg(src: u8, pf_ps: u16, topic: u64) -> DecodedMessage {
        let id = ((pf_ps as u32) << 8) | src as u32;
        let id = (7u32 << 26) | id; // priority 7
        let assembled = AssembledMessage::new(id, vec![], 1000);
        DecodedMessage {
            title: String::new(),
            topic_id: topic,
            outputs: Vec::new(),
            updates: Vec::new(),
            assembled_message: assembled,
        }
    }

    #[test]
    fn test_from_message_extracts_fields() {
        // Key fields must mirror what DecodedMessage reports for the same message.
        let msg = make_msg(10, 0xEF00, 42);
        let key = LatestKey::from_message(&msg);
        assert_eq!(key.source_address, msg.source_address());
        assert_eq!(key.destination_address, msg.dest_address());
        assert_eq!(key.topic_id, 42);

        let msg = make_msg(3, 0xEC05, 7);
        let key = LatestKey::from_message(&msg);
        assert_eq!(key.source_address, msg.source_address());
        assert_eq!(key.destination_address, msg.dest_address());
        assert_eq!(key.topic_id, 7);
    }

    #[test]
    fn test_ordering_source_precedes_destination() {
        let a = LatestKey {
            source_address: 1,
            destination_address: 200,
            topic_id: 9,
        };
        let b = LatestKey {
            source_address: 2,
            destination_address: 1,
            topic_id: 9,
        };
        assert!(a < b);
    }

    #[test]
    fn test_ordering_destination_precedes_topic() {
        let a = LatestKey {
            source_address: 5,
            destination_address: 3,
            topic_id: 900,
        };
        let b = LatestKey {
            source_address: 5,
            destination_address: 4,
            topic_id: 1,
        };
        assert!(a < b);
    }

    #[test]
    fn test_ordering_topic_last() {
        let a = LatestKey {
            source_address: 5,
            destination_address: 3,
            topic_id: 2,
        };
        let b = LatestKey {
            source_address: 5,
            destination_address: 3,
            topic_id: 10,
        };
        assert!(a < b);
    }
}
