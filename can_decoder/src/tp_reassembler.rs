use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::types::{AssembledMessage, RawFrame, PGN};

/// Type of Transport Protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpMessageType {
    /// Broadcast Compressed Message (PGN < 0xEF00), single frame up to 8 bytes payload.
    BroadcastCompressed,
    /// Connection Management frame (RTS/CTS/EOM).
    ConnectionManagement,
    /// Actual data packet in a Total Message Transfer sequence.
    DataPacket,
}

/// J1939 Transport Protocol variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpProtocol {
    BroadcastCompressed,
    TotalMessageTransfer,
}

/// State of an in-progress multi-frame assembly.
#[derive(Debug, Clone)]
pub struct TpAssemblyState {
    pub source_address: u8,
    pub destination_address: u8,
    pub pgn: PGN,
    pub total_size: usize,
    pub page_number: u8,
    pub packets_received: Vec<(u8, Vec<u8>)>,
    pub start_time: u64,
    pub last_packet_time: u64,
}

/// Result of TP reassembly for a single frame.
#[derive(Debug, Clone)]
pub enum TpReassemblyResult {
    /// Waiting for more packets to complete the message.
    Pending,
    /// Successfully assembled complete message.
    Complete(AssembledMessage),
    /// Timeout exceeded while waiting; contains partial data.
    Timeout(PartialAssembly),
}

/// Partial assembly emitted when --force-output-partial-tp is set.
#[derive(Debug, Clone)]
pub struct PartialAssembly {
    pub source_address: u8,
    pub destination_address: u8,
    pub pgn: PGN,
    pub data: Vec<u8>,
    pub total_expected: usize,
    pub timestamp: u64,
}

/// Errors that can occur during TP reassembly.
#[derive(Debug, Clone)]
pub enum TpError {
    InvalidPacketNumber,
    DuplicatePacket,
    OutOfOrderPacket,
    SizeMismatch {
        expected: usize,
        received: usize,
    },
    Timeout {
        source: u8,
        destination: u8,
        pgn: PGN,
        elapsed_ms: u64,
    },
    CorruptedData(String),
}

impl fmt::Display for TpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TpError::InvalidPacketNumber => write!(f, "Invalid packet number"),
            TpError::DuplicatePacket => write!(f, "Duplicate packet detected"),
            TpError::OutOfOrderPacket => write!(f, "Out-of-order packet received"),
            TpError::SizeMismatch { expected, received } => {
                write!(
                    f,
                    "Size mismatch: expected {} bytes, received {}",
                    expected, received
                )
            }
            TpError::Timeout {
                source,
                destination,
                pgn,
                elapsed_ms,
            } => write!(
                f,
                "TP timeout after {}ms: source={:#04X}, dest={:#04X}, PGN={:#06X}",
                elapsed_ms, source, destination, pgn.pgn
            ),
            TpError::CorruptedData(msg) => write!(f, "Corrupted data: {}", msg),
        }
    }
}

impl std::error::Error for TpError {}

/// J1939 Transport Protocol reassembler.
///
/// Handles both Broadcast Compressed messages (single frame) and
/// Total Message Transfer with RTS/CTS/data packet flow control.
pub struct TpReassembler {
    /// Whether to emit partial assemblies on timeout instead of discarding.
    force_partial: bool,
    /// Timeout in milliseconds before giving up on an assembly.
    timeout_ms: u64,
    /// Active multi-frame assemblies keyed by (source, dest, pgn).
    assemblies: HashMap<(u8, u8, PGN), TpAssemblyState>,
    /// Track which source addresses have initiated RTS to avoid re-RTS confusion.
    rts_pending_sources: HashSet<u8>,
}

impl TpReassembler {
    /// Create a new TpReassembler with the given timeout and force-partial setting.
    pub fn new(force_partial: bool, timeout_ms: u64) -> Self {
        TpReassembler {
            force_partial,
            timeout_ms,
            assemblies: HashMap::new(),
            rts_pending_sources: HashSet::new(),
        }
    }

