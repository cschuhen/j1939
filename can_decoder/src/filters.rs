use crate::traits::Filter;
use crate::types::{DecodedField, DecodedMessage, FlagValue, Numeric, Severity};
use j1939_async::Id;
use regex::Regex;
use std::future::Future;
use std::pin::Pin;

/// A filter that matches string messages using a regular expression.
pub struct RegexFilter {
    pub regex: Regex,
}

impl RegexFilter {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(pattern)?,
        })
    }
}

impl Filter for RegexFilter {
    fn name(&self) -> &str {
        "regex_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = if let Some(DecodedField::StringMessage { text, .. }) = message
            .outputs
            .iter()
            .find(|o| matches!(o, DecodedField::StringMessage { .. }))
        {
            self.regex.is_match(text)
        } else {
            false
        };
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        if let Some(DecodedField::StringMessage { text, .. }) = message
            .outputs
            .iter()
            .find(|o| matches!(o, DecodedField::StringMessage { .. }))
        {
            self.regex.is_match(text)
        } else {
            false
        }
    }
}

/// A filter that matches numeric values by title and range or exact values.
/// Supports decimal (144), hex (0x90, 0xfe), ranges (>=144, 0x90-0xFE),
/// and comma-separated exact values (144,254 or 0x90,0xfe).
pub struct NumericFilter {
    pub title: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub exact_values: Vec<f64>,
}

impl Filter for NumericFilter {
    fn name(&self) -> &str {
        "numeric_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        message.outputs.iter().any(|output| {
            if let DecodedField::Value { title, value, .. } = output {
                if title == &self.title {
                    match value {
                        Numeric::Int(i) => {
                            let val: f64 = *i as f64;
                            if !self.exact_values.is_empty() {
                                self.exact_values.contains(&val)
                            } else {
                                (self.min.is_none_or(|m| val >= m))
                                    && (self.max.is_none_or(|m| val <= m))
                            }
                        }
                        Numeric::Float(f) => {
                            if !self.exact_values.is_empty() {
                                self.exact_values.contains(f)
                            } else {
                                (self.min.is_none_or(|m| *f >= m))
                                    && (self.max.is_none_or(|m| *f <= m))
                            }
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        })
    }
}

/// A filter that matches flag values by title.
pub struct FlagFilter {
    pub title: String,
    pub value: FlagValue,
}

impl Filter for FlagFilter {
    fn name(&self) -> &str {
        "flag_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        message.outputs.iter().any(|output| {
            if let DecodedField::Value {
                title,
                value: Numeric::Flag(value),
                ..
            } = output
            {
                title == &self.title && *value == self.value
            } else {
                false
            }
        })
    }
}

/// A filter that matches severity levels.
pub struct SeverityFilter {
    pub severity: Severity,
}

impl Filter for SeverityFilter {
    fn name(&self) -> &str {
        "severity_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        message.outputs.iter().any(|output| {
            if let DecodedField::StringMessage { severity, .. } = output {
                *severity == self.severity
            } else {
                false
            }
        })
    }
}

/// A filter that matches PGN values by title and specific PGN.
pub struct PgnFilter {
    pub pgn: u32,
}

impl PgnFilter {
    pub fn new(pgn: u32) -> Self {
        Self { pgn }
    }
}

impl Filter for PgnFilter {
    fn name(&self) -> &str {
        "pgn_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        self.pgn == message.assembled_message.pgn()
    }
}

/// A filter that matches by output title (substring match).
pub struct TitleFilter {
    pub title_contains: String,
}

impl TitleFilter {
    pub fn new(title: &str) -> Self {
        Self {
            title_contains: title.to_lowercase(),
        }
    }
}

impl Filter for TitleFilter {
    fn name(&self) -> &str {
        "title_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        let msg_match = message.title.to_lowercase().contains(&self.title_contains);
        let output_match = message.outputs.iter().any(|output| match output {
            DecodedField::Value { title, .. } => {
                title.to_lowercase().contains(&self.title_contains)
            }
            DecodedField::StringMessage { text, .. } => {
                text.to_lowercase().contains(&self.title_contains)
            }
        });
        msg_match || output_match
    }
}

