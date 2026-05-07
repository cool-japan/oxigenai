# サンプル出力 / Sample Outputs

OxigenAI CLI の実際の出力サンプルです。GCP（Vertex AI Gemini 2.5 Flash + BigQuery）に接続した状態で生成されました。

## compile — 法令 XML → Legalis DSL

| ファイル | コマンド | 概要 |
|---------|---------|------|
| [compile-labor-standards-act.md](compile-labor-standards-act.md) | `oxigenai compile --query "労働基準法"` | 労働基準法 251条文 → 8件の Legalis DSL に形式化 |

## formalize — 法的事実の形式化・適用判定

| ファイル | コマンド | 概要 |
|---------|---------|------|
| [formalize-fixed-term-conversion.md](formalize-fixed-term-conversion.md) | `oxigenai formalize "有期雇用5年超の無期転換権" --age 35 --attr employment_type=fixed_term --attr years_employed=6` | 有期雇用 6年の35歳に対する無期転換権の適用判定 |

## simulate — 政策シミュレーション

| ファイル | コマンド | 概要 |
|---------|---------|------|
| [simulate-labor-standards-act.md](simulate-labor-standards-act.md) | `oxigenai simulate "労働基準法の適用範囲" --population 5000` | 人口 5,000人モデルへの労働基準法適用シミュレーション |

## query — 法令レポート生成（源内互換）

| ファイル | コマンド | 概要 |
|---------|---------|------|
| [query-ai-law.md](query-ai-law.md) | `oxigenai "AIに関する法令"` | AI推進法（人工知能関連技術の研究開発及び活用の推進に関する法律）の包括分析 |
| [query-bankruptcy-law.md](query-bankruptcy-law.md) | `oxigenai "破産法の適用範囲"` | 破産法の適用範囲・要件・実務の包括分析 |
| [query-overtime-regulation.md](query-overtime-regulation.md) | `oxigenai "労働基準法の時間外規制"` | 労働基準法の時間外労働規制の包括分析 |

---

各出力には OxigenAI が源内に追加した `## 法的整合性検証` セクション（OxiZ SMT 検証結果 + `LegalResult<T>` 品質グレード）が含まれます。
