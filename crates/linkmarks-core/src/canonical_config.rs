//! Per-domain canonicalization rules.
use std::collections::HashMap;

/// Parameters that are functional by default.
pub const ALWAYS_FUNCTIONAL: &[&str] = &[
    "id", "page", "q", "query", "search", "lang", "locale", "sort", "order", "limit", "offset",
    "cursor", "tab", "filter",
];

#[derive(Debug, Clone, Default)]
/// Canonicalization configuration grouped by domain.
pub struct CanonicalConfig {
    /// Domain-specific preservation rules.
    pub domains: HashMap<String, DomainRules>,
}

#[derive(Debug, Clone, Default)]
/// Preservation rules for one domain.
pub struct DomainRules {
    /// Lowercase query parameter names to preserve.
    pub preserve_params: Vec<String>,
}

impl CanonicalConfig {
    /// Returns whether a parameter should be retained.
    pub fn is_preserved(&self, host: &str, param_lc: &str) -> bool {
        if ALWAYS_FUNCTIONAL.contains(&param_lc) {
            return true;
        }
        self.domains
            .get(host)
            .is_some_and(|r| r.preserve_params.iter().any(|p| p == param_lc))
    }

    /// Returns the safe default configuration.
    pub fn default_rules() -> Self {
        Self::default()
    }
}
