use legalis_core::LegalResult;
use serde::{Deserialize, Serialize};

/// Classification of a report section based on legal determinism.
/// Maps to legalis-core::`LegalResult<T>` semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SectionClassification {
    /// Mechanically derivable from statute text (e.g. age >= 18 → adult)
    Deterministic,
    /// Requires human judicial interpretation (e.g. "just cause", "reasonable")
    JudicialDiscretion,
    /// Contains logical contradiction detected by OxiZ SMT solver
    Void,
}

impl SectionClassification {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Deterministic => "✅ 決定論的",
            Self::JudicialDiscretion => "⚖️ 司法裁量",
            Self::Void => "❌ 論理矛盾",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Deterministic => "条文から機械的に導出可能",
            Self::JudicialDiscretion => "人間の解釈・裁量が必要",
            Self::Void => "OxiZ SMT ソルバーが論理矛盾を検出",
        }
    }
}

/// Connect legalis-core `LegalResult<T>` directly to SectionClassification.
impl<T> From<&LegalResult<T>> for SectionClassification {
    fn from(result: &LegalResult<T>) -> Self {
        match result {
            LegalResult::Deterministic(_) => Self::Deterministic,
            LegalResult::JudicialDiscretion { .. } => Self::JudicialDiscretion,
            LegalResult::Void { .. } => Self::Void,
        }
    }
}

/// A report section with its legal determinism classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedSection {
    /// Section heading
    pub heading: String,
    /// Section content
    pub content: String,
    /// Determinism classification from Legalis-RS
    pub classification: SectionClassification,
    /// Related statute identifiers (law_num or article unique_anchor)
    pub related_statutes: Vec<String>,
}

/// Summary of verification results for the entire report.
#[derive(Debug, Clone, Default)]
pub struct VerificationSummary {
    /// Sections classified as deterministic
    pub deterministic_count: usize,
    /// Sections requiring judicial discretion
    pub discretion_count: usize,
    /// Sections with detected contradictions
    pub void_count: usize,
    /// Detected contradiction descriptions
    pub contradictions: Vec<String>,
    /// Total statutes submitted to the verifier
    pub statutes_analyzed: usize,
    /// Statutes successfully parsed (DSL or domain-matched)
    pub statutes_parsed: usize,
    /// Overall quality grade (A-F) from legalis-verifier QualityMetrics
    pub quality_grade: Option<char>,
    /// Verification errors from StatuteVerifier::verify()
    pub verification_errors: Vec<String>,
    /// Verification warnings from StatuteVerifier::verify()
    pub verification_warnings: Vec<String>,
    /// Suggestions from StatuteVerifier::verify()
    pub suggestions: Vec<String>,
}

impl VerificationSummary {
    pub fn has_contradictions(&self) -> bool {
        self.void_count > 0 || !self.contradictions.is_empty()
    }

    /// Formats the verification summary as a Markdown section.
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("## 法的整合性検証\n\n");

        // Grade badge
        if let Some(grade) = self.quality_grade {
            md.push_str(&format!("**品質グレード: {}** ", grade));
            let bar = match grade {
                'A' => "🟢🟢🟢🟢🟢",
                'B' => "🟢🟢🟢🟢⚪",
                'C' => "🟡🟡🟡⚪⚪",
                'D' => "🟠🟠⚪⚪⚪",
                _ => "🔴⚪⚪⚪⚪",
            };
            md.push_str(bar);
            md.push_str("\n\n");
        }

        // Parse stats
        if self.statutes_analyzed > 0 {
            md.push_str(&format!(
                "条文解析: **{}件** 中 **{}件** 形式化成功\n\n",
                self.statutes_analyzed, self.statutes_parsed
            ));
        }

        // Contradiction / clean result
        if self.contradictions.is_empty() && self.void_count == 0 {
            md.push_str("✅ **OxiZ SMT 検証済み** — 論理矛盾は検出されませんでした。\n\n");
        } else {
            md.push_str("❌ **論理矛盾が検出されました**\n\n");
            for (i, c) in self.contradictions.iter().enumerate() {
                md.push_str(&format!("{}. {}\n", i + 1, c));
            }
            md.push('\n');
        }

        // Verification errors
        if !self.verification_errors.is_empty() {
            md.push_str("### 検証エラー\n");
            for e in &self.verification_errors {
                md.push_str(&format!("- ⚠️ {}\n", e));
            }
            md.push('\n');
        }

        // Warnings
        if !self.verification_warnings.is_empty() {
            md.push_str("### 注意事項\n");
            for w in &self.verification_warnings {
                md.push_str(&format!("- 💡 {}\n", w));
            }
            md.push('\n');
        }

        // Suggestions
        if !self.suggestions.is_empty() {
            md.push_str("### 改善提案\n");
            for s in &self.suggestions {
                md.push_str(&format!("- 📌 {}\n", s));
            }
            md.push('\n');
        }

        // Classification table
        md.push_str(&format!(
            "| 分類 | セクション数 |\n|------|----------|\n| {} | {} |\n| {} | {} |\n| {} | {} |\n",
            SectionClassification::Deterministic.label(),
            self.deterministic_count,
            SectionClassification::JudicialDiscretion.label(),
            self.discretion_count,
            SectionClassification::Void.label(),
            self.void_count,
        ));

        md.push_str("\n*Powered by Legalis-RS + OxiZ SMT Solver*\n");
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legalis_core::Uuid;

    #[test]
    fn test_classification_labels() {
        assert!(SectionClassification::Deterministic.label().contains("✅"));
        assert!(SectionClassification::Void.label().contains("❌"));
        assert!(
            SectionClassification::JudicialDiscretion
                .label()
                .contains("⚖️")
        );
    }

    #[test]
    fn test_from_legal_result_deterministic() {
        let r: LegalResult<i32> = LegalResult::Deterministic(42);
        assert_eq!(
            SectionClassification::from(&r),
            SectionClassification::Deterministic
        );
    }

    #[test]
    fn test_from_legal_result_discretion() {
        let r: LegalResult<i32> = LegalResult::JudicialDiscretion {
            issue: "interpretation required".into(),
            context_id: Uuid::new_v4(),
            narrative_hint: None,
        };
        assert_eq!(
            SectionClassification::from(&r),
            SectionClassification::JudicialDiscretion
        );
    }

    #[test]
    fn test_from_legal_result_void() {
        let r: LegalResult<i32> = LegalResult::Void {
            reason: "contradiction".into(),
        };
        assert_eq!(SectionClassification::from(&r), SectionClassification::Void);
    }

    #[test]
    fn test_verification_summary_no_contradictions() {
        let summary = VerificationSummary {
            deterministic_count: 3,
            discretion_count: 1,
            statutes_analyzed: 5,
            statutes_parsed: 4,
            quality_grade: Some('A'),
            ..Default::default()
        };
        assert!(!summary.has_contradictions());
        let md = summary.to_markdown();
        assert!(md.contains("矛盾は検出されませんでした"));
        assert!(md.contains("品質グレード: A"));
        assert!(md.contains("5件"));
    }

    #[test]
    fn test_verification_summary_with_contradictions() {
        let summary = VerificationSummary {
            deterministic_count: 2,
            void_count: 1,
            contradictions: vec!["第3条と第5条の適用範囲が矛盾しています".to_string()],
            verification_errors: vec!["循環参照が検出されました".to_string()],
            ..Default::default()
        };
        assert!(summary.has_contradictions());
        let md = summary.to_markdown();
        assert!(md.contains("論理矛盾が検出されました"));
        assert!(md.contains("循環参照"));
    }
}
