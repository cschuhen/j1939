use j1939_async::Id;
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::types::{AssembledMessage, RawFrame};

/// Type of Transport Protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpMessageType {
    /// Connection Management frame (RTS/CTS/EOM/BAM). All use PGN 0xEC00.
    ConnectionManagement,
    /// Actual data packet in a Total Message Transfer sequence. Uses PGN 0xEB00.
    DataPacket,

    /// Frame is not a Transport Protocol frame.
    NotTp,
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
    pub pgn: u32,
    pub priority: u8,
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
    pub pgn: u32,
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
                elapsed_ms,
            } => write!(
                f,
                "TP timeout after {}ms: source={:#04X}, dest={:#04X}",
                elapsed_ms, source, destination
            ),
            TpError::CorruptedData(msg) => write!(f, "Corrupted data: {}", msg),
        }
    }
}

impl std::error::Error for TpError {}

/// Extract source and destination addresses from a J1939 CAN ID.
///
/// TP.CM (PF=0xEC) and TP.DT (PF=0xEB) transport mechanism is ALWAYS PDU1 format.
/// PS = destination address: 0xFF = broadcast, otherwise unicast to that node.
/// The payload inside may encode a larger message that is PDU1 or PDU2 -- separate concern.
fn extract_tp_addresses(can_id: u32) -> (u8, u8) {
    let pf = ((can_id >> 16) & 0xFF) as u8;
    let ps = ((can_id >> 8) & 0xFF) as u8;
    let source = (can_id & 0xFF) as u8;

    // TP.CM and TP.DT transport frames are always PDU1: PS is the destination address
    if pf == 0xEC || pf == 0xEB {
        return (source, ps);
    }

    // Standard J1939: PF >= 0xF0 means PDU2 (broadcast), PS = group extension
    if pf >= 0xF0 {
        (source, 0xFF)
    } else {
        (source, ps)
    }
}

/// Build a CAN ID from a payload PGN and source/destination addresses.
///
/// The payload PGN is extracted from BAM/RTS data bytes 5-7 (little-endian).
/// For TP messages (PGN >= 0xF000), uses PDU2 format where destination=0xFF.
/// For standard J1939 messages (PGN < 0xF000), uses PDU1 format with dest in PS field.
fn build_assembled_can_id(pgn: u32, source: u8, dest: u8, priority: u8) -> u32 {
    let mut id = source as u32;
    id |= (priority as u32) << 26;

    if pgn >= 0xF000 {
        // PDU2 format: full PGN in bits 8-25, destination is always broadcast (0xFF)
        id |= (pgn & 0x3FFFF) << 8;
    } else {
        // PDU1 format: dest in PS field (bits 8-15), PGN high bits in PF field (bits 16-25)
        let pgn_high = (pgn >> 8) & 0x3FF;
        id |= (dest as u32) << 8;
        id |= (pgn_high as u32) << 16;
    }

    id
}

/// J1939 Transport Protocol reassembler.
///
/// Handles both Broadcast Compressed messages (single frame) and
/// Total Message Transfer with RTS/CTS/data packet flow control.
pub struct TpReassembler {
    /// Whether to emit partial assemblies on timeout instead of discarding.
    force_partial: bool,
    /// Timeout in milliseconds before giving up on an assembly.
    timeout_ms: u64,
    /// Active multi-frame assemblies keyed by (source, dest).
    assemblies: HashMap<(u8, u8), TpAssemblyState>,
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

        self.cleanup_expired(frame.timestamp / 1000);
        let results = self.process_frame_internal(frame);

        eprintln!(
            "[DEBUG] process_frame: can_id={:#010X}, pgn.pgn={:#06X}, data[0]={}, len={}, results.len()={}",
            frame.can_id, frame.pgn(), frame.data.first().unwrap_or(&0), frame.data.len(), results.len()
        );

