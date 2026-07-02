//! Generative-jurisprudence offline benchmarks (precedent search + ranking).
//!
//! Measures the **pure / offline** portion of `oxigenai::verifier::jurisprudence`
//! (the compute behind `POST /predict-ruling`, excluding the two Gemini calls):
//!
//! * `CaseLawPredictor::search_local` — area-filtered + keyword-only passes over
//!   the curated in-memory landmark-precedent corpus, merged + de-duplicated.
//! * `rank_results` — ranks results by `relevance_score x precedent_authority`
//!   and keeps the top-K.
//!
//! See `benches/domain_matching.rs` for run instructions and the Python-baseline
//! comparison notes.

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use legalis_jp::case_law::LegalArea;
use oxigenai::verifier::jurisprudence::{CaseLawPredictor, rank_results};
use std::hint::black_box;

/// Build the labeled (area, keywords) search cases.
fn search_cases() -> Vec<(&'static str, LegalArea, Vec<String>)> {
    vec![
        (
            "labor_dismissal",
            LegalArea::Labor,
            vec!["解雇".to_string(), "安全配慮義務".to_string()],
        ),
        (
            "civil_privacy",
            LegalArea::Civil,
            vec!["プライバシー".to_string(), "個人情報".to_string()],
        ),
    ]
}

/// `CaseLawPredictor::search_local` across a couple of representative queries.
fn bench_search_local(c: &mut Criterion) {
    let predictor = CaseLawPredictor::new();
    let cases = search_cases();
    let mut group = c.benchmark_group("jurisprudence/search_local");
    for (label, area, keywords) in &cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            keywords,
            |b, keywords| {
                b.iter(|| {
                    black_box(
                        predictor.search_local(black_box(*area), black_box(keywords.as_slice())),
                    )
                });
            },
        );
    }
    group.finish();
}

/// `rank_results` on a freshly-searched labor result set (search is the untimed setup).
fn bench_rank_results(c: &mut Criterion) {
    let predictor = CaseLawPredictor::new();
    let keywords = vec!["解雇".to_string(), "安全配慮義務".to_string()];
    c.bench_function("jurisprudence/rank_results", |b| {
        b.iter_batched(
            || predictor.search_local(LegalArea::Labor, &keywords),
            |results| black_box(rank_results(black_box(results))),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_search_local, bench_rank_results);
criterion_main!(benches);
