# OxigenAI — Future Plan / 将来計画

> このドキュメントは OxigenAI の中長期ロードマップを記述します。
> Phase 1・Phase 2 は完了済みです（詳細は [TODO.md](../TODO.md) を参照）。

---

## 現在地 / Current Status（Phase 2 完了）

| 機能 | 状態 |
|------|------|
| Axum REST API（源内互換） | ✅ |
| OxiZ SMT 論理矛盾検出 | ✅ |
| `LegalResult<T>` 決定論的/裁量分離 | ✅ |
| 法令 XML → Legalis DSL コンパイル | ✅ |
| 政策シミュレーション（legalis-sim） | ✅ |
| 統一 CLI バイナリ（query/serve/compile/simulate/formalize） | ✅ |
| JpDomainMatcher（労働法・個人情報保護法・下請法・消費者契約法） | ✅ |

---

## Phase 3 — 多法域・高度推論（中期）

### 3-1. 23 法域対応（Legalis-RS の強みを全開に）

Legalis-RS は設計当初から多法域（Multi-Jurisdiction）を想定しています。
現状は日本法（`legalis-jp`）のみですが、同一 API で EU・US・国際条約に拡張できます。

**対象法域（優先順）:**

| 優先度 | 法域 | 用途 |
|--------|------|------|
| 高 | 日本法（現行） | 行政・企業法務 |
| 高 | EU GDPR | 個人情報・越境データ |
| 中 | US Federal Law | グローバル展開企業 |
| 中 | 国際条約（WTO / CISG） | 貿易・通商 |
| 低 | ASEAN 各国法 | 東南アジア進出支援 |

**実装方針:**
- `legalis-jp` と同様の `legalis-eu`・`legalis-us` モジュールを Legalis-RS エコシステムに追加
- `JpDomainMatcher` → `MultiJurisdictionMatcher` に昇格
- `/` エンドポイントに `jurisdiction` パラメータを追加（デフォルト: `JP`）

### 3-2. 法制度整備支援（対法務省 ODA）

**法制度整備支援**（ほうせいどせいびしえん）とは、日本が開発途上国・市場経済移行国に対し、法律の起草・裁判制度の運用・法曹（裁判官・検察官・弁護士）の育成を支援する政府開発援助（ODA）活動です。ベトナム・ラオス・カンボジア・ウズベキスタンなどで、日本民法・商法の経験を活かした民法典制定や司法制度の構築を技術的にサポートしてきました。

OxigenAI + Legalis-RS が解決する課題：**支援対象国の法令を日本法・国際標準と形式的に比較・検証できる。**

1. **条文の多法域形式化**（`/compile`）
   ベトナム民法・ラオス民法などの条文テキストを Legalis DSL にコンパイルし、日本民法の対応条文と同一の形式表現に変換。「規定の趣旨は同一か」「要件・効果の論理構造が等価か」を SMT で機械的に比較します。

2. **日本法との矛盾・欠缺の検出**（OxiZ SMT バックエンド）
   支援国の草案条文を日本の参照条文群と合わせて OxiZ SMT に投入し、論理的に相矛盾する規定・適用範囲の欠缺・定義の循環を自動検出します。

3. **住民影響シミュレーション**（`/simulate`）
   支援国の人口構造モデルに草案条文を適用し、「この民法典では何割の国民が決定論的に保護を受け、何割が裁量判断に委ねられるか」を統計出力します。

4. **法曹育成教材生成**
   `LegalResult::JudicialDiscretion { hint }` の `hint` フィールドに判例根拠・裁量判断の論拠を付与し、裁判官・検察官・弁護士の研修教材として活用します。

### 3-3. 法令外国語訳整備事業（対法務省）

