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
}

/// A filter that matches numeric values by title and range.
pub struct NumericFilter {
    pub title: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Filter for NumericFilter {
    fn name(&self) -> &str {
        "numeric_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = message.outputs.iter().any(|output| {
            if let DecodedField::Value { title, value, .. } = output {
                if title == &self.title {
                    match value {
                        Numeric::Int(i) => {
                            let val: f64 = *i as f64;
                            (self.min.is_none_or(|m| val >= m))
                                && (self.max.is_none_or(|m| val <= m))
                        }
                        Numeric::Float(f) => {
                            (self.min.is_none_or(|m| *f >= m)) && (self.max.is_none_or(|m| *f <= m))
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        });
        Box::pin(async move { result })
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
        let result = message.outputs.iter().any(|output| {
            if let DecodedField::Flag { title, value } = output {
                title == &self.title && *value == self.value
            } else {
                false
            }
        });
        Box::pin(async move { result })
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
        let result = message.outputs.iter().any(|output| {
            if let DecodedField::StringMessage { severity, .. } = output {
                *severity == self.severity
            } else {
                false
            }
        });
        Box::pin(async move { result })
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
        let result = self.pgn == message.assembled_message.pgn();
        Box::pin(async move { result })
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
        let msg_match = message.title.to_lowercase().contains(&self.title_contains);
        let output_match = message.outputs.iter().any(|output| match output {
            DecodedField::Value { title, .. } => {
                title.to_lowercase().contains(&self.title_contains)
            }
            DecodedField::Flag { title, .. } => title.to_lowercase().contains(&self.title_contains),
            DecodedField::StringMessage { text, .. } => {
                text.to_lowercase().contains(&self.title_contains)
            }
        });
        let result = msg_match || output_match;
        Box::pin(async move { result })
    }
}

/// A filter that matches by source address.
pub struct SourceFilter {
    pub source: u8,
}

impl SourceFilter {
    pub fn new(source: u8) -> Self {
        Self { source }
    }
}

impl Filter for SourceFilter {
    fn name(&self) -> &str {
        "source_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.source == message.assembled_message.source();
        Box::pin(async move { result })
    }
}

/// A filter that matches by destination address.
pub struct DestFilter {
    pub dest: u8,
}

impl DestFilter {
    pub fn new(dest: u8) -> Self {
        Self { dest }
    }
}

impl Filter for DestFilter {
    fn name(&self) -> &str {
        "dest_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = self.dest == message.assembled_message.destination();
        Box::pin(async move { result })
    }
}

/// A filter that matches by source NAME (64-bit J1939 NAME).
pub struct SourceNameFilter {
    pub source_name: u64,
}

impl SourceNameFilter {
    pub fn new(source_name: u64) -> Self {
        Self { source_name }
    }
}

impl Filter for SourceNameFilter {
    fn name(&self) -> &str {
        "source_name_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = message.assembled_message.source_name == Some(self.source_name);
        Box::pin(async move { result })
    }
}

/// A filter that matches by destination NAME (64-bit J1939 NAME).
pub struct DestNameFilter {
    pub dest_name: u64,
}

impl DestNameFilter {
    pub fn new(dest_name: u64) -> Self {
        Self { dest_name }
    }
}

impl Filter for DestNameFilter {
    fn name(&self) -> &str {
        "dest_name_filter"
    }

    fn matches(&self, message: &DecodedMessage) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = message.assembled_message.dest_name == Some(self.dest_name);
        Box::pin(async move { result })
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
            "  {}.{}:{}         Match numeric (exact N, range N-M, >=N, <=N; e.g. {} or {})",
            "numeric".green().bold(),
            "<title>".dimmed(),
            "<range>".dimmed(),
            "numeric:RPM:0-5000".yellow(),
            "numeric:Element:10".yellow()
        );
        eprintln!(
            "  {}.{}                Match string message text against regex pattern",
            "regex".green().bold(),
            "<pattern>".dimmed()
        );
        eprintln!(
            "  {}.{}              Match by source address (0-255, e.g. {})",
            "source".green().bold(),
            "<addr>".dimmed(),
            "source:144".yellow()
        );
        eprintln!(
            "  {}.{}              Match by destination address (0-255, e.g. {})",
            "dest".green().bold(),
            "<addr>".dimmed(),
            "dest:255".yellow()
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
        eprintln!("  --filter {}", "source:144".yellow());
        eprintln!("  --filter {}", "dest:255".yellow());
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
            // numeric:<title>:<range> where <range> is: N (exact), N-M (range), >=N (min), <=N (max)
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() < 2 {
                anyhow::bail!("numeric filter requires title and range (e.g. numeric:RPM:0-5000)");
            }
            let title = parts[0].to_string();
            let range_str = parts[1];
            let (min, max) = if let Some(stripped) = range_str.strip_prefix(">=") {
                (Some(stripped.parse::<f64>()?), None)
            } else if let Some(stripped) = range_str.strip_prefix("<=") {
                (None, Some(stripped.parse::<f64>()?))
            } else if let Some(pos) = range_str.find('-') {
                let min_val: f64 = range_str[..pos].parse()?;
                let max_val: f64 = range_str[pos + 1..].parse()?;
                (Some(min_val), Some(max_val))
            } else {
                let val: f64 = range_str.parse()?;
                (Some(val), Some(val))
            };
            Ok(Box::new(NumericFilter { title, min, max }))
        } else if let Some(rest) = expr.strip_prefix("source:") {
            let source: u8 = rest.parse()?;
            Ok(Box::new(SourceFilter::new(source)))
        } else if let Some(rest) = expr.strip_prefix("dest:") {
            let dest: u8 = rest.parse()?;
            Ok(Box::new(DestFilter::new(dest)))
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