    /// Process a single RawFrame and return reassembly results.
    pub fn process_frame(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        if frame.data.is_empty() {
            return vec![];
        }

        let pgn = PGN::from_can_id(frame.can_id);
        self.cleanup_expired(frame.timestamp / 1000);
        let results = self.process_frame_internal(frame, &pgn);

        eprintln!(
            "[DEBUG] process_frame: can_id={:#010X}, pgn.pgn={:#06X}, data[0]={}, len={}, results.len()={}",
            frame.can_id, pgn.pgn, frame.data.first().unwrap_or(&0), frame.data.len(), results.len()
        );

        results
    }

    fn process_frame_internal(&mut self, frame: &RawFrame, pgn: &PGN) -> Vec<TpReassemblyResult> {
        if pgn.pgn < 0xEF00 {
            return self.handle_broadcast_compressed(frame, pgn);
        }

        let msg_type = self.detect_message_type(frame, pgn);

        match msg_type {
            TpMessageType::ConnectionManagement => vec![],
            TpMessageType::DataPacket => self.handle_data_packet(frame, pgn),
            TpMessageType::BroadcastCompressed => self.handle_broadcast_compressed(frame, pgn),
        }
    }

    fn detect_message_type(&self, frame: &RawFrame, pgn: &PGN) -> TpMessageType {
        if pgn.pgn < 0xEF00 {
            return TpMessageType::BroadcastCompressed;
        }

        // Connection Management uses PGN 0xEC (which is < 0xEF00, handled as broadcast).
        // For PGN >= 0xEF00, check if this looks like a data packet vs CM frame.
        if pgn.pgn == 0xEC {
            return TpMessageType::ConnectionManagement;
        }

        let data = &frame.data;

        // Data packets: byte 0 is packet sequence number (1-based), bytes 1-7 are payload
        if data.len() >= 2 && data[0] > 0 {
            return TpMessageType::DataPacket;
        }

        TpMessageType::ConnectionManagement
    }

    fn handle_broadcast_compressed(&self, frame: &RawFrame, pgn: &PGN) -> Vec<TpReassemblyResult> {
        let source_address = (frame.can_id & 0xFF) as u8;

        let assembled = AssembledMessage {
            pgn: pgn.clone(),
            source_address,
            destination_address: 0xFF,
            data: frame.data.clone(),
            timestamp: frame.timestamp,
        };

        vec![TpReassemblyResult::Complete(assembled)]
    }

    fn handle_data_packet(&mut self, frame: &RawFrame, pgn: &PGN) -> Vec<TpReassemblyResult> {
        let data = &frame.data;

        if data.len() < 2 {
            eprintln!(
                "[TP] Data packet too short ({} bytes) for PGN={:#06X}, source={:#04X}",
                data.len(),
                pgn.pgn,
                frame.source_address()
            );
            return vec![];
        }

        let packet_num = data[0];
        if packet_num == 0 {
            eprintln!(
                "[TP] Invalid zero packet number for PGN={:#06X}, source={:#04X}",
                pgn.pgn,
                frame.source_address()
            );
            return vec![];
        }

        let payload = data[1..].to_vec();
        if payload.len() > 7 {
            eprintln!(
                "[TP] Data packet payload too large ({} bytes) for PGN={:#06X}",
                payload.len(),
                pgn.pgn
            );
            return vec![];
        }

        let source_address = frame.source_address();
        let destination_address = frame.destination_address();

        let key = (source_address, destination_address, pgn.clone());

        // Check if we have an active assembly for this stream
        if !self.assemblies.contains_key(&key) {
            eprintln!(
                "[TP] Data packet received without prior RTS/CTS for PGN={:#06X}, source={:#04X}",
                pgn.pgn, source_address
            );
            return vec![];
        }

        let assembly = self.assemblies.get(&key).unwrap().clone();

        // Check for duplicate packet number
        if assembly
            .packets_received
            .iter()
            .any(|(pn, _)| *pn == packet_num)
        {
            eprintln!(
                "[TP] Duplicate packet {} for PGN={:#06X}, source={:#04X}",
                packet_num, pgn.pgn, source_address
            );
            return vec![];
        }

        // Check if out of order (packet number less than next expected)
        let next_expected = assembly.packets_received.len() + 1;
        if packet_num < next_expected as u8 {
            eprintln!(
                "[TP] Out-of-order packet {} (expected >= {}) for PGN={:#06X}",
                packet_num, next_expected, pgn.pgn
            );
            return vec![];
        }

        // Check if packet number exceeds total expected packets
        let max_packets = (assembly.total_size + 6) / 7;
        if packet_num > max_packets as u8 {
            eprintln!(
                "[TP] Packet {} exceeds maximum ({}) for PGN={:#06X}",
                packet_num, max_packets, pgn.pgn
            );
            return vec![];
        }

        // Update assembly state
        let mut updated = self.assemblies.get(&key).unwrap().clone();
        updated.packets_received.push((packet_num, payload));
        updated.last_packet_time = frame.timestamp / 1000;

        // Sort packets by number for proper ordering
        updated.packets_received.sort_by_key(|(pn, _)| *pn);

        let total_data_len: usize = updated.packets_received.iter().map(|(_, d)| d.len()).sum();

        if packet_num == max_packets as u8 && total_data_len >= assembly.total_size {
            // Last packet received and we have enough data
            let packets_copy = updated.packets_received.clone();

            let mut assembled_data = Vec::with_capacity(assembly.total_size);
            for (_, payload) in &packets_copy {
                assembled_data.extend_from_slice(payload);
            }

            if assembled_data.len() > assembly.total_size {
                assembled_data.truncate(assembly.total_size);
            }

            self.assemblies.insert(key.clone(), updated);

            let assembled = AssembledMessage {
                pgn: pgn.clone(),
                source_address,
                destination_address,
                data: assembled_data,
                timestamp: frame.timestamp,
            };

            self.assemblies.remove(&key);
            return vec![TpReassemblyResult::Complete(assembled)];
        } else {
            // Still waiting for more packets
            self.assemblies.insert(key, updated);
            return vec![TpReassemblyResult::Pending];
        }
    }

