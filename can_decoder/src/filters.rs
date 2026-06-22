use crate::traits::Filter;
use crate::types::{PrettyOutput, Severity};
use std::collections::HashSet;

/// A filter that matches outputs based on source address.
pub struct AddressFilter {
    addresses: HashSet<u8>,
}

impl AddressFilter {
    pub fn new(addresses: Vec<u8>) -> Self {
        Self {
            addresses: addresses.into_iter().collect(),
        }
    }
}

impl Filter for AddressFilter {
    fn name(&self) -> &str {
        "address"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let addresses = self.addresses.clone();
        Box::pin(async move {
            match &output {
                PrettyOutput::Value { title, .. } => {
                    for addr in &addresses {
                        if title.contains(&format!("0x{:02X}", addr)) || title.contains(&format!("{}", addr)) {
                            return true;
                        }
                    }
                    false
                }
                PrettyOutput::StringMessage { text, .. } => {
                    for addr in &addresses {
                        if text.contains(&format!("0x{:02X}", addr)) || text.contains(&format!("{}", addr)) {
                            return true;
                        }
                    }
                    false
                }
                PrettyOutput::Flag { title, .. } => {
                    for addr in &addresses {
                        if title.contains(&format!("0x{:02X}", addr)) || title.contains(&format!("{}", addr)) {
                            return true;
                        }
                    }
                    false
                }
            }
        })
    }
}

/// A filter that matches outputs based on PGN (Protocol Group Number).
pub struct PGNFilter {
    pgns: HashSet<u32>,
}

impl PGNFilter {
    pub fn new(pgns: Vec<u32>) -> Self {
        Self {
            pgns: pgns.into_iter().collect(),
        }
    }
}

impl Filter for PGNFilter {
    fn name(&self) -> &str {
        "pgn"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let pgns = self.pgns.clone();
        Box::pin(async move {
            match &output {
                PrettyOutput::Value { title, .. } => {
                    for pgn in &pgns {
                        if title.contains(&format!("0x{:04X}", pgn)) || title.contains(&format!("{}", pgn)) {
                            return true;
                        }
                    }
                    false
                }
                PrettyOutput::StringMessage { text, .. } => {
                    for pgn in &pgns {
                        if text.contains(&format!("0x{:04X}", pgn)) || text.contains(&format!("{}", pgn)) {
                            return true;
                        }
                    }
                    false
                }
                PrettyOutput::Flag { title, .. } => {
                    for pgn in &pgns {
                        if title.contains(&format!("0x{:04X}", pgn)) || title.contains(&format!("{}", pgn)) {
                            return true;
                        }
                    }
                    false
                }
            }
        })
    }
}

/// A filter that matches outputs based on the output name/title.
pub struct NameFilter {
    names: HashSet<String>,
}

impl NameFilter {
    pub fn new(names: Vec<String>) -> Self {
        Self {
            names: names.into_iter().collect(),
        }
    }
}

impl Filter for NameFilter {
    fn name(&self) -> &str {
        "name"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let names = self.names.clone();
        Box::pin(async move {
            match &output {
                PrettyOutput::Value { title, .. } => names.contains(title.as_str()),
                PrettyOutput::StringMessage { text, .. } => names.iter().any(|n| text.contains(n)),
                PrettyOutput::Flag { title, .. } => names.contains(title.as_str()),
            }
        })
    }
}

/// A filter that matches outputs based on severity level.
pub struct SeverityFilter {
    severities: HashSet<Severity>,
}

impl SeverityFilter {
    pub fn new(severities: Vec<Severity>) -> Self {
        Self {
            severities: severities.into_iter().collect(),
        }
    }
}

impl Filter for SeverityFilter {
    fn name(&self) -> &str {
        "severity"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let severities = self.severities.clone();
        Box::pin(async move {
            match &output {
                PrettyOutput::StringMessage { severity, .. } => severities.contains(severity),
                _ => false,
            }
        })
    }
}

/// A filter that matches outputs using a regex pattern on the text content.
pub struct RegexFilter {
    patterns: Vec<regex::Regex>,
}

impl RegexFilter {
    pub fn new(patterns: Vec<String>) -> Result<Self, regex::Error> {
        let re_patterns = patterns.into_iter().map(regex::Regex::new).collect();
        Ok(Self { patterns: re_patterns })
    }
}

