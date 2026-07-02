use crate::models::law::FullArticle;
use crate::verifier::dsl_bridge::StatuteBridge;
use crate::verifier::jurisdiction::DEFAULT_JURISDICTION;
use legalis_core::{AttributeBasedContext, EvaluationError, LegalResult, Uuid};
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
    /// Build an `AttributeBasedContext` from this facts object (jurisdiction `"JP"`).
    /// Numeric fields are stringified so they can be used with `AttributeEquals` conditions.
    #[must_use]
    pub fn to_context(&self) -> AttributeBasedContext {
        self.to_context_for(DEFAULT_JURISDICTION)
    }

    /// Build an `AttributeBasedContext`, tagging the `jurisdiction` attribute.
    /// Numeric fields are stringified so they can be used with `AttributeEquals` conditions.
    #[must_use]
    pub fn to_context_for(&self, jurisdiction: &str) -> AttributeBasedContext {
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
        attrs.insert("jurisdiction".to_string(), jurisdiction.to_string());

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

    /// Evaluate the given articles against user facts (jurisdiction `"JP"`).
    ///
    /// Thin wrapper over [`Self::evaluate_for`] preserving the JP default.
    #[must_use]
    pub fn evaluate(&self, articles: &[FullArticle], facts: &UserFacts) -> Vec<FactEvaluation> {
        self.evaluate_for(articles, facts, DEFAULT_JURISDICTION)
    }

    /// Evaluate the given articles against user facts for `jurisdiction`.
    ///
    /// Uses jurisdiction-aware domain-only conversion for speed (no Gemini call).
    /// Returns one `FactEvaluation` per statute that was evaluated.
    #[must_use]
    pub fn evaluate_for(
        &self,
        articles: &[FullArticle],
        facts: &UserFacts,
        jurisdiction: &str,
    ) -> Vec<FactEvaluation> {
        // Convert articles → statutes (domain-only, no async Gemini needed)
        let article_statutes =
            StatuteBridge::convert_articles_domain_only_for(articles, jurisdiction);

        if article_statutes.is_empty() {
            return vec![];
        }

        let context = facts.to_context_for(jurisdiction);

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

        // Evaluate each statute's preconditions directly via the inherent, trait-bounded
        // `Condition::evaluate::<AttributeBasedContext>` — NOT `EntailmentEngine::entail`
        // (which internally uses `Condition::evaluate_simple`, whose catch-all `_ => Ok(true)`
        // trivially "satisfies" structured `Duration`/`Threshold`/`SetMembership`/`Custom`
        // conditions regardless of facts). See `statute_to_legal_result` for the aggregation
        // policy that fixes this.
        statutes
            .iter()
            .map(|statute| {
                let legal_result = statute_to_legal_result(statute, &context);
                FactEvaluation::from_legal_result(
                    statute.id.clone(),
                    statute.title.clone(),
                    &legal_result,
                )
            })
            .collect()
    }
}

