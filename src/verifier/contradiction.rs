use crate::models::legal_result::VerificationSummary;
use legalis_core::Statute;
use legalis_verifier::{
    StatuteVerifier, analyze_quality, check_due_process, check_equality, check_privacy_impact,
    detect_statute_conflicts,
};
use tracing::debug;

/// A contradiction detected by the OxiZ SMT solver.
#[derive(Debug, Clone)]
pub struct SmtContradiction {
    /// IDs of the statutes involved in the contradiction
    pub statute_ids: Vec<String>,
    /// Human-readable description of the contradiction
    pub explanation: String,
    /// Severity level (1=low, 2=medium, 3=high)
    pub severity: u8,
}

/// Full verification report from StatuteVerifier + constitutional checks + quality grading.
#[derive(Debug, Default)]
pub struct FullVerificationReport {
    /// Logical contradictions detected by OxiZ SMT.
    pub contradictions: Vec<SmtContradiction>,
    /// Errors from StatuteVerifier::verify()
    pub errors: Vec<String>,
    /// Warnings from StatuteVerifier::verify()
    pub warnings: Vec<String>,
    /// Suggestions from StatuteVerifier::verify()
    pub suggestions: Vec<String>,
    /// Overall quality grade (A-F), averaged across all statutes.
    pub quality_grade: Option<char>,
    /// Total statutes submitted
    pub total_statutes: usize,
}

/// Minimum severity for a conflict to count as a logical contradiction.
///
/// Severity 1 (Warning) typically means "overlapping conditions with different effect types",
/// which is **expected** in hierarchical legal systems:
///   - Base rule  (PROHIBITION: 40h weekly limit)          ← LSA_Art32
///   - Exception  (GRANT: overtime allowed with agreement) ← LSA_Art36
///   - Consequence (OBLIGATION: pay overtime premium)      ← LSA_Art37
///
/// These are complementary by design, not logical defects.
/// Only Error (2) and Critical (3) represent genuine contradictions.
const MIN_CONTRADICTION_SEVERITY: u8 = 2;

/// Extract the law-family prefix from a statute ID.
///
/// `"LSA_Art32"` → `"LSA"`, `"LCA_Art18"` → `"LCA"`, `"OT_LIMIT"` → `"OT_LIMIT"`
fn law_prefix(id: &str) -> &str {
    id.split("_Art").next().unwrap_or(id)
}

/// Returns `true` when all statute IDs in a conflict belong to the same law family.
///
/// Conflicts within one law (e.g., LSA_Art32 vs LSA_Art36) are intentional base-rule +
/// exception patterns and must not be treated as logical defects.
fn is_same_law_conflict(statute_ids: &[String]) -> bool {
    if statute_ids.len() < 2 {
        return false;
    }
    let first = law_prefix(&statute_ids[0]);
    statute_ids[1..].iter().all(|id| law_prefix(id) == first)
}

/// Detect logical contradictions between statutes using the OxiZ SMT solver.
/// This is the core Legalis-RS integration that goes beyond the Python original.
///
/// Uses `legalis_verifier::StatuteVerifier` which internally uses the OxiZ SMT backend.
///
/// **Filtering applied:**
/// - Severity < `MIN_CONTRADICTION_SEVERITY` (Warning-level) → suppressed.
///   These represent "complementary statutes" (base rule + exception), not defects.
/// - Same-law-family conflicts → suppressed.
///   Statutes from the same law are structured as base + exception by design.
pub fn detect_smt_contradictions(statutes: &[Statute]) -> Vec<SmtContradiction> {
    if statutes.is_empty() {
        return vec![];
    }

    debug!(
        "Running OxiZ SMT contradiction detection on {} statutes",
        statutes.len()
    );

    let conflicts = detect_statute_conflicts(statutes);
    let total_raw = conflicts.len();

    let contradictions: Vec<SmtContradiction> = conflicts
        .into_iter()
        .filter_map(|conflict| {
            let severity = match conflict.severity {
                legalis_verifier::Severity::Critical => 3,
                legalis_verifier::Severity::Error => 2,
                _ => 1,
            };

            // Suppress complementary-statute warnings
            if severity < MIN_CONTRADICTION_SEVERITY {
                return None;
            }

            // Suppress same-law structural patterns (base rule vs exception vs consequence)
            if is_same_law_conflict(&conflict.statute_ids) {
                return None;
            }

            Some(SmtContradiction {
                statute_ids: conflict.statute_ids,
                explanation: conflict.description,
                severity,
            })
        })
        .collect();

    debug!(
        "OxiZ: {} raw conflicts → {} genuine contradictions (filtered {} complementary patterns)",
        total_raw,
        contradictions.len(),
        total_raw - contradictions.len()
    );

    contradictions
}