/// A filter that matches by source address (supports comma-separated list).
pub struct SourceFilter {
    pub sources: Vec<u8>,
}

impl SourceFilter {
    pub fn new(source: u8) -> Self {
        Self {
            sources: vec![source],
        }
    }

    pub fn from_list(sources: Vec<u8>) -> Self {
        Self { sources }
    }
}

impl Filter for SourceFilter {
    fn name(&self) -> &str {
        "source_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        self.sources.contains(&message.assembled_message.source())
    }
}

/// A filter that matches by destination address (supports comma-separated list).
pub struct DestFilter {
    pub dests: Vec<u8>,
}

impl DestFilter {
    pub fn new(dest: u8) -> Self {
        Self { dests: vec![dest] }
    }

    pub fn from_list(dests: Vec<u8>) -> Self {
        Self { dests }
    }
}

impl Filter for DestFilter {
    fn name(&self) -> &str {
        "dest_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        self.dests
            .contains(&message.assembled_message.destination())
    }
}

/// A filter that matches by source NAME (64-bit J1939 NAME).
pub struct SourceNameFilter {
    pub names: Vec<u64>,
}

impl SourceNameFilter {
    pub fn new(source_name: u64) -> Self {
        Self {
            names: vec![source_name],
        }
    }

    pub fn from_list(names: Vec<u64>) -> Self {
        Self { names }
    }
}

impl Filter for SourceNameFilter {
    fn name(&self) -> &str {
        "source_name_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        self.names
            .iter()
            .any(|name| message.assembled_message.source_name == Some(*name))
    }
}

/// A filter that matches by destination NAME (64-bit J1939 NAME).
pub struct DestNameFilter {
    pub names: Vec<u64>,
}

impl DestNameFilter {
    pub fn new(dest_name: u64) -> Self {
        Self {
            names: vec![dest_name],
        }
    }

    pub fn from_list(names: Vec<u64>) -> Self {
        Self { names }
    }
}

impl Filter for DestNameFilter {
    fn name(&self) -> &str {
        "dest_name_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.matches_sync(message);
        Box::pin(async move { result })
    }

    fn matches_sync(&self, message: &DecodedMessage) -> bool {
        self.names
            .iter()
            .any(|name| message.assembled_message.dest_name == Some(*name))
    }
}

/// A filter that matches all sub-filters using AND logic.
pub struct CompositeFilter {
    pub filters: Vec<Box<dyn Filter>>,
}

impl CompositeFilter {
    pub fn new(filters: Vec<Box<dyn Filter>>) -> Self {
        Self { filters }
    }
}

impl Filter for CompositeFilter {
    fn name(&self) -> &str {
        "composite"
    }

    fn matches<'a>(
        &'a self,
        message: &'a DecodedMessage,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let mut result = true;
            for i in 0..self.filters.len() {
                if !self.filters[i].matches(message).await {
                    result = false;
                    break;
                }
            }
            result
        })
    }
}

fn parse_hex_or_dec_u32(s: &str) -> Result<u32, anyhow::Error> {
    s.parse().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        u32::from_str_radix(stripped, 16).map_err(anyhow::Error::from)
    })
}

fn parse_hex_or_dec_u64(s: &str) -> Result<u64, anyhow::Error> {
    s.parse().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        u64::from_str_radix(stripped, 16).map_err(anyhow::Error::from)
    })
}