/// Evaluate all of a statute's preconditions (the `Vec<Condition>` is an implicit AND)
/// against `context` using `Condition::evaluate`, and aggregate into a `LegalResult<String>`.
///
/// Aggregation policy:
/// - Zero preconditions, or all evaluate `Ok(true)`      → `Deterministic(effect.description)`
/// - No errors, but at least one `Ok(false)`             → `JudicialDiscretion`
/// - At least one `Err(EvaluationError::Custom { .. })`  → `JudicialDiscretion`
///   (a genuinely qualitative/discretionary precondition — this is the semantic fix:
///   such a condition can never be mechanically "satisfied", so it must never make a
///   statute `Deterministic`)
/// - At least one `Err(_)` of any other kind (e.g. missing attribute/context data)
///   → also `JudicialDiscretion`
///
/// `Void` is intentionally never constructed here — it is reserved exclusively for the
/// OxiZ SMT contradiction detector (`contradiction.rs::detect_smt_contradictions`), which
/// is a completely separate code path from per-fact statute evaluation.
fn statute_to_legal_result(
    statute: &legalis_core::Statute,
    context: &AttributeBasedContext,
) -> LegalResult<String> {
    let mut all_satisfied = true;
    let mut saw_custom_error = false;
    let mut saw_other_error = false;

    for condition in &statute.preconditions {
        match condition.evaluate(context) {
            Ok(true) => {}
            Ok(false) => all_satisfied = false,
            Err(EvaluationError::Custom { .. }) => {
                all_satisfied = false;
                saw_custom_error = true;
            }
            Err(_) => {
                all_satisfied = false;
                saw_other_error = true;
            }
        }
    }

    if all_satisfied {
        return LegalResult::Deterministic(statute.effect.description.clone());
    }

    // Preconditions not (fully) satisfied — this requires human judgment about applicability.
    let issue = if saw_custom_error {
        format!(
            "条文「{}」の適用には裁量的判断が必要です（機械的に決定不能な要件を含みます）。",
            statute.title
        )
    } else if saw_other_error {
        format!(
            "条文「{}」の適用条件を判断するための事実情報が不足しています。",
            statute.title
        )
    } else if statute.discretion_logic.is_some() {
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
            statute.effect.description
        )),
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

    // ─── Phase 4: Condition::evaluate aggregation fix regression tests ──────────
    //
    // These prove `/formalize` no longer trivially reports `Deterministic` for
    // statutes with structured `Threshold`/`Duration`/`SetMembership`/`Custom`
    // preconditions (the `EntailmentEngine::entail` / `evaluate_simple` catch-all
    // `_ => Ok(true)` bug — see TODO.md Phase 4, line ~153).

    fn facts_with(pairs: &[(&str, &str)], description: &str) -> UserFacts {
        let mut attrs = HashMap::new();
        for (k, v) in pairs {
            attrs.insert((*k).to_string(), (*v).to_string());
        }
        UserFacts {
            age: None,
            income: None,
            attributes: attrs,
            description: description.to_string(),
        }
    }

    fn flsa_overtime_article() -> FullArticle {
        FullArticle {
            law_id: "us_flsa".to_string(),
            title: "FLSA overtime pay".to_string(),
            content: "Overtime compensation for hours worked in excess of 40 in a workweek."
                .to_string(),
            unique_anchor: "FLSA_Sec207_Article".to_string(),
            anchor: None,
            url: "https://www.law.cornell.edu/uscode/text/29/207".to_string(),
        }
    }

    #[test]
    fn test_flsa_overtime_deterministic_when_over_40() {
        let service = FormalizeService::new();
        let articles = vec![flsa_overtime_article()];
        let facts = facts_with(
            &[
                ("employee_classification", "non_exempt"),
                ("weekly_hours", "45"),
            ],
            "週45時間労働する非適用除外従業員",
        );

        let evaluations = service.evaluate_for(&articles, &facts, "US");
        let eval = evaluations
            .iter()
            .find(|e| e.statute_id == "FLSA_Sec207")
            .expect("FLSA_Sec207 should be in evaluations");
        // employee_classification==non_exempt (true) AND weekly_hours(45) > 40 (true)
        // → both preconditions Ok(true) → Deterministic.
        assert_eq!(eval.result_type, "deterministic");
        assert!(eval.applies);
    }

    #[test]
    fn test_flsa_overtime_discretion_when_under_40() {
        let service = FormalizeService::new();
        let articles = vec![flsa_overtime_article()];
        let facts = facts_with(
            &[
                ("employee_classification", "non_exempt"),
                ("weekly_hours", "30"),
            ],
            "週30時間労働する非適用除外従業員",
        );

        let evaluations = service.evaluate_for(&articles, &facts, "US");
        let eval = evaluations
            .iter()
            .find(|e| e.statute_id == "FLSA_Sec207")
            .expect("FLSA_Sec207 should be in evaluations");
        // weekly_hours(30) > 40 is Ok(false) → JudicialDiscretion.
        // This is the single most important regression test in this suite: it proves
        // Threshold actually gates through the full public /formalize pipeline with
        // zero confounders (FLSA_Sec207 has no Custom precondition).
        assert_eq!(eval.result_type, "judicial_discretion");
        assert!(!eval.applies);
    }

    #[test]
    fn test_flsa_overtime_discretion_when_hours_missing() {
        let service = FormalizeService::new();
        let articles = vec![flsa_overtime_article()];
        // weekly_hours intentionally omitted → Threshold precondition errors
        // (MissingAttribute), which must map to JudicialDiscretion, never Void.
        let facts = facts_with(
            &[("employee_classification", "non_exempt")],
            "労働時間が未提供の非適用除外従業員",
        );

        let evaluations = service.evaluate_for(&articles, &facts, "US");
        let eval = evaluations
            .iter()
            .find(|e| e.statute_id == "FLSA_Sec207")
            .expect("FLSA_Sec207 should be in evaluations");
        assert_eq!(eval.result_type, "judicial_discretion");
        assert_ne!(eval.result_type, "void");
    }

    #[test]
    fn test_ada_employee_count_threshold() {
        use crate::verifier::us_statutes::ada_section_12112_nondiscrimination;

        let statute = ada_section_12112_nondiscrimination();
        // preconditions[0] is the structured Threshold half of the AND
        // (employee_count >= 15); preconditions[1] is a retained Custom
        // condition (see test_ada_full_pipeline_discretion_regardless_of_employee_count
        // for why the full pipeline can't be used to prove this half in isolation).
        assert_eq!(statute.preconditions.len(), 2);
        let threshold_condition = &statute.preconditions[0];

        let ctx_20 =
            facts_with(&[("employee_count", "20")], "従業員20名の使用者").to_context_for("US");
        let ctx_8 =
            facts_with(&[("employee_count", "8")], "従業員8名の使用者").to_context_for("US");

        assert_eq!(threshold_condition.evaluate(&ctx_20), Ok(true));
        assert_eq!(threshold_condition.evaluate(&ctx_8), Ok(false));
    }

    #[test]
    fn test_ada_full_pipeline_discretion_regardless_of_employee_count() {
        // ADA_Sec12112 retains a Condition::Custom precondition (the qualitative
        // "qualified individual, employment decision" judgment) alongside the
        // structured employee_count >= 15 Threshold. Through the full
        // FormalizeService pipeline this means the statute resolves to
        // JudicialDiscretion for EVERY employee_count value — arguably legally
        // correct (whether a specific decision is disability discrimination
        // always needs human judgment even once the size threshold is met).
        // This documents that result_type/applies alone cannot discriminate on
        // employee_count through the full pipeline; see
        // test_ada_employee_count_threshold for the isolated proof that the
        // Threshold half still genuinely gates.
        let service = FormalizeService::new();
        let articles = vec![FullArticle {
            law_id: "us_ada".to_string(),
            title: "ADA disability discrimination".to_string(),
            content: "Prohibition of discrimination on the basis of disability.".to_string(),
            unique_anchor: "ADA_Sec12112_Article".to_string(),
            anchor: None,
            url: "https://www.eeoc.gov/statutes/americans-disabilities-act-1990".to_string(),
        }];

        for employee_count in ["20", "8"] {
            let facts = facts_with(
                &[("employee_count", employee_count)],
                "障害者に関する雇用上の決定",
            );
            let evaluations = service.evaluate_for(&articles, &facts, "US");
            let eval = evaluations
                .iter()
                .find(|e| e.statute_id == "ADA_Sec12112")
                .expect("ADA_Sec12112 should be in evaluations");
            assert_eq!(eval.result_type, "judicial_discretion");
        }
    }

    #[test]
    fn test_fmla_dual_threshold() {
        use crate::verifier::us_statutes::fmla_section_2612_leave_entitlement;

        let statute = fmla_section_2612_leave_entitlement();
        // preconditions[0] = Duration (duration_months >= 12)
        // preconditions[1] = Threshold (hours_worked_12mo >= 1250)
        // preconditions[2] = retained Custom condition (qualifying event) — same
        // nuance as ADA_Sec12112, see test_fmla_full_pipeline_discretion_regardless_of_hours.
        assert_eq!(statute.preconditions.len(), 3);
        let duration_condition = &statute.preconditions[0];
        let hours_condition = &statute.preconditions[1];

        let ctx_ok = facts_with(
            &[("duration_months", "12"), ("hours_worked_12mo", "1500")],
            "12か月以上勤務し1500時間労働した労働者",
        )
        .to_context_for("US");
        assert_eq!(duration_condition.evaluate(&ctx_ok), Ok(true));
        assert_eq!(hours_condition.evaluate(&ctx_ok), Ok(true));

        let ctx_low_hours = facts_with(
            &[("duration_months", "12"), ("hours_worked_12mo", "1000")],
            "12か月以上勤務したが労働時間が1000時間の労働者",
        )
        .to_context_for("US");
        // 1000 < 1250 → Threshold precondition evaluates Ok(false).
        assert_eq!(hours_condition.evaluate(&ctx_low_hours), Ok(false));
        // Duration is unaffected by the hours change.
        assert_eq!(duration_condition.evaluate(&ctx_low_hours), Ok(true));
    }

    #[test]
    fn test_fmla_full_pipeline_discretion_regardless_of_hours() {
        // Same retained-Custom nuance as ADA_Sec12112: FMLA_Sec2612 always
        // resolves to JudicialDiscretion through the full pipeline regardless
        // of hours_worked_12mo, because preconditions[2] is a permanent-Err
        // Custom condition. test_fmla_dual_threshold proves the Duration/
        // Threshold halves genuinely gate in isolation.
        let service = FormalizeService::new();
        let articles = vec![FullArticle {
            law_id: "us_fmla".to_string(),
            title: "FMLA medical leave entitlement".to_string(),
            content: "Family and medical leave entitlement.".to_string(),
            unique_anchor: "FMLA_Sec2612_Article".to_string(),
            anchor: None,
            url: "https://www.dol.gov/agencies/whd/fmla".to_string(),
        }];

        for hours in ["1500", "1000"] {
            let facts = facts_with(
                &[("duration_months", "12"), ("hours_worked_12mo", hours)],
                "家族・医療休暇の対象事由",
            );
            let evaluations = service.evaluate_for(&articles, &facts, "US");
            let eval = evaluations
                .iter()
                .find(|e| e.statute_id == "FMLA_Sec2612")
                .expect("FMLA_Sec2612 should be in evaluations");
            assert_eq!(eval.result_type, "judicial_discretion");
        }
    }

    #[test]
    fn test_gdpr_breach_notification_deterministic() {
        let service = FormalizeService::new();
        let articles = vec![FullArticle {
            law_id: "eu_gdpr".to_string(),
            title: "GDPR data breach notification".to_string(),
            content: "Notification of a personal data breach to the supervisory authority."
                .to_string(),
            unique_anchor: "GDPR_Art33_Article".to_string(),
            anchor: None,
            url: "https://gdpr-info.eu/art-33-gdpr/".to_string(),
        }];
        let facts = facts_with(
            &[("breach_occurred", "true")],
            "個人データの侵害が発生した場合",
        );

        let evaluations = service.evaluate_for(&articles, &facts, "EU");
        let eval = evaluations
            .iter()
            .find(|e| e.statute_id == "GDPR_Art33")
            .expect("GDPR_Art33 should be in evaluations");
        assert_eq!(eval.result_type, "deterministic");
        assert!(eval.applies);
    }

    #[test]
    fn test_minpo_public_policy_stays_discretion() {
        let service = FormalizeService::new();
        let articles = vec![FullArticle {
            law_id: "jp_minpo".to_string(),
            title: "民法 第90条".to_string(),
            content: "公の秩序又は善良の風俗に反する法律行為は、無効とする。".to_string(),
            unique_anchor: "Minpo_Article_90".to_string(),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/129AC0000000089".to_string(),
        }];
        let facts = facts_with(&[], "公序良俗に反するとされる契約");

        // Before this fix: Minpo_Art90's sole Condition::Custom precondition was
        // trivially `Ok(true)` under `evaluate_simple`'s catch-all → incorrectly
        // "deterministic"/applies==true. After the fix: Custom always errors →
        // correctly "judicial_discretion"/applies==false.
        let evaluations = service.evaluate(&articles, &facts);
        let eval = evaluations
            .iter()
            .find(|e| e.statute_id == "Minpo_Art90")
            .expect("Minpo_Art90 should be in evaluations");
        assert_eq!(eval.result_type, "judicial_discretion");
        assert!(!eval.applies);
    }
}
