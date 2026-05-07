use crate::models::response::{EstimatedCostInfo, UsageSummaryEntry};
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Pricing per 1M tokens for each model.
struct ModelPricing {
    input: f64,
    cached_input: f64,
    output: f64,
    thoughts: f64,
}

/// Model pricing table — mirrors MODEL_PRICING in gemini_usage_tracker.py.
static MODEL_PRICING: Lazy<HashMap<&'static str, ModelPricing>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        "gemini-2.5-flash-lite",
        ModelPricing {
            input: 0.10,
            cached_input: 0.025,
            output: 0.40,
            thoughts: 0.40,
        },
    );
    m.insert(
        "gemini-2.5-flash",
        ModelPricing {
            input: 0.30,
            cached_input: 0.075,
            output: 2.50,
            thoughts: 2.50,
        },
    );
    m.insert(
        "gemini-2.5-pro",
        ModelPricing {
            input: 1.25,
            cached_input: 0.3125,
            output: 10.00,
            thoughts: 10.00,
        },
    );
    m.insert(
        "gemini-2.5-flash-image-preview",
        ModelPricing {
            input: 0.10,
            cached_input: 0.10,
            output: 30.00,
            thoughts: 0.40,
        },
    );
    m
});

/// Token usage data for a single Gemini API call.
#[derive(Debug, Clone, Default)]
pub struct UsageMetadata {
    pub model_version: String,
    pub prompt_token_count: u64,
    pub candidates_token_count: u64,
    pub cached_content_token_count: u64,
    pub tool_use_prompt_token_count: u64,
    pub thoughts_token_count: u64,
}

/// Accumulates token usage across multiple Gemini API calls within a pipeline run.
/// Mirrors UsageTracker class in gemini_usage_tracker.py.
#[derive(Debug, Default)]
pub struct UsageTracker {
    usages: Vec<UsageMetadata>,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record usage from a Gemini API response.
    pub fn add_usage(&mut self, usage: UsageMetadata) {
        self.usages.push(usage);
    }

    /// Aggregate usage by model version and compute cost estimates.
    /// Mirrors `get_usage_summary()` in gemini_usage_tracker.py.
    pub fn get_usage_summary(&self) -> Vec<UsageSummaryEntry> {
        // Group by model version
        let mut by_model: HashMap<String, Vec<&UsageMetadata>> = HashMap::new();
        for usage in &self.usages {
            let version = normalize_model_version(&usage.model_version);
            by_model.entry(version).or_default().push(usage);
        }

        let mut results = vec![];
        for (model_version, usages) in by_model {
            let request_count = usages.len() as u32;

            // Aggregate token counts
            let mut tokens: HashMap<String, u64> = HashMap::new();
            let mut total_prompt = 0u64;
            let mut total_output = 0u64;
            let mut total_cached = 0u64;
            let mut total_tool = 0u64;
            let mut total_thoughts = 0u64;

            for u in &usages {
                total_prompt += u.prompt_token_count;
                total_output += u.candidates_token_count;
                total_cached += u.cached_content_token_count;
                total_tool += u.tool_use_prompt_token_count;
                total_thoughts += u.thoughts_token_count;
            }

            if total_prompt > 0 {
                tokens.insert("promptTokenCount".to_string(), total_prompt);
            }
            if total_output > 0 {
                tokens.insert("candidatesTokenCount".to_string(), total_output);
            }
            if total_cached > 0 {
                tokens.insert("cachedContentTokenCount".to_string(), total_cached);
            }
            if total_tool > 0 {
                tokens.insert("toolUsePromptTokenCount".to_string(), total_tool);
            }
            if total_thoughts > 0 {
                tokens.insert("thoughtsTokenCount".to_string(), total_thoughts);
            }

            // Compute estimated cost
            let estimated_cost_info = calculate_cost(
                &model_version,
                total_prompt,
                total_output,
                total_cached,
                total_thoughts,
            );

            results.push(UsageSummaryEntry {
                model_version,
                request_count,
                tokens,
                estimated_cost_info,
            });
        }

        results
    }
}

/// Normalize model version string (strip project path prefix if present).
fn normalize_model_version(version: &str) -> String {
    // Handle "projects/.../models/gemini-2.5-flash" → "gemini-2.5-flash"
    version
        .split('/')
        .next_back()
        .unwrap_or(version)
        .to_string()
}

/// Calculate estimated cost for a model based on token counts.
/// Mirrors `_calculate_estimated_cost` in gemini_usage_tracker.py.
fn calculate_cost(
    model_version: &str,
    prompt_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    thoughts_tokens: u64,
) -> Option<EstimatedCostInfo> {
    // Try exact match, then prefix match
    let pricing = MODEL_PRICING.get(model_version).or_else(|| {
        MODEL_PRICING
            .keys()
            .find(|k| model_version.starts_with(**k))
            .and_then(|k| MODEL_PRICING.get(k))
    })?;

    const M: f64 = 1_000_000.0;

    let input_cost = (prompt_tokens as f64 / M) * pricing.input;
    let cached_cost = (cached_tokens as f64 / M) * pricing.cached_input;
    let output_cost = (output_tokens as f64 / M) * pricing.output;
    let thoughts_cost = (thoughts_tokens as f64 / M) * pricing.thoughts;
    let total = input_cost + cached_cost + output_cost + thoughts_cost;

    Some(EstimatedCostInfo {
        estimated_cost: total,
        currency: "USD".to_string(),
        input_tokens: prompt_tokens,
        output_tokens,
        input_token_1m_price: pricing.input,
        output_token_1m_price: pricing.output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_model_version() {
        assert_eq!(
            normalize_model_version("projects/my-proj/locations/us/models/gemini-2.5-flash"),
            "gemini-2.5-flash"
        );
        assert_eq!(normalize_model_version("gemini-2.5-pro"), "gemini-2.5-pro");
    }

    #[test]
    fn test_cost_calculation_flash() {
        // 1M input tokens + 1M output tokens for gemini-2.5-flash
        let cost = calculate_cost("gemini-2.5-flash", 1_000_000, 1_000_000, 0, 0).unwrap();
        assert!((cost.estimated_cost - 2.80).abs() < 0.01); // $0.30 + $2.50
        assert_eq!(cost.currency, "USD");
    }

    #[test]
    fn test_cost_calculation_pro() {
        let cost = calculate_cost("gemini-2.5-pro", 1_000_000, 1_000_000, 0, 0).unwrap();
        assert!((cost.estimated_cost - 11.25).abs() < 0.01); // $1.25 + $10.00
    }

    #[test]
    fn test_usage_tracker_aggregation() {
        let mut tracker = UsageTracker::new();
        tracker.add_usage(UsageMetadata {
            model_version: "gemini-2.5-flash".to_string(),
            prompt_token_count: 1000,
            candidates_token_count: 500,
            ..Default::default()
        });
        tracker.add_usage(UsageMetadata {
            model_version: "gemini-2.5-flash".to_string(),
            prompt_token_count: 2000,
            candidates_token_count: 800,
            ..Default::default()
        });

        let summary = tracker.get_usage_summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].request_count, 2);
        assert_eq!(summary[0].tokens["promptTokenCount"], 3000);
        assert_eq!(summary[0].tokens["candidatesTokenCount"], 1300);
    }

    #[test]
    fn test_unknown_model_no_cost() {
        let cost = calculate_cost("unknown-model-xyz", 1000, 1000, 0, 0);
        assert!(cost.is_none());
    }
}
