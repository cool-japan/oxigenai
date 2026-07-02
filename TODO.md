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

## Phase 3（中長期）— ✅ 完了（上流ブロックを除く）

> 詳細計画。各項目は独立ファイル分割で並行実装（subagent）。
> 完了基準: `cargo clippy --all-targets --all-features -- -D warnings` ゼロ警告 + `cargo nextest run --all-features` 全通過（182テスト）。

### 3-0. 法域カバレッジ拡大（JpDomainMatcher 全法域対応）— ✅ 完了
legalis-jp 0.1.5 の全ドメインへ拡張し検証カバレッジを最大化。
- [x] `src/verifier/jp_statutes.rs` — 民法(不法行為709/710/715・90/415/541)・商法/会社法・憲法・知的財産(著作権/特許)・環境法・行政手続法・建設業法/宅建業法 の statute ビルダー追加
- [x] `src/verifier/dsl_bridge.rs` — `JpDomainMatcher::match_law_title` に上記ドメインのキーワードマッチ追加（specific→generic 順）
- [x] 各ドメインのユニットテスト（123/123 通過）

### 3-1. 多法域対応（Multi-Jurisdiction）— ✅ 完了
- [x] `src/verifier/jurisdiction.rs` — `JurisdictionMatcher` trait + `MultiJurisdictionMatcher` registry（JP/EU/US）
- [x] `RequestBody` / CLI（全5サブコマンド `-j/--jurisdiction`）に `jurisdiction` 追加（デフォルト `JP`、後方互換）
- [x] pipeline（`ReportOptions`）→ bridge → verifier へ jurisdiction 伝搬。legalis-core hierarchy 活用
- [x] EU(GDPR Art.6/7/15/17/33) / US(FLSA/ADA/FMLA) を実シード条文として登録（`eu_statutes.rs`/`us_statutes.rs`）
- [x] `GET /jurisdictions` ディスカバリエンドポイント

### 3-2. Generative Jurisprudence（判例案自動生成）— ✅ 完了
- [x] `src/verifier/jurisprudence.rs` — `CaseLawPredictor`（判例検索 + Gemini web-grounded 取得 + 決定論的合成）
- [x] 実在ランドマーク判例8件をキュレーション（日本食塩製造/三菱樹脂/電通/住基ネット 他）
- [x] `LegalResult::JudicialDiscretion { narrative_hint }` に判例根拠を付与
- [x] `src/handlers/predict.rs` — POST `/predict-ruling`
- [x] `oxigenai predict` CLIサブコマンド

### 3-3. WebUI 統合 + 翻訳整合性検証 — ✅ 完了
- [x] **axum から自己完結型 SPA を配信** — `static/index.html`（640行・約29KB、インラインCSS/JSのみ）を `include_str!` で埋め込み、`src/handlers/web.rs::index` が `GET /` と `GET /ui` で配信
  - 法域セレクタ（`GET /jurisdictions` から動的取得）、クエリ/事実関係入力、3アクション（**法令レポート生成** `POST /` `{inputs:{input_text, jurisdiction}}`／**判例予測** `POST /predict-ruling` `{input, facts?}`／**シミュレーション** `POST /simulate` `{query, population_size}`）
  - 手書きの最小Markdownレンダラ（見出し/太字/斜体/リスト/リンク/コード/表/引用/水平線、HTMLエスケープ＋安全URLスキームのみ許可でXSS対策）。usageMetadata（トークン/コスト）表示。JSON `{error}` ／プレーンテキスト両形式のエラーを表示
  - `POST /` をメソッドルーティングで GET=UI / POST=API に統合（既存POST APIは不変）
  - **完全エアギャップ対応**: 外部CDN/フォント/JS/CSSを一切参照しない（テスト `test_index_html_is_air_gapped` で `http(s)://` 不在を保証）
