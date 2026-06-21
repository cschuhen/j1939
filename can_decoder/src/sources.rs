use std::error::Error;
use std::fs::File;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use socketcan::{CanDataFrame, CanFrame, CanSocket, EmbeddedFrame, Frame, Socket, SocketOptions};
use tokio::sync::mpsc;

use crate::traits::Source;
use crate::types::RawFrame;

/// Reads raw CAN frames from a SocketCAN interface.
///
/// Implements the `Source` trait to feed `RawFrame`s into the pipeline via an unbounded channel.
/// Uses `tokio::task::spawn_blocking` for all I/O since the underlying socketcan crate is synchronous.
/// On startup, sends a J1939 "Request all address claims" broadcast (PGN 0x0EA00).
pub struct SocketCanSource {
    /// Human-readable name including interface identifier.
    name: String,
    /// SocketCAN interface name (e.g., "can0", "can1").
    iface: String,
}

impl SocketCanSource {
    /// Create a new SocketCanSource for the given interface (e.g., "can0").
    pub fn new(iface: &str) -> Self {
        let iface_str = iface.to_string();
        SocketCanSource {
            name: format!("socketcan:{}", iface_str),
            iface: iface_str,
        }
    }

    /// Open the CAN socket and enable reception of all error frames.
    fn open_socket(iface: &str) -> Result<CanSocket, Box<dyn Error + Send + Sync>> {
        let addr = socketcan::CanAddr::from_iface(iface)?;
        let sock = CanSocket::open_addr(&addr)?;

        // Enable reception of error frames by setting ERR_FILTER to CAN_ERR_MASK (all errors)
        use socketcan::socket::{CAN_RAW_ERR_FILTER, SOL_CAN_RAW};
        let err_mask: u32 = 0xFFFFFFFF; // Receive all error types
        sock.set_socket_option(SOL_CAN_RAW, CAN_RAW_ERR_FILTER, &err_mask)?;

        Ok(sock)
    }

    /// Convert a socketcan CanFrame to our RawFrame type.
    fn frame_to_raw(frame: &CanFrame) -> RawFrame {
        let can_id = frame.id_word();
        let data = frame.data()[..frame.dlc()].to_vec();
        RawFrame::new(can_id, data)
    }

    /// Send a J1939 "Request all address claims" broadcast (PGN 0x0EA00).
    ///
    /// Per requirements: when running in live mode, send this request onto the bus.
    fn send_request_all_address_claims(sock: &CanSocket) -> Result<(), Box<dyn Error + Send + Sync>> {
        // PGN Request for "all address claims" uses PDU1 format:
        // [3-bit priority][1-bit DP][8-bit reserved][7-bit PGN extension][8-bit PGN base]
        let pgn = 0x0EA00u32;
        let priority = 6u32;

        // Build extended CAN ID for J1939 TP DM Request All Address Claims
        let can_id = ((priority as u32) << 26) | (pgn & 0x1FFFFF);

        if let Some(frame) = CanDataFrame::new(
            socketcan::Id::Extended(socketcan::ExtendedId::new(can_id).unwrap()),
            &[0u8; 8],
        ) {
            let _ = sock.write_frame(&frame);
        }
        Ok(())
    }
}

impl Source for SocketCanSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn start(
        self: Arc<Self>,
        tx: mpsc::UnboundedSender<RawFrame>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>> {
        let iface = self.iface.clone();
        let name = self.name.clone();

        Box::pin(async move {
            // Open socket in a blocking task since CanSocket is synchronous
            println!("[{}] Opening interface {}...", name, iface);
            let sock_iface = iface.clone();
            let sock = tokio::task::spawn_blocking(move || SocketCanSource::open_socket(&iface))
                .await
                .map_err(|e| format!("Join error: {}", e))?;

            match sock {
                Ok(s) => {
                    println!("[{}] Listening...", name);

                    // Send "Request all address claims" broadcast per requirements
                    if SocketCanSource::send_request_all_address_claims(&s).is_err() {
                        eprintln!("[{}] Warning: Failed to send request all address claims", name);
                    }

                    // Wrap socket in Arc so it can be shared across spawn_blocking calls
                    let sock = Arc::new(s);
                    loop {
                        let frame_result = {
                            let sock_clone = Arc::clone(&sock);
                            tokio::task::spawn_blocking(move || sock_clone.read_frame())
                                .await
                                .map_err(|e| Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other, format!("Join error: {}", e)
                                )) as Box<dyn Error + Send + Sync>)
                                .and_then(|r| r.map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>))
                        };

                        match frame_result {
                            Ok(frame) => {
                                if tx.send(SocketCanSource::frame_to_raw(&frame)).is_err() {
                                    println!("[{}] Channel closed, stopping.", name);
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("[{}] Read error: {}", name, e);
                                // Continue reading - don't crash on transient errors
                                std::thread::sleep(std::time::Duration::from_millis(10));
                            }
                        }
                    }

                    Ok(())
                }
                Err(e) => Err(format!("Failed to open CAN interface '{}': {}", sock_iface, e).into()),
            }
        })
    }
}

