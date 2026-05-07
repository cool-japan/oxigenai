use crate::models::law::FullArticle;
use crate::verifier::dsl_bridge::StatuteBridge;
use legalis_core::{AttributeBasedContext, EntailmentEngine, LegalResult, Uuid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User-supplied facts for statute evaluation.
///
/// These represent facts about a person or situation that the statutes will be
/// evaluated against. Translates to `AttributeBasedContext` for legalis-core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFacts {
    /// Applicant's age in years (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<u32>,
    /// Annual income in JPY (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub income: Option<u64>,
    /// Additional key-value attributes (e.g. employment_type, years_employed).
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    /// Free-form Japanese description of the situation.
    pub description: String,
}

impl UserFacts {
    /// Build an `AttributeBasedContext` from this facts object.
    /// Numeric fields are stringified so they can be used with `AttributeEquals` conditions.
    #[must_use]
    pub fn to_context(&self) -> AttributeBasedContext {
        let mut attrs = self.attributes.clone();

        if let Some(age) = self.age {
            attrs.insert("age".to_string(), age.to_string());
            // Japanese legal aliases
            attrs.insert("年齢".to_string(), age.to_string());
        }
        if let Some(income) = self.income {
            attrs.insert("income".to_string(), income.to_string());
            attrs.insert("base_wage".to_string(), income.to_string());
            attrs.insert("所得".to_string(), income.to_string());
        }
        // Duration proxies from attributes (e.g. years_employed → months for LCA_Art18)
        if let Some(years) = attrs
            .get("years_employed")
            .and_then(|v| v.parse::<u32>().ok())
        {
            let months = years * 12;
            attrs.insert("duration_months".to_string(), months.to_string());
        }

        attrs.insert("description".to_string(), self.description.clone());
        attrs.insert("jurisdiction".to_string(), "JP".to_string());

        AttributeBasedContext::new(attrs)
    }
}

/// Result of evaluating a single statute against user facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactEvaluation {
    /// Statute ID (e.g. "LCA_Art18", "LSA_Art32").
    pub statute_id: String,
    /// Human-readable statute title.
    pub statute_title: String,
    /// Result type: "deterministic", "judicial_discretion", or "void".
    pub result_type: String,
    /// The legal effect description (present when applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    /// Human-readable explanation of the result.
    pub explanation: String,
    /// Whether this statute's preconditions are satisfied by the facts.
    pub applies: bool,
}

impl FactEvaluation {
    fn from_legal_result(
        statute_id: String,
        statute_title: String,
        result: &LegalResult<String>,
    ) -> Self {
        match result {
            LegalResult::Deterministic(effect) => Self {
                statute_id,
                statute_title,
                result_type: "deterministic".to_string(),
                effect: Some(effect.clone()),
                explanation: "条文の適用条件が充足されています。機械的に適用されます。".to_string(),
                applies: true,
            },
            LegalResult::JudicialDiscretion { issue, .. } => Self {
                statute_id,
                statute_title,
                result_type: "judicial_discretion".to_string(),
                effect: None,
                explanation: format!("適用判断には人間の裁量が必要です: {}", issue),
                applies: false,
            },
            LegalResult::Void { reason } => Self {
                statute_id,
                statute_title,
                result_type: "void".to_string(),
                effect: None,
                explanation: format!("OxiZ SMT ソルバーが論理矛盾を検出しました: {}", reason),
                applies: false,
            },
        }
    }
}

/// Service for evaluating legal statutes against user-supplied facts.
pub struct FormalizeService {
    bridge: StatuteBridge,
}

impl Default for FormalizeService {
    fn default() -> Self {
        Self::new()
    }
}

impl FormalizeService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bridge: StatuteBridge::new(),
        }
    }

    /// Evaluate the given articles against user facts.
    ///
    /// Uses domain-only conversion for speed (no Gemini call).
    /// Returns one `FactEvaluation` per statute that was evaluated.
    #[must_use]
    pub fn evaluate(&self, articles: &[FullArticle], facts: &UserFacts) -> Vec<FactEvaluation> {
        // Convert articles → statutes (domain-only, no async Gemini needed)
        let article_statutes = StatuteBridge::convert_articles_domain_only(articles);

        if article_statutes.is_empty() {
            return vec![];
        }

        let context = facts.to_context();

        // Deduplicate statutes by ID
        let mut seen_ids = std::collections::HashSet::new();
        let statutes: Vec<legalis_core::Statute> = article_statutes
            .into_iter()
            .filter_map(|a| {
                if seen_ids.insert(a.statute.id.clone()) {
                    Some(a.statute)
                } else {
                    None
                }
            })
            .collect();

        let engine = EntailmentEngine::new(statutes.clone());
        let entailment_results = engine.entail(&context);

        // Convert entailment results to LegalResult<String> → FactEvaluation
        entailment_results
            .into_iter()
            .zip(statutes.iter())
            .map(|(entailment, statute)| {
                let legal_result = entailment_to_legal_result(&entailment, statute);
                FactEvaluation::from_legal_result(
                    entailment.statute_id.clone(),
                    statute.title.clone(),
                    &legal_result,
                )
            })
            .collect()
    }
}