- [x] **`/translate-check`（翻訳整合性検証）** — 原文・訳文の2つのe-Gov XMLを双方 Legalis DSL へコンパイルし、`legalis_core::Statute` 集合を**構造比較**（言語非依存）
  - `src/verifier/translate_check.rs` — 純粋比較ロジック（条文数・`EffectType`多重集合・前提条件ノード種別の再帰多重集合）。Sørensen–Dice係数で部分スコア算出、重み付き総合スコア＋差異リスト＋構造シグネチャを返す。オフライン単体テスト7件（恒等/効果反転/枝欠落/条文欠落 等の注入差異検出）
  - `src/handlers/translate.rs` — `POST /translate-check`（`{source_xml, target_xml, source_lang?, target_lang?}` → `{equivalent, score, count/effect/condition_score, divergences, *_signature, *_dsl}`）
  - `src/verifier/compiler.rs::compile_xml_to_statutes` — XML→生`Statute`集合（構造解析の基盤、`compile_xml` と同一パース経路）
  - `oxigenai translate-check --source a.xml --target b.xml [--json]` CLIサブコマンド（`known` リスト登録済み）
  - **legalis-i18n 調査結果**: 0.1.5 は採用見送り。公開APIは機械翻訳エンジン（`NeuralMachineTranslator`/`TranslationMemory`）・手動キュレーションの等価レジストリ（`RegulatoryEquivalenceMapper`/`TermEquivalence`）・引用書式（`CitationComponents`）が中心で、コンパイル済み `legalis_core::Statute` ADT の構造等価性を判定する機能は皆無。lib.rs も自動生成（`types_3..types_13` 等）で不透明。よって**legalis-i18n に依存せず**、自己完結の構造比較として実装（外部通信・MT不要で完全オフライン検証）

### 3-4. インフラ
- [x] GitHub Actions CI/CD（fmt + clippy -D warnings + nextest + build / 手動 release）
- [x] パフォーマンスベンチマーク（criterion: ドメインマッチ・DSLコンパイル・矛盾検出・シミュレーション・判例）

### 上流（legalis）対応済み
- [x] GPU 対応 legalis-sim — **実装完了**（2026-06-13、`legalis`）。legalis-sim に実 NVIDIA CUDA バックエンド（cudarc 0.19・NVRTC ランタイムカーネルコンパイル）を `cuda` フィーチャとして追加。`GpuExecutor::execute` / `evaluate_population_threshold`（`Condition::Threshold` の GPU 版）がデバイス上で実行され、GPU 不在時は CPU へ透過フォールバック。実機 NVIDIA RTX A4000（CUDA 12.0）で検証: `cargo nextest run --features cuda` 694 passing、`cargo clippy --all-targets --features cuda -- -D warnings` 警告ゼロ。詳細は `crates/legalis-sim/TODO.md` の「Real CUDA Acceleration」節。

---

## Phase 4 — 深度強化（監査由来 0.1.1）— ✅ 完了

> `/ultra` 監査により発見された実装ギャップ。TODO.md 上は Phase 1-3 が全て `[x]` だが、監査で「本物だが未加工」なギャップ（`/formalize` の潜在的誤判定バグを含む）が見つかったため着手。

- [x] Cargo.toml: legalis-* を 0.1.5 → 0.1.6 に更新（Latest crates policy。Cargo.lock は既に0.1.6解決済み） (planned 2026-07-02)
  - **Goal:** `Cargo.toml` の `legalis-{core,verifier,sim,dsl,jp}` 5行を `"0.1.6"` に統一し、pin と実解決バージョンを一致させる。
  - **Design:** バージョン文字列のみの変更。API破壊的変更は想定していない。
  - **Files:** `Cargo.toml`
  - **Prerequisites:** なし
  - **Tests:** 既存スイート（`cargo nextest run --all-features`）が緑のまま。
  - **Risk:** 0.1.6 で API drift があれば要修正。build/nextest で検知。

- [x] gemini_client.rs: `searchEntryPoint` の死んだ抽出コードを削除 (planned 2026-07-02)
  - **Goal:** `extract_grounding_web_hits` 内、`searchEntryPoint.renderedContent` を読んで即座に `let _ = points;` で捨てている no-op ブロックを削除する。
  - **Design:** 該当 `if let Some(points) = meta["searchEntryPoint"]["renderedContent"].as_str() { let _ = points; }` ブロックを丸ごと削除。`groundingChunks` からの hits 抽出ロジックは変更しない。
  - **Files:** `src/services/gemini_client.rs`
  - **Prerequisites:** なし
  - **Tests:** 既存の `extract_grounding_web_hits` テストが緑のまま。
  - **Risk:** なし（デッドコード削除のみ）。

