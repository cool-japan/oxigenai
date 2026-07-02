//! Generative Jurisprudence (生成的法解釈) — predict a likely judicial ruling.
//!
//! This module implements [`CaseLawPredictor`], the engine behind the
//! `POST /predict-ruling` endpoint and the `oxigenai predict` CLI subcommand.
//!
//! Pipeline:
//! 1. Infer a [`LegalArea`] and keywords from the user's query / matched articles
//!    (leveraging [`JpDomainMatcher`] and article titles).
//! 2. Search a curated in-memory corpus of **real Japanese landmark precedents**
//!    (最高裁 等) via [`CaseLawSearchEngine`], ranking by
//!    `relevance_score × precedent_authority`.
//! 3. Use Gemini **web-grounded** retrieval (temp 0) to surface/confirm additional
//!    current precedents, then a **deterministic synthesis** call (temp 0) that —
//!    given the precedents + the user's facts/articles — predicts a holding (結論),
//!    reasoning grounded in the cited precedents (判例の射程), and a confidence
//!    assessment, flagging genuine 司法裁量.
//! 4. Map the outcome onto a `legalis_core::LegalResult::JudicialDiscretion`.
//!
//! The corpus, search, ranking, citation and Markdown rendering are all pure /
//! offline so they are unit-tested without any network or GCP access. Only the
//! two Gemini calls inside [`CaseLawPredictor::predict_ruling`] touch the network.

use crate::error::Result;
use crate::models::law::FullArticle;
use crate::models::legal_result::SectionClassification;
use crate::models::response::UsageSummaryEntry;
use crate::services::gemini_client::{GeminiService, GenerationConfig, Grounding};
use crate::services::usage_tracker::{UsageMetadata, UsageTracker};
use crate::verifier::dsl_bridge::JpDomainMatcher;
use chrono::{DateTime, Utc};
use legalis_core::{LegalResult, Uuid};
use legalis_jp::case_law::citation::citation_link;
use legalis_jp::case_law::{
    CaseLawDatabase, CaseLawError, CaseLawSearchEngine, CaseMetadata, CaseOutcome, CaseSearchQuery,
    CaseSearchResult, CitationFormatter, CitationStyle, Court, CourtDecision, CourtLevel, Holding,
    InMemoryCaseDatabase, LegalArea, Party,
};
use serde::Serialize;
use tracing::{debug, info, warn};

/// Number of top precedents carried into synthesis / surfaced to the caller.
const TOP_K: usize = 5;
/// Max output tokens for the web-grounded precedent retrieval call.
const GROUNDED_MAX_TOKENS: u32 = 2048;
/// Max output tokens for the deterministic synthesis call.
const SYNTHESIS_MAX_TOKENS: u32 = 4096;
/// Official 裁判所 判例検索システム portal — used as an honest "search link"
/// source for landmark precedents (we never fabricate per-case detail URLs).
const COURTS_SEARCH_URL: &str = "https://www.courts.go.jp/app/hanrei_jp/search1";

// ─── Public output models ────────────────────────────────────────────────────────

/// A single precedent cited in support of a prediction.
#[derive(Debug, Clone, Serialize)]
pub struct CitedPrecedent {
    /// Stable corpus identifier.
    pub id: String,
    /// Popular case name (判例通称), e.g. "日本食塩製造事件".
    pub case_name: String,
    /// Court level Japanese name (e.g. "最高裁判所").
    pub court: String,
    /// Legal area Japanese name (e.g. "労働法").
    pub legal_area: String,
    /// Formatted standard Japanese citation.
    pub citation: String,
    /// Relevance score from the search engine (0.0–1.0).
    pub relevance_score: f64,
    /// Precedent weight (0 = highest authority / Supreme Court).
    pub precedent_weight: u8,
    /// One-line summary of the leading holding (判旨).
    pub holding_summary: String,
    /// Statutes cited by the decision.
    pub cited_statutes: Vec<String>,
    /// Source/search URL (official 判例検索 portal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Whether this is a binding Supreme Court precedent.
    pub is_binding: bool,
}

/// The full result of a ruling prediction.
#[derive(Debug, Clone, Serialize)]
pub struct RulingPrediction {
    /// The query (and any folded-in fact pattern) that was analysed.
    pub query: String,
    /// The legal issue framed for the discretion record.
    pub issue: String,
    /// Inferred legal area Japanese name.
    pub inferred_legal_area: String,
    /// Predicted holding / 結論.
    pub predicted_holding: String,
    /// Reasoning grounded in the cited precedents (Markdown).
    pub reasoning: String,
    /// Precedents the prediction is grounded in (ranked, top-K).
    pub cited_precedents: Vec<CitedPrecedent>,
    /// Structural confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Human label for the confidence (高い / 中程度 / 低い).
    pub confidence_label: String,
    /// legalis-core classification — always `"judicial_discretion"` for rulings.
    pub legal_classification: String,
    /// `LegalResult::JudicialDiscretion` context id (UUID).
    pub context_id: String,
    /// Full human-readable Markdown summary.
    pub markdown_summary: String,
    /// Token usage for the Gemini calls made during this prediction.
    pub usage: Vec<UsageSummaryEntry>,
}

// ─── Predictor ────────────────────────────────────────────────────────────────────

/// Predicts a likely judicial ruling from case law.
pub struct CaseLawPredictor {
    engine: CaseLawSearchEngine<InMemoryCaseDatabase>,
}