/// Parses a filter expression string into a Box<dyn Filter>.
/// Supported syntax:
///   - "title:<substring>" — TitleFilter
///   - "pgn:<hex_or_dec>"  — PgnFilter (e.g. pgn:0xEA00 or pgn:59904)
///   - "severity:<level>"   — SeverityFilter (info, warning, error)
///   - "flag:<title>=<value>" — FlagFilter (off, on, error, unavailable)
///   - "numeric:<title>:<range>"    — NumericFilter (exact N, range N-M, >=N, <=N)
///   - "regex:<pattern>"    — RegexFilter
///   - "source:<addr>"      — SourceFilter (0-255, e.g. source:144)
///   - "dest:<addr>"        — DestFilter (0-255, e.g. dest:255)
///   - "src-name:<hex>"     — SourceNameFilter (64-bit NAME in hex, e.g. src-name:0x80000000000F2EEC)
///   - "dest-name:<hex>"    — DestNameFilter (64-bit NAME in hex, e.g. dest-name:0x80000000000A1EEC)
pub struct FilterParser;

impl FilterParser {
    pub fn print_help() {
        use owo_colors::OwoColorize;
        eprintln!("{}", "Available filter types:".bold().cyan());
        eprintln!(
            "  {}.{}              Match by output title (case-insensitive substring)",
            "title".green().bold(),
            "<substring>".dimmed()
        );
        eprintln!(
            "  {}.{}                   Match by PGN number (decimal or hex, e.g. {} or {})",
            "pgn".green().bold(),
            "<number>".dimmed(),
            "pgn:51968".yellow(),
            "pgn:0xCB00".yellow()
        );
        eprintln!(
            "  {}.{}               Match by severity (info, warning, error)",
            "severity".green().bold(),
            "<level>".dimmed()
        );
        eprintln!(
            "  {}.{}={}           Match flag value (off, on, error, unavailable)",
            "flag".green().bold(),
            "<title>".dimmed(),
            "<value>".dimmed()
        );
        eprintln!(
            "  {}.{}:{}         Match numeric (exact N, range N-M, >=N, <=N; hex supported; e.g. {} or {})",
            "numeric".green().bold(),
            "<title>".dimmed(),
            "<range>".dimmed(),
            "numeric:RPM:0-5000".yellow(),
            "numeric:Element:144,0xfe".yellow()
        );
        eprintln!(
            "  {}.{}                Match string message text against regex pattern",
            "regex".green().bold(),
            "<pattern>".dimmed()
        );
        eprintln!(
            "  {}.{}              Match by source address (0-255, hex supported; e.g. {})",
            "source".green().bold(),
            "<addr>".dimmed(),
            "source:144,0xfe".yellow()
        );
        eprintln!(
            "  {}.{}              Match by destination address (0-255, hex supported; e.g. {})",
            "dest".green().bold(),
            "<addr>".dimmed(),
            "dest:0x90,254".yellow()
        );
        eprintln!(
            "  {}.{}              Match by source NAME in hex (e.g. {})",
            "src-name".green().bold(),
            "<hex>".dimmed(),
            "src-name:0x80000000000F2EEC".yellow()
        );
        eprintln!(
            "  {}.{}              Match by destination NAME in hex (e.g. {})",
            "dest-name".green().bold(),
            "<hex>".dimmed(),
            "dest-name:0x80000000000A1EEC".yellow()
        );
        eprintln!();
        eprintln!("{}", "Examples:".bold().cyan());
        eprintln!("  --filter {}", "pgn:51968".yellow());
        eprintln!("  --filter {}", "source:144,0xfe".yellow());
        eprintln!("  --filter {}", "dest:0x90,254".yellow());
        eprintln!("  --filter {}", "src-name:0x80000000000F2EEC".yellow());
        eprintln!("  --filter {}", "severity:error".yellow());
        eprintln!("  --filter {}", "title:speed".yellow());
        eprintln!("  --filter {}", "numeric:RPM:0-5000".yellow());
        eprintln!("  --filter {}", "numeric:Element:10".yellow());
        eprintln!("  --filter {}", "flag:Engine=on".yellow());
    }
    pub fn parse(expr: &str) -> Result<Box<dyn Filter>, anyhow::Error> {
        if let Some(rest) = expr.strip_prefix("title:") {
            Ok(Box::new(TitleFilter::new(rest)))
        } else if let Some(rest) = expr.strip_prefix("pgn:") {
            let pgn: u32 = parse_hex_or_dec_u32(rest)?;
            Ok(Box::new(PgnFilter::new(pgn)))
        } else if let Some(rest) = expr.strip_prefix("severity:") {
            let severity = match rest.to_lowercase().as_str() {
                "info" => Severity::Info,
                "warning" => Severity::Warning,
                "error" => Severity::Error,
                _ => anyhow::bail!("unknown severity: {}", rest),
            };
            Ok(Box::new(SeverityFilter { severity }))
        } else if let Some(rest) = expr.strip_prefix("flag:") {
            match rest.find('=') {
                Some(eq_pos) => {
                    let title = rest[..eq_pos].to_string();
                    let val_str = &rest[eq_pos + 1..];
                    let value = match val_str.to_lowercase().as_str() {
                        "off" => FlagValue::Off,
                        "on" => FlagValue::On,
                        "error" => FlagValue::Error,
                        "unavailable" => FlagValue::Unavailable,
                        _ => anyhow::bail!("unknown flag value: {}", val_str),
                    };
                    Ok(Box::new(FlagFilter { title, value }))
                }
                None => anyhow::bail!("flag filter requires '=' (e.g. flag:title=on)"),
            }
        } else if let Some(rest) = expr.strip_prefix("numeric:") {
            // numeric:<title>:<range> where <range> supports: N (exact), N-M (range), >=N (min), <=N (max), or comma-separated values with hex support
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() < 2 {
                anyhow::bail!("numeric filter requires title and range (e.g. numeric:RPM:0-5000)");
            }
            let title = parts[0].to_string();
            let range_str = parts[1];

            // Check for comma-separated exact values (supports hex: "144,0xfe" or "0x90,254")
            if range_str.contains(',') {
                let exact_values: Vec<f64> = range_str
                    .split(',')
                    .map(|s| parse_hex_or_dec_f64(s.trim()))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Box::new(NumericFilter {
                    title,
                    min: None,
                    max: None,
                    exact_values,
                }))
            } else {
                let (min, max) = if let Some(stripped) = range_str.strip_prefix(">=") {
                    (Some(parse_hex_or_dec_f64(stripped)?), None)
                } else if let Some(stripped) = range_str.strip_prefix("<=") {
                    (None, Some(parse_hex_or_dec_f64(stripped)?))
                } else if let Some(pos) = range_str.find('-') {
                    let min_val: f64 = parse_hex_or_dec_f64(&range_str[..pos])?;
                    let max_val: f64 = parse_hex_or_dec_f64(&range_str[pos + 1..])?;
                    (Some(min_val), Some(max_val))
                } else {
                    let val: f64 = parse_hex_or_dec_f64(range_str)?;
                    (Some(val), Some(val))
                };
                Ok(Box::new(NumericFilter {
                    title,
                    min,
                    max,
                    exact_values: vec![],
                }))
            }
        } else if let Some(rest) = expr.strip_prefix("source:") {
            let sources: Vec<u8> = rest
                .split(',')
                .map(|s| parse_hex_or_dec_u8(s))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Box::new(SourceFilter::from_list(sources)))
        } else if let Some(rest) = expr.strip_prefix("dest:") {
            let dests: Vec<u8> = rest
                .split(',')
                .map(|s| parse_hex_or_dec_u8(s))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Box::new(DestFilter::from_list(dests)))
        } else if let Some(rest) = expr.strip_prefix("src-name:") {
            let name: u64 = parse_hex_or_dec_u64(rest)?;
            Ok(Box::new(SourceNameFilter::new(name)))
        } else if let Some(rest) = expr.strip_prefix("dest-name:") {
            let name: u64 = parse_hex_or_dec_u64(rest)?;
            Ok(Box::new(DestNameFilter::new(name)))
        } else if let Some(rest) = expr.strip_prefix("regex:") {
            RegexFilter::new(rest)
                .map(|f| Box::new(f) as Box<dyn Filter>)
                .map_err(anyhow::Error::from)
        } else {
            FilterParser::print_help();
            Err(anyhow::anyhow!("unknown filter type"))
        }
    }
}

