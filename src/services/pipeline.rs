use crate::config::AppConfig;
use crate::error::{OxigenError, Result};
use crate::models::law::{ArticleWithSummary, FullArticle, ReferenceItem, WebHit};
use crate::models::response::UsageSummaryEntry;
use crate::prompts::builder::{
    build_divergence_warning, build_mentioned_articles_prefix, build_report_prompt,
    build_substitution_warning, expand_law_names_with_ordinances, extract_law_names_from_query,
    extract_urls_from_query, query_has_url,
};
use crate::services::bq_retriever::BigQueryRetriever;
use crate::services::gemini_client::{GeminiService, GenerationConfig, Grounding};
use crate::services::report_utils::{
    build_citations_section, convert_citation_to_external_link, filter_references_by_citations,
    format_reference_for_prompt, sanitize_mermaid_content,
};
use crate::services::usage_tracker::UsageTracker;
use crate::verifier::integration::LegalVerifier;
use chrono::Local;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

// Maximum number of articles to include in the report
const MAX_SELECTED_ARTICLES: usize = 20;
// Trigger AI article selection if more than this many articles found
const ARTICLE_SELECTION_THRESHOLD: usize = 5;
// Max output tokens for report generation
const REPORT_MAX_OUTPUT_TOKENS: u32 = 8192;
// Max output tokens for law name estimation
const ESTIMATION_MAX_OUTPUT_TOKENS: u32 = 2048;

static CODEBLOCK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^```(?:json)?\s*\n?|```\s*$").unwrap());

static JSON_EXTRACT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)\{[^{}]*"law_names"[^{}]*\[[^\]]*\][^{}]*\}"#).unwrap());

// Law name patterns for Stage 3 extraction
static LAW_PATTERN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([^\u3002\u3001\n]*(?:\u6cd5|\u898f\u5247|\u7701\u4ee4|\u653f\u4ee4|\u6761\u4f8b)[^\u3002\u3001\n]*)").unwrap()
});

// Markdown bold law name patterns for Stage 4
static MD_BOLD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\*\*([^*]*(?:法律|法|規則|省令|政令|条例)[^*]*)\*\*").unwrap());

static MD_ITALIC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\*([^*]*(?:法律|法|規則|省令|政令|条例)[^*]*)\*").unwrap());

static PAREN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*[（(][^)）]*[）)]").unwrap());

/// Application context shared across pipeline runs.
pub struct PipelineContext {
    pub gemini: Arc<GeminiService>,
    pub bq: Arc<BigQueryRetriever>,
    pub config: Arc<AppConfig>,
    pub verifier: Arc<LegalVerifier>,
}