impl Default for CaseLawPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl CaseLawPredictor {
    /// Build a predictor seeded with the curated landmark-precedent corpus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: CaseLawSearchEngine::new(seed_landmark_cases()),
        }
    }

    /// Number of precedents in the corpus.
    #[must_use]
    pub fn case_count(&self) -> usize {
        self.engine.stats().total_cases
    }

    /// Offline precedent search: area-filtered + keyword-only passes, merged and
    /// de-duplicated by case id. Never panics; engine `NoResultsFound` → empty.
    #[must_use]
    pub fn search_local(&self, area: LegalArea, keywords: &[String]) -> Vec<CaseSearchResult> {
        let area_query = build_search_query(Some(area), keywords);
        let mut merged = run_search(&self.engine, &area_query);
        let mut seen: std::collections::HashSet<String> =
            merged.iter().map(|r| r.decision.id.clone()).collect();

        // Broader keyword-only pass to recover cross-area landmark precedents.
        let keyword_query = build_search_query(None, keywords);
        for result in run_search(&self.engine, &keyword_query) {
            if seen.insert(result.decision.id.clone()) {
                merged.push(result);
            }
        }

        merged
    }

    /// Predict a likely ruling for `query`, grounded in `articles` + the corpus.
    ///
    /// Makes two Gemini calls (web-grounded retrieval + deterministic synthesis).
    /// The grounded call is best-effort; synthesis is required.
    pub async fn predict_ruling(
        &self,
        query: &str,
        articles: &[FullArticle],
        gemini: &GeminiService,
    ) -> Result<RulingPrediction> {
        let area = infer_legal_area(query, articles);
        let keywords = extract_keywords(query, articles);
        debug!(
            "predict_ruling: area={}, keywords={:?}",
            area.japanese_name(),
            keywords
        );

        let ranked = rank_results(self.search_local(area, &keywords));
        let cited: Vec<CitedPrecedent> = ranked.iter().map(to_cited_precedent).collect();
        info!(
            "predict_ruling: {} local precedents matched for '{}'",
            cited.len(),
            truncate(query, 40)
        );

        let mut tracker = UsageTracker::new();

        // 1) Best-effort web-grounded precedent confirmation / discovery.
        let (grounded_text, grounded_usage) =
            match grounded_precedent_search(query, &cited, gemini).await {
                Ok(value) => value,
                Err(e) => {
                    warn!("Web-grounded precedent search failed (continuing): {e}");
                    (String::new(), None)
                }
            };
        if let Some(usage) = grounded_usage {
            tracker.add_usage(usage);
        }

        // 2) Deterministic synthesis of the predicted ruling.
        let (synthesis_text, synthesis_usage) =
            synthesize_ruling(query, articles, &cited, &grounded_text, gemini).await?;
        if let Some(usage) = synthesis_usage {
            tracker.add_usage(usage);
        }

        let reasoning = synthesis_text.trim().to_string();
        let predicted_holding = extract_section(&reasoning, "## 予測される結論")
            .unwrap_or_else(|| first_paragraph(&reasoning));
        let (confidence, confidence_label) = compute_confidence(&cited);

        // Map onto legalis-core: predicting a ruling is inherently 司法裁量.
        let issue = format!("「{}」についての裁判所の判断予測", truncate(query, 60));
        let legal_result: LegalResult<String> = LegalResult::JudicialDiscretion {
            issue: issue.clone(),
            context_id: Uuid::new_v4(),
            narrative_hint: Some(reasoning.clone()),
        };
        let legal_classification = classification_label(&legal_result).to_string();
        let context_id = discretion_context_id(&legal_result);

        let mut prediction = RulingPrediction {
            query: query.to_string(),
            issue,
            inferred_legal_area: area.japanese_name().to_string(),
            predicted_holding,
            reasoning,
            cited_precedents: cited,
            confidence,
            confidence_label: confidence_label.to_string(),
            legal_classification,
            context_id,
            markdown_summary: String::new(),
            usage: tracker.get_usage_summary(),
        };
        prediction.markdown_summary = render_markdown(&prediction);

        info!(
            "predict_ruling complete: {} precedents, confidence={:.2} ({})",
            prediction.cited_precedents.len(),
            prediction.confidence,
            prediction.confidence_label
        );
        Ok(prediction)
    }
}

// ─── Gemini calls (network) ────────────────────────────────────────────────────────

/// System prompt for the web-grounded precedent retrieval call.
const GROUNDED_SYSTEM_PROMPT: &str = "あなたは日本の判例法に精通した法律調査アシスタントです。\
Google検索を用いて、与えられた論点・事実関係に関連する『実在する』日本の裁判例\
（特に最高裁判所の判例）を調査してください。各判例について、事件名・裁判所・判決年月日・\
判旨（法理）・引用条文を簡潔にMarkdownの箇条書きでまとめてください。\
推測や創作は禁止です。確認できた判例のみを挙げ、不確実なものはその旨を明記してください。";

/// System prompt for the deterministic synthesis call.
const SYNTHESIS_SYSTEM_PROMPT: &str = "あなたは日本の裁判所の判断を予測する計算法学\
（computational law）アシスタントです。与えられた『判例』『条文』『事実関係』のみに基づき、\
想定される判決の結論と理由を予測してください。\n\
【厳守事項】\n\
1. すべての主張は、提示された判例・条文に明示的に根拠づけること（判例の射程を意識する）。\n\
2. 創作・憶測は禁止。提示資料にない事実・判例を新たに作り出さないこと。\n\
3. 本件が本質的に裁判官の裁量（司法裁量）に委ねられる論点である場合は、その旨を明確に指摘すること。\n\
4. 出力は必ず次の3つの見出しを持つ日本語Markdownとすること:\n\
## 予測される結論\n\
## 理由（判例の射程）\n\
## 確信度評価\n\
『確信度評価』では確信度を「高い／中程度／低い」で述べ、その根拠（参照判例の数・最高裁拘束力の\
有無・事実との適合度）を簡潔に示すこと。";

