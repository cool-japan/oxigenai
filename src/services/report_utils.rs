use crate::models::law::ReferenceItem;
use once_cell::sync::Lazy;
use regex::Regex;

// Regex for citation numbers like [1], [2, 3], [1,2,3]
static CITATION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[(\d+(?:,\s*\d+)*)\]").expect("invariant: CITATION_RE pattern is valid")
});

// Regex to detect Mermaid diagram blocks
static MERMAID_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)```mermaid(.*?)```").expect("invariant: MERMAID_BLOCK_RE pattern is valid")
});

/// Format a reference item for display in the report's 出典 section.
/// Mirrors `_format_reference` in report_utils.py.
pub fn format_reference(index: usize, item: &ReferenceItem) -> String {
    let title = item.title();
    let url = item.url();
    let preview = normalize_content(item.content_preview(), 200);

    if item.is_egov() {
        format!(
            "[{}] 【e-laws公式条文】 **[{}]({})** — {}",
            index, title, url, preview
        )
    } else {
        format!("[{}] **[{}]({})** — {}", index, title, url, preview)
    }
}

/// Format a reference item for inclusion in the Gemini prompt.
/// Mirrors `_format_reference_for_prompt` in report_utils.py.
/// e-laws articles get full content; web hits get title+snippet only.
pub fn format_reference_for_prompt(index: usize, item: &ReferenceItem) -> String {
    match item {
        ReferenceItem::Article(article) => {
            format!(
                "[{}] 【e-laws公式条文】 {}\n{}",
                index, article.title, article.content
            )
        }
        ReferenceItem::WebHit(hit) => {
            format!("[{}] {}\n{}", index, hit.title, hit.snippet)
        }
    }
}

/// Normalize content for display: collapse whitespace and truncate.
/// Mirrors `_normalize_content` in report_utils.py.
pub fn normalize_content(text: &str, max_len: usize) -> String {
    let collapsed = text
        .replace(['\n', '\u{3000}'], " ") // newline and fullwidth space
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if collapsed.chars().count() > max_len {
        let truncated: String = collapsed.chars().take(max_len).collect();
        format!("{}...", truncated)
    } else {
        collapsed
    }
}

/// Extract cited reference indices from report text.
/// Returns `(original_0based_index, item)` pairs for cited references only.
/// Mirrors `_filter_references_by_citations` in report_utils.py.
pub fn filter_references_by_citations<'a>(
    report_text: &str,
    all_refs: &'a [ReferenceItem],
) -> Vec<(usize, &'a ReferenceItem)> {
    let cited_indices: std::collections::HashSet<usize> = CITATION_RE
        .captures_iter(report_text)
        .flat_map(|cap| {
            cap[1]
                .split(',')
                .filter_map(|s| {
                    let trimmed = s.trim();
                    trimmed.parse::<usize>().ok().map(|n| n.saturating_sub(1))
                })
                .collect::<Vec<_>>()
        })
        .filter(|&i| i < all_refs.len())
        .collect();

    if cited_indices.is_empty() {
        // Fallback: return all references
        return all_refs.iter().enumerate().collect::<Vec<_>>();
    }

    let mut result: Vec<(usize, &ReferenceItem)> = cited_indices
        .into_iter()
        .map(|i| (i, &all_refs[i]))
        .collect();
    result.sort_by_key(|(i, _)| *i);
    result
}

/// Convert `[n]` citation markers in report text to external links.
/// Skips content inside Mermaid blocks.
/// Mirrors `convert_citation_to_external_link` in report_utils.py.
pub fn convert_citation_to_external_link(
    text: &str,
    references: &[(usize, &ReferenceItem)],
) -> String {
    // Build index map: 1-based citation number → URL
    let url_map: std::collections::HashMap<usize, &str> = references
        .iter()
        .map(|(i, item)| (i + 1, item.url()))
        .collect();

    // Protect Mermaid blocks from substitution
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for mermaid_match in MERMAID_BLOCK_RE.find_iter(text) {
        let before = &text[last_end..mermaid_match.start()];
        result.push_str(&replace_citations(before, &url_map));
        result.push_str(mermaid_match.as_str());
        last_end = mermaid_match.end();
    }
    result.push_str(&replace_citations(&text[last_end..], &url_map));
    result
}

fn replace_citations(text: &str, url_map: &std::collections::HashMap<usize, &str>) -> String {
    CITATION_RE
        .replace_all(text, |caps: &regex::Captures| {
            let indices: Vec<usize> = caps[1]
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect();

            let links: Vec<String> = indices
                .iter()
                .map(|&n| {
                    if let Some(url) = url_map.get(&n) {
                        format!("[[{}]]({})", n, url)
                    } else {
                        format!("[{}]", n)
                    }
                })
                .collect();

            links.join(" ")
        })
        .into_owned()
}

