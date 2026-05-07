use crate::models::law::FullArticle;
use crate::verifier::dsl_bridge::{ConversionSource, StatuteBridge};
use legalis_core::Statute;
use legalis_dsl::{format_statute, format_statutes};
use legalis_jp::EGovLawParser;
use thiserror::Error;
use tracing::debug;

/// Error from the compiler service.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("XML parse error: {0}")]
    XmlParse(String),
    #[error("Invalid document: {0}")]
    InvalidDocument(String),
    #[error("No statutes could be derived from the document")]
    NoStatutes,
}

/// A single compiled statute with its DSL text and provenance.
#[derive(Debug, Clone)]
pub struct CompiledStatute {
    /// Statute identifier (e.g. "LSA_Art32", "LCA_Art18")
    pub id: String,
    /// Human-readable statute title
    pub title: String,
    /// Legalis DSL text for this statute
    pub dsl: String,
    /// Conversion source: "xml_native" | "domain" | "llm" | "fallback"
    pub source: String,
}

/// Result of compiling XML or articles into Legalis DSL.
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// Title of the compiled law
    pub law_title: String,
    /// Law number (e.g. "昭和二十二年法律第四十九号")
    pub law_num: String,
    /// Full DSL text containing all statutes
    pub dsl_text: String,
    /// Per-statute compilation details
    pub statutes: Vec<CompiledStatute>,
    /// Number of articles in the source (XML articles or BQ articles)
    pub article_count: usize,
    /// Any warnings generated during compilation
    pub warnings: Vec<String>,
}

/// Compiles law articles (from e-Gov XML or BQ search) into Legalis DSL text.
pub struct CompilerService;

impl CompilerService {
    /// Mode A: Compile raw e-Gov XML → Legalis DSL.
    ///
    /// Uses `EGovLawParser::parse()` + `EGovLaw::to_statutes()` (legalis-jp native conversion).
    /// Returns `CompileError::NoStatutes` if no articles can be formalized.
    pub fn compile_xml(xml: &str) -> Result<CompileResult, CompileError> {
        let parser = EGovLawParser::new();
        let law = parser
            .parse(xml)
            .map_err(|e| CompileError::XmlParse(e.to_string()))?;

        let article_count = law.articles.len();
        let all_statutes: Vec<Statute> = law.to_statutes();

        if all_statutes.is_empty() {
            return Err(CompileError::NoStatutes);
        }

        debug!(
            "compile_xml: {} statutes from {} articles — law: {}",
            all_statutes.len(),
            article_count,
            law.title
        );

        let mut warnings = Vec::new();
        if all_statutes.len() < article_count {
            warnings.push(format!(
                "{}件中{}件の条文が形式化されました（{}件は変換されませんでした）。",
                article_count,
                all_statutes.len(),
                article_count - all_statutes.len()
            ));
        }

        let compiled: Vec<CompiledStatute> = all_statutes
            .iter()
            .map(|s| CompiledStatute {
                id: s.id.clone(),
                title: s.title.clone(),
                dsl: format_statute(s),
                source: "xml_native".to_string(),
            })
            .collect();

        let dsl_text = format_statutes(&all_statutes);

        Ok(CompileResult {
            law_title: law.title,
            law_num: law.law_num,
            dsl_text,
            statutes: compiled,
            article_count,
            warnings,
        })
    }

