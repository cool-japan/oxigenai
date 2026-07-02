use crate::models::law::FullArticle;
use crate::models::legal_result::{SectionClassification, VerificationSummary};
use crate::services::gemini_client::GeminiService;
use crate::verifier::contradiction::{
    build_verification_summary_from_report, format_contradiction_warnings, run_full_verification,
};
use crate::verifier::dsl_bridge::{ConversionSource, StatuteBridge};
use crate::verifier::jurisdiction::DEFAULT_JURISDICTION;
use legalis_core::{LegalResult, Statute};
use tracing::info;

/// Annotated report with verification results.
#[derive(Debug)]
pub struct AnnotatedReport {
    /// The final report text with the verification section appended
    pub report_text: String,
    /// Verification summary for this report
    pub verification_summary: VerificationSummary,
}

/// Contradiction warning suitable for prepending to the report prompt.
#[derive(Debug)]
pub struct ContradictionPrefix {
    /// The warning text to prepend to references
    pub text: String,
    /// Whether any contradictions were found
    pub has_contradictions: bool,
}

/// Legal verifier integrating Legalis-RS with OxiZ SMT solver.
/// This is the new capability layer that goes beyond the Python original.
pub struct LegalVerifier {
    bridge: StatuteBridge,
}

impl LegalVerifier {
    /// Create a new LegalVerifier instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bridge: StatuteBridge::new(),
        }
    }

    /// Classify a `LegalResult<T>` into a `SectionClassification`.
    /// This is the direct legalis-core → OxigenAI bridge.
    #[must_use]
    pub fn classify_from_legal_result<T>(result: &LegalResult<T>) -> SectionClassification {
        SectionClassification::from(result)
    }

    /// Detect logical contradictions between statutes using OxiZ SMT (jurisdiction `"JP"`).
    /// Returns a prefix string to prepend to the Gemini report prompt.
    ///
    /// Thin wrapper over [`Self::build_contradiction_prefix_for`] (JP default).
    pub async fn build_contradiction_prefix(
        &self,
        articles: &[FullArticle],
        gemini: &GeminiService,
    ) -> ContradictionPrefix {
        self.build_contradiction_prefix_for(articles, gemini, DEFAULT_JURISDICTION)
            .await
    }

    /// Detect logical contradictions for `jurisdiction` using OxiZ SMT.
    ///
    /// Uses the jurisdiction-aware `StatuteBridge` with Gemini DSL translation
    /// for unknown domains.
    pub async fn build_contradiction_prefix_for(
        &self,
        articles: &[FullArticle],
        gemini: &GeminiService,
        jurisdiction: &str,
    ) -> ContradictionPrefix {
        if articles.is_empty() {
            return ContradictionPrefix {
                text: String::new(),
                has_contradictions: false,
            };
        }

        let article_statutes = self
            .bridge
            .convert_articles_for(articles, gemini, jurisdiction)
            .await;
        let statutes: Vec<Statute> = article_statutes.into_iter().map(|a| a.statute).collect();

        if statutes.is_empty() {
            return ContradictionPrefix {
                text: String::new(),
                has_contradictions: false,
            };
        }

        let report = run_full_verification(&statutes);
        let has_contradictions = !report.contradictions.is_empty();
        let text = format_contradiction_warnings(&report.contradictions);

        ContradictionPrefix {
            text,
            has_contradictions,
        }
    }

    /// Annotate a completed report with verification results (jurisdiction `"JP"`).
    /// Appends a "## 法的整合性検証" section to the report.
    ///
    /// Thin wrapper over [`Self::annotate_report_for`] (JP default).
    pub async fn annotate_report(
        &self,
        report_text: &str,
        articles: &[FullArticle],
        gemini: &GeminiService,
    ) -> AnnotatedReport {
        self.annotate_report_for(report_text, articles, gemini, DEFAULT_JURISDICTION)
            .await
    }

    /// Annotate a completed report with verification results for `jurisdiction`.
    /// Appends a "## 法的整合性検証" section to the report.
    ///
    /// Uses Gemini for DSL translation of non-domain articles.
    pub async fn annotate_report_for(
        &self,
        report_text: &str,
        articles: &[FullArticle],
        gemini: &GeminiService,
        jurisdiction: &str,
    ) -> AnnotatedReport {
        if articles.is_empty() {
            let summary = VerificationSummary::default();
            let verification_section = summary.to_markdown();
            return AnnotatedReport {
                report_text: format!("{}\n\n{}", report_text, verification_section),
                verification_summary: summary,
            };
        }

        // Convert articles → statutes via hybrid bridge
        let article_statutes = self
            .bridge
            .convert_articles_for(articles, gemini, jurisdiction)
            .await;

        let statutes_analyzed = articles.len();
        let statutes_parsed = article_statutes
            .iter()
            .filter(|a| a.source != ConversionSource::Fallback)
            .count();

        let statutes: Vec<Statute> = article_statutes.into_iter().map(|a| a.statute).collect();

        // Run full verification
        let report = run_full_verification(&statutes);

        // Classify sections
        let (deterministic_count, discretion_count) =
            classify_report_sections(report_text, &statutes);

        info!(
            "Legalis verification: {} statutes, {} parsed, grade={:?}, {} contradictions",
            statutes_analyzed,
            statutes_parsed,
            report.quality_grade,
            report.contradictions.len()
        );

        let verification_summary = build_verification_summary_from_report(
            &report,
            statutes_analyzed,
            statutes_parsed,
            deterministic_count,
            discretion_count,
        );

        let verification_section = verification_summary.to_markdown();
        let annotated_text = format!("{}\n\n{}", report_text, verification_section);

        AnnotatedReport {
            report_text: annotated_text,
            verification_summary,
        }
    }
}

