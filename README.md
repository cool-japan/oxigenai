# OxigenAI（オキシ源内、おきしげんない）

> 44億円の補正予算で進む源内（デジタル庁ガバメントAI）の
> Google Cloud版法制度AI実装を、Pure Rust + 形式検証で再構築。
> **Legalis-RS + OxiZ powered 源内 Rust Edition**

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust 2024](https://img.shields.io/badge/Rust-2024_edition-orange.svg)](https://www.rust-lang.org/)
[![crates.io](https://img.shields.io/crates/v/oxigenai.svg)](https://crates.io/crates/oxigenai)
[![Legalis-RS](https://img.shields.io/badge/Legalis--RS-0.1.6-green.svg)](https://crates.io/crates/legalis-core)

---

## 概要 / Overview

**OxigenAI** は、デジタル庁のガバメントAI施策として **44億円の補正予算**（2025年度）を投じて開発・運用されているGoogle Cloud版 法制度AI「**源内**（源内Web + 源内AIアプリ、2026年4月24日 MITライセンス公開）」を **Pure Rust + 形式検証** で完全再構築するプロジェクトです。

Python / Vertex AI / BigQuery ML ベースのクラウドネイティブ実装を起点に、**Legalis-RS** の Computational Law 基盤（legalis-verifier + OxiZ SMT ソルバー + `LegalResult<T>`）を統合することで、「確率的な LLM + RAG アプリ」から「法令を論理的に検証・実行・シミュレーションできる本物の Computational Law 基盤」へと昇華させます。

**OxigenAI** rebuilds the Digital Agency's government AI legal system "Genai" — developed with ¥4.4B supplementary budget on Google Cloud (Vertex AI Gemini + BigQuery ML) and open-sourced April 24, 2026 — in Pure Rust with formal verification. The Legalis-RS framework adds OxiZ SMT-based contradiction detection and `LegalResult<T>` for deterministic/discretionary outcome separation, transforming a probabilistic LLM system into a verifiable Computational Law platform.

---

## アーキテクチャ / Architecture

```
                         ┌─────────────────────────────────────┐
                         │          OxigenAI (Axum + Tokio)    │
                         │                                     │
  HTTP POST /   ────────►│  handlers/report.rs                 │
  HTTP POST /compile ───►│  handlers/compile.rs                │
  HTTP POST /simulate ──►│  handlers/simulate.rs               │
  HTTP POST /formalize ──►│  handlers/formalize.rs             │
                         │         │                           │
                         │         ▼                           │
                         │  services/pipeline.rs               │
                         │    ┌────┴────────────────────┐      │
                         │    │  1. 法令名推定           │      │
                         │    │     (Gemini + Web検索)   │      │
                         │    │  2. BigQuery VECTOR_SEARCH│     │
                         │    │  3. 条文選択 (AI)        │      │
                         │    │  4. ★ Legalis-RS検証     │      │
                         │    │     (OxiZ SMT矛盾検出)   │      │
                         │    │  5. レポート生成 (Gemini) │     │
                         │    │  6. ★ LegalResult<T>分類 │      │
                         │    │  7. 出典付き最終レポート  │     │
                         │    └──────────────────────────┘     │
                         │         │                           │
                         │         ▼                           │
                         │  legalis-verifier (OxiZ SMT)        │
                         │  legalis-core (LegalResult<T>)      │
                         │  legalis-jp (日本法域)               │
                         │  legalis-dsl (法令DSLパーサ)         │
                         │  legalis-sim (ECSシミュレータ)       │
                         └─────────────────────────────────────┘
                                   │              │
                         ┌─────────┴──┐  ┌────────┴──────────┐
                         │  Vertex AI  │  │ BigQuery ML        │
                         │  Gemini     │  │ VECTOR_SEARCH      │
                         │  2.5 Flash  │  │ (e-Gov法令XML)     │
                         └────────────┘  └────────────────────┘
```

### レイヤー比較 / Layer Comparison

| 項目 | 源内 | OxigenAI OSS（Apache-2.0） | OxigenAI Commercial（OpenCORE） |
|------|------|--------------------------|--------------------------------|
| 言語 | TypeScript + Python | Pure Rust | Pure Rust |
| クラウド依存 | AWS / Azure / GCP 前提 | GCP 前提 | **非依存**（オフライン動作） |
| 動作形態 | Web アプリ | CLI + Web API（統一バイナリ） | CLI + Web API + エアギャップ |
| 推論エンジン | Vertex AI Gemini | Vertex AI Gemini | **オフライン LLM**（Candle / mistral.rs + 量子化済みモデル同梱） |
| 法令解釈の保証 | LLM 確率的応答 | SMT 形式検証 | SMT 形式検証 **+ 証明書生成** |
| 多法域対応 | 日本のみ | 多法域（既存） | 多法域（既存） |
| エコシステム | デジタル庁単独 | 660 クレートに連結 | 660 クレートに連結 |
| 認証・アクセス制御 | なし | なし | **SAML / OIDC / Active Directory** |
| マルチテナント | なし | なし | **組織・部署別アクセス制御 + 監査ログ** |
| 法令データ更新 | クラウド経由 | クラウド経由 | **エアギャップ差分配信 + 署名検証** |
| ハードウェア保証 | — | — | **富士通 PRIMERGY・NEC Express5800・国産 GPU 基板** |
| サポート・SLA | なし | なし | **LTS + 脆弱性対応 SLA** |
| 開発予算 | ~44 億円（補正予算） | 個人開発の延長 | 商用契約ベース |
| ライセンス | MIT | Apache-2.0 | プロプライエタリ |

---

## 機能 / Features

### Phase 1（MVP）
- ✅ Axum REST API（源内と完全互換エンドポイント）
- ✅ 6種類の法令レポート生成
  - 定義確認型・手続き確認型・比較検討型・解釈適用型・政策研究型・包括分析型
- ✅ **OxiZ SMT による論理矛盾自動検出**（補完条文パターンのフィルタリング込み）
- ✅ **`LegalResult<T>` による決定論的/裁量部分の明確分離**
- ✅ BigQuery ベクトル検索（e-Gov 法令 XML）
- ✅ Web グラウンディング（Google 検索連携）
- ✅ 使用量・コスト追跡

### Phase 2（完成）
- ✅ **法令 XML → Legalis DSL 自動コンパイル**（`/compile` エンドポイント + `oxigenai compile` CLI）
- ✅ **政策シミュレーション機能**（`/simulate` エンドポイント + `oxigenai simulate` CLI）
  - 日本2024年国勢調査ベースの人口モデル（10万エージェント対応）
  - 条文別 決定論的適用率・裁量率・矛盾率の統計出力
- ✅ **法的事実形式化**（`/formalize` エンドポイント + `oxigenai formalize` CLI）
- ✅ **統一CLIバイナリ**（`oxigenai` 単体でサーバー・CLI・全機能を提供）

### Phase 3（完成）
- ✅ **法域カバレッジ拡大**（`JpDomainMatcher` 全法域対応: 不法行為・商法/会社法・憲法・知的財産・環境法・行政手続法・建設業法/宅建業法 等）
- ✅ **多法域対応**（Multi-Jurisdiction）
  - JP/EU/US の3法域レジストリ（`MultiJurisdictionMatcher`）
  - 全CLIサブコマンドの `-j/--jurisdiction` フラグ（デフォルト `JP`）
  - `GET /jurisdictions` ディスカバリエンドポイント
- ✅ **Generative Jurisprudence**（判例案自動生成）
  - 実在ランドマーク判例8件のキュレーション + Gemini web-grounded検索
  - `POST /predict-ruling` エンドポイント + `oxigenai predict` CLI
- ✅ **WebUI 統合**（axum配信の自己完結型SPA、`GET /` `GET /ui`、外部CDN一切不使用でエアギャップ対応）
- ✅ **翻訳整合性検証**（`POST /translate-check` + `oxigenai translate-check` CLI、原文・訳文2つのe-Gov XMLを構造比較、Sørensen–Dice係数）
- ✅ **ローカル検証パイプライン**（fmt + clippy + nextest + build 相当のワークフロー定義済み、コスト管理のためデフォルト無効化 / criterion パフォーマンスベンチマーク）

### Phase 4（深度強化）
- ✅ **`/formalize` 誤判定バグ修正** — 評価エンジンを trait `Condition::evaluate` に切り替え、`Custom`/未充足条件を誤って `Deterministic` と判定していたバグを修正
- ✅ **矛盾検出の真SMT化** — `contradiction.rs` を真の OxiZ SMT 充足可能性検証（`SmtVerifier::is_satisfiable`）に置き換え、旧来の discriminant ベースのヒューリスティックから移行
- ✅ **seed statute の構造化** — US/EU/JP の precondition を自由記述の `Condition::Custom` から構造化 SMT 条件（`Threshold`/`Duration`/`AttributeEquals`/`SetMembership`）へ変換
- ✅ **`/simulate` プロファイル選択バグ修正** — HTTP・CLI 両方に `jp_2024`/`us_2024`/`eu_2024` のプロファイル選択機能を追加。従来は `profile` パラメータが無視され常に日本プロファイルが使われていたバグを修正

### Beyond（中長期・将来構想）
- 🚀 GPU 対応シミュレーション（upstream legalis-sim は CUDA 対応済み — oxigenai 側の feature passthrough・配線は未実装）
- 🚀 マルチクラウド対応
- 🚀 JP/EU/US 以外への追加法域対応

---

## 技術スタック / Tech Stack

| カテゴリ | 技術 | バージョン |
|---------|------|---------|
| 言語 | Rust | 2024 edition |
| Web | Axum + Tower + Tokio | 0.8 / 0.5 / 1.x |
| 法令基盤 | Legalis-RS | 0.1.6 |
| SMT ソルバー | OxiZ (legalis-verifier 内蔵) | 0.1.6 |
| シミュレータ | legalis-sim (ECS エンジン) | 0.1.6 |
| GCP SDK | gcp-bigquery-client | 0.28 |
| GCP Auth | google-cloud-auth | 1.13 |
| AI モデル | Vertex AI Gemini 2.5 Flash | - |
| テスト | cargo-nextest | - |

---

## クイックスタート / Quick Start

crates.io からバイナリをインストールして、10分で動かす最短手順です。

```bash
# 1. バイナリインストール
cargo install oxigenai

# 2. 作業ディレクトリ作成 (/tmp/oxigenai/ なども可)
mkdir ~/oxigenai-workspace
cd ~/oxigenai-workspace

# 3. 環境変数テンプレートを取得
curl -fsSL https://raw.githubusercontent.com/cool-japan/oxigenai/master/scripts/.env.example -o .env
# wget を使う場合: wget https://raw.githubusercontent.com/cool-japan/oxigenai/master/scripts/.env.example -O .env

# 4. .env を編集（GCP プロジェクト ID 等を設定）
$EDITOR .env

# 5. GCP 認証
gcloud auth application-default login

# 6. 起動
PORT=8080 oxigenai serve
```

サーバーが `http://localhost:8080` で起動します。動作確認：

```bash
curl -s http://localhost:8080/health

curl -s -X POST http://localhost:8080/ \
  -H 'Content-Type: application/json' \
  -d '{"inputs":{"input_text":"個人情報保護法の「個人情報」の定義を教えてください"}}'
```

---

## CLI リファレンス / CLI Reference

`oxigenai` は統一バイナリです。サブコマンドを省略した場合は `query` として動作します。

```bash
# 法令クエリ（デフォルト — サブコマンド省略可）
oxigenai "個人情報保護法の「個人情報」の定義を教えてください"
oxigenai query "下請法の支払期日規制を教えてください"
oxigenai q "労働基準法の時間外労働の上限規制は？"   # 短縮エイリアス

# オプション
oxigenai "クエリ" --json        # HTTP API と同一スキーマで JSON 出力
oxigenai "クエリ" --no-usage    # 使用量・コスト表示を抑制
oxigenai "クエリ" -j EU         # 法域指定（JP/EU/US、デフォルト JP。query/compile/simulate/formalize/predict 共通）

# Web サーバー起動
oxigenai serve                  # デフォルト port 8080
oxigenai serve --port 8081      # ポート指定（-p 8081 も可）
oxigenai serve -p 8081          # 短縮フラグ
PORT=8081 oxigenai serve        # 環境変数でも可（--port が優先）
oxigenai server -p 8081         # server エイリアス

# 法令 XML → Legalis DSL コンパイル
# （compile のみ --xml / --query フラグ — 2つのモードが排他的なため）
oxigenai compile --query "労働基準法"            # BQ 検索経由
oxigenai compile --xml law.xml                   # ローカル XML ファイル
oxigenai compile --query "労働基準法" --dsl-only  # DSL テキストのみ出力
oxigenai compile --query "労働基準法" --json      # JSON 出力

# 政策シミュレーション（クエリは位置引数）
oxigenai simulate "労働基準法の適用シミュレーション"
oxigenai simulate "労働基準法" --population 500   # 人口規模指定（最大 10,000）
oxigenai simulate "労働基準法" --profile us_2024   # デモグラフィックプロファイル指定（jp_2024/us_2024/eu_2024、デフォルト jp_2024）
oxigenai sim "労働基準法"                          # 短縮エイリアス

# 法的事実の形式化・適用判定（クエリは位置引数）
oxigenai formalize "有期雇用5年超の無期転換権" \
  --age 35 \
  --attr employment_type=fixed_term \
  --attr years_employed=6 \
  --description "5年以上の有期雇用契約を更新してきた"
oxigenai eval "無期転換"     # 短縮エイリアス

# 判例予測（Generative Jurisprudence、クエリは位置引数）
oxigenai predict "5年勤続の有期社員を経営不振で解雇できるか" --facts "解雇回避努力なし"
oxigenai pred "無期転換拒否は有効か" --json        # 短縮エイリアス / JSON 出力

# 翻訳整合性検証（オフライン — GCP 接続不要）
oxigenai translate-check --source ja.xml --target en.xml
oxigenai txcheck --source ja.xml --target en.xml --json   # 短縮エイリアス / JSON 出力
```

> **注意**: `query` / `compile` / `simulate` / `formalize` / `predict` は Google Cloud（Vertex AI + BigQuery）への接続が必要です。
> 事前に `gcloud auth application-default login` と `.env` の設定を行ってください。`translate-check` は完全オフラインで動作します。

実際の出力例は **[docs/result/](docs/result/README.md)** を参照してください。

---

## セットアップ / Setup（ソースビルド）

### 前提条件 / Prerequisites

- Rust 1.86+ (2024 edition)
- Google Cloud プロジェクト（Vertex AI + BigQuery 有効化）
- Application Default Credentials (`gcloud auth application-default login`)
- cargo-nextest (`cargo install cargo-nextest`)

### 環境変数 / Environment Variables

```env
# GCP設定（必須）
GOOGLE_CLOUD_PROJECT=your-project-id
INFERENCE_PROJECT_ID=your-inference-project-id  # 省略時はGOOGLE_CLOUD_PROJECTを使用
INFERENCE_LOCATION=asia-northeast1               # デフォルト

# Gemini設定
MODEL_ID=gemini-2.5-flash                        # デフォルト
GENERATION_TEMPERATURE=0.5                       # デフォルト

# BigQuery設定
BQ_DATASET_ID=e_laws_search                      # デフォルト

# 任意
GCS_BUCKET_NAME=your-bucket
LOG_LEVEL=INFO
```

### ビルド / Build

```bash
git clone https://github.com/cool-japan/oxigenai
cd oxigenai

# 環境変数の設定
cp .env.example .env
# .envを編集してGCPプロジェクト等を設定

# ビルド
cargo build

# テスト（no-warnings policy、2026-07-02時点 231テスト全通過）
cargo nextest run

# インストール
cargo install --path .
```

---

## API 仕様 / API Specification

### エンドポイント / Endpoints

#### `POST /`

源内互換の法令レポート生成エンドポイント。

**リクエスト**

```json
{
  "inputs": {
    "input_text": "個人情報保護法の「個人情報」の定義について教えてください"
  }
}
```

`inputs.jurisdiction`（後方互換のためトップレベル `jurisdiction` も可、デフォルト `JP`）で `JP`/`EU`/`US` を指定すると、当該法域の条文を対象に検証・矛盾検出が行われます。HTTPエンドポイントで `jurisdiction` を受け付けるのは `POST /` のみで、`/compile` `/simulate` `/formalize` `/predict-ruling` は現時点では JP 固定です（CLI側は `-j/--jurisdiction` で5サブコマンド共通、上記CLIリファレンス参照）。

**レスポンス**

```json
{
  "outputs": "# 個人情報保護法における「個人情報」の定義\n\n...\n\n## 法的整合性検証\n✅ 矛盾なし（OxiZ SMT 検証済み）\n\n## 出典\n[1] 【e-laws公式条文】...",
  "usageMetadata": [
    {
      "modelVersion": "gemini-2.5-flash",
      "requestCount": 3,
      "tokens": { "promptTokenCount": 12500, "candidatesTokenCount": 3200 },
      "estimatedCostInfo": { "estimatedCost": 0.012, "currency": "USD" }
    }
  ]
}
```

**OxigenAI 追加フィールド（源内との差分）**

レポート末尾に `## 法的整合性検証` セクションが自動追加されます：
- OxiZ SMT による形式検証結果（補完条文パターンはフィルタリング済み）
- `LegalResult<T>` による決定論的/裁量部分の分類と品質グレード（A〜F）

#### `POST /compile`

法令 XML または検索クエリから Legalis DSL を生成します。

**リクエスト（Mode A: XML）**

```json
{ "xml": "<Law>...</Law>" }
```

**リクエスト（Mode B: クエリ）**

```json
{ "query": "労働基準法" }
```

**レスポンス**

```json
{
  "law_title": "労働基準法",
  "law_num": "昭和二十二年法律第四十九号",
  "dsl_text": "STATUTE LSA_Art32: ...\nSTATUTE LSA_Art34: ...",
  "statutes": [
    { "id": "LSA_Art32", "title": "労働時間", "dsl": "STATUTE ...", "source": "xml_native" }
  ],
  "article_count": 8,
  "statute_count": 8,
  "warnings": []
}
```

#### `POST /simulate`

日本・米国・EU の人口統計プロファイルを選択して政策シミュレーションを実行します。

**リクエスト**

```json
{
  "query": "労働基準法の適用シミュレーション",
  "population_size": 1000,
  "profile": "jp_2024"
}
```

`profile`（省略時デフォルト `jp_2024`）で `us_2024` / `eu_2024` の人口統計プロファイルも選択できます。未知の値は 400 Bad Request で拒否されます。

**レスポンス**

```json
{
  "query": "労働基準法の適用シミュレーション",
  "population_size": 1000,
  "statute_count": 11,
  "total_applications": 11000,
  "deterministic_count": 8234,
  "discretion_count": 2156,
  "void_count": 610,
  "deterministic_ratio": 0.749,
  "discretion_ratio": 0.196,
  "statute_details": [...],
  "markdown_summary": "## 政策シミュレーション結果\n..."
}
```

#### `POST /formalize`

ユーザーの事実情報を構造化し、法令の適用可否を判定します。

**リクエスト**

```json
{
  "query": "有期雇用5年超の無期転換権",
  "facts": {
    "age": 35,
    "attributes": { "employment_type": "fixed_term", "years_employed": "6" },
    "description": "5年以上の有期雇用契約を更新してきた"
  }
}
```

#### `GET /health`

ヘルスチェックエンドポイント。HTTP 200 を返します。

#### `GET /jurisdictions`

利用可能な法域の一覧と、リクエストで `jurisdiction` を省略した場合のデフォルト値を返すディスカバリエンドポイント（リクエストボディなし）。

**レスポンス**

```json
{
  "jurisdictions": [
    { "code": "EU", "display_name": "欧州連合法 / European Union Law (GDPR)", "is_populated": true },
    { "code": "JP", "display_name": "日本法 / Japanese Law", "is_populated": true },
    { "code": "US", "display_name": "米国連邦法 / United States Federal Law", "is_populated": true }
  ],
  "default": "JP"
}
```

#### `POST /predict-ruling`

判例コーパス検索 + Gemini web-grounded 検索により、判決予測（Generative Jurisprudence／生成的法解釈）を行います。

**リクエスト**

```json
{
  "input": "5年勤続の有期契約社員を経営不振で解雇できるか",
  "facts": "業績は悪化しているが希望退職の募集など解雇回避努力は行っていない"
}
```

`input` は `query` / `input_text` をエイリアスとして受け付けます。`facts`（事実関係の自由記述）は省略可能です。

**レスポンス**

```json
{
  "query": "5年勤続の有期契約社員を経営不振で解雇できるか\n\n【事実関係】\n業績は悪化しているが希望退職の募集など解雇回避努力は行っていない",
  "issue": "経営不振を理由とする有期契約労働者の雇止めの有効性",
  "inferred_legal_area": "労働法",
  "predicted_holding": "解雇回避努力が尽くされていないため、雇止めは無効と判断される可能性が高い",
  "reasoning": "（判例の射程に基づく推論、Markdown形式）",
  "cited_precedents": [
    { "id": "nihon-ensei-1975", "case_name": "日本食塩製造事件", "court": "最高裁判所", "legal_area": "労働法", "citation": "最判昭和五十年四月二十五日", "relevance_score": 0.83, "precedent_weight": 0, "holding_summary": "解雇権濫用法理の基礎を示した判例（現・労働契約法16条）", "cited_statutes": ["労働契約法16条"], "source_url": "https://www.courts.go.jp/app/hanrei_jp/search1", "is_binding": true }
  ],
  "confidence": 0.72,
  "confidence_label": "中程度",
  "legal_classification": "judicial_discretion",
  "context_id": "5f2c1e2a-9b3d-4e21-8f3a-1234567890ab",
  "markdown_summary": "## 判決予測\n\n...",
  "usage": [{ "modelVersion": "gemini-2.5-flash", "requestCount": 2, "tokens": { "promptTokenCount": 4200, "candidatesTokenCount": 1100 }, "estimatedCostInfo": { "estimatedCost": 0.004, "currency": "USD" } }]
}
```

#### `POST /translate-check`

原文・訳文の2つの e-Gov 法令 XML を Legalis DSL にコンパイルし、`legalis_core::Statute` 集合を構造比較して翻訳の論理的一貫性を検証します（完全オフライン、機械翻訳・ネットワーク通信なし）。

**リクエスト**

```json
{
  "source_xml": "<Law>...日本語原文...</Law>",
  "target_xml": "<Law>...English translation...</Law>",
  "source_lang": "ja",
  "target_lang": "en"
}
```

`source_lang`（デフォルト `"ja"`）/ `target_lang`（デフォルト `"en"`）は比較結果に影響しない付帯ラベルです。

**レスポンス**

```json
{
  "equivalent": false,
  "score": 0.86,
  "count_score": 1.0,
  "effect_score": 0.8,
  "condition_score": 0.78,
  "divergences": [
    { "kind": "condition_kind", "detail": "前提条件の構造が一致しません（原文: Threshold 1件 → 訳文: Custom 1件）", "source": "1", "target": "0" }
  ],
  "source_signature": { "statute_count": 1, "precondition_count": 3, "effect_types": { "Prohibition": 1 }, "condition_kinds": { "And": 1, "Threshold": 1, "AttributeEquals": 1 } },
  "target_signature": { "statute_count": 1, "precondition_count": 3, "effect_types": { "Prohibition": 1 }, "condition_kinds": { "And": 1, "Custom": 1, "AttributeEquals": 1 } },
  "source_lang": "ja",
  "target_lang": "en",
  "source_dsl": "STATUTE JP_Civil_Art95: ...",
  "target_dsl": "STATUTE JLT_Civil_Art95: ..."
}
```

---

## Legalis-RS 統合 / Legalis-RS Integration

OxigenAI は [Legalis-RS](https://github.com/cool-japan/legalis)（Computational Law エコシステム）の中核コンポーネントを統合します。Legalis-RS は単なるライブラリ群ではなく、**法令を実行可能な論理式として扱う**ことを設計原点としており、OxigenAI はその日本法域実装（`legalis-jp`）を通じて、源内が想定するすべての省庁ユースケースをカバーします。

### ドメイン対応表 / Ministry Domain Coverage

| 省庁 / ユースケース | Legalis-RS コンポーネント | OxigenAI エンドポイント |
|-------------------|--------------------------|------------------------|
| **法務省 — 法制度整備支援（ODA）** | `legalis-jp` + `legalis-dsl` + `legalis-verifier` | `/compile`, `/formalize`, `/` |
| **法務省 — 法令外国語訳整備事業** | `legalis-jp` + `legalis-dsl` + `legalis-verifier` | `/compile` (lang pair) |
| デジタル庁 — 法令 RAG | `GeminiService`（`gemini_client.rs`） + `legalis-jp` | `/`（源内互換） |
| 厚生労働省 — 労働法適用判定 | `legalis-jp` + `legalis-sim` | `/simulate`, `/formalize` |
| 国土交通省 — 建築基準法・都市計画 | `legalis-jp` + `legalis-core` | `/`, `/compile` |
| 財務省 — 税法解釈 | `legalis-jp` + `legalis-verifier` | `/`, `/formalize` |
| 総務省 — 地方自治法・条例 | `legalis-jp` + `legalis-dsl` | `/compile`, `/` |

### 法制度整備支援（対法務省）/ Legal System Development Assistance (Ministry of Justice ODA)

> **Legalis-RS は、法務省が担う「法制度整備支援」をソフトウェアとして完全にドメインに取り込んでいます。**

**法制度整備支援**（ほうせいどせいびしえん）とは、日本が開発途上国・市場経済移行国に対し、法律の起草・裁判制度の運用・法曹（裁判官・検察官・弁護士）の育成を支援する政府開発援助（ODA）活動です。ベトナム・ラオス・カンボジア・ウズベキスタンなどで、日本民法・商法の経験を活かした民法典制定や司法制度の構築を技術的にサポートしてきました。法務省が主体となり、JICA を通じて実施されています。

この分野において、OxigenAI + Legalis-RS は以下の課題を解決します：

**支援対象国の法令を日本法・国際標準と形式的に比較・検証できる。**

1. **条文の多法域形式化**（`/compile`）  
   ベトナム民法・ラオス民法などの条文テキストを Legalis DSL にコンパイルし、日本民法の対応条文と同一の形式表現に変換。「規定の趣旨は同一か」「要件・効果の論理構造が等価か」を SMT で機械的に比較します。

2. **日本法との矛盾・欠缺の検出**（OxiZ SMT バックエンド）  
   支援国の草案条文を日本の参照条文群と合わせて OxiZ SMT に投入し、論理的に相矛盾する規定・適用範囲の欠缺・定義の循環を自動検出します。従来は両国の法律専門家が数週間かけて目視で行っていた作業を自動化します。

3. **住民影響シミュレーション**（`/simulate`）  
   支援国の人口構造モデルに草案条文を適用し、「この民法典では何割の国民が決定論的に保護を受け、何割が裁量判断に委ねられるか」を統計出力します。制度設計段階での影響評価を可能にします。

4. **`LegalResult<T>` による裁量条文の明示**  
   条文の適用結果を `Deterministic`（機械的適用）・`JudicialDiscretion`（裁量必要）・`Void`（論理矛盾）に三分類し、「立法者が意図した裁量」と「論理的欠陥による不確定性」を峻別します。法曹育成の教材としても活用できます。

```rust
// 例：ベトナム民法の条文と日本民法対応条文を形式比較
use legalis_jp::statute_from_article;
use legalis_core::LegalResult;

// 支援国（ベトナム）の草案条文を DSL に変換
let vn_statute = statute_from_dsl(vietnam_civil_code_art117)?;
// 日本の参照条文（民法95条：錯誤）を DSL に変換
let jp_statute = statute_from_article(&jp_civil_code_art95)?;

// OxiZ SMT で論理等価性・矛盾を検証
let result: LegalResult<bool> = vn_statute.compare_with(&jp_statute);

match result {
    LegalResult::Deterministic(equivalent) => {
        println!("論理構造: {} 等価", if equivalent { "✅" } else { "⚠️ 相違あり" });
    }
    LegalResult::JudicialDiscretion { hint } => {
        // 裁量条文として設計されている部分
        println!("裁量条文（育成教材向け）: {hint}");
    }
    LegalResult::Void { reason } => {
        // 支援国草案の論理矛盾を検出
        println!("⚠️ 矛盾検出（要修正）: {reason}");
    }
}
```

### 法令外国語訳整備事業（対法務省）/ Legal Translation Development (Ministry of Justice)

**法令外国語訳整備事業**は、法務省が推進する日本法令の英訳・多言語化事業です。[日本法令外国語訳データベースシステム（JLT）](https://www.japaneselawtranslation.go.jp/)を通じて民法・商法・刑法・会社法など主要法令の英訳を公開しており、外資系企業の日本市場参入や国際仲裁、在日外国人の法的アクセス向上を目的としています。

この分野で OxigenAI + Legalis-RS が解決する課題は**「訳文の論理的一貫性を機械的に保証できない」**問題です。

**日本語条文と外国語訳の構造的対応検証：**

Legalis DSL は言語中立です。日本語の条文も英訳も、同一の DSL 形式（要件 → 効果 → 例外の述語論理式）に変換すれば、「翻訳前後で法的な意味構造が等価か」を OxiZ SMT で機械的に検証できます。

```
# 日本語原文（民法第95条：錯誤）→ DSL
STATUTE JP_Civil_Art95:
  condition: declaration AND material_mistake AND excusable
  effect: VOIDABLE(legal_act)
  exception: good_faith_third_party

# 英訳（JLT 掲載版）→ DSL
STATUTE JLT_Civil_Art95:
  condition: manifestation_of_intent AND mistake AND essential
  effect: VOIDABLE(juridical_act)
  exception: bona_fide_third_party
```

OxiZ SMT が両 DSL の論理等価性を検証。`manifestation_of_intent` ⟺ `declaration`、`juridical_act` ⟺ `legal_act` などの対応関係を型レベルで追跡し、**「訳し落とし」や「意味の転換」を自動検出**します。

**具体的なユースケース：**

1. **改正追従チェック**：法令が改正された際、対応する英訳が DSL レベルで未更新であれば自動的にフラグを立てます（JLT の慢性的な訳文遅延問題への対処）。
2. **多言語展開の整合性検証**：英語・中国語・韓国語・ベトナム語の各訳文を同一 DSL に変換し、言語間の意味ドリフトを一括検出します。
3. **`/compile` エンドポイントの言語対オプション**：`oxigenai compile --query "民法第95条" --lang en` で日英 DSL ペアを生成し、差分を出力します。

ドメイン対応表の「法務省 — 法制度整備支援」行と組み合わせると、OxigenAI は**途上国支援（ODA）と国内訳文整備を同一パイプラインで処理**できる唯一のシステムになります。

### コンポーネント詳細 / Component Details

#### `legalis-jp` — 日本法域モジュール

`legalis-jp` は Legalis-RS エコシステムにおける**日本法ドメイン完全実装**です：

- **e-Gov 法令 XML パーサ**：法令番号・条番号・項・号の完全構造化
- **日本法特有の概念マッピング**：「条」「項」「号」「附則」「別表」の DSL 対応
- **施行令・施行規則の親法令リンク**：委任関係を型レベルで表現
- **改廃履歴追跡**：改正前条文と現行条文の差分を形式化
- **法制審議会答申フォーマット対応**：答申テキストから DSL への半自動変換

#### `legalis-dsl` — 法令 DSL コンパイラ

法令テキストを形式言語にコンパイルします：

```
STATUTE LSA_Art32:
  title: "労働時間"
  condition: weekly_hours <= 40 AND daily_hours <= 8
  effect: LAWFUL(employment_contract)
  exception: STATUTE_LSA_Art33  // やむを得ない事由（裁量条文）
```

- **双方向変換**：DSL → 自然言語日本語の逆生成（説明文自動生成）
- **型チェック**：条文間の引用整合性を静的に検証
- **差分出力**：改正前後の DSL を diff 形式で表示（法制局向け）

#### `legalis-verifier` (OxiZ SMT)

条文間の論理矛盾を形式的に検出します。補完条文パターン（基本規定＋例外規定）は自動フィルタリングされます：

```rust
// OxiZ SMT で矛盾検出（Error/Critical のみ、補完条文パターンを除外）
let contradictions = detect_smt_contradictions(&statutes);

if !contradictions.is_empty() {
    report.push_str(&format_contradiction_warnings(&contradictions));
}
```

**検出可能な矛盾パターン：**
- 同一要件に対する相反する法効果（A 条「○○の場合は許可」vs B 条「○○の場合は禁止」）
- 充足不能な複合要件（論理的に誰も満たせない条件の組み合わせ）
- 循環参照（A 条が B 条を準用し B 条が A 条を準用）
- 空集合適用域（施行範囲が空集合になる条文）

#### `legalis-core::LegalResult<T>`

レポートの各結論を分類します：

- `Deterministic(T)` — 機械的に導出可能（例：年齢 ≥ 18 → 成人）
- `JudicialDiscretion` — 人間の解釈が必要（例：「正当な理由」「やむを得ない事由」）
- `Void` — 論理的矛盾を含む（OxiZ が不充足を証明）

#### `legalis-sim` (ECS シミュレータ)

日本2024年国勢調査ベースの人口モデルで政策影響をシミュレーションします：

```rust
// 日本人口モデル（年齢 Normal(48.4, 18.0)、雇用形態分布込み）
let config = SimulationConfig { population_size: 1000, profile: jp_2024_profile() };
let result = SimulatorService::run(statutes, &config).await?;
println!("{}", result.markdown_summary);
```

#### `GeminiService`（`gemini_client.rs`）— LLM 統合

OxigenAI 自身が実装する Gemini 統合レイヤーです。**`legalis-llm` クレートには依存していません**（`Cargo.toml` / `src/` のいずれにも参照なし）— Vertex AI Gemini REST API を直接呼び出します：

- Application Default Credentials（`google-cloud-auth`）でアクセストークンを取得し、`reqwest` 経由で REST API を直接呼び出し
- Web グラウンディング付き呼び出し（`call_with_grounding`）・厳格出力呼び出し（`call_strict`）・レポート生成呼び出し（`call_for_report`）・埋め込み生成（`embed_texts`）
- グラウンディング検索結果の抽出・リダイレクト解決（`extract_grounding_web_hits` / `resolve_redirect_web_hits`）

---

## OpenCORE（商用版）/ OpenCORE Commercial Edition

OxigenAI の OSS 版（Apache-2.0）は GCP 前提のクラウド動作を対象としています。**OpenCORE** は、自治体・官公庁・エンタープライズ向けに完全オフライン・エアギャップ運用を可能にする商用エディションです。

### オフライン推論エンジン統合

クラウド API 不要で動作する LLM 推論レイヤー。

- [Candle](https://github.com/huggingface/candle)（HuggingFace 製 Pure Rust 推論フレームワーク）統合
- [mistral.rs](https://github.com/EricLBuehler/mistral.rs) 統合
- 量子化済みモデル（GGUF / GGML）の同梱・署名付き配布
- **[OxiBonsai](https://github.com/cool-japan/oxibonsai)（検討中）** — COOLJAPAN Rust Ecosystem が開発した**世界初**の Pure Rust サブ2ビット LLM 推論エンジン（[記事](https://medium.com/@kitasanio/oxibonsai-the-worlds-first-pure-rust-1-bit-llm-inference-engine-4c15abf53fce)：Pure Rust zero-FFI かつ sub-2-bit の組み合わせとして先行実装なし）

**OxiBonsai について：**

[OxiBonsai](https://github.com/cool-japan/oxibonsai) は Pure Rust zero-FFI・zero-C/C++ で実装された PrismML Bonsai モデルファミリー専用推論エンジンです（v0.1.3、3,560 テスト通過、~139k 行 Pure Rust）。llama.cpp・BLAS・Fortran に一切依存せず、COOLJAPAN エコシステム（SciRS2・OxiBLAS・OxiFFT）のみで構成されています。

| モデル | サイズ | VRAM / RAM | 速度（Metal） |
|-------|--------|-----------|-------------|
| Ternary-Bonsai-1.7B | ~390 MB | ~390 MB | ~50 tok/s |
| Ternary-Bonsai-4B | ~900 MB | ~900 MB | — |
| Ternary-Bonsai-8B | ~1.75 GB | ~1.75 GB | — |
| Bonsai-8B（1-bit） | 1.15 GB | 1.15 GB | ~14.6 tok/s |

OxigenAI OpenCORE との統合検討理由：

1. **Pure Rust 一貫性**：OxigenAI のゼロ FFI 方針と完全に一致。サプライチェーン攻撃対象面を最小化します。
2. **エアギャップ適性**：Ternary-Bonsai-1.7B は ~390 MB ── 署名付き USB メディア 1 枚に収まります。
3. **`oxibonsai-rag` クレート**：OxiBonsai はベクトルストアと RAG パイプラインを内蔵しており、BigQuery 非依存のオフライン RAG として OxigenAI の `bq_retriever` を置き換え可能です。
4. **OpenAI 互換 API**：`/v1/chat/completions` + SSE ストリーミングを提供するため、`GeminiService` のオフラインドロップイン代替として実装コストが低い。

```rust
// OpenCORE 検討中: GeminiService のオフライン差し替えイメージ
// （OxiBonsai は OpenAI 互換 API を提供するため reqwest 層はそのまま）
use oxibonsai_runtime::{EngineBuilder, SamplingPreset};

let engine = EngineBuilder::new()
    .model_path("models/Ternary-Bonsai-1.7B.gguf")  // 390MB — USB 1本
    .preset(SamplingPreset::Greedy)  // 決定論的（温度0）→ 法令解釈に最適
    .max_seq_len(4096)
    .build()?;
```

### エアギャップ運用機能

インターネット非接続環境での完全運用。

- 法令データ（e-Gov 法令 XML）の差分配信パッケージ
- 配信パッケージへの署名検証（Ed25519）
- 承認済みメディアからのオフライン更新フロー

### エンタープライズ認証

| プロトコル | 用途 |
|-----------|------|
| SAML 2.0 | 省庁・自治体統合 IdP |
| OIDC | クラウドネイティブ SaaS 連携 |
| Active Directory / LDAP | 既存ドメイン統合 |

### マルチテナント・組織管理

- 自治体内の部署別ロールベースアクセス制御（RBAC）および関係ベースアクセス制御（ReBAC / Google Zanzibar モデル）
- 操作監査ログ（改ざん防止付き）
- 部署ごとの法令データセット分離

### 形式検証の高度機能

OSS 版の OxiZ SMT 矛盾検出に加え：

- 検証結果の **証明書生成**（PDF / XML、タイムスタンプ付き）
- 証明書の長期保存対応（LTV 署名）

### 専用ハードウェア動作保証

ガバメントクラウドおよび調達標準機材での動作検証・サポート：

- 富士通 PRIMERGY シリーズ
- NEC Express5800 シリーズ
- 国産 GPU 基板（MN-Core 2 等）

### 長期サポート（LTS）

- セキュリティ脆弱性対応 SLA
- 後方互換性維持の保証期間
- 商用サポート窓口

---

## Contributing

Pull Request を歓迎します。以下のルールを守ってください：

- Rust 2024 edition
- `cargo fmt` + `cargo clippy -- -D warnings`（ゼロ警告ポリシー）
- `cargo nextest run`（全テスト通過）

---

## ライセンス / License

### デュアルライセンス構造 / Dual License Structure

OxigenAI は **Apache-2.0 + 商用ライセンス** のデュアル構造を採用しています。現代のコマーシャル OSS（HashiCorp、Elastic 等）と同様のモデルです。

| | OSS 版 | 商用版（OpenCORE） |
|---|--------|-------------------|
| ライセンス | Apache License 2.0 | プロプライエタリ |
| 対象 | 開発者・小規模利用・コミュニティ | 自治体・官公庁・エンタープライズ |
| 動作環境 | GCP（クラウド前提） | ローカル・エアギャップ・オンプレミス |
| 推論エンジン | Vertex AI Gemini | オフライン LLM（Candle / mistral.rs） |
| サポート | コミュニティ | 商用 SLA |

### OSS 版（Apache-2.0）

Apache License 2.0. See [LICENSE](LICENSE).

本プロジェクトのベースである源内（源内Web・源内AIアプリ）は MIT ライセンスで公開されています。

本プロジェクトのメンテナー（Team KitaSan）は Apache License 2.0 の起草時にリーガルメーリングリスト（非公開）に参加しており、Apache Software Foundation の初期の個人スポンサーでもあります。

The maintainer (Team KitaSan) participated in the legal mailing list during the drafting of Apache License 2.0 and was an early individual sponsor of the Apache Software Foundation.

### 商用版（OpenCORE）

商用ライセンスに関するお問い合わせは [contact@cooljapan.tech](mailto:contact@cooljapan.tech)（COOLJAPAN OU / Team KitaSan）までご連絡ください。

---

## 謝辞 / Acknowledgments

- [デジタル庁](https://www.digital.go.jp/) — 源内（源内Web・源内AIアプリ）の公開
- [lawsy-custom-bq](https://github.com/digital-go-jp/genai-ai-api/tree/main/google-cloud/lawsy-custom-bq) — 本プロジェクトが Rust で再実装した源内の GCP 実装（MIT ライセンス）
- [Legalis-RS](https://github.com/cool-japan/legalis) — Computational Law 基盤
- [Legalis-verifier](https://crates.io/crates/legalis-verifier) — Pure Rust SMT ソルバー
- [OxiZ](https://github.com/cool-japan/oxiz) — OxiZ (replacement of Z3 in Pure Rust)
- [OxiBonsai](https://github.com/cool-japan/oxibonsai) — 世界初 Pure Rust サブ2ビット LLM 推論エンジン（zero-FFI かつ sub-2-bit の組み合わせとして先行実装なし・[記事](https://medium.com/@kitasanio/oxibonsai-the-worlds-first-pure-rust-1-bit-llm-inference-engine-4c15abf53fce)）（OpenCORE オフライン推論として検討中）
