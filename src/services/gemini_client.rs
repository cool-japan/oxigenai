use crate::config::AppConfig;
use crate::error::{OxigenError, Result};
use crate::models::law::WebHit;
use crate::services::usage_tracker::UsageMetadata;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client as HttpClient;
use serde::Serialize;
use serde_json::{Value, json};
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, warn};

// Redirect hosts that need URL resolution (Vertex AI search redirects)
static REDIRECT_HOST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"vertexaisearch\.cloud\.google\.com")
        .expect("invariant: REDIRECT_HOST_RE pattern is valid")
});

// HTML title extraction regex
static TITLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)<title[^>]*>([^<]+)</title>").expect("invariant: TITLE_RE pattern is valid")
});

/// Response from a Gemini API call.
#[derive(Debug, Clone)]
pub struct GeminiResponse {
    /// Extracted text (excluding thinking parts)
    pub text: String,
    /// Model version used
    pub model_version: String,
    /// Token usage metadata
    pub usage: Option<UsageMetadata>,
    /// Web search grounding hits
    pub grounding_hits: Vec<WebHit>,
}

/// Generation configuration for Gemini API calls.
#[derive(Debug, Clone, Serialize)]
pub struct GenerationConfig {
    pub temperature: f64,
    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: u32,
    #[serde(rename = "topP")]
    pub top_p: f64,
    #[serde(rename = "topK")]
    pub top_k: u32,
    #[serde(rename = "candidateCount")]
    pub candidate_count: u32,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            max_output_tokens: 8192,
            top_p: 1.0,
            top_k: 1,
            candidate_count: 1,
        }
    }
}

/// Grounding mode for Gemini API calls.
#[derive(Debug, Clone, PartialEq)]
pub enum Grounding {
    WebSearch,
    UrlContext,
    None,
}

/// Gemini service wrapping the Vertex AI REST API.
/// Provides all functionality from Python's gemini_helpers.py.
pub struct GeminiService {
    http: HttpClient,
    config: Arc<AppConfig>,
}

impl GeminiService {
    /// Create a new GeminiService with the given configuration.
    pub async fn new(config: Arc<AppConfig>) -> Result<Self> {
        let http = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(120))
            .user_agent("OxigenAI/0.1.0")
            .build()
            .map_err(OxigenError::Http)?;