    /// Clean up assemblies that have exceeded the timeout.
    pub fn cleanup_expired(&mut self, current_time_ms: u64) {
        let mut expired_keys = Vec::new();

        for (key, assembly) in &self.assemblies {
            let elapsed = current_time_ms.saturating_sub(assembly.start_time);
            if elapsed > self.timeout_ms {
                expired_keys.push(key.clone());

                eprintln!(
                    "[TP] Timeout after {}ms: source={:#04X}, dest={:#04X}, PGN={}, {} packets received",
                    elapsed,
                    assembly.source_address,
                    assembly.destination_address,
                    assembly.pgn.pgn,
                    assembly.packets_received.len()
                );

                if self.force_partial {
                    let mut assembled_data = Vec::new();
                    for (_, payload) in &assembly.packets_received {
                        assembled_data.extend_from_slice(payload);
                    }

                    eprintln!(
                        "[TP] Timeout for PGN={:#06X}: source={:#04X}, dest={:#04X}",
                        assembly.pgn.pgn, assembly.source_address, assembly.destination_address
                    );
                } else {
                    eprintln!(
                        "[TP] Discarding incomplete TP message: PGN={:#06X}, source={:#04X}",
                        assembly.pgn.pgn, assembly.source_address
                    );
                }
            }
        }

        for key in expired_keys {
            self.assemblies.remove(&key);
        }
    }

    /// Get the number of active assemblies.
    pub fn active_assemblies_count(&self) -> usize {
        self.assemblies.len()
    }

    /// Clear all pending assemblies (useful for reset/restart).
    pub fn clear_all(&mut self) {
        self.assemblies.clear();
        self.rts_pending_sources.clear();
    }
}

impl RawFrame {
    /// Extract source address from a 29-bit J1939 CAN ID.
    pub fn source_address(&self) -> u8 {
        (self.can_id & 0xFF) as u8
    }

