//! Multi-jurisdiction matching layer (Phase 3-1).
//!
//! Promotes the JP-only `JpDomainMatcher` into an extensible abstraction so the
//! same hybrid bridge can resolve law titles for multiple jurisdictions. Each
//! jurisdiction implements [`JurisdictionMatcher`]; [`MultiJurisdictionMatcher`]
//! registers them and dispatches by code (default `"JP"`).
//!
//! - `JP` delegates to the existing [`JpDomainMatcher`] — behavior is unchanged.
//! - `EU` ships a real GDPR seed set (see [`crate::verifier::eu_statutes`]).
//! - `US` ships a real federal-law seed set (see [`crate::verifier::us_statutes`]).

use crate::verifier::dsl_bridge::JpDomainMatcher;
use crate::verifier::eu_statutes::{
    all_gdpr_statutes, gdpr_article_6_lawful_processing, gdpr_article_7_consent,
    gdpr_article_15_right_of_access, gdpr_article_17_right_to_erasure,
    gdpr_article_33_breach_notification,
};
use crate::verifier::us_statutes::{
    ada_section_12112_nondiscrimination, all_us_federal_statutes, flsa_section_206_minimum_wage,
    flsa_section_207_overtime, fmla_section_2612_leave_entitlement,
};
use legalis_core::Statute;
use serde::Serialize;
use std::collections::HashMap;
use tracing::warn;

/// Default jurisdiction code used when none is supplied or an unknown code is given.
pub const DEFAULT_JURISDICTION: &str = "JP";

/// A jurisdiction-specific matcher from law title keywords → pre-built statutes.
///
/// Implementations are the per-jurisdiction equivalents of `JpDomainMatcher`.
pub trait JurisdictionMatcher: Send + Sync {
    /// Stable uppercase jurisdiction code (e.g. `"JP"`, `"EU"`, `"US"`).
    fn code(&self) -> &'static str;
    /// Human-readable bilingual display name.
    fn display_name(&self) -> &'static str;
    /// Whether this jurisdiction ships a real (non-empty) statute set.
    fn is_populated(&self) -> bool;
    /// Returns statutes when `law_title` matches a known domain, else `None`.
    fn match_law_title(&self, law_title: &str) -> Option<Vec<Statute>>;
}

// ─── JP ──────────────────────────────────────────────────────────────────────────

/// Japanese-law matcher — delegates to the established [`JpDomainMatcher`].
pub struct JpMatcher;

impl JurisdictionMatcher for JpMatcher {
    fn code(&self) -> &'static str {
        "JP"
    }
    fn display_name(&self) -> &'static str {
        "日本法 / Japanese Law"
    }
    fn is_populated(&self) -> bool {
        true
    }
    fn match_law_title(&self, law_title: &str) -> Option<Vec<Statute>> {
        JpDomainMatcher::match_law_title(law_title)
    }
}

// ─── EU ──────────────────────────────────────────────────────────────────────────

/// European-Union-law matcher — backed by the GDPR seed set.
pub struct EuMatcher;

impl JurisdictionMatcher for EuMatcher {
    fn code(&self) -> &'static str {
        "EU"
    }
    fn display_name(&self) -> &'static str {
        "欧州連合法 / European Union Law (GDPR)"
    }
    fn is_populated(&self) -> bool {
        true
    }
    fn match_law_title(&self, law_title: &str) -> Option<Vec<Statute>> {
        // Lowercasing leaves Japanese unchanged, so a single haystack covers both
        // ASCII (case-insensitive) and Japanese (case-irrelevant) keywords.
        let hay = law_title.to_lowercase();
        let has = |needle: &str| hay.contains(needle);

        // Focused subsets first, then the broad GDPR catch-all.
        if has("erasure") || has("forgotten") || has("忘れられる権利") || has("消去") {
            return Some(vec![gdpr_article_17_right_to_erasure()]);
        }
        if has("breach") || has("notification") || has("漏えい") || has("漏洩") || has("侵害通知")
        {
            return Some(vec![gdpr_article_33_breach_notification()]);
        }
        if has("consent") || has("同意") {
            return Some(vec![
                gdpr_article_7_consent(),
                gdpr_article_6_lawful_processing(),
            ]);
        }
        if has("right of access") || has("subject access") || has("開示請求") || has("アクセス権")
        {
            return Some(vec![gdpr_article_15_right_of_access()]);
        }
        if has("gdpr")
            || has("personal data")
            || has("data protection")
            || has("data subject")
            || has("個人データ")
            || has("個人情報")
            || has("データ保護")
            || has("一般データ保護規則")
            || has("privacy")
            || has("プライバシー")
        {
            return Some(all_gdpr_statutes());
        }

        None
    }
}

