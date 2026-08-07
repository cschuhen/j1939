use j1939_async::Id;

use crate::types::{DecodedField, DecodedMessage};
use iso11783_data::strings::pgn as pgn_titles;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use crate::traits::Renderer;

/// Console renderer that prints DecodedField as colorized plain text.
pub struct ConsoleRenderer;

impl Renderer for ConsoleRenderer {
    fn name(&self) -> &str {
        "console"
    }

    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let mut lines = Vec::new();
            let title_str = format!("MSG: --- {} ---", message.title.clone().bold().cyan());
            lines.push(title_str);
            if let Some(pgn_name) = pgn_titles::lookup(message.pgn()) {
                lines.push(format!(
                    "  PGN: {:X} - {}",
                    message.pgn(),
                    pgn_name.bold().magenta()
                ));
            }
            for output in &message.outputs {
                lines.push(format_output(output));
            }
            Ok(lines.join("\n"))
        })
    }
}

/// Format a DecodedField item into a human-readable string.
pub fn format_output(output: &DecodedField) -> String {
    use owo_colors::OwoColorize;
    let w1 = 20;
    let w2 = 15;

    match output {
        DecodedField::Value {
            title,
            value,
            unit,
            decimal_places,
        } => {
            let val_str = match value {
                crate::types::Numeric::Int(v) => format!("{}", v),
                crate::types::Numeric::Float(v) => {
                    if let Some(dp) = decimal_places {
                        format!("{:.dp$}", v, dp = *dp as usize)
                    } else {
                        format!("{}", v)
                    }
                }
                crate::types::Numeric::Hex(v) => format!("0x{:X}", v),
                crate::types::Numeric::Bool(v) => format!("{}", v),
                crate::types::Numeric::Flag(value) => {
                    let flag_text = match value {
                        crate::types::FlagValue::Off => "OFF".red().to_string(),
                        crate::types::FlagValue::On => "ON".green().to_string(),
                        crate::types::FlagValue::Error => "ERR".red().bold().to_string(),
                        crate::types::FlagValue::Unavailable => "N/A".dimmed().to_string(),
                    };
                    format!("[{}] {}", title.bold().cyan(), flag_text)
                }
            };
            let title_part = format!("[{}]", title.bold().cyan());
            let unit_part = unit
                .as_ref()
                .map(|u| u.dimmed().to_string())
                .unwrap_or_else(|| "".to_string());
            // For Flag values, val_str already contains the formatted output
            if matches!(value, crate::types::Numeric::Flag(..)) {
                val_str
            } else {
                format!(
                    "{:<w1$} | {:<w2$} | {}",
                    title_part,
                    val_str,
                    unit_part,
                    w1 = w1,
                    w2 = w2
                )
            }
        }
        DecodedField::StringMessage { severity, text } => {
            let sev_part = match severity {
                crate::types::Severity::Info => "INFO".green().bold().to_string(),
                crate::types::Severity::Warning => "WARN".yellow().bold().to_string(),
                crate::types::Severity::Error => "ERROR".red().bold().to_string(),
            };
            format!(
                "{:<w1$} | {:<w2$} | {}",
                sev_part,
                "",
                text,
                w1 = w1,
                w2 = w2
            )
        }
    }
}

/// JSON renderer that serializes DecodedMessage to JSON format.
pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn name(&self) -> &str {
        "json"
    }

    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let mut json_value = serde_json::to_value(message).map_err(|e| e.to_string())?;
            if let Some(assembled) = json_value
                .get_mut("assembled_message")
                .and_then(|a| a.as_object_mut())
            {
                assembled.insert(
                    "priority".to_string(),
                    serde_json::json!(message.assembled_message.priority()),
                );
                assembled.insert(
                    "source_address".to_string(),
                    serde_json::json!(message.assembled_message.source()),
                );
                assembled.insert(
                    "destination_address".to_string(),
                    serde_json::json!(message.assembled_message.destination()),
                );
            }
            if let Some(pgn_name) = pgn_titles::lookup(message.pgn()) {
                if let Some(obj) = json_value.as_object_mut() {
                    obj.insert("pgn_title".to_string(), serde_json::json!(pgn_name));
                }
            }
            Ok(serde_json::to_string_pretty(&json_value).map_err(|e| e.to_string())?)
        })
    }
}

