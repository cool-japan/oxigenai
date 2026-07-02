use crate::handlers::report::AppState;
use crate::verifier::jurisprudence::{CaseLawPredictor, RulingPrediction};
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use tracing::info;

/// Request body for `POST /predict-ruling`.
#[derive(Debug, Deserialize)]
pub struct PredictRequest {
    /// The legal question / fact pattern to predict a ruling for.
    /// Accepts `query` / `input_text` as aliases for convenience.
    #[serde(alias = "query", alias = "input_text")]
    pub input: String,
    /// Optional free-form description of the facts (事実関係), in Japanese.
    #[serde(default)]
    pub facts: Option<String>,
}

impl PredictRequest {
    /// Effective query analysed by the predictor — the question with any supplied
    /// fact pattern folded in so that synthesis is grounded in the user's facts.
    fn effective_query(&self) -> String {
        match self.facts.as_deref().map(str::trim) {
            Some(facts) if !facts.is_empty() => {
                format!("{}\n\n【事実関係】\n{}", self.input, facts)
            }
            _ => self.input.clone(),
        }
    }
}

/// POST /predict-ruling
///
/// Predicts a likely judicial ruling from case law (生成的法解釈).
///
/// 1. Embeds `input` via Vertex AI and retrieves matching articles from BigQuery.
/// 2. Searches a curated corpus of real Japanese landmark precedents.
/// 3. Uses Gemini web-grounded retrieval + deterministic synthesis (temp 0) to
///    predict a holding (結論), reasoning grounded in the precedents (判例の射程),
///    and a confidence assessment, flagging genuine 司法裁量.
///
/// Example request:
/// ```json
/// {
///   "input": "5年勤続の有期契約社員を経営不振で解雇できるか",
///   "facts": "業績は悪化しているが希望退職の募集など解雇回避努力は行っていない"
/// }
/// ```
pub async fn predict_ruling(
    State(ctx): State<AppState>,
    Json(req): Json<PredictRequest>,
) -> Result<Json<RulingPrediction>, (StatusCode, String)> {
    if req.input.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "「input」は空にできません。法的論点を入力してください。".to_string(),
        ));
    }

    info!(
        "POST /predict-ruling — input='{}', has_facts={}",
        req.input,
        req.facts.as_deref().is_some_and(|f| !f.trim().is_empty())
    );

    // Embed the query → BQ search → relevant law articles (same as sibling handlers).
    let embeddings = ctx
        .gemini
        .embed_texts(std::slice::from_ref(&req.input))
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

    // Convert ArticleWithSummary → FullArticle.
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

    let predictor = CaseLawPredictor::new();
    let prediction = predictor
        .predict_ruling(&req.effective_query(), &full_articles, &ctx.gemini)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("判決予測に失敗しました: {e}"),
            )
        })?;

    info!(
        "/predict-ruling: {} precedents, confidence={:.2}",
        prediction.cited_precedents.len(),
        prediction.confidence
    );

    Ok(Json(prediction))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predict_request_deserialize_input() {
        let json = r#"{"input": "解雇は有効か", "facts": "勤続5年"}"#;
        let req: PredictRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.input, "解雇は有効か");
        assert_eq!(req.facts.as_deref(), Some("勤続5年"));
    }

    #[test]
    fn test_predict_request_query_alias() {
        // `query` and `input_text` are accepted aliases for `input`.
        let req: PredictRequest = serde_json::from_str(r#"{"query": "プライバシー侵害"}"#).unwrap();
        assert_eq!(req.input, "プライバシー侵害");
        assert!(req.facts.is_none());

        let req2: PredictRequest = serde_json::from_str(r#"{"input_text": "損害賠償"}"#).unwrap();
        assert_eq!(req2.input, "損害賠償");
    }

    #[test]
    fn test_effective_query_folds_facts() {
        let req = PredictRequest {
            input: "解雇は有効か".to_string(),
            facts: Some("業績不振".to_string()),
        };
        let effective = req.effective_query();
        assert!(effective.contains("解雇は有効か"));
        assert!(effective.contains("【事実関係】"));
        assert!(effective.contains("業績不振"));
    }

    #[test]
    fn test_effective_query_without_facts() {
        let req = PredictRequest {
            input: "解雇は有効か".to_string(),
            facts: None,
        };
        assert_eq!(req.effective_query(), "解雇は有効か");

        // Whitespace-only facts are ignored.
        let req2 = PredictRequest {
            input: "解雇は有効か".to_string(),
            facts: Some("   ".to_string()),
        };
        assert_eq!(req2.effective_query(), "解雇は有効か");
    }
}
