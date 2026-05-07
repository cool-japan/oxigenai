use crate::handlers::report::AppState;
use crate::verifier::compiler::{CompileError, CompileResult, CompiledStatute, CompilerService};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Request body for POST /compile.
///
/// Exactly one of `xml` or `query` must be provided.
#[derive(Debug, Deserialize)]
pub struct CompileRequest {
    /// Raw e-Gov XML to compile (Mode A).
    #[serde(default)]
    pub xml: Option<String>,
    /// Natural language query to find law articles (Mode B).
    #[serde(default)]
    pub query: Option<String>,
}

/// Per-statute item in the compile response.
#[derive(Debug, Serialize)]
pub struct CompiledStatuteJson {
    pub id: String,
    pub title: String,
    pub dsl: String,
    pub source: String,
}

impl From<CompiledStatute> for CompiledStatuteJson {
    fn from(s: CompiledStatute) -> Self {
        Self {
            id: s.id,
            title: s.title,
            dsl: s.dsl,
            source: s.source,
        }
    }
}

/// Response body for POST /compile.
#[derive(Debug, Serialize)]
pub struct CompileResponse {
    pub law_title: String,
    pub law_num: String,
    pub dsl_text: String,
    pub statutes: Vec<CompiledStatuteJson>,
    pub article_count: usize,
    pub statute_count: usize,
    pub warnings: Vec<String>,
}

impl From<CompileResult> for CompileResponse {
    fn from(r: CompileResult) -> Self {
        let statute_count = r.statutes.len();
        Self {
            law_title: r.law_title,
            law_num: r.law_num,
            dsl_text: r.dsl_text,
            statutes: r.statutes.into_iter().map(Into::into).collect(),
            article_count: r.article_count,
            statute_count,
            warnings: r.warnings,
        }
    }
}

/// POST /compile
///
/// Compiles Japanese law into Legalis DSL.
///
/// **Mode A** (XML input): Parses raw e-Gov XML using `EGovLawParser`,
/// converts articles to statutes via `EGovLaw::to_statutes()`, then formats as DSL.
///
/// **Mode B** (query input): Embeds the query, retrieves matching articles from
/// BigQuery, then converts using `StatuteBridge::convert_articles_domain_only()`.
///
/// Example (Mode B):
/// ```json
/// { "query": "労働基準法" }
/// ```
///
/// Example (Mode A):
/// ```json
/// { "xml": "<Law>...</Law>" }
/// ```
pub async fn compile(
    State(ctx): State<AppState>,
    Json(req): Json<CompileRequest>,
) -> Result<Json<CompileResponse>, (StatusCode, String)> {
    match (&req.xml, &req.query) {
        // Mode A: XML compilation — no BQ/Gemini needed
        (Some(xml), _) => {
            info!("POST /compile [xml] — {} bytes", xml.len());
            let result = CompilerService::compile_xml(xml).map_err(|e| match e {
                CompileError::XmlParse(msg) => (
                    StatusCode::BAD_REQUEST,
                    format!("XML解析エラー: {msg}"),
                ),
                CompileError::InvalidDocument(msg) => (
                    StatusCode::BAD_REQUEST,
                    format!("不正なXML文書: {msg}"),
                ),
                CompileError::NoStatutes => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "条文を形式化できませんでした。XMLに有効な条文が含まれていない可能性があります。"
                        .to_string(),
                ),
            })?;
            info!(
                "/compile [xml]: {} statutes compiled from {} articles",
                result.statutes.len(),
                result.article_count
            );
            Ok(Json(result.into()))
        }

        // Mode B: Query-based compilation via BQ
        (None, Some(query)) => {
            info!("POST /compile [query] — '{}'", query);
            let embeddings = ctx
                .gemini
                .embed_texts(std::slice::from_ref(query))
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("埋め込みに失敗しました: {e}"),
                    )
                })?;

            let articles = ctx
                .bq
                .get_articles_by_nearest_law(&embeddings)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("BQ検索に失敗しました: {e}"),
                    )
                })?;

            // Convert ArticleWithSummary → FullArticle
            let full_articles: Vec<crate::models::law::FullArticle> = articles
                .iter()
                .filter_map(|a| {
                    let content = a.content.as_ref().or(a.article_summary.as_ref())?;
                    let url = crate::models::law::FullArticle::build_egov_url(&a.law_id, None);
                    Some(crate::models::law::FullArticle {
                        law_id: a.law_id.clone(),
                        title: a.law_title.clone(),
                        content: content.clone(),
                        unique_anchor: a.unique_anchor.clone(),
                        anchor: None,
                        url,
                    })
                })
                .collect();

            let result = CompilerService::compile_articles(&full_articles);
            info!(
                "/compile [query]: {} statutes compiled from {} articles",
                result.statutes.len(),
                result.article_count
            );
            Ok(Json(result.into()))
        }

        // Neither field provided
        (None, None) => Err((
            StatusCode::BAD_REQUEST,
            "`xml` または `query` のどちらかを指定してください。".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verifier::compiler::CompiledStatute;

    #[test]
    fn test_compile_request_xml_deserialize() {
        let json = r#"{"xml": "<Law>...</Law>"}"#;
        let req: CompileRequest = serde_json::from_str(json).unwrap();
        assert!(req.xml.is_some());
        assert!(req.query.is_none());
    }

    #[test]
    fn test_compile_request_query_deserialize() {
        let json = r#"{"query": "労働基準法"}"#;
        let req: CompileRequest = serde_json::from_str(json).unwrap();
        assert!(req.query.is_some());
        assert!(req.xml.is_none());
    }

    #[test]
    fn test_compile_response_serialize() {
        let resp = CompileResponse {
            law_title: "労働基準法".to_string(),
            law_num: "昭和二十二年法律第四十九号".to_string(),
            dsl_text: "STATUTE LSA_Art32: \"労働時間\" { ... }".to_string(),
            statutes: vec![CompiledStatuteJson {
                id: "LSA_Art32".to_string(),
                title: "労働時間".to_string(),
                dsl: "STATUTE LSA_Art32: \"労働時間\" { WHEN AGE >= 18 THEN OBLIGATION \"...\" }"
                    .to_string(),
                source: "xml_native".to_string(),
            }],
            article_count: 1,
            statute_count: 1,
            warnings: vec![],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("LSA_Art32"));
        assert!(json.contains("xml_native"));
        assert!(json.contains("statute_count"));
    }

    #[test]
    fn test_compiled_statute_into_json() {
        let s = CompiledStatute {
            id: "test".to_string(),
            title: "テスト".to_string(),
            dsl: "STATUTE test { ... }".to_string(),
            source: "domain".to_string(),
        };
        let j: CompiledStatuteJson = s.into();
        assert_eq!(j.id, "test");
        assert_eq!(j.source, "domain");
    }
}