impl Default for LegalVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Classify report sections by scanning headings for discretionary language.
/// Returns `(deterministic_count, discretion_count)`.
fn classify_report_sections(report_text: &str, statutes: &[Statute]) -> (usize, usize) {
    let section_headings: Vec<&str> = report_text
        .lines()
        .filter(|l| l.starts_with("## ") || l.starts_with("### "))
        .collect();

    if section_headings.is_empty() {
        // No headings — classify based on statutes
        let deterministic = statutes
            .iter()
            .filter(|s| {
                s.preconditions
                    .iter()
                    .all(|c| !is_discretionary_condition(c))
            })
            .count();
        let discretion = statutes.len().saturating_sub(deterministic);
        return (deterministic, discretion);
    }

    let mut deterministic = 0usize;
    let mut discretion = 0usize;

    for heading in section_headings {
        if contains_discretionary_language(heading) {
            discretion += 1;
        } else {
            deterministic += 1;
        }
    }

    (deterministic, discretion)
}

/// Check if a legalis-core Condition involves judicial discretion.
fn is_discretionary_condition(condition: &legalis_core::Condition) -> bool {
    match condition {
        legalis_core::Condition::Custom { description } => {
            contains_discretionary_language(description)
        }
        _ => false,
    }
}

/// Check if text contains Japanese legal discretionary language.
/// These phrases indicate judicial discretion rather than mechanical application.
fn contains_discretionary_language(text: &str) -> bool {
    const DISCRETIONARY_PHRASES: &[&str] = &[
        "正当な理由",
        "相当の理由",
        "やむを得ない",
        "合理的な",
        "相当と認める",
        "必要と認める",
        "適切と認める",
        "裁量",
        "酌量",
        "妥当",
        "考慮",
        "衡量",
        "比例原則",
    ];

    DISCRETIONARY_PHRASES
        .iter()
        .any(|phrase| text.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legal_verifier_creation() {
        let verifier = LegalVerifier::new();
        let _ = verifier; // Just test it doesn't panic
    }

    #[test]
    fn test_discretionary_language_detection() {
        assert!(contains_discretionary_language("正当な理由がある場合"));
        assert!(contains_discretionary_language("裁量権の逸脱"));
        assert!(!contains_discretionary_language("年齢が18歳以上であること"));
    }

    #[test]
    fn test_classify_from_legal_result_deterministic() {
        let r: LegalResult<i32> = LegalResult::Deterministic(42);
        let cls = LegalVerifier::classify_from_legal_result(&r);
        assert_eq!(cls, SectionClassification::Deterministic);
    }

    #[test]
    fn test_classify_from_legal_result_void() {
        let r: LegalResult<i32> = LegalResult::Void {
            reason: "test".into(),
        };
        let cls = LegalVerifier::classify_from_legal_result(&r);
        assert_eq!(cls, SectionClassification::Void);
    }

    #[test]
    fn test_classify_report_sections_no_headings() {
        use legalis_jp::reasoning::lsa_article_32_working_hours;
        let statutes = vec![lsa_article_32_working_hours()];
        let (det, disc) = classify_report_sections("No headings here.", &statutes);
        assert_eq!(det + disc, statutes.len());
    }

    #[test]
    fn test_classify_report_sections_with_headings() {
        let (det, disc) = classify_report_sections("## 概要\n内容\n## 裁量事項\n内容", &[]);
        assert_eq!(det, 1);
        assert_eq!(disc, 1);
    }
}