/// Generate a complete legal report for the given query.
/// This is the main entry point, equivalent to `generate_law_report` in law_report_pipeline.py.
pub async fn generate_law_report(
    query: &str,
    ctx: &PipelineContext,
) -> Result<(String, Vec<UsageSummaryEntry>)> {
    let tracker = Arc::new(Mutex::new(UsageTracker::new()));
    let has_url = query_has_url(query);
    let query_urls = extract_urls_from_query(query);

    info!(
        "Starting law report pipeline for query ({} chars)",
        query.len()
    );

    // Step 1: Extract law names from query text (regex)
    let query_law_names = extract_law_names_from_query(query);
    debug!("Extracted {} law names from query", query_law_names.len());

    // Step 2: Estimate law names with Gemini + web grounding (4-stage fallback)
    let (estimated_law_names, raw_web_hits) =
        estimate_law_names(query, ctx, &tracker, has_url).await;

    if estimated_law_names.is_empty() {
        warn!("Law name estimation returned empty result");
    }

    // Step 3: Expand law names with 施行令/施行規則
    let search_law_names = expand_law_names_with_ordinances(&estimated_law_names);
    debug!(
        "Expanded to {} law names for search",
        search_law_names.len()
    );

    // Step 4: Parallel execution: BQ search + redirect resolution + URL fetch
    let (bq_articles, resolved_web_hits, url_hits) = tokio::join!(
        search_articles(&search_law_names, ctx),
        ctx.gemini.resolve_redirect_web_hits(raw_web_hits),
        fetch_url_hits(query_urls, &ctx.gemini)
    );

    debug!(
        "BQ found {} articles, {} web hits, {} URL hits",
        bq_articles.len(),
        resolved_web_hits.len(),
        url_hits.len()
    );

    // Step 5: Build substitution/divergence warnings
    let substitution_warning = build_substitution_warning(&query_law_names, &estimated_law_names);
    let divergence_warning = build_divergence_warning(&estimated_law_names, &bq_articles);

    // Step 6: AI article selection (if > THRESHOLD articles)
    let selected_articles = select_articles(query, bq_articles, ctx, &tracker).await;

    // Step 7: Convert to FullArticle format
    let final_articles = to_full_articles(&selected_articles);
    debug!("Converted {} articles to FullArticle", final_articles.len());

    // Step 8: NEW — Legalis-RS + OxiZ SMT contradiction detection
    let contradiction_prefix = ctx
        .verifier
        .build_contradiction_prefix(&final_articles, &ctx.gemini)
        .await;
    debug!(
        "Contradiction detection: has_contradictions={}",
        contradiction_prefix.has_contradictions
    );

    // Step 9: Build references text for Gemini prompt
    let mut all_refs: Vec<ReferenceItem> = vec![];

    // URL hits first (highest priority for URL-based queries)
    for hit in &url_hits {
        all_refs.push(ReferenceItem::WebHit(hit.clone()));
    }
    // e-Gov articles
    for article in &final_articles {
        all_refs.push(ReferenceItem::Article(article.clone()));
    }
    // Web search hits
    for hit in &resolved_web_hits {
        if !all_refs.iter().any(|r| r.url() == hit.url) {
            all_refs.push(ReferenceItem::WebHit(hit.clone()));
        }
    }

    let mut references_text = build_references_text(&all_refs);

    // Prepend warnings
    let mut prefix_parts = vec![];
    if !contradiction_prefix.text.is_empty() {
        prefix_parts.push(contradiction_prefix.text.clone());
    }
    if !substitution_warning.is_empty() {
        prefix_parts.push(substitution_warning);
    }
    if !divergence_warning.is_empty() {
        prefix_parts.push(divergence_warning);
    }

    // Prepend mentioned articles verification
    let mentioned_prefix = build_mentioned_articles_prefix(query, &final_articles);
    if !mentioned_prefix.is_empty() {
        prefix_parts.push(mentioned_prefix);
    }

    if !prefix_parts.is_empty() {
        references_text = format!("{}\n{}", prefix_parts.join("\n"), references_text);
    }

    // Step 10: Generate complete report with Gemini
    let report_grounding = if has_url {
        Grounding::UrlContext
    } else {
        Grounding::None
    };
    let raw_report =
        generate_complete_report(query, &references_text, ctx, &tracker, report_grounding).await?;

    // Step 11: NEW — Legalis-RS annotation (adds 法的整合性検証 section)
    let annotated = ctx
        .verifier
        .annotate_report(&raw_report, &final_articles, &ctx.gemini)
        .await;
    let report_with_verification = annotated.report_text;

    // Step 12: Finalize — citation filtering, external links, mermaid, 出典 section
    let final_report = finalize_report(&report_with_verification, &all_refs);

    let usage_summary = tracker.lock().await.get_usage_summary();
    info!("Pipeline complete. Report: {} chars", final_report.len());

    Ok((final_report, usage_summary))
}

