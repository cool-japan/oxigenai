# `oxigenai formalize` — 有期雇用5年超の無期転換権

```bash
oxigenai formalize "有期雇用5年超の無期転換権" \
  --age 35 \
  --attr employment_type=fixed_term \
  --attr years_employed=6 \
  --description "5年以上の有期雇用契約を更新してきた"
```

```text
=== 法令適用評価: 有期雇用5年超の無期転換権 ===

適用条文: 10件 / 評価: 11件

【適用される条文】
  ✅ LSA_Art32 — 法定労働時間 / Statutory Working Hours (Art. 32)
     効果: 1日8時間、週40時間を超えて労働させてはならない / Max 8 hours/day, 40 hours/week
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LSA_Art34 — 休憩時間 / Rest Periods (Art. 34)
     効果: 6時間超で45分、8時間超で60分の休憩を与えなければならない / 45min for 6h+, 60min for 8h+
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LSA_Art35 — 週休 / Weekly Day Off (Art. 35)
     効果: 毎週少なくとも1日の休日を与えなければならない / At least 1 day off per week
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LSA_Art37 — 割増賃金 / Overtime Premium Pay (Art. 37)
     効果: 時間外25%、深夜25%、休日35%以上の割増賃金 / 25% overtime, 25% late night, 35% holiday premium
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LSA_Art20 — 解雇予告 / Advance Notice of Dismissal (Art. 20)
     効果: 30日前の予告または30日分の平均賃金支払が必要 / 30 days notice or payment in lieu
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LSA_Art39 — 年次有給休暇 / Annual Paid Leave (Art. 39)
     効果: 6ヶ月継続勤務で10日、以降増加 / 10 days after 6 months, increasing thereafter
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LCA_Art18 — 無期転換ルール / Indefinite Conversion Rule (Art. 18)
     効果: 有期契約5年超で無期転換申込権 / Right to convert after 5+ years of fixed-term
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ LCA_Art16 — 解雇権濫用法理 / Abusive Dismissal Doctrine (Art. 16)
     効果: 客観的合理的理由と社会通念上の相当性を欠く解雇は無効 / Dismissal void if lacking objective reason
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ MWA — 最低賃金 / Minimum Wage Act
     効果: 地域別または特定産業別の最低賃金 / Regional or industry-specific minimum wage
     条文の適用条件が充足されています。機械的に適用されます。
  ✅ OT_LIMIT — 時間外労働上限規制 / Overtime Limit Regulation
     効果: 原則月45時間・年360時間、特別条項でも月100時間未満・年720時間以内 / Max 45h/month, 360h/year normally
     条文の適用条件が充足されています。機械的に適用されます。

【適用されない / 裁量的判断が必要な条文】
  ⚖️  LSA_Art36 — 36協定 / Overtime Agreement (Art. 36)
     適用判断には人間の裁量が必要です: 条文「36協定 / Overtime Agreement (Art. 36)」の適用条件が現在の事実では充足されません。
```
