/// Pure formatting utilities for display across renderers and GUIs.
/// These functions have no dependencies on TUI frameworks.
use crate::types::{FlagValue, Numeric};

/// Format a Numeric value into a human-readable string (no color codes).
pub fn format_value(value: &Numeric) -> String {
    match value {
        Numeric::Int(i) => format!("{}", i),
        Numeric::Float(f) => format!("{:.4}", f),
        Numeric::Hex(h) => format!("0x{:X}", h),
        Numeric::Bool(b) => format!("{}", b),
        Numeric::Flag(flag_value) => match flag_value {
            FlagValue::Off => "OFF".to_string(),
            FlagValue::On => "ON".to_string(),
            FlagValue::Error => "ERR".to_string(),
            FlagValue::Unavailable => "N/A".to_string(),
        },
    }
}

/// Format a microsecond timestamp into "seconds.useconds" string.
pub fn format_timestamp(timestamp: u64) -> String {
    let seconds = timestamp / 1_000_000;
    let usec = timestamp % 1_000_000;
    format!("{}.{}", seconds, format!("{:06}", usec))
}

/// Format elapsed time since a global start point as "seconds.milliseconds".
pub fn format_elapsed_time(timestamp: u64, global_start: Option<u64>) -> String {
    if let Some(start) = global_start {
        let elapsed_us = timestamp.saturating_sub(start);
        let secs = elapsed_us / 1_000_000;
        let frac_ms = (elapsed_us % 1_000_000) / 1000;
        format!("{}.{:03}", secs, frac_ms)
    } else {
        "0.000".to_string()
    }
}

/// Format a byte slice as space-separated hex bytes, truncated to 8 bytes with "...".
pub fn format_data_hex(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let max_bytes = 8usize;
    let display_len = data.len().min(max_bytes);
    let hex_parts: Vec<String> = data[..display_len]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect();
    let mut result = hex_parts.join(" ");
    if data.len() > max_bytes {
        result.push_str(" ...");
    }
    result
}

pub fn build_detail_string(msg: &crate::types::DecodedMessage, max_width: u16) -> String {
    let mut parts = Vec::new();
    for output in &msg.outputs {
        match output {
            crate::types::DecodedField::Value {
                title, value, unit, ..
            } => {
                if let Numeric::Flag(flag_value) = &value {
                    let flag_str = match flag_value {
                        FlagValue::Off => "OFF",
                        FlagValue::On => "ON",
                        FlagValue::Error => "ERR",
                        FlagValue::Unavailable => "N/A",
                    };
                    parts.push(format!("{}={}", title, flag_str));
                } else {
                    let val_str = crate::formats::format_value(&value);
                    if let Some(u) = unit {
                        parts.push(format!("{}={} {}", title, val_str, u));
                    } else {
                        parts.push(format!("{}={}", title, val_str));
                    }
                }
            }
            crate::types::DecodedField::StringMessage { text, .. } => {
                parts.push(text.clone());
            }
        }
    }
    let detail = parts.join(", ");
    if detail.len() > max_width as usize {
        format!("{}...", &detail[..max_width as usize - 3])
    } else {
        detail
    }
}

pub fn build_detail_condensed_string(msg: &crate::types::DecodedMessage, max_width: u16) -> String {
    let mut parts = Vec::new();
    for output in &msg.outputs {
        match output {
            crate::types::DecodedField::Value { value, unit, .. } => {
                if let Numeric::Flag(flag_value) = &value {
                    let flag_str = match flag_value {
                        FlagValue::Off => "OFF",
                        FlagValue::On => "ON",
                        FlagValue::Error => "ERR",
                        FlagValue::Unavailable => "N/A",
                    };
                    parts.push(flag_str.to_string());
                } else {
                    let val_str = crate::formats::format_value(&value);
                    if let Some(u) = unit {
                        parts.push(format!("{} {}", val_str, u));
                    } else {
                        parts.push(val_str);
                    }
                }
            }
            crate::types::DecodedField::StringMessage { text, .. } => {
                parts.push(text.clone());
            }
        }
    }
    let detail = parts.join(", ");
    if detail.len() > max_width as usize {
        format!("{}...", &detail[..max_width as usize - 3])
    } else {
        detail
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FlagValue, Numeric};

    #[test]
    fn test_format_value_int() {
        assert_eq!(format_value(&Numeric::Int(42)), "42");
        assert_eq!(format_value(&Numeric::Int(-17)), "-17");
    }

    #[test]
    fn test_format_value_float() {
        assert_eq!(format_value(&Numeric::Float(3.14159)), "3.1416");
        assert_eq!(format_value(&Numeric::Float(0.25)), "0.2500");
    }

    #[test]
    fn test_format_value_hex() {
        assert_eq!(format_value(&Numeric::Hex(0xDEADBEEF)), "0xDEADBEEF");
        assert_eq!(format_value(&Numeric::Hex(0xFF)), "0xFF");
    }

    #[test]
    fn test_format_value_bool() {
        assert_eq!(format_value(&Numeric::Bool(true)), "true");
        assert_eq!(format_value(&Numeric::Bool(false)), "false");
    }

    #[test]
    fn test_format_value_flag() {
        assert_eq!(format_value(&Numeric::Flag(FlagValue::On)), "ON");
        assert_eq!(format_value(&Numeric::Flag(FlagValue::Off)), "OFF");
        assert_eq!(format_value(&Numeric::Flag(FlagValue::Error)), "ERR");
        assert_eq!(format_value(&Numeric::Flag(FlagValue::Unavailable)), "N/A");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "0.000000");
        assert_eq!(format_timestamp(1_000_000), "1.000000");
        assert_eq!(format_timestamp(1_500_000), "1.500000");
        assert_eq!(format_timestamp(1234567890_123456), "1234567890.123456");
    }

    #[test]
    fn test_format_elapsed_time() {
        let start = 1_000_000;
        assert_eq!(format_elapsed_time(1_000_000, Some(start)), "0.000");
        assert_eq!(format_elapsed_time(2_000_000, Some(start)), "1.000");
        assert_eq!(format_elapsed_time(1_500_000, Some(start)), "0.500");
        assert_eq!(format_elapsed_time(3_750_000, Some(start)), "2.750");
        assert_eq!(format_elapsed_time(500_000, Some(start)), "0.000");
        assert_eq!(format_elapsed_time(1_000_000, None), "0.000");
    }

    #[test]
    fn test_format_data_hex() {
        assert_eq!(format_data_hex(&[]), "");
        assert_eq!(format_data_hex(&[0xAB]), "AB");
        assert_eq!(format_data_hex(&[0x01, 0x23, 0x45]), "01 23 45");
        assert_eq!(format_data_hex(&[0xFF; 8]), "FF FF FF FF FF FF FF FF");
        assert_eq!(format_data_hex(&[0xFF; 9]), "FF FF FF FF FF FF FF FF ...");
        let mut repeated: Vec<u8> = Vec::new();
        for _ in 0..5 {
            repeated.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        }
        assert_eq!(format_data_hex(&repeated), "DE AD BE EF DE AD BE EF ...");
    }
}
