use crate::traits::Filter;
use crate::types::{FlagValue, Numeric, PrettyOutput, Severity};
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

    fn matches(&self, output: &PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = if let PrettyOutput::StringMessage { text, .. } = output {
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

    fn matches(&self, output: &PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = if let PrettyOutput::Value { title, value, .. } = output {
            if title == &self.title {
                match value {
                    Numeric::Int(i) => {
                        let val: f64 = *i as f64;
                        (self.min.is_none_or(|m| val >= m)) && (self.max.is_none_or(|m| val <= m))
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
        };
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

    fn matches(&self, output: &PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = if let PrettyOutput::Flag { title, value } = output {
            title == &self.title && *value == self.value
        } else {
            false
        };
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

    fn matches(&self, output: &PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = if let PrettyOutput::StringMessage { severity, .. } = output {
            *severity == self.severity
        } else {
            false
        };
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

    fn matches(&self, output: &PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = if let PrettyOutput::Value { title, .. } = output {
            // Match on known PGN-containing titles from the decoder
            title.contains("PGN") || title.contains("pgn")
        } else {
            false
        };
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

    fn matches(&self, output: &PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = match output {
            PrettyOutput::Value { title, .. } => {
                title.to_lowercase().contains(&self.title_contains)
            }
            PrettyOutput::Flag { title, .. } => title.to_lowercase().contains(&self.title_contains),
            PrettyOutput::StringMessage { text, .. } => {
                text.to_lowercase().contains(&self.title_contains)
            }
        };
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
        output: &'a PrettyOutput,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move {
            let mut result = true;
            for i in 0..self.filters.len() {
                if !self.filters[i].matches(output).await {
                    result = false;
                    break;
                }
            }
            result
        })
    }
}

/// Parses a filter expression string into a Box<dyn Filter>.
/// Supported syntax:
///   - "title:<substring>" — TitleFilter
///   - "pgn:<hex_or_dec>"  — PgnFilter (e.g. pgn:0xEA00 or pgn:59904)
///   - "severity:<level>"   — SeverityFilter (info, warning, error)
///   - "flag:<title>=<value>" — FlagFilter (off, on, error, unavailable)
///   - "numeric:<title>:<min>-<max>" — NumericFilter
///   - "regex:<pattern>"    — RegexFilter
pub struct FilterParser;

impl FilterParser {
    pub fn parse(expr: &str) -> Result<Box<dyn Filter>, anyhow::Error> {
        if let Some(rest) = expr.strip_prefix("title:") {
            Ok(Box::new(TitleFilter::new(rest)))
        } else if let Some(rest) = expr.strip_prefix("pgn:") {
            let pgn: u32 = u32::from_str_radix(rest, 16)?;
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
            // numeric:<title>:<min>-<max> or numeric:<title>:>=<min> or numeric:<title>:<=<max>
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() < 2 {
                anyhow::bail!("numeric filter requires title and range (e.g. numeric:RPM:0-5000)");
            }
            let title = parts[0].to_string();
            let range_str = parts[1];
            let min = if let Some(stripped) = range_str.strip_prefix(">=") {
                Some(stripped.parse::<f64>()?)
            } else {
                None
            };
            let max = if let Some(stripped) = range_str.strip_prefix("<=") {
                Some(stripped.parse::<f64>()?)
            } else {
                None
            };
            Ok(Box::new(NumericFilter { title, min, max }))
        } else if let Some(rest) = expr.strip_prefix("regex:") {
            RegexFilter::new(rest)
                .map(|f| Box::new(f) as Box<dyn Filter>)
                .map_err(anyhow::Error::from)
        } else {
            Err(anyhow::anyhow!(
                "unknown filter type (use title:, pgn:, severity:, flag:, numeric:, regex:)"
            ))
        }
    }
}