- [x] /simulate: `profile` パラメータを実際に配線し、US/EU プロファイル + 条文関連属性サンプリングを追加 (planned 2026-07-02)
  - **Goal:** 現状 `simulate.rs:167` が `req.profile` を無視して常に `jp_2024_profile()` を使っている（"us_2024" 等を送っても黙って JP プロファイルが返る）バグを修正。`us_2024_profile()` / `eu_2024_profile()` を追加し、`profile_by_name()` で選択、未知の値は 400 で拒否。さらに各プロファイルへ `weekly_hours` 等の条文関連属性サンプリングを追加し、Phase 4 の構造化条件（下記）がシミュレーションで意味のある deterministic/discretion 分布を生むようにする。
  - **Design:** `simulator.rs` に `us_2024_profile()`（米国 2024 年央値: age ~38.9, income logNormal ~ln(80000)≈11.29）・`eu_2024_profile()`（EU-27 年央値 age ~44.4）を `jp_2024_profile()` と同じ形（Normal age, LogNormal income, Discrete employment_type_code）で追加し、`profile_by_name(&str) -> Option<DemographicProfile>` で `"jp_2024"|"us_2024"|"eu_2024"` を解決。`simulate.rs:167` を `profile_by_name(&req.profile)` に置き換え、`None` の場合 `StatusCode::BAD_REQUEST` で有効な値一覧を返す。3プロファイル全てに `weekly_hours`（Normal ~40）等、下記 Phase 4 条件で使う属性のサンプリングを追加し、`SimulatorService::run` の後処理で数値文字列属性として設定。
  - **Files:** `src/verifier/simulator.rs`, `src/handlers/simulate.rs`
  - **Prerequisites:** なし
  - **Tests:** `test_us_2024_profile_distributions`, `test_eu_2024_profile_distributions`, `test_profile_by_name_known`/`unknown`、ハンドラの不明プロファイル→400テスト、`all_us_federal_statutes()` に対するシミュレーションが非自明な deterministic/discretion 分布を生むテスト。
  - **Risk:** 低。プロファイル追加は加算的で既存動作を壊さない。

- [x] verifier: seed statute の `Condition::Custom` を構造化 SMT 条件へ変換（US/EU/JP） (planned 2026-07-02)
  - **Goal:** `us_statutes.rs` / `eu_statutes.rs` / `jp_statutes.rs` にある、数値・カテゴリ閾値がフリーテキストに埋もれた `Condition::Custom` を、`legalis-jp` の `statute_adapter` パターンに倣った構造化 `Condition`（`Threshold`/`Duration`/`AttributeEquals`/`SetMembership`）に変換する。真に開放的（裁量的）な規範は `Custom` のまま維持し、その理由をコメントで明記する。
  - **Design:** US: `FLSA_Sec207`→`And(AttributeEquals{employee_classification=="non_exempt"}, Threshold{[("weekly_hours",1.0)],GreaterThan,40.0})`（`DurationUnit` に `Hours` が無いため `Duration` ではなく `Threshold`）。`FLSA_Sec206`→`AttributeEquals{employee_classification=="non_exempt"}`。`ADA_Sec12112`→`And(Threshold{[("employee_count",1.0)],GreaterOrEqual,15.0}, Custom{障害配慮のバランシング})`。`FMLA_Sec2612`→`And(And(Duration{≥12 Months}, Threshold{[("hours_worked_12mo",1.0)],GreaterOrEqual,1250.0}), Custom{該当事由})`。EU: `GDPR_Art6`→`SetMembership{lawful_basis ∈ {consent,contract,legal_obligation,vital_interests,public_task,legitimate_interests}}`。`Art15`→`AttributeEquals{request_type=="access"}`。`Art17`→`And(AttributeEquals{request_type=="erasure"}, Custom{優越的保持根拠の有無})`。`Art33`→`AttributeEquals{breach_occurred=="true"}`。`Art7` は既に構造化済みで変更不要。JP: PIPA 取得/利用/提供/開示系（`AttributeEquals` ロール/リクエストフラグ）、環境法の施設運営者系（`AttributeEquals`）、`EIA_Art5`→`AttributeEquals{project_class=="class_1"}`、`Construction_Art3`→`Threshold{contract_value_jpy≥5_000_000}` の計 ~8件を変換。民法90/415/541、憲法13/14/21/25/29、著作権法32条（フェアユース的判断）、特許法100条、大気汚染防止法25条（因果関係）の計 ~12件は真に裁量的なので `Custom` のまま維持し理由をコメント化。既存25件の `AttributeEquals` は変更不要。**禁止:** `Percentage`（評価器間でキー不一致 `percentage_{ctx}` vs `{ctx}_percentage`）と `Calculation`（数式エンジン未実装で常に `Err`）は使用しない — 閾値には `Threshold` を使う。
  - **Files:** `src/verifier/us_statutes.rs`, `src/verifier/eu_statutes.rs`, `src/verifier/jp_statutes.rs`
  - **Prerequisites:** なし
  - **Tests:** 変換した各 precondition が期待する variant/属性名/演算子/値を持つことを検証するユニットテスト。`all_*` 関数群が引き続きビルドされ、jurisdiction タグを保持すること。
  - **Risk:** 中。構造化により `legalis-verifier` のSMT矛盾検出が新たなクロス法域の数値矛盾を顕在化させる可能性がある。次項の矛盾回帰テストで件数を固定し、偽陽性が出た場合は `Custom` に戻すのではなく属性名を非重複にする。