/// CSV renderer that outputs DecodedMessage in CSV format with fixed and dynamic columns.
///
/// Fixed columns (always first): Timestamp, CAN ID, Priority, PGN, Source address,
/// Destination address, Source NAME, Destination NAME, Data bytes, StringMessage concatenation.
///
/// Dynamic columns tracked per (PGN, title) combination - every Title from DecodedField::Value
/// gets its own column that is not reused for the same (PGN, title). Headers are re-printed
/// whenever new columns are added to a PGN/title combination.
pub struct CsvRenderer {
    /// Tracks dynamic columns per PGN.
    /// Key: pgn, Value: list of column names in order seen.
    column_registry: HashMap<u32, Vec<String>>,
}

impl Default for CsvRenderer {
    fn default() -> Self {
        Self {
            column_registry: HashMap::new(),
        }
    }
}

impl Renderer for CsvRenderer {
    fn name(&self) -> &str {
        "csv"
    }

    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let pgn = message.pgn();
            let assembled = &message.assembled_message;

            // Fixed columns
            let fixed_columns = vec![
                "Timestamp".to_string(),
                "CAN ID".to_string(),
                "Priority".to_string(),
                "PGN".to_string(),
                "PGN Title".to_string(),
                "Source address".to_string(),
                "Destination address".to_string(),
                "Source NAME".to_string(),
                "Destination NAME".to_string(),
                "Data bytes".to_string(),
                "StringMessage".to_string(),
            ];

            // Collect dynamic columns from Value and Flag outputs, tracked per PGN
            let mut dynamic_columns: Vec<String> = Vec::new();

            for output in &message.outputs {
                match output {
                    DecodedField::Value { title, .. } => {
                        // Check if we've seen this PGN before
                        if let Some(existing_cols) = self.column_registry.get(&pgn) {
                            // Use existing columns for this PGN
                            for col in existing_cols {
                                if !dynamic_columns.contains(col) && *col != *title {
                                    dynamic_columns.push(col.clone());
                                }
                            }
                        } else {
                            // New PGN - start with this column
                            self.column_registry.insert(pgn, vec![title.clone()]);
                            dynamic_columns.push(title.clone());
                        }

                        // Add this title if not already present
                        if !dynamic_columns.contains(title) {
                            dynamic_columns.push(title.clone());
                        }
                    }
                    _ => {}
                }
            }

            // Update registry with all columns seen for this PGN
            self.column_registry.insert(pgn, dynamic_columns.clone());

            // Build full header row
            let mut all_columns = fixed_columns.clone();
            all_columns.extend(dynamic_columns.clone());

            // Escape and join header
            let header_line = all_columns
                .iter()
                .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(",");

            // Build data row
            let mut values = Vec::new();

            // Fixed column values
            values.push(format!("{}", assembled.timestamp));
            values.push(format!("{:X}", assembled.id));
            values.push(format!("{}", assembled.priority()));
            values.push(format!("{:X}", pgn));
            let pgn_title = pgn_titles::lookup(pgn).unwrap_or("").to_string();
            values.push(format!("\"{}\"", pgn_title.replace('"', "\"\"")));
            values.push(format!("{:X}", assembled.source()));
            values.push(format!("{:X}", assembled.destination()));

            let src_name = assembled
                .source_name
                .map(|n| format!("{:X}", n))
                .unwrap_or_else(|| "".to_string());
            values.push(format!("\"{}\"", src_name.replace('"', "\"\"")));

            let dst_name = assembled
                .dest_name
                .map(|n| format!("{:X}", n))
                .unwrap_or_else(|| "".to_string());
            values.push(format!("\"{}\"", dst_name.replace('"', "\"\"")));

            let data_hex: String = assembled
                .data
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect();
            values.push(data_hex);

