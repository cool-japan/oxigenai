use crate::handlers::report::AppState;
use crate::verifier::formalize::{FactEvaluation, FormalizeService, UserFacts};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Request body for POST /formalize.
#[derive(Debug, Deserialize)]
pub struct FormalizeRequest {
    /// Japanese legal query to find relevant statutes.
    pub query: String,
    /// User's situational facts to evaluate against found statutes.
    pub facts: UserFacts,
}

/// Response body for POST /formalize.
#[derive(Debug, Serialize)]
pub struct FormalizeResponse {
    /// Per-statute evaluation results.
    pub evaluations: Vec<FactEvaluation>,
    /// Number of statutes evaluated.
    pub total_evaluated: usize,
    /// Number of statutes that apply to the given facts.
    pub applicable_count: usize,
}

/// POST /formalize
///
/// Evaluates Japanese law statutes against user-supplied facts.
/// Uses the legalis-core EntailmentEngine for deterministic legal reasoning.
///
/// Example request:
/// ```json
/// {
///   "query": "有期雇用5年超の無期転換権について",
///   "facts": {
///     "age": 35,
///     "attributes": {"employment_type": "fixed_term", "years_employed": "6"},
///     "description": "5年以上の有期雇用契約を更新してきた"
///   }
/// }
/// ```
pub async fn formalize(
    State(ctx): State<AppState>,
    Json(req): Json<FormalizeRequest>,
) -> Result<Json<FormalizeResponse>, (StatusCode, String)> {
    info!(
        "POST /formalize: query='{}', facts='{}'",
        req.query, req.facts.description
    );

    // Embed query to find relevant law articles
    let embeddings = ctx
        .gemini
        .embed_texts(std::slice::from_ref(&req.query))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Embedding failed: {e}"),
            )
        })?;

    let articles = ctx
        .bq
        .get_articles_by_nearest_law(&embeddings)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("BQ retrieval failed: {e}"),
            )
        })?;

    // Convert ArticleWithSummary → FullArticle format expected by FormalizeService
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

    // Evaluate statutes against user facts
    let service = FormalizeService::new();
    let evaluations = service.evaluate(&full_articles, &req.facts);
    let applicable_count = evaluations.iter().filter(|e| e.applies).count();
    let total_evaluated = evaluations.len();

    info!(
        "/formalize: {} statutes evaluated, {} applicable",
        total_evaluated, applicable_count
    );

    Ok(Json(FormalizeResponse {
        evaluations,
        total_evaluated,
        applicable_count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formalize_request_deserialize() {
        let json = r#"{
            "query": "有期雇用5年超の無期転換権",
            "facts": {
                "age": 35,
                "attributes": {"employment_type": "fixed_term", "years_employed": "6"},
                "description": "5年以上の有期雇用契約を更新してきた"
            }
        }"#;
        let req: FormalizeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "有期雇用5年超の無期転換権");
        assert_eq!(req.facts.age, Some(35));
        assert_eq!(
            req.facts
                .attributes
                .get("employment_type")
                .map(|s| s.as_str()),
            Some("fixed_term")
        );
    }

    #[test]
    fn test_formalize_response_serialize() {
        let resp = FormalizeResponse {
            evaluations: vec![FactEvaluation {
                statute_id: "LCA_Art18".to_string(),
                statute_title: "無期転換ルール".to_string(),
                result_type: "deterministic".to_string(),
                effect: Some("無期転換申込権が発生".to_string()),
                explanation: "条文の適用条件が充足されています。機械的に適用されます。".to_string(),
                applies: true,
            }],
            total_evaluated: 1,
            applicable_count: 1,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("LCA_Art18"));
        assert!(json.contains("deterministic"));
    }
}