/// Convert an `EntailmentResult` to `LegalResult<String>`.
///
/// - `conditions_satisfied == true`  → `Deterministic(effect.description)`
/// - Has evaluation errors            → `Void { reason }`
/// - Conditions not satisfied         → `JudicialDiscretion` (needs human judgment)
fn entailment_to_legal_result(
    entailment: &legalis_core::types::EntailmentResult,
    statute: &legalis_core::Statute,
) -> LegalResult<String> {
    if !entailment.errors.is_empty() {
        return LegalResult::Void {
            reason: entailment.errors.join("; "),
        };
    }

    if entailment.conditions_satisfied {
        LegalResult::Deterministic(entailment.effect.description.clone())
    } else {
        // Preconditions not satisfied — this requires human judgment about applicability
        let issue = if statute.discretion_logic.is_some() {
            format!(
                "条文の適用条件（{}）が充足されていません。裁量的判断が必要です。",
                statute.title
            )
        } else {
            format!(
                "条文「{}」の適用条件が現在の事実では充足されません。",
                statute.title
            )
        };

        LegalResult::JudicialDiscretion {
            issue,
            context_id: Uuid::new_v4(),
            narrative_hint: Some(format!(
                "追加の事実確認または専門家による解釈が必要: {}",
                entailment.effect.description
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_facts(employment_type: &str, years: u32) -> UserFacts {
        let mut attrs = HashMap::new();
        attrs.insert("employment_type".to_string(), employment_type.to_string());
        attrs.insert("years_employed".to_string(), years.to_string());

        UserFacts {
            age: Some(35),
            income: Some(300_000),
            attributes: attrs,
            description: format!("{}年間有期雇用契約を更新してきた労働者", years),
        }
    }

    #[test]
    fn test_user_facts_to_context() {
        let facts = sample_facts("fixed_term", 6);
        let ctx = facts.to_context();
        assert_eq!(
            ctx.attributes.get("employment_type").map(|s| s.as_str()),
            Some("fixed_term")
        );
        assert_eq!(ctx.attributes.get("age").map(|s| s.as_str()), Some("35"));
        // 6 years → 72 months
        assert_eq!(
            ctx.attributes.get("duration_months").map(|s| s.as_str()),
            Some("72")
        );
    }

    #[test]
    fn test_formalize_service_labor_articles() {
        let service = FormalizeService::new();

        let articles = vec![FullArticle {
            law_id: "test_labor".to_string(),
            title: "労働基準法 第32条".to_string(),
            content: "使用者は、労働者に、休憩時間を除き一週間について四十時間を超えて、労働させてはならない。".to_string(),
            unique_anchor: "Article_32".to_string(),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/320AC0000000049".to_string(),
        }];

        let facts = sample_facts("fixed_term", 6);
        let evaluations = service.evaluate(&articles, &facts);

        // Should have evaluations (from domain-matched labor statutes)
        assert!(!evaluations.is_empty());
        // All should have statute IDs
        for eval in &evaluations {
            assert!(!eval.statute_id.is_empty());
            assert!(!eval.statute_title.is_empty());
        }
    }

    #[test]
    fn test_formalize_no_domain_match() {
        let service = FormalizeService::new();

        let articles = vec![FullArticle {
            law_id: "copyright".to_string(),
            title: "著作権法 第1条".to_string(),
            content: "この法律は著作物の利用に関して著作者の権利を定める。".to_string(),
            unique_anchor: "Article_1".to_string(),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/345AC0000000048".to_string(),
        }];

        let facts = UserFacts {
            age: None,
            income: None,
            attributes: HashMap::new(),
            description: "著作権に関する問題".to_string(),
        };

        let evaluations = service.evaluate(&articles, &facts);
        // Fallback statutes have Condition::Custom which always errors on attribute check
        // Result could be empty or have void evaluations
        for eval in &evaluations {
            // Must be one of the valid result types
            assert!(
                ["deterministic", "judicial_discretion", "void"]
                    .contains(&eval.result_type.as_str())
            );
        }
    }

    #[test]
    fn test_lca_art18_indefinite_conversion() {
        use crate::models::law::FullArticle;

        let service = FormalizeService::new();

        let articles = vec![FullArticle {
            law_id: "labor_contract".to_string(),
            title: "労働契約法 第18条".to_string(),
            content: "有期労働契約が通算して五年を超えて反復更新された場合、労働者の申込みにより、期間の定めのない労働契約に転換する。".to_string(),
            unique_anchor: "Article_18".to_string(),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/419AC0000000128".to_string(),
        }];

        // 6 years of fixed-term employment → should satisfy LCA_Art18 conditions
        let facts_6yr = sample_facts("fixed_term", 6);
        let evals_6yr = service.evaluate(&articles, &facts_6yr);

        // Find LCA_Art18 evaluation
        let lca_eval = evals_6yr.iter().find(|e| e.statute_id == "LCA_Art18");
        assert!(lca_eval.is_some(), "LCA_Art18 should be in evaluations");
        if let Some(eval) = lca_eval {
            // 6 years = 72 months ≥ 60 months AND employment_type=fixed_term
            // → Deterministic (both conditions met)
            assert_eq!(eval.result_type, "deterministic");
            assert!(eval.applies);
        }
    }
}
