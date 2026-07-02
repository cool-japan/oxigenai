use crate::verifier::compiler::{CompileError, CompilerService};
use crate::verifier::translate_check::{
    Divergence, StructuralSignature, TranslationComparison, compare_statutes,
};
use axum::{Json, http::StatusCode};
use legalis_dsl::format_statutes;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Request body for `POST /translate-check`.
///
/// Both documents are e-Gov-style law XML in their respective languages. The
/// `*_lang` fields are advisory labels only — the comparison itself is purely
/// structural and language-agnostic.
#[derive(Debug, Deserialize)]
pub struct TranslateCheckRequest {
    /// Source-language law XML (e.g. the Japanese original).
    pub source_xml: String,
    /// Target-language law XML (e.g. the English translation).
    pub target_xml: String,
    /// Advisory source language tag (default `"ja"`).
    #[serde(default = "default_source_lang")]
    pub source_lang: String,
    /// Advisory target language tag (default `"en"`).
    #[serde(default = "default_target_lang")]
    pub target_lang: String,
}

fn default_source_lang() -> String {
    "ja".to_string()
}

fn default_target_lang() -> String {
    "en".to_string()
}

/// Response body for `POST /translate-check`.
#[derive(Debug, Serialize)]
pub struct TranslateCheckResponse {
    /// `true` iff source and target compile to structurally identical statutes.
    pub equivalent: bool,
    /// Overall weighted structural similarity in `[0.0, 1.0]`.
    pub score: f64,
    /// Sub-score for matching statute counts.
    pub count_score: f64,
    /// Sub-score for the effect-type multiset (Sørensen–Dice).
    pub effect_score: f64,
    /// Sub-score for the precondition-kind multiset (Sørensen–Dice).
    pub condition_score: f64,
    /// Itemised structural differences (empty ⟺ `equivalent`).
    pub divergences: Vec<Divergence>,
    /// Structural fingerprint of the source document.
    pub source_signature: StructuralSignature,
    /// Structural fingerprint of the target document.
    pub target_signature: StructuralSignature,
    /// Advisory source language tag echoed from the request.
    pub source_lang: String,
    /// Advisory target language tag echoed from the request.
    pub target_lang: String,
    /// Legalis DSL compiled from the source document.
    pub source_dsl: String,
    /// Legalis DSL compiled from the target document.
    pub target_dsl: String,
}

impl TranslateCheckResponse {
    fn assemble(
        comparison: TranslationComparison,
        source_lang: String,
        target_lang: String,
        source_dsl: String,
        target_dsl: String,
    ) -> Self {
        Self {
            equivalent: comparison.equivalent,
            score: comparison.score,
            count_score: comparison.count_score,
            effect_score: comparison.effect_score,
            condition_score: comparison.condition_score,
            divergences: comparison.divergences,
            source_signature: comparison.source_signature,
            target_signature: comparison.target_signature,
            source_lang,
            target_lang,
            source_dsl,
            target_dsl,
        }
    }
}

/// Maps a [`CompileError`] for one side to an HTTP error, tagging which document.
fn compile_error(side: &str, err: CompileError) -> (StatusCode, String) {
    match err {
        CompileError::XmlParse(msg) => (
            StatusCode::BAD_REQUEST,
            format!("{side}のXML解析エラー: {msg}"),
        ),
        CompileError::InvalidDocument(msg) => (
            StatusCode::BAD_REQUEST,
            format!("{side}の不正なXML文書: {msg}"),
        ),
        CompileError::NoStatutes => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "{side}から条文を形式化できませんでした。有効な条文が含まれていない可能性があります。"
            ),
        ),
    }
}

/// POST /translate-check
///
/// Verifies that a statute and its translation carry the **same legal meaning**
/// by compiling *both* e-Gov-style XML documents to Legalis DSL and comparing the
/// resulting `legalis_core::Statute` sets **structurally** (statute count,
/// effect-type multiset, recursive precondition-kind multiset). The check is
/// fully offline — no network, no machine translation.
///
/// Example request:
/// ```json
/// {
///   "source_xml": "<Law>…日本語原文…</Law>",
///   "target_xml": "<Law>…English translation…</Law>",
///   "source_lang": "ja",
///   "target_lang": "en"
/// }
/// ```
pub async fn translate_check(
    Json(req): Json<TranslateCheckRequest>,
) -> Result<Json<TranslateCheckResponse>, (StatusCode, String)> {
    info!(
        "POST /translate-check — source={} bytes ({}), target={} bytes ({})",
        req.source_xml.len(),
        req.source_lang,
        req.target_xml.len(),
        req.target_lang
    );

    let source_statutes = CompilerService::compile_xml_to_statutes(&req.source_xml)
        .map_err(|e| compile_error("源文", e))?;
    let target_statutes = CompilerService::compile_xml_to_statutes(&req.target_xml)
        .map_err(|e| compile_error("訳文", e))?;

    let source_dsl = format_statutes(&source_statutes);
    let target_dsl = format_statutes(&target_statutes);

    let comparison = compare_statutes(&source_statutes, &target_statutes);

    info!(
        "/translate-check: equivalent={}, score={:.4}, divergences={}",
        comparison.equivalent,
        comparison.score,
        comparison.divergences.len()
    );

    Ok(Json(TranslateCheckResponse::assemble(
        comparison,
        req.source_lang,
        req.target_lang,
        source_dsl,
        target_dsl,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_defaults_langs() {
        let json = r#"{"source_xml": "<Law/>", "target_xml": "<Law/>"}"#;
        let req: TranslateCheckRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source_lang, "ja");
        assert_eq!(req.target_lang, "en");
    }

    #[test]
    fn test_request_explicit_langs() {
        let json = r#"{
            "source_xml": "<Law/>",
            "target_xml": "<Law/>",
            "source_lang": "ja",
            "target_lang": "fr"
        }"#;
        let req: TranslateCheckRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.target_lang, "fr");
    }

    #[test]
    fn test_response_serialize_shape() {
        let comparison = compare_statutes(&[], &[]);
        let resp = TranslateCheckResponse::assemble(
            comparison,
            "ja".to_string(),
            "en".to_string(),
            "STATUTE a {}".to_string(),
            "STATUTE a {}".to_string(),
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"equivalent\":true"));
        assert!(json.contains("\"score\""));
        assert!(json.contains("\"source_dsl\""));
        assert!(json.contains("\"target_dsl\""));
        assert!(json.contains("\"divergences\""));
    }

    #[tokio::test]
    async fn test_handler_rejects_invalid_source_xml() {
        let req = TranslateCheckRequest {
            source_xml: "not xml at all".to_string(),
            target_xml: "<Law/>".to_string(),
            source_lang: "ja".to_string(),
            target_lang: "en".to_string(),
        };
        let result = translate_check(Json(req)).await;
        assert!(result.is_err());
        let (status, msg) = result.err().unwrap();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(msg.contains("源文"), "error should tag the source side");
    }
}