        Ok(Self { http, config })
    }

    /// Get a valid access token for Vertex AI using Application Default Credentials.
    async fn get_access_token(&self) -> Result<String> {
        use google_cloud_auth::credentials::Builder;

        let credentials = Builder::default()
            .build_access_token_credentials()
            .map_err(|e| OxigenError::Config(format!("Failed to build credentials: {e}")))?;

        let token = credentials
            .access_token()
            .await
            .map_err(|e| OxigenError::Config(format!("Failed to get access token: {e}")))?;

        Ok(token.token)
    }

    /// Call Gemini with web search grounding (for law name estimation).
    pub async fn call_with_grounding(
        &self,
        input_text: &str,
        system_instruction: &str,
        grounding: Grounding,
        gen_config: &GenerationConfig,
    ) -> Result<GeminiResponse> {
        let token = self.get_access_token().await?;
        self.call_gemini_api(
            input_text,
            system_instruction,
            grounding,
            gen_config,
            &token,
        )
        .await
    }

    /// Call Gemini in strict/deterministic mode (for article selection).
    pub async fn call_strict(
        &self,
        input_text: &str,
        system_instruction: &str,
        gen_config: &GenerationConfig,
    ) -> Result<GeminiResponse> {
        let token = self.get_access_token().await?;
        self.call_gemini_api(
            input_text,
            system_instruction,
            Grounding::None,
            gen_config,
            &token,
        )
        .await
    }

    /// Call Gemini for report generation (optional url_context grounding).
    pub async fn call_for_report(
        &self,
        input_text: &str,
        system_instruction: &str,
        grounding: Grounding,
        gen_config: &GenerationConfig,
    ) -> Result<GeminiResponse> {
        let token = self.get_access_token().await?;
        self.call_gemini_api(
            input_text,
            system_instruction,
            grounding,
            gen_config,
            &token,
        )
        .await
    }

    /// Generate text embeddings using Vertex AI text-multilingual-embedding-002.
    /// Returns one embedding vector per input text.
    pub async fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f64>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let token = self.get_access_token().await?;
        let embedding_model = "text-multilingual-embedding-002";
        let url = format!(
            "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:predict",
            location = self.config.location,
            project = self.config.project_id,
            model = embedding_model,
        );

        let instances: Vec<Value> = texts.iter().map(|t| json!({"content": t})).collect();

        let body = json!({"instances": instances});

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(OxigenError::Http)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(OxigenError::Gemini(format!(
                "Embedding API error {status}: {text}"
            )));
        }

        let data: Value = resp.json().await.map_err(OxigenError::Http)?;
        let predictions = data["predictions"].as_array().ok_or_else(|| {
            OxigenError::Gemini("Missing predictions in embedding response".into())
        })?;

        let embeddings = predictions
            .iter()
            .map(|p| {
                p["embeddings"]["values"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .collect::<Vec<f64>>()
            })
            .collect();

        Ok(embeddings)
    }

    /// Core Gemini API call implementation using Vertex AI REST API.
    async fn call_gemini_api(
        &self,
        input_text: &str,
        system_instruction: &str,
        grounding: Grounding,
        gen_config: &GenerationConfig,
        token: &str,
    ) -> Result<GeminiResponse> {
        let endpoint = format!(
            "{}/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
            self.config.vertex_ai_endpoint(),
            self.config.project_id,
            self.config.location,
            self.config.model_id,
        );

        let mut tools: Vec<Value> = vec![];
        match &grounding {
            Grounding::WebSearch => {
                tools.push(json!({"googleSearch": {}}));
            }
            Grounding::UrlContext => {
                tools.push(json!({"urlContext": {}}));
            }
            Grounding::None => {}
        }

        let mut body = json!({
            "contents": [{
                "role": "user",
                "parts": [{"text": input_text}]
            }],
            "systemInstruction": {
                "parts": [{"text": system_instruction}]
            },
            "generationConfig": {
                "temperature": gen_config.temperature,
                "maxOutputTokens": gen_config.max_output_tokens,
                "topP": gen_config.top_p,
                "topK": gen_config.top_k,
                "candidateCount": gen_config.candidate_count,
            },
            "safetySettings": [
                {"category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "BLOCK_NONE"},
                {"category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE"},
                {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE"},
                {"category": "HARM_CATEGORY_HARASSMENT", "threshold": "BLOCK_NONE"},
            ]
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        debug!("Calling Gemini at {}", endpoint);

        let response = self
            .http
            .post(&endpoint)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(OxigenError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(OxigenError::Gemini(format!(
                "Vertex AI API error {}: {}",
                status, text
            )));
        }

        let json: Value = response.json().await.map_err(OxigenError::Http)?;
        self.parse_gemini_response(json)
    }

    /// Parse Gemini API JSON response into GeminiResponse.
    fn parse_gemini_response(&self, json: Value) -> Result<GeminiResponse> {
        // Extract text from candidates
        let mut text_parts: Vec<String> = vec![];
        if let Some(candidates) = json["candidates"].as_array() {
            for candidate in candidates {
                if let Some(parts) = candidate["content"]["parts"].as_array() {
                    for part in parts {
                        // Skip thinking parts (thought=true)
                        let is_thought = part
                            .get("thought")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        if !is_thought && let Some(text) = part["text"].as_str() {
                            text_parts.push(text.to_string());
                        }
                    }
                }
            }
        }

        let text = text_parts.join("");

        // Extract model version
        let model_version = json["modelVersion"]
            .as_str()
            .unwrap_or(&self.config.model_id)
            .split('/')
            .next_back()
            .unwrap_or(&self.config.model_id)
            .to_string();

        // Extract usage metadata
        let usage = self.parse_usage_metadata(&json, &model_version);

        // Extract grounding hits
        let grounding_hits = self.extract_grounding_web_hits(&json);

        Ok(GeminiResponse {
            text,
            model_version,
            usage,
            grounding_hits,
        })
    }

    /// Parse usage metadata from Gemini response.
    fn parse_usage_metadata(&self, json: &Value, model_version: &str) -> Option<UsageMetadata> {
        let meta = json.get("usageMetadata")?;
        Some(UsageMetadata {
            model_version: model_version.to_string(),
            prompt_token_count: meta["promptTokenCount"].as_u64().unwrap_or(0),
            candidates_token_count: meta["candidatesTokenCount"].as_u64().unwrap_or(0),
            cached_content_token_count: meta["cachedContentTokenCount"].as_u64().unwrap_or(0),
            tool_use_prompt_token_count: meta["toolUsePromptTokenCount"].as_u64().unwrap_or(0),
            thoughts_token_count: meta["thoughtsTokenCount"].as_u64().unwrap_or(0),
        })
    }

    /// Extract web search grounding hits from Gemini response.
    /// Mirrors `extract_grounding_web_hits` in gemini_helpers.py.
    pub fn extract_grounding_web_hits(&self, json: &Value) -> Vec<WebHit> {
        let mut hits = vec![];

        // Phase 1: grounding metadata → search entry points
        if let Some(candidates) = json["candidates"].as_array() {
            for candidate in candidates {
                let meta = &candidate["groundingMetadata"];

                // Grounding chunks (search results)
                if let Some(chunks) = meta["groundingChunks"].as_array() {
                    for chunk in chunks {
                        let web = &chunk["web"];
                        if let (Some(url), Some(title)) =
                            (web["uri"].as_str(), web["title"].as_str())
                        {
                            hits.push(WebHit {
                                title: title.to_string(),
                                snippet: String::new(),
                                url: url.to_string(),
                            });
                        }
                    }
                }

                // Citation metadata
                if let Some(citations) = meta["citationMetadata"]["citations"].as_array() {
                    for citation in citations {
                        if let Some(url) = citation["uri"].as_str() {
                            let title = citation["title"].as_str().unwrap_or(url);
                            if !hits.iter().any(|h: &WebHit| h.url == url) {
                                hits.push(WebHit {
                                    title: title.to_string(),
                                    snippet: String::new(),
                                    url: url.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        hits
    }

    /// Resolve redirect URLs (vertexaisearch) in parallel.
    /// Mirrors `resolve_redirect_web_hits` in gemini_helpers.py.
    pub async fn resolve_redirect_web_hits(&self, hits: Vec<WebHit>) -> Vec<WebHit> {
        let mut tasks = vec![];
        let http = self.http.clone();

        for hit in hits {
            let http = http.clone();
            let url = hit.url.clone();

            tasks.push(tokio::spawn(async move {
                if REDIRECT_HOST_RE.is_match(&url) {
                    match http.get(&url).send().await {
                        Ok(resp) => {
                            let final_url = resp.url().to_string();
                            WebHit {
                                url: final_url,
                                ..hit
                            }
                        }
                        Err(_) => hit,
                    }
                } else {
                    hit
                }
            }));
        }

        let mut resolved = vec![];
        for task in tasks {
            match task.await {
                Ok(hit) => resolved.push(hit),
                Err(e) => warn!("Redirect resolution task failed: {}", e),
            }
        }
        resolved
    }

    /// Fetch page title for a URL (SSRF-safe).
    /// Mirrors `_fetch_page_info` in gemini_helpers.py.
    pub async fn fetch_page_info(&self, url: &str) -> (String, String) {
        match self.safe_get(url).await {
            Ok(response) => {
                let final_url = response.url().to_string();
                let body = response.text().await.unwrap_or_default();
                let title = TITLE_RE
                    .captures(&body)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_else(|| url.to_string());
                (title, final_url)
            }
            Err(e) => {
                debug!("Failed to fetch page info for {}: {}", url, e);
                (url.to_string(), url.to_string())
            }
        }
    }

    /// SSRF-protected HTTP GET.
    /// Blocks requests to private/loopback addresses.
    /// Mirrors url_validator.safe_get() from Python.
    pub async fn safe_get(&self, url: &str) -> Result<reqwest::Response> {
        let parsed = url::Url::parse(url).map_err(OxigenError::UrlParse)?;

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(OxigenError::SsrfBlocked(format!(
                "Blocked scheme: {}",
                scheme
            )));
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| OxigenError::SsrfBlocked("No host in URL".to_string()))?;

        // Block obvious internal hostnames
        if host == "localhost" || host == "metadata.google.internal" {
            return Err(OxigenError::SsrfBlocked(format!(
                "Blocked internal host: {}",
                host
            )));
        }

        // Check for IP addresses
        if let Ok(ip) = host.parse::<IpAddr>() {
            if ip.is_loopback() || ip.is_unspecified() {
                return Err(OxigenError::SsrfBlocked(format!("Blocked IP: {}", ip)));
            }
            // Block private ranges
            if is_private_ip(&ip) {
                return Err(OxigenError::SsrfBlocked(format!(
                    "Blocked private IP: {}",
                    ip
                )));
            }
        }

        self.http
            .get(url)
            .timeout(std::time::Duration::from_secs(6))
            .send()
            .await
            .map_err(OxigenError::Http)
    }
}

/// Check if an IP address is in a private range.
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 10.0.0.0/8
            octets[0] == 10
            // 172.16.0.0/12
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
            // 192.168.0.0/16
            || (octets[0] == 192 && octets[1] == 168)
            // 169.254.0.0/16 (link-local / GCP metadata)
            || (octets[0] == 169 && octets[1] == 254)
        }
        IpAddr::V6(v6) => {
            // fc00::/7 (ULA)
            v6.segments()[0] & 0xfe00 == 0xfc00
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 0, 1))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn test_generation_config_defaults() {
        let cfg = GenerationConfig::default();
        assert_eq!(cfg.temperature, 0.0);
        assert_eq!(cfg.max_output_tokens, 8192);
        assert_eq!(cfg.top_k, 1);
    }
}