- [x] verifier: formalize.rs の評価エンジンを trait `Condition::evaluate` に切り替え（`/formalize` の潜在的誤判定バグ修正） (planned 2026-07-02)
  - **Goal:** 現状 `/formalize` は `EntailmentEngine::entail`（内部で `Condition::evaluate_simple` を使用）を使っており、そのcatch-all `_ => Ok(true)` が `Custom`/`Duration`/`Threshold`/`SetMembership` を事実に関わらず「充足済み」として扱ってしまうため、純粋な `Custom` 前提条件を持つ seed statute は常に `Deterministic` を誤って返している（本来は `JudicialDiscretion` であるべき）。上記の構造化条件変換を実際に機能させ、この誤判定バグを修正する。
  - **Design:** `formalize.rs` の `EntailmentEngine::entail` 呼び出しと `entailment_to_legal_result`（該当行付近）を、trait版 `Condition::evaluate(&AttributeBasedContext)`（`use legalis_core::EvaluationContext;`）を使った precondition ごとのループに置き換える。集約方針: 全 precondition が `Ok(true)`（または無し）→ `Deterministic(effect.description)`；`Err` 無しで `Ok(false)` あり → `JudicialDiscretion`；`Err(Custom)` あり → `JudicialDiscretion`（正しい修正後の挙動）；`Err(Missing...)` あり → `JudicialDiscretion`（`Void` はSMT矛盾検出専用のまま変更しない）。`UserFacts::to_context_for` は既に `attributes` をそのまま透過させ `years_employed` から `duration_months` を導出しているため、`weekly_hours`/`employee_count`/`hours_worked_12mo`/`lawful_basis`/`request_type`/`breach_occurred` は既存の仕組みでコンテキストに届く。
  - **Files:** `src/verifier/formalize.rs`, `src/verifier/contradiction.rs`（矛盾件数の回帰テストのみ追加）
  - **Prerequisites:** 直前の「seed statute の `Condition::Custom` を構造化 SMT 条件へ変換」項目が完了していること（このテストは変換後の統計条件を前提とする）。
  - **Tests:** `test_flsa_overtime_deterministic_when_over_40`（`weekly_hours:"45"`→deterministic）、`test_flsa_overtime_discretion_when_under_40`（`"30"`→judicial_discretion）、`test_flsa_overtime_discretion_when_hours_missing`（欠落→judicial_discretion、void ではない）、`test_ada_employee_count_threshold`（`"20"`→det, `"8"`→disc）、`test_fmla_dual_threshold`、`test_gdpr_breach_notification_deterministic`、`test_minpo_public_policy_stays_discretion`（`Custom` 維持statute→judicial_discretion、誤判定バグ修正の証明）、`all_us_federal_statutes`/`all_gdpr_statutes`/JP各 `all_*` に対する矛盾検出件数の回帰テスト、既存 `test_lca_art18_indefinite_conversion` が引き続き deterministic であること。
  - **Risk:** 中。公開エンドポイントの挙動変更（より正確になる方向）。既存テストで `/formalize` が `Void` を返すことを assert しているものは無いため `Err→JudicialDiscretion` は安全。

### Proposed follow-ups

