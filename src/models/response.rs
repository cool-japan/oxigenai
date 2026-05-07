use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cost breakdown for a Gemini API call.
/// Maps to Python's EstimatedCostInfo in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedCostInfo {
    /// Total estimated cost in USD
    #[serde(rename = "estimatedCost")]
    pub estimated_cost: f64,
    /// Currency (always "USD")
    pub currency: String,
    /// Number of input tokens
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    /// Number of output tokens
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    /// Price per 1M input tokens
    #[serde(rename = "inputToken1MUnitPrice")]
    pub input_token_1m_price: f64,
    /// Price per 1M output tokens
    #[serde(rename = "outputToken1MUnitPrice")]
    pub output_token_1m_price: f64,
}

/// Token usage summary for one model version.
/// Maps to Python's UsageSummaryEntry in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummaryEntry {
    /// Model version string (e.g. "gemini-2.5-flash")
    #[serde(rename = "modelVersion")]
    pub model_version: String,
    /// Number of requests to this model in this pipeline run
    #[serde(rename = "requestCount")]
    pub request_count: u32,
    /// Token counts by category (camelCase keys)
    pub tokens: HashMap<String, u64>,
    /// Estimated cost breakdown
    #[serde(skip_serializing_if = "Option::is_none", rename = "estimatedCostInfo")]
    pub estimated_cost_info: Option<EstimatedCostInfo>,
}

/// Main API response body.
/// Maps to Python's ResponseBody in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseBody {
    /// The generated legal report as Markdown text
    pub outputs: String,
    /// Usage metadata per model version
    #[serde(skip_serializing_if = "Option::is_none", rename = "usageMetadata")]
    pub usage_metadata: Option<Vec<UsageSummaryEntry>>,
}

/// Error response body for HTTP error responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl ErrorResponse {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_body_serialize() {
        let resp = ResponseBody {
            outputs: "# テストレポート".to_string(),
            usage_metadata: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("outputs"));
        assert!(!json.contains("usageMetadata"));
    }

    #[test]
    fn test_usage_entry_camel_case() {
        let entry = UsageSummaryEntry {
            model_version: "gemini-2.5-flash".to_string(),
            request_count: 3,
            tokens: {
                let mut m = HashMap::new();
                m.insert("promptTokenCount".to_string(), 1000u64);
                m
            },
            estimated_cost_info: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("modelVersion"));
        assert!(json.contains("requestCount"));
    }
}