/// Web-grounded retrieval of additional / confirmatory real precedents.
async fn grounded_precedent_search(
    query: &str,
    cited: &[CitedPrecedent],
    gemini: &GeminiService,
) -> Result<(String, Option<UsageMetadata>)> {
    let gen_config = GenerationConfig {
        temperature: 0.0,
        max_output_tokens: GROUNDED_MAX_TOKENS,
        top_p: 1.0,
        top_k: 1,
        candidate_count: 1,
    };
    let input = format!(
        "# 法的論点・事実関係\n{}\n\n# 内部判例データベースの候補\n{}\n\n\
         上記の論点について、関連する実在の日本の裁判例（特に最高裁判所）をWeb検索で確認し、\
         事件名・裁判所・判決年月日・判旨・引用条文を整理してください。",
        query,
        format_precedents_for_prompt(cited)
    );
    let response = gemini
        .call_with_grounding(
            &input,
            GROUNDED_SYSTEM_PROMPT,
            Grounding::WebSearch,
            &gen_config,
        )
        .await?;
    Ok((response.text.trim().to_string(), response.usage))
}

/// Deterministic synthesis of the predicted ruling from the supplied evidence.
async fn synthesize_ruling(
    query: &str,
    articles: &[FullArticle],
    cited: &[CitedPrecedent],
    grounded_text: &str,
    gemini: &GeminiService,
) -> Result<(String, Option<UsageMetadata>)> {
    let gen_config = GenerationConfig {
        temperature: 0.0,
        max_output_tokens: SYNTHESIS_MAX_TOKENS,
        top_p: 1.0,
        top_k: 1,
        candidate_count: 1,
    };
    let grounded_block = if grounded_text.is_empty() {
        "（Web調査結果なし）".to_string()
    } else {
        grounded_text.to_string()
    };
    let input = format!(
        "# 法的論点・事実関係\n{}\n\n# 関連条文\n{}\n\n# 関連判例（内部判例データベース）\n{}\n\n\
         # Web調査による補足\n{}\n\n以上の資料のみに基づいて、想定される判決を予測してください。",
        query,
        format_articles_for_prompt(articles),
        format_precedents_for_prompt(cited),
        grounded_block
    );
    let response = gemini
        .call_strict(&input, SYNTHESIS_SYSTEM_PROMPT, &gen_config)
        .await?;
    Ok((response.text.trim().to_string(), response.usage))
}

// ─── Inference, search, ranking (pure / offline) ───────────────────────────────────

/// Infer the most relevant [`LegalArea`] from the query and matched articles.
///
/// Domain-matched article titles (recognised by [`JpDomainMatcher`]) are treated
/// as high-confidence signals; otherwise the query text is classified.
#[must_use]
pub fn infer_legal_area(query: &str, articles: &[FullArticle]) -> LegalArea {
    for article in articles {
        if JpDomainMatcher::has_domain_match(&article.title)
            && let Some(area) = classify_area(&article.title)
        {
            return area;
        }
    }
    classify_area(query).unwrap_or(LegalArea::Civil)
}

/// Classify free text to a [`LegalArea`] by keyword rules. `None` if no rule hits.
fn classify_area(text: &str) -> Option<LegalArea> {
    const RULES: &[(&str, LegalArea)] = &[
        ("解雇", LegalArea::Labor),
        ("雇止め", LegalArea::Labor),
        ("懲戒", LegalArea::Labor),
        ("就業規則", LegalArea::Labor),
        ("賃金", LegalArea::Labor),
        ("残業", LegalArea::Labor),
        ("時間外労働", LegalArea::Labor),
        ("過労", LegalArea::Labor),
        ("労災", LegalArea::Labor),
        ("労働", LegalArea::Labor),
        ("雇用", LegalArea::Labor),
        ("採用", LegalArea::Labor),
        ("思想", LegalArea::Constitutional),
        ("信条", LegalArea::Constitutional),
        ("表現の自由", LegalArea::Constitutional),
        ("人権", LegalArea::Constitutional),
        ("憲法", LegalArea::Constitutional),
        ("著作", LegalArea::IntellectualProperty),
        ("特許", LegalArea::IntellectualProperty),
        ("商標", LegalArea::IntellectualProperty),
        ("知的財産", LegalArea::IntellectualProperty),
        ("消費者", LegalArea::ConsumerProtection),
        ("取締役", LegalArea::Commercial),
        ("株主", LegalArea::Commercial),
        ("会社", LegalArea::Commercial),
        ("行政処分", LegalArea::Administrative),
        ("行政指導", LegalArea::Administrative),
        ("許認可", LegalArea::Administrative),
        ("相続", LegalArea::Family),
        ("離婚", LegalArea::Family),
        ("親権", LegalArea::Family),
        ("婚姻", LegalArea::Family),
        ("課税", LegalArea::Tax),
        ("租税", LegalArea::Tax),
        ("犯罪", LegalArea::Criminal),
        ("刑罰", LegalArea::Criminal),
        ("個人情報", LegalArea::Civil),
        ("プライバシー", LegalArea::Civil),
        ("名誉", LegalArea::Civil),
        ("不法行為", LegalArea::Civil),
        ("損害賠償", LegalArea::Civil),
        ("慰謝料", LegalArea::Civil),
        ("債務不履行", LegalArea::Civil),
        ("契約", LegalArea::Civil),
    ];
    RULES
        .iter()
        .find(|(keyword, _)| text.contains(keyword))
        .map(|(_, area)| *area)
}

/// Extract salient Japanese legal keywords from the query and article titles.
#[must_use]
pub fn extract_keywords(query: &str, articles: &[FullArticle]) -> Vec<String> {
    const TOKENS: &[&str] = &[
        "解雇権濫用",
        "解雇",
        "雇止め",
        "懲戒",
        "就業規則",
        "不利益変更",
        "安全配慮義務",
        "過労死",
        "過労自殺",
        "労災",
        "残業",
        "賃金",
        "労働契約",
        "採用",
        "思想信条",
        "私人間",
        "本採用拒否",
        "プライバシー",
        "個人情報",
        "漏えい",
        "名誉毀損",
        "名誉",
        "不法行為",
        "損害賠償",
        "慰謝料",
        "出版差止",
        "差止",
        "人格権",
        "表現の自由",
        "自己情報",
        "債務不履行",
        "契約",
    ];

    let mut haystack = query.to_string();
    for article in articles {
        haystack.push('\n');
        haystack.push_str(&article.title);
    }

    let mut keywords: Vec<String> = Vec::new();
    for token in TOKENS {
        if haystack.contains(token) && !keywords.iter().any(|existing| existing == token) {
            keywords.push((*token).to_string());
        }
    }

    // Fallback: derive a single keyword from the inferred area so the engine has
    // at least one signal to work with.
    if keywords.is_empty()
        && let Some(area) = classify_area(&haystack)
    {
        keywords.push(area.japanese_name().to_string());
    }

    keywords
}