- [x] CLI (`oxigenai simulate`) の `run_simulate` も `/simulate` HTTPハンドラと同じバグを持つ (planned 2026-07-02)
  - **Goal:** `oxigenai simulate` に `--profile` フラグ（デフォルト `"jp_2024"`）を追加し、既存の `profile_by_name()` で解決、未知の値は明確なエラーで拒否する — 既に完了済みの HTTP `/simulate` 修正と同じ内容を CLI 側にも適用する。`-j/--jurisdiction`（条文選択、既存）と `--profile`（人口統計、新規）は意図的に独立した軸のまま維持する（HTTPハンドラも同様に両者を独立させており、jurisdiction から profile を推測する仕組みは存在しない）。
  - **Design:** `main.rs` の `Commands::Simulate`（114-132行付近）に `#[arg(long, default_value = "jp_2024")] profile: String,` を追加し、doc comment（現状「Japanese demographic population (jp_2024_profile...)」とハードコードされている）を更新。`src/handlers/simulate.rs` の `resolve_profile` を参考に、`main.rs` 内に同等のリゾルバを追加（`profile_by_name` を呼び、`None` の場合は明確なエラー — ただし axum の `(StatusCode, String)` ではなく、`main.rs` の他のサブコマンドが使っているエラー型・パターンに合わせる）。`run_simulate`（604-670行付近）内の `jp_2024_profile()` ハードコード（639行付近）を解決済みプロファイルに置き換える。39行付近の `use oxigenai::verifier::simulator::{...}` importに `profile_by_name` を追加（`jp_2024_profile` が他で使われていなければ整理）。
  - **Files:** `src/main.rs`
  - **Prerequisites:** なし
  - **Tests:** `main.rs` に新規 `#[cfg(test)] mod tests` を追加し、リゾルバの既知プロファイル解決・未知プロファイルのエラーを検証。CLIエンドツーエンドのテストハーネスはこのリポジトリに存在しない（`tests/`・`assert_cmd` 無し）ため新規に作らない。
  - **Risk:** 低。加算的な変更で、既に出荷・テスト済みの `profile_by_name` 自体には手を加えない。

