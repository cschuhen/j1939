pub const ADDRESS_CLAIMED: u32 = 0x00ee00; // 60928
pub const PROCESS_DATA: u32 = 0x00cb00; // 51968
pub const REQUEST: u32 = 0x00ea00; // 59904

pub fn encode(pgn: u32) -> [u8; 3] {
    [(pgn >> 0) as u8, (pgn >> 8) as u8, (pgn >> 16) as u8]
}
pub fn decode(buf: &[u8]) -> u32 {
    (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16)
}