/// Sanitize Mermaid diagram content to prevent parsing errors.
/// Replaces dangerous characters in node labels while preserving diagram syntax.
/// Mirrors `sanitize_mermaid_content` in report_utils.py.
pub fn sanitize_mermaid_content(text: &str) -> String {
    // Dangerous chars in Mermaid node labels (not in arrows/keywords)
    const DANGEROUS_REPLACEMENTS: &[(&str, &str)] = &[
        ("(", "（"),
        (")", "）"),
        ("{", "｛"),
        ("}", "｝"),
        ("[", "【"),
        ("]", "】"),
        ("\"", "＂"),
        (";", "；"),
    ];

    MERMAID_BLOCK_RE
        .replace_all(text, |caps: &regex::Captures| {
            let inner = &caps[1];
            let mut sanitized = inner.to_string();

            // Only replace in node label content (between brackets), not in arrows
            // Simple approach: replace in quoted strings and bracket content
            for (from, to) in DANGEROUS_REPLACEMENTS {
                // Protect arrow syntax: -->, --->, ==>, -.->
                sanitized = sanitized
                    .replace(&format!("\"{}\"", from), &format!("\"{}\"", to))
                    .replace(&format!("[{}]", from), &format!("【{}】", to));
            }

            format!("```mermaid{}```", sanitized)
        })
        .into_owned()
}

/// Build the 出典 (references) section for the final report.
pub fn build_citations_section(references: &[(usize, &'_ ReferenceItem)]) -> String {
    if references.is_empty() {
        return String::new();
    }

    let mut section = String::from("\n\n## 出典\n");
    for (i, item) in references {
        section.push_str(&format!("{}\n", format_reference(i + 1, item)));
    }
    section
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::law::{FullArticle, WebHit};

    fn make_web_hit(title: &str, url: &str, snippet: &str) -> ReferenceItem {
        ReferenceItem::WebHit(WebHit {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
        })
    }

    fn make_article(title: &str, url: &str, content: &str) -> ReferenceItem {
        ReferenceItem::Article(FullArticle {
            law_id: "test".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            unique_anchor: "Main_Article_1".to_string(),
            anchor: None,
            url: url.to_string(),
        })
    }

    #[test]
    fn test_normalize_content_truncation() {
        let long = "a".repeat(300);
        let result = normalize_content(&long, 200);
        assert!(result.chars().count() <= 204); // 200 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_normalize_content_newlines() {
        let text = "line1\nline2\n\u{3000}line3";
        let result = normalize_content(text, 200);
        assert!(!result.contains('\n'));
    }

    #[test]
    fn test_filter_citations_basic() {
        let refs = vec![
            make_web_hit("A", "https://a.com", ""),
            make_web_hit("B", "https://b.com", ""),
            make_web_hit("C", "https://c.com", ""),
        ];
        let report = "テキスト [1] とさらに [3] を参照";
        let cited = filter_references_by_citations(report, &refs);
        let indices: Vec<usize> = cited.iter().map(|(i, _)| *i).collect();
        assert!(indices.contains(&0));
        assert!(indices.contains(&2));
        assert!(!indices.contains(&1));
    }

    #[test]
    fn test_filter_citations_multi() {
        let refs: Vec<ReferenceItem> = (0..5)
            .map(|i| {
                make_web_hit(
                    &format!("Item {}", i),
                    &format!("https://example.com/{}", i),
                    "",
                )
            })
            .collect();
        let report = "参照 [1, 3] および [5]";
        let cited = filter_references_by_citations(report, &refs);
        let indices: Vec<usize> = cited.iter().map(|(i, _)| *i).collect();
        assert!(indices.contains(&0)); // [1] → index 0
        assert!(indices.contains(&2)); // [3] → index 2
        assert!(indices.contains(&4)); // [5] → index 4
    }

    #[test]
    fn test_convert_citation_to_link() {
        let refs = [
            make_web_hit("TestA", "https://a.com", ""),
            make_web_hit("TestB", "https://b.com", ""),
        ];
        let refs_indexed: Vec<(usize, &ReferenceItem)> = refs.iter().enumerate().collect();
        let text = "詳細は [1] および [2] を参照";
        let result = convert_citation_to_external_link(text, &refs_indexed);
        assert!(result.contains("[[1]](https://a.com)"));
        assert!(result.contains("[[2]](https://b.com)"));
    }

    #[test]
    fn test_egov_reference_label() {
        let article = make_article(
            "個人情報保護法 第2条",
            "https://laws.e-gov.go.jp/law/415AC0000000057",
            "この法律において「個人情報」とは...",
        );
        let formatted = format_reference(1, &article);
        assert!(formatted.contains("【e-laws公式条文】"));
    }
}
