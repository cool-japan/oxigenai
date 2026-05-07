/// Manually-defined Legalis `Statute` objects for Japanese law domains not yet
/// covered by prebuilt functions in `legalis-jp`.
///
/// These allow `JpDomainMatcher` to provide SMT-verifiable statutes for common
/// queries such as 個人情報保護法, 下請法, and 消費者契約法 — instead of falling
/// back to Gemini DSL translation (which almost always fails for these laws).
///
/// Naming convention for IDs:
///   PIPA_ArtN  — 個人情報の保護に関する法律 (Personal Information Protection Act)
///   SHA_ArtN   — 下請代金支払遅延等防止法 (Subcontracting Act)
///   CCA_ArtN   — 消費者契約法 (Consumer Contract Act)
use legalis_core::{Condition, Effect, EffectType, Statute};

// ─── 個人情報の保護に関する法律 (PIPA) ─────────────────────────────────────────

/// Art.17 — 適正な取得（不正な手段による個人情報の取得禁止）
#[must_use]
pub fn pipa_article_17_lawful_acquisition() -> Statute {
    Statute::new(
        "PIPA_Art17",
        "適正な取得 / Lawful Acquisition of Personal Information (Art. 17)",
        Effect::new(
            EffectType::Prohibition,
            "偽りその他不正な手段により個人情報を取得してはならない \
             / Must not acquire personal information by deception or other unjust means",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "個人情報取扱事業者が個人情報を取得する場合 \
                      / When a personal information handling business operator acquires personal information"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.18 — 利用目的の特定・通知・公表義務
#[must_use]
pub fn pipa_article_18_purpose_specification() -> Statute {
    Statute::new(
        "PIPA_Art18",
        "利用目的の特定・通知 / Purpose Specification and Notification (Art. 18)",
        Effect::new(
            EffectType::Obligation,
            "個人情報を取得した場合は利用目的を本人に通知または公表しなければならない \
             / Must notify or publicly announce purpose of use when acquiring personal information",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "個人情報を取得したとき / Upon acquisition of personal information"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.27 — 第三者提供の制限（本人同意なき第三者提供禁止）
#[must_use]
pub fn pipa_article_27_third_party_restriction() -> Statute {
    Statute::new(
        "PIPA_Art27",
        "第三者提供の制限 / Third-Party Disclosure Restriction (Art. 27)",
        Effect::new(
            EffectType::Prohibition,
            "あらかじめ本人の同意を得ないで個人データを第三者に提供してはならない \
             / Must not provide personal data to third parties without prior consent",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "個人データを第三者に提供しようとする場合 \
                      / When intending to provide personal data to a third party"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.28 — 外国にある第三者への提供制限
#[must_use]
pub fn pipa_article_28_cross_border_transfer() -> Statute {
    Statute::new(
        "PIPA_Art28",
        "外国第三者提供の制限 / Cross-Border Transfer Restriction (Art. 28)",
        Effect::new(
            EffectType::Prohibition,
            "基準適合体制が整備されていない外国の第三者へ個人データを提供してはならない \
             / Must not transfer personal data to foreign third parties without adequate protection standards",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "外国にある第三者に個人データを提供する場合 \
                      / When providing personal data to a foreign third party"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.33 — 開示請求権（本人による保有個人データの開示請求）
#[must_use]
pub fn pipa_article_33_disclosure_right() -> Statute {
    Statute::new(
        "PIPA_Art33",
        "開示請求権 / Right to Request Disclosure (Art. 33)",
        Effect::new(
            EffectType::Grant,
            "本人は保有個人データの開示を請求できる \
             / The data subject may request disclosure of retained personal data",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "本人が自己の保有個人データの開示を求めるとき \
                      / When the data subject seeks disclosure of their personal data"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key PIPA statutes for matcher use.
#[must_use]
pub fn all_pipa_statutes() -> Vec<Statute> {
    vec![
        pipa_article_17_lawful_acquisition(),
        pipa_article_18_purpose_specification(),
        pipa_article_27_third_party_restriction(),
        pipa_article_28_cross_border_transfer(),
        pipa_article_33_disclosure_right(),
    ]
}

// ─── 下請代金支払遅延等防止法 (Subcontracting Act / SHA) ────────────────────────

/// Art.3 — 書面の交付義務
#[must_use]
pub fn sha_article_3_written_document() -> Statute {
    Statute::new(
        "SHA_Art3",
        "書面交付義務 / Written Document Obligation (Art. 3)",
        Effect::new(
            EffectType::Obligation,
            "親事業者は発注時に直ちに必要事項を記載した書面を下請事業者に交付しなければならない \
             / Principal contractor must immediately provide written document specifying required items upon ordering",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_principal_contractor".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.4(1)(ii) — 下請代金の支払期日（受領後60日以内）
#[must_use]
pub fn sha_article_4_payment_deadline() -> Statute {
    Statute::new(
        "SHA_Art4_PaymentDeadline",
        "下請代金支払期日 / Subcontract Payment Deadline (Art. 4(1)(ii))",
        Effect::new(
            EffectType::Obligation,
            "給付受領日から起算して60日以内に下請代金を支払わなければならない \
             / Must pay subcontract consideration within 60 days from the date of acceptance",
        )
        .with_parameter("max_payment_days", "60"),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_principal_contractor".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.4(1)(iii) — 下請代金の減額禁止
#[must_use]
pub fn sha_article_4_price_reduction_ban() -> Statute {
    Statute::new(
        "SHA_Art4_PriceReductionBan",
        "下請代金の減額禁止 / Prohibition of Price Reduction (Art. 4(1)(iii))",
        Effect::new(
            EffectType::Prohibition,
            "あらかじめ定めた下請代金を理由なく減額してはならない \
             / Must not reduce the predetermined subcontract consideration without justification",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_principal_contractor".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.4(1)(iv) — 返品禁止
#[must_use]
pub fn sha_article_4_return_ban() -> Statute {
    Statute::new(
        "SHA_Art4_ReturnBan",
        "返品禁止 / Prohibition of Returns (Art. 4(1)(iv))",
        Effect::new(
            EffectType::Prohibition,
            "受領した給付の目的物を理由なく返品してはならない \
             / Must not return accepted deliverables without justification",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_principal_contractor".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key SHA statutes for matcher use.
#[must_use]
pub fn all_sha_statutes() -> Vec<Statute> {
    vec![
        sha_article_3_written_document(),
        sha_article_4_payment_deadline(),
        sha_article_4_price_reduction_ban(),
        sha_article_4_return_ban(),
    ]
}

// ─── 消費者契約法 (Consumer Contract Act / CCA) ─────────────────────────────────

/// Art.4 — 不当勧誘による契約の取消権
#[must_use]
pub fn cca_article_4_right_to_rescind() -> Statute {
    Statute::new(
        "CCA_Art4",
        "不当勧誘による取消権 / Right to Rescind due to Improper Solicitation (Art. 4)",
        Effect::new(
            EffectType::Grant,
            "不実告知・断定的判断の提供・不退去・退去妨害等による勧誘で締結した契約を取り消せる \
             / Consumer may rescind contract concluded through misrepresentation, assertive judgment, \
               failure to leave, or obstruction of departure",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "entity_type".to_string(),
        value: "consumer".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.8 — 事業者の損害賠償責任を免除する条項の無効
#[must_use]
pub fn cca_article_8_liability_clause_invalidity() -> Statute {
    Statute::new(
        "CCA_Art8",
        "損害賠償責任免除条項の無効 / Invalidity of Liability Exclusion Clauses (Art. 8)",
        Effect::new(
            EffectType::Prohibition,
            "事業者の故意または重大な過失による損害賠償責任を全部免除する条項は無効 \
             / Clauses fully exempting operators from liability for intentional or grossly negligent acts are void",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "entity_type".to_string(),
        value: "business_operator".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.9 — 損害賠償額の予定等に関する条項の制限
#[must_use]
pub fn cca_article_9_penalty_clause_limit() -> Statute {
    Statute::new(
        "CCA_Art9",
        "過大違約金条項の制限 / Limitation of Excessive Penalty Clauses (Art. 9)",
        Effect::new(
            EffectType::Prohibition,
            "平均的な損害の額を超える違約金条項は超過部分が無効 \
             / Penalty clauses exceeding the average loss amount are void for the excess portion",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "entity_type".to_string(),
        value: "business_operator".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.10 — 消費者の利益を一方的に害する条項の無効
#[must_use]
pub fn cca_article_10_unfair_clause_invalidity() -> Statute {
    Statute::new(
        "CCA_Art10",
        "消費者利益侵害条項の無効 / Invalidity of Clauses Unilaterally Harming Consumer Interests (Art. 10)",
        Effect::new(
            EffectType::Prohibition,
            "任意規定の適用による場合と比べ消費者の権利を制限し義務を加重する条項は無効 \
             / Clauses restricting consumer rights or adding burdens beyond default rules are void",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "entity_type".to_string(),
        value: "business_operator".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key CCA statutes for matcher use.
#[must_use]
pub fn all_cca_statutes() -> Vec<Statute> {
    vec![
        cca_article_4_right_to_rescind(),
        cca_article_8_liability_clause_invalidity(),
        cca_article_9_penalty_clause_limit(),
        cca_article_10_unfair_clause_invalidity(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipa_statutes_ids() {
        let statutes = all_pipa_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(statutes.iter().all(|s| s.id.starts_with("PIPA_")));
        assert_eq!(statutes[0].id, "PIPA_Art17");
        assert_eq!(statutes[4].id, "PIPA_Art33");
    }

    #[test]
    fn test_sha_statutes_ids() {
        let statutes = all_sha_statutes();
        assert_eq!(statutes.len(), 4);
        assert!(statutes.iter().all(|s| s.id.starts_with("SHA_")));
        // Payment deadline must have the 60-day parameter
        let deadline = sha_article_4_payment_deadline();
        assert!(deadline.effect.parameters.contains_key("max_payment_days"));
    }

    #[test]
    fn test_cca_statutes_ids() {
        let statutes = all_cca_statutes();
        assert_eq!(statutes.len(), 4);
        assert!(statutes.iter().all(|s| s.id.starts_with("CCA_")));
        // Art.4 grants right to consumer, Art.8 prohibits operator clause
        let rescind = cca_article_4_right_to_rescind();
        assert!(matches!(
            rescind.effect.effect_type,
            legalis_core::EffectType::Grant
        ));
        let liability = cca_article_8_liability_clause_invalidity();
        assert!(matches!(
            liability.effect.effect_type,
            legalis_core::EffectType::Prohibition
        ));
    }

    #[test]
    fn test_pipa_jurisdictions() {
        for s in all_pipa_statutes() {
            assert_eq!(s.jurisdiction.as_deref(), Some("JP"));
        }
    }
}
