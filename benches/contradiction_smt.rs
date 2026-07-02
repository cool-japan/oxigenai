//! OxiZ SMT contradiction-detection benchmarks (pure, offline).
//!
//! Measures the Legalis-RS SMT path directly on pre-built domain statutes — the
//! `Vec<legalis_core::Statute>` entry points, **not** the Gemini-driven verifier:
//!
//! * `detect_smt_contradictions` — the pure OxiZ SMT detector (pairwise conflict
//!   search + complementary-pattern filtering). Swept over increasing statute-set
//!   sizes built from in-crate offline builders.
//! * `run_full_verification` — the fuller pure pipeline (SMT + constitutional
//!   checks + quality grading) on the labor statute set.
//!
//! SMT is the slowest stage, so the group uses a small `sample_size` and a bounded
//! `measurement_time`. See `benches/domain_matching.rs` for run / baseline notes.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use legalis_core::Statute;
use oxigenai::verifier::contradiction::{detect_smt_contradictions, run_full_verification};
use oxigenai::verifier::dsl_bridge::JpDomainMatcher;
use oxigenai::verifier::jp_statutes::{
    all_commercial_statutes, all_constitution_statutes, all_pipa_statutes,
};
use std::hint::black_box;
use std::time::Duration;

/// The 労働基準法 domain set (LSA articles + overtime regulation) — the canonical
/// SMT input, sourced offline through the domain matcher.
fn labor_statutes() -> Vec<Statute> {
    JpDomainMatcher::match_law_title("労働基準法").expect("労働基準法 must domain-match")
}

/// Increasing statute sets to expose the (roughly O(n^2)) pairwise SMT cost:
/// labor → labor+PIPA → labor+PIPA+commercial+constitution.
fn statute_sets() -> Vec<Vec<Statute>> {
    let labor = labor_statutes();

    let mut medium = labor.clone();
    medium.extend(all_pipa_statutes());

    let mut large = medium.clone();
    large.extend(all_commercial_statutes());
    large.extend(all_constitution_statutes());

    vec![labor, medium, large]
}

/// `detect_smt_contradictions` over domain statute sets of increasing size.
fn bench_detect_smt(c: &mut Criterion) {
    let sets = statute_sets();
    let mut group = c.benchmark_group("contradiction_smt/detect_smt_contradictions");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));
    for statutes in &sets {
        let size = statutes.len();
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            statutes,
            |b, statutes| {
                b.iter(|| black_box(detect_smt_contradictions(black_box(statutes.as_slice()))));
            },
        );
    }
    group.finish();
}

/// `run_full_verification` (SMT + constitutional checks + quality grading) on labor.
fn bench_full_verification(c: &mut Criterion) {
    let statutes = labor_statutes();
    let mut group = c.benchmark_group("contradiction_smt/run_full_verification");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));
    group.throughput(Throughput::Elements(statutes.len() as u64));
    group.bench_function("labor", |b| {
        b.iter(|| black_box(run_full_verification(black_box(statutes.as_slice()))));
    });
    group.finish();
}

criterion_group!(benches, bench_detect_smt, bench_full_verification);
criterion_main!(benches);