// ─── US ──────────────────────────────────────────────────────────────────────────

/// United-States-federal-law matcher — backed by the FLSA/ADA/FMLA seed set.
pub struct UsMatcher;

impl JurisdictionMatcher for UsMatcher {
    fn code(&self) -> &'static str {
        "US"
    }
    fn display_name(&self) -> &'static str {
        "米国連邦法 / United States Federal Law"
    }
    fn is_populated(&self) -> bool {
        true
    }
    fn match_law_title(&self, law_title: &str) -> Option<Vec<Statute>> {
        let hay = law_title.to_lowercase();
        let has = |needle: &str| hay.contains(needle);

        // Focused subsets first, then the broad employment-law catch-all.
        if has("overtime") || has("時間外") || has("残業") {
            return Some(vec![flsa_section_207_overtime()]);
        }
        if has("minimum wage") || has("最低賃金") {
            return Some(vec![flsa_section_206_minimum_wage()]);
        }
        if has("flsa") || has("fair labor standards") || has("公正労働基準法") {
            return Some(vec![
                flsa_section_207_overtime(),
                flsa_section_206_minimum_wage(),
            ]);
        }
        if has("ada") || has("disability") || has("americans with disabilities") || has("障害") {
            return Some(vec![ada_section_12112_nondiscrimination()]);
        }
        if has("fmla")
            || has("family and medical leave")
            || has("medical leave")
            || has("育児")
            || has("休暇")
        {
            return Some(vec![fmla_section_2612_leave_entitlement()]);
        }
        if has("labor")
            || has("labour")
            || has("employment")
            || has("employee")
            || has("worker")
            || has("労働")
            || has("雇用")
        {
            return Some(all_us_federal_statutes());
        }

        None
    }
}

// ─── Discovery model ──────────────────────────────────────────────────────────────

/// Serializable descriptor of a registered jurisdiction (for `GET /jurisdictions`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JurisdictionInfo {
    /// Stable jurisdiction code (e.g. `"JP"`).
    pub code: String,
    /// Human-readable bilingual display name.
    pub display_name: String,
    /// Whether the jurisdiction ships a real statute set.
    pub is_populated: bool,
}

// ─── Registry ─────────────────────────────────────────────────────────────────────

/// Registry of jurisdiction matchers, keyed by uppercase code.
///
/// The default selection (empty or unknown code) is [`DEFAULT_JURISDICTION`].
pub struct MultiJurisdictionMatcher {
    matchers: HashMap<String, Box<dyn JurisdictionMatcher>>,
}

impl Default for MultiJurisdictionMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiJurisdictionMatcher {
    /// Builds the registry with the built-in JP, EU and US matchers.
    #[must_use]
    pub fn new() -> Self {
        let mut matchers: HashMap<String, Box<dyn JurisdictionMatcher>> = HashMap::new();
        let registered: [Box<dyn JurisdictionMatcher>; 3] = [
            Box::new(JpMatcher),
            Box::new(EuMatcher),
            Box::new(UsMatcher),
        ];
        for matcher in registered {
            matchers.insert(matcher.code().to_string(), matcher);
        }
        Self { matchers }
    }

    /// Normalizes a raw jurisdiction code into a registered one.
    ///
    /// Uppercases the input; maps empty or unknown codes to
    /// [`DEFAULT_JURISDICTION`] (`"JP"`) and logs a warning for unknown codes.
    #[must_use]
    pub fn normalize_code(&self, code: &str) -> String {
        let upper = code.trim().to_uppercase();
        if upper.is_empty() {
            return DEFAULT_JURISDICTION.to_string();
        }
        if self.matchers.contains_key(&upper) {
            upper
        } else {
            warn!(
                "Unknown jurisdiction code '{}' — falling back to '{}'",
                code, DEFAULT_JURISDICTION
            );
            DEFAULT_JURISDICTION.to_string()
        }
    }