/// Parse a numeric string that may be decimal or hex (0x prefix).
fn parse_hex_or_dec_f64(s: &str) -> Result<f64, anyhow::Error> {
    s.parse::<f64>().or_else(|_| {
        let stripped = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .ok_or_else(|| anyhow::anyhow!("invalid numeric value: {}", s))?;
        u64::from_str_radix(stripped, 16)
            .map(|v| v as f64)
            .map_err(anyhow::Error::from)
    })
}

/// Parse a single byte address that may be decimal or hex (0x prefix).
fn parse_hex_or_dec_u8(s: &str) -> Result<u8, anyhow::Error> {
    s.trim().parse::<u8>().or_else(|_| {
        let stripped = s
            .trim()
            .strip_prefix("0x")
            .or_else(|| s.trim().strip_prefix("0X"))
            .ok_or_else(|| anyhow::anyhow!("invalid address value: {}", s))?;
        u8::from_str_radix(stripped, 16).map_err(anyhow::Error::from)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_or_dec_f64_decimal() {
        assert_eq!(parse_hex_or_dec_f64("144").unwrap(), 144.0);
        assert_eq!(parse_hex_or_dec_f64("254").unwrap(), 254.0);
    }

    #[test]
    fn test_parse_hex_or_dec_f64_hex() {
        assert_eq!(parse_hex_or_dec_f64("0x90").unwrap(), 144.0);
        assert_eq!(parse_hex_or_dec_f64("0xfe").unwrap(), 254.0);
        assert_eq!(parse_hex_or_dec_f64("0X90").unwrap(), 144.0);
    }

    #[test]
    fn test_parse_hex_or_dec_f64_invalid() {
        assert!(parse_hex_or_dec_f64("abc").is_err());
        assert!(parse_hex_or_dec_f64("0xxyz").is_err());
    }

    #[test]
    fn test_numeric_filter_parser_hex_exact_values() {
        let filter = FilterParser::parse("numeric:RPM:144,0xfe");
        assert!(filter.is_ok());
        let f = filter.unwrap();
        assert_eq!(f.name(), "numeric_filter");
    }

    #[test]
    fn test_numeric_filter_parser_hex_range() {
        let filter = FilterParser::parse("numeric:RPM:0x90-0xfe");
        assert!(filter.is_ok());
        let f = filter.unwrap();
        assert_eq!(f.name(), "numeric_filter");
    }

    #[test]
    fn test_numeric_filter_parser_hex_min() {
        let filter = FilterParser::parse("numeric:RPM:>=0x90");
        assert!(filter.is_ok());
        let f = filter.unwrap();
        assert_eq!(f.name(), "numeric_filter");
    }

    #[test]
    fn test_source_filter_parser_hex() {
        let filter = FilterParser::parse("source:144,0xfe");
        assert!(filter.is_ok());
        let f = filter.unwrap();
        assert_eq!(f.name(), "source_filter");
    }

    #[test]
    fn test_dest_filter_parser_hex() {
        let filter = FilterParser::parse("dest:0x90,254");
        assert!(filter.is_ok());
        let f = filter.unwrap();
        assert_eq!(f.name(), "dest_filter");
    }

    #[test]
    fn test_parse_hex_or_dec_u8_decimal() {
        assert_eq!(parse_hex_or_dec_u8("144").unwrap(), 144);
        assert_eq!(parse_hex_or_dec_u8("255").unwrap(), 255);
    }

    #[test]
    fn test_parse_hex_or_dec_u8_hex() {
        assert_eq!(parse_hex_or_dec_u8("0x90").unwrap(), 144);
        assert_eq!(parse_hex_or_dec_u8("0xfe").unwrap(), 254);
        assert_eq!(parse_hex_or_dec_u8("0XFF").unwrap(), 255);
    }

    #[test]
    fn test_parse_hex_or_dec_u8_invalid() {
        assert!(parse_hex_or_dec_u8("300").is_err());
        assert!(parse_hex_or_dec_u8("abc").is_err());
    }
}
