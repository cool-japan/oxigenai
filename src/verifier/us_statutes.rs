/// Manually-defined Legalis `Statute` objects for United States federal law.
///
/// This is the US seed set used by `UsMatcher` in the multi-jurisdiction layer.
/// Like `jp_statutes.rs`, each function returns a single SMT-verifiable `Statute`
/// built with `legalis_core`, tagged `.with_jurisdiction("US")`.
///
/// Sources: Fair Labor Standards Act (29 U.S.C. §§ 206–207), Americans with
/// Disabilities Act (42 U.S.C. § 12112), Family and Medical Leave Act
/// (29 U.S.C. § 2612).
///
/// Naming convention for IDs:
///   FLSA_SecN — Fair Labor Standards Act, 29 U.S.C. § N
///   ADA_SecN  — Americans with Disabilities Act, 42 U.S.C. § N
///   FMLA_SecN — Family and Medical Leave Act, 29 U.S.C. § N
use legalis_core::{ComparisonOp, Condition, DurationUnit, Effect, EffectType, Statute};

/// Jurisdiction code applied to every US seed statute.
const US: &str = "US";

// ─── Fair Labor Standards Act (FLSA) ─────────────────────────────────────────────

/// 29 U.S.C. §206 — Minimum wage.
///
/// A covered, non-exempt employee must be paid at least the federal minimum
/// wage for all hours worked.
///
/// Modeled precondition: `employee_classification == "non_exempt"`.
#[must_use]
pub fn flsa_section_206_minimum_wage() -> Statute {
    Statute::new(
        "FLSA_Sec206",
        "連邦最低賃金 / Federal Minimum Wage (FLSA, 29 U.S.C. §206)",
        Effect::new(
            EffectType::Obligation,
            "使用者は対象となる非適用除外労働者に対し、労働時間につき連邦最低賃金以上を支払わなければならない \
             / An employer must pay each covered non-exempt employee at least the federal minimum wage \
             for all hours worked",
        )
        .with_parameter("federal_minimum_wage_usd", "7.25"),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "employee_classification".to_string(),
        value: "non_exempt".to_string(),
    })
    .with_jurisdiction(US)
}

/// 29 U.S.C. §207 — Maximum hours / overtime compensation.
///
/// Hours worked beyond 40 in a workweek must be compensated at one and one-half
/// times the regular rate of pay.
///
/// Modeled preconditions (conjunction — both must hold):
/// `employee_classification == "non_exempt"` AND `weekly_hours > 40`.
/// (`Threshold` is used rather than `Duration` because `DurationUnit` has no
/// `Hours` variant — see `legalis_core::DurationUnit`.)
#[must_use]
pub fn flsa_section_207_overtime() -> Statute {
    Statute::new(
        "FLSA_Sec207",
        "時間外労働割増賃金 / Overtime Compensation (FLSA, 29 U.S.C. §207)",
        Effect::new(
            EffectType::Obligation,
            "使用者は1週間に40時間を超えて労働させた非適用除外労働者に対し、 \
             通常賃金の1.5倍の割増賃金を支払わなければならない \
             / An employer must compensate a non-exempt employee at one and one-half times the regular \
             rate for hours worked in excess of 40 in a workweek",
        )
        .with_parameter("overtime_multiplier", "1.5")
        .with_parameter("weekly_threshold_hours", "40"),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "employee_classification".to_string(),
        value: "non_exempt".to_string(),
    })
    .with_precondition(Condition::Threshold {
        attributes: vec![("weekly_hours".to_string(), 1.0)],
        operator: ComparisonOp::GreaterThan,
        value: 40.0,
    })
    .with_jurisdiction(US)
}

// ─── Americans with Disabilities Act (ADA) ───────────────────────────────────────

/// 42 U.S.C. §12112 — Discrimination prohibited (ADA Title I).
///
/// A covered employer must not discriminate against a qualified individual on
/// the basis of disability in any term or condition of employment.
///
/// Modeled preconditions (conjunction): `employee_count >= 15` (the structured
/// "covered employer" size threshold) AND a retained `Custom` condition for
/// the qualitative "qualified individual with a disability, employment
/// decision" judgment — this statute stays partly discretionary by design,
/// since whether a given decision is disability-based discrimination is not
/// mechanically decidable from attributes alone.
#[must_use]
pub fn ada_section_12112_nondiscrimination() -> Statute {
    Statute::new(
        "ADA_Sec12112",
        "障害に基づく雇用差別の禁止 / Prohibition of Disability Discrimination (ADA, 42 U.S.C. §12112)",
        Effect::new(
            EffectType::Prohibition,
            "対象使用者は、採用・昇進・報酬その他の雇用条件において、 \
             資格を有する障害者を障害を理由に差別してはならない \
             / A covered employer must not discriminate against a qualified individual on the basis of \
             disability in hiring, advancement, compensation, or other terms of employment",
        ),
    )
    .with_precondition(Condition::Threshold {
        attributes: vec![("employee_count".to_string(), 1.0)],
        operator: ComparisonOp::GreaterOrEqual,
        value: 15.0,
    })
    .with_precondition(Condition::Custom {
        description: "資格を有する障害者に関する雇用上の決定（採用・昇進・報酬等）を行うとき \
                      / When making an employment decision (hiring, advancement, compensation, etc.) \
                      regarding a qualified individual with a disability"
            .to_string(),
    })
    .with_jurisdiction(US)
}

// ─── Family and Medical Leave Act (FMLA) ─────────────────────────────────────────

