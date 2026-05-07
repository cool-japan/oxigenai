# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/cool-japan/oxigenai/releases/tag/v0.1.0
