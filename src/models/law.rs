use serde::{Deserialize, Serialize};

/// A candidate law found by vector similarity search.
/// Maps to Python's LawCandidate in retrieval_bq.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawCandidate {
    /// Law number (法令番号), e.g. "平成十五年法律第五十七号"
    pub law_num: String,
    /// Law title (法令名), e.g. "個人情報の保護に関する法律"
    pub law_title: String,
    /// Cosine similarity score from vector search
    pub score: f64,
}

/// An article with its summary from BigQuery.
/// Maps to Python's ArticleWithSummary in retrieval_bq.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleWithSummary {
    /// Law number
    pub law_num: String,
    /// e-Gov law ID (used to construct URLs)
    pub law_id: String,
    /// Law title
    pub law_title: String,
    /// Unique anchor identifier, e.g. "Main_Article_1"
    pub unique_anchor: String,
    /// AI-generated article summary
    pub article_summary: Option<String>,
    /// Article content (may be summary-only for large laws > 100k chars)
    pub content: Option<String>,
    /// True if content is summary-only (law text > 100,000 characters)
    #[serde(default)]
    pub is_summary_only: bool,
}

impl ArticleWithSummary {
    /// Returns the best available text (content or summary).
    pub fn best_text(&self) -> Option<&str> {
        self.content.as_deref().or(self.article_summary.as_deref())
    }

    /// Extracts the article number from unique_anchor (e.g. "Main_Article_42" → Some(42)).
    pub fn article_number(&self) -> Option<u32> {
        self.unique_anchor
            .split('_')
            .next_back()
            .and_then(|s| s.parse().ok())
    }
}

/// A full article with e-Gov URL for citation.
/// Maps to Python's FullArticle in retrieval_bq.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullArticle {
    /// e-Gov law ID
    pub law_id: String,
    /// Display title (law name + article summary)
    pub title: String,
    /// Full article text content
    pub content: String,
    /// Unique anchor identifier
    pub unique_anchor: String,
    /// URL anchor fragment (for direct linking to the article)
    pub anchor: Option<String>,
    /// Full e-Gov URL: <https://laws.e-gov.go.jp/law/{law_id}#{anchor}>
    pub url: String,
}

impl FullArticle {
    /// Constructs the e-Gov URL for this article.
    pub fn build_egov_url(law_id: &str, anchor: Option<&str>) -> String {
        // law_id format: "415AC0000000057_20230401_505AC0000000033" → base is "415AC0000000057"
        let base_id = law_id.split('_').next().unwrap_or(law_id);
        match anchor {
            Some(a) if !a.is_empty() => {
                format!("https://laws.e-gov.go.jp/law/{}#{}", base_id, a)
            }
            _ => format!("https://laws.e-gov.go.jp/law/{}", base_id),
        }
    }

    /// Returns true if this is an e-Gov official article (not a web hit).
    pub fn is_egov(&self) -> bool {
        self.url.contains("laws.e-gov.go.jp")
    }
}

/// Structured output for law name estimation by Gemini.
/// Maps to Python's LawNamesEstimation in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawNamesEstimation {
    /// List of estimated formal law names
    pub law_names: Vec<String>,
}

/// Structured output for article selection by Gemini.
/// Maps to Python's SelectionResult in schemas.py.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResult {
    /// 1-based indices of selected articles
    pub selected_indices: Vec<usize>,
}

/// A web search hit from Gemini grounding metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebHit {
    /// Page title
    pub title: String,
    /// Snippet or content summary
    pub snippet: String,
    /// Source URL
    pub url: String,
}

/// Union type for references (either a law article or a web hit).
#[derive(Debug, Clone)]
pub enum ReferenceItem {
    Article(FullArticle),
    WebHit(WebHit),
}

impl ReferenceItem {
    pub fn title(&self) -> &str {
        match self {
            Self::Article(a) => &a.title,
            Self::WebHit(w) => &w.title,
        }
    }

    pub fn url(&self) -> &str {
        match self {
            Self::Article(a) => &a.url,
            Self::WebHit(w) => &w.url,
        }
    }

    pub fn is_egov(&self) -> bool {
        matches!(self, Self::Article(a) if a.is_egov())
    }

    pub fn content_preview(&self) -> &str {
        match self {
            Self::Article(a) => &a.content,
            Self::WebHit(w) => &w.snippet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_egov_url_with_anchor() {
        let url = FullArticle::build_egov_url("415AC0000000057_20230401", Some("Article_2"));
        assert_eq!(
            url,
            "https://laws.e-gov.go.jp/law/415AC0000000057#Article_2"
        );
    }

    #[test]
    fn test_build_egov_url_no_anchor() {
        let url = FullArticle::build_egov_url("415AC0000000057", None);
        assert_eq!(url, "https://laws.e-gov.go.jp/law/415AC0000000057");
    }

    #[test]
    fn test_article_number_extraction() {
        let article = ArticleWithSummary {
            law_num: "test".to_string(),
            law_id: "test".to_string(),
            law_title: "test".to_string(),
            unique_anchor: "Main_Article_42".to_string(),
            article_summary: None,
            content: None,
            is_summary_only: false,
        };
        assert_eq!(article.article_number(), Some(42));
    }

    #[test]
    fn test_best_text_prefers_content() {
        let article = ArticleWithSummary {
            law_num: "test".to_string(),
            law_id: "test".to_string(),
            law_title: "test".to_string(),
            unique_anchor: "Main_Article_1".to_string(),
            article_summary: Some("summary".to_string()),
            content: Some("full content".to_string()),
            is_summary_only: false,
        };
        assert_eq!(article.best_text(), Some("full content"));
    }
}
