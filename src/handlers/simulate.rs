use crate::handlers::report::AppState;
use crate::verifier::dsl_bridge::StatuteBridge;
use crate::verifier::simulator::{
    SimulationConfig, SimulationResult, SimulatorError, SimulatorService, StatuteDetail,
    jp_2024_profile,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Request body for POST /simulate.
#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
    /// Natural language query to find relevant statutes.
    pub query: String,
    /// Number of simulated agents (default: 1000, max: 10_000).
    #[serde(default = "default_population_size")]
    pub population_size: usize,
    /// Demographic profile: "jp_2024" (default, only option for now).
    #[serde(default = "default_profile")]
    pub profile: String,
}

fn default_population_size() -> usize {
    1000
}

fn default_profile() -> String {
    "jp_2024".to_string()
}

/// Response body for POST /simulate.
#[derive(Debug, Serialize)]
pub struct SimulateResponse {
    pub query: String,
    pub population_size: usize,
    pub statute_count: usize,
    pub total_applications: usize,
    pub deterministic_count: usize,
    pub discretion_count: usize,
    pub void_count: usize,
    pub deterministic_ratio: f64,
    pub discretion_ratio: f64,
    pub statute_details: Vec<StatuteDetail>,
    pub markdown_summary: String,
}

impl SimulateResponse {
    fn from_result(query: String, result: SimulationResult) -> Self {
        Self {
            query,
            population_size: result.population_size,
            statute_count: result.statute_count,
            total_applications: result.metrics.total_applications,
            deterministic_count: result.metrics.deterministic_count,
            discretion_count: result.metrics.discretion_count,
            void_count: result.metrics.void_count,
            deterministic_ratio: result.metrics.deterministic_ratio(),
            discretion_ratio: result.metrics.discretion_ratio(),
            statute_details: result.statute_details,
            markdown_summary: result.markdown_summary,
        }
    }
}

/// POST /simulate
///
/// Runs a population-level policy simulation using Legalis-Sim.
///
/// 1. Embeds `query` via Vertex AI and retrieves matching articles from BigQuery
/// 2. Converts articles to `Statute` objects (domain matching for known laws, fallback for others)
/// 3. Generates a Japanese demographic population (`jp_2024_profile`)
/// 4. Runs `SimEngine::run_simulation()` — applies all statutes to all agents
/// 5. Returns `SimulationMetrics` + per-statute breakdown + Markdown summary
///
/// Example:
/// ```json
/// {
///   "query": "労働基準法の適用シミュレーション",
///   "population_size": 500
/// }
/// ```
pub async fn simulate(
    State(ctx): State<AppState>,
    Json(req): Json<SimulateRequest>,
) -> Result<Json<SimulateResponse>, (StatusCode, String)> {
    // Cap population size to prevent excessive resource use
    let population_size = req.population_size.clamp(1, 10_000);

    info!(
        "POST /simulate — query='{}', population={}, profile='{}'",
        req.query, population_size, req.profile
    );

    // Embed query → BQ search → articles
    let embeddings = ctx
        .gemini
        .embed_texts(std::slice::from_ref(&req.query))
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

    // Convert articles → statutes via domain matching (no Gemini call for speed)
    let article_statutes = StatuteBridge::convert_articles_domain_only(&full_articles);

    // Deduplicate statutes by ID
    let mut seen = std::collections::HashSet::new();
    let statutes: Vec<legalis_core::Statute> = article_statutes
        .into_iter()
        .filter_map(|a| {
            if seen.insert(a.statute.id.clone()) {
                Some(a.statute)
            } else {
                None
            }
        })
        .collect();

    if statutes.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "シミュレーション対象の条文が見つかりませんでした。クエリを変更して再試行してください。"
                .to_string(),
        ));
    }

    info!(
        "/simulate: {} statutes found, running simulation with {} agents",
        statutes.len(),
        population_size
    );

    let config = SimulationConfig {
        population_size,
        profile: jp_2024_profile(),
    };

    let result = SimulatorService::run(statutes, &config)
        .await
        .map_err(|e| match e {
            SimulatorError::NoStatutes => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "シミュレーション対象の条文がありません。".to_string(),
            ),
            SimulatorError::EmptyPopulation => {
                (StatusCode::BAD_REQUEST, "人口サイズが0です。".to_string())
            }
        })?;

    info!(
        "/simulate: total_applications={}, deterministic_ratio={:.2}",
        result.metrics.total_applications,
        result.metrics.deterministic_ratio()
    );

    Ok(Json(SimulateResponse::from_result(req.query, result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulate_request_defaults() {
        let json = r#"{"query": "労働基準法"}"#;
        let req: SimulateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "労働基準法");
        assert_eq!(req.population_size, 1000);
        assert_eq!(req.profile, "jp_2024");
    }

    #[test]
    fn test_simulate_request_custom_population() {
        let json = r#"{"query": "最低賃金", "population_size": 500}"#;
        let req: SimulateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.population_size, 500);
    }

    #[test]
    fn test_simulate_response_serialize() {
        let resp = SimulateResponse {
            query: "労働基準法".to_string(),
            population_size: 100,
            statute_count: 2,
            total_applications: 200,
            deterministic_count: 150,
            discretion_count: 40,
            void_count: 10,
            deterministic_ratio: 0.75,
            discretion_ratio: 0.20,
            statute_details: vec![],
            markdown_summary: "## 政策シミュレーション結果\n".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("deterministic_ratio"));
        assert!(json.contains("statute_details"));
        assert!(json.contains("markdown_summary"));
    }
}
