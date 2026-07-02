# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-07-02

### Added
- `/simulate` (HTTP and CLI): selectable demographic profile (`jp_2024`/`us_2024`/`eu_2024`) via `profile_by_name()`, with US and EU census-based profiles added alongside the existing JP one; unknown values are now rejected with a clear error instead of silently falling back to Japan.
- Real OxiZ SMT-backed contradiction detection (`SmtVerifier::is_satisfiable`) in `contradiction.rs`, replacing a discriminant-based heuristic, via `legalis-verifier`'s `smt-solver` feature (Pure Rust, no C/C++).
- Restored ID-collision, jurisdictional-overlap, and temporal-conflict detection in `contradiction.rs` (faithfully ported from `legalis-verifier`'s internal heuristics).
- Multi-jurisdiction support: a new `MultiJurisdictionMatcher` registry adds EU (GDPR Art. 6/7/15/17/33 — lawful processing, consent, right of access, right to erasure, breach notification) and US (FLSA §§206–207, ADA §12112, FMLA §2612) statute seed sets alongside JP (`eu_statutes.rs`/`us_statutes.rs`). A new `-j`/`--jurisdiction` CLI flag (default `JP`) selects the jurisdiction on the `query`, `compile`, `simulate`, `formalize`, and `predict` subcommands; on the HTTP side only `POST /` gains an optional `jurisdiction` request field (threaded end-to-end through contradiction detection and verification annotation) — `/compile`, `/simulate`, `/formalize`, and `/predict-ruling` do not yet expose a `jurisdiction` field and remain JP-only. A new `GET /jurisdictions` endpoint lists the registered jurisdictions and the default code.
- JP domain-matched statute coverage expanded: `JpDomainMatcher` now also instantly resolves (no Gemini call) the Civil Code's general and tort provisions (民法, including 公序良俗 Art.90, 債務不履行 Art.415, and 解除 Art.541), the Companies Act/Commercial Code (会社法/商法), the Constitution of Japan, the Copyright and Patent Acts (著作権法/特許法), environmental law (air/water pollution, waste management, environmental impact assessment), the Administrative Procedure Act (行政手続法), and Construction Business/Real Estate Brokerage Law (建設業法/宅地建物取引業法).
- New `POST /predict-ruling` endpoint and `oxigenai predict` (alias `pred`) CLI subcommand implementing Generative Jurisprudence (生成的法解釈): searches a curated corpus of 8 real Japanese landmark precedents spanning labor, constitutional, and civil/privacy law, ranks them by relevance × precedent authority, then combines a Gemini web-grounded retrieval call with a temperature-0 deterministic synthesis call to produce a predicted holding, precedent-grounded reasoning, and a confidence assessment — always classified as `LegalResult::JudicialDiscretion`.
- New `POST /translate-check` endpoint and `oxigenai translate-check` (alias `txcheck`) CLI subcommand: verifies a statute and its translation carry the same legal meaning by compiling both e-Gov XML documents to Legalis DSL and comparing the resulting statute sets structurally (statute count, effect-type multiset, and recursive precondition-kind multiset combined into a weighted Sørensen–Dice score) — fully offline and language-agnostic, with no machine translation involved.
- New self-contained single-page WebUI (`static/index.html`, embedded at compile time) served at `GET /` and `GET /ui`, with no external CDN or network assets so it works in air-gapped deployments.
- All three demographic profiles (`jp_2024`/`us_2024`/`eu_2024`) now also sample a `weekly_hours` attribute (Normal distribution centered on the 40-hour statutory workweek), enabling simulation of hours-based statute preconditions such as FLSA overtime.
- New criterion benchmark suite (`benches/domain_matching.rs`, `dsl_compile.rs`, `contradiction_smt.rs`, `simulation.rs`, `jurisprudence.rs`, run via `cargo bench`) covering domain matching, DSL compilation, SMT contradiction detection, population simulation, and precedent search/ranking — all pure, offline hot paths; adds a `criterion` 0.8 dev-dependency with the `async_tokio` feature.

### Changed
- Seed statutes (US/EU/JP) had numeric and categorical preconditions converted from free-text `Condition::Custom` to structured, SMT-verifiable conditions (`Threshold`/`Duration`/`AttributeEquals`/`SetMembership`); genuinely open-textured legal standards (constitutional balancing, fair use, public-policy clauses) were deliberately retained as `Custom`.
- `legalis-core`/`legalis-verifier`/`legalis-sim`/`legalis-dsl`/`legalis-jp` dependencies bumped 0.1.5 → 0.1.6.
- `tower-http` bumped 0.6 → 0.7; `google-cloud-auth` bumped 1.9 → 1.13; `mockall` (dev-dependency) bumped 0.13 → 0.15.
- `reqwest` bumped 0.12 → 0.13: the removed `rustls-tls` convenience feature is replaced with `rustls`, which now pulls in `rustls-platform-verifier` for OS-native certificate-store verification, replacing the previous bundled `webpki-roots` trust store (the TLS implementation itself remains pure rustls; no native-tls/OpenSSL).
- Static regex compilation (`pipeline.rs`, `prompts/builder.rs`, `gemini_client.rs`, `report_utils.rs`) now uses `.expect()` with descriptive invariant messages instead of bare `.unwrap()`.
- Removed the unused `mockall` and `tokio-test` dev-dependencies from `Cargo.toml`, confirmed to have zero references anywhere in the source via `cargo +nightly udeps` and a source grep; `criterion` is now the sole remaining dev-dependency.
- `.github/workflows/ci.yml` and `.github/workflows/release.yml` were renamed to `ci.yml.disabled` and `release.yml.disabled` per COOLJAPAN CI cost-control policy, which permits only `pypi-publish.yml`/`npm-publish.yml` to remain active and this project has neither — no CI currently runs automatically on push/PR. `ci.yml` previously ran fmt/clippy/nextest/build validation and `release.yml` was a manual-dispatch-only workflow that built a downloadable binary artifact and never published anything; both files are retained with their content preserved for future re-enabling.

### Fixed
- `/formalize`: preconditions using `Custom` or other unstructured conditions were previously treated as trivially satisfied regardless of the submitted facts, always reporting `Deterministic`; the evaluation engine now uses `Condition::evaluate` per precondition and correctly reports `JudicialDiscretion` when a condition is qualitative or unsatisfied by the facts.
- `/formalize`: evaluation errors other than qualitative `Custom` conditions (e.g. a precondition referencing a missing fact/attribute) previously produced a `Void` result; they now correctly produce `JudicialDiscretion` instead, since `Void` is reserved exclusively for the OxiZ SMT contradiction detector.
- `/simulate` (HTTP and CLI): the `profile` request/flag value was accepted but silently ignored, always using the Japanese demographic profile regardless of what was requested.
- `gemini_client.rs`: removed a dead code path in `extract_grounding_web_hits` that read `searchEntryPoint.renderedContent` and immediately discarded it.
- `.gitignore` was ignoring `Cargo.lock` despite `oxigenai` being a binary application rather than a library, contradicting the file's own comment ("keep for binaries, ignore for libraries") and standard Rust guidance to commit `Cargo.lock` for reproducible builds; the ignore rule was removed and `Cargo.lock` is now tracked in git for the first time.

### Security
- `cargo audit` reports two HIGH severity (CVSS 7.5) RUSTSEC advisories in the transitive dependency `quick-xml 0.40.1`: `RUSTSEC-2026-0195` (unbounded namespace-declaration allocation in `NsReader`, enabling memory-exhaustion denial of service) and `RUSTSEC-2026-0194` (quadratic run time when checking a start tag for duplicate attribute names). `quick-xml 0.40.1` is pulled in transitively via `legalis-core 0.1.6` and `legalis-jp 0.1.6`, both of which declare `quick-xml = "0.40"` (i.e. `^0.40`) in their published `Cargo.toml`.
- Not fixable from `oxigenai` alone: a `[patch.crates-io]` override (tried both as a bare version bump and as a git-tag-pinned `quick-xml` 0.41.0) was attempted and confirmed not to work, because Cargo's SemVer contract enforcement will not let a patched dependency violate the `^0.40` requirement declared by `legalis-core`, so the resolver falls back to the original `0.40.1` for that edge every time. The actual fix requires `legalis-core`/`legalis-jp` to publish a new version with `quick-xml` bumped to `>=0.41`; this fix already exists, unpublished, in the upstream Legalis-RS source at version 0.1.7 — pending that release, this advisory remains open and tracked.

## [0.1.0] - 2026-05-07

### Initial
- Axum REST API with full endpoint compatibility with the Digital Agency's Genai system
- Six legal report generation modes: definition, procedure, comparison, interpretation, policy research, and comprehensive analysis
- OxiZ SMT-based automatic logical contradiction detection in statutory provisions
- `LegalResult<T>` classification for deterministic/discretionary outcome separation via Legalis-RS
- BigQuery vector search over e-Gov statutory XML corpus
- Web grounding via Google Search integration for law name inference
- Token usage and cost tracking per request
- Statute XML to Legalis DSL compiler (`/compile` endpoint + `oxigenai compile` CLI)
- Policy simulation engine (`/simulate` endpoint + `oxigenai simulate` CLI) backed by a Japan 2024 census-based population model supporting up to 100,000 agents
- Per-article statistics output for deterministic application rate, discretionary rate, and contradiction rate
- Legal fact formalization and applicability determination (`/formalize` endpoint + `oxigenai formalize` CLI)
- Unified single binary (`oxigenai`) serving both HTTP server and all CLI subcommands
- `oxigenai serve` / `server` for launching the HTTP server with configurable port
- Short command aliases: `q` (query), `sim` (simulate), `eval` (formalize)
- JSON output mode (`--json`) producing the same schema as the HTTP API
- Integration with Vertex AI Gemini 2.5 Flash for report generation and law name inference
- Legalis-RS framework integration (legalis-core, legalis-verifier, legalis-dsl, legalis-sim, legalis-jp)

[0.1.1]: https://github.com/cool-japan/oxigenai/releases/tag/v0.1.1
[0.1.0]: https://github.com/cool-japan/oxigenai/releases/tag/v0.1.0
