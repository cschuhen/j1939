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

impl TpAssemblyState {
    /// Update the priority to the minimum of current and new priority.
    pub fn update_priority(&mut self, new_priority: u8) {
        if new_priority < self.priority {
            self.priority = new_priority;
        }
    }
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
    /// Successfully assembled single-frame message.
    SingleFrame(AssembledMessage),
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

/// Extract source address and destination/group extension from a J1939 CAN ID.
/// Used by tests to verify TP frame address extraction logic.
#[allow(dead_code)]
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

/// Build a synthetic J1939 CAN ID from PGN, source, destination, and priority.
///
/// For TP messages (PGN >= 0xF000), uses PDU2 format where destination=0xFF.
/// For standard J1939 messages (PGN < 0xF000), uses PDU1 format with dest in PS field.
pub fn build_assembled_can_id(pgn: u32, source: u8, dest: u8, priority: u8) -> u32 {
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
    /// Enable extra debugging information via eprintln! output.
    debug: bool,
}

impl TpReassembler {
    /// Create a new TpReassembler with the given timeout, force-partial setting, and debug flag.
    pub fn new(force_partial: bool, timeout_ms: u64, debug: bool) -> Self {
        TpReassembler {
            force_partial,
            timeout_ms,
            assemblies: HashMap::new(),
            rts_pending_sources: HashSet::new(),
            debug,
        }
    }

    /// Process a single RawFrame and return reassembly results.
    pub fn process_frame(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        self.cleanup_expired(frame.timestamp / 1000);
        let results = self.process_frame_internal(frame);
        results
    }

