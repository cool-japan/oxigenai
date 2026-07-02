//! DSL-compilation benchmarks — e-Gov XML / BQ articles → Legalis DSL.
//!
//! Measures the two offline entry points of [`oxigenai::verifier::compiler::CompilerService`]:
//!
//! * `compile_xml` — parses an embedded e-Gov-style XML fixture (a 労働基準法 `Law`
//!   with several `Article`/`Paragraph` nodes) via `EGovLawParser` and converts it
//!   to Legalis DSL with `EGovLaw::to_statutes()` + `format_statutes()`.
//! * `compile_articles` — domain-only conversion of a `Vec<FullArticle>` through
//!   `StatuteBridge::convert_articles_domain_only` (no Gemini call).
//!
//! Both are fully offline. See `benches/domain_matching.rs` for run instructions
//! and the Python-baseline comparison notes.

use criterion::{Criterion, criterion_group, criterion_main};
use oxigenai::models::law::FullArticle;
use oxigenai::verifier::compiler::CompilerService;
use std::hint::black_box;

/// Small but realistic e-Gov XML fixture: 労働基準法 with three articles, each
/// carrying a paragraph whose sentence uses an explicit deontic marker
/// (してはならない / なければならない) so the legalis-jp converter can formalize it.
const EGOV_XML: &str = r#"<Law Era="Showa" Num="49" Type="Act" Year="22">
<LawNum>昭和二十二年法律第四十九号</LawNum>
<LawBody>
  <LawTitle>労働基準法</LawTitle>
  <MainProvision>
    <Chapter Num="4">
      <ChapterTitle>第四章　労働時間、休憩、休日及び年次有給休暇</ChapterTitle>
      <Article Num="32">
        <ArticleCaption>（労働時間）</ArticleCaption>
        <ArticleTitle>第三十二条</ArticleTitle>
        <Paragraph Num="1">
          <ParagraphNum/>
          <ParagraphSentence>
            <Sentence>使用者は、労働者に、休憩時間を除き一週間について四十時間を超えて、労働させてはならない。</Sentence>
          </ParagraphSentence>
        </Paragraph>
        <Paragraph Num="2">
          <ParagraphNum>２</ParagraphNum>
          <ParagraphSentence>
            <Sentence>使用者は、一週間の各日については、労働者に、休憩時間を除き一日について八時間を超えて、労働させてはならない。</Sentence>
          </ParagraphSentence>
        </Paragraph>
      </Article>
      <Article Num="34">
        <ArticleCaption>（休憩）</ArticleCaption>
        <ArticleTitle>第三十四条</ArticleTitle>
        <Paragraph Num="1">
          <ParagraphNum/>
          <ParagraphSentence>
            <Sentence>使用者は、労働時間が六時間を超える場合においては少なくとも四十五分、八時間を超える場合においては少なくとも一時間の休憩時間を労働時間の途中に与えなければならない。</Sentence>
          </ParagraphSentence>
        </Paragraph>
      </Article>
      <Article Num="35">
        <ArticleCaption>（休日）</ArticleCaption>
        <ArticleTitle>第三十五条</ArticleTitle>
        <Paragraph Num="1">
          <ParagraphNum/>
          <ParagraphSentence>
            <Sentence>使用者は、労働者に対して、毎週少なくとも一回の休日を与えなければならない。</Sentence>
          </ParagraphSentence>
        </Paragraph>
      </Article>
    </Chapter>
  </MainProvision>
</LawBody>
</Law>"#;

/// Build a batch of domain-matched `FullArticle`s (労働基準法) so `compile_articles`
/// exercises the real domain-conversion + DSL-formatting path rather than fallback.
fn labor_articles() -> Vec<FullArticle> {
    ["第32条", "第34条", "第35条", "第36条", "第39条"]
        .iter()
        .enumerate()
        .map(|(index, article)| FullArticle {
            law_id: "320AC0000000049".to_string(),
            title: format!("労働基準法 {article}"),
            content: "使用者は、所定の労働条件に関する義務を負う。".to_string(),
            unique_anchor: format!("Article_{}", index + 1),
            anchor: None,
            url: "https://laws.e-gov.go.jp/law/320AC0000000049".to_string(),
        })
        .collect()
}

/// `CompilerService::compile_xml` — e-Gov XML → Legalis DSL (parse + convert + format).
fn bench_compile_xml(c: &mut Criterion) {
    c.bench_function("dsl_compile/compile_xml", |b| {
        b.iter(|| black_box(CompilerService::compile_xml(black_box(EGOV_XML))));
    });
}

/// `CompilerService::compile_articles` — domain-only `Vec<FullArticle>` → Legalis DSL.
fn bench_compile_articles(c: &mut Criterion) {
    let articles = labor_articles();
    c.bench_function("dsl_compile/compile_articles", |b| {
        b.iter(|| black_box(CompilerService::compile_articles(black_box(&articles))));
    });
}

criterion_group!(benches, bench_compile_xml, bench_compile_articles);
criterion_main!(benches);