/// Estimate law names using Gemini web grounding with 4-stage JSON fallback.
/// Mirrors `_estimate_law_names` from law_report_pipeline.py.
async fn estimate_law_names(
    query: &str,
    ctx: &PipelineContext,
    tracker: &Arc<Mutex<UsageTracker>>,
    has_url: bool,
) -> (Vec<String>, Vec<WebHit>) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let url_instruction = if has_url {
        "クエリにURLが含まれる場合はそのURLの内容を必ず読み取り、内容に基づいて法令名を特定すること。"
    } else {
        ""
    };

    let system_instruction = format!(
        "本日の日付は {} です。{}クエリに関連する日本の法令を調査し、関連する法令名を以下のJSON形式で回答してください。\
        調査の際はe-Govや各省庁の公式サイト（.go.jpドメイン）を優先して参照してください。\
        必ず有効なJSONのみを出力し、説明文やマークダウンは一切含めないでください：\
        {{\"law_names\": [\"法令名1\", \"法令名2\", \"法令名3\"]}}。\
        【重要1】廃止・失効した法令は絶対に含めないこと。\
        【重要2】通称・略称の場合、正式名称が確実に特定できる場合のみ採用すること。",
        today, url_instruction
    );

    let input_text = format!(
        "以下のクエリに関連する日本の法令名を調査して、JSON形式で回答してください。説明文は不要です。JSONのみ出力してください。\n\nクエリ: {}",
        query
    );

    let grounding = if has_url {
        Grounding::UrlContext
    } else {
        Grounding::WebSearch
    };
    let gen_config = GenerationConfig {
        temperature: 0.0,
        max_output_tokens: ESTIMATION_MAX_OUTPUT_TOKENS,
        top_p: 1.0,
        top_k: 1,
        candidate_count: 1,
    };

    let response = match ctx
        .gemini
        .call_with_grounding(&input_text, &system_instruction, grounding, &gen_config)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("Law name estimation failed: {}", e);
            return (vec![], vec![]);
        }
    };

    // Record usage
    if let Some(usage) = response.usage {
        tracker.lock().await.add_usage(usage);
    }

    let web_hits = response.grounding_hits;
    let response_text = response.text;
    debug!(
        "Law name estimation response: {} chars",
        response_text.len()
    );

    // 4-stage JSON fallback parsing
    parse_law_names_from_response(&response_text, web_hits)
}

/// Parse law names from Gemini response with 4-stage fallback.
/// Mirrors the 4-stage parsing in `_estimate_law_names` from law_report_pipeline.py.
fn parse_law_names_from_response(
    response_text: &str,
    web_hits: Vec<WebHit>,
) -> (Vec<String>, Vec<WebHit>) {
    // Stage 1: Direct JSON parsing (strip ```json``` wrapper)
    let stripped = CODEBLOCK_RE.replace_all(response_text.trim(), "");
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&stripped)
        && let Some(names_json) = value["law_names"].as_array()
    {
        let names: Vec<String> = names_json
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        if !names.is_empty() {
            debug!("Stage 1 success: {} law names", names.len());
            return (names, web_hits);
        }
    }

    // Stage 2: Regex-extract JSON object containing "law_names"
    for cap in JSON_EXTRACT_RE.find_iter(response_text) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(cap.as_str())
            && let Some(names_json) = value["law_names"].as_array()
        {
            let names: Vec<String> = names_json
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            if !names.is_empty() {
                debug!("Stage 2 success: {} law names", names.len());
                return (names, web_hits);
            }
        }
    }

    // Stage 3: Regex extraction of law name patterns from text
    let names: Vec<String> = LAW_PATTERN_RE
        .find_iter(response_text)
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| {
            let len = s.chars().count();
            len > 3 && len < 50
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .take(10)
        .collect();

    if !names.is_empty() {
        debug!("Stage 3 success: {} law names", names.len());
        return (names, web_hits);
    }

    // Stage 4: Markdown bold/italic extraction
    let mut md_names: Vec<String> = vec![];
    for re in [&*MD_BOLD_RE, &*MD_ITALIC_RE] {
        for cap in re.captures_iter(response_text) {
            let raw = &cap[1];
            let cleaned = PAREN_RE.replace_all(raw, "").trim().to_string();
            let cleaned = cleaned
                .trim_matches(&[' ', '*', '-', '・'] as &[_])
                .to_string();
            let len = cleaned.chars().count();
            if (4..=60).contains(&len) {
                md_names.push(cleaned);
            }
        }
        if !md_names.is_empty() {
            break;
        }
    }

    md_names.dedup();
    let md_names: Vec<String> = md_names.into_iter().take(10).collect();

    if !md_names.is_empty() {
        debug!("Stage 4 success: {} law names", md_names.len());
        (md_names, web_hits)
    } else {
        warn!("All 4 stages failed to extract law names");
        (vec![], web_hits)
    }
}

