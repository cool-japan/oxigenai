use crate::error::{OxigenError, Result};

const DEFAULT_LOG_LEVEL: &str = "INFO";
const DEFAULT_MODEL_ID: &str = "gemini-2.5-flash";
const DEFAULT_LOCATION: &str = "asia-northeast1";
const DEFAULT_BQ_DATASET_ID: &str = "e_laws_search";
const DEFAULT_TEMPERATURE: f64 = 0.5;
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4098;
const DEFAULT_TOP_P: f64 = 1.0;
const DEFAULT_TOP_K: u32 = 1;
const DEFAULT_CANDIDATE_COUNT: u32 = 1;
const DEFAULT_SYSTEM_INSTRUCTION: &str = "You are a friendly and helpful assistant. \
    Ensure answers are complete unless the user requests brevity. \
    When generating code, include explanations.";

/// Application configuration loaded from environment variables.
/// Maps 1:1 to Python's GeminiConfig dataclass in gemini_config.py.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Log level (INFO, DEBUG, WARN, ERROR)
    pub log_level: String,
    /// GCP project ID for Vertex AI inference
    pub project_id: String,
    /// Vertex AI location (e.g. "asia-northeast1")
    pub location: String,
    /// Gemini model ID (e.g. "gemini-2.5-flash")
    pub model_id: String,
    /// Optional GCS bucket name for file uploads
    pub gcs_bucket_name: Option<String>,
    /// Generation temperature (0.0-1.0)
    pub temperature: f64,
    /// Maximum output tokens for Gemini
    pub max_output_tokens: u32,
    /// Top-p sampling parameter
    pub top_p: f64,
    /// Top-k sampling parameter
    pub top_k: u32,
    /// Number of candidate responses
    pub candidate_count: u32,
    /// Default system instruction for Gemini
    pub system_instruction: String,
    /// Whether to pass files by GCS URI (true when job and inference projects match)
    pub pass_file_by_uri: bool,
    /// BigQuery project ID
    pub bq_project_id: String,
    /// BigQuery dataset ID (e.g. "e_laws_search")
    pub bq_dataset_id: String,
}

impl AppConfig {
    /// Load configuration from environment variables.
    /// Mirrors `load_gemini_config()` from gemini_config.py.
    pub fn from_env() -> Result<Self> {
        // The project where this service is running
        let job_project_id = std::env::var("GOOGLE_CLOUD_PROJECT").map_err(|_| {
            OxigenError::Config("GOOGLE_CLOUD_PROJECT environment variable must be set".to_string())
        })?;

        // The project where Gemini inference runs (falls back to job project)
        let inference_project_id =
            std::env::var("INFERENCE_PROJECT_ID").unwrap_or_else(|_| job_project_id.clone());

        // Determine file passing mode: URI if same project, inline otherwise
        let pass_file_by_uri = job_project_id == inference_project_id;

        // BigQuery project: prefer env var, fall back to job project
        let bq_project_id =
            std::env::var("BQ_PROJECT_ID").unwrap_or_else(|_| job_project_id.clone());

        let bq_dataset_id =
            std::env::var("BQ_DATASET_ID").unwrap_or_else(|_| DEFAULT_BQ_DATASET_ID.to_string());

        Ok(Self {
            log_level: get_env_str("LOG_LEVEL", DEFAULT_LOG_LEVEL).to_uppercase(),
            project_id: inference_project_id,
            location: get_env_str("INFERENCE_LOCATION", DEFAULT_LOCATION),
            model_id: get_env_str("MODEL_ID", DEFAULT_MODEL_ID),
            gcs_bucket_name: std::env::var("GCS_BUCKET_NAME").ok(),
            temperature: get_env_f64("GENERATION_TEMPERATURE", DEFAULT_TEMPERATURE),
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
            top_p: get_env_f64("GENERATION_TOP_P", DEFAULT_TOP_P),
            top_k: get_env_u32("GENERATION_TOP_K", DEFAULT_TOP_K),
            candidate_count: get_env_u32("GENERATION_CANDIDATE_COUNT", DEFAULT_CANDIDATE_COUNT),
            system_instruction: get_env_str(
                "GENERATION_SYSTEM_INSTRUCTION",
                DEFAULT_SYSTEM_INSTRUCTION,
            ),
            pass_file_by_uri,
            bq_project_id,
            bq_dataset_id,
        })
    }

    /// Returns the full Vertex AI model path.
    pub fn model_path(&self) -> String {
        format!(
            "projects/{}/locations/{}/publishers/google/models/{}",
            self.project_id, self.location, self.model_id
        )
    }

    /// Returns Vertex AI endpoint base URL.
    pub fn vertex_ai_endpoint(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1beta1",
            self.location
        )
    }
}

fn get_env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn get_env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn get_env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path_format() {
        let config = AppConfig {
            log_level: "INFO".to_string(),
            project_id: "test-project".to_string(),
            location: "asia-northeast1".to_string(),
            model_id: "gemini-2.5-flash".to_string(),
            gcs_bucket_name: None,
            temperature: 0.5,
            max_output_tokens: 4098,
            top_p: 1.0,
            top_k: 1,
            candidate_count: 1,
            system_instruction: "test".to_string(),
            pass_file_by_uri: true,
            bq_project_id: "test-project".to_string(),
            bq_dataset_id: "e_laws_search".to_string(),
        };
        assert!(config.model_path().contains("gemini-2.5-flash"));
        assert!(config.model_path().contains("test-project"));
    }

    #[test]
    fn test_defaults() {
        assert_eq!(DEFAULT_TEMPERATURE, 0.5);
        assert_eq!(DEFAULT_TOP_K, 1);
        assert_eq!(DEFAULT_MAX_OUTPUT_TOKENS, 4098);
    }
}
