use serde::{Deserialize, Serialize};

/// Grounding mode for Gemini API calls.
/// Maps to Python's Grounding enum in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GroundingMode {
    WebSearch,
    UrlContext,
}

/// File input for Gemini API (base64 content or GCS URI).
/// Maps to Python's FileInput in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInput {
    /// Identifier for this file
    pub key: String,
    /// Filename
    pub filename: String,
    /// Base64-encoded file content (mutually exclusive with gcs_uri)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// GCS URI (mutually exclusive with content)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcs_uri: Option<String>,
}

/// Main request body matching the Python RequestBody in schemas.py.
/// The outer envelope is `{"inputs": {"input_text": "..."}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    /// The user's legal query text
    pub input_text: String,
    /// Optional chat history for multi-turn conversations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_history: Option<Vec<serde_json::Value>>,
    /// Grounding mode (web_search or url_context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding: Option<GroundingMode>,
    /// Generation temperature override (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Max output tokens override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Top-p override (0.0-1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Top-k override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Candidate count override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<u32>,
    /// Custom system instruction override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,
    /// Files to include in the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileInput>>,
    /// Thinking budget for reasoning models (0 = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,
    /// Jurisdiction code for statute resolution (e.g. "JP", "EU", "US").
    /// When absent the pipeline defaults to "JP" (backward-compatible).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
}

/// The outer HTTP request envelope: {"inputs": {"input_text": "..."}}
#[derive(Debug, Deserialize)]
pub struct HttpRequestEnvelope {
    pub inputs: InputsField,
}

#[derive(Debug, Deserialize)]
pub struct InputsField {
    pub input_text: String,
    /// Optional jurisdiction code (defaults to "JP" when absent).
    #[serde(default)]
    pub jurisdiction: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_body_deserialize() {
        let json = r#"{"input_text": "個人情報保護法について"}"#;
        let body: RequestBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.input_text, "個人情報保護法について");
        assert!(body.grounding.is_none());
        // Absent jurisdiction stays None — callers default it to "JP".
        assert!(body.jurisdiction.is_none());
    }

    #[test]
    fn test_request_body_with_jurisdiction() {
        let json = r#"{"input_text": "GDPR erasure", "jurisdiction": "EU"}"#;
        let body: RequestBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.jurisdiction.as_deref(), Some("EU"));
    }

    #[test]
    fn test_http_envelope_deserialize() {
        let json = r#"{"inputs": {"input_text": "テスト"}}"#;
        let envelope: HttpRequestEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(envelope.inputs.input_text, "テスト");
        assert!(envelope.inputs.jurisdiction.is_none());
    }

    #[test]
    fn test_http_envelope_with_jurisdiction() {
        let json = r#"{"inputs": {"input_text": "テスト", "jurisdiction": "US"}}"#;
        let envelope: HttpRequestEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(envelope.inputs.jurisdiction.as_deref(), Some("US"));
    }

    #[test]
    fn test_grounding_mode_serde() {
        let mode = GroundingMode::WebSearch;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#""web_search""#);
    }
}