/// Search for articles by law name using BigQuery with fallback.
/// Embeds law names via Vertex AI first, then passes vectors to VECTOR_SEARCH.
async fn search_articles(law_names: &[String], ctx: &PipelineContext) -> Vec<ArticleWithSummary> {
    if law_names.is_empty() {
        return vec![];
    }

    // Generate embeddings via Vertex AI — avoids ML.GENERATE_EMBEDDING in BQ
    // (synchronous jobs.query does not support BigQuery ML remote model calls).
    let embeddings = match ctx.gemini.embed_texts(law_names).await {
        Ok(e) => e,
        Err(e) => {
            warn!("Embedding failed: {}", e);
            return vec![];
        }
    };

    match ctx.bq.get_articles_by_nearest_law(&embeddings).await {
        Ok(articles) if !articles.is_empty() => {
            info!("BQ found {} articles", articles.len());
            articles
        }
        Ok(_) => {
            warn!("No articles found. Trying broader search...");
            let broader: Vec<String> = law_names
                .iter()
                .cloned()
                .chain(["法律".to_string(), "規則".to_string(), "政令".to_string()])
                .collect();
            let broad_embeddings = match ctx.gemini.embed_texts(&broader).await {
                Ok(e) => e,
                Err(e) => {
                    warn!("Broader embedding failed: {}", e);
                    return vec![];
                }
            };
            ctx.bq
                .get_articles_by_nearest_law(&broad_embeddings)
                .await
                .unwrap_or_default()
        }
        Err(e) => {
            warn!("BigQuery search failed: {}", e);
            vec![]
        }
    }
}

/// Select the most relevant articles using AI (triggered when > THRESHOLD articles).
/// Mirrors `_select_articles` from law_report_pipeline.py.
async fn select_articles(
    query: &str,
    articles: Vec<ArticleWithSummary>,
    ctx: &PipelineContext,
    tracker: &Arc<Mutex<UsageTracker>>,
) -> Vec<ArticleWithSummary> {
    if articles.len() <= ARTICLE_SELECTION_THRESHOLD {
        return articles;
    }

    info!("AI selecting from {} articles...", articles.len());

    let summary_list: String = articles
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let summary = a.article_summary.as_deref().unwrap_or("概要なし");
            format!("{}. {} - {}", i + 1, a.law_title, summary)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let input_text = format!("元のクエリ: {}\n\n条文概要リスト:\n{}", query, summary_list);

    let gen_config = GenerationConfig {
        temperature: 0.0,
        max_output_tokens: 8192,
        top_p: 1.0,
        top_k: 1,
        candidate_count: 1,
    };

    let system_instruction = crate::prompts::templates::PROMPT_SELECT_RELEVANT_ARTICLES;

    match ctx
        .gemini
        .call_strict(&input_text, system_instruction, &gen_config)
        .await
    {
        Ok(response) => {
            if let Some(usage) = response.usage {
                tracker.lock().await.add_usage(usage);
            }
            let selected = parse_ai_selection(&response.text, articles.len());
            if selected.is_empty() {
                warn!("AI article selection parsing failed, using all articles");
                articles
            } else {
                info!("AI selected {} articles", selected.len());
                selected
                    .into_iter()
                    .filter_map(|i| articles.get(i.saturating_sub(1)).cloned())
                    .collect()
            }
        }
        Err(e) => {
            warn!("AI article selection failed: {}, using all articles", e);
            articles
        }
    }
}