法務省が推進する[日本法令外国語訳データベースシステム（JLT）](https://www.japaneselawtranslation.go.jp/)の訳文整備支援。

**課題：「訳文の論理的一貫性を機械的に保証できない」**

Legalis DSL は言語中立です。日本語の条文も英訳も、同一の DSL 形式（要件 → 効果 → 例外の述語論理式）に変換すれば、「翻訳前後で法的な意味構造が等価か」を OxiZ SMT で機械的に検証できます。

**具体的なユースケース：**

1. **改正追従チェック**：法令が改正された際、対応する英訳が DSL レベルで未更新であれば自動的にフラグ（JLT の慢性的な訳文遅延問題への対処）
2. **多言語展開の整合性検証**：英語・中国語・韓国語・ベトナム語の各訳文を同一 DSL に変換し、言語間の意味ドリフトを一括検出
3. **`/compile` 言語対オプション**：`oxigenai compile --query "民法第95条" --lang en` で日英 DSL ペアを生成し差分出力

法制度整備支援（ODA）と組み合わせると、OxigenAI は**途上国支援と国内訳文整備を同一パイプラインで処理**できる唯一のシステムになります。

### 3-4. Generative Jurisprudence（判例案自動生成）

「この法令解釈の争点に対して、過去の判例傾向から予測される司法判断」を生成する機能。

**アーキテクチャ案:**
1. `legalis-jp` の判例データベース（`CaseLaw` モジュール）を統合
2. 類似判例検索（BQ Vector Search + `CaseLawSearchEngine`）
3. Gemini で「この事実関係に最も近い判例群」から推定判断を生成
4. `LegalResult::JudicialDiscretion { hint }` の `hint` フィールドに判例根拠を付与

**想定エンドポイント:** `POST /predict-ruling`

### 3-5. GPU 対応シミュレーション

現行の `legalis-sim` ECS エンジンは CPU 単スレッドです。
大規模シミュレーション（100万エージェント以上）に向けて GPU 対応を検討します。

**技術選択肢:**
- [Candle](https://github.com/huggingface/candle) の CUDA バックエンドを legalis-sim に組み込む
- WGPU（クロスプラットフォーム GPU）ベースの ECS エンジン置き換え

### 3-6. パフォーマンスベンチマーク

源内（Python / Cloud Functions）との定量比較を公開します。

**計測項目:** レイテンシ（p50/p95/p99）・スループット（req/s）・GCP コスト比較・メモリ使用量

---

## Phase 4 — OpenCORE 商用版（長期）

OSS 版（Apache-2.0）を基盤に、エアギャップ・オフライン運用を可能にする商用エディション。
対象：自治体・官公庁・防衛関連機関・エンタープライズ法務部門。

### 4-1. オフライン推論エンジン

クラウド API なしで動作する LLM 推論レイヤー。

- [Candle](https://github.com/huggingface/candle) 統合（Pure Rust 推論）
- [mistral.rs](https://github.com/EricLBuehler/mistral.rs) 統合
- 量子化モデル（GGUF / GGML）の同梱・署名配布
- **[OxiBonsai](https://github.com/cool-japan/oxibonsai)（検討中）** — COOLJAPAN が開発した**世界初** Pure Rust サブ2ビット LLM 推論エンジン（[記事](https://medium.com/@kitasanio/oxibonsai-the-worlds-first-pure-rust-1-bit-llm-inference-engine-4c15abf53fce)）

**OxiBonsai について:**

[OxiBonsai](https://github.com/cool-japan/oxibonsai) は Pure Rust zero-FFI・zero-C/C++ で実装された PrismML Bonsai モデルファミリー専用推論エンジンです（v0.1.3、3,560 テスト通過、~139k 行 Pure Rust）。llama.cpp・BLAS・Fortran に一切依存せず、COOLJAPAN エコシステム（SciRS2・OxiBLAS・OxiFFT）のみで構成されています。

| モデル | サイズ | VRAM / RAM | 速度（Metal） |
|-------|--------|-----------|-------------|
| Ternary-Bonsai-1.7B | ~390 MB | ~390 MB | ~50 tok/s |
| Ternary-Bonsai-4B | ~900 MB | ~900 MB | — |
| Ternary-Bonsai-8B | ~1.75 GB | ~1.75 GB | — |
| Bonsai-8B（1-bit） | 1.15 GB | 1.15 GB | ~14.6 tok/s |

OxigenAI OpenCORE との統合検討理由：

1. **Pure Rust 一貫性**：ゼロ FFI 方針と完全一致。サプライチェーン攻撃対象面を最小化
2. **エアギャップ適性**：Ternary-Bonsai-1.7B は ~390 MB ── 署名付き USB 1枚に収まる
3. **`oxibonsai-rag` クレート**：ベクトルストアと RAG パイプラインを内蔵し、BigQuery 非依存のオフライン RAG として `bq_retriever` を置き換え可能
4. **OpenAI 互換 API**：`/v1/chat/completions` + SSE ストリーミングで `GeminiService` のオフラインドロップイン代替として実装コストが低い

```rust
// OpenCORE 検討中: GeminiService のオフライン差し替えイメージ
use oxibonsai_runtime::{EngineBuilder, SamplingPreset};

let engine = EngineBuilder::new()
    .model_path("models/Ternary-Bonsai-1.7B.gguf")  // 390MB — USB 1本
    .preset(SamplingPreset::Greedy)  // 決定論的（温度0）→ 法令解釈に最適
    .max_seq_len(4096)
    .build()?;
```

### 4-2. エアギャップ運用

- e-Gov 法令 XML の差分配信パッケージ（USB / 専用メディア）
- Ed25519 署名による配信物検証
- オフライン BigQuery 相当の組み込み Vector DB（usearch / hnswlib）
- オフライン RAG（`oxibonsai-rag` 経由）

### 4-3. エンタープライズ認証・マルチテナント

**認証プロトコル:**

| プロトコル | 用途 |
|-----------|------|
| SAML 2.0 | 省庁・自治体統合 IdP |
| OIDC | クラウドネイティブ SaaS 連携 |
| Active Directory / LDAP | 既存ドメイン統合 |

**アクセス制御:**

- 部署別ロールベースアクセス制御（RBAC）
- **関係ベースアクセス制御（ReBAC / Google Zanzibar モデル）** — 「A 省の B 部の C 係長は D 法令データセットを閲覧可能」という関係グラフによる細粒度制御
- 操作監査ログ（改ざん防止付き）
- 部署ごとの法令データセット分離

### 4-4. 形式検証証明書

- OxiZ SMT 検証結果の PDF/XML 証明書生成（タイムスタンプ付き）
- 長期署名（LTV）対応
- 法制局・審査機関への提出フォーマット対応

### 4-5. 専用ハードウェア保証

- 富士通 PRIMERGY シリーズ
- NEC Express5800 シリーズ
- 国産 GPU 基板（MN-Core 2 等）

---

## インフラ / CI・CD（継続的改善）

- GitHub Actions による `cargo nextest` + `cargo clippy` の自動実行
- crates.io への自動パブリッシュ（tag push トリガー）
- Terraform IaC の更新（GCP Cloud Run デプロイ自動化）
- マルチクラウド対応テンプレート（AWS Lambda / Azure Functions）

---

## 問い合わせ / Contact

商用版（OpenCORE）に関するお問い合わせ：[contact@cooljapan.tech](mailto:contact@cooljapan.tech)（COOLJAPAN OU / Team KitaSan）