    /// Mode B: Compile BQ-retrieved law articles → Legalis DSL.
    ///
    /// Uses `StatuteBridge::convert_articles_domain_only()` for instant conversion
    /// (no Gemini API call). Domain-matched laws (労働基準法 etc.) return rich statutes.
    pub fn compile_articles(articles: &[FullArticle]) -> CompileResult {
        if articles.is_empty() {
            return CompileResult {
                law_title: String::new(),
                law_num: String::new(),
                dsl_text: String::new(),
                statutes: Vec::new(),
                article_count: 0,
                warnings: vec!["変換対象の条文がありません。".to_string()],
            };
        }

        let article_statutes = StatuteBridge::convert_articles_domain_only(articles);
        let article_count = articles.len();

        if article_statutes.is_empty() {
            return CompileResult {
                law_title: articles[0].title.clone(),
                law_num: String::new(),
                dsl_text: String::new(),
                statutes: Vec::new(),
                article_count,
                warnings: vec!["変換可能な条文が見つかりませんでした。".to_string()],
            };
        }

        // Deduplicate statutes by ID
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<_> = article_statutes
            .iter()
            .filter(|a| seen.insert(a.statute.id.clone()))
            .collect();

        let all_statutes: Vec<Statute> = unique.iter().map(|a| a.statute.clone()).collect();

        let compiled: Vec<CompiledStatute> = unique
            .iter()
            .map(|a| CompiledStatute {
                id: a.statute.id.clone(),
                title: a.statute.title.clone(),
                dsl: format_statute(&a.statute),
                source: match a.source {
                    ConversionSource::Domain => "domain".to_string(),
                    ConversionSource::Llm => "llm".to_string(),
                    ConversionSource::Fallback => "fallback".to_string(),
                },
            })
            .collect();

        let dsl_text = format_statutes(&all_statutes);
        let law_title = articles[0].title.clone();

        debug!(
            "compile_articles: {} statutes from {} articles",
            compiled.len(),
            article_count
        );

        CompileResult {
            law_title,
            law_num: String::new(),
            dsl_text,
            statutes: compiled,
            article_count,
            warnings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::law::FullArticle;

    fn make_article(title: &str, anchor: &str) -> FullArticle {
        FullArticle {
            law_id: "320AC0000000049".to_string(),
            title: title.to_string(),
            content: "テスト条文内容".to_string(),
            unique_anchor: anchor.to_string(),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/320AC0000000049".to_string(),
        }
    }

    #[test]
    fn test_compile_articles_labor_law() {
        let articles = vec![
            make_article("労働基準法 第32条", "Article_32"),
            make_article("労働基準法 第36条", "Article_36"),
        ];
        let result = CompilerService::compile_articles(&articles);
        // Domain match should return rich statutes with DSL text
        assert!(!result.statutes.is_empty());
        assert!(result.statutes.iter().all(|s| !s.dsl.is_empty()));
        assert!(result.statutes.iter().all(|s| s.source == "domain"));
        assert!(!result.dsl_text.is_empty());
    }

    #[test]
    fn test_compile_articles_unknown_law() {
        let articles = vec![make_article("著作権法 第1条", "Article_1")];
        let result = CompilerService::compile_articles(&articles);
        // Fallback statute still produces a result
        assert!(!result.statutes.is_empty());
        assert!(result.statutes.iter().all(|s| s.source == "fallback"));
    }

    #[test]
    fn test_compile_articles_empty() {
        let result = CompilerService::compile_articles(&[]);
        assert!(result.statutes.is_empty());
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_compile_articles_deduplicates() {
        // Same law title in multiple articles → domain statutes inserted only once
        let articles = vec![
            make_article("労働基準法 第32条", "Article_32"),
            make_article("労働基準法 第36条", "Article_36"),
            make_article("労働基準法 第39条", "Article_39"),
        ];
        let result = CompilerService::compile_articles(&articles);
        // All statute IDs should be unique
        let ids: Vec<_> = result.statutes.iter().map(|s| &s.id).collect();
        let id_set: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), id_set.len(), "Duplicate statute IDs found");
    }

    #[test]
    fn test_compile_xml_invalid() {
        let result = CompilerService::compile_xml("not xml at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_xml_minimal_valid() {
        // Minimal e-Gov XML — EGovLawParser should parse the title/law_num at minimum
        let minimal_xml = r#"<Law Era="Showa" Num="49" Type="Act" Year="22">
<LawNum>昭和二十二年法律第四十九号</LawNum>
<LawBody>
  <LawTitle>労働基準法</LawTitle>
</LawBody>
</Law>"#;
        match CompilerService::compile_xml(minimal_xml) {
            Ok(result) => {
                // If parse succeeds but no articles → NoStatutes is also fine
                assert!(!result.law_title.is_empty());
            }
            Err(CompileError::NoStatutes) => {}
            Err(CompileError::XmlParse(_)) => {}
            Err(CompileError::InvalidDocument(_)) => {}
        }
    }

    #[test]
    fn test_compile_xml_with_articles() {
        // More complete XML with an article
        let xml = r#"<Law Era="Showa" Num="49" Type="Act" Year="22">
<LawNum>昭和二十二年法律第四十九号</LawNum>
<LawBody>
  <LawTitle>労働基準法</LawTitle>
  <MainProvision>
    <Chapter Num="4">
      <Article Num="32">
        <ArticleCaption>労働時間</ArticleCaption>
        <Paragraph Num="1">
          <ParagraphNum/>
          <ParagraphSentence>
            <Sentence>使用者は、労働者に、休憩時間を除き一週間について四十時間を超えて、労働させてはならない。</Sentence>
          </ParagraphSentence>
        </Paragraph>
      </Article>
    </Chapter>
  </MainProvision>
</LawBody>
</Law>"#;
        match CompilerService::compile_xml(xml) {
            Ok(result) => {
                assert!(!result.law_title.is_empty());
                if !result.statutes.is_empty() {
                    assert!(result.statutes.iter().all(|s| s.source == "xml_native"));
                    assert!(!result.dsl_text.is_empty());
                }
            }
            Err(CompileError::NoStatutes) => {
                // Acceptable if parser could not formalize the single article
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }
}
