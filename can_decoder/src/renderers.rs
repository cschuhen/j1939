use j1939_async::Id;

use crate::types::{DecodedField, DecodedMessage};
use owo_colors::OwoColorize;
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
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            let mut lines = Vec::new();
            lines.push(format!(
                "MSG: --- {} ---",
                message.title.clone().bold().cyan()
            ));
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
                crate::types::Numeric::Hex(v) => {
                    format!(
                        "0x{}",
                        v.iter().map(|b| format!("{:02X}", b)).collect::<String>()
                    )
                }
                crate::types::Numeric::Bool(v) => format!("{}", v),
            };
            let title_part = format!("[{}]", title.bold().cyan());
            let unit_part = unit
                .as_ref()
                .map(|u| u.dimmed().to_string())
                .unwrap_or_else(|| "".to_string());
            format!(
                "{:<w1$} | {:<w2$} | {}",
                title_part,
                val_str,
                unit_part,
                w1 = w1,
                w2 = w2
            )
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
        DecodedField::Flag { title, value } => {
            let flag_text = match value {
                crate::types::FlagValue::Off => "OFF".red().to_string(),
                crate::types::FlagValue::On => "ON".green().to_string(),
                crate::types::FlagValue::Error => "ERR".red().bold().to_string(),
                crate::types::FlagValue::Unavailable => "N/A".dimmed().to_string(),
            };
            let title_part = format!("[{}]", title.bold().cyan());
            format!(
                "{:<w1$} | {:<w2$} | {}",
                title_part,
                "",
                flag_text,
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
    ) -> Pin<Box<dyn Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            let mut json_value = serde_json::to_value(message).map_err(|e| e.to_string())?;
            if let Some(assembled) = json_value.get_mut("assembled_message").and_then(|a| a.as_object_mut()) {
                assembled.insert("priority".to_string(), serde_json::json!(message.assembled_message.priority()));
                assembled.insert("source_address".to_string(), serde_json::json!(message.assembled_message.source()));
                assembled.insert("destination_address".to_string(), serde_json::json!(message.assembled_message.destination()));
            }
            Ok(serde_json::to_string_pretty(&json_value).map_err(|e| e.to_string())?)
        })
    }
}