/// Parse AI article selection response.
/// Mirrors `_parse_ai_selection` from law_report_pipeline.py.
fn parse_ai_selection(selection_str: &str, max_index: usize) -> Vec<usize> {
    let mut indices = std::collections::HashSet::new();

    // Try comma-separated format: "1, 3, 5, 7"
    for part in selection_str.split([',', '\n']) {
        let trimmed = part.trim();
        if let Ok(n) = trimmed.parse::<usize>() {
            if n >= 1 && n <= max_index {
                indices.insert(n);
            }
        } else {
            // Try "1." or "1)" format
            let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<usize>()
                && n >= 1
                && n <= max_index
            {
                indices.insert(n);
            }
        }
    }

    if indices.is_empty() {
        // Fallback: distribute evenly
        if max_index <= 3 {
            (1..=max_index).collect()
        } else {
            vec![1, max_index / 2, max_index]
        }
    } else {
        let mut result: Vec<usize> = indices.into_iter().collect();
        result.sort();
        result.truncate(MAX_SELECTED_ARTICLES);
        result
    }
}

/// Convert ArticleWithSummary to FullArticle format.
/// Mirrors `_to_full_articles` from law_report_pipeline.py.
fn to_full_articles(articles: &[ArticleWithSummary]) -> Vec<FullArticle> {
    articles
        .iter()
        .filter_map(|a| {
            let content = a.content.as_ref().or(a.article_summary.as_ref())?;
            let url = FullArticle::build_egov_url(&a.law_id, None);
            Some(FullArticle {
                law_id: a.law_id.clone(),
                title: a.law_title.clone(),
                content: content.clone(),
                unique_anchor: a.unique_anchor.clone(),
                anchor: None,
                url,
            })
        })
        .collect()
}

/// Fetch page info for URLs found in the query.
/// Mirrors `_build_url_web_hits` from law_report_pipeline.py.
async fn fetch_url_hits(urls: Vec<String>, gemini: &GeminiService) -> Vec<WebHit> {
    let mut hits = vec![];
    for url in urls {
        let (title, final_url) = gemini.fetch_page_info(&url).await;
        hits.push(WebHit {
            title,
            snippet: String::new(),
            url: final_url,
        });
    }
    hits
}

