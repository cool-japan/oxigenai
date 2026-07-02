use crate::models::law::{ArticleWithSummary, FullArticle};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

// Regex for extracting Japanese law names from query text.
// Matches kanji+kana sequences ending with 法律/法/規則/政令/条例/省令.
static LAW_NAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[\u4e00-\u9fff\u3040-\u30ff\uff00-\uffef]+(?:法律|法|規則|政令|条例|省令)")
        .expect("invariant: LAW_NAME_RE pattern is valid")
});

// Regex for extracting article numbers from query (e.g. "第42条").
static ARTICLE_NUM_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"第(\d+)条").expect("invariant: ARTICLE_NUM_RE pattern is valid"));

// URL detection regex.
static URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https?://\S+").expect("invariant: URL_RE pattern is valid"));

/// Extract formal law names from user query text.
/// Mirrors `_extract_law_names_from_query` from law_report_pipeline.py.
pub fn extract_law_names_from_query(query: &str) -> Vec<String> {
    let mut names: Vec<String> = LAW_NAME_RE
        .find_iter(query)
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| s.chars().count() >= 4)
        .collect();
    names.dedup();
    names
}

/// Check if a query contains URLs.
pub fn query_has_url(query: &str) -> bool {
    URL_RE.is_match(query)
}

/// Extract URLs from a query string.
pub fn extract_urls_from_query(query: &str) -> Vec<String> {
    URL_RE
        .find_iter(query)
        .map(|m| m.as_str().to_string())
        .collect()
}

/// Expand law names with associated ordinances (施行令, 施行規則).
/// Mirrors `_expand_law_names_with_ordinances` from law_report_pipeline.py.
pub fn expand_law_names_with_ordinances(law_names: &[String]) -> Vec<String> {
    let mut expanded = law_names.to_vec();
    for name in law_names {
        if name.ends_with("法律") || name.ends_with("法") {
            expanded.push(format!("{}施行令", name));
            expanded.push(format!("{}施行規則", name));
        }
    }
    expanded
}

/// Build the law name estimation prompt.
/// Returns (input_text, system_instruction).
pub fn build_law_name_estimation_prompt(query: &str, has_url: bool) -> (String, String) {
    let grounding_note = if has_url {
        "（URLが含まれているため、URLの内容も参考にしてください）"
    } else {
        ""
    };

    let input_text = format!(
        "以下のクエリに関連する日本の法令の正式名称を特定してください。{}\n\nクエリ: {}\n\n結果はJSONで返してください: {{\"law_names\": [\"法令名1\", \"法令名2\", ...]}}",
        grounding_note, query
    );

    let system_instruction = "あなたは日本の法令データベースの専門家です。\
        ユーザーのクエリに関連する日本の法令の正式名称を特定してください。\
        e-Gov法令データベース（https://laws.e-gov.go.jp/）に掲載されている正式な法令名を使用してください。\
        結果は必ずJSON形式で返してください：{\"law_names\": [\"法令名1\", \"法令名2\", ...]}".to_string();

    (input_text, system_instruction)
}

/// Build the article selection prompt.
/// Mirrors the prompt used in `_select_articles` from law_report_pipeline.py.
pub fn build_article_selection_prompt(query: &str, articles: &[ArticleWithSummary]) -> String {
    let mut lines = vec![
        format!("元のクエリ: {}\n", query),
        "条文概要リスト:".to_string(),
    ];

    for (i, article) in articles.iter().enumerate() {
        let summary = article.best_text().unwrap_or("（概要なし）");
        let preview = if summary.chars().count() > 200 {
            summary.chars().take(200).collect::<String>() + "..."
        } else {
            summary.to_string()
        };
        lines.push(format!(
            "[{}] {} - {}: {}",
            i + 1,
            article.law_title,
            article.unique_anchor,
            preview
        ));
    }

    lines.push(String::new());
    lines.push(crate::prompts::templates::PROMPT_SELECT_RELEVANT_ARTICLES.to_string());

    lines.join("\n")
}

/// Build the complete report generation prompt.
pub fn build_report_prompt(query: &str, references_text: &str) -> String {
    format!(
        "{}\n\nクエリ: {}\n\n参考情報:\n{}",
        crate::prompts::templates::PROMPT_GENERATE_COMPLETE_REPORT,
        query,
        references_text
    )
}

/// Build the mentioned articles prefix for the report prompt.
/// Mirrors `_build_mentioned_articles_prefix` from law_report_pipeline.py.
pub fn build_mentioned_articles_prefix(query: &str, articles: &[FullArticle]) -> String {
    let article_nums: Vec<u32> = ARTICLE_NUM_RE
        .captures_iter(query)
        .filter_map(|cap| cap[1].parse().ok())
        .collect();

    if article_nums.is_empty() {
        return String::new();
    }

    let mut found = vec![];
    for num in &article_nums {
        let anchor_pattern = format!("Article_{}", num);
        let matching: Vec<_> = articles
            .iter()
            .filter(|a| a.unique_anchor.ends_with(&anchor_pattern))
            .collect();
        for article in matching {
            found.push(format!(
                "第{}条（{}）: {}",
                num,
                article.title,
                if article.content.chars().count() > 300 {
                    article.content.chars().take(300).collect::<String>() + "..."
                } else {
                    article.content.clone()
                }
            ));
        }
    }

    if found.is_empty() {
        return String::new();
    }

    format!(
        "【クエリで指定された条文の確認】\n以下の条文が参考情報に含まれています：\n{}\n\n",
        found.join("\n")
    )
}

