//! Population-simulation benchmarks (async, offline).
//!
//! Measures `oxigenai::verifier::simulator::SimulatorService::run` — generate a
//! Japanese population via `jp_2024_profile()`, apply the labor statutes through
//! the legalis-sim ECS engine, and collect metrics. This is async, so it uses
//! criterion's `async_tokio` executor (`b.to_async(&rt).iter(..)`).
//!
//! Population sizes are kept modest (100 / 1 000) so the bench stays quick; the
//! group reports `Throughput::Elements` (agents/s). The statute set is sourced
//! offline through the domain matcher. See `benches/domain_matching.rs` for run
//! instructions and the Python-baseline comparison notes.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use legalis_core::Statute;
use oxigenai::verifier::dsl_bridge::JpDomainMatcher;
use oxigenai::verifier::simulator::{SimulationConfig, SimulatorService, jp_2024_profile};
use std::hint::black_box;
use std::time::Duration;

/// Population sizes to sweep. Kept small so the suite remains quick to run.
const POPULATIONS: &[usize] = &[100, 1_000];

/// The 労働基準法 domain set, sourced offline through the domain matcher.
fn labor_statutes() -> Vec<Statute> {
    JpDomainMatcher::match_law_title("労働基準法").expect("労働基準法 must domain-match")
}

/// `SimulatorService::run` on the labor statutes at each population size.
fn bench_simulation(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("build tokio runtime");
    let statutes = labor_statutes();

    let mut group = c.benchmark_group("simulation/run_labor");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    for &population in POPULATIONS {
        group.throughput(Throughput::Elements(population as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(population),
            &population,
            |b, &population| {
                b.to_async(&runtime).iter(|| {
                    // Clone the (tiny) statute set per iteration: `run` takes it by value.
                    let statutes = statutes.clone();
                    async move {
                        let config = SimulationConfig {
                            population_size: population,
                            profile: jp_2024_profile(),
                        };
                        black_box(
                            SimulatorService::run(black_box(statutes), &config)
                                .await
                                .expect("simulation run"),
                        )
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_simulation);
criterion_main!(benches);
