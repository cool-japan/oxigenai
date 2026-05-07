# OxigenAI TODO

## Phase 1（MVP）— ✅ 完了

### ✅ Step 1: プロジェクトスキャフォールド
- [x] Cargo.toml（全依存関係）
- [x] LICENSE (Apache-2.0)
- [x] README.md（日本語+英語）

### ✅ Step 2: エラー型・設定
- [x] `src/error.rs` — OxigenError enum
- [x] `src/config.rs` — AppConfig + from_env()

### ✅ Step 3: データモデル
- [x] `src/models/mod.rs`
- [x] `src/models/request.rs` — RequestBody, FileInput, GroundingMode
- [x] `src/models/response.rs` — ResponseBody, UsageSummaryEntry, EstimatedCostInfo
- [x] `src/models/law.rs` — LawCandidate, FullArticle, LawNamesEstimation, SelectionResult, WebHit
- [x] `src/models/legal_result.rs` — ClassifiedSection, VerificationSummary

### ✅ Step 4: プロンプトテンプレート・ビルダー
- [x] `src/prompts/templates.rs` — PROMPT_SELECT_RELEVANT_ARTICLES, PROMPT_GENERATE_COMPLETE_REPORT（6パターン）
- [x] `src/prompts/builder.rs` — 法令名推定・条文選択・レポート生成プロンプト構築

### ✅ Step 5: サービス層
- [x] `src/services/usage_tracker.rs` — UsageTracker, MODEL_PRICING
- [x] `src/services/report_utils.rs` — ReferenceItem, format_reference_for_prompt, filter_references_by_citations
- [x] `src/services/gemini_client.rs` — GeminiService, call_with_grounding, call_strict, call_for_report
- [x] `src/services/bq_retriever.rs` — BigQueryRetriever, VECTOR_SEARCH, get_articles_by_nearest_law

### ✅ Step 6: Legalis-RS 検証統合
- [x] `src/verifier/integration.rs` — LegalVerifier, build_contradiction_prefix, annotate_report
- [x] `src/verifier/contradiction.rs` — detect_smt_contradictions（補完条文フィルタリング込み）, format_contradiction_warnings
- [x] `src/verifier/dsl_bridge.rs` — JpDomainMatcher, DslTranslator（並行Gemini呼び出し）, StatuteBridge
- [x] `src/verifier/formalize.rs` — FormalizeService, UserFacts, FactEvaluation

### ✅ Step 7: パイプラインオーケストレータ
- [x] `src/services/pipeline.rs` — PipelineContext, generate_law_report（メイン関数）

### ✅ Step 8: HTTP ハンドラ・エントリポイント（統一CLIバイナリ）
- [x] `src/handlers/report.rs` — POST /、GET /health
- [x] `src/handlers/compile.rs` — POST /compile（Mode A: XML、Mode B: クエリ）
- [x] `src/handlers/simulate.rs` — POST /simulate
- [x] `src/handlers/formalize.rs` — POST /formalize
- [x] `src/main.rs` — 統一CLIバイナリ（query/serve/compile/simulate/formalize サブコマンド）

### ✅ Step 9: 品質保証
- [x] `cargo nextest run` — 107/107 テスト通過
- [x] `cargo clippy -- -D warnings` — ゼロ警告
- [x] `cargo install --path .` — バイナリインストール済み

---

## Phase 2 — ✅ 完了

- [x] **法令 XML → Legalis DSL 自動コンパイル**
  - [x] `src/verifier/compiler.rs` — CompilerService（compile_xml / compile_articles）
  - [x] `src/handlers/compile.rs` — POST /compile
  - [x] `oxigenai compile` CLIサブコマンド
- [x] **政策シミュレーション機能**（legalis-sim）
  - [x] `src/verifier/simulator.rs` — SimulatorService, jp_2024_profile(), SimulationResult
  - [x] `src/handlers/simulate.rs` — POST /simulate
  - [x] `oxigenai simulate` CLIサブコマンド
- [x] **OxiZ SMT 誤検知修正** — 補完条文パターン（基本規定＋例外規定）を正常フィルタリング
- [x] **CLIバイナリ統一** — `oxigenai` + `oxigenai-query` → `oxigenai` 単体（サブコマンド方式）
- [x] **Gemini DSL呼び出し並行化** — 非ドメインマッチ条文の並行処理（`join_all`）

---

## Phase 3（中長期）

- [ ] 23 法域対応（EU・US・JP 横断法令検索）
- [ ] Generative Jurisprudence（判例案自動生成）
- [ ] GPU 対応 legalis-sim
- [ ] WebUI 統合（genai-web 互換）
- [ ] GitHub Actions CI/CD
- [ ] パフォーマンスベンチマーク（Python 版との比較）

---

## 参考実装 / Reference Implementation

- 源内（Python）: `/mnt/g/tmp/genai-ai-api/google-cloud/lawsy-custom-bq/`
- Legalis-RS: https://github.com/cool-japan/legalis
- OxiZ SMT: https://crates.io/crates/legalis-verifier