    /// Extract destination address from a CAN ID.
    /// For j1939-async format: (priority << 26) | (pgn << 8) | source
    /// Destination defaults to broadcast (0xFF).
    pub fn destination_address(&self) -> u8 {
        0xFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(can_id: u32, data: &[u8]) -> RawFrame {
        RawFrame {
            timestamp: 1_000_000,
            can_id,
            data: data.to_vec(),
        }
    }

    // ========================================================================
    // Broadcast Compressed Message Tests
    // ========================================================================

    #[test]
    fn test_broadcast_compressed_single_frame() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let can_id = (3u32 << 26) | (0x1000 << 8) | 0xF8;
        let frame = make_frame(can_id, &[0x01, 0x02, 0x03, 0x04]);

        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.pgn.pgn, 0x1000);
                assert_eq!(msg.source_address, 0xF8);
                assert_eq!(msg.destination_address, 0xFF);
                assert_eq!(msg.data, vec![0x01, 0x02, 0x03, 0x04]);
            }
            _ => panic!("Expected Complete result"),
        }
    }

    #[test]
    fn test_broadcast_compressed_empty_data() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let can_id = (3u32 << 26) | (0x2000 << 8) | 0x40;
        let frame = make_frame(can_id, &[]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_broadcast_compressed_max_payload() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let can_id = (3u32 << 26) | ((0xEEFF) << 8) | 0x20;
        let data: Vec<u8> = (0..=7).collect();
        let frame = make_frame(can_id, &data);

        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 8);
                assert_eq!(msg.data, data);
            }
            _ => panic!("Expected Complete result"),
        }
    }

    // ========================================================================
    // Total Message Transfer - Data Packet Tests
    // ========================================================================

    #[test]
    fn test_data_packet_without_rts() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF000;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | (pgn_val << 8) | source as u32;
        let _ = dest; // Keep dest in scope to avoid warning
        let frame = make_frame(can_id, &[0x01, 0x41, 0x42]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_data_packet_invalid_zero_number() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF000;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | (pgn_val << 8) | source as u32;
        let frame = make_frame(can_id, &[0x00, 0x41]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_data_packet_too_short() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF000;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | (pgn_val << 8) | source as u32;
        let frame = make_frame(can_id, &[0x01]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_data_packet_payload_too_large() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF000;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | (pgn_val << 8) | source as u32;
        let frame = make_frame(
            can_id,
            &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48],
        );

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    // ========================================================================
    // Complete RTS -> Data Flow Tests
    // ========================================================================

    #[test]
    fn test_complete_single_packet_transfer() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF000;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);
        let key = (source, dest, pgn_struct.clone());

        reassembler.assemblies.insert(
            key.clone(),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 5,
                page_number: 0,
                packets_received: vec![],
                start_time: 1000,
                last_packet_time: 1000,
            },
        );

        let frame = make_frame(can_id_base, &[0x01, 0x10, 0x20, 0x30, 0x40, 0x50]);

        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.pgn.pgn, pgn_val);
                assert_eq!(msg.source_address, source);
                assert_eq!(msg.destination_address, dest);
                assert_eq!(msg.data, vec![0x10, 0x20, 0x30, 0x40, 0x50]);
            }
            _ => panic!("Expected Complete result"),
        }

        assert!(!reassembler.assemblies.contains_key(&key));
    }

    #[test]
    fn test_complete_multi_packet_transfer() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);
        let key = (source, dest, pgn_struct.clone());

        reassembler.assemblies.insert(
            key.clone(),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 20,
                page_number: 1,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let mut results = Vec::new();

        let frame1 = make_frame(
            can_id_base,
            &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01],
        );
        results.extend(reassembler.process_frame(&frame1));

        let frame2 = make_frame(
            can_id_base,
            &[0x02, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77],
        );
        results.extend(reassembler.process_frame(&frame2));

        let frame3 = make_frame(can_id_base, &[0x03, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC]);
        results.extend(reassembler.process_frame(&frame3));

        let complete_results: Vec<_> = results
            .iter()
            .filter(|r| matches!(r, TpReassemblyResult::Complete(_)))
            .collect();

        assert_eq!(complete_results.len(), 1);
        match &complete_results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.pgn.pgn, pgn_val);
                assert_eq!(msg.source_address, source);
                assert_eq!(msg.data.len(), 20);
                let expected = vec![
                    0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01, // packet 1 (7 bytes)
                    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, // packet 2 (7 bytes after fix)
                    0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, // packet 3 (6 bytes to reach 20)
                ];
                assert_eq!(msg.data, expected);
            }
            _ => panic!("Expected Complete result"),
        }

        assert!(!reassembler.assemblies.contains_key(&key));
    }

    #[test]
    fn test_pending_during_multi_packet() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);
        let key = (source, dest, pgn_struct.clone());

        reassembler.assemblies.insert(
            key.clone(),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 14,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let frame1 = make_frame(
            can_id_base,
            &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01],
        );
        let results = reassembler.process_frame(&frame1);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Pending => {}
            other => panic!("Expected Pending, got {:?}", other),
        }

        assert!(reassembler.assemblies.contains_key(&key));
    }

    #[test]
    fn test_pending_result_has_correct_count() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 21,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let frame = make_frame(
            can_id_base,
            &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01],
        );
        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Pending => {}
            _other => panic!("Expected Pending for incomplete assembly"),
        }
    }

    // ========================================================================
    // Timeout Handling Tests
    // ========================================================================

    #[test]
    fn test_timeout_discards_assembly() {
        let mut reassembler = TpReassembler::new(false, 100);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;

        let pgn_struct = PGN::from_can_id(pgn_val);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(700);

        assert!(
            !reassembler
                .assemblies
                .contains_key(&(source, dest, pgn_struct.clone())),
            "Assembly should be removed after timeout"
        );
    }

    #[test]
    fn test_timeout_emits_partial_assembly() {
        let mut reassembler = TpReassembler::new(true, 100);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;

        let pgn_struct = PGN::from_can_id(pgn_val);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(700);

        assert!(
            !reassembler
                .assemblies
                .contains_key(&(source, dest, pgn_struct.clone())),
            "Partial assembly should be removed after emission"
        );
    }

    #[test]
    fn test_timeout_does_not_expire_recent() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;

        let pgn_struct = PGN::from_can_id(pgn_val);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(800);

        assert!(reassembler
            .assemblies
            .contains_key(&(source, dest, pgn_struct.clone())));
    }

    #[test]
    fn test_multiple_assembly_timeouts() {
        let mut reassembler = TpReassembler::new(false, 100);

        let pgn_a = PGN::from_can_id(0);
        let pgn_b = PGN::from_can_id(1);

        reassembler.assemblies.insert(
            (0x20, 0xFF, pgn_a.clone()),
            TpAssemblyState {
                source_address: 0x20,
                destination_address: 0xFF,
                pgn: pgn_a.clone(),
                total_size: 10,
                page_number: 0,
                packets_received: vec![],
                start_time: 100,
                last_packet_time: 100,
            },
        );

        reassembler.assemblies.insert(
            (0x30, 0xFF, pgn_b.clone()),
            TpAssemblyState {
                source_address: 0x30,
                destination_address: 0xFF,
                pgn: pgn_b.clone(),
                total_size: 20,
                page_number: 0,
                packets_received: vec![],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(300);

        assert!(
            !reassembler
                .assemblies
                .contains_key(&(0x20, 0xFF, pgn_a.clone())),
            "Old assembly should be removed"
        );
        assert!(
            reassembler
                .assemblies
                .contains_key(&(0x30, 0xFF, pgn_b.clone())),
            "Newer assembly should remain"
        );
    }

    // ========================================================================
    // Duplicate and Out-of-Order Packet Tests
    // ========================================================================

    #[test]
    fn test_duplicate_packet_rejected() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 14,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let frame = make_frame(
            can_id_base,
            &[0x01, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22],
        );
        let results = reassembler.process_frame(&frame);

        assert!(results.is_empty());

        let assembly = reassembler
            .assemblies
            .get(&(source, dest, pgn_struct.clone()))
            .unwrap();
        assert_eq!(assembly.packets_received.len(), 1);
        assert_eq!(assembly.packets_received[0].1, vec![0xAA; 7]);
    }

    #[test]
    fn test_out_of_order_packet_rejected() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7]), (2, vec![0xBB; 7])],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let frame = make_frame(
            can_id_base,
            &[0x02, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33],
        );
        let results = reassembler.process_frame(&frame);

        assert!(results.is_empty());
    }

    // ========================================================================
    // Size Mismatch Tests
    // ========================================================================

    #[test]
    fn test_data_truncated_to_expected_size() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 5,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let frame = make_frame(
            can_id_base,
            &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01],
        );
        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 5);
                assert_eq!(msg.data, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
            }
            _ => panic!("Expected Complete result"),
        }
    }

    // ========================================================================
    // TpError Display Tests
    // ========================================================================

    #[test]
    fn test_tp_error_display_invalid_packet_number() {
        let err = TpError::InvalidPacketNumber;
        assert_eq!(format!("{}", err), "Invalid packet number");
    }

    #[test]
    fn test_tp_error_display_duplicate_packet() {
        let err = TpError::DuplicatePacket;
        assert_eq!(format!("{}", err), "Duplicate packet detected");
    }

    #[test]
    fn test_tp_error_display_out_of_order() {
        let err = TpError::OutOfOrderPacket;
        assert_eq!(format!("{}", err), "Out-of-order packet received");
    }

    #[test]
    fn test_tp_error_display_size_mismatch() {
        let err = TpError::SizeMismatch {
            expected: 20,
            received: 15,
        };
        assert_eq!(
            format!("{}", err),
            "Size mismatch: expected 20 bytes, received 15"
        );
    }

    #[test]
    fn test_tp_error_display_timeout() {
        let can_id = (3u32 << 26) | (0xF000u32 << 8);
        let pgn = PGN::from_can_id(can_id);
        let err = TpError::Timeout {
            source: 0x20,
            destination: 0xFF,
            pgn,
            elapsed_ms: 1500,
        };
        let display = format!("{}", err);
        assert!(display.contains("1500ms"));
        assert!(display.contains("F000"));
    }

    #[test]
    fn test_tp_error_display_corrupted_data() {
        let err = TpError::CorruptedData("checksum mismatch".to_string());
        assert_eq!(format!("{}", err), "Corrupted data: checksum mismatch");
    }

    // ========================================================================
    // TpReassembler Lifecycle Tests
    // ========================================================================

    #[test]
    fn test_new_reassembler_defaults() {
        let reassembler = TpReassembler::new(false, 1000);

        assert_eq!(reassembler.active_assemblies_count(), 0);
        assert!(reassembler.assemblies.is_empty());
    }

    #[test]
    fn test_clear_all_removes_everything() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_a = PGN::from_can_id(0);

        reassembler.assemblies.insert(
            (0x20, 0xFF, pgn_a.clone()),
            TpAssemblyState {
                source_address: 0x20,
                destination_address: 0xFF,
                pgn: pgn_a.clone(),
                total_size: 10,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        assert_eq!(reassembler.active_assemblies_count(), 1);

        reassembler.clear_all();

        assert_eq!(reassembler.active_assemblies_count(), 0);
    }

    #[test]
    fn test_reassembler_with_different_timeout() {
        let mut reassembler = TpReassembler::new(true, 5000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;

        let pgn_struct = PGN::from_can_id(pgn_val);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 10,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        reassembler.cleanup_expired(9000);

        assert!(reassembler
            .assemblies
            .contains_key(&(source, dest, pgn_struct.clone())));

        reassembler.cleanup_expired(11000);

        assert!(
            !reassembler
                .assemblies
                .contains_key(&(source, dest, pgn_struct.clone())),
            "Should expire after 5s timeout"
        );
    }

    #[test]
    fn test_broadcast_compressed_preserves_data() {
        let mut reassembler = TpReassembler::new(false, 1000);

        for pgn_val in [0x0000u32, 0x1000, 0xEEFF] {
            let can_id = (3u32 << 26) | (pgn_val << 8) | 0xF8;
            let data: Vec<u8> = (0..=7).collect();
            let frame = make_frame(can_id, &data);

            let results = reassembler.process_frame(&frame);

            assert_eq!(results.len(), 1);
            match &results[0] {
                TpReassemblyResult::Complete(msg) => {
                    assert_eq!(msg.pgn.pgn, pgn_val);
                    assert_eq!(msg.data, data);
                }
                _ => panic!("Expected Complete for PGN={:#06X}", pgn_val),
            }
        }
    }

    #[test]
    fn test_complete_flow_with_timestamps() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 7,
                page_number: 0,
                packets_received: vec![],
                start_time: 1_000_000,
                last_packet_time: 1_000_000,
            },
        );

        let frame = make_frame(
            can_id_base,
            &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01],
        );
        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.timestamp, 1_000_000);
            }
            _ => panic!("Expected Complete result"),
        }
    }

    #[test]
    fn test_packet_sequence_ordering() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 28,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let mut results = Vec::new();

        for (i, payload_byte) in [b'A', b'B', b'C', b'D'].iter().enumerate() {
            let frame_data: Vec<u8> = std::iter::once((i + 1) as u8)
                .chain(std::iter::repeat(*payload_byte).take(7))
                .collect();

            results.extend(reassembler.process_frame(&make_frame(can_id_base, &frame_data)));
        }

        let complete: Vec<_> = results
            .iter()
            .filter(|r| matches!(r, TpReassemblyResult::Complete(_)))
            .collect();

        assert_eq!(complete.len(), 1);
    }

    #[test]
    fn test_error_implements_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(TpError::InvalidPacketNumber);
        let _display = format!("{}", err);
    }

    #[test]
    fn test_partial_assembly_fields() {
        let pgn = PGN::from_can_id(0xF500);

        let partial = PartialAssembly {
            source_address: 0x20,
            destination_address: 0xFF,
            pgn: pgn.clone(),
            data: vec![0xAA; 10],
            total_expected: 20,
            timestamp: 5_000_000,
        };

        assert_eq!(partial.source_address, 0x20);
        assert_eq!(partial.destination_address, 0xFF);
        assert_eq!(partial.data.len(), 10);
        assert_eq!(partial.total_expected, 20);
        assert_eq!(partial.timestamp, 5_000_000);
    }

    #[test]
    fn test_assembly_state_clone() {
        let pgn = PGN::from_can_id(0xF500);

        let state = TpAssemblyState {
            source_address: 0x20,
            destination_address: 0xFF,
            pgn,
            total_size: 14,
            page_number: 1,
            packets_received: vec![(1, vec![0xAA; 7]), (2, vec![0xBB; 7])],
            start_time: 5000,
            last_packet_time: 6000,
        };

        let cloned = state.clone();

        assert_eq!(cloned.source_address, state.source_address);
        assert_eq!(cloned.destination_address, state.destination_address);
        assert_eq!(cloned.total_size, state.total_size);
        assert_eq!(cloned.packets_received.len(), 2);
    }

    #[test]
    fn test_rawframe_source_address() {
        let frame = make_frame(0x18EF4000, &[0x01]);
        assert_eq!(frame.source_address(), 0x00);

        let frame2 = make_frame(0x18EFF0F8, &[0x01]);
        assert_eq!(frame2.source_address(), 0xF8);
    }

    #[test]
    fn test_rawframe_destination_address_broadcast() {
        let frame = make_frame(0x18EF4000, &[0x01]);
        assert_eq!(frame.destination_address(), 0xFF);
    }

    #[test]
    fn test_complete_flow_with_max_packet_size() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 7,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let frame = make_frame(
            can_id_base,
            &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01],
        );
        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                assert_eq!(msg.data, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]);
            }
            _ => panic!("Expected Complete result"),
        }
    }

    #[test]
    fn test_assembly_with_many_packets() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id_base = (7u32 << 26) | (pgn_val << 8) | source as u32;

        let pgn_struct = PGN::from_can_id(can_id_base);

        reassembler.assemblies.insert(
            (source, dest, pgn_struct.clone()),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_struct.clone(),
                total_size: 50,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        let mut results = Vec::new();

        for i in 1..=8 {
            let payload_start = (i - 1) as u8 * 7;
            let frame_data: Vec<u8> = std::iter::once(i as u8)
                .chain((payload_start..payload_start + 7).map(|b| b + 0x10))
                .collect();

            let mut data = frame_data;
            while data.len() < 8 {
                data.push(0x00);
            }

            results.extend(reassembler.process_frame(&make_frame(can_id_base, &data)));
        }

        let complete: Vec<_> = results
            .iter()
            .filter(|r| matches!(r, TpReassemblyResult::Complete(_)))
            .collect();

        assert_eq!(complete.len(), 1);
    }
}
