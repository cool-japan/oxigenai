use crate::models::response::{ErrorResponse, ResponseBody};
use crate::services::pipeline::{PipelineContext, generate_law_report};
use axum::{Json, body::Bytes, extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;
use tracing::{error, info};

/// Shared application state for all handlers.
pub type AppState = Arc<PipelineContext>;

/// POST / — Generate a legal report.
/// Mirrors the main Cloud Function handler in main.py.
///
/// Request: `{"inputs": {"input_text": "..."}}`
/// Response: `{"outputs": "...", "usageMetadata": [...]}`
pub async fn generate_report(State(ctx): State<AppState>, body: Bytes) -> impl IntoResponse {
    // Parse raw body to extract input_text
    let input_text = match extract_input_text(&body) {
        Ok(t) => t,
        Err(err_response) => return err_response,
    };

    info!("Received query: {} chars", input_text.len());

    match generate_law_report(&input_text, &ctx).await {
        Ok((outputs, usage_metadata)) => {
            let response = ResponseBody {
                outputs,
                usage_metadata: Some(usage_metadata),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Pipeline error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(format!("Internal Server Error: {}", e))),
            )
                .into_response()
        }
    }
}

/// Extract `inputs.input_text` from the request body.
/// Returns an error response if the body is malformed.
#[allow(clippy::result_large_err)]
fn extract_input_text(body: &Bytes) -> Result<String, axum::response::Response> {
    // Parse JSON
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("Invalid JSON")),
            )
                .into_response());
        }
    };

    // Extract inputs.input_text
    let input_text = value
        .get("inputs")
        .and_then(|v| v.get("input_text"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match input_text {
        Some(t) if !t.trim().is_empty() => Ok(t),
        Some(_) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                r#"Invalid payload. "inputs.input_text" must not be empty."#,
            )),
        )
            .into_response()),
        None => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                r#"Invalid payload. "inputs.input_text" is required."#,
            )),
        )
            .into_response()),
    }
}

/// GET /health — Health check endpoint.
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_input_text_valid() {
        let body = b"{\"inputs\": {\"input_text\": \"test query\"}}";
        let result = extract_input_text(&Bytes::from_static(body));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test query");
    }

    #[test]
    fn test_extract_input_text_missing() {
        let body = b"{\"inputs\": {}}";
        let result = extract_input_text(&Bytes::from_static(body));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_input_text_invalid_json() {
        let body = b"not json at all";
        let result = extract_input_text(&Bytes::from_static(body));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_input_text_empty() {
        let body = b"{\"inputs\": {\"input_text\": \"   \"}}";
        let result = extract_input_text(&Bytes::from_static(body));
        assert!(result.is_err());
    }
}
