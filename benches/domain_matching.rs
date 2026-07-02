//! # OxigenAI (源内) offline hot-path benchmarks
//!
//! Criterion micro-benchmarks for the **pure, offline** compute stages of
//! OxigenAI's legal-AI pipeline. None of these benchmarks touch the network,
//! Gemini, or BigQuery — every input is built from in-crate offline APIs, so the
//! whole suite runs deterministically and reproducibly on any machine.
//!
//! ## Benchmark groups (one `[[bench]]` file each)
//!
//! | bench file            | group(s)                                   | measures (exact API)                                                                 |
//! |-----------------------|--------------------------------------------|--------------------------------------------------------------------------------------|
//! | `domain_matching`     | `domain_matching/*`                        | [`JpDomainMatcher::match_law_title`] + [`MultiJurisdictionMatcher::match_for`] (JP/EU/US) |
//! | `dsl_compile`         | `dsl_compile/*`                            | `CompilerService::compile_xml` (e-Gov XML → Legalis DSL) + `compile_articles`         |
//! | `contradiction_smt`   | `contradiction_smt/*`                      | `detect_smt_contradictions` (pure OxiZ SMT) + `run_full_verification` on domain statutes |
//! | `simulation`          | `simulation/*`                             | `SimulatorService::run` (async, `jp_2024_profile()`) at population 100 / 1 000        |
//! | `jurisprudence`       | `jurisprudence/*`                          | `CaseLawPredictor::search_local` + `rank_results` (offline precedent search/ranking)  |
//!
//! ## How to run
//!
//! ```bash
//! cargo bench                       # run the whole suite (SMT + simulation are slow)
//! cargo bench --bench domain_matching   # one group only
//! cargo bench --bench domain_matching -- --warm-up-time 0.5 --measurement-time 1   # quick smoke
//! cargo bench -- --save-baseline main    # record a baseline …
//! cargo bench -- --baseline main         # … then compare a later run against it
//! ```
//!
//! HTML reports (criterion default feature) land in `target/criterion/report/index.html`.
//!
//! ## Comparing against the Python 源内 baseline
//!
//! The original 源内 (`lawsy-custom-bq`) ran as Python **GCP Cloud Functions**: each
//! request fanned out to BigQuery vector search, e-Gov fetches, and several Gemini
//! calls. Its end-to-end latency (p50 / p95) and throughput were dominated by
//! **network / GCP round-trips**, not by the legal-reasoning compute itself.
//!
//! OxigenAI moves the deterministic legal-reasoning stages **in-process and pure
//! Rust**. These benchmarks isolate exactly those stages — domain matching, DSL
//! compilation, SMT contradiction detection, and population simulation — so their
//! numbers are directly comparable to the *compute-only* slice of the Python
//! pipeline. When comparing, contrast:
//!
//! * **Latency** — criterion's per-iteration time (point estimate ± CI) here vs the
//!   Python stage's p50 / p95 wall-clock (which folded in GCP round-trips).
//! * **Throughput** — `simulation` and `contradiction_smt` report
//!   `Throughput::Elements` (agents/s, statutes/s); compare against the Python
//!   stage's sustained ops/s.
//!
//! The expectation: stages that were *seconds* in Python (because of the GCP
//! round-trip) collapse to *micro-/milliseconds* here, because the Rust path keeps
//! the work local and only the genuinely network-bound stages (Gemini synthesis)
//! remain — and those are deliberately excluded from these offline benchmarks.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxigenai::verifier::dsl_bridge::JpDomainMatcher;
use oxigenai::verifier::jurisdiction::MultiJurisdictionMatcher;
use std::hint::black_box;

/// Representative Japanese law titles spanning several matched domains plus one
/// intentionally unmatched title (道路交通法) that exercises the `None` fast path.
const JP_TITLES: &[(&str, &str)] = &[
    ("labor_standards", "労働基準法 第32条"),
    ("personal_info", "個人情報の保護に関する法律"),
    ("civil_code", "民法"),
    ("copyright", "著作権法"),
    ("unmatched", "道路交通法"),
];

/// (label, jurisdiction code, law title) tuples for the multi-jurisdiction matcher.
const JURISDICTION_CASES: &[(&str, &str, &str)] = &[
    ("jp", "JP", "労働基準法"),
    ("eu", "EU", "GDPR personal data processing"),
    ("us", "US", "FLSA overtime pay"),
];

/// `JpDomainMatcher::match_law_title` — keyword → pre-built `Vec<Statute>` lookup.
fn bench_jp_match_law_title(c: &mut Criterion) {
    let mut group = c.benchmark_group("domain_matching/jp_match_law_title");
    for &(label, title) in JP_TITLES {
        group.bench_with_input(BenchmarkId::from_parameter(label), &title, |b, &title| {
            b.iter(|| black_box(JpDomainMatcher::match_law_title(black_box(title))));
        });
    }
    group.finish();
}

/// `MultiJurisdictionMatcher::match_for` dispatched across JP / EU / US.
///
/// The registry is built once (the realistic hot path: construct once, match many).
fn bench_multi_jurisdiction_match_for(c: &mut Criterion) {
    let registry = MultiJurisdictionMatcher::new();
    let mut group = c.benchmark_group("domain_matching/multi_jurisdiction_match_for");
    for &(label, code, title) in JURISDICTION_CASES {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &(code, title),
            |b, &(code, title)| {
                b.iter(|| black_box(registry.match_for(black_box(code), black_box(title))));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_jp_match_law_title,
    bench_multi_jurisdiction_match_for
);
criterion_main!(benches);