/// Build a [`CaseSearchQuery`] from an optional area filter and keywords.
fn build_search_query(area: Option<LegalArea>, keywords: &[String]) -> CaseSearchQuery {
    let mut query = CaseSearchQuery::new().with_limit(TOP_K * 3);
    if let Some(area) = area {
        query = query.with_legal_area(area);
    }
    for keyword in keywords {
        query = query.with_keyword(keyword.as_str());
    }
    query
}

/// Run a search, mapping the empty-result error to an empty vector.
fn run_search(
    engine: &CaseLawSearchEngine<InMemoryCaseDatabase>,
    query: &CaseSearchQuery,
) -> Vec<CaseSearchResult> {
    match engine.search(query) {
        Ok(results) => results,
        Err(CaseLawError::NoResultsFound) => Vec::new(),
        Err(e) => {
            warn!("case-law search error: {e}");
            Vec::new()
        }
    }
}

/// Rank results by `relevance_score × authority_factor` and keep the top-K.
#[must_use]
pub fn rank_results(mut results: Vec<CaseSearchResult>) -> Vec<CaseSearchResult> {
    results.sort_by(|a, b| {
        let score_a = a.relevance_score * authority_factor(a.decision.precedent_weight());
        let score_b = b.relevance_score * authority_factor(b.decision.precedent_weight());
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(TOP_K);
    results
}

/// Authority multiplier: higher for more binding courts (weight 0 = Supreme).
fn authority_factor(precedent_weight: u8) -> f64 {
    match precedent_weight {
        0 => 1.0,
        1 => 0.8,
        2 => 0.6,
        _ => 0.4,
    }
}

/// Convert a search result into a serialisable [`CitedPrecedent`].
#[must_use]
pub fn to_cited_precedent(result: &CaseSearchResult) -> CitedPrecedent {
    let decision = &result.decision;
    let citation = CitationFormatter::format(decision, CitationStyle::Standard)
        .unwrap_or_else(|_| decision.metadata.case_number.clone());
    let holding_summary = decision
        .holdings
        .first()
        .map(|h| h.principle.clone())
        .unwrap_or_else(|| decision.summary.clone());

    CitedPrecedent {
        id: decision.id.clone(),
        case_name: decision.metadata.case_number.clone(),
        court: decision.metadata.court.level.japanese_name().to_string(),
        legal_area: decision.metadata.legal_area.japanese_name().to_string(),
        citation,
        relevance_score: result.relevance_score,
        precedent_weight: decision.precedent_weight(),
        holding_summary,
        cited_statutes: decision.metadata.cited_statutes.clone(),
        source_url: citation_link(decision),
        is_binding: decision.is_supreme_court_decision(),
    }
}

/// Compute a structural confidence and label from the matched precedents.
#[must_use]
pub fn compute_confidence(precedents: &[CitedPrecedent]) -> (f64, &'static str) {
    if precedents.is_empty() {
        return (0.2, "低い");
    }

    let mut score = 0.3;
    if precedents.iter().any(|p| p.is_binding) {
        score += 0.25;
    }
    let count_factor = (precedents.len().min(4) as f64 / 4.0) * 0.25;
    score += count_factor;
    let avg_relevance =
        precedents.iter().map(|p| p.relevance_score).sum::<f64>() / precedents.len() as f64;
    score += avg_relevance * 0.2;

    let score = score.clamp(0.0, 1.0);
    let label = if score >= 0.66 {
        "高い"
    } else if score >= 0.45 {
        "中程度"
    } else {
        "低い"
    };
    (score, label)
}

// ─── Markdown / text helpers (pure) ────────────────────────────────────────────────

/// Render the full human-readable Markdown summary for a prediction.
#[must_use]
pub fn render_markdown(prediction: &RulingPrediction) -> String {
    let binding_count = prediction
        .cited_precedents
        .iter()
        .filter(|p| p.is_binding)
        .count();

    let mut md = String::from("# 判決予測（生成的法解釈 / Generative Jurisprudence）\n\n");
    md.push_str(
        "> ⚖️ 本予測は司法裁量（judicial discretion）領域に関する参考情報であり、\
         最終的な判断は裁判所に委ねられます。法的助言ではありません。\n\n",
    );
    md.push_str(&format!("- 法的論点: {}\n", first_line(&prediction.query)));
    md.push_str(&format!(
        "- 推定法分野: {}\n",
        prediction.inferred_legal_area
    ));
    md.push_str(&format!(
        "- 参照判例数: {}件（うち最高裁拘束力あり: {}件）\n",
        prediction.cited_precedents.len(),
        binding_count
    ));
    md.push_str(&format!(
        "- 確信度: {}（{:.0}%）\n\n",
        prediction.confidence_label,
        prediction.confidence * 100.0
    ));

    md.push_str(prediction.reasoning.trim());
    md.push_str("\n\n## 引用判例\n");
    if prediction.cited_precedents.is_empty() {
        md.push_str(
            "（内部判例データベースから関連判例は見つかりませんでした。\
             Web調査結果を参照してください。）\n",
        );
    } else {
        for (index, precedent) in prediction.cited_precedents.iter().enumerate() {
            let binding_tag = if precedent.is_binding {
                "拘束力あり（最高裁）"
            } else {
                "参考"
            };
            md.push_str(&format!(
                "{}. **{}** — {}\n   - 判旨: {}\n   - 関連度: {:.2} / {} / {}\n",
                index + 1,
                precedent.case_name,
                precedent.citation,
                precedent.holding_summary,
                precedent.relevance_score,
                precedent.legal_area,
                binding_tag
            ));
            if !precedent.cited_statutes.is_empty() {
                md.push_str(&format!(
                    "   - 引用条文: {}\n",
                    precedent.cited_statutes.join("、")
                ));
            }
            if let Some(url) = &precedent.source_url {
                md.push_str(&format!("   - 出典検索: {}\n", url));
            }
        }
    }

    md.push_str("\n---\n*Powered by Legalis-RS Case-Law Engine + Gemini Grounded Synthesis*\n");
    md
}

/// Format articles for the synthesis prompt.
fn format_articles_for_prompt(articles: &[FullArticle]) -> String {
    if articles.is_empty() {
        return "（該当条文なし）".to_string();
    }
    articles
        .iter()
        .enumerate()
        .map(|(index, article)| {
            format!(
                "{}. {}\n{}",
                index + 1,
                article.title,
                truncate(&article.content, 400)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Format precedents for a Gemini prompt as a compact bullet list.
fn format_precedents_for_prompt(cited: &[CitedPrecedent]) -> String {
    if cited.is_empty() {
        return "（内部判例データベースに該当なし）".to_string();
    }
    cited
        .iter()
        .map(|p| {
            let statutes = if p.cited_statutes.is_empty() {
                "—".to_string()
            } else {
                p.cited_statutes.join("、")
            };
            format!(
                "- {}（{}）: {}（引用条文: {}）",
                p.case_name, p.citation, p.holding_summary, statutes
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract the body of a Markdown section identified by its `## ` heading.
fn extract_section(markdown: &str, heading: &str) -> Option<String> {
    let mut found = false;
    let mut body: Vec<&str> = Vec::new();
    for line in markdown.lines() {
        if found {
            if line.trim_start().starts_with("## ") {
                break;
            }
            body.push(line);
        } else if line.trim() == heading {
            found = true;
        }
    }
    if !found {
        return None;
    }
    let joined = body.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// First paragraph (up to a blank line) of `text`, truncated for use as a holding.
fn first_paragraph(text: &str) -> String {
    let trimmed = text.trim();
    let paragraph = trimmed
        .split("\n\n")
        .find(|p| !p.trim().is_empty())
        .unwrap_or(trimmed)
        .trim();
    truncate(paragraph, 400)
}

/// First line of `text` (used for the headline).
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text).trim()
}

/// Character-safe truncation with an ellipsis suffix.
fn truncate(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// legalis-core classification label for a [`LegalResult`].
fn classification_label(result: &LegalResult<String>) -> &'static str {
    match SectionClassification::from(result) {
        SectionClassification::Deterministic => "deterministic",
        SectionClassification::JudicialDiscretion => "judicial_discretion",
        SectionClassification::Void => "void",
    }
}

/// Extract the discretion context id from a [`LegalResult`] as a string.
fn discretion_context_id(result: &LegalResult<String>) -> String {
    match result {
        LegalResult::JudicialDiscretion { context_id, .. } => context_id.to_string(),
        _ => String::new(),
    }
}

// ─── Curated landmark-precedent corpus ─────────────────────────────────────────────

/// A real Japanese landmark precedent specification used to seed the corpus.
struct LandmarkSpec {
    id: &'static str,
    /// Popular case name (判例通称) — also used as the citation's case label,
    /// since exact docket numbers are intentionally not fabricated.
    popular_name: &'static str,
    year: i32,
    month: u32,
    day: u32,
    court_level: CourtLevel,
    location: &'static str,
    division: &'static str,
    legal_area: LegalArea,
    outcome: CaseOutcome,
    summary: &'static str,
    principle: &'static str,
    reasoning: &'static str,
    keywords: &'static [&'static str],
    cited_statutes: &'static [&'static str],
}

/// Curated set of well-known real Japanese landmark precedents across the domains
/// oxigenai covers (労働 / プライバシー・個人情報 / 不法行為 / 憲法).
///
/// Court levels, approximate years, holdings and cited statutes are realistic;
/// precise docket numbers are deliberately omitted (popular name + court + year).
const LANDMARK_SPECS: &[LandmarkSpec] = &[
    LandmarkSpec {
        id: "nihon-ensei-1975",
        popular_name: "日本食塩製造事件",
        year: 1975,
        month: 4,
        day: 25,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "第二小法廷",
        legal_area: LegalArea::Labor,
        outcome: CaseOutcome::AppealDismissed,
        summary: "使用者による普通解雇の効力が争われた事案。最高裁が解雇権濫用法理を判示した。",
        principle: "使用者の解雇権の行使も、客観的に合理的な理由を欠き社会通念上相当として是認することができない場合には、権利の濫用として無効となる。",
        reasoning: "労働者にとっての解雇の重大性に鑑み、解雇には合理的理由と社会的相当性を要するとした。この法理は後に労働契約法第16条として明文化された。",
        keywords: &["解雇", "解雇権濫用", "普通解雇", "労働契約", "権利濫用"],
        cited_statutes: &["民法第1条第3項（権利濫用）", "労働基準法"],
    },
    LandmarkSpec {
        id: "mitsubishi-jushi-1973",
        popular_name: "三菱樹脂事件",
        year: 1973,
        month: 12,
        day: 12,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "大法廷",
        legal_area: LegalArea::Constitutional,
        outcome: CaseOutcome::Remanded,
        summary: "試用期間後の本採用拒否の可否と、憲法の自由権規定の私人間効力が争われた事案。",
        principle: "憲法第14条・第19条等の自由権規定は私人相互の関係を直接規律するものではなく、企業者は契約締結の自由を有し、特定の思想・信条を理由に雇入れを拒んでも当然に違法とはいえない。",
        reasoning: "私的自治の原則のもと、基本権規定は民法第90条等の一般条項を通じて間接的に適用されるとした（間接適用説）。",
        keywords: &[
            "採用",
            "採用の自由",
            "思想信条",
            "私人間効力",
            "本採用拒否",
            "試用期間",
        ],
        cited_statutes: &["日本国憲法第14条", "日本国憲法第19条", "民法第90条"],
    },
    LandmarkSpec {
        id: "shuhoku-bus-1968",
        popular_name: "秋北バス事件",
        year: 1968,
        month: 12,
        day: 25,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "大法廷",
        legal_area: LegalArea::Labor,
        outcome: CaseOutcome::AppealDismissed,
        summary: "就業規則による定年制の新設（労働条件の不利益変更）の効力が争われた事案。",
        principle: "就業規則の変更が合理的なものである限り、個々の労働者がこれに同意しないことを理由としてその適用を拒むことは許されない。",
        reasoning: "就業規則は多数の労働者の労働条件を統一的・画一的に決定する必要から作成されるものであり、合理的な変更には拘束力が認められるとした。",
        keywords: &["就業規則", "不利益変更", "合理性", "定年制", "労働条件"],
        cited_statutes: &["労働基準法第89条", "労働基準法第93条"],
    },
    LandmarkSpec {
        id: "dentsu-2000",
        popular_name: "電通事件",
        year: 2000,
        month: 3,
        day: 24,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "第二小法廷",
        legal_area: LegalArea::Labor,
        outcome: CaseOutcome::Remanded,
        summary: "長時間労働による過労自殺について、使用者の損害賠償責任が争われた事案。",
        principle: "使用者は、業務の遂行に伴う疲労や心理的負荷等が過度に蓄積して労働者の心身の健康を損なうことがないよう注意する義務（安全配慮義務）を負う。",
        reasoning: "使用者が労働者の健康状態の悪化を認識し得たのに負担軽減措置を執らなかった場合には不法行為責任を負うとし、損害額の算定につき過失相殺の可否も判示して原審に差し戻した。",
        keywords: &[
            "過労自殺",
            "過労死",
            "安全配慮義務",
            "使用者責任",
            "損害賠償",
            "労災",
            "長時間労働",
            "不法行為",
        ],
        cited_statutes: &["民法第709条", "民法第715条", "民法第415条", "労働基準法"],
    },
    LandmarkSpec {
        id: "utage-no-ato-1964",
        popular_name: "「宴のあと」事件",
        year: 1964,
        month: 9,
        day: 28,
        court_level: CourtLevel::District,
        location: "東京",
        division: "民事部",
        legal_area: LegalArea::Civil,
        outcome: CaseOutcome::PlaintiffWins,
        summary: "小説によるプライバシー侵害が争われ、日本で初めてプライバシー権を法的権利として承認した事案。",
        principle: "私生活をみだりに公開されないという法的保障ないし権利（プライバシー権）の侵害に対しては、不法行為に基づく損害賠償が認められる。",
        reasoning: "私生活上の事実の公開が、(1)私生活上の事実らしく受け取られ、(2)一般人の感受性を基準に公開を欲しないであろう事柄で、(3)未だ一般に知られていない事柄である場合に侵害が成立するとした。",
        keywords: &["プライバシー", "私生活", "不法行為", "名誉", "損害賠償"],
        cited_statutes: &["民法第709条", "民法第710条"],
    },
    LandmarkSpec {
        id: "ishi-ni-oyogu-uo-2002",
        popular_name: "「石に泳ぐ魚」事件",
        year: 2002,
        month: 9,
        day: 24,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "第三小法廷",
        legal_area: LegalArea::Civil,
        outcome: CaseOutcome::AppealDismissed,
        summary: "モデル小説によるプライバシー・名誉侵害に対する出版差止めの可否が争われた事案。",
        principle: "人格的価値を侵害された者は人格権に基づき加害行為の差止めを求めることができ、侵害が重大で回復困難な損害を生じさせるおそれがある場合には出版の差止めが認められる。",
        reasoning: "侵害行為の対象の社会的地位や侵害の程度等を較量し、本件では出版の差止めを認めた原審の判断を是認した。",
        keywords: &[
            "プライバシー",
            "出版差止",
            "差止",
            "名誉毀損",
            "人格権",
            "表現の自由",
        ],
        cited_statutes: &["民法第709条", "民法第723条", "日本国憲法第13条"],
    },
    LandmarkSpec {
        id: "juki-net-2008",
        popular_name: "住基ネット訴訟",
        year: 2008,
        month: 3,
        day: 6,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "第一小法廷",
        legal_area: LegalArea::Constitutional,
        outcome: CaseOutcome::AppealDismissed,
        summary: "住民基本台帳ネットワークによる本人確認情報の管理・利用がプライバシー権を侵害するかが争われた事案。",
        principle: "個人に関する情報をみだりに第三者に開示又は公表されない自由は憲法第13条により保障されるが、住基ネットによる本人確認情報の管理・利用等は正当な行政目的の範囲内で行われており、プライバシー権を侵害しない。",
        reasoning: "情報の性質、管理・利用の体制及び目的の正当性を考慮し、具体的な危険が生じているとはいえないとした。",
        keywords: &["個人情報", "プライバシー", "住基ネット", "自己情報", "行政"],
        cited_statutes: &["日本国憲法第13条"],
    },
    LandmarkSpec {
        id: "benesse-2017",
        popular_name: "ベネッセ個人情報漏えい事件",
        year: 2017,
        month: 10,
        day: 23,
        court_level: CourtLevel::Supreme,
        location: "東京",
        division: "第二小法廷",
        legal_area: LegalArea::Civil,
        outcome: CaseOutcome::Remanded,
        summary: "個人情報の大規模漏えいについて、本人が被った精神的損害の賠償が争われた事案。",
        principle: "個人情報を漏えいされた本人は、プライバシーの侵害による精神的損害について不法行為に基づく損害賠償を請求し得るのであり、原審は損害の有無等につき更に審理を尽くすべきである。",
        reasoning: "精神的損害が不快感等にとどまるか否か等の事情を個別に審理する必要があるとして、損害の発生を否定した原審を破棄し差し戻した。",
        keywords: &[
            "個人情報",
            "漏えい",
            "プライバシー",
            "損害賠償",
            "慰謝料",
            "不法行為",
        ],
        cited_statutes: &["民法第709条", "個人情報の保護に関する法律"],
    },
];

/// Build the in-memory corpus from [`LANDMARK_SPECS`].
fn seed_landmark_cases() -> InMemoryCaseDatabase {
    let mut db = InMemoryCaseDatabase::new();
    for spec in LANDMARK_SPECS {
        // `add_case` only errors on backend failure, which the in-memory DB never
        // returns; ignore the always-Ok result.
        let _ = db.add_case(build_decision(spec));
    }
    db
}

/// Construct a [`CourtDecision`] from a [`LandmarkSpec`].
fn build_decision(spec: &LandmarkSpec) -> CourtDecision {
    let mut court = Court::new(spec.court_level);
    if !spec.location.is_empty() {
        court = court.with_location(spec.location);
    }
    if !spec.division.is_empty() {
        court = court.with_division(spec.division);
    }

    let mut metadata = CaseMetadata::new(
        spec.popular_name,
        ymd_utc(spec.year, spec.month, spec.day),
        court,
        spec.legal_area,
        spec.outcome,
    );
    for keyword in spec.keywords {
        metadata.add_keyword(*keyword);
    }
    for statute in spec.cited_statutes {
        metadata.add_cited_statute(*statute);
    }

    let mut decision =
        CourtDecision::new(spec.id, metadata, spec.summary).with_source_url(COURTS_SEARCH_URL);
    decision.add_holding(Holding {
        principle: spec.principle.to_string(),
        reasoning: spec.reasoning.to_string(),
        related_statutes: spec
            .cited_statutes
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        is_leading_case: true,
    });
    decision.add_party(Party {
        party_type: "上告人".to_string(),
        name: "（匿名）".to_string(),
        representative: None,
    });
    decision.add_party(Party {
        party_type: "被上告人".to_string(),
        name: "（匿名）".to_string(),
        representative: None,
    });
    decision
}

/// Build a `DateTime<Utc>` for a known-valid calendar date without panicking.
fn ymd_utc(year: i32, month: u32, day: u32) -> DateTime<Utc> {
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|naive| naive.and_utc())
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(title: &str) -> FullArticle {
        FullArticle {
            law_id: "test".to_string(),
            title: title.to_string(),
            content: "テスト条文本文".to_string(),
            unique_anchor: "Article_1".to_string(),
            anchor: None,
            url: "https://example.com".to_string(),
        }
    }

    #[test]
    fn test_corpus_seeded_with_landmark_cases() {
        let predictor = CaseLawPredictor::new();
        assert_eq!(predictor.case_count(), LANDMARK_SPECS.len());
        assert!(
            predictor.case_count() >= 6,
            "need at least 6 landmark cases"
        );
    }

    #[test]
    fn test_corpus_contains_supreme_court_cases() {
        // A broad labor search should surface the Supreme Court 日本食塩製造事件.
        let predictor = CaseLawPredictor::new();
        let results = predictor.search_local(LegalArea::Labor, &["解雇".to_string()]);
        assert!(results.iter().any(|r| r.decision.id == "nihon-ensei-1975"));
        assert!(
            results
                .iter()
                .any(|r| r.decision.is_supreme_court_decision())
        );
    }

    #[test]
    fn test_ymd_utc_year() {
        let date = ymd_utc(1975, 4, 25);
        assert_eq!(date.format("%Y-%m-%d").to_string(), "1975-04-25");
    }

    #[test]
    fn test_infer_legal_area_labor() {
        assert_eq!(
            infer_legal_area("不当解雇された場合の救済", &[]),
            LegalArea::Labor
        );
        assert_eq!(
            infer_legal_area("就業規則の不利益変更は有効か", &[]),
            LegalArea::Labor
        );
    }

    #[test]
    fn test_infer_legal_area_privacy_and_tort() {
        assert_eq!(
            infer_legal_area("個人情報の漏えいによるプライバシー侵害", &[]),
            LegalArea::Civil
        );
        assert_eq!(
            infer_legal_area("不法行為に基づく損害賠償請求", &[]),
            LegalArea::Civil
        );
    }

    #[test]
    fn test_infer_legal_area_from_domain_article_title() {
        // 労働基準法 is domain-matched by JpDomainMatcher → Labor.
        let articles = vec![article("労働基準法 第32条")];
        assert_eq!(
            infer_legal_area("時間外労働について", &articles),
            LegalArea::Labor
        );
    }

    #[test]
    fn test_infer_legal_area_default_civil() {
        assert_eq!(
            infer_legal_area("よくわからない一般的な質問", &[]),
            LegalArea::Civil
        );
    }

    #[test]
    fn test_extract_keywords_hits_query_and_titles() {
        let keywords = extract_keywords("解雇は有効か", &[article("労働契約法 第16条")]);
        assert!(keywords.iter().any(|k| k == "解雇"));
        assert!(keywords.iter().any(|k| k == "労働契約"));
    }

    #[test]
    fn test_extract_keywords_fallback_to_area() {
        // No curated token present, but classify_area finds 会社 → Commercial name.
        let keywords = extract_keywords("会社の組織再編について", &[]);
        assert!(!keywords.is_empty());
    }

    #[test]
    fn test_search_local_privacy_returns_relevant_cases() {
        let predictor = CaseLawPredictor::new();
        let results = predictor.search_local(
            LegalArea::Civil,
            &["プライバシー".to_string(), "個人情報".to_string()],
        );
        assert!(!results.is_empty());
        // Should include at least one privacy/personal-info landmark.
        assert!(results.iter().any(|r| {
            ["utage-no-ato-1964", "benesse-2017", "ishi-ni-oyogu-uo-2002"]
                .contains(&r.decision.id.as_str())
        }));
    }

    #[test]
    fn test_search_local_no_match_is_empty_not_panic() {
        let predictor = CaseLawPredictor::new();
        let results =
            predictor.search_local(LegalArea::Tax, &["全く無関係なキーワードxyz".to_string()]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rank_results_prefers_supreme_authority() {
        let predictor = CaseLawPredictor::new();
        let results = predictor.search_local(
            LegalArea::Labor,
            &["解雇".to_string(), "安全配慮義務".to_string()],
        );
        let ranked = rank_results(results);
        assert!(ranked.len() <= TOP_K);
        assert!(!ranked.is_empty());
        // Top precedent should be a binding Supreme Court decision.
        assert!(ranked[0].decision.is_supreme_court_decision());
    }

    #[test]
    fn test_to_cited_precedent_supreme() {
        let predictor = CaseLawPredictor::new();
        let results = predictor.search_local(LegalArea::Labor, &["解雇".to_string()]);
        let nihon_ensei = results
            .iter()
            .find(|r| r.decision.id == "nihon-ensei-1975")
            .expect("日本食塩製造事件 should be present");
        let cited = to_cited_precedent(nihon_ensei);
        assert_eq!(cited.case_name, "日本食塩製造事件");
        assert_eq!(cited.court, "最高裁判所");
        assert!(cited.citation.contains("最高裁判所"));
        assert!(cited.is_binding);
        assert_eq!(cited.precedent_weight, 0);
        assert!(cited.source_url.is_some());
        assert!(!cited.cited_statutes.is_empty());
    }

    #[test]
    fn test_authority_factor_ordering() {
        assert!(authority_factor(0) > authority_factor(1));
        assert!(authority_factor(1) > authority_factor(2));
        assert!(authority_factor(2) > authority_factor(3));
    }

    #[test]
    fn test_compute_confidence_with_binding_precedent() {
        let precedent = CitedPrecedent {
            id: "x".to_string(),
            case_name: "テスト事件".to_string(),
            court: "最高裁判所".to_string(),
            legal_area: "労働法".to_string(),
            citation: "最高裁判所令和2年1月1日判決 テスト事件".to_string(),
            relevance_score: 0.9,
            precedent_weight: 0,
            holding_summary: "判旨".to_string(),
            cited_statutes: vec!["民法第709条".to_string()],
            source_url: Some(COURTS_SEARCH_URL.to_string()),
            is_binding: true,
        };
        let (score, label) = compute_confidence(std::slice::from_ref(&precedent));
        assert!(score > 0.5);
        assert!(["高い", "中程度"].contains(&label));
    }

    #[test]
    fn test_compute_confidence_empty() {
        let (score, label) = compute_confidence(&[]);
        assert!(score < 0.3);
        assert_eq!(label, "低い");
    }

    #[test]
    fn test_extract_section() {
        let md = "## 予測される結論\n解雇は無効と判断される可能性が高い。\n\n## 理由（判例の射程）\n判例によれば...";
        let holding = extract_section(md, "## 予測される結論").unwrap();
        assert!(holding.contains("解雇は無効"));
        assert!(!holding.contains("判例によれば"));
        assert!(extract_section(md, "## 存在しない見出し").is_none());
    }

    #[test]
    fn test_truncate_char_safe() {
        let s = "あいうえおかきくけこ";
        assert_eq!(truncate(s, 3), "あいう…");
        assert_eq!(truncate(s, 100), s);
    }

    #[test]
    fn test_render_markdown_structure() {
        let prediction = RulingPrediction {
            query: "解雇は有効か".to_string(),
            issue: "解雇の効力".to_string(),
            inferred_legal_area: "労働法".to_string(),
            predicted_holding: "解雇は無効と判断される可能性が高い。".to_string(),
            reasoning: "## 予測される結論\n解雇は無効。\n\n## 理由（判例の射程）\n日本食塩製造事件による。\n\n## 確信度評価\n中程度。".to_string(),
            cited_precedents: vec![CitedPrecedent {
                id: "nihon-ensei-1975".to_string(),
                case_name: "日本食塩製造事件".to_string(),
                court: "最高裁判所".to_string(),
                legal_area: "労働法".to_string(),
                citation: "最高裁判所昭和50年4月25日判決 日本食塩製造事件".to_string(),
                relevance_score: 0.85,
                precedent_weight: 0,
                holding_summary: "解雇権濫用法理".to_string(),
                cited_statutes: vec!["民法第1条第3項（権利濫用）".to_string()],
                source_url: Some(COURTS_SEARCH_URL.to_string()),
                is_binding: true,
            }],
            confidence: 0.7,
            confidence_label: "高い".to_string(),
            legal_classification: "judicial_discretion".to_string(),
            context_id: "00000000-0000-0000-0000-000000000000".to_string(),
            markdown_summary: String::new(),
            usage: vec![],
        };
        let md = render_markdown(&prediction);
        assert!(md.contains("# 判決予測"));
        assert!(md.contains("推定法分野: 労働法"));
        assert!(md.contains("## 引用判例"));
        assert!(md.contains("日本食塩製造事件"));
        assert!(md.contains("拘束力あり（最高裁）"));
        assert!(md.contains(COURTS_SEARCH_URL));
    }

    #[test]
    fn test_classification_label_discretion() {
        let result: LegalResult<String> = LegalResult::JudicialDiscretion {
            issue: "x".to_string(),
            context_id: Uuid::new_v4(),
            narrative_hint: None,
        };
        assert_eq!(classification_label(&result), "judicial_discretion");
        assert!(!discretion_context_id(&result).is_empty());
    }

    #[test]
    fn test_format_precedents_for_prompt_empty() {
        assert!(format_precedents_for_prompt(&[]).contains("該当なし"));
    }

    #[test]
    fn test_format_articles_for_prompt() {
        assert!(format_articles_for_prompt(&[]).contains("該当条文なし"));
        let formatted = format_articles_for_prompt(&[article("労働基準法 第32条")]);
        assert!(formatted.contains("労働基準法 第32条"));
    }
}