/// Build the references text string for the Gemini prompt.
fn build_references_text(refs: &[ReferenceItem]) -> String {
    refs.iter()
        .enumerate()
        .map(|(i, item)| format_reference_for_prompt(i + 1, item))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// Generate the complete report using Gemini.
/// Mirrors `_generate_complete_report` from law_report_pipeline.py.
async fn generate_complete_report(
    query: &str,
    references_text: &str,
    ctx: &PipelineContext,
    tracker: &Arc<Mutex<UsageTracker>>,
    grounding: Grounding,
) -> Result<String> {
    let input_text = build_report_prompt(query, references_text);
    let system_instruction = ctx.config.system_instruction.clone();

    let gen_config = GenerationConfig {
        temperature: 0.0,
        max_output_tokens: REPORT_MAX_OUTPUT_TOKENS,
        top_p: 1.0,
        top_k: 1,
        candidate_count: 1,
    };

    info!(
        "Generating complete report ({} char prompt)...",
        input_text.len()
    );

    let response = ctx
        .gemini
        .call_for_report(&input_text, &system_instruction, grounding, &gen_config)
        .await
        .map_err(|e| OxigenError::Pipeline(format!("Report generation failed: {e}")))?;

    if let Some(usage) = response.usage {
        tracker.lock().await.add_usage(usage);
    }

    // Strip any preamble before the first heading
    let text = &response.text;
    let report = if let Some(pos) = text.find("# ") {
        text[pos..].to_string()
    } else {
        text.clone()
    };

    Ok(report)
}

/// Finalize the report: filter citations, convert to links, sanitize mermaid, add 出典.
/// Mirrors `_finalize_report` from law_report_pipeline.py.
fn finalize_report(report_text: &str, all_refs: &[ReferenceItem]) -> String {
    // Filter to only cited references
    let cited_refs = filter_references_by_citations(report_text, all_refs);

    // Convert [n] citations to external links
    let linked_report = convert_citation_to_external_link(report_text, &cited_refs);

    // Sanitize Mermaid content
    let sanitized = sanitize_mermaid_content(&linked_report);

    // Append 出典 section
    let citations_section = build_citations_section(&cited_refs);

    format!("{}{}", sanitized, citations_section)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ai_selection_comma_separated() {
        let result = parse_ai_selection("1, 3, 5, 7", 10);
        assert!(result.contains(&1));
        assert!(result.contains(&3));
        assert!(result.contains(&5));
        assert!(result.contains(&7));
    }

    #[test]
    fn test_parse_ai_selection_numbered_list() {
        let result = parse_ai_selection("1. 個人情報保護法\n3. 著作権法\n5. 民法", 10);
        assert!(result.contains(&1));
        assert!(result.contains(&3));
        assert!(result.contains(&5));
    }

    #[test]
    fn test_parse_ai_selection_fallback() {
        let result = parse_ai_selection("no numbers here", 10);
        // Fallback: [1, 5, 10]
        assert!(!result.is_empty());
        assert!(result.len() <= 3);
    }

    #[test]
    fn test_parse_ai_selection_max_20() {
        let long_list: String = (1..=30)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let result = parse_ai_selection(&long_list, 30);
        assert!(result.len() <= MAX_SELECTED_ARTICLES);
    }

    #[test]
    fn test_parse_law_names_stage1() {
        let response = r#"{"law_names": ["個人情報保護法", "著作権法"]}"#;
        let (names, _) = parse_law_names_from_response(response, vec![]);
        assert_eq!(names, vec!["個人情報保護法", "著作権法"]);
    }

    #[test]
    fn test_parse_law_names_stage1_with_codeblock() {
        let response = "```json\n{\"law_names\": [\"民法\"]}\n```";
        let (names, _) = parse_law_names_from_response(response, vec![]);
        assert_eq!(names, vec!["民法"]);
    }

    #[test]
    fn test_to_full_articles_skip_no_content() {
        let articles = vec![
            ArticleWithSummary {
                law_num: "test".to_string(),
                law_id: "test_id".to_string(),
                law_title: "テスト法".to_string(),
                unique_anchor: "Main_Article_1".to_string(),
                article_summary: None,
                content: None, // No content
                is_summary_only: false,
            },
            ArticleWithSummary {
                law_num: "test2".to_string(),
                law_id: "test_id2".to_string(),
                law_title: "テスト法2".to_string(),
                unique_anchor: "Main_Article_1".to_string(),
                article_summary: Some("概要".to_string()),
                content: Some("本文".to_string()),
                is_summary_only: false,
            },
        ];
        let full = to_full_articles(&articles);
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].title, "テスト法2");
    }

    #[test]
    fn test_finalize_report_adds_citations() {
        use crate::models::law::{FullArticle, ReferenceItem};
        let article = FullArticle {
            law_id: "test".to_string(),
            title: "テスト法 第1条".to_string(),
            content: "テスト条文".to_string(),
            unique_anchor: "Main_Article_1".to_string(),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/test".to_string(),
        };
        let refs = vec![ReferenceItem::Article(article)];
        let report = "# テスト\n\nこれは [1] を参照した内容です。";
        let result = finalize_report(report, &refs);
        assert!(result.contains("## 出典"));
    }
}