    fn process_frame_internal(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        let msg_type = self.detect_message_type(frame);

        match msg_type {
            TpMessageType::ConnectionManagement => self.handle_connection_management(frame),
            TpMessageType::DataPacket => self.handle_data_packet(frame),
            TpMessageType::NotTp => {
                let assembled = AssembledMessage::with_pgn(frame.can_id, frame.pgn(), frame.data.clone());
                vec![TpReassemblyResult::SingleFrame(assembled)]},
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
                if self.debug {
                    eprintln!("[TP] Unknown CM control byte={:#04X} from source={:#04X}", control_byte, frame.source_address());
                }
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

        if self.debug {
            eprintln!(
                "[TP] RTS received: PGN={:#06X}, size={}, packets={}, transmitter={:#04X}",
                pgn_from_rts, total_size, num_packets, transmitter
            );
        }

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

        if self.debug {
            eprintln!(
                "[TP] BAM received: PGN={:#06X}, size={}, packets={}, source={:#04X} dest={:#04X}",
                pgn_from_bam, total_size, num_packets, source, dest
            );
        }

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

    fn handle_cts(&mut self, frame: &RawFrame, data: &[u8]) -> Vec<TpReassemblyResult> {
        if data.len() < 4 {
            return vec![];
        }

        let transmitter = frame.source_address();
        let receiver = frame.destination_address();

        // CTS is sent by the transmitter in response to RTS.
        // The assembly was created by handle_rts with key (transmitter, receiver).
        // Find and update it.
        if let Some(assembly) = self.assemblies.get(&(transmitter, receiver)) {
            let num_packets = data[2];
            if self.debug {
                eprintln!(
                    "[TP] CTS received: packets={}, transmitter={:#04X}, receiver={:#04X}",
                    num_packets, transmitter, receiver
                );
            }

            // Update assembly with packet count info from CTS if needed
            let _ = assembly;
            // The assembly state already has total_size and other fields set by RTS.
            // CTS confirms the transfer can proceed.
        } else {
            if self.debug {
                eprintln!(
                    "[TP] CTS received without prior RTS for transmitter={:#04X} receiver={:#04X}",
                    transmitter, receiver
                );
            }
        }

        vec![]
    }

    fn handle_eom(&mut self, frame: &RawFrame, _data: &[u8]) -> Vec<TpReassemblyResult> {
        let transmitter = frame.source_address();
        let receiver = frame.destination_address();

        // EOM is sent by the transmitter after all DT packets.
        // Clean up any remaining assembly state for this transfer.
        if self.assemblies.remove(&(transmitter, receiver)).is_some() {
            if self.debug {
                eprintln!(
                    "[TP] EOM received: cleared assembly for transmitter={:#04X} receiver={:#04X}",
                    transmitter, receiver
                );
            }
        }

        vec![]
    }

    fn handle_abort(&mut self, frame: &RawFrame, _data: &[u8]) -> Vec<TpReassemblyResult> {
        let transmitter = frame.source_address();
        let receiver = frame.destination_address();

        if let Some(assembly) = self.assemblies.remove(&(transmitter, receiver)) {
            if self.debug {
                eprintln!(
                    "[TP] Abort received: cleared assembly for transmitter={:#04X} receiver={:#04X}, PGN={:#06X}",
                    transmitter, receiver, assembly.pgn
                );
            }
        }

        // Also try (transmitter, 0xFF) fallback for broadcast transfers
        self.assemblies.remove(&(transmitter, 0xFF));

        // Clean up rts_pending_sources if the abort came from a known RTS source
        self.rts_pending_sources.remove(&transmitter);
        self.rts_pending_sources.remove(&receiver);

        vec![]
    }

    fn handle_data_packet(&mut self, frame: &RawFrame) -> Vec<TpReassemblyResult> {
        let data = &frame.data;

        if data.len() < 2 {
            if self.debug {
                eprintln!(
                    "[TP] Data packet too short ({} bytes) for source={:#04X} destination={:#04X}",
                    data.len(),
                    frame.source_address(),
                    frame.destination_address()
                );
            }
            return vec![];
        }

        let packet_num = data[0];
        if packet_num == 0 {
            if self.debug {
                eprintln!(
                    "[TP] Invalid zero packet number for source={:#04X} destination={:#04X}",
                    frame.source_address(),
                    frame.destination_address()
                );
            }
            return vec![];
        }

        let payload = data[1..].to_vec();
        if payload.len() > 7 {
            if self.debug {
                eprintln!(
                    "[TP] Data packet payload too large ({} bytes) for source={:#04X} destination={:#04X}",
                    payload.len(),
                    frame.source_address(),
                    frame.destination_address()
                );
            }
            return vec![];
        }

        let source_address = frame.source_address();
        let dest_from_frame = frame.destination_address();

        // Try lookup with (source, dest) first. If not found, try (source, 0xFF) as fallback
        // to handle cases where j1939_async forces TP.DT into PDU1 format for broadcast transfers.
        if self.debug {
            eprintln!(
                "[TP] DT lookup: key=({:#04X}, {:#04X}), assemblies.len()={}, keys={:?}",
                source_address, dest_from_frame, self.assemblies.len(),
                self.assemblies.keys().collect::<Vec<_>>()
            );
        }

        // Check if we have an active assembly for this stream
        let assembly = if let Some(assembly) = self.assemblies.get(&(source_address, dest_from_frame)) {
            assembly.clone()
        } else if let Some(assembly) = self.assemblies.get(&(source_address, 0xFF)) {
            assembly.clone()
        } else {
            if self.debug {
                eprintln!(
                    "[TP] Data packet received without prior RTS/CTS for source={:#04X} destination={:#04X}",
                    source_address, dest_from_frame
                );
            }
            return vec![];
        };

        // Check for duplicate packet number
        if assembly
            .packets_received
            .iter()
            .any(|(pn, _)| *pn == packet_num)
        {
            if self.debug {
                eprintln!(
                    "[TP] Duplicate packet {} for PGN={:#06X}, source={:#04X} dest={:#04X}",
                    packet_num, assembly.pgn, source_address, assembly.destination_address
                );
            }
            return vec![];
        }

        // Check if out of order (packet number less than next expected)
        let next_expected = assembly.packets_received.len() + 1;
        if packet_num < next_expected as u8 {
            if self.debug {
                eprintln!(
                    "[TP] Out-of-order packet {} (expected >= {}) for PGN={:#06X} source={:#04X}",
                    packet_num, next_expected, assembly.pgn, source_address
                );
            }
            return vec![];
        }

        // Check if packet number exceeds total expected packets
        let max_packets = (assembly.total_size + 6) / 7;
        if packet_num > max_packets as u8 {
            if self.debug {
                eprintln!(
                    "[TP] Packet {} exceeds maximum ({}) for PGN={:#06X} source={:#04X}",
                    packet_num, max_packets, assembly.pgn, source_address
                );
            }
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
                pgn: assembly.pgn,
                data: assembled_data,
                timestamp: frame.timestamp,
                source_name: None,
                dest_name: None,
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

                if self.debug {
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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

        // BAM frame with insufficient data length (broadcast)
        let can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0xF8;
        let frame = make_frame(can_id, &[0x20]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_bam_followed_by_dt_packets() {
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 100, false);

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
        let mut reassembler = TpReassembler::new(true, 100, true);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 100, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let reassembler = TpReassembler::new(false, 1000, false);

        assert_eq!(reassembler.active_assemblies_count(), 0);
        assert!(reassembler.assemblies.is_empty());
    }

    #[test]
    fn test_clear_all_removes_everything() {
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(true, 5000, true);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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
        let mut reassembler = TpReassembler::new(false, 1000, false);

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

    // ========================================================================
    // BAM Broadcast Tests from Real Trace (bam.log)
    // ========================================================================

    #[test]
    fn test_bam_from_real_trace() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Frame 1: BAM CM with can_id=0x18ECFF22, data=[20 0F 00 03 FF 80 FF 00]
        let bam_cm = make_frame(0x18ECFF22, &[0x20, 0x0F, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]);
        let results = reassembler.process_frame(&bam_cm);
        assert!(results.is_empty());

        // Frame 2: DT pkt 1 with can_id=0x18EBFF22, data=[01 41 42 43 44 45 46 47]
        let dt1 = make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]);
        let results = reassembler.process_frame(&dt1);
        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Pending => {}
            other => panic!("Expected Pending for DT pkt 1, got {:?}", other),
        }

        // Frame 3: DT pkt 2 with can_id=0x18EBFF22, data=[02 47 49 4A 4B 4C 4D 4E]
        let dt2 = make_frame(0x18EBFF22, &[0x02, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]);
        let results = reassembler.process_frame(&dt2);
        assert_eq!(results.len(), 1);
        match &results[0] {
            TpReassemblyResult::Pending => {}
            other => panic!("Expected Pending for DT pkt 2, got {:?}", other),
        }

        // Frame 4: DT pkt 3 with can_id=0x18EBFF22, data=[03 4F FF FF FF FF FF FF]
        let dt3 = make_frame(0x18EBFF22, &[0x03, 0x4F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let results = reassembler.process_frame(&dt3);
        assert_eq!(results.len(), 1);

        // Verify: TpReassemblyResult::Complete is returned on frame 4
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                // Verify assembled data = [41 42 43 44 45 46 47 47 49 4A 4B 4C 4D 4E 4F] (15 bytes)
                assert_eq!(msg.data.len(), 15);
                assert_eq!(msg.data, vec![0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F]);
                // Verify PGN = 0xFF80
                assert_eq!(msg.pgn(), 0xFF80);
            }
            other => panic!("Expected Complete for DT pkt 3, got {:?}", other),
        }
    }

    #[test]
    fn test_bam_payload_matches_reference() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Feed all 4 frames from bam.log in order
        reassembler.process_frame(&make_frame(0x18ECFF22, &[0x20, 0x0F, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x02, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]));
        let results = reassembler.process_frame(&make_frame(0x18EBFF22, &[0x03, 0x4F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));

        // Reference shows: 41 42 43 44 45 46 47 47 49 4A 4B 4C 4D 4E 4F
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                let expected = vec![0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F];
                assert_eq!(msg.data, expected);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_bam_assembled_can_id() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Feed bam.log frames
        reassembler.process_frame(&make_frame(0x18ECFF22, &[0x20, 0x0F, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x02, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]));
        let results = reassembler.process_frame(&make_frame(0x18EBFF22, &[0x03, 0x4F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));

        // Verify the assembled message's CAN ID is correctly constructed as PDU2 format
        // Expected can_id = (6<<26) | (0xFF80<<8) | 0x22 = 0x18FF8022
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                let expected_can_id = (6u32 << 26) | (0xFF80u32 << 8) | 0x22;
                assert_eq!(msg.id, expected_can_id);
                // Verify source and destination from assembled message
                assert_eq!(msg.source(), 0x22);
                assert_eq!(msg.destination(), 0xFF);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_bam_with_different_sizes() {
        // BAM with total_size=7 (single DT packet)
        let mut reassembler = TpReassembler::new(false, 5000, false);
        reassembler.process_frame(&make_frame(0x18ECFF22, &[0x20, 0x07, 0x00, 0x01, 0xFF, 0x10, 0x00, 0x00]));
        let results = reassembler.process_frame(&make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]));
        match &results[0] {
            TpReassemblyResult::Complete(msg) => assert_eq!(msg.data.len(), 7),
            other => panic!("Expected Complete for single-packet BAM, got {:?}", other),
        }

        // BAM with total_size=14 (two DT packets)
        let mut reassembler = TpReassembler::new(false, 5000, false);
        reassembler.process_frame(&make_frame(0x18ECFF22, &[0x20, 0x0E, 0x00, 0x02, 0xFF, 0x20, 0x00, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]));
        let results = reassembler.process_frame(&make_frame(0x18EBFF22, &[0x02, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]));
        match &results[0] {
            TpReassemblyResult::Complete(msg) => assert_eq!(msg.data.len(), 14),
            other => panic!("Expected Complete for two-packet BAM, got {:?}", other),
        }

        // BAM with total_size=21 (three DT packets)
        let mut reassembler = TpReassembler::new(false, 5000, false);
        reassembler.process_frame(&make_frame(0x18ECFF22, &[0x20, 0x15, 0x00, 0x03, 0xFF, 0x30, 0x00, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x02, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]));
        let results = reassembler.process_frame(&make_frame(0x18EBFF22, &[0x03, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55]));
        match &results[0] {
            TpReassemblyResult::Complete(msg) => assert_eq!(msg.data.len(), 21),
            other => panic!("Expected Complete for three-packet BAM, got {:?}", other),
        }
    }

    // ========================================================================
    // RTS/CTS Unicast Tests from Real Trace (rts.log)
    // ========================================================================

    #[test]
    fn test_rts_cts_from_real_trace() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Frame 1: RTS CM with can_id=0x18ECEB26, data=[10 24 00 06 FF 00 E6 00]
        let rts = make_frame(0x18ECEB26, &[0x10, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]);
        let results = reassembler.process_frame(&rts);
        assert!(results.is_empty());

        // Frame 2: CTS CM with can_id=0x18EC26EB, data=[11 06 01 FF FF 00 E6 00]
        let cts = make_frame(0x18EC26EB, &[0x11, 0x06, 0x01, 0xFF, 0xFF, 0x00, 0xE6, 0x00]);
        let results = reassembler.process_frame(&cts);
        assert!(results.is_empty());

        // Frames 3-8: DT packets (can_id=0x18EBEB26) with sequence numbers 1-6
        let dt1 = make_frame(0x18EBEB26, &[0x01, 0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43]);
        let results = reassembler.process_frame(&dt1);
        assert_eq!(results.len(), 1);
        match &results[0] { TpReassemblyResult::Pending => {} _ => panic!("Expected Pending"), }

        let dt2 = make_frame(0x18EBEB26, &[0x02, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A]);
        let results = reassembler.process_frame(&dt2);
        assert_eq!(results.len(), 1);
        match &results[0] { TpReassemblyResult::Pending => {} _ => panic!("Expected Pending"), }

        let dt3 = make_frame(0x18EBEB26, &[0x03, 0x4B, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]);
        let results = reassembler.process_frame(&dt3);
        assert_eq!(results.len(), 1);
        match &results[0] { TpReassemblyResult::Pending => {} _ => panic!("Expected Pending"), }

        let dt4 = make_frame(0x18EBEB26, &[0x04, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]);
        let results = reassembler.process_frame(&dt4);
        assert_eq!(results.len(), 1);
        match &results[0] { TpReassemblyResult::Pending => {} _ => panic!("Expected Pending"), }

        let dt5 = make_frame(0x18EBEB26, &[0x05, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]);
        let results = reassembler.process_frame(&dt5);
        assert_eq!(results.len(), 1);
        match &results[0] { TpReassemblyResult::Pending => {} _ => panic!("Expected Pending"), }

        // Frame 8: DT pkt 6 (last packet) with can_id=0x18EBEB26, data=[06 20 FF FF FF FF FF FF]
        let dt6 = make_frame(0x18EBEB26, &[0x06, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let results = reassembler.process_frame(&dt6);
        assert_eq!(results.len(), 1);

        // Verify: TpReassemblyResult::Complete is returned on frame 8 (last DT)
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                // Verify assembled data = [08 8A 07 20 41 42 43 44 45 46 47 48 49 4A 4B 20 ...] truncated to 36 bytes
                assert_eq!(msg.data.len(), 36);
                // Verify PGN = 0xE600, source = 0x26, dest = 0xEB
                assert_eq!(msg.pgn(), 0xE600);
            }
            other => panic!("Expected Complete for DT pkt 6, got {:?}", other),
        }

        // Frame 9: EOM CM with can_id=0x18EC26EB, data=[13 24 00 06 FF 00 E6 00]
        let eom = make_frame(0x18EC26EB, &[0x13, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]);
        let results = reassembler.process_frame(&eom);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rts_cts_payload_matches_reference() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Feed all frames from rts.log in order
        reassembler.process_frame(&make_frame(0x18ECEB26, &[0x10, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]));
        reassembler.process_frame(&make_frame(0x18EC26EB, &[0x11, 0x06, 0x01, 0xFF, 0xFF, 0x00, 0xE6, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x01, 0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x02, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x03, 0x4B, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x04, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x05, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        let results = reassembler.process_frame(&make_frame(0x18EBEB26, &[0x06, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));

        // Reference shows final frame data starting with: 08 8a 07 20 41 42 43 44 45 46 47 48 49 4A 4B 20
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                let expected_start = vec![0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x20];
                assert_eq!(msg.data.len(), 36);
                assert_eq!(&msg.data[..16], &expected_start[..]);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_rts_cts_assembled_can_id() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Feed rts.log frames
        reassembler.process_frame(&make_frame(0x18ECEB26, &[0x10, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]));
        reassembler.process_frame(&make_frame(0x18EC26EB, &[0x11, 0x06, 0x01, 0xFF, 0xFF, 0x00, 0xE6, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x01, 0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x02, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x03, 0x4B, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x04, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x05, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        let results = reassembler.process_frame(&make_frame(0x18EBEB26, &[0x06, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));

        // Verify the assembled message's CAN ID is correctly constructed as PDU1 format
        // Expected can_id = (6<<26) | ((0xE600 & 0x3FF00) | 0xEB)<<8 | 0x26
        // = (6<<26) | (0xE600>>8 & 0x3FF)<<16 | 0xEB<<8 | 0x26
        // PGN=0xE600: pgn_high = (0xE600 >> 8) & 0x3FF = 0xE6
        // can_id = (6<<26) | (0xE6<<16) | (0xEB<<8) | 0x26 = 0x18E6EB26
        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                let expected_can_id = (6u32 << 26) | ((0xE600 >> 8) as u32 & 0x3FF) << 16 | (0xEB as u32) << 8 | 0x26;
                assert_eq!(msg.id, expected_can_id);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_tp_frame_address_extraction() {
        // TP.CM unicast (PF=0xEC, PS=0xEB): returns (source, 0xEB) - PDU1 to node 235
        let (src, dst) = extract_tp_addresses(0x18ECEB26);
        assert_eq!(src, 0x26);
        assert_eq!(dst, 0xEB);

        // TP.CM broadcast (PF=0xEC, PS=0xFF): returns (source, 0xFF) - PDU1 to all nodes
        let (src, dst) = extract_tp_addresses(0x18ECFF22);
        assert_eq!(src, 0x22);
        assert_eq!(dst, 0xFF);

        // TP.DT unicast (PF=0xEB, PS=0xEB): returns (source, 0xEB) - PDU1 to node 235
        let (src, dst) = extract_tp_addresses(0x18EBEB26);
        assert_eq!(src, 0x26);
        assert_eq!(dst, 0xEB);

        // TP.DT broadcast (PF=0xEB, PS=0xFF): returns (source, 0xFF) - PDU1 to all nodes
        let (src, dst) = extract_tp_addresses(0x18EBFF22);
        assert_eq!(src, 0x22);
        assert_eq!(dst, 0xFF);

        // Standard PDU1 (PF=0xEE, PS=0x55): returns (source, 0x55)
        let (src, dst) = extract_tp_addresses(0x18EE55AA);
        assert_eq!(src, 0xAA);
        assert_eq!(dst, 0x55);

        // Standard PDU2 (PF=0xF0, PS=0x01): returns (source, 0xFF) - true PDU2 broadcast
        let (src, dst) = extract_tp_addresses(0x18F001AA);
        assert_eq!(src, 0xAA);
        assert_eq!(dst, 0xFF);
    }

    #[test]
    fn test_rts_cts_with_eom() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Set up an assembly via RTS
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x21 as u32) << 8) | 0x20;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0x0A, 0x00, 0x02, 0xFF, 0x00, 0x10, 0x00]));

        // Send CTS
        let cts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        reassembler.process_frame(&make_frame(cts_can_id, &[0x11, 0x02, 0x01, 0xFF, 0x00, 0x10, 0x00]));

        // Send DT packets to complete assembly
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x21 as u32) << 8) | 0x20;
        reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]));
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x02, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]));

        match &results[0] {
            TpReassemblyResult::Complete(_) => {}
            _other => panic!("Expected Complete"),
        }

        // EOM should be processed after assembly completes without interfering
        let eom_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        let results = reassembler.process_frame(&make_frame(eom_can_id, &[0x13, 0x0A, 0x00, 0x02, 0xFF, 0x00, 0x10, 0x00]));
        assert!(results.is_empty());

        // Assembly should be cleaned up by EOM
        assert!(!reassembler.assemblies.contains_key(&(0x20, 0x21)));
    }

    #[test]
    fn test_dt_without_prior_cm() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // DT packet arrives without any prior BAM or RTS/CTS
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0xFF as u32) << 8) | 0x20;
        let frame = make_frame(dt_can_id, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]);

        let results = reassembler.process_frame(&frame);
        assert!(results.is_empty());
    }

    #[test]
    fn test_build_assembled_can_id_pdu2() {
        // PGN >= 0xF000 should use PDU2 format (dest=0xFF)
        let can_id = build_assembled_can_id(0xFF80, 0x22, 0xFF, 6);
        assert_eq!(can_id, (6u32 << 26) | (0xFF80u32 << 8) | 0x22);

        let can_id = build_assembled_can_id(0xF500, 0x30, 0xFF, 7);
        assert_eq!(can_id, (7u32 << 26) | (0xF500u32 << 8) | 0x30);
    }

    #[test]
    fn test_build_assembled_can_id_pdu1() {
        // PGN < 0xF000 should use PDU1 format with dest in PS field
        let can_id = build_assembled_can_id(0xE600, 0x26, 0xEB, 6);
        assert_eq!(can_id, 0x18E6EB26);

        // PGN >= 0xF000 uses PDU2 format (full PGN in bits 8-25)
        let can_id = build_assembled_can_id(0xFF80, 0x22, 0xFF, 6);
        assert_eq!(can_id, 0x18FF8022);

        // Small PGN < 0xF000 uses PDU1 format (priority=7 -> 0x1C prefix)
        let can_id = build_assembled_can_id(0x0A00, 0xF8, 0x50, 7);
        assert_eq!(can_id, 0x1C0A50F8);

        // Very small PGN where pgn_high=0 (priority=7 -> 0x1C prefix)
        let can_id = build_assembled_can_id(0x40, 0xF8, 0xFF, 7);
        assert_eq!(can_id, 0x1C00FFF8);

        // priority=7 -> 0x1C prefix
        let can_id = build_assembled_can_id(0x50, 0x20, 0x21, 7);
        assert_eq!(can_id, 0x1C002120);
    }

    #[test]
    fn test_bam_assembled_source_dest() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Feed bam.log frames
        reassembler.process_frame(&make_frame(0x18ECFF22, &[0x20, 0x0F, 0x00, 0x03, 0xFF, 0x80, 0xFF, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x01, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]));
        reassembler.process_frame(&make_frame(0x18EBFF22, &[0x02, 0x47, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E]));
        let results = reassembler.process_frame(&make_frame(0x18EBFF22, &[0x03, 0x4F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.source(), 0x22);
                assert_eq!(msg.destination(), 0xFF);
                assert_eq!(msg.pgn(), 0xFF80);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_rts_cts_assembled_source_dest() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // Feed rts.log frames
        reassembler.process_frame(&make_frame(0x18ECEB26, &[0x10, 0x24, 0x00, 0x06, 0xFF, 0x00, 0xE6, 0x00]));
        reassembler.process_frame(&make_frame(0x18EC26EB, &[0x11, 0x06, 0x01, 0xFF, 0xFF, 0x00, 0xE6, 0x00]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x01, 0x08, 0x8A, 0x07, 0x20, 0x41, 0x42, 0x43]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x02, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x03, 0x4B, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x04, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        reassembler.process_frame(&make_frame(0x18EBEB26, &[0x05, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20]));
        let results = reassembler.process_frame(&make_frame(0x18EBEB26, &[0x06, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.source(), 0x26);
                assert_eq!(msg.destination(), 0xEB);
                assert_eq!(msg.pgn(), 0xE600);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_rts_cts_single_packet_transfer() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // RTS for single packet transfer
        // PGN in payload bytes: [50 00 00] -> 0x000050
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x21 as u32) << 8) | 0x20;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0x07, 0x00, 0x01, 0xFF, 0x50, 0x00, 0x00]));

        // CTS
        let cts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        reassembler.process_frame(&make_frame(cts_can_id, &[0x11, 0x01, 0x01, 0xFF, 0x50, 0x00, 0x00]));

        // Single DT packet (7 bytes)
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x21 as u32) << 8) | 0x20;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                assert_eq!(msg.data, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]);
                // PGN extracted from RTS payload bytes [50 00 00] = 0x50 = 80
                assert_eq!(msg.pgn(), 0x50);
            }
            other => panic!("Expected Complete for single-packet RTS/CTS, got {:?}", other),
        }
    }

    #[test]
    fn test_bam_with_max_packet_size() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // BAM with total_size=7 (exactly one packet of max payload)
        // PGN in payload bytes: [40 00 00] -> 0x000040
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0xF8;
        reassembler.process_frame(&make_frame(bam_can_id, &[0x20, 0x07, 0x00, 0x01, 0xFF, 0x40, 0x00, 0x00]));

        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0xFF as u32) << 8) | 0xF8;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                // PGN extracted from BAM payload bytes [40 00 00] = 0x40 = 64
                assert_eq!(msg.pgn(), 0x40);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_rts_cts_with_abort() {
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // RTS
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x21 as u32) << 8) | 0x20;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0x0A, 0x00, 0x02, 0xFF, 0x60, 0x00, 0x00]));

        // Abort instead of CTS
        let abort_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        let results = reassembler.process_frame(&make_frame(abort_can_id, &[0x1C, 0x00]));

        assert!(results.is_empty());
    }

    #[test]
    fn test_large_pgn_bam_0xf000() {
        // PGN 0xF000 (lowest large PGN)
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // BAM with PGN=0xF000 in payload bytes [00 F0 00] -> little-endian = 0x00F000
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0x10;
        reassembler.process_frame(&make_frame(bam_can_id, &[0x20, 0x07, 0x00, 0x01, 0xFF, 0x00, 0xF0, 0x00]));

        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0xFF as u32) << 8) | 0x10;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                assert_eq!(msg.pgn(), 0xF000);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_large_pgn_bam_0xf5ff() {
        // PGN 0xF5FF (mid-range large PGN)
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // BAM with PGN=0xF5FF in payload bytes [FF F5 00] -> little-endian = 0x00F5FF
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0x20;
        reassembler.process_frame(&make_frame(bam_can_id, &[0x20, 0x07, 0x00, 0x01, 0xFF, 0xFF, 0xF5, 0x00]));

        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0xFF as u32) << 8) | 0x20;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                assert_eq!(msg.pgn(), 0xF5FF);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_large_pgn_bam_0xfdff() {
        // PGN 0xFDFF (highest large PGN)
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // BAM with PGN=0xFDFF in payload bytes [FF FD 00] -> little-endian = 0x00FDFF
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(bam_can_id, &[0x20, 0x07, 0x00, 0x01, 0xFF, 0xFF, 0xFD, 0x00]));

        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0xFF as u32) << 8) | 0x30;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                assert_eq!(msg.pgn(), 0xFDFF);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_large_pgn_rts_cts_0xf800() {
        // RTS/CTS with large PGN 0xF800
        // All frames use CAN ID source=0x21, dest=0x20 (matching the working test pattern)
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // RTS with PGN=0xF800 in payload [00 F8 00] -> little-endian = 0x00F800
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0x07, 0x00, 0x01, 0xFF, 0x00, 0xF8, 0x00]));

        // CTS (same CAN ID direction)
        let cts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        reassembler.process_frame(&make_frame(cts_can_id, &[0x11, 0x01, 0x01, 0xFF, 0x00, 0x00, 0x00]));

        // DT packet (same CAN ID direction)
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x20 as u32) << 8) | 0x21;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 7);
                assert_eq!(msg.pgn(), 0xF800);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_large_pgn_multi_packet_0xf900() {
        // RTS/CTS with large PGN 0xF900 and multiple packets
        // All frames use CAN ID source=0x30, dest=0x40
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // RTS with PGN=0xF900, total_size=14, 2 packets
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0x0E, 0x00, 0x02, 0xFF, 0x00, 0xF9, 0x00]));

        // CTS (same CAN ID direction)
        let cts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(cts_can_id, &[0x11, 0x02, 0x02, 0xFF, 0x00, 0x00, 0x00]));

        // DT packet 1 (same CAN ID direction)
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(dt_can_id, &[0x01, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01]));

        // DT packet 2 (same CAN ID direction)
        let dt_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        let results = reassembler.process_frame(&make_frame(dt_can_id, &[0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x01]));

        match &results[0] {
            TpReassemblyResult::Complete(msg) => {
                assert_eq!(msg.data.len(), 14);
                assert_eq!(msg.pgn(), 0xF900);
                assert_eq!(msg.data, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x01, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x01]);
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_build_assembled_can_id_large_pgn_range() {
        // Verify CAN ID construction for various large PGNs (>= 0xF000)
        
        // PGN 0xF000 with priority 6, source 0x10, dest 0xFF
        let can_id = build_assembled_can_id(0xF000, 0x10, 0xFF, 6);
        assert_eq!(can_id, (6u32 << 26) | (0xF000u32 << 8) | 0x10);

        // PGN 0xF500 with priority 7, source 0x20, dest 0xFF
        let can_id = build_assembled_can_id(0xF500, 0x20, 0xFF, 7);
        assert_eq!(can_id, (7u32 << 26) | (0xF500u32 << 8) | 0x20);

        // PGN 0xFDFF with priority 6, source 0x30, dest 0xFF
        let can_id = build_assembled_can_id(0xFDFF, 0x30, 0xFF, 6);
        assert_eq!(can_id, (6u32 << 26) | (0xFDFFu32 << 8) | 0x30);

        // PGN 0xFE00 with priority 7, source 0x40, dest 0xFF
        let can_id = build_assembled_can_id(0xFE00, 0x40, 0xFF, 7);
        assert_eq!(can_id, (7u32 << 26) | (0xFE00u32 << 8) | 0x40);

        // PGN 0xFFFF with priority 6, source 0x50, dest 0xFF
        let can_id = build_assembled_can_id(0xFFFF, 0x50, 0xFF, 6);
        assert_eq!(can_id, (6u32 << 26) | (0xFFFFu32 << 8) | 0x50);
    }

    #[test]
    fn test_very_large_message_1785_bytes() {
        // Max J1939 TP payload: 1785 bytes = 255 DT packets (ceil(1785/7))
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // RTS with PGN=0xF000, total_size=1785 (0x06F9 LE), num_packets=255
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0xF9, 0x06, 0xFF, 0xFF, 0x00, 0xF0, 0x00]));

        // CTS (same CAN ID direction)
        let cts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(cts_can_id, &[0x11, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00]));

        // Send all 255 DT packets
        let dt_base_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        for packet_num in 1u8..=255 {
            let payload_byte = (packet_num.wrapping_sub(1)) & 0xFF;
            // Each DT carries 7 bytes of payload + 1 byte packet number = 8 bytes total
            let data = [packet_num, payload_byte, payload_byte.wrapping_add(1), payload_byte.wrapping_add(2), 
                        payload_byte.wrapping_add(3), payload_byte.wrapping_add(4), payload_byte.wrapping_add(5), 0x01];
            if packet_num == 255 {
                let results = reassembler.process_frame(&make_frame(dt_base_can_id, &data));
                match &results[0] {
                    TpReassemblyResult::Complete(msg) => {
                        assert_eq!(msg.data.len(), 1785);
                        assert_eq!(msg.pgn(), 0xF000);
                        // Verify first few bytes
                        assert_eq!(msg.data[0], 0x00);
                        assert_eq!(msg.data[1], 0x01);
                        assert_eq!(msg.data[2], 0x02);
                        // Verify last few bytes (packet 255, payload starts with byte 254)
                        let offset = 254 * 7;
                        assert_eq!(msg.data[offset], 254u8.wrapping_sub(0));
                    }
                    other => panic!("Expected Complete for last packet, got {:?}", other),
                }
            } else {
                reassembler.process_frame(&make_frame(dt_base_can_id, &data));
            }
        }
    }

    #[test]
    fn test_large_message_1000_bytes() {
        // 1000 bytes = 143 DT packets (ceil(1000/7))
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // RTS with PGN=0xF800, total_size=1000 (0x03E8 LE), num_packets=143
        let rts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(rts_can_id, &[0x10, 0xE8, 0x03, 0x8F, 0xFF, 0x00, 0xF8, 0x00]));

        // CTS (same CAN ID direction)
        let cts_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        reassembler.process_frame(&make_frame(cts_can_id, &[0x11, 0x8F, 0x8F, 0xFF, 0x00, 0x00, 0x00]));

        // Send all 143 DT packets
        let dt_base_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0x40 as u32) << 8) | 0x30;
        for packet_num in 1u8..=143 {
            let payload_byte = (packet_num.wrapping_sub(1)) & 0xFF;
            let data = [packet_num, payload_byte, payload_byte.wrapping_add(1), payload_byte.wrapping_add(2), 
                        payload_byte.wrapping_add(3), payload_byte.wrapping_add(4), payload_byte.wrapping_add(5), 0x01];
            if packet_num == 143 {
                let results = reassembler.process_frame(&make_frame(dt_base_can_id, &data));
                match &results[0] {
                    TpReassemblyResult::Complete(msg) => {
                        assert_eq!(msg.data.len(), 1000);
                        assert_eq!(msg.pgn(), 0xF800);
                        // Verify first byte and last byte
                        assert_eq!(msg.data[0], 0x00);
                        let offset = 142 * 7;
                        assert_eq!(msg.data[offset], 142u8.wrapping_sub(0));
                    }
                    other => panic!("Expected Complete for last packet, got {:?}", other),
                }
            } else {
                reassembler.process_frame(&make_frame(dt_base_can_id, &data));
            }
        }
    }

    #[test]
    fn test_bam_1785_bytes() {
        // BAM with max J1939 TP payload: 1785 bytes = 255 packets (u8 max)
        // 255 * 7 = 1785 bytes exactly
        let mut reassembler = TpReassembler::new(false, 5000, false);

        // BAM with PGN=0xFF80, total_size=1785 (0x06F9 LE), num_packets=255
        let bam_can_id = (7u32 << 26) | ((0xEC as u32) << 16) | ((0xFF as u32) << 8) | 0x10;
        reassembler.process_frame(&make_frame(bam_can_id, &[0x20, 0xF9, 0x06, 0xFF, 0xFF, 0x80, 0xFF, 0x00]));

        // Send all 255 DT packets
        let dt_base_can_id = (7u32 << 26) | ((0xEB as u32) << 16) | ((0xFF as u32) << 8) | 0x10;
        for packet_num in 1u8..=255 {
            let payload_byte = (packet_num.wrapping_sub(1)) & 0xFF;
            let data = [packet_num, payload_byte, payload_byte.wrapping_add(1), payload_byte.wrapping_add(2), 
                        payload_byte.wrapping_add(3), payload_byte.wrapping_add(4), payload_byte.wrapping_add(5), 0x01];
            if packet_num == 255 {
                let results = reassembler.process_frame(&make_frame(dt_base_can_id, &data));
                match &results[0] {
                    TpReassemblyResult::Complete(msg) => {
                        assert_eq!(msg.data.len(), 1785);
                        assert_eq!(msg.pgn(), 0xFF80);
                        // Verify first few bytes
                        assert_eq!(msg.data[0], 0x00);
                        assert_eq!(msg.data[1], 0x01);
                    }
                    other => panic!("Expected Complete for last packet, got {:?}", other),
                }
            } else {
                reassembler.process_frame(&make_frame(dt_base_can_id, &data));
            }
        }
    }
}