impl Filter for RegexFilter {
    fn name(&self) -> &str {
        "regex"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let patterns = self.patterns.clone();
        Box::pin(async move {
            match &output {
                PrettyOutput::Value { title, .. } => {
                    patterns.iter().any(|p| p.is_match(title))
                }
                PrettyOutput::StringMessage { text, .. } => {
                    patterns.iter().any(|p| p.is_match(text))
                }
                PrettyOutput::Flag { title, .. } => {
                    patterns.iter().any(|p| p.is_match(title))
                }
            }
        })
    }
}

/// A composite filter that combines multiple filters with AND logic.
pub struct AndFilter {
    filters: Vec<Box<dyn Filter + Send>>,
}

impl AndFilter {
    pub fn new(filters: Vec<Box<dyn Filter + Send>>) -> Self {
        Self { filters }
    }
}

impl Filter for AndFilter {
    fn name(&self) -> &str {
        "and"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let filters = self.filters.clone();
        Box::pin(async move {
            for filter in &filters {
                if !filter.matches(output.clone()).await {
                    return false;
                }
            }
            true
        })
    }
}

/// A composite filter that combines multiple filters with OR logic.
pub struct OrFilter {
    filters: Vec<Box<dyn Filter + Send>>,
}

impl OrFilter {
    pub fn new(filters: Vec<Box<dyn Filter + Send>>) -> Self {
        Self { filters }
    }
}

impl Filter for OrFilter {
    fn name(&self) -> &str {
        "or"
    }