/// 29 U.S.C. §2612 — Leave requirement.
///
/// An eligible employee is entitled to 12 workweeks of unpaid, job-protected
/// leave during any 12-month period for qualifying family or medical reasons.
///
/// Modeled preconditions (conjunction): `duration >= 12 Months` (length of
/// service) AND `hours_worked_12mo >= 1250` AND a retained `Custom` condition
/// for the qualitative "qualifying family or medical event" judgment.
#[must_use]
pub fn fmla_section_2612_leave_entitlement() -> Statute {
    Statute::new(
        "FMLA_Sec2612",
        "家族・医療休暇の権利 / Family and Medical Leave Entitlement (FMLA, 29 U.S.C. §2612)",
        Effect::new(
            EffectType::Grant,
            "受給資格を有する労働者は、対象となる家族・医療上の事由について、 \
             12か月の期間内に最大12労働週の無給かつ復職保障付き休暇を取得する権利を有する \
             / An eligible employee is entitled to up to 12 workweeks of unpaid, job-protected leave \
             during any 12-month period for qualifying family or medical reasons",
        )
        .with_parameter("max_leave_weeks", "12"),
    )
    .with_precondition(Condition::Duration {
        operator: ComparisonOp::GreaterOrEqual,
        value: 12,
        unit: DurationUnit::Months,
    })
    .with_precondition(Condition::Threshold {
        attributes: vec![("hours_worked_12mo".to_string(), 1.0)],
        operator: ComparisonOp::GreaterOrEqual,
        value: 1250.0,
    })
    .with_precondition(Condition::Custom {
        description: "対象となる家族・医療上の事由が生じたとき \
                      / When a qualifying family or medical event occurs"
            .to_string(),
    })
    .with_jurisdiction(US)
}

/// Returns all US federal seed statutes for matcher use.
#[must_use]
pub fn all_us_federal_statutes() -> Vec<Statute> {
    vec![
        flsa_section_206_minimum_wage(),
        flsa_section_207_overtime(),
        ada_section_12112_nondiscrimination(),
        fmla_section_2612_leave_entitlement(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_us_statutes_jurisdiction() {
        let statutes = all_us_federal_statutes();
        assert_eq!(statutes.len(), 4);
        assert!(
            statutes
                .iter()
                .all(|s| s.jurisdiction.as_deref() == Some("US"))
        );
    }

    #[test]
    fn test_us_ids_and_effects() {
        let statutes = all_us_federal_statutes();
        // FLSA overtime — OBLIGATION with 1.5x multiplier
        let overtime = statutes.iter().find(|s| s.id == "FLSA_Sec207").unwrap();
        assert_eq!(overtime.effect.effect_type, EffectType::Obligation);
        assert_eq!(
            overtime.effect.parameters.get("overtime_multiplier"),
            Some(&"1.5".to_string())
        );
        // ADA — PROHIBITION
        let ada = statutes.iter().find(|s| s.id == "ADA_Sec12112").unwrap();
        assert_eq!(ada.effect.effect_type, EffectType::Prohibition);
        // FMLA — GRANT
        let fmla = statutes.iter().find(|s| s.id == "FMLA_Sec2612").unwrap();
        assert_eq!(fmla.effect.effect_type, EffectType::Grant);
    }

    #[test]
    fn test_flsa_206_precondition_structured() {
        let statute = flsa_section_206_minimum_wage();
        assert_eq!(statute.preconditions.len(), 1);
        assert_eq!(
            statute.preconditions[0],
            Condition::AttributeEquals {
                key: "employee_classification".to_string(),
                value: "non_exempt".to_string(),
            }
        );
    }

    #[test]
    fn test_flsa_207_preconditions_structured() {
        let statute = flsa_section_207_overtime();
        assert_eq!(statute.preconditions.len(), 2);
        assert_eq!(
            statute.preconditions[0],
            Condition::AttributeEquals {
                key: "employee_classification".to_string(),
                value: "non_exempt".to_string(),
            }
        );
        assert_eq!(
            statute.preconditions[1],
            Condition::Threshold {
                attributes: vec![("weekly_hours".to_string(), 1.0)],
                operator: ComparisonOp::GreaterThan,
                value: 40.0,
            }
        );
    }

    #[test]
    fn test_ada_12112_preconditions_structured() {
        let statute = ada_section_12112_nondiscrimination();
        assert_eq!(statute.preconditions.len(), 2);
        assert_eq!(
            statute.preconditions[0],
            Condition::Threshold {
                attributes: vec![("employee_count".to_string(), 1.0)],
                operator: ComparisonOp::GreaterOrEqual,
                value: 15.0,
            }
        );
        assert!(matches!(
            &statute.preconditions[1],
            Condition::Custom { .. }
        ));
    }

    #[test]
    fn test_fmla_2612_preconditions_structured() {
        let statute = fmla_section_2612_leave_entitlement();
        assert_eq!(statute.preconditions.len(), 3);
        assert_eq!(
            statute.preconditions[0],
            Condition::Duration {
                operator: ComparisonOp::GreaterOrEqual,
                value: 12,
                unit: DurationUnit::Months,
            }
        );
        assert_eq!(
            statute.preconditions[1],
            Condition::Threshold {
                attributes: vec![("hours_worked_12mo".to_string(), 1.0)],
                operator: ComparisonOp::GreaterOrEqual,
                value: 1250.0,
            }
        );
        assert!(matches!(
            &statute.preconditions[2],
            Condition::Custom { .. }
        ));
    }
}
