/// Manually-defined Legalis `Statute` objects for Japanese law domains not yet
/// covered by prebuilt functions in `legalis-jp`.
///
/// These allow `JpDomainMatcher` to provide SMT-verifiable statutes for common
/// queries such as 個人情報保護法, 下請法, and 消費者契約法 — instead of falling
/// back to Gemini DSL translation (which almost always fails for these laws).
///
/// Naming convention for IDs:
///   PIPA_ArtN          — 個人情報の保護に関する法律 (Personal Information Protection Act)
///   SHA_ArtN           — 下請代金支払遅延等防止法 (Subcontracting Act)
///   CCA_ArtN           — 消費者契約法 (Consumer Contract Act)
///   Minpo_ArtN         — 民法 (Civil Code; tort articles reuse legalis-jp `minpo-709/710/715-1`)
///   CompaniesAct_ArtN  — 会社法 (Companies Act) / CommercialCode_ArtN — 商法 (Commercial Code)
///   Const_ArtN         — 日本国憲法 (Constitution of Japan)
///   Copyright_ArtN     — 著作権法 (Copyright Act) / Patent_ArtN — 特許法 (Patent Act)
///   AirPollution_ArtN / WaterPollution_ArtN / WasteManagement_ArtN / EIA_ArtN — 環境法 (Environmental Law)
///   AdminProc_ArtN     — 行政手続法 (Administrative Procedure Act)
///   Construction_ArtN  — 建設業法 / RealEstate_ArtN — 宅地建物取引業法 (Construction & Real Estate)
use legalis_core::{ComparisonOp, Condition, Effect, EffectType, Statute};
use legalis_jp::minpo::{article_709, article_710, article_715_1};

// ─── 個人情報の保護に関する法律 (PIPA) ─────────────────────────────────────────