- [x] `contradiction.rs` の「OxiZ SMT」矛盾検出は実際には浅いヒューリスティックであり、真のSMT充足可能性解検証ではない (planned 2026-07-02)
  - **Goal:** `contradiction.rs` のヒューリスティックな precondition 重複判定を、真の OxiZ SMT 充足可能性検証に置き換え、このコードベース全体で使われている「OxiZ SMT」というブランディングを名実共に一致させる。既存の `SmtContradiction { statute_ids, explanation, severity }` 契約、および下流のフィルタリング（`MIN_CONTRADICTION_SEVERITY`, `is_same_law_conflict`）は維持する。
  - **Design（事前調査済み・再調査不要）:** `legalis-verifier 0.1.6` の本物の SMT モジュール（`SmtVerifier`, `src/smt.rs`, 1588行、テスト34件+proptest、stub無し）は Cargo feature **`smt-solver`**（正確な名称）で有効化され、`oxiz-solver`/`oxiz-core`/`num-bigint`（全て Pure Rust、C/C++無し、ポリシー準拠）を引き込む。`SmtVerifier` は `legalis_core::Condition`（oxigenai の statute が既に使っている型）を直接扱う。重要な注意点: feature を有効化しても `legalis_verifier::detect_statute_conflicts`（oxigenai が現在呼んでいる関数）自体はアップグレードされない — その `conditions_overlap`（discriminant一致判定）と `effects_contradict`（`EffectType`ペアの静的ルックアップ）にSMT版は存在しない。SMTの実体は `SmtVerifier` のプリミティブAPIと `StatuteVerifier::verify()` の内部ゲート済みヘルパーのみにあり、後者は非構造化の `errors: Vec<String>` を返す（`{statute_ids, severity}` と互換性が無い）。**したがって oxigenai 自身で新しいSMTベースのペアワイズループを実装する必要があり、ライブラリ側にドロップイン代替は無い。**
    1. `Cargo.toml`: `legalis-verifier = "0.1.6"` → `legalis-verifier = { version = "0.1.6", features = ["smt-solver"] }`。OxiZ は Pure Rust なので oxigenai 側の独自 feature gate は不要。
    2. `contradiction.rs`: 新しいペアワイズ関数を追加 — `detect_effect_conflicts` の既存ゲート（同一 jurisdiction・temporal-validity-overlap）を踏襲し、各 statute の `Vec<Condition>` precondition群を `Condition::And` で1つに畳み込み、`legalis_verifier::SmtVerifier::is_satisfiable(&Condition::And(chain_a, chain_b))` で真の同時充足可能性を判定（旧来の discriminant 一致判定を置き換え）。真に重複する場合のみ、現行と同じ `EffectType` ペア矛盾ルール（既存の `effects_contradict` の match armsをそのまま踏襲）で矛盾・severityを判定し、`statute_ids`/`explanation` を現行通り生成。`SmtVerifier` の `Result` はソルバーエラー時にそのペアをスキップする形で保守的に処理（`.unwrap()` 禁止ポリシー）。`is_same_law_conflict`/`MIN_CONTRADICTION_SEVERITY` フィルタリングはそのまま維持。
    3. `test_contradiction_baseline_counts_post_phase4_structuring` を本物のSMTで再ベースライン化 — そのテスト自身のdocコメントが既に義務付けている通り「値を更新する前に差分を必ず調査する（ただ数値を書き換えない）」を実行し、変化した件数それぞれについて理由をコメントで明記する。
    4. 少なくとも1件、手作りの回帰テストを追加 — 同一属性上で数値範囲が排他的な2つの statute（例: 一方が `age < 18`、他方が `age >= 65`）を用意し、旧ヒューリスティック（discriminant一致）なら「重複」と誤判定するが、真のSMTでは同時充足不可能と正しく判定され、たとえ `EffectType` が矛盾する組み合わせでも矛盾として報告されないことを assert する — これが今回の修正が見せかけでない証明になる。
    5. `run_full_verification`（既に `StatuteVerifier::verify()` を呼んでいる）が、feature有効化によって内部ゲート済みヘルパーも動き出した後も既存テスト（例: `test_run_full_verification_with_labor_statutes`）を壊さないことを確認する（新たな real error/warning が出るのは改善であり退行ではない）。
    6. `benches/contradiction_smt.rs` が引き続きコンパイル・実行できることを確認する（実行時間は変化して当然）。
  - **Files:** `Cargo.toml`, `src/verifier/contradiction.rs`
  - **Prerequisites:** なし（調査済み。型は互換、feature は成熟しPure Rust）
  - **Tests:** 再ベースライン化した `test_contradiction_baseline_counts_post_phase4_structuring`（調査コメント付き）、真のSMT意味論を証明する新規回帰テスト、既存の `run_full_verification`/ベンチが引き続き健全であること。
  - **Risk:** 中。矛盾検出件数は変化する（それが目的） — 更新前の調査を必須とすることで緩和。Pure Rustな新規推移的依存が約7個増える（コンパイル時間増は許容）。statute ペアごとに O(n²) でソルバーをインスタンス化する性能面は既存ベンチで計測し、ベンチが問題を示さない限り早すぎる最適化（`SmtVerifier` の使い回し等）はしない。

- [x] `legalis_verifier::detect_statute_conflicts` の `JurisdictionalOverlap`（1法域あたり20件超の statute）検出の復元 (planned 2026-07-02)
  - **Goal:** `legalis_verifier::detect_jurisdictional_overlaps` を忠実に移植する: `.jurisdiction` でグルーピング（`None` は完全にスキップ — upstream の `if let Some(jurisdiction)` ガードと一致）、20件を**超える**（`> 20`、`>=` ではない）法域ごとに1件の警告を生成し、法域名・件数・upstream と同じ2つの改善提案文字列を含める。
  - **Design（事前調査済み・逐語的に確認済み）:** upstream 原文: グルーピングは `HashMap<jurisdiction String, Vec<statute id>>`（`None` はスキップ）、閾値は `statute_ids.len() > 20`、description は `format!("Jurisdiction '{}' has {} statutes, which may indicate overlap or redundancy", jurisdiction, count)`、提案は `"Review statutes for consolidation opportunities"` と `"Consider creating sub-jurisdictions for better organization"`。`StatuteConflict::new` は `JurisdictionalOverlap` を常に `Severity::Warning` に割り当てるため、`contradictions`/`SmtContradiction` パイプライン（`MIN_CONTRADICTION_SEVERITY=2` で常に弾かれる）には流さず、`pub fn detect_jurisdictional_overlap_warnings(statutes: &Statute) -> Vec<String>` として `run_full_verification` の `warnings` へ直接投入する（`check_equality` issue と同じ重複排除パターン: `if !warnings.contains(&w) { warnings.push(w) }`）。2つの提案文字列は破棄せず、警告文字列に埋め込む。
  - **Files:** `src/verifier/contradiction.rs`
  - **Prerequisites:** なし（調査済み、上記コード片は確認済み）
  - **Tests:** ちょうど20件（同一法域）→警告なし（境界値）。21件→法域名と"21"を含む警告が1件。`jurisdiction: None` の statute（件数問わず）→バケツ化されず発火しない。
  - **Risk:** 低。現在の全フィクスチャに対して不活性（このコードベースにはまだ20件超の法域が存在しない）。