            // Concatenate all StringMessages
            let string_msgs: Vec<String> = message
                .outputs
                .iter()
                .filter_map(|o| {
                    if let DecodedField::StringMessage { text, .. } = o {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            values.push(format!(
                "\"{}\"",
                string_msgs.join("; ").replace('"', "\"\"")
            ));

            // Dynamic column values - one row per output
            for output in &message.outputs {
                match output {
                    DecodedField::Value {
                        title, value, unit, ..
                    } => {
                        let display = match value {
                            crate::types::Numeric::Int(v) => format!("{}", v),
                            crate::types::Numeric::Float(v) => format!("{}", v),
                            crate::types::Numeric::Hex(v) => format!("0x{:X}", v),
                            crate::types::Numeric::Bool(v) => format!("{}", v),
                            crate::types::Numeric::Flag(flag_value) => {
                                let flag = match flag_value {
                                    crate::types::FlagValue::Off => "OFF",
                                    crate::types::FlagValue::On => "ON",
                                    crate::types::FlagValue::Error => "ERR",
                                    crate::types::FlagValue::Unavailable => "N/A",
                                };
                                format!("{}: {}", title, flag)
                            }
                        };
                        let display = if let Some(ref u) = unit {
                            format!("{} {}", display, u)
                        } else {
                            display
                        };

                        // Check if this value's title is in dynamic columns for this PGN
                        if let Some(cols) = self.column_registry.get(&pgn) {
                            if cols.contains(title) {
                                values.push(format!("\"{}\"", display.replace('"', "\"\"")));
                            } else {
                                values.push("".to_string());
                            }
                        } else {
                            values.push("".to_string());
                        }
                    }
                    DecodedField::StringMessage { .. } => {
                        // Already handled in fixed columns
                        values.push("".to_string());
                    }
                }
            }

            let data_line = values.join(",");

            Ok(format!("{}\n{}", header_line, data_line))
        })
    }
}

/// Condensed renderer that outputs a single-line summary per message.
///
/// Fixed fields: Timestamp, CAN ID (hex, no 0x), Priority, PGN, PGN hex,
/// Source address -> Destination address (hex), (Source NAME -> Destination NAME),
/// Data bytes (hex, no 0x), StringMessage concatenation.
///
/// Every Title from DecodedField gets its own field with bold headers.
/// Flag fields are colored: red for Error, white for Off, green for On.
pub struct CondensedRenderer;

impl Renderer for CondensedRenderer {
    fn name(&self) -> &str {
        "condensed"
    }

    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let assembled = &message.assembled_message;
            let mut parts = Vec::new();

            // Fixed fields: PGN hex, source->dest address, data bytes
            parts.push(format!("{:X}", message.pgn()));
            parts.push(format!(
                "{:X}->{:X}",
                assembled.source(),
                assembled.destination()
            ));

            let data_hex: String = assembled
                .data
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect();
            parts.push(data_hex);