    /// Returns the matcher for `code` (case-insensitive), or `None` if unknown.
    #[must_use]
    pub fn matcher(&self, code: &str) -> Option<&dyn JurisdictionMatcher> {
        self.matchers
            .get(&code.trim().to_uppercase())
            .map(|m| m.as_ref())
    }

    /// Matches `law_title` under `jurisdiction` (case-insensitive code lookup).
    ///
    /// An unknown code yields `None` — the caller decides how to fall back. Use
    /// [`Self::normalize_code`] first when a default selection is desired.
    #[must_use]
    pub fn match_for(&self, jurisdiction: &str, law_title: &str) -> Option<Vec<Statute>> {
        self.matchers
            .get(&jurisdiction.trim().to_uppercase())?
            .match_law_title(law_title)
    }

    /// Returns the registered jurisdictions, sorted by code (for discovery).
    #[must_use]
    pub fn available(&self) -> Vec<JurisdictionInfo> {
        let mut infos: Vec<JurisdictionInfo> = self
            .matchers
            .values()
            .map(|m| JurisdictionInfo {
                code: m.code().to_string(),
                display_name: m.display_name().to_string(),
                is_populated: m.is_populated(),
            })
            .collect();
        infos.sort_by(|a, b| a.code.cmp(&b.code));
        infos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jp_delegates_to_domain_matcher() {
        let matcher = JpMatcher;
        // A known JP statute title resolves through the existing JpDomainMatcher.
        let statutes = matcher.match_law_title("労働基準法").expect("JP match");
        assert!(statutes.iter().any(|s| s.id == "LSA_Art32"));
        // Out-of-domain title returns None, exactly as before.
        assert!(matcher.match_law_title("道路交通法").is_none());
    }

    #[test]
    fn test_eu_matches_gdpr_keyword() {
        let matcher = EuMatcher;
        let statutes = matcher
            .match_law_title("GDPR personal data processing")
            .expect("EU match");
        assert!(!statutes.is_empty());
        assert!(
            statutes
                .iter()
                .all(|s| s.jurisdiction.as_deref() == Some("EU"))
        );
        // 忘れられる権利 narrows to the erasure right (Art.17).
        let erasure = matcher.match_law_title("忘れられる権利").expect("erasure");
        assert!(erasure.iter().any(|s| s.id == "GDPR_Art17"));
    }

    #[test]
    fn test_us_matches_federal_keyword() {
        let matcher = UsMatcher;
        let overtime = matcher
            .match_law_title("FLSA overtime pay")
            .expect("US match");
        assert!(overtime.iter().any(|s| s.id == "FLSA_Sec207"));
        assert!(
            overtime
                .iter()
                .all(|s| s.jurisdiction.as_deref() == Some("US"))
        );
        let ada = matcher
            .match_law_title("ADA disability discrimination")
            .expect("ADA match");
        assert!(ada.iter().any(|s| s.id == "ADA_Sec12112"));
    }

    #[test]
    fn test_normalize_code_defaulting() {
        let registry = MultiJurisdictionMatcher::new();
        assert_eq!(registry.normalize_code(""), "JP");
        assert_eq!(registry.normalize_code("jp"), "JP");
        assert_eq!(registry.normalize_code("  eu "), "EU");
        // Unknown code falls back to the default.
        assert_eq!(registry.normalize_code("XX"), "JP");
    }

    #[test]
    fn test_match_for_unknown_code_is_none() {
        let registry = MultiJurisdictionMatcher::new();
        // Unknown code → None (caller decides), even with a matchable title.
        assert!(registry.match_for("XX", "GDPR").is_none());
        // Known code, case-insensitive.
        assert!(registry.match_for("eu", "GDPR").is_some());
        assert!(registry.match_for("JP", "労働基準法").is_some());
    }

    #[test]
    fn test_available_lists_three() {
        let registry = MultiJurisdictionMatcher::new();
        let available = registry.available();
        assert_eq!(available.len(), 3);
        let codes: Vec<&str> = available.iter().map(|j| j.code.as_str()).collect();
        assert_eq!(codes, vec!["EU", "JP", "US"]);
        assert!(available.iter().all(|j| j.is_populated));
    }

    #[test]
    fn test_matcher_lookup() {
        let registry = MultiJurisdictionMatcher::new();
        assert_eq!(registry.matcher("us").map(|m| m.code()), Some("US"));
        assert!(registry.matcher("zz").is_none());
    }
}