/// Art.17 — 適正な取得（不正な手段による個人情報の取得禁止）
///
/// Modeled precondition: `is_personal_info_operator == "true"`.
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
    .with_precondition(Condition::AttributeEquals {
        key: "is_personal_info_operator".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.18 — 利用目的の特定・通知・公表義務
///
/// Modeled precondition: `is_personal_info_operator == "true"`.
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
    .with_precondition(Condition::AttributeEquals {
        key: "is_personal_info_operator".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.27 — 第三者提供の制限（本人同意なき第三者提供禁止）
///
/// Modeled precondition: `is_personal_info_operator == "true"`.
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
    .with_precondition(Condition::AttributeEquals {
        key: "is_personal_info_operator".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.28 — 外国にある第三者への提供制限
///
/// Modeled precondition: `is_personal_info_operator == "true"`.
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
    .with_precondition(Condition::AttributeEquals {
        key: "is_personal_info_operator".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.33 — 開示請求権（本人による保有個人データの開示請求）
///
/// Modeled precondition: `request_type == "disclosure"`.
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
    .with_precondition(Condition::AttributeEquals {
        key: "request_type".to_string(),
        value: "disclosure".to_string(),
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

// ─── 民法 (Civil Code / Minpo) ──────────────────────────────────────────────
//
// 不法行為 (tort) statutes reuse the prebuilt legalis-jp builders
// (`minpo::article_709/710/715_1`). General and contract provisions below are
// hand-written following the same pattern.

/// Returns the prebuilt 民法 tort statutes (不法行為): Art.709, 710, 715(1).
#[must_use]
pub fn all_minpo_tort_statutes() -> Vec<Statute> {
    vec![article_709(), article_710(), article_715_1()]
}

/// Art.90 — 公序良俗違反の法律行為の無効
///
/// Kept as `Condition::Custom`: "public order or good morals" (公序良俗) is a
/// genuinely open-textured standard resolved case-by-case by judicial
/// balancing of the act's purpose, effect, and social context — no fixed
/// attribute set can mechanically decide it without misrepresenting judicial
/// discretion as mechanical application.
#[must_use]
pub fn minpo_article_90_public_policy() -> Statute {
    Statute::new(
        "Minpo_Art90",
        "公序良俗違反の無効 / Acts Against Public Policy are Void (Art. 90)",
        Effect::new(
            EffectType::Prohibition,
            "公の秩序又は善良の風俗に反する法律行為は無効とする \
             / A juridical act contrary to public order or good morals is void",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "法律行為が公の秩序又は善良の風俗に反する場合 \
                      / When a juridical act is contrary to public order or good morals"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.415 — 債務不履行による損害賠償
///
/// Kept as `Condition::Custom`: whether performance was "consistent with the
/// purpose of the obligation" (債務の本旨に従った履行) or "impossible"
/// (履行不能) requires judicial evaluation of the specific contract's terms
/// and circumstances — a genuinely discretionary standard, not a mechanical
/// attribute check.
#[must_use]
pub fn minpo_article_415_breach_damages() -> Statute {
    Statute::new(
        "Minpo_Art415",
        "債務不履行による損害賠償 / Damages for Non-Performance (Art. 415)",
        Effect::new(
            EffectType::Obligation,
            "債務者がその債務の本旨に従った履行をしないときは債権者は生じた損害の賠償を請求できる \
             / Where an obligor fails to perform consistent with the purpose of the obligation, \
               it must compensate the obligee for the damage arising therefrom",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "債務者がその債務の本旨に従った履行をせず、又は債務の履行が不能であるとき \
                      / When the obligor fails to perform per the obligation's purpose, or performance is impossible"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Art.541 — 催告による契約の解除権
///
/// Kept as `Condition::Custom`: whether a "reasonable period" (相当の期間)
/// was specified in the demand, and whether performance genuinely failed to
/// occur within it, are fact-sensitive judicial determinations rather than a
/// fixed threshold.
#[must_use]
pub fn minpo_article_541_rescission() -> Statute {
    Statute::new(
        "Minpo_Art541",
        "催告による解除 / Right of Rescission upon Demand (Art. 541)",
        Effect::new(
            EffectType::Grant,
            "相当の期間を定めて履行を催告し、その期間内に履行がないときは相手方は契約を解除できる \
             / A party may rescind the contract if performance is not made within a reasonable period \
               specified in a demand",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "当事者の一方が債務を履行しない場合に相手方が相当の期間を定めて履行を催告したとき \
                      / When one party fails to perform and the other demands performance within a reasonable period"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 民法 statutes (不法行為 tort + general/contract provisions).
#[must_use]
pub fn all_minpo_statutes() -> Vec<Statute> {
    let mut statutes = all_minpo_tort_statutes();
    statutes.push(minpo_article_90_public_policy());
    statutes.push(minpo_article_415_breach_damages());
    statutes.push(minpo_article_541_rescission());
    statutes
}

// ─── 商法・会社法 (Commercial Code / Companies Act) ────────────────────────────

/// 会社法 Art.330 — 取締役と会社の関係（委任）に基づく善管注意義務
#[must_use]
pub fn companies_act_article_330_duty_of_care() -> Statute {
    Statute::new(
        "CompaniesAct_Art330",
        "取締役の善管注意義務 / Director's Duty of Care (Art. 330)",
        Effect::new(
            EffectType::Obligation,
            "取締役は委任に関する規定に従い善良な管理者の注意をもって会社のため職務を行わなければならない \
             / A director owes the company a duty of care of a prudent manager under the mandate provisions",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_director".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 会社法 Art.355 — 取締役の忠実義務
#[must_use]
pub fn companies_act_article_355_duty_of_loyalty() -> Statute {
    Statute::new(
        "CompaniesAct_Art355",
        "取締役の忠実義務 / Director's Duty of Loyalty (Art. 355)",
        Effect::new(
            EffectType::Obligation,
            "取締役は法令・定款及び株主総会の決議を遵守し会社のため忠実にその職務を行わなければならない \
             / A director must faithfully perform their duties for the company in compliance with laws, the articles, and resolutions",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_director".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 会社法 Art.356 — 競業及び利益相反取引の制限（承認義務）
#[must_use]
pub fn companies_act_article_356_conflict_of_interest() -> Statute {
    Statute::new(
        "CompaniesAct_Art356",
        "競業・利益相反取引の制限 / Restriction on Competing and Conflicting Transactions (Art. 356)",
        Effect::new(
            EffectType::Obligation,
            "取締役は競業取引又は利益相反取引をしようとするときは重要な事実を開示し株主総会（取締役会）の承認を受けなければならない \
             / A director must disclose material facts and obtain shareholder/board approval before competing or self-dealing transactions",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_director".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 会社法 Art.423 — 役員等の株式会社に対する損害賠償責任
#[must_use]
pub fn companies_act_article_423_liability_to_company() -> Statute {
    Statute::new(
        "CompaniesAct_Art423",
        "役員等の対会社責任 / Officers' Liability to the Company (Art. 423)",
        Effect::new(
            EffectType::Obligation,
            "取締役・監査役等の役員等がその任務を怠ったときは会社に対し生じた損害を賠償する責任を負う \
             / An officer who neglects their duties is liable to compensate the company for the resulting damage",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_company_officer".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 商法 Art.512 — 商人の報酬請求権
#[must_use]
pub fn commercial_code_article_512_remuneration() -> Statute {
    Statute::new(
        "CommercialCode_Art512",
        "商人の報酬請求権 / Merchant's Right to Remuneration (Art. 512)",
        Effect::new(
            EffectType::Grant,
            "商人がその営業の範囲内において他人のために行為をしたときは相当の報酬を請求できる \
             / A merchant who acts for another within the scope of its business may claim reasonable remuneration",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_merchant".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 商法・会社法 statutes for matcher use.
#[must_use]
pub fn all_commercial_statutes() -> Vec<Statute> {
    vec![
        companies_act_article_330_duty_of_care(),
        companies_act_article_355_duty_of_loyalty(),
        companies_act_article_356_conflict_of_interest(),
        companies_act_article_423_liability_to_company(),
        commercial_code_article_512_remuneration(),
    ]
}

// ─── 日本国憲法 (Constitution of Japan) ────────────────────────────────────────

/// 憲法 Art.13 — 個人の尊重・幸福追求権
///
/// Kept as `Condition::Custom`: rights under the "pursuit of happiness"
/// clause are bounded by "the public welfare" (公共の福祉), an
/// open-textured constitutional balancing standard that courts weigh
/// case-by-case — not a mechanically decidable attribute.
#[must_use]
pub fn constitution_article_13_individual_dignity() -> Statute {
    Statute::new(
        "Const_Art13",
        "個人の尊重・幸福追求権 / Respect for Individuals and Pursuit of Happiness (Art. 13)",
        Effect::new(
            EffectType::Grant,
            "すべて国民は個人として尊重され、生命・自由及び幸福追求に対する権利は公共の福祉に反しない限り最大限尊重される \
             / All people are respected as individuals; their right to life, liberty, and the pursuit of happiness is supremely respected within the public welfare",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "国民が生命・自由・幸福追求に関わる利益を主張する場合 \
                      / When a person asserts interests concerning life, liberty, or the pursuit of happiness"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// 憲法 Art.14 — 法の下の平等（差別の禁止）
///
/// Kept as `Condition::Custom`: whether a distinction amounts to
/// unconstitutional discrimination requires the judiciary's rationality/strict-
/// scrutiny balancing test (合理的区別か否かの審査) applied to the specific
/// classification — not a fixed attribute check.
#[must_use]
pub fn constitution_article_14_equality() -> Statute {
    Statute::new(
        "Const_Art14",
        "法の下の平等 / Equality Under the Law (Art. 14)",
        Effect::new(
            EffectType::Prohibition,
            "人種・信条・性別・社会的身分又は門地により政治的・経済的・社会的関係において差別してはならない \
             / Discrimination in political, economic, or social relations based on race, creed, sex, social status, or family origin is prohibited",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "国家が国民を政治的・経済的・社会的関係において取り扱う場合 \
                      / When the state treats persons in political, economic, or social relations"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// 憲法 Art.21 — 表現の自由
///
/// Kept as `Condition::Custom`: permissible limits on expression (e.g. time,
/// place, and manner restrictions vs. content-based censorship) are decided
/// through judicial balancing tests, not a mechanically decidable attribute.
#[must_use]
pub fn constitution_article_21_freedom_of_expression() -> Statute {
    Statute::new(
        "Const_Art21",
        "表現の自由 / Freedom of Expression (Art. 21)",
        Effect::new(
            EffectType::Grant,
            "集会・結社及び言論・出版その他一切の表現の自由は保障され、検閲は禁止される \
             / Freedom of assembly, association, speech, press, and all other forms of expression is guaranteed; censorship is prohibited",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "表現・集会・結社その他の表現行為を行う場合 \
                      / When engaging in expression, assembly, association, or other expressive conduct"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// 憲法 Art.25 — 生存権
///
/// Kept as `Condition::Custom`: the "minimum standards of wholesome and
/// cultured living" (健康で文化的な最低限度の生活) is a programmatic
/// standard the Supreme Court has held is concretized through legislative
/// and administrative discretion (立法裁量), not a fixed numeric threshold.
#[must_use]
pub fn constitution_article_25_right_to_life() -> Statute {
    Statute::new(
        "Const_Art25",
        "生存権 / Right to a Minimum Standard of Living (Art. 25)",
        Effect::new(
            EffectType::Grant,
            "すべて国民は健康で文化的な最低限度の生活を営む権利を有する \
             / All people have the right to maintain the minimum standards of wholesome and cultured living",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "国民が健康で文化的な最低限度の生活の保障を求める場合 \
                      / When a person claims the guarantee of minimum standards of wholesome and cultured living"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// 憲法 Art.29 — 財産権
///
/// Kept as `Condition::Custom`: whether a restriction or taking is "for
/// public use" with "just compensation" (正当な補償) requires a
/// proportionality/balancing determination on the specific facts, not a
/// mechanically decidable attribute.
#[must_use]
pub fn constitution_article_29_property_rights() -> Statute {
    Statute::new(
        "Const_Art29",
        "財産権の保障 / Guarantee of Property Rights (Art. 29)",
        Effect::new(
            EffectType::Grant,
            "財産権は侵してはならず、私有財産は正当な補償の下に公共のために用いることができる \
             / Property rights are inviolable; private property may be taken for public use upon just compensation",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "国民の財産権が制約され又は公共のために収用される場合 \
                      / When a person's property rights are restricted or taken for public use"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 憲法 statutes for matcher use.
#[must_use]
pub fn all_constitution_statutes() -> Vec<Statute> {
    vec![
        constitution_article_13_individual_dignity(),
        constitution_article_14_equality(),
        constitution_article_21_freedom_of_expression(),
        constitution_article_25_right_to_life(),
        constitution_article_29_property_rights(),
    ]
}

// ─── 知的財産法 (Intellectual Property: 著作権法・特許法) ────────────────────────

/// 著作権法 Art.21 — 複製権
#[must_use]
pub fn copyright_article_21_reproduction_right() -> Statute {
    Statute::new(
        "Copyright_Art21",
        "複製権 / Right of Reproduction (Art. 21)",
        Effect::new(
            EffectType::Grant,
            "著作者はその著作物を複製する権利を専有する \
             / The author has the exclusive right to reproduce the work",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_copyright_holder".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 著作権法 Art.27 — 翻訳権・翻案権等
#[must_use]
pub fn copyright_article_27_adaptation_right() -> Statute {
    Statute::new(
        "Copyright_Art27",
        "翻訳権・翻案権等 / Right of Translation and Adaptation (Art. 27)",
        Effect::new(
            EffectType::Grant,
            "著作者はその著作物を翻訳し、編曲し、若しくは変形し、又は脚色し、映画化し、その他翻案する権利を専有する \
             / The author has the exclusive right to translate, arrange, transform, dramatize, cinematize, or otherwise adapt the work",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_copyright_holder".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 著作権法 Art.32 — 引用（公正な慣行に合致する適法な引用）
///
/// Kept as `Condition::Custom`: whether a quotation is "consistent with fair
/// practice" and "within a justifiable scope" (公正な慣行/正当な範囲内) is a
/// multi-factor, fair-use-like judicial judgment (purpose, proportion,
/// necessity, attribution) — not a mechanically decidable attribute.
#[must_use]
pub fn copyright_article_32_quotation() -> Statute {
    Statute::new(
        "Copyright_Art32",
        "引用 / Permissible Quotation (Art. 32)",
        Effect::new(
            EffectType::Grant,
            "公表された著作物は公正な慣行に合致し報道・批評・研究等の目的上正当な範囲内であれば引用して利用できる \
             / A published work may be quoted within a justifiable scope consistent with fair practice for news, criticism, or research",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "公表された著作物を公正な慣行に従い正当な範囲内で引用する場合 \
                      / When quoting a published work within a justifiable scope consistent with fair practice"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// 特許法 Art.68 — 特許権の効力（業としての実施の専有）
#[must_use]
pub fn patent_article_68_patent_right_effect() -> Statute {
    Statute::new(
        "Patent_Art68",
        "特許権の効力 / Effect of a Patent Right (Art. 68)",
        Effect::new(
            EffectType::Grant,
            "特許権者は業として特許発明の実施をする権利を専有する \
             / A patentee has the exclusive right to commercially work the patented invention",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_patent_holder".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 特許法 Art.100 — 差止請求権
///
/// Kept as `Condition::Custom`: whether infringement or a substantial threat
/// of infringement exists requires claim-construction and infringement
/// analysis specific to the patent and accused conduct — a judicial
/// determination, not a mechanically decidable attribute.
#[must_use]
pub fn patent_article_100_injunction() -> Statute {
    Statute::new(
        "Patent_Art100",
        "差止請求権 / Right to Seek Injunction (Art. 100)",
        Effect::new(
            EffectType::Grant,
            "特許権者は自己の特許権を侵害する者又は侵害するおそれがある者に対し侵害の停止又は予防を請求できる \
             / A patentee may demand that an infringer or potential infringer stop or prevent the infringement",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "特許権者の特許権を侵害し又は侵害するおそれがある者が存在する場合 \
                      / When a person infringes or is likely to infringe the patentee's patent right"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 知的財産法 statutes for matcher use.
#[must_use]
pub fn all_ip_statutes() -> Vec<Statute> {
    vec![
        copyright_article_21_reproduction_right(),
        copyright_article_27_adaptation_right(),
        copyright_article_32_quotation(),
        patent_article_68_patent_right_effect(),
        patent_article_100_injunction(),
    ]
}

// ─── 環境法 (Environmental Law) ───────────────────────────────────────────────

/// 大気汚染防止法 Art.13 — ばい煙の排出基準遵守義務
///
/// Modeled precondition: `operates_soot_smoke_facility == "true"`.
#[must_use]
pub fn air_pollution_article_13_emission_standards() -> Statute {
    Statute::new(
        "AirPollution_Art13",
        "ばい煙排出基準の遵守義務 / Compliance with Soot and Smoke Emission Standards (Art. 13)",
        Effect::new(
            EffectType::Obligation,
            "ばい煙発生施設を設置する者は排出口において排出基準に適合しないばい煙を排出してはならない \
             / An operator of a soot-and-smoke facility must not emit soot and smoke exceeding the emission standards at the outlet",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "operates_soot_smoke_facility".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 水質汚濁防止法 Art.12 — 排水基準の遵守義務
///
/// Modeled precondition: `operates_effluent_facility == "true"`.
#[must_use]
pub fn water_pollution_article_12_effluent_standards() -> Statute {
    Statute::new(
        "WaterPollution_Art12",
        "排水基準の遵守義務 / Compliance with Effluent Standards (Art. 12)",
        Effect::new(
            EffectType::Obligation,
            "特定事業場から公共用水域に水を排出する者は排水基準に適合しない排出水を排出してはならない \
             / A person discharging from a specified establishment into public water areas must not discharge effluent exceeding the effluent standards",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "operates_effluent_facility".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 廃棄物処理法 Art.3 — 事業者の処理責任
#[must_use]
pub fn waste_management_article_3_operator_responsibility() -> Statute {
    Statute::new(
        "WasteManagement_Art3",
        "事業者の処理責任 / Generator's Responsibility for Proper Disposal (Art. 3)",
        Effect::new(
            EffectType::Obligation,
            "事業者は事業活動に伴って生じた廃棄物を自らの責任において適正に処理しなければならない \
             / A business operator must properly dispose of waste generated by its activities at its own responsibility",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_business_operator".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 大気汚染防止法 Art.25 — 無過失損害賠償責任（健康被害）
///
/// Kept as `Condition::Custom`: strict (no-fault) liability still requires
/// judicial findings of causation between the specific emission and the
/// health harm (因果関係の立証) — a fact-intensive determination, not a
/// mechanically decidable attribute.
#[must_use]
pub fn air_pollution_article_25_strict_liability() -> Statute {
    Statute::new(
        "AirPollution_Art25",
        "無過失損害賠償責任 / Strict Liability for Health Damage (Art. 25)",
        Effect::new(
            EffectType::Obligation,
            "工場・事業場のばい煙又は特定物質の排出により人の生命又は身体を害したときは故意過失を問わず損害を賠償しなければならない \
             / A factory must compensate, regardless of fault, for harm to life or body caused by its emission of soot, smoke, or designated substances",
        ),
    )
    .with_precondition(Condition::Custom {
        description: "工場・事業場のばい煙又は特定物質の排出により人の生命又は身体が害された場合 \
                      / When emission of soot, smoke, or designated substances harms a person's life or body"
            .to_string(),
    })
    .with_jurisdiction("JP")
}

/// 環境影響評価法 Art.5 — 環境影響評価の実施（第一種事業）
///
/// Modeled precondition: `project_class == "class_1"`.
#[must_use]
pub fn eia_article_5_environmental_assessment() -> Statute {
    Statute::new(
        "EIA_Art5",
        "環境影響評価の実施義務 / Obligation to Conduct Environmental Impact Assessment (Art. 5)",
        Effect::new(
            EffectType::Obligation,
            "第一種事業を実施しようとする事業者は環境影響評価方法書を作成し環境影響評価を行わなければならない \
             / A proponent of a Class-1 project must prepare a scoping document and conduct an environmental impact assessment",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "project_class".to_string(),
        value: "class_1".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 環境法 statutes for matcher use.
#[must_use]
pub fn all_environmental_statutes() -> Vec<Statute> {
    vec![
        air_pollution_article_13_emission_standards(),
        water_pollution_article_12_effluent_standards(),
        waste_management_article_3_operator_responsibility(),
        air_pollution_article_25_strict_liability(),
        eia_article_5_environmental_assessment(),
    ]
}

// ─── 行政手続法 (Administrative Procedure Act) ─────────────────────────────────

/// 行政手続法 Art.5 — 審査基準の設定・公表
#[must_use]
pub fn apa_article_5_review_standards() -> Statute {
    Statute::new(
        "AdminProc_Art5",
        "審査基準の設定・公表 / Establishment and Publication of Review Standards (Art. 5)",
        Effect::new(
            EffectType::Obligation,
            "行政庁は許認可等の審査基準を定め、行政上特別の支障があるときを除き原則として公にしておかなければならない \
             / An administrative agency must establish and, as a rule, make public the review standards for permissions",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_administrative_agency".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 行政手続法 Art.8 — 申請拒否処分の理由提示
#[must_use]
pub fn apa_article_8_reasons_for_denial() -> Statute {
    Statute::new(
        "AdminProc_Art8",
        "申請拒否処分の理由提示 / Statement of Reasons for Denial of Application (Art. 8)",
        Effect::new(
            EffectType::Obligation,
            "行政庁は申請により求められた許認可等を拒否する処分をする場合は申請者に対し同時にその理由を示さなければならない \
             / When denying an application, the agency must simultaneously show the applicant the reasons therefor",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_administrative_agency".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 行政手続法 Art.13 — 不利益処分の聴聞・弁明の機会の付与
#[must_use]
pub fn apa_article_13_hearing_opportunity() -> Statute {
    Statute::new(
        "AdminProc_Art13",
        "不利益処分の聴聞・弁明の機会 / Opportunity for Hearing before Adverse Disposition (Art. 13)",
        Effect::new(
            EffectType::Obligation,
            "行政庁は不利益処分をしようとする場合は聴聞又は弁明の機会の付与の手続を執らなければならない \
             / Before imposing an adverse disposition, the agency must grant the party an opportunity for a hearing or explanation",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_administrative_agency".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 行政手続法 Art.14 — 不利益処分の理由提示
#[must_use]
pub fn apa_article_14_reasons_for_adverse_disposition() -> Statute {
    Statute::new(
        "AdminProc_Art14",
        "不利益処分の理由提示 / Statement of Reasons for Adverse Disposition (Art. 14)",
        Effect::new(
            EffectType::Obligation,
            "行政庁は不利益処分をする場合はその名宛人に対し同時に当該不利益処分の理由を示さなければならない \
             / When imposing an adverse disposition, the agency must simultaneously show the addressee the reasons therefor",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_administrative_agency".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 行政手続法 Art.32 — 行政指導の一般原則（任意性・不利益取扱いの禁止）
#[must_use]
pub fn apa_article_32_administrative_guidance() -> Statute {
    Statute::new(
        "AdminProc_Art32",
        "行政指導の一般原則 / General Principles of Administrative Guidance (Art. 32)",
        Effect::new(
            EffectType::Prohibition,
            "行政指導は相手方の任意の協力によってのみ実現されるものであり、これに従わなかったことを理由に不利益な取扱いをしてはならない \
             / Administrative guidance is realized only through voluntary cooperation; no disadvantageous treatment may be imposed for non-compliance",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_administrative_agency".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 行政手続法 statutes for matcher use.
#[must_use]
pub fn all_admin_procedure_statutes() -> Vec<Statute> {
    vec![
        apa_article_5_review_standards(),
        apa_article_8_reasons_for_denial(),
        apa_article_13_hearing_opportunity(),
        apa_article_14_reasons_for_adverse_disposition(),
        apa_article_32_administrative_guidance(),
    ]
}

// ─── 建設業法・宅地建物取引業法 (Construction Business / Real Estate Brokerage) ──

/// 建設業法 Art.3 — 建設業の許可
///
/// Modeled precondition: `contract_value_jpy >= 5,000,000` (¥5,000,000).
///
/// Simplification note: the real statute (建設業法施行令 Art.1-2) sets a
/// **dual** minor-works threshold — ¥5,000,000 for most construction work,
/// but ¥15,000,000 (or ≥150m² of floor area for wooden residential work) for
/// building construction complete works (建築一式工事). This model applies
/// the lower, more conservative ¥5,000,000 threshold uniformly across work
/// types; a future refinement could branch on a `work_type` attribute to
/// apply the ¥15,000,000 threshold specifically for 建築一式工事.
#[must_use]
pub fn construction_article_3_license() -> Statute {
    Statute::new(
        "Construction_Art3",
        "建設業の許可 / License Requirement for Construction Business (Art. 3)",
        Effect::new(
            EffectType::Obligation,
            "建設業を営もうとする者は軽微な建設工事のみを請け負う場合を除き国土交通大臣又は都道府県知事の許可を受けなければならない \
             / A person intending to operate a construction business must obtain a license, except when undertaking only minor works",
        ),
    )
    .with_precondition(Condition::Threshold {
        attributes: vec![("contract_value_jpy".to_string(), 1.0)],
        operator: ComparisonOp::GreaterOrEqual,
        value: 5_000_000.0,
    })
    .with_jurisdiction("JP")
}

/// 建設業法 Art.19 — 建設工事の請負契約の書面交付
#[must_use]
pub fn construction_article_19_written_contract() -> Statute {
    Statute::new(
        "Construction_Art19",
        "建設工事請負契約の書面交付 / Written Construction Contract Requirement (Art. 19)",
        Effect::new(
            EffectType::Obligation,
            "建設工事の請負契約の当事者は契約締結に際し工事内容・請負代金額等を記載した書面を相互に交付しなければならない \
             / Parties to a construction contract must mutually deliver a written document stating the work details and contract price",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_construction_contractor".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 宅地建物取引業法 Art.35 — 重要事項の説明
#[must_use]
pub fn real_estate_article_35_important_matters() -> Statute {
    Statute::new(
        "RealEstate_Art35",
        "重要事項の説明義務 / Duty to Explain Important Matters (Art. 35)",
        Effect::new(
            EffectType::Obligation,
            "宅地建物取引業者は契約成立前に宅地建物取引士をして重要事項を記載した書面を交付して説明させなければならない \
             / Before a contract is concluded, a real estate broker must have a registered agent deliver and explain a written statement of important matters",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_real_estate_broker".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// 宅地建物取引業法 Art.37 — 契約締結時の書面交付
#[must_use]
pub fn real_estate_article_37_contract_document() -> Statute {
    Statute::new(
        "RealEstate_Art37",
        "契約締結時の書面交付義務 / Duty to Deliver Contract Document (Art. 37)",
        Effect::new(
            EffectType::Obligation,
            "宅地建物取引業者は契約が成立したときは遅滞なく所定の事項を記載した書面を当該契約の当事者に交付しなければならない \
             / Upon conclusion of a contract, a real estate broker must promptly deliver to the parties a document stating the prescribed matters",
        ),
    )
    .with_precondition(Condition::AttributeEquals {
        key: "is_real_estate_broker".to_string(),
        value: "true".to_string(),
    })
    .with_jurisdiction("JP")
}

/// Returns all key 建設業法・宅地建物取引業法 statutes for matcher use.
#[must_use]
pub fn all_construction_real_estate_statutes() -> Vec<Statute> {
    vec![
        construction_article_3_license(),
        construction_article_19_written_contract(),
        real_estate_article_35_important_matters(),
        real_estate_article_37_contract_document(),
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

    #[test]
    fn test_pipa_preconditions_structured() {
        // Art.17/18/27/28 (取得・利用・提供) share the operator role flag.
        let operator_flag = Condition::AttributeEquals {
            key: "is_personal_info_operator".to_string(),
            value: "true".to_string(),
        };
        assert_eq!(
            pipa_article_17_lawful_acquisition().preconditions,
            vec![operator_flag.clone()]
        );
        assert_eq!(
            pipa_article_18_purpose_specification().preconditions,
            vec![operator_flag.clone()]
        );
        assert_eq!(
            pipa_article_27_third_party_restriction().preconditions,
            vec![operator_flag.clone()]
        );
        assert_eq!(
            pipa_article_28_cross_border_transfer().preconditions,
            vec![operator_flag]
        );
        // Art.33 (開示請求) is a request-type flag, distinct from the operator role.
        assert_eq!(
            pipa_article_33_disclosure_right().preconditions,
            vec![Condition::AttributeEquals {
                key: "request_type".to_string(),
                value: "disclosure".to_string(),
            }]
        );
    }

    #[test]
    fn test_minpo_statutes() {
        let statutes = all_minpo_statutes();
        // 3 reused tort articles (legalis-jp) + 3 hand-written general/contract provisions
        assert_eq!(statutes.len(), 6);
        assert!(statutes.iter().any(|s| s.id == "minpo-709"));
        assert!(statutes.iter().any(|s| s.id == "Minpo_Art415"));
        // The tort-only wrapper reuses the prebuilt legalis-jp builders
        let tort = all_minpo_tort_statutes();
        assert_eq!(tort.len(), 3);
        assert!(tort.iter().any(|s| s.id == "minpo-715-1"));
    }

    #[test]
    fn test_commercial_statutes_ids() {
        let statutes = all_commercial_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(statutes.iter().any(|s| s.id == "CompaniesAct_Art423"));
        assert!(statutes.iter().any(|s| s.id == "CommercialCode_Art512"));
        // Director liability is an obligation; merchant remuneration is a grant
        assert!(matches!(
            companies_act_article_423_liability_to_company()
                .effect
                .effect_type,
            EffectType::Obligation
        ));
        assert!(matches!(
            commercial_code_article_512_remuneration()
                .effect
                .effect_type,
            EffectType::Grant
        ));
    }

    #[test]
    fn test_constitution_statutes_ids() {
        let statutes = all_constitution_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(statutes.iter().all(|s| s.id.starts_with("Const_")));
        assert!(statutes.iter().any(|s| s.id == "Const_Art13"));
        // Equality (Art.14) is modeled as a prohibition on discrimination
        assert!(matches!(
            constitution_article_14_equality().effect.effect_type,
            EffectType::Prohibition
        ));
    }

    #[test]
    fn test_ip_statutes_ids() {
        let statutes = all_ip_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(statutes.iter().any(|s| s.id == "Copyright_Art21"));
        assert!(statutes.iter().any(|s| s.id == "Patent_Art68"));
        // Reproduction right is a grant of an exclusive right
        assert!(matches!(
            copyright_article_21_reproduction_right().effect.effect_type,
            EffectType::Grant
        ));
    }

    #[test]
    fn test_environmental_statutes_ids() {
        let statutes = all_environmental_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(statutes.iter().any(|s| s.id == "AirPollution_Art13"));
        assert!(statutes.iter().any(|s| s.id == "EIA_Art5"));
        // Emission-standard compliance is an obligation
        assert!(matches!(
            air_pollution_article_13_emission_standards()
                .effect
                .effect_type,
            EffectType::Obligation
        ));
    }

    #[test]
    fn test_environmental_facility_preconditions_structured() {
        assert_eq!(
            air_pollution_article_13_emission_standards().preconditions,
            vec![Condition::AttributeEquals {
                key: "operates_soot_smoke_facility".to_string(),
                value: "true".to_string(),
            }]
        );
        assert_eq!(
            water_pollution_article_12_effluent_standards().preconditions,
            vec![Condition::AttributeEquals {
                key: "operates_effluent_facility".to_string(),
                value: "true".to_string(),
            }]
        );
        assert_eq!(
            eia_article_5_environmental_assessment().preconditions,
            vec![Condition::AttributeEquals {
                key: "project_class".to_string(),
                value: "class_1".to_string(),
            }]
        );
        // Art.25 (strict liability / causation) stays genuinely discretionary.
        let strict_liability = air_pollution_article_25_strict_liability();
        assert_eq!(strict_liability.preconditions.len(), 1);
        assert!(matches!(
            &strict_liability.preconditions[0],
            Condition::Custom { .. }
        ));
    }

    #[test]
    fn test_admin_procedure_statutes_ids() {
        let statutes = all_admin_procedure_statutes();
        assert_eq!(statutes.len(), 5);
        assert!(statutes.iter().all(|s| s.id.starts_with("AdminProc_")));
        // The administrative-guidance principle is a prohibition on coercive treatment
        assert!(matches!(
            apa_article_32_administrative_guidance().effect.effect_type,
            EffectType::Prohibition
        ));
    }

    #[test]
    fn test_construction_real_estate_statutes_ids() {
        let statutes = all_construction_real_estate_statutes();
        assert_eq!(statutes.len(), 4);
        assert!(statutes.iter().any(|s| s.id == "Construction_Art3"));
        assert!(statutes.iter().any(|s| s.id == "RealEstate_Art35"));
    }

    #[test]
    fn test_construction_art3_threshold_precondition() {
        assert_eq!(
            construction_article_3_license().preconditions,
            vec![Condition::Threshold {
                attributes: vec![("contract_value_jpy".to_string(), 1.0)],
                operator: ComparisonOp::GreaterOrEqual,
                value: 5_000_000.0,
            }]
        );
    }

    #[test]
    fn test_discretionary_statutes_stay_custom() {
        // These ~11 genuinely open-textured legal standards must remain
        // `Condition::Custom` — converting them would misrepresent judicial
        // discretion as mechanical application. This is a regression test:
        // if any of these ever gets accidentally "structured", this fails.
        let discretionary_statutes = vec![
            minpo_article_90_public_policy(),
            minpo_article_415_breach_damages(),
            minpo_article_541_rescission(),
            constitution_article_13_individual_dignity(),
            constitution_article_14_equality(),
            constitution_article_21_freedom_of_expression(),
            constitution_article_25_right_to_life(),
            constitution_article_29_property_rights(),
            copyright_article_32_quotation(),
            patent_article_100_injunction(),
            air_pollution_article_25_strict_liability(),
        ];
        assert_eq!(discretionary_statutes.len(), 11);
        for statute in &discretionary_statutes {
            assert_eq!(
                statute.preconditions.len(),
                1,
                "statute {} should have exactly one precondition",
                statute.id
            );
            assert!(
                matches!(&statute.preconditions[0], Condition::Custom { .. }),
                "statute {} should remain Condition::Custom",
                statute.id
            );
        }
    }

    #[test]
    fn test_new_domain_jurisdictions_are_jp() {
        let all = all_minpo_statutes()
            .into_iter()
            .chain(all_commercial_statutes())
            .chain(all_constitution_statutes())
            .chain(all_ip_statutes())
            .chain(all_environmental_statutes())
            .chain(all_admin_procedure_statutes())
            .chain(all_construction_real_estate_statutes());
        for s in all {
            assert_eq!(s.jurisdiction.as_deref(), Some("JP"));
        }
    }
}