/// Reads raw CAN frames from a candump-style log file.
///
/// Parses lines in the format: `(timestamp_sec.timestamp_usec) iface can_id#data`
/// where `can_id` may be 3-digit (standard) or 8-digit (extended), and `data` is hex bytes.
pub struct CandumpFileSource {
    /// Human-readable name including file path.
    name: String,
    /// Path to the candump-style capture file.
    input_file: PathBuf,
}

impl CandumpFileSource {
    /// Create a new CandumpFileSource for the given file path.
    pub fn new(input_file: impl Into<PathBuf>) -> Self {
        let path = input_file.into();
        let name = format!("candump:{}", path.display());
        CandumpFileSource { name, input_file: path }
    }

    /// Parse a single candump line into a RawFrame.
    ///
    /// Expected format: `(1234567890.123456) can0 7E8#DEADBEEF`
    /// Returns None for blank lines or comment lines starting with '#'.
    fn parse_line(line: &str) -> Option<RawFrame> {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        // Parse: (timestamp_sec.usec) interface can_id#data
        let open_paren = line.find('(')?;
        let close_paren = line.find(')')?;
        let timestamp_str = &line[open_paren + 1..close_paren];

        let rest = &line[close_paren + 2..]; // skip ") "

        // Split off interface name, then can_id#data
        // Format: "(timestamp) iface can_id#data"
        let space_after_iface = rest.find(' ')?;
        let id_data = &rest[space_after_iface + 1..]; // skip "can0 "

        // Split can_id#data
        let hash_pos = id_data.find('#')?;
        let id_str = &id_data[..hash_pos].trim();
        let data_str = &id_data[hash_pos + 1..];

        // Parse timestamp as seconds.microseconds
        let parts: Vec<&str> = timestamp_str.split('.').collect();
        if parts.len() != 2 {
            eprintln!("Warning: malformed timestamp '{}'", timestamp_str);
            return None;
        }

        let sec: u64 = parts[0].parse().ok()?;
        let usec: u64 = parts[1].parse().ok()?;
        let timestamp = (sec * 1_000_000) + (usec % 1_000_000);

        // Parse CAN ID (hex, may be 3-digit standard or 8-digit extended)
        let can_id: u32 = u32::from_str_radix(id_str.trim(), 16).ok()?;

        // Parse data bytes (hex pairs separated by spaces, or continuous hex string)
        let data_bytes: Vec<u8> = if data_str.contains(' ') {
            data_str.split_whitespace()
                .filter_map(|b| u8::from_str_radix(b, 16).ok())
                .collect()
        } else {
            // Continuous hex string like DEADBEEF -> [DE, AD, BE, EF]
            (0..data_str.len())
                .step_by(2)
                .filter_map(|i| {
                    if i + 2 <= data_str.len() {
                        u8::from_str_radix(&data_str[i..i+2], 16).ok()
                    } else {
                        None
                    }
                })
                .collect()
        };

        Some(RawFrame { timestamp, can_id, data: data_bytes })
    }

    /// Read and parse all frames from the file in a blocking task.
    fn read_file(path: &PathBuf) -> Result<Vec<RawFrame>, Box<dyn Error + Send + Sync>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut frames = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            match line_result {
                Ok(line) => {
                    if let Some(frame) = CandumpFileSource::parse_line(&line) {
                        frames.push(frame);
                    } else if !line.trim().is_empty() && !line.trim().starts_with('#') {
                        eprintln!("Warning: skipping malformed line {}: {}", line_num + 1, line);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: error reading line {}: {}", line_num + 1, e);
                }
            }
        }

        Ok(frames)
    }
}

impl Source for CandumpFileSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn start(
        self: Arc<Self>,
        tx: mpsc::UnboundedSender<RawFrame>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>> {
        let path = self.input_file.clone();
        let name = self.name.clone();

        Box::pin(async move {
            println!("[{}] Reading file: {}", name, path.display());

            // Read entire file in blocking task
            let display_path = path.clone();
            let frames_result = tokio::task::spawn_blocking(move || CandumpFileSource::read_file(&path))
                .await
                .map_err(|e| format!("Join error: {}", e))?;

            match frames_result {
                Ok(frames) => {
                    println!("[{}] Loaded {} frames", name, frames.len());

                    for (i, frame) in frames.iter().enumerate() {
                        if tx.send(frame.clone()).is_err() {
                            println!("[{}] Channel closed after {} frames.", name, i);
                            break;
                        }
                    }

                    println!("[{}] Finished sending all frames.", name);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to read file '{}': {}", display_path.display(), e).into()),
            }
        })
    }
}
