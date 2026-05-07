use thiserror::Error;

#[derive(Error, Debug)]
pub enum OxigenError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("BigQuery error: {0}")]
    BigQuery(String),

    #[error("Gemini API error: {0}")]
    Gemini(String),

    #[error("Law name estimation failed: {0}")]
    LawNameEstimation(String),

    #[error("No relevant laws found for query")]
    NoLawsFound,

    #[error("No articles found for the given law names")]
    NoArticlesFound,

    #[error("Legal verification error: {0}")]
    Verification(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SSRF attack blocked: {0}")]
    SsrfBlocked(String),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OxigenError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = OxigenError::Config("missing PROJECT_ID".to_string());
        assert!(err.to_string().contains("Configuration error"));

        let err = OxigenError::NoLawsFound;
        assert!(err.to_string().contains("No relevant laws"));
    }
}