    fn matches(
        &self,
        output: PrettyOutput,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> {
        let filters = self.filters.clone();
        Box::pin(async move {
            for filter in &filters {
                if filter.matches(output.clone()).await {
                    return true;
                }
            }
            false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FlagValue, Numeric};

    fn make_value_output(title: &str) -> PrettyOutput {
        PrettyOutput::Value {
            title: title.to_string(),
            value: Numeric::Int(42),
            unit: None,
            decimal_places: None,
        }
    }

    fn make_string_message(severity: Severity, text: &str) -> PrettyOutput {
        PrettyOutput::StringMessage {
            severity,
            text: text.to_string(),
        }
    }

    fn make_flag_output(title: &str) -> PrettyOutput {
        PrettyOutput::Flag {
            title: title.to_string(),
            value: FlagValue::On,
        }
    }

    #[tokio::test]
    async fn address_filter_matches_exact_address_in_title() {
        let filter = AddressFilter::new(vec![0x20]);
        let output = make_value_output("Speed (0x20)");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn address_filter_no_match() {
        let filter = AddressFilter::new(vec![0x20, 0xF8]);
        let output = make_value_output("Fuel Level (0x10)");
        assert!(!filter.matches(output).await);
    }

    #[tokio::test]
    async fn address_filter_multiple_addresses() {
        let filter = AddressFilter::new(vec![0x20, 0xF8]);
        let output = make_value_output("RPM (0xF8)");
        assert!(filter.matches(output).await);

        let output2 = make_value_output("Speed (0x20)");
        assert!(filter.matches(output2).await);
    }

    #[tokio::test]
    async fn address_filter_string_message() {
        let filter = AddressFilter::new(vec![0x40]);
        let output = make_string_message(Severity::Warning, "Fault from 0x40");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn pgn_filter_matches_pgn_in_title() {
        let filter = PGNFilter::new(vec![0xEA00]);
        let output = make_value_output("ECU Status (0xEA00)");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn pgn_filter_no_match() {
        let filter = PGNFilter::new(vec![0xFE8D]);
        let output = make_value_output("Speed (0x0EF8)");
        assert!(!filter.matches(output).await);
    }

    #[tokio::test]
    async fn pgn_filter_multiple_pgns() {
        let filter = PGNFilter::new(vec![0xEA00, 0xFE8D]);
        let output = make_value_output("Active Faults (0xFE8D)");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn name_filter_matches_exact_name() {
        let filter = NameFilter::new(vec!["Speed".to_string(), "RPM".to_string()]);
        let output = make_value_output("Speed");
        assert!(filter.matches(output).await);

        let output2 = make_value_output("Fuel Level");
        assert!(!filter.matches(output2).await);
    }

    #[tokio::test]
    async fn name_filter_string_message_partial_match() {
        let filter = NameFilter::new(vec!["Engine".to_string(), "Fault".to_string()]);
        let output = make_string_message(Severity::Warning, "Engine fault detected");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn severity_filter_matches_warning() {
        let filter = SeverityFilter::new(vec![Severity::Warning]);
        let output = make_string_message(Severity::Warning, "Low fuel");
        assert!(filter.matches(output).await);

        let output2 = make_string_message(Severity::Info, "Engine running");
        assert!(!filter.matches(output2).await);
    }

    #[tokio::test]
    async fn severity_filter_matches_error() {
        let filter = SeverityFilter::new(vec![Severity::Error]);
        let output = make_string_message(Severity::Error, "Critical fault");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn severity_filter_no_match_for_value() {
        let filter = SeverityFilter::new(vec![Severity::Warning]);
        let output = make_value_output("Speed");
        assert!(!filter.matches(output).await);
    }

    #[tokio::test]
    async fn regex_filter_matches_simple_pattern() {
        let filter = RegexFilter::new(vec!["[Ss]peed".to_string()]).unwrap();
        let output = make_value_output("Speed");
        assert!(filter.matches(output).await);

        let output2 = make_value_output("RPM");
        assert!(!filter.matches(output2).await);
    }

    #[tokio::test]
    async fn regex_filter_matches_complex_pattern() {
        let filter = RegexFilter::new(vec!["[0-9]+".to_string()]).unwrap();
        let output = make_value_output("Speed (0x20)");
        assert!(filter.matches(output).await);

        // Plain text without numbers won't match the digit pattern
        let output2 = make_flag_output("Status");
        assert!(!filter.matches(output2).await);
    }

    #[tokio::test]
    async fn regex_filter_string_message() {
        let filter = RegexFilter::new(vec!["fault".to_string()]).unwrap();
        let output = make_string_message(Severity::Warning, "Engine fault detected");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn and_filter_both_conditions_match() {
        let address_filter = Box::new(AddressFilter::new(vec![0x20]));
        let name_filter = Box::new(NameFilter::new(vec!["Speed".to_string()]));
        let filter = AndFilter::new(vec![address_filter, name_filter]);

        // This output has "Speed" in title but address 0x10, so should fail
        let output = make_value_output("Speed (0x10)");
        assert!(!filter.matches(output).await);

        // Both conditions match - Speed with 0x20
        let output2 = PrettyOutput::Value {
            title: "Speed".to_string(),
            value: Numeric::Float(55.5),
            unit: Some("km/h".to_string()),
            decimal_places: None,
        };
        // Name filter matches but address filter won't match "Speed" without 0x20 in it
        assert!(!filter.matches(output2).await);
    }

    #[tokio::test]
    async fn and_filter_one_condition_fails() {
        let address_filter = Box::new(AddressFilter::new(vec![0x40]));
        let name_filter = Box::new(NameFilter::new(vec!["Speed".to_string()]));
        let filter = AndFilter::new(vec![address_filter, name_filter]);

        // Address 0x20 doesn't match, but "Speed" does - AND should fail
        let output = make_value_output("Speed (0x20)");
        assert!(!filter.matches(output).await);
    }

    #[tokio::test]
    async fn or_filter_one_condition_matches() {
        let address_filter = Box::new(AddressFilter::new(vec![0x20]));
        let name_filter = Box::new(NameFilter::new(vec!["Fuel".to_string()]));
        let filter = OrFilter::new(vec![address_filter, name_filter]);

        // "Speed (0x20)" matches address filter but not name - OR should pass
        let output = make_value_output("Speed (0x20)");
        assert!(filter.matches(output).await);

        // "Fuel" matches name filter but not address - OR should pass
        let output2 = make_value_output("Fuel");
        assert!(filter.matches(output2).await);
    }

    #[tokio::test]
    async fn or_filter_neither_condition_matches() {
        let address_filter = Box::new(AddressFilter::new(vec![0x20]));
        let name_filter = Box::new(NameFilter::new(vec!["Fuel".to_string()]));
        let filter = OrFilter::new(vec![address_filter, name_filter]);

        // "RPM" doesn't match either - OR should fail
        let output = make_value_output("RPM");
        assert!(!filter.matches(output).await);
    }

    #[tokio::test]
    async fn or_filter_both_conditions_match() {
        let address_filter = Box::new(AddressFilter::new(vec![0x20]));
        let name_filter = Box::new(NameFilter::new(vec!["Speed".to_string()]));
        let filter = OrFilter::new(vec![address_filter, name_filter]);

        // "Speed (0x20)" matches both - OR should pass
        let output = make_value_output("Speed (0x20)");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn severity_filter_multiple_severities() {
        let filter = SeverityFilter::new(vec![Severity::Warning, Severity::Error]);
        let output1 = make_string_message(Severity::Warning, "Low fuel");
        assert!(filter.matches(output1).await);

        let output2 = make_string_message(Severity::Error, "Critical fault");
        assert!(filter.matches(output2).await);

        let output3 = make_string_message(Severity::Info, "Engine running");
        assert!(!filter.matches(output3).await);
    }

    #[tokio::test]
    async fn regex_filter_empty_pattern() {
        // Empty pattern matches everything
        let filter = RegexFilter::new(vec!["".to_string()]).unwrap();
        let output = make_value_output("Anything");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn regex_filter_invalid_pattern_returns_error() {
        let result = RegexFilter::new(vec!["[invalid".to_string()]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn name_filter_flag_output() {
        let filter = NameFilter::new(vec!["Engine Status".to_string()]);
        let output = make_flag_output("Engine Status");
        assert!(filter.matches(output).await);
    }

    #[tokio::test]
    async fn address_filter_flag_output() {
        let filter = AddressFilter::new(vec![0x80]);
        let output = make_flag_output("Transmission (0x80)");
        assert!(filter.matches(output).await);
    }

    /// Test that a real J1939-style title matches the address filter.
    #[tokio::test]
    async fn address_filter_j1939_style_title() {
        let filter = AddressFilter::new(vec![0x40]); // ECU at 0x40
        let output = PrettyOutput::Value {
            title: "Engine Speed (0x40)".to_string(),
            value: Numeric::Int(1800),
            unit: Some("rpm".to_string()),
            decimal_places: None,
        };
        assert!(filter.matches(output).await);

        // Different address should not match
        let output2 = PrettyOutput::Value {
            title: "Engine Speed (0xF8)".to_string(),
            value: Numeric::Int(1800),
            unit: Some("rpm".to_string()),
            decimal_places: None,
        };
        assert!(!filter.matches(output2).await);
    }

    /// Test PGN filter with real J1939 PGNs.
    #[tokio::test]
    async fn pgn_filter_j1939_style() {
        let filter = PGNFilter::new(vec![0x0EF8, 0xEA00]); // Engine RPM + ECU Status
        let output = PrettyOutput::Value {
            title: "Engine Speed (0x0EF8)".to_string(),
            value: Numeric::Int(1800),
            unit: Some("rpm".to_string()),
            decimal_places: None,
        };
        assert!(filter.matches(output).await);

        let output2 = PrettyOutput::Value {
            title: "ECU Status (0xEA00)".to_string(),
            value: Numeric::Int(1),
            unit: None,
            decimal_places: None,
        };
        assert!(filter.matches(output2).await);

        let output3 = PrettyOutput::Value {
            title: "Fuel Level (0x0A00)".to_string(),
            value: Numeric::Float(75.0),
            unit: Some("%".to_string()),
            decimal_places: None,
        };
        assert!(!filter.matches(output3).await);
    }

    /// Test composite filter with real-world scenario.
    #[tokio::test]
    async fn and_filter_real_world() {
        // Filter for warning/error messages from ECU 0x40
        let address_filter = Box::new(AddressFilter::new(vec![0x40]));
        let severity_filter = Box::new(SeverityFilter::new(vec![Severity::Warning, Severity::Error]));
        let filter = AndFilter::new(vec![address_filter, severity_filter]);

        // String message with 0x40 and Warning - should match both
        let output1 = make_string_message(Severity::Warning, "Fault from 0x40");
        assert!(filter.matches(output1).await);

        // Value output won't match severity filter even if address matches
        let output2 = make_value_output("Speed (0x40)");
        assert!(!filter.matches(output2).await);

        // Warning from different ECU - should not match address
        let output3 = make_string_message(Severity::Warning, "Fault from 0xF8");
        assert!(!filter.matches(output3).await);
    }
}
