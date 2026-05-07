# `oxigenai simulate` — 労働基準法 政策シミュレーション（人口 5,000人）

```bash
oxigenai simulate "労働基準法の適用範囲" --population 5000
```

> 条文: 8件 | 人口: 5000人 — シミュレーション実行中...

## 政策シミュレーション結果

**対象人口:** 5000人 | **対象条文:** 8件 | **総適用試行:** 40000件

### 全体影響度

| 分類 | 件数 | 割合 |
|------|------|------|
| ✅ 決定論的適用 | 30734 | 76.8% |
| ⚖️ 裁量的判断 | 9213 | 23.0% |
| ❌ 論理矛盾 | 53 | 0.1% |

**品質評価: B** — 法令の適用は概ね決定論的です。

### 条文別影響分析

| 条文ID | タイトル | 適用率 | 曖昧度 | 総計 |
|--------|----------|--------|--------|------|
| LSA_Art32 | 法定労働時間 / Statutory Working Hou | 100.0% | 0.0% | 5000 |
| LSA_Art34 | 休憩時間 / Rest Periods (Art. 34) | 100.0% | 0.0% | 5000 |
| LSA_Art35 | 週休 / Weekly Day Off (Art. 35) | 100.0% | 0.0% | 5000 |
| LSA_Art36 | 36協定 / Overtime Agreement (Art | 0.0% | 100.0% | 5000 |
| LSA_Art37 | 割増賃金 / Overtime Premium Pay (A | 100.0% | 0.0% | 5000 |
| LSA_Art20 | 解雇予告 / Advance Notice of Dismi | 100.0% | 0.0% | 5000 |
| LSA_Art39 | 年次有給休暇 / Annual Paid Leave (Ar | 14.7% | 84.3% | 5000 |
| OT_LIMIT | 時間外労働上限規制 / Overtime Limit Reg | 100.0% | 0.0% | 5000 |

*Powered by Legalis-Sim ECS Engine + OxiZ SMT Solver*

決定論的適用率: 76.8%
