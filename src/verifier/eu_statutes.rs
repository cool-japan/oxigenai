/// Manually-defined Legalis `Statute` objects for European Union law (GDPR).
///
/// This is the EU seed set used by `EuMatcher` in the multi-jurisdiction layer.
/// Like `jp_statutes.rs`, each function returns a single SMT-verifiable `Statute`
/// built with `legalis_core`, tagged `.with_jurisdiction("EU")`.
///
/// Source: Regulation (EU) 2016/679 (General Data Protection Regulation).
///
/// Naming convention for IDs:
///   GDPR_ArtN — General Data Protection Regulation, Article N
use legalis_core::{Condition, Effect, EffectType, Statute};

/// Jurisdiction code applied to every EU seed statute.
const EU: &str = "EU";

// ─── Regulation (EU) 2016/679 — GDPR ────────────────────────────────────────────

/// Art.6 — Lawfulness of processing.
///
/// Processing is lawful only if at least one legal basis under Art.6(1) applies
/// (consent, contract, legal obligation, vital interests, public task, or
/// legitimate interests).
///
/// Modeled precondition: `lawful_basis` is a member of the six Art.6(1) bases.
#[must_use]
pub fn gdpr_article_6_lawful_processing() -> Statute {
    Statute::new(
        "GDPR_Art6",
        "適法な処理 / Lawfulness of Processing (GDPR Art. 6)",
        Effect::new(
            EffectType::Obligation,
            "個人データの処理は、同意・契約・法的義務・重要な利益・公共の利益・正当な利益のいずれかの \
             適法根拠に基づかなければならない \
             / Processing of personal data must rely on at least one lawful basis (consent, contract, \
             legal obligation, vital interests, public task, or legitimate interests)",
        ),
    )
    .with_precondition(Condition::SetMembership {
        attribute: "lawful_basis".to_string(),
        values: vec![
            "consent".to_string(),
            "contract".to_string(),
            "legal_obligation".to_string(),
            "vital_interests".to_string(),
            "public_task".to_string(),
            "legitimate_interests".to_string(),
        ],
        negated: false,
    })
    .with_jurisdiction(EU)
}

/// Art.7 — Conditions for consent.
///
/// Where processing is based on consent, the controller must be able to
/// demonstrate that consent was given, and consent may be withdrawn at any time.
#[must_use]
pub fn gdpr_article_7_consent() -> Statute {
    Statute::new(
        "GDPR_Art7",
        "同意の要件 / Conditions for Consent (GDPR Art. 7)",
        Effect::new(
            EffectType::Obligation,
            "同意を根拠とする場合、管理者は本人が同意したことを証明できなければならず、 \
             同意は自由意思により与えられ、いつでも撤回できなければならない \
             / Where processing is based on consent, the controller must be able to demonstrate consent; \
             consent must be freely given and withdrawable at any time",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "lawful_basis".to_string(),
        value: "consent".to_string(),
    })
    .with_jurisdiction(EU)
}

/// Art.15 — Right of access by the data subject.
///
/// The data subject may obtain confirmation of whether their personal data is
/// processed and access to that data together with the prescribed information.
///
/// Modeled precondition: `request_type == "access"`.
#[must_use]
pub fn gdpr_article_15_right_of_access() -> Statute {
    Statute::new(
        "GDPR_Art15",
        "アクセス権 / Right of Access by the Data Subject (GDPR Art. 15)",
        Effect::new(
            EffectType::Grant,
            "本人は自己の個人データが処理されているか否かの確認と当該データへのアクセスを求める権利を有する \
             / The data subject has the right to obtain confirmation of, and access to, their personal data",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "request_type".to_string(),
        value: "access".to_string(),
    })
    .with_jurisdiction(EU)
}

/// Art.17 — Right to erasure ('right to be forgotten').
///
/// The data subject may obtain erasure of personal data without undue delay
/// where one of the Art.17(1) grounds applies and no overriding ground for
/// retention exists.
///
/// Modeled preconditions (conjunction): `request_type == "erasure"` AND a
/// retained `Custom` condition for "no overriding ground for retention
/// exists" — whether a specific retention ground overrides erasure (e.g.
/// freedom of expression, legal compliance, public interest archiving) is a
/// balancing judgment, not a mechanically decidable fact.
#[must_use]
pub fn gdpr_article_17_right_to_erasure() -> Statute {
    Statute::new(
        "GDPR_Art17",
        "消去権（忘れられる権利） / Right to Erasure — Right to Be Forgotten (GDPR Art. 17)",
        Effect::new(
            EffectType::Grant,
            "本人は不当な遅滞なく自己の個人データの消去を管理者に求める権利を有する \
             / The data subject has the right to obtain erasure of personal data concerning them \
             without undue delay",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "request_type".to_string(),
        value: "erasure".to_string(),
    })
    .with_precondition(Condition::Custom {
        description: "保持を正当化する優越的な法的根拠が存在しないとき \
                      / When no overriding legal ground for retention applies"
            .to_string(),
    })
    .with_jurisdiction(EU)
}

