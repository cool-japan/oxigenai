//! Self-contained single-page WebUI served by axum (Phase 3-3).
//!
//! The page is embedded at compile time via `include_str!`, so the binary needs
//! no filesystem assets at runtime and the UI pulls **no external resources**
//! (all CSS/JS is inlined) — a hard requirement for offline / air-gapped
//! government deployment.

use axum::response::Html;

/// The embedded single-page application (inline CSS + JS, no external assets).
const INDEX_HTML: &str = include_str!("../../static/index.html");

/// GET `/` and GET `/ui` — serve the self-contained WebUI.
pub async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

#[cfg(test)]
mod tests {
    use super::INDEX_HTML;

    #[test]
    fn test_index_html_is_embedded_and_nontrivial() {
        // A real page, not a stub.
        assert!(INDEX_HTML.len() > 4_000, "embedded UI looks too small");
        assert!(INDEX_HTML.contains("<!DOCTYPE html>"));
        assert!(INDEX_HTML.contains("</html>"));
    }

    #[test]
    fn test_index_html_has_branding_and_form() {
        assert!(INDEX_HTML.contains("OxigenAI"));
        assert!(INDEX_HTML.contains("源内"));
        // Core form controls the JS drives.
        assert!(INDEX_HTML.contains("id=\"query\""));
        assert!(INDEX_HTML.contains("id=\"jurisdiction\""));
    }

    #[test]
    fn test_index_html_references_all_endpoints() {
        // The UI must call exactly the routes the server exposes.
        assert!(INDEX_HTML.contains("/jurisdictions"));
        assert!(INDEX_HTML.contains("/predict-ruling"));
        assert!(INDEX_HTML.contains("/simulate"));
        assert!(INDEX_HTML.contains("/translate-check"));
        // The report endpoint is posted to root with the exact body shape.
        assert!(INDEX_HTML.contains("input_text"));
    }

    #[test]
    fn test_index_html_is_air_gapped() {
        // No external CDN / network assets allowed (offline deployment).
        assert!(!INDEX_HTML.contains("http://"));
        assert!(!INDEX_HTML.contains("https://"));
        assert!(!INDEX_HTML.contains("//cdn"));
        assert!(!INDEX_HTML.contains("src=\"//"));
    }
}