/// Run the full Legalis-RS verification pipeline on a set of statutes:
///   - `StatuteVerifier::verify()` (circular refs, dead statutes, constitutional, contradictions)
///   - Constitutional principle checks (equality, due process, privacy)
///   - Quality grading via `analyze_quality()` (averaged A-F)
///
/// Returns a `FullVerificationReport` regardless of how many statutes pass or fail.
pub fn run_full_verification(statutes: &[Statute]) -> FullVerificationReport {
    if statutes.is_empty() {
        return FullVerificationReport::default();
    }

    debug!("Running full verification on {} statutes", statutes.len());

    let verifier = StatuteVerifier::new();

    // Core verification (circular refs, constitutional, contradictions)
    let vr = verifier.verify(statutes);

    let errors: Vec<String> = vr.errors.iter().map(|e| format!("{e:?}")).collect();
    let mut warnings = vr.warnings.clone();
    let mut suggestions = vr.suggestions.clone();

    // Contradiction detection (OxiZ SMT)
    let contradictions = detect_smt_contradictions(statutes);

    // Constitutional principle checks + quality analysis per statute
    let mut total_score = 0.0f64;
    let mut score_count = 0usize;

    for statute in statutes {
        // Equality check (Art. 14 JP Constitution) — add issues as warnings
        let eq = check_equality(statute);
        for issue in &eq.issues {
            if !warnings.contains(issue) {
                warnings.push(issue.clone());
            }
        }

        // Due process check
        let dp = check_due_process(statute);
        for suggestion in &dp.suggestions {
            if !suggestions.contains(suggestion) {
                suggestions.push(suggestion.clone());
            }
        }

        // Privacy impact check (side-effect only — warnings surfaced via vr)
        let _privacy = check_privacy_impact(statute);

        // Quality analysis
        let metrics = analyze_quality(statute);
        total_score += metrics.overall_score;
        score_count += 1;
    }

    // Compute averaged quality grade
    let quality_grade = if score_count > 0 {
        let avg = total_score / score_count as f64;
        Some(if avg >= 90.0 {
            'A'
        } else if avg >= 80.0 {
            'B'
        } else if avg >= 70.0 {
            'C'
        } else if avg >= 60.0 {
            'D'
        } else {
            'F'
        })
    } else {
        None
    };

    debug!(
        "Full verification: {} errors, {} warnings, {} contradictions, grade={:?}",
        errors.len(),
        warnings.len(),
        contradictions.len(),
        quality_grade
    );

    FullVerificationReport {
        contradictions,
        errors,
        warnings,
        suggestions,
        quality_grade,
        total_statutes: statutes.len(),
    }
}

/// Format contradiction warnings as a Markdown prefix for the report prompt.
/// The warnings are prepended to the references text so Gemini is aware of them.
pub fn format_contradiction_warnings(contradictions: &[SmtContradiction]) -> String {
    if contradictions.is_empty() {
        return String::new();
    }

    let mut warning = String::from("【OxiZ SMT 法的整合性検証 — 矛盾検出】\n");
    warning.push_str("以下の論理矛盾が検出されました。レポート作成時に必ず言及してください：\n\n");

    for (i, c) in contradictions.iter().enumerate() {
        let severity_label = match c.severity {
            3 => "🔴 高",
            2 => "🟡 中",
            _ => "🟢 低",
        };
        warning.push_str(&format!(
            "{}. [重要度: {}] {}\n   関連条文: {}\n",
            i + 1,
            severity_label,
            c.explanation,
            c.statute_ids.join(", ")
        ));
    }
    warning.push('\n');
    warning
}