/// Art.33 — Notification of a personal data breach to the supervisory authority.
///
/// On becoming aware of a breach, the controller must notify the competent
/// supervisory authority without undue delay and, where feasible, within 72 hours.
///
/// Modeled precondition: `breach_occurred == "true"`.
#[must_use]
pub fn gdpr_article_33_breach_notification() -> Statute {
    Statute::new(
        "GDPR_Art33",
        "個人データ侵害の通知 / Personal Data Breach Notification (GDPR Art. 33)",
        Effect::new(
            EffectType::Obligation,
            "個人データの侵害が発生した場合、管理者は不当な遅滞なく、可能な限り72時間以内に \
             所轄監督機関へ通知しなければならない \
             / In the case of a personal data breach, the controller must notify the competent supervisory \
             authority without undue delay and, where feasible, within 72 hours",
        )
        .with_parameter("max_notification_hours", "72"),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "breach_occurred".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction(EU)
}

/// Returns all GDPR seed statutes for matcher use.
#[must_use]
pub fn all_gdpr_statutes() -> Vec<Statute> {
    vec![
        gdpr_article_6_lawful_processing(),
        gdpr_article_7_consent(),
        gdpr_article_15_right_of_access(),
        gdpr_article_17_right_to_erasure(),
        gdpr_article_33_breach_notification(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_gdpr_statutes_jurisdiction() {
        let statutes = all_gdpr_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(
            statutes
                .iter()
                .all(|s| s.jurisdiction.as_deref() == Some("EU"))
        );
    }

    #[test]
    fn test_gdpr_ids_and_effects() {
        let statutes = all_gdpr_statutes();
        assert!(statutes.iter().any(|s| s.id == "GDPR_Art6"));
        assert!(statutes.iter().any(|s| s.id == "GDPR_Art17"));
        // Art.17 is a GRANT (right to be forgotten)
        let erasure = statutes.iter().find(|s| s.id == "GDPR_Art17").unwrap();
        assert_eq!(erasure.effect.effect_type, EffectType::Grant);
        // Art.33 is an OBLIGATION carrying the 72-hour parameter
        let breach = statutes.iter().find(|s| s.id == "GDPR_Art33").unwrap();
        assert_eq!(breach.effect.effect_type, EffectType::Obligation);
        assert_eq!(
            breach.effect.parameters.get("max_notification_hours"),
            Some(&"72".to_string())
        );
    }

    #[test]
    fn test_gdpr_art6_precondition_structured() {
        let statute = gdpr_article_6_lawful_processing();
        assert_eq!(statute.preconditions.len(), 1);
        assert_eq!(
            statute.preconditions[0],
            Condition::SetMembership {
                attribute: "lawful_basis".to_string(),
                values: vec![
                    "consent".to_string(),
                    "contract".to_string(),
                    "legal_obligation".to_string(),
                    "vital_interests".to_string(),
                    "public_task".to_string(),
                    "legitimate_interests".to_string(),
                ],
                negated: false,
            }
        );
    }

    #[test]
    fn test_gdpr_art7_precondition_unchanged() {
        // Art.7 was already structured prior to this task; confirm it stays that way.
        let statute = gdpr_article_7_consent();
        assert_eq!(statute.preconditions.len(), 1);
        assert_eq!(
            statute.preconditions[0],
            Condition::AttributeEquals {
                key: "lawful_basis".to_string(),
                value: "consent".to_string(),
            }
        );
    }

    #[test]
    fn test_gdpr_art15_precondition_structured() {
        let statute = gdpr_article_15_right_of_access();
        assert_eq!(statute.preconditions.len(), 1);
        assert_eq!(
            statute.preconditions[0],
            Condition::AttributeEquals {
                key: "request_type".to_string(),
                value: "access".to_string(),
            }
        );
    }

    #[test]
    fn test_gdpr_art17_preconditions_structured() {
        let statute = gdpr_article_17_right_to_erasure();
        assert_eq!(statute.preconditions.len(), 2);
        assert_eq!(
            statute.preconditions[0],
            Condition::AttributeEquals {
                key: "request_type".to_string(),
                value: "erasure".to_string(),
            }
        );
        assert!(matches!(
            &statute.preconditions[1],
            Condition::Custom { .. }
        ));
    }

    #[test]
    fn test_gdpr_art33_precondition_structured() {
        let statute = gdpr_article_33_breach_notification();
        assert_eq!(statute.preconditions.len(), 1);
        assert_eq!(
            statute.preconditions[0],
            Condition::AttributeEquals {
                key: "breach_occurred".to_string(),
                value: "true".to_string(),
            }
        );
    }
}