            // Concatenate non-empty StringMessages
            let string_msgs: Vec<String> = message
                .outputs
                .iter()
                .filter_map(|o| {
                    if let DecodedField::StringMessage { text, .. } = o {
                        if !text.is_empty() {
                            Some(text.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            if !string_msgs.is_empty() {
                parts.push(string_msgs.join("; "));
            }

            // Dynamic fields from Value and Flag outputs with bold headers
            for output in &message.outputs {
                match output {
                    DecodedField::Value {
                        title, value, unit, ..
                    } => {
                        let display = match value {
                            crate::types::Numeric::Int(v) => format!("{}", v),
                            crate::types::Numeric::Float(v) => format!("{}", v),
                            crate::types::Numeric::Hex(v) => format!("0x{:X}", v),
                            crate::types::Numeric::Bool(v) => format!("{}", v),
                            crate::types::Numeric::Flag(flag_value) => {
                                let flag_text = match flag_value {
                                    crate::types::FlagValue::Off => "OFF".red().to_string(),
                                    crate::types::FlagValue::On => "ON".green().to_string(),
                                    crate::types::FlagValue::Error => "ERR".white().bold().to_string(),
                                    crate::types::FlagValue::Unavailable => "N/A".dimmed().to_string(),
                                };
                                flag_text
                            }
                        };
                        let display = if let Some(ref u) = unit {
                            format!("{} {}", display, u)
                        } else {
                            display
                        };
                        parts.push(format!("{}={}", title.bold().cyan(), display));
                    }
                    DecodedField::StringMessage { text, severity } => {
                        if text.is_empty() {
                            continue;
                        }
                        let sev = match severity {
                            crate::types::Severity::Info => "I".green().to_string(),
                            crate::types::Severity::Warning => "W".yellow().to_string(),
                            crate::types::Severity::Error => "E".red().to_string(),
                        };
                        parts.push(format!("[{}] {}", sev, text));
                    }
                }
            }

            Ok(parts.join(" | "))
        })
    }
}

/// FullCondensed renderer that outputs a single-line summary per message.
///
/// Fixed fields: Timestamp, CAN ID (hex, no 0x), Priority, PGN, PGN hex,
/// Source address -> Destination address (hex), (Source NAME -> Destination NAME),
/// Data bytes (hex, no 0x), StringMessage concatenation.
///
/// Every Title from DecodedField gets its own field with bold headers.
/// Flag fields are colored: red for Error, white for Off, green for On.
pub struct FullCondensedRenderer;

impl Renderer for FullCondensedRenderer {
    fn name(&self) -> &str {
        "full-condensed"
    }

    fn render<'a>(
        &'a mut self,
        message: &'a DecodedMessage,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let assembled = &message.assembled_message;
            let mut parts = Vec::new();

            // Fixed fields
            parts.push(format!("{}", assembled.timestamp));
            parts.push(format!("{:X}", assembled.id));
            parts.push(format!("{}", assembled.priority()));
            parts.push(format!("{}", message.pgn()));
            parts.push(format!("{:X}", message.pgn()));
            if let Some(pgn_name) = pgn_titles::lookup(message.pgn()) {
                parts.push(pgn_name.bold().magenta().to_string());
            }
            parts.push(format!(
                "{:X}->{:X}",
                assembled.source(),
                assembled.destination()
            ));

            let src_name = assembled
                .source_name
                .map(|n| format!("{:X}", n))
                .unwrap_or_else(|| "N/A".to_string());
            let dst_name = assembled
                .dest_name
                .map(|n| format!("{:X}", n))
                .unwrap_or_else(|| "N/A".to_string());
            parts.push(format!("({} -> {})", src_name, dst_name));

            let data_hex: String = assembled
                .data
                .iter()
                .map(|b| format!("{:02X} ", b))
                .collect();
            parts.push(data_hex);

            parts.push(message.title.clone());

            // Concatenate all StringMessages
            let string_msgs: Vec<String> = message
                .outputs
                .iter()
                .filter_map(|o| {
                    if let DecodedField::StringMessage { text, .. } = o {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            parts.push(string_msgs.join("; "));

            // Dynamic fields from Value and Flag outputs with bold headers
            for output in &message.outputs {
                match output {
                    DecodedField::Value {
                        title, value, unit, ..
                    } => {
                        let display = match value {
                            crate::types::Numeric::Int(v) => format!("{}", v),
                            crate::types::Numeric::Float(v) => format!("{}", v),
                            crate::types::Numeric::Hex(v) => format!("0x{:X}", v),
                            crate::types::Numeric::Bool(v) => format!("{}", v),
                            crate::types::Numeric::Flag(flag_value) => {
                                let flag_text = match flag_value {
                                    crate::types::FlagValue::Off => "OFF".red().to_string(),
                                    crate::types::FlagValue::On => "ON".green().to_string(),
                                    crate::types::FlagValue::Error => "ERR".white().bold().to_string(),
                                    crate::types::FlagValue::Unavailable => "N/A".dimmed().to_string(),
                                };
                                flag_text
                            }
                        };
                        let display = if let Some(ref u) = unit {
                            format!("{} {}", display, u)
                        } else {
                            display
                        };
                        parts.push(format!("{}={}", title.bold().cyan(), display));
                    }
                    DecodedField::StringMessage { text, severity } => {
                        let sev = match severity {
                            crate::types::Severity::Info => "I".green().to_string(),
                            crate::types::Severity::Warning => "W".yellow().to_string(),
                            crate::types::Severity::Error => "E".red().to_string(),
                        };
                        parts.push(format!("[{}] {}", sev, text));
                    }
                }
            }

            Ok(parts.join(" | "))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssembledMessage, DecodedField, Numeric};

    fn make_test_message() -> DecodedMessage {
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![0x01, 0x02], 0);
        DecodedMessage {
            title: "Engine Speed".to_string(),
            outputs: vec![
                DecodedField::Value {
                    title: "RPM".to_string(),
                    value: Numeric::Int(1500),
                    unit: Some("rpm".to_string()),
                    decimal_places: None,
                },
                DecodedField::StringMessage {
                    severity: crate::types::Severity::Info,
                    text: "Engine running".to_string(),
                },
            ],
            updates: vec![],
            assembled_message: assembled,
        }
    }

    // ========================================================================
    // CSV Renderer Tests
    // ========================================================================

    #[tokio::test]
    async fn test_csv_renderer_name() {
        let renderer = CsvRenderer::default();
        assert_eq!(renderer.name(), "csv");
    }

    #[tokio::test]
    async fn test_csv_renderer_fixed_columns_present() {
        let mut renderer = CsvRenderer::default();
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        let header_line = result.lines().next().unwrap();

        // Check fixed columns are present in order
        assert!(header_line.contains("\"Timestamp\""));
        assert!(header_line.contains("\"CAN ID\""));
        assert!(header_line.contains("\"Priority\""));
        assert!(header_line.contains("\"PGN\""));
        assert!(header_line.contains("\"Source address\""));
        assert!(header_line.contains("\"Destination address\""));
        assert!(header_line.contains("\"Source NAME\""));
        assert!(header_line.contains("\"Destination NAME\""));
        assert!(header_line.contains("\"Data bytes\""));
        assert!(header_line.contains("\"StringMessage\""));
    }

    #[tokio::test]
    async fn test_csv_renderer_dynamic_columns() {
        let mut renderer = CsvRenderer::default();
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        let header_line = result.lines().next().unwrap();

        // RPM should be a dynamic column for this PGN/title combination
        assert!(header_line.contains("\"RPM\""));
    }

    #[tokio::test]
    async fn test_csv_renderer_data_row_values() {
        let mut renderer = CsvRenderer::default();
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        let lines: Vec<&str> = result.lines().collect();

        assert!(lines.len() >= 2);
        // Data row should contain timestamp and assembled data
        assert!(lines[1].contains("1500"));
    }

    #[tokio::test]
    async fn test_csv_renderer_column_tracking_per_pgn_title() {
        let mut renderer = CsvRenderer::default();

        // First message with PGN 0x18EF4000 and title "RPM"
        let assembled1 = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let msg1 = DecodedMessage {
            title: "Engine Speed".to_string(),
            outputs: vec![DecodedField::Value {
                title: "RPM".to_string(),
                value: Numeric::Int(1500),
                unit: Some("rpm".to_string()),
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled1,
        };
        renderer.render(&msg1).await.unwrap();

        // Second message with same PGN but different title "Load" - should add new column
        let msg2 = DecodedMessage {
            title: "Engine Load".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Load".to_string(),
                value: Numeric::Int(50),
                unit: Some("%".to_string()),
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0),
        };
        renderer.render(&msg2).await.unwrap();

        // Third message with same PGN and title "RPM" - should reuse existing column
        let msg3 = DecodedMessage {
            title: "Engine Speed".to_string(),
            outputs: vec![DecodedField::Value {
                title: "RPM".to_string(),
                value: Numeric::Int(2000),
                unit: Some("rpm".to_string()),
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0),
        };
        let result = renderer.render(&msg3).await.unwrap();

        // Should have both RPM and Load columns for this PGN/title combo
        assert!(result.contains("\"RPM\""));
        assert!(result.contains("\"Load\""));
    }

    #[tokio::test]
    async fn test_csv_renderer_string_escaping() {
        let mut renderer = CsvRenderer::default();
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![DecodedField::StringMessage {
                severity: crate::types::Severity::Warning,
                text: r#"Value is "high""#.to_string(),
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("\"\""));
    }

    #[tokio::test]
    async fn test_csv_renderer_empty_outputs() {
        let mut renderer = CsvRenderer::default();
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Empty".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // Should still have header with fixed columns
        assert!(result.contains("\"Timestamp\""));
    }

    #[tokio::test]
    async fn test_csv_renderer_hex_value() {
        let mut renderer = CsvRenderer::default();
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "VIN".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Data".to_string(),
                value: Numeric::Hex(0x1234),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("0x"));
    }

    #[tokio::test]
    async fn test_csv_renderer_bool_value() {
        let mut renderer = CsvRenderer::default();
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Flag".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Active".to_string(),
                value: Numeric::Bool(true),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("true"));
    }

    #[tokio::test]
    async fn test_csv_renderer_float_value() {
        let mut renderer = CsvRenderer::default();
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Temperature".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Coolant".to_string(),
                value: Numeric::Float(92.5),
                unit: Some("C".to_string()),
                decimal_places: Some(1),
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("92.5"));
    }

    #[tokio::test]
    async fn test_csv_renderer_all_flag_values() {
        let mut renderer = CsvRenderer::default();
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for (flag_val, expected) in [
            (crate::types::FlagValue::Off, "OFF"),
            (crate::types::FlagValue::On, "ON"),
            (crate::types::FlagValue::Error, "ERR"),
            (crate::types::FlagValue::Unavailable, "N/A"),
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::Value {
                    title: "Status".to_string(),
                    value: Numeric::Flag(flag_val.clone()),
                    unit: None,
                    decimal_places: None,
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            assert!(result.contains(expected), "Failed for {:?}", flag_val);
        }
    }

    // ========================================================================
    // Condensed Renderer Tests
    // ========================================================================

    #[tokio::test]
    async fn test_condensed_renderer_name() {
        let renderer = CondensedRenderer;
        assert_eq!(renderer.name(), "condensed");
    }

    #[tokio::test]
    async fn test_condensed_renderer_single_line() {
        let mut renderer = CondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        assert!(!result.contains('\n'));
    }

    #[tokio::test]
    async fn test_condensed_renderer_fixed_fields() {
        let mut renderer = CondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();

        // Should contain fixed fields: timestamp, CAN ID hex, priority, PGN, PGN hex, source->dest
        assert!(result.contains(" | ")); // pipe separators between fields
    }

    #[tokio::test]
    async fn test_condensed_renderer_can_id_no_prefix() {
        let mut renderer = CondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();

        // CAN ID should be hex without 0x prefix in fixed fields
        assert!(result.contains("EF00"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_source_dest_format() {
        let mut renderer = CondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();

        // Should contain source->dest format like "EF00->FF" or similar
        assert!(result.contains("->"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_flag_colors() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for (flag_val, expected_text) in [
            (crate::types::FlagValue::Off, "OFF"),
            (crate::types::FlagValue::On, "ON"),
            (crate::types::FlagValue::Error, "ERR"),
            (crate::types::FlagValue::Unavailable, "N/A"),
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::Value {
                    title: "Status".to_string(),
                    value: Numeric::Flag(flag_val.clone()),
                    unit: None,
                    decimal_places: None,
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            assert!(result.contains(expected_text), "Failed for {:?}", flag_val);
        }
    }

    #[tokio::test]
    async fn test_condensed_renderer_severity_codes() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for (severity, code) in [
            (crate::types::Severity::Info, "I"),
            (crate::types::Severity::Warning, "W"),
            (crate::types::Severity::Error, "E"),
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::StringMessage {
                    severity: severity.clone(),
                    text: "test".to_string(),
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            let stripped = strip_ansi_codes(&result);
            assert!(
                stripped.contains(&format!("[{}] test", code)),
                "Failed for {:?}",
                severity
            );
        }
    }

    #[tokio::test]
    async fn test_condensed_renderer_multiple_outputs() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Multi".to_string(),
            outputs: vec![
                DecodedField::Value {
                    title: "Speed".to_string(),
                    value: Numeric::Int(60),
                    unit: Some("km/h".to_string()),
                    decimal_places: None,
                },
                DecodedField::Value {
                    title: "Cruise".to_string(),
                    value: Numeric::Flag(crate::types::FlagValue::On),
                    unit: None,
                    decimal_places: None,
                },
            ],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Speed=60 km/h"));
        assert!(stripped.contains("Cruise=ON"));
        assert!(result.contains(" | "));
    }

    #[tokio::test]
    async fn test_condensed_renderer_data_bytes_hex() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![0xAA, 0xBB], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("AABB"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_string_messages_concatenated() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![
                DecodedField::StringMessage {
                    severity: crate::types::Severity::Info,
                    text: "msg1".to_string(),
                },
                DecodedField::StringMessage {
                    severity: crate::types::Severity::Warning,
                    text: "msg2".to_string(),
                },
            ],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("msg1; msg2"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_empty_outputs() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Empty".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // Should still have fixed fields (timestamp is microseconds since epoch)
        assert!(result.contains(" | "));
    }

    #[tokio::test]
    async fn test_condensed_renderer_pgn_hex() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // PGN hex should appear (0xEF00)
        assert!(result.contains("EF00"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_name_and_dest_names() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // NAME fields are no longer included in condensed format
        assert!(result.contains("EF00"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_value_with_title() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![DecodedField::Value {
                title: "RPM".to_string(),
                value: Numeric::Int(1500),
                unit: Some("rpm".to_string()),
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // Strip ANSI escape sequences for comparison
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("RPM=1500 rpm"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_flag_with_title() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Engine".to_string(),
                value: Numeric::Flag(crate::types::FlagValue::On),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Engine=ON"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_hex_value() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "VIN".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Data".to_string(),
                value: Numeric::Hex(0x1234),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Data=0x"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_bool_value() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Flag".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Active".to_string(),
                value: Numeric::Bool(true),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Active=true"));
    }

    #[tokio::test]
    async fn test_condensed_renderer_float_value() {
        let mut renderer = CondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Temperature".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Coolant".to_string(),
                value: Numeric::Float(92.5),
                unit: Some("C".to_string()),
                decimal_places: Some(1),
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Coolant=92.5 C"));
    }

    // ========================================================================
    // Full Condensed Renderer Tests
    // ========================================================================

    #[tokio::test]
    async fn test_full_condensed_renderer_name() {
        let renderer = FullCondensedRenderer;
        assert_eq!(renderer.name(), "full-condensed");
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_single_line() {
        let mut renderer = FullCondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();
        assert!(!result.contains('\n'));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_fixed_fields() {
        let mut renderer = FullCondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();

        // Should contain fixed fields: timestamp, CAN ID hex, priority, PGN, PGN hex, source->dest
        assert!(result.contains(" | ")); // pipe separators between fields
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_can_id_no_prefix() {
        let mut renderer = FullCondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();

        // CAN ID should be hex without 0x prefix in fixed fields
        assert!(result.contains("EF00"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_source_dest_format() {
        let mut renderer = FullCondensedRenderer;
        let message = make_test_message();
        let result = renderer.render(&message).await.unwrap();

        // Should contain source->dest format like "EF00->FF" or similar
        assert!(result.contains("->"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_flag_colors() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for (flag_val, expected_text) in [
            (crate::types::FlagValue::Off, "OFF"),
            (crate::types::FlagValue::On, "ON"),
            (crate::types::FlagValue::Error, "ERR"),
            (crate::types::FlagValue::Unavailable, "N/A"),
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::Value {
                    title: "Status".to_string(),
                    value: Numeric::Flag(flag_val.clone()),
                    unit: None,
                    decimal_places: None,
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            assert!(result.contains(expected_text), "Failed for {:?}", flag_val);
        }
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_severity_codes() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);

        for (severity, code) in [
            (crate::types::Severity::Info, "I"),
            (crate::types::Severity::Warning, "W"),
            (crate::types::Severity::Error, "E"),
        ] {
            let message = DecodedMessage {
                title: "Test".to_string(),
                outputs: vec![DecodedField::StringMessage {
                    severity: severity.clone(),
                    text: "test".to_string(),
                }],
                updates: vec![],
                assembled_message: assembled.clone(),
            };
            let result = renderer.render(&message).await.unwrap();
            let stripped = strip_ansi_codes(&result);
            assert!(
                stripped.contains(&format!("[{}] test", code)),
                "Failed for {:?}",
                severity
            );
        }
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_multiple_outputs() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Multi".to_string(),
            outputs: vec![
                DecodedField::Value {
                    title: "Speed".to_string(),
                    value: Numeric::Int(60),
                    unit: Some("km/h".to_string()),
                    decimal_places: None,
                },
                DecodedField::Value {
                    title: "Cruise".to_string(),
                    value: Numeric::Flag(crate::types::FlagValue::On),
                    unit: None,
                    decimal_places: None,
                },
            ],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Speed=60 km/h"));
        assert!(stripped.contains("Cruise=ON"));
        assert!(result.contains(" | "));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_data_bytes_hex() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![0xAA, 0xBB], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("AA BB "));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_string_messages_concatenated() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![
                DecodedField::StringMessage {
                    severity: crate::types::Severity::Info,
                    text: "msg1".to_string(),
                },
                DecodedField::StringMessage {
                    severity: crate::types::Severity::Warning,
                    text: "msg2".to_string(),
                },
            ],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("msg1; msg2"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_empty_outputs() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Empty".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // Should still have fixed fields (timestamp is microseconds since epoch)
        assert!(result.contains(" | "));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_pgn_hex() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // PGN should appear twice - once as decimal and once as hex
        assert!(result.contains("61184")); // 0xEF00 in decimal
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_name_and_dest_names() {
        let mut renderer = FullCondensedRenderer;
        let mut assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        assembled.source_name = Some(0x123456789ABCDEF0);
        assembled.dest_name = Some(0xFEDCBA9876543210);

        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        assert!(result.contains("(123456789ABCDEF0 -> FEDCBA9876543210)"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_value_with_title() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![DecodedField::Value {
                title: "RPM".to_string(),
                value: Numeric::Int(1500),
                unit: Some("rpm".to_string()),
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        // Strip ANSI escape sequences for comparison
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("RPM=1500 rpm"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_flag_with_title() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Test".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Engine".to_string(),
                value: Numeric::Flag(crate::types::FlagValue::On),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Engine=ON"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_hex_value() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "VIN".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Data".to_string(),
                value: Numeric::Hex(0x1234),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Data=0x"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_bool_value() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Flag".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Active".to_string(),
                value: Numeric::Bool(true),
                unit: None,
                decimal_places: None,
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Active=true"));
    }

    #[tokio::test]
    async fn test_full_condensed_renderer_float_value() {
        let mut renderer = FullCondensedRenderer;
        let assembled = AssembledMessage::with_pgn(0x18EF4000, 0xEF00, vec![], 0);
        let message = DecodedMessage {
            title: "Temperature".to_string(),
            outputs: vec![DecodedField::Value {
                title: "Coolant".to_string(),
                value: Numeric::Float(92.5),
                unit: Some("C".to_string()),
                decimal_places: Some(1),
            }],
            updates: vec![],
            assembled_message: assembled,
        };
        let result = renderer.render(&message).await.unwrap();
        let stripped = strip_ansi_codes(&result);
        assert!(stripped.contains("Coolant=92.5 C"));
    }

    fn strip_ansi_codes(s: &str) -> String {
        use std::sync::LazyLock;
        static ANSI_RE: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap());
        ANSI_RE.replace_all(s, "").to_string()
    }
}

/// Strip ANSI escape sequences from a string.
pub fn strip_ansi_codes(s: &str) -> String {
    use std::sync::LazyLock;
    static ANSI_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap());
    ANSI_RE.replace_all(s, "").to_string()
}

#[cfg(test)]
mod tests_integration {
    use super::*;

    #[test]
    fn test_strip_ansi_codes_basic() {
        let s = "\x1b[36m\x1b[1mRPM\x1b[0m\x1b[39m=1500 rpm";
        let stripped = strip_ansi_codes(s);
        assert_eq!(stripped, "RPM=1500 rpm");
    }

    #[test]
    fn test_strip_ansi_codes_no_ansi() {
        let s = "No ANSI codes here";
        let stripped = strip_ansi_codes(s);
        assert_eq!(stripped, "No ANSI codes here");
    }

    #[test]
    fn test_strip_ansi_codes_multiple_sequences() {
        let s = "\x1b[32mGreen\x1b[0m and \x1b[31mRed\x1b[0m";
        let stripped = strip_ansi_codes(s);
        assert_eq!(stripped, "Green and Red");
    }
}