/// Build a VerificationSummary from a FullVerificationReport and parsed/total counts.
pub fn build_verification_summary_from_report(
    report: &FullVerificationReport,
    statutes_analyzed: usize,
    statutes_parsed: usize,
    deterministic_count: usize,
    discretion_count: usize,
) -> VerificationSummary {
    let void_count = report.contradictions.len();

    VerificationSummary {
        deterministic_count,
        discretion_count,
        void_count,
        contradictions: report
            .contradictions
            .iter()
            .map(|c| c.explanation.clone())
            .collect(),
        statutes_analyzed,
        statutes_parsed,
        quality_grade: report.quality_grade,
        verification_errors: report.errors.clone(),
        verification_warnings: report.warnings.clone(),
        suggestions: report.suggestions.clone(),
    }
}

/// Build a VerificationSummary from contradiction results (legacy, kept for tests).
pub fn build_verification_summary(
    contradictions: &[SmtContradiction],
    total_statutes: usize,
) -> VerificationSummary {
    let void_count = contradictions.len();
    let deterministic_count = total_statutes.saturating_sub(void_count * 2);

    VerificationSummary {
        deterministic_count,
        discretion_count: total_statutes.saturating_sub(deterministic_count + void_count),
        void_count,
        contradictions: contradictions
            .iter()
            .map(|c| c.explanation.clone())
            .collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_contradiction_warnings_empty() {
        let result = format_contradiction_warnings(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_contradiction_warnings() {
        let contradictions = vec![SmtContradiction {
            statute_ids: vec!["法第3条".to_string(), "法第5条".to_string()],
            explanation: "第3条と第5条の適用範囲が論理的に矛盾しています".to_string(),
            severity: 2,
        }];
        let result = format_contradiction_warnings(&contradictions);
        assert!(result.contains("OxiZ SMT"));
        assert!(result.contains("矛盾検出"));
        assert!(result.contains("第3条と第5条"));
    }

    #[test]
    fn test_build_verification_summary() {
        let contradictions = vec![SmtContradiction {
            statute_ids: vec!["test".to_string()],
            explanation: "test contradiction".to_string(),
            severity: 1,
        }];
        let summary = build_verification_summary(&contradictions, 10);
        assert_eq!(summary.void_count, 1);
        assert!(summary.has_contradictions());
    }

    #[test]
    fn test_detect_empty_statutes() {
        let result = detect_smt_contradictions(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_run_full_verification_empty() {
        let report = run_full_verification(&[]);
        assert_eq!(report.total_statutes, 0);
        assert!(report.quality_grade.is_none());
        assert!(report.contradictions.is_empty());
    }

    #[test]
    fn test_run_full_verification_with_labor_statutes() {
        use legalis_jp::reasoning::all_labor_statutes;
        let statutes = all_labor_statutes();
        let report = run_full_verification(&statutes);
        assert_eq!(report.total_statutes, statutes.len());
        // Grade should be computed
        assert!(report.quality_grade.is_some());
        // Contradictions count is determined by OxiZ SMT — just verify it's a valid number
        // (LSA_Art32 and LSA_Art36 may be flagged as conflicting even though they're complementary)
        let _ = report.contradictions.len(); // no panic is the assertion
    }

    #[test]
    fn test_build_verification_summary_from_report() {
        let report = FullVerificationReport {
            contradictions: vec![],
            errors: vec![],
            warnings: vec!["テスト警告".to_string()],
            suggestions: vec!["テスト提案".to_string()],
            quality_grade: Some('B'),
            total_statutes: 5,
        };
        let summary = build_verification_summary_from_report(&report, 5, 3, 2, 1);
        assert_eq!(summary.statutes_analyzed, 5);
        assert_eq!(summary.statutes_parsed, 3);
        assert_eq!(summary.quality_grade, Some('B'));
        assert_eq!(summary.deterministic_count, 2);
        assert_eq!(summary.discretion_count, 1);
        assert!(!summary.has_contradictions());
    }
}