- [x] `legalis_verifier::detect_statute_conflicts` の `TemporalConflict`（statute バージョン相違）検出の復元 (planned 2026-07-02)
  - **Goal:** `legalis_verifier::detect_temporal_conflicts` とその `title_similarity` ヘルパーを忠実に移植する — 「関連する」（同一法域 or タイトル類似）かつ時間的に重複し、かつバージョンが異なる statute のペアを検出するペアワイズループ。
  - **Design（事前調査済み・逐語的に確認済み、2つの罠に注意）:** upstream 原文: `related = statute1.jurisdiction == statute2.jurisdiction || title_similarity(&statute1.title, &statute2.title) > 0.5`（`related` かつ `temporal_validity_overlaps(...)` かつ `statute1.version != statute2.version` で発火）。description は `format!("Statutes '{}' (v{}) and '{}' (v{}) have overlapping validity periods", ...)`、提案は `"Set expiry date on older version when newer version takes effect"` と `"Use version control and temporal validity to manage transitions"`。**罠1:** `related` の法域比較は `Option<String>` の直接等価判定であり（`None == None` は true だが `Some("US") == None` は false）、`contradiction.rs` 既存の `same_jurisdiction`（`None` をどちらか一方でも「何にでもマッチ」として扱う）とは意味論が異なる — **`same_jurisdiction` を再利用してはならない**。専用の直接比較（またはそれ専用の別名ヘルパー、例: `jurisdiction_equal`）を書き、`same_jurisdiction` と意図的に異なる理由をコメントで明記すること。**罠2:** `title_similarity`（`split_whitespace()` でトークン化・`HashSet`で重複排除したJaccard類似度、**大文字小文字を区別**・句読点分割なし、両方空なら1.0）は「改善」してはならない — 逐語的に移植する（小文字化やパンクチュエーション除去を勝手に追加しない）。`temporal_validity_overlaps` は Phase 5 で既に `contradiction.rs` に実装済みでそのまま再利用可。`JurisdictionalOverlap` 同様、常に `Severity::Warning` のため `contradictions` パイプラインではなく `pub fn detect_temporal_conflict_warnings(statutes: &Statute) -> Vec<String>` として `run_full_verification` の `warnings` へ直接投入する。
  - **Files:** `src/verifier/contradiction.rs`
  - **Prerequisites:** なし（調査済み、上記コード片は確認済み）
  - **Tests:** `title_similarity` の単体テスト（同一タイトル→1.0、無関係→0.0、両方空→1.0、大文字小文字区別の確認: `"Traffic Law"` vs `"traffic law"` → 0.0、部分一致: `"Traffic Law Amendment"` vs `"Traffic Law"` → 2/3）。`detect_temporal_conflict_warnings`: 同一法域・バージョン相違・デフォルト（無期限）の temporal validity → 両IDとバージョンを含む警告1件。バージョン同一なら警告なし。法域は異なるがタイトル類似度>0.5・バージョン相違 → それでも発火（OR分岐の証明）。`None`/`Some("US")` の法域ペア（タイトルも非類似）→ 関連なしと判定される（罠1の回帰テスト — `same_jurisdiction` を誤って再利用した場合にのみ誤って通ってしまうケース）。
  - **Risk:** 低。現在の全フィクスチャに対して不活性（このコードベースの全 statute は `Statute::new` 経由で `.with_version(...)` を呼んでおらず、全て version 1 で統一されているため `version1 != version2` は現状常に false）。

---

## 参考実装 / Reference Implementation

- 源内（Python）: `/mnt/g/tmp/genai-ai-api/google-cloud/lawsy-custom-bq/`
- Legalis-RS: https://github.com/cool-japan/legalis
- OxiZ SMT: https://crates.io/crates/legalis-verifier
