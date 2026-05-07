# `oxigenai compile` — 労働基準法 DSL コンパイル

```bash
oxigenai compile --query "労働基準法"
```

**法令: 労働基準法**

```text
条文: 251件 → 8件 形式化
STATUTE LSA_Art32: "法定労働時間 / Statutory Working Hours (Art. 32)" {
    JURISDICTION "JP"
    THEN PROHIBITION "1日8時間、週40時間を超えて労働させてはならない / Max 8 hours/day, 40 hours/week"
}

STATUTE LSA_Art34: "休憩時間 / Rest Periods (Art. 34)" {
    JURISDICTION "JP"
    THEN GRANT "6時間超で45分、8時間超で60分の休憩を与えなければならない / 45min for 6h+, 60min for 8h+"
}

STATUTE LSA_Art35: "週休 / Weekly Day Off (Art. 35)" {
    JURISDICTION "JP"
    THEN GRANT "毎週少なくとも1日の休日を与えなければならない / At least 1 day off per week"
}

STATUTE LSA_Art36: "36協定 / Overtime Agreement (Art. 36)" {
    JURISDICTION "JP"
    WHEN "has_36_agreement" = "true"
    THEN GRANT "36協定締結により時間外労働が可能 / Overtime permitted with agreement"
}

STATUTE LSA_Art37: "割増賃金 / Overtime Premium Pay (Art. 37)" {
    JURISDICTION "JP"
    THEN GRANT "時間外25%、深夜25%、休日35%以上の割増賃金 / 25% overtime, 25% late night, 35% holiday premium"
}

STATUTE LSA_Art20: "解雇予告 / Advance Notice of Dismissal (Art. 20)" {
    JURISDICTION "JP"
    THEN OBLIGATION "30日前の予告または30日分の平均賃金支払が必要 / 30 days notice or payment in lieu"
}

STATUTE LSA_Art39: "年次有給休暇 / Annual Paid Leave (Art. 39)" {
    JURISDICTION "JP"
    WHEN DURATION >= 6 months
    THEN GRANT "6ヶ月継続勤務で10日、以降増加 / 10 days after 6 months, increasing thereafter"
}

STATUTE OT_LIMIT: "時間外労働上限規制 / Overtime Limit Regulation" {
    JURISDICTION "JP"
    THEN PROHIBITION "原則月45時間・年360時間、特別条項でも月100時間未満・年720時間以内 / Max 45h/month, 360h/year normally"
}
```
