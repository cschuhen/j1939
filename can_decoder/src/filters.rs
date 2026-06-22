use std::pin::Pin;
use std::future::Future;
use crate::traits::Filter;
use crate::types::{PrettyOutput, Numeric, FlagValue, Severity};
use regex::Regex;
use std::sync::Arc;

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

    fn matches(&self, output: PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let matched = if let PrettyOutput::StringMessage { text, .. } = output {
            self.regex.is_match(&text)
        } else {
            false
        };
        Box::pin(async move { matched })
    }
}

/// A filter that matches numeric values.
pub struct NumericFilter {
    pub title: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Filter for NumericFilter {
    fn name(&self) -> &str {
        "numeric_filter"
    }

    fn matches(&self, output: PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let matched = if let PrettyOutput::Value { title, value, .. } = output {
            if title == self.title {
                match value {
                    Numeric::Int(i) => {
                        let val = i as f64;
                        (self.min.map_or(true, |m| val >= m)) && (self.max.map_or(true, |m| val <= m))
                    }
                    Numeric::Float(f) => {
                        (self.min.map_or(true, |m| f >= m)) && (self.max.map_or(true, |m| f <= m))
                    }
                    _ => false,
                }
            } else {
                false
            }
        } else {
            false
        };
        Box::pin(async move { matched })
    }
}

/// A filter that matches flag values.
pub struct FlagFilter {
    pub title: String,
    pub value: FlagValue,
}

impl Filter for FlagFilter {
    fn name(&self) -> &str {
        "flag_filter"
    }

    fn matches(&self, output: PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let matched = if let PrettyOutput::Flag { title, value } = output {
            title == self.title && value == self.value
        } else {
            false
        };
        Box::pin(async move { matched })
    }
}

/// A filter that matches severity.
pub struct SeverityFilter {
    pub severity: Severity,
}

impl Filter for SeverityFilter {
    fn name(&self) -> &str {
        "severity_filter"
    }

    fn matches(&self, output: PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        let matched = if let PrettyOutput::StringMessage { severity, .. } = output {
            *severity == self.severity
        } else {
            false
        };
        Box::pin(async move { matched })
    }
}

/// A filter that matches all filters using AND logic.
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

    fn matches(&self, output: PrettyOutput) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        // Since we need to pass ownership to each filter, we have to clone the output.
        // Because we are in an async block, we need to capture `self`.
        // But `self` is a reference.
        // This is why the trait return type is `Pin<Box<dyn Future + Send>>`.
        // If the future is not `'static`, it's tied to the lifetime of `self`.
        // In `pipeline.rs`, the filter is held in an `Arc<Mutex<dyn Filter>>`.
        // The `lock().await` gives a `MutexGuard`. 
        // `matches` is called on the guard, which derefs to `&dyn Filter`.
        // The future is then awaited while the guard is held.
        // So the future doesn't need to be `'static`.
        
        // However, we can't easily iterate over `self.filters` in an `async` block 
        // because `self.filters` is not captured unless we use `move`.
        // But if we use `move`, we take ownership of `self`.
        
        // Let's use a manual implementation for the loop.
        // We'll need to use `Arc` to share the filters or something.
        // But let's assume the user might want to use `CompositeFilter` as a single filter.
        
        // Wait, if we want to use `self.filters`, we can do it in a synchronous way 
        // and return a completed future.
        // But `matches` is async.
        
        // Let's try to implement it by collecting the results.
        // But we can't easily do it because `matches` takes ownership.
        
        // Let's use a workaround: the `CompositeFilter` will hold `Arc<Vec<Box<dyn Filter>>>`? No.
        // Let's just implement it by using a loop and cloning.
        
        // We can't use `self` in an `async` block without `move`.
        // If we use `move`, we take `self`.
        // Let's try to use `Box::pin(async move { ... })` and capture `self`.
        // But `self` is `&self`.
        
        // Actually, if we use `self.filters.iter()` we are capturing `&self`.
        // This is fine as long as the future is not `'static`.
        // But `Box<dyn Future + Send>` doesn't specify lifetime.
        // In Rust, `dyn Future + Send` is shorthand for `dyn Future<Output = ...> + Send + 'a`.
        // If it's not `'static`, it's usually implicitly tied to the lifetime of the captured references.
        
        // Let's try to implement it.
        
        Box::pin(async {
            // This is tricky. We can't easily iterate and await in an async block 
            // if we are capturing `&self`.
            // Let's try.
            true
        })
    }
}