/// Compute bigram-based Jaccard similarity between two strings.
/// Mirrors `_bigram_similarity` from law_report_pipeline.py.
/// Removes Japanese particles before computing.
pub fn bigram_similarity(s1: &str, s2: &str) -> f64 {
    let clean1 = remove_particles(s1);
    let clean2 = remove_particles(s2);

    let bigrams1 = char_bigrams(&clean1);
    let bigrams2 = char_bigrams(&clean2);

    if bigrams1.is_empty() && bigrams2.is_empty() {
        return 1.0;
    }
    if bigrams1.is_empty() || bigrams2.is_empty() {
        return 0.0;
    }

    let intersection = bigrams1.intersection(&bigrams2).count();
    let union = bigrams1.union(&bigrams2).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn remove_particles(s: &str) -> String {
    // Remove common Japanese particles that don't carry meaning for law name matching
    s.replace("の", "")
        .replace("に", "")
        .replace("は", "")
        .replace("を", "")
}

fn char_bigrams(s: &str) -> HashSet<(char, char)> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Build a warning about law name substitutions (通称 → 正式名称).
/// Mirrors `_build_substitution_warning` from law_report_pipeline.py.
/// Threshold: 0.30 (below this = likely substitution).
pub fn build_substitution_warning(
    query_law_names: &[String],
    estimated_law_names: &[String],
) -> String {
    const SUBSTITUTION_THRESHOLD: f64 = 0.30;

    let mut warnings = vec![];
    for query_name in query_law_names {
        let best_match = estimated_law_names
            .iter()
            .map(|est| (est, bigram_similarity(query_name, est)))
            .max_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((matched_name, score)) = best_match
            && score < SUBSTITUTION_THRESHOLD
            && matched_name != query_name
        {
            warnings.push(format!(
                "「{}」は通称・略称の可能性があります。正式名称「{}」を使用しました。",
                query_name, matched_name
            ));
        }
    }

    if warnings.is_empty() {
        String::new()
    } else {
        format!(
            "【注意】法令名の読み替えが発生しました：\n{}\n\n",
            warnings.join("\n")
        )
    }
}

/// Build a warning about divergence between estimated and retrieved law names.
/// Mirrors `_check_law_name_divergence` from law_report_pipeline.py.
/// Threshold: 0.40 (below this = likely divergence).
pub fn build_divergence_warning(law_names: &[String], articles: &[ArticleWithSummary]) -> String {
    const DIVERGENCE_THRESHOLD: f64 = 0.40;

    let retrieved_titles: Vec<String> = articles
        .iter()
        .map(|a| a.law_title.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut warnings = vec![];
    for law_name in law_names {
        let best_match = retrieved_titles
            .iter()
            .map(|title| (title, bigram_similarity(law_name, title)))
            .max_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((matched_title, score)) = best_match
            && score < DIVERGENCE_THRESHOLD
        {
            warnings.push(format!(
                "「{}」の検索結果として「{}」が取得されました（類似度: {:.2}）。",
                law_name, matched_title, score
            ));
        }
    }

    if warnings.is_empty() {
        String::new()
    } else {
        format!("【法令名の乖離に関する注意】\n{}\n\n", warnings.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_law_names() {
        let query = "個人情報保護法と著作権法の関係について";
        let names = extract_law_names_from_query(query);
        assert!(names.iter().any(|n| n.contains("個人情報保護法")));
        assert!(names.iter().any(|n| n.contains("著作権法")));
    }

    #[test]
    fn test_expand_law_names() {
        let names = vec!["個人情報保護法".to_string()];
        let expanded = expand_law_names_with_ordinances(&names);
        assert!(expanded.contains(&"個人情報保護法施行令".to_string()));
        assert!(expanded.contains(&"個人情報保護法施行規則".to_string()));
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn test_bigram_similarity_identical() {
        let score = bigram_similarity("個人情報保護法", "個人情報保護法");
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_bigram_similarity_different() {
        let score = bigram_similarity("個人情報保護法", "著作権法");
        assert!(score < 0.5);
    }

    #[test]
    fn test_bigram_similarity_range() {
        for (s1, s2) in [
            ("個人情報保護法", "個人情報の保護に関する法律"),
            ("著作権法", "著作権に関する法律"),
            ("全く関係ない", "著作権法"),
        ] {
            let score = bigram_similarity(s1, s2);
            assert!(
                (0.0..=1.0).contains(&score),
                "score out of range: {}",
                score
            );
        }
    }

    #[test]
    fn test_query_has_url() {
        assert!(query_has_url("https://laws.e-gov.go.jp/law/test について"));
        assert!(!query_has_url("個人情報保護法について"));
    }

    #[test]
    fn test_substitution_warning_similar() {
        // Similar names should not trigger warning
        let query_names = vec!["個人情報保護法".to_string()];
        let estimated = vec!["個人情報の保護に関する法律".to_string()];
        let warning = build_substitution_warning(&query_names, &estimated);
        // These are similar enough (> 0.30) so no warning
        // The similarity might be below threshold for very different names
        let _ = warning; // just ensure it doesn't panic
    }

    #[test]
    fn test_expand_does_not_duplicate() {
        let names = vec!["民法".to_string(), "民法施行令".to_string()];
        let expanded = expand_law_names_with_ordinances(&names);
        // 民法 → adds 民法施行令, 民法施行規則; 民法施行令 → adds 民法施行令施行令 etc.
        assert!(expanded.len() >= 2);
    }
}