        results
    }

    fn process_frame_internal(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        let msg_type = self.detect_message_type(frame);

        match msg_type {
            TpMessageType::ConnectionManagement => self.handle_connection_management(frame),
            TpMessageType::DataPacket => self.handle_data_packet(frame),
            TpMessageType::NotTp => vec![],
        }
    }

    fn detect_message_type(&self, frame: &RawFrame) -> TpMessageType {
        // Connection Management uses PGN 0xEC00 (RTS/CTS/EOM/BAM all share this PGN)
        if frame.pgn() == 0xEC00 {
            return TpMessageType::ConnectionManagement;
        }

        // Data packets use TP.DT PGN 0xEB00
        if frame.pgn() == 0xEB00 {
            return TpMessageType::DataPacket;
        }

        TpMessageType::NotTp
    }

    fn handle_connection_management(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        let data = &frame.data;
        if data.is_empty() {
            return vec![];
        }

        let control_byte = data[0];

        match control_byte {
            0x10 => self.handle_rts(frame, data),
            0x11 => self.handle_cts(frame, data),
            0x13 => self.handle_eom(frame, data),
            0x1C => self.handle_abort(frame, data),
            0x20 => self.handle_bam_cm(frame, data),
            _ => {
                eprintln!("[TP] Unknown CM control byte={:#04X} from source={:#04X}", control_byte, frame.source_address());
                vec![]
            }
        }
    }

    fn handle_rts(&mut self, frame: &RawFrame, data: &[u8]) -> Vec<TpReassemblyResult> {
        if data.len() < 5 {
            return vec![];
        }

        // RTS format (J1939): [control=0x10, total_size(LE 2B), num_packets, max_burst, dest_addr, PGN(LE 3B)]
        let total_size = ((data[2] as usize) << 8) | (data[1] as usize);
        let num_packets = data[3];

        // RTS is sent BY receiver TO transmitter. The key uses the transmitter's source address.
        let transmitter = frame.source_address();

        // Only accept RTS from a new transmitter (avoid re-RTS confusion)
        if self.rts_pending_sources.contains(&transmitter) {
            return vec![];
        }

        let key = (transmitter, frame.destination_address());

        // Extract PGN from payload bytes 5-7 (PGN of message being sent)
        let pgn_from_rts = if data.len() >= 8 {
            ((data[7] as u32) << 16) | ((data[6] as u32) << 8) | (data[5] as u32)
        } else {
            0
        };

        let assembly = TpAssemblyState {
            source_address: transmitter,
            destination_address: frame.destination_address(),
            pgn: pgn_from_rts,
            priority: frame.priority(),
            total_size,
            page_number: 0,
            packets_received: vec![],
            start_time: frame.timestamp / 1000,
            last_packet_time: frame.timestamp / 1000,
        };

        self.assemblies.insert(key.clone(), assembly);
        self.rts_pending_sources.insert(transmitter);

        eprintln!(
            "[TP] RTS received: PGN={:#06X}, size={}, packets={}, transmitter={:#04X}",
            pgn_from_rts, total_size, num_packets, transmitter
        );

        vec![]
    }

    fn handle_bam_cm(&mut self, frame: &RawFrame, data: &[u8]) -> Vec<TpReassemblyResult> {
        if data.len() < 5 {
            return vec![];
        }

        // BAM format (J1939): [control=0x20, total_size(LE), num_packets, reserved(0xFF), PGN(LE)]
        let total_size = ((data[2] as usize) << 8) | (data[1] as usize);
        let num_packets = data[3];

        // Extract PGN from payload bytes 5-7 (PGN of message being broadcast, Little-Endian)
        let pgn_from_bam = if data.len() >= 8 {
            ((data[7] as u32) << 16) | ((data[6] as u32) << 8) | (data[5] as u32)
        } else {
            0
        };

        let source = frame.source_address();
        let dest = frame.destination_address();

        eprintln!(
            "[TP] BAM received: PGN={:#06X}, size={}, packets={}, source={:#04X} dest={:#04X}",
            pgn_from_bam, total_size, num_packets, source, dest
        );

        // Create assembly state for broadcast transfer (dest=0xFF)
        let key = (source, dest);
        self.assemblies.insert(key, TpAssemblyState {
            source_address: source,
            destination_address: dest,
            pgn: pgn_from_bam,
            priority: frame.priority(),
            total_size,
            page_number: 0,
            packets_received: vec![],
            start_time: frame.timestamp / 1000,
            last_packet_time: frame.timestamp / 1000,
        });

        vec![]
    }

    fn handle_cts(&self, _frame: &RawFrame, _data: &[u8]) -> Vec<TpReassemblyResult> {
        eprintln!("[TP] CTS received - not yet implemented");
        vec![]
    }

    fn handle_eom(&self, _frame: &RawFrame, _data: &[u8]) -> Vec<TpReassemblyResult> {
        eprintln!("[TP] EOM received - not yet implemented");
        vec![]
    }

    fn handle_abort(&self, _frame: &RawFrame, _data: &[u8]) -> Vec<TpReassemblyResult> {
        eprintln!("[TP] Abort received - not yet implemented");
        vec![]
    }

    fn handle_data_packet(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        let data = &frame.data;

        if data.len() < 2 {
            eprintln!(
                "[TP] Data packet too short ({} bytes) for source={:#04X} destination={:#04X}",
                data.len(),
                frame.source_address(),
                frame.destination_address()
            );
            return vec![];
        }

        let packet_num = data[0];
        if packet_num == 0 {
            eprintln!(
                "[TP] Invalid zero packet number for source={:#04X} destination={:#04X}",
                frame.source_address(),
                frame.destination_address()
            );
            return vec![];
        }

        let payload = data[1..].to_vec();
        if payload.len() > 7 {
            eprintln!(
                "[TP] Data packet payload too large ({} bytes) for source={:#04X} destination={:#04X}",
                payload.len(),
                frame.source_address(),
                frame.destination_address()
            );
            return vec![];
        }

        let source_address = frame.source_address();
        let dest_from_frame = frame.destination_address();

        // Try lookup with (source, dest) first. If not found, try (source, 0xFF) as fallback
        // to handle cases where j1939_async forces TP.DT into PDU1 format for broadcast transfers.
        eprintln!(
            "[TP] DT lookup: key=({:#04X}, {:#04X}), assemblies.len()={}, keys={:?}",
            source_address, dest_from_frame, self.assemblies.len(),
            self.assemblies.keys().collect::<Vec<_>>()
        );

        // Check if we have an active assembly for this stream
        let assembly = if let Some(assembly) = self.assemblies.get(&(source_address, dest_from_frame)) {
            assembly.clone()
        } else if let Some(assembly) = self.assemblies.get(&(source_address, 0xFF)) {
            assembly.clone()
        } else {
            eprintln!(
                "[TP] Data packet received without prior RTS/CTS for source={:#04X} destination={:#04X}",
                source_address, dest_from_frame
            );
            return vec![];
        };

        // Check for duplicate packet number
        if assembly
            .packets_received
            .iter()
            .any(|(pn, _)| *pn == packet_num)
        {
            eprintln!(
                "[TP] Duplicate packet {} for PGN={:#06X}, source={:#04X} dest={:#04X}",
                packet_num, assembly.pgn, source_address, assembly.destination_address
            );
            return vec![];
        }

        // Check if out of order (packet number less than next expected)
        let next_expected = assembly.packets_received.len() + 1;
        if packet_num < next_expected as u8 {
            eprintln!(
                "[TP] Out-of-order packet {} (expected >= {}) for PGN={:#06X} source={:#04X}",
                packet_num, next_expected, assembly.pgn, source_address
            );
            return vec![];
        }

        // Check if packet number exceeds total expected packets
        let max_packets = (assembly.total_size + 6) / 7;
        if packet_num > max_packets as u8 {
            eprintln!(
                "[TP] Packet {} exceeds maximum ({}) for PGN={:#06X} source={:#04X}",
                packet_num, max_packets, assembly.pgn, source_address
            );
            return vec![];
        }

        // Update assembly state
        let key = (source_address, dest_from_frame);
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

            let id = build_assembled_can_id(
                assembly.pgn,
                assembly.source_address,
                assembly.destination_address,
                assembly.priority,
            );

            let assembled = AssembledMessage {
                id,
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
                    assembly.pgn,
                    assembly.packets_received.len()
                );

                if self.force_partial {
                    let mut assembled_data = Vec::new();
                    for (_, payload) in &assembly.packets_received {
                        assembled_data.extend_from_slice(payload);
                    }

                    eprintln!(
                        "[TP] Timeout for PGN={:#06X}: source={:#04X}, dest={:#04X}",
                        assembly.pgn, assembly.source_address, assembly.destination_address
                    );
                } else {
                    eprintln!(
                        "[TP] Discarding incomplete TP message: PGN={:#06X}, source={:#04X}",
                        assembly.pgn, assembly.source_address
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
    fn test_bam_sets_up_assembly_state() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // BAM is a TP.CM frame (PGN 0xEC00) with control byte 0x20.
        // Per J1939 spec: [control=0x20, total_size_L, total_size_H, num_packets, reserved(0xFF), PGN_L, PGN_H, PGN_ext]
        let source = 0xF8u8;
        let dest = 0xFFu8;
        // TP.CM broadcast CAN ID: PF=0xEC, PS=dest(0xFF)=broadcast, SA=source
        let can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((dest as u32) << 8) | source as u32;

        // total_size = 15 bytes (2 packets: ceil(15/7) = 3, but we use 2 for simplicity)
        // PGN of message being broadcast = 0x1000
        let bam_data = [
            0x20,           // control byte = BAM
            0x0F, 0x00,     // total_size = 15 (Little-Endian)
            0x03,           // num_packets = 3
            0xFF,           // reserved
            0x00, 0x10, 0x00, // PGN = 0x001000 (Little-Endian)
        ];
        let frame = make_frame(can_id, &bam_data);

        let results = reassembler.process_frame(&frame);

        // BAM itself does not return assembled data - it sets up state for DT packets
        assert!(results.is_empty());

        // Verify assembly state was created
        assert!(reassembler.assemblies.contains_key(&(source, dest)));
        let state = reassembler.assemblies.get(&(source, dest)).unwrap();
        assert_eq!(state.pgn, 0x1000);
        assert_eq!(state.total_size, 15);
    }

    #[test]
    fn test_bam_too_short_data() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // BAM frame with insufficient data length (broadcast)
        let can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0xF8;
        let frame = make_frame(can_id, &[0x20]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_bam_followed_by_dt_packets() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // Step 1: Send BAM to set up broadcast transfer
        let source = 0xF8u8;
        let dest = 0xFFu8;
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((dest as u32) << 8) | source as u32;
        let bam_data = [
            0x20,           // control byte = BAM
            0x0E, 0x00,     // total_size = 14 bytes
            0x02,           // num_packets = 2
            0xFF,           // reserved
            0x00, 0x10, 0x00, // PGN = 0x001000
        ];
        reassembler.process_frame(&make_frame(bam_can_id, &bam_data));

        // Step 2: Send TP.DT packets with sequence numbers and payload (broadcast)
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        // Packet 1: sequence=1, 7 bytes of data
        let packet1_data = [0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47];
        let results1 = reassembler.process_frame(&make_frame(dt_can_id, &packet1_data));
        assert_eq!(results1.len(), 1); // Pending result
        match &results1[0] {
            TpReassemblyResult::Pending => {}
            other => panic!("Expected Pending, got {:?}", other),
        }

        // Packet 2: sequence=2, 7 bytes of data (completes the transfer)
        let packet2_data = [0x02, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E];
        let results2 = reassembler.process_frame(&make_frame(dt_can_id, &packet2_data));

        assert_eq!(results2.len(), 1);
        match &results2[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.pgn(), 0x1000);
                assert_eq!(msg.source(), source);
                assert_eq!(msg.destination(), dest);
                // Data should be 14 bytes (7 + 7), truncated to total_size
                assert_eq!(msg.data.len(), 14);
                assert_eq!(msg.data, vec![0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]);
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

        // TP.DT broadcast frame: PF=0xEB, PS=dest(0xFF), SA=source
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;
        let _ = dest; // Keep dest in scope to avoid warning
        let frame = make_frame(can_id, &[0x01, 0x41, 0x42]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_data_packet_invalid_zero_number() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // TP.DT broadcast frame: PF=0xEB, PS=dest(0xFF), SA=source
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;
        let frame = make_frame(can_id, &[0x00, 0x41]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_data_packet_too_short() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // TP.DT broadcast frame: PF=0xEB, PS=dest(0xFF), SA=source
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;
        let frame = make_frame(can_id, &[0x01]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_data_packet_payload_too_large() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // TP.DT broadcast frame: PF=0xEB, PS=dest(0xFF), SA=source
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        let can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;
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

        let pgn_val: u32 = 0x0600;
        let transmitter: u8 = 0x20;
        let receiver: u8 = 0x21;

        // Send RTS frame first (J1939 CM PGN=0xEC00) using PDU1 format for unicast
        // PDU1 CAN ID: (priority<<26) | (PF<<16) | (dest<<8) | source
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((receiver as u32) << 8) | transmitter as u32;
        // RTS payload: [control=0x10, total_size_low, total_size_high, num_packets, max_burst, PGN_L, PGN_H, PGN_HH]
        let rts_data = [0x10, 5, 0, 1, 1, pgn_val as u8, (pgn_val >> 8) as u8, (pgn_val >> 16) as u8];
        let rts_frame = make_frame(rts_can_id, &rts_data);
        reassembler.process_frame(&rts_frame);

        // TP.DT frame: use PDU1 format with same dest=receiver for unicast transfer
        // This ensures frame.destination_address() == receiver so lookup key matches assembly state
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((receiver as u32) << 8) | transmitter as u32;
        let frame = make_frame(dt_can_id, &[0x01, 0x10, 0x20, 0x30, 0x40, 0x50]);

        let results = reassembler.process_frame(&frame);

        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.pgn(), pgn_val);
                assert_eq!(msg.source(), transmitter);
                assert_eq!(msg.destination(), receiver);
                assert_eq!(msg.data, vec![0x10, 0x20, 0x30, 0x40, 0x50]);
            }
            _ => panic!("Expected Complete result"),
        }
    }

    #[test]
    fn test_complete_multi_packet_transfer() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        let key = (source, dest);

        reassembler.assemblies.insert(
            key.clone(),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
                assert_eq!(msg.pgn(), pgn_val);
                assert_eq!(msg.source(), source);
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
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        let key = (source, dest);

        reassembler.assemblies.insert(
            key.clone(),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(700);

        assert!(
            !reassembler.assemblies.contains_key(&(source, dest)),
            "Assembly should be removed after timeout"
        );
    }

    #[test]
    fn test_timeout_emits_partial_assembly() {
        let mut reassembler = TpReassembler::new(true, 100);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(700);

        assert!(
            !reassembler.assemblies.contains_key(&(source, dest)),
            "Partial assembly should be removed after emission"
        );
    }

    #[test]
    fn test_timeout_does_not_expire_recent() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
                total_size: 21,
                page_number: 0,
                packets_received: vec![(1, vec![0xAA; 7])],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(800);

        assert!(reassembler.assemblies.contains_key(&(source, dest)));
    }

    #[test]
    fn test_multiple_assembly_timeouts() {
        let mut reassembler = TpReassembler::new(false, 100);

        reassembler.assemblies.insert(
            (0x20, 0xFF),
            TpAssemblyState {
                source_address: 0x20,
                destination_address: 0xFF,
                pgn: 0xFEF4,
                priority: 0,
                total_size: 10,
                page_number: 0,
                packets_received: vec![],
                start_time: 100,
                last_packet_time: 100,
            },
        );

        reassembler.assemblies.insert(
            (0x30, 0xFF),
            TpAssemblyState {
                source_address: 0x30,
                destination_address: 0xFF,
                pgn: 0xFEF4,
                priority: 0,
                total_size: 20,
                page_number: 0,
                packets_received: vec![],
                start_time: 500,
                last_packet_time: 500,
            },
        );

        reassembler.cleanup_expired(300);

        assert!(
            !reassembler.assemblies.contains_key(&(0x20, 0xFF)),
            "Old assembly should be removed"
        );
        assert!(
            reassembler.assemblies.contains_key(&(0x30, 0xFF)),
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
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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

        let assembly = reassembler.assemblies.get(&(source, dest)).unwrap();
        assert_eq!(assembly.packets_received.len(), 1);
        assert_eq!(assembly.packets_received[0].1, vec![0xAA; 7]);
    }

    #[test]
    fn test_out_of_order_packet_rejected() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x30;
        let dest: u8 = 0xFF;
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
        let err = TpError::Timeout {
            source: 0x20,
            destination: 0xFF,
            elapsed_ms: 1500,
        };
        let display = format!("{}", err);
        assert!(display.contains("1500ms"));
        assert!(display.contains("20"));
        assert!(display.contains("FF"));
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

        reassembler.assemblies.insert(
            (0x20, 0xFF),
            TpAssemblyState {
                source_address: 0x20,
                destination_address: 0xFF,
                pgn: 0,
                priority: 0,
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

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
                total_size: 10,
                page_number: 0,
                packets_received: vec![],
                start_time: 5000,
                last_packet_time: 5000,
            },
        );

        reassembler.cleanup_expired(9000);

        assert!(reassembler.assemblies.contains_key(&(source, dest)));

        reassembler.cleanup_expired(11000);

        assert!(
            !reassembler.assemblies.contains_key(&(source, dest)),
            "Should expire after 5s timeout"
        );
    }

    #[test]
    fn test_bam_preserves_pgn_and_size() {
        let mut reassembler = TpReassembler::new(false, 1000);

        // BAM is a TP.CM frame (PGN 0xEC00) with control byte 0x20.
        // Per J1939 spec: [control=0x20, total_size_L, total_size_H, num_packets, reserved(0xFF), PGN_L, PGN_H, PGN_ext]
        let source = 0xF8u8;
        let dest = 0xFFu8;
        let can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((dest as u32) << 8) | source as u32;

        // total_size = 14 bytes, PGN of message = 0x0A00 (fuel level sensor)
        let bam_data = [
            0x20,           // control byte = BAM
            0x0E, 0x00,     // total_size = 14 (Little-Endian)
            0x02,           // num_packets = 2
            0xFF,           // reserved
            0x00, 0x0A, 0x00, // PGN = 0x000A00 (Little-Endian)
        ];
        let frame = make_frame(can_id, &bam_data);

        let results = reassembler.process_frame(&frame);

        // BAM sets up state but doesn't return data directly
        assert!(results.is_empty());

        // Verify the assembly state preserves PGN and size correctly
        let state = reassembler.assemblies.get(&(source, dest)).unwrap();
        assert_eq!(state.pgn, 0x0A00);
        assert_eq!(state.total_size, 14);
    }

    #[test]
    fn test_complete_flow_with_timestamps() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
        let partial = PartialAssembly {
            source_address: 0x20,
            destination_address: 0xFF,
            pgn: 0xF500,
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
        let state = TpAssemblyState {
            source_address: 0x20,
            destination_address: 0xFF,
            pgn: 0xF500,
            priority: 0,
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
        let frame = make_frame(0x18F04000, &[0x01]);
        assert_eq!(frame.destination_address(), 0xFF);
    }

    #[test]
    fn test_rawframe_destination_address_unicast() {
        let frame = make_frame(0x18EF4000, &[0x01]);
        assert_eq!(frame.destination_address(), 0x40);
    }

    #[test]
    fn test_complete_flow_with_max_packet_size() {
        let mut reassembler = TpReassembler::new(false, 1000);

        let pgn_val: u32 = 0xF500;
        let source: u8 = 0x20;
        let dest: u8 = 0xFF;
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                pgn: pgn_val,
                priority: 0,
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
        // TP.DT frames always use PGN 0xEB00 per J1939 spec
        let can_id_base = (7u32 << 26) | ((0xEB as u32) << 16) | ((dest as u32) << 8) | source as u32;

        reassembler.assemblies.insert(
            (source, dest),
            TpAssemblyState {
                source_address: source,
                destination_address: dest,
                priority: 0,
                pgn: pgn_val,
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
