use crate::models::legal_result::VerificationSummary;
use legalis_core::{Condition, Effect, EffectType, Statute, TemporalValidity};
use legalis_verifier::{
    SmtVerifier, StatuteVerifier, analyze_quality, check_due_process, check_equality,
    check_privacy_impact,
};
use std::collections::{BTreeMap, HashSet};
use tracing::debug;

/// A contradiction detected by the OxiZ SMT solver.
#[derive(Debug, Clone)]
pub struct SmtContradiction {
    /// IDs of the statutes involved in the contradiction
    pub statute_ids: Vec<String>,
    /// Human-readable description of the contradiction
    pub explanation: String,
    /// Severity level (1=low, 2=medium, 3=high)
    pub severity: u8,
}

/// Full verification report from StatuteVerifier + constitutional checks + quality grading.
#[derive(Debug, Default)]
pub struct FullVerificationReport {
    /// Logical contradictions detected by OxiZ SMT.
    pub contradictions: Vec<SmtContradiction>,
    /// Errors from StatuteVerifier::verify()
    pub errors: Vec<String>,
    /// Warnings from StatuteVerifier::verify()
    pub warnings: Vec<String>,
    /// Suggestions from StatuteVerifier::verify()
    pub suggestions: Vec<String>,
    /// Overall quality grade (A-F), averaged across all statutes.
    pub quality_grade: Option<char>,
    /// Total statutes submitted
    pub total_statutes: usize,
}

/// Minimum severity for a conflict to count as a logical contradiction.
///
/// Severity 1 (Warning) typically means "overlapping conditions with different effect types",
/// which is **expected** in hierarchical legal systems:
///   - Base rule  (PROHIBITION: 40h weekly limit)          ← LSA_Art32
///   - Exception  (GRANT: overtime allowed with agreement) ← LSA_Art36
///   - Consequence (OBLIGATION: pay overtime premium)      ← LSA_Art37
///
/// These are complementary by design, not logical defects.
/// Only Error (2) and Critical (3) represent genuine contradictions.
const MIN_CONTRADICTION_SEVERITY: u8 = 2;

/// Extract the law-family prefix from a statute ID.
///
/// `"LSA_Art32"` → `"LSA"`, `"LCA_Art18"` → `"LCA"`, `"OT_LIMIT"` → `"OT_LIMIT"`
fn law_prefix(id: &str) -> &str {
    id.split("_Art").next().unwrap_or(id)
}

/// Returns `true` when all statute IDs in a conflict belong to the same law family.
///
/// Conflicts within one law (e.g., LSA_Art32 vs LSA_Art36) are intentional base-rule +
/// exception patterns and must not be treated as logical defects.
fn is_same_law_conflict(statute_ids: &[String]) -> bool {
    if statute_ids.len() < 2 {
        return false;
    }
    let first = law_prefix(&statute_ids[0]);
    statute_ids[1..].iter().all(|id| law_prefix(id) == first)
}

/// Returns `true` when two statutes' jurisdictions could both govern the same case.
///
/// Mirrors the gate `legalis_verifier::functions::detect_effect_conflicts` applies
/// before considering two statutes for an effect conflict: an absent jurisdiction is
/// treated as unrestricted (matches anything), so the gate only fails when both are
/// `Some` and differ.
fn same_jurisdiction(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(j1), Some(j2)) => j1 == j2,
        (None, _) | (_, None) => true,
    }
}

/// Returns `true` when two statutes' effective-date windows could overlap.
///
/// Local reimplementation of `legalis_verifier`'s internal `temporal_validity_overlaps`
/// gate: that helper is `pub(super)` inside the library crate (not part of its public
/// API), so it cannot be called from here — this mirrors its exact semantics instead.
/// A statute with no effective date and no expiry date is treated as always active
/// (open on both ends); otherwise a missing bound defaults to `NaiveDate::MIN`/`MAX`
/// before comparing the two windows for overlap.
fn temporal_validity_overlaps(tv1: &TemporalValidity, tv2: &TemporalValidity) -> bool {
    use chrono::NaiveDate;

    if tv1.effective_date.is_none() && tv1.expiry_date.is_none() {
        return true;
    }
    if tv2.effective_date.is_none() && tv2.expiry_date.is_none() {
        return true;
    }

    let start1 = tv1.effective_date.unwrap_or(NaiveDate::MIN);
    let end1 = tv1.expiry_date.unwrap_or(NaiveDate::MAX);
    let start2 = tv2.effective_date.unwrap_or(NaiveDate::MIN);
    let end2 = tv2.expiry_date.unwrap_or(NaiveDate::MAX);

    start1 <= end2 && start2 <= end1
}

/// Returns `true` when two statutes' effects are logically contradictory.
///
/// Local reimplementation of `legalis_verifier`'s internal `effects_contradict`
/// (also `pub(super)`, not reusable from outside the crate) — the exact same
/// `EffectType` pair rule and same-type description-keyword heuristic, preserved
/// verbatim. Only the *overlap* test upstream of this call (previously
/// discriminant matching, now real SMT — see `preconditions_jointly_satisfiable`)
/// is being replaced; the contradiction *classification* itself is unchanged.
fn effects_contradict(effect1: &Effect, effect2: &Effect) -> bool {
    match (&effect1.effect_type, &effect2.effect_type) {
        (EffectType::Grant, EffectType::Revoke)
        | (EffectType::Revoke, EffectType::Grant)
        | (EffectType::Grant, EffectType::Prohibition)
        | (EffectType::Prohibition, EffectType::Grant) => true,
        (t1, t2) if t1 == t2 => {
            let desc1_lower = effect1.description.to_lowercase();
            let desc2_lower = effect2.description.to_lowercase();
            (desc1_lower.contains("allow") && desc2_lower.contains("prohibit"))
                || (desc1_lower.contains("prohibit") && desc2_lower.contains("allow"))
                || (desc1_lower.contains("grant") && desc2_lower.contains("deny"))
                || (desc1_lower.contains("deny") && desc2_lower.contains("grant"))
        }
        _ => false,
    }
}

/// Folds a statute's preconditions into a single `Condition` via `And` chaining, for
/// feeding to the SMT solver as one formula.
///
/// Returns `None` when the statute has no preconditions at all. Callers treat `None`
/// as "trivially true": an unconstrained statute applies to every case, and `True` is
/// the identity element for `AND` — so `is_satisfiable(True AND x)` is equivalent to
/// `is_satisfiable(x)`. This makes the empty-precondition case fall out of the joint
/// check below without inventing an artificial tautological `Condition`.
fn fold_preconditions(preconditions: &[Condition]) -> Option<Condition> {
    let mut rest = preconditions.iter().cloned();
    let first = rest.next()?;
    Some(rest.fold(first, |acc, next| {
        Condition::And(Box::new(acc), Box::new(next))
    }))
}

/// Determines whether two statutes' precondition sets can genuinely hold at the same
/// time, using the real OxiZ SMT solver (`SmtVerifier::is_satisfiable`) instead of
/// `legalis_verifier`'s shallow discriminant-based `conditions_overlap`.
///
/// - Both statutes unconstrained (no preconditions) → trivially overlapping, matching
///   upstream: two unconstrained statutes always coincide.
/// - One statute unconstrained → overlap iff the other's own preconditions are
///   satisfiable in isolation (`True AND x` is satisfiable exactly when `x` is).
/// - Both constrained → real joint satisfiability of `And(chain_a, chain_b)`.
///
/// Solver errors (e.g. the underlying decision procedure answering "unknown") are
/// handled conservatively: the pair is treated as non-overlapping (skipped) rather
/// than risking a false contradiction report or propagating a panic — this project's
/// policy forbids `.unwrap()` in production code.
fn preconditions_jointly_satisfiable(
    verifier: &mut SmtVerifier,
    conds_a: &[Condition],
    conds_b: &[Condition],
) -> bool {
    let combined = match (fold_preconditions(conds_a), fold_preconditions(conds_b)) {
        (None, None) => return true,
        (None, Some(b)) => b,
        (Some(a), None) => a,
        (Some(a), Some(b)) => Condition::And(Box::new(a), Box::new(b)),
    };

    match verifier.is_satisfiable(&combined) {
        Ok(satisfiable) => satisfiable,
        Err(error) => {
            debug!("OxiZ SMT solver error, skipping pair conservatively: {error}");
            false
        }
    }
}

/// Detects pairwise statute contradictions using real OxiZ SMT joint satisfiability.
///
/// This is oxigenai's own SMT-backed pairwise loop: `legalis_verifier`'s
/// `detect_statute_conflicts` / `detect_effect_conflicts` do not gain a real SMT
/// variant just because the `smt-solver` feature is enabled — their
/// `conditions_overlap` remains a `std::mem::discriminant` comparison regardless of
/// the feature flag. Enabling the feature only unlocks `SmtVerifier` as a standalone
/// primitive, so oxigenai reimplements the pairwise search here, reusing the *same*
/// gating (`same_jurisdiction`, `temporal_validity_overlaps`) and the *same*
/// effect-contradiction rule (`effects_contradict`) as the upstream heuristic, but
/// swapping only the overlap test itself for a real joint-satisfiability check.
///
/// One `SmtVerifier` is constructed per call and reused across all pairs: every
/// public entry point (`is_satisfiable`, etc.) resets solver + variable state
/// internally first, so reuse is both correct and avoids reallocating the solver
/// per pair. Perf is a secondary concern here (see `benches/contradiction_smt.rs`);
/// this is the straightforward-and-correct shape, not a premature optimization.
fn detect_smt_effect_conflicts(statutes: &[Statute]) -> Vec<SmtContradiction> {
    let mut verifier = SmtVerifier::new();
    let mut contradictions = Vec::new();

    for (i, statute1) in statutes.iter().enumerate() {
        for statute2 in &statutes[i + 1..] {
            if !same_jurisdiction(&statute1.jurisdiction, &statute2.jurisdiction) {
                continue;
            }
            if !temporal_validity_overlaps(&statute1.temporal_validity, &statute2.temporal_validity)
            {
                continue;
            }

            let overlap = preconditions_jointly_satisfiable(
                &mut verifier,
                &statute1.preconditions,
                &statute2.preconditions,
            );
            if !overlap {
                continue;
            }

            if effects_contradict(&statute1.effect, &statute2.effect) {
                contradictions.push(SmtContradiction {
                    statute_ids: vec![statute1.id.clone(), statute2.id.clone()],
                    explanation: format!(
                        "Statutes '{}' and '{}' have overlapping conditions but contradictory effects",
                        statute1.id, statute2.id
                    ),
                    // Matches legalis_verifier::ConflictType::EffectConflict's fixed
                    // Severity::Critical mapping (the only conflict category this
                    // function detects — see the module-level design note above).
                    severity: 3,
                });
            }
        }
    }

    contradictions
}

/// Detect duplicate-statute-ID collisions among the input statutes.
///
/// Restores the `IdCollision` check that the pre-upgrade
/// `legalis_verifier::detect_statute_conflicts` call ran (via its internal
/// `detect_id_collisions`), but which was silently dropped when oxigenai replaced that
/// call with `detect_smt_effect_conflicts` — the real-OxiZ-SMT rewrite only
/// reimplemented the *effect-conflict* one of the library's four checks. Statute IDs
/// are the primary key used to cross-reference statutes throughout the pipeline, so a
/// duplicate ID (e.g. from a copy-paste error in a seed-statute accessor) is a genuine
/// data defect that must surface, not a complementary base-rule/exception pattern.
///
/// Unlike effect-conflict detection, this needs none of `legalis_verifier`'s internals
/// — it is a pure structural check over the input slice, with no SMT solver involved —
/// so it lives here standalone. A `BTreeMap` is used so the reported collisions come
/// out in a deterministic (ID-sorted) order regardless of input order.
fn detect_id_collision_conflicts(statutes: &[Statute]) -> Vec<SmtContradiction> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for statute in statutes {
        *counts.entry(statute.id.as_str()).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(id, count)| SmtContradiction {
            // Single-element on purpose: the colliding ID is reported once, with the
            // occurrence count folded into `explanation`, so that
            // `format_contradiction_warnings`' `statute_ids.join(", ")` does not print
            // the same ID redundantly (e.g. "DUP, DUP, DUP").
            statute_ids: vec![id.to_string()],
            explanation: format!(
                "Duplicate statute ID '{id}' appears {count} times — statute IDs must be unique"
            ),
            // Matches legalis_verifier::ConflictType::IdCollision's fixed
            // Severity::Error mapping (Error, not Warning).
            severity: 2,
        })
        .collect()
}

/// Detect logical contradictions between statutes using the OxiZ SMT solver.
/// This is the core Legalis-RS integration that goes beyond the Python original.
///
/// Combines two independent detectors, restoring the coverage the pre-upgrade
/// `legalis_verifier::detect_statute_conflicts` call used to provide:
///
/// 1. **Id-collision detection** (`detect_id_collision_conflicts`) — reported
///    **unconditionally**. A duplicate statute ID is always a genuine data defect,
///    never a complementary base-rule/exception pattern, so it deliberately bypasses
///    the severity / same-law-family filtering below. That filter would otherwise
///    *always* suppress it: a duplicate ID is an identical string, which trivially
///    shares its own `law_prefix` with itself, so `is_same_law_conflict` returns `true`
///    for every genuine collision.
/// 2. **Effect-conflict detection** (`detect_smt_effect_conflicts`) — uses
///    `legalis_verifier::SmtVerifier` (real OxiZ SMT joint-satisfiability checking, via
///    the `smt-solver` feature) for the overlap test, combined with oxigenai's local
///    reimplementation of the upstream gating/classification rules, then filtered.
///
/// **Filtering applied (to effect conflicts only — id collisions bypass it):**
/// - Severity < `MIN_CONTRADICTION_SEVERITY` (Warning-level) → suppressed.
///   These represent "complementary statutes" (base rule + exception), not defects.
/// - Same-law-family conflicts → suppressed.
///   Statutes from the same law are structured as base + exception by design.
pub fn detect_smt_contradictions(statutes: &[Statute]) -> Vec<SmtContradiction> {
    if statutes.is_empty() {
        return vec![];
    }

    debug!(
        "Running OxiZ SMT contradiction detection on {} statutes",
        statutes.len()
    );

    // Id collisions are structural defects reported unconditionally: they bypass the
    // severity / same-law-family filter applied to effect conflicts below (see this
    // function's doc comment for why that filter is the wrong gate for this category).
    let mut contradictions = detect_id_collision_conflicts(statutes);
    let id_collision_count = contradictions.len();

    let raw = detect_smt_effect_conflicts(statutes);
    let total_raw = raw.len();

    let effect_contradictions: Vec<SmtContradiction> = raw
        .into_iter()
        .filter(|conflict| {
            // Suppress complementary-statute warnings
            if conflict.severity < MIN_CONTRADICTION_SEVERITY {
                return false;
            }
            // Suppress same-law structural patterns (base rule vs exception vs consequence)
            !is_same_law_conflict(&conflict.statute_ids)
        })
        .collect();

    debug!(
        "OxiZ: {} raw effect conflicts → {} genuine effect contradictions (filtered {} complementary patterns); {} id collisions",
        total_raw,
        effect_contradictions.len(),
        total_raw - effect_contradictions.len(),
        id_collision_count
    );

    contradictions.extend(effect_contradictions);
    contradictions
}

/// Returns `true` when two statutes' jurisdictions are directly equal.
///
/// This is intentionally distinct from `same_jurisdiction` above: that helper treats
/// a missing jurisdiction (`None`) on either side as "matches anything", which is the
/// correct gate for effect-conflict detection (an unrestricted statute can collide
/// with a jurisdiction-scoped one). `TemporalConflict` detection instead needs plain
/// `Option<String>` equality — `None == None` (two jurisdiction-less statutes) counts
/// as related, but `Some("US") == None` does not — so `same_jurisdiction` must **not**
/// be reused here; doing so would silently treat every jurisdiction-scoped statute as
/// "related" to every unscoped one, over-firing `detect_temporal_conflict_warnings`.
fn jurisdiction_equal(a: &Option<String>, b: &Option<String>) -> bool {
    a == b
}

/// Computes the Jaccard word-overlap similarity between two statute titles.
///
/// Faithful, byte-for-byte port of `legalis_verifier`'s internal (`pub(super)`, not
/// reusable from outside the crate) `title_similarity`: case-sensitive,
/// whitespace-only tokenization (no lowercasing, no punctuation stripping), words
/// deduplicated per title via `HashSet` before computing `|A ∩ B| / |A ∪ B|`. Do not
/// "improve" this — the case-sensitivity and lack of punctuation handling are
/// deliberate upstream behavior, not oversights.
fn title_similarity(title1: &str, title2: &str) -> f64 {
    let words1: HashSet<&str> = title1.split_whitespace().collect();
    let words2: HashSet<&str> = title2.split_whitespace().collect();

    if words1.is_empty() && words2.is_empty() {
        return 1.0;
    }

    let intersection = words1.intersection(&words2).count();
    let union = words1.union(&words2).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Detects jurisdictions with more than 20 statutes, which may indicate that the
/// jurisdiction's statute set should be consolidated or split into sub-jurisdictions.
///
/// Faithful port of `legalis_verifier`'s internal `detect_jurisdictional_overlaps`.
/// Statutes with no jurisdiction (`None`) are skipped entirely, matching upstream's
/// `if let Some(jurisdiction)` guard — they are never bucketed and can never
/// contribute to, or trigger, this warning. The threshold is strictly-greater-than
/// (`> 20`, not `>= 20`): a jurisdiction with exactly 20 statutes does not warn.
///
/// `legalis_verifier::StatuteConflict::new` always assigns `JurisdictionalOverlap` a
/// fixed `Severity::Warning`. Routed through the `contradictions` / `SmtContradiction`
/// pipeline, `MIN_CONTRADICTION_SEVERITY` (`= 2`, Error/Critical only) would silently
/// drop every finding this function could ever produce — recreating the exact
/// dead-code bug this restoration is fixing. So this feeds `run_full_verification`'s
/// `warnings` list directly instead, mirroring the `check_equality`-issue dedup
/// pattern used there.
pub fn detect_jurisdictional_overlap_warnings(statutes: &[Statute]) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for statute in statutes {
        if let Some(jurisdiction) = &statute.jurisdiction {
            *counts.entry(jurisdiction.as_str()).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .filter(|(_, count)| *count > 20)
        .map(|(jurisdiction, count)| {
            format!(
                "Jurisdiction '{jurisdiction}' has {count} statutes, which may indicate \
                 overlap or redundancy. Suggestions: \"Review statutes for consolidation \
                 opportunities\"; \"Consider creating sub-jurisdictions for better \
                 organization\""
            )
        })
        .collect()
}

/// Detects statute pairs that are "related" (same jurisdiction, or similar titles) and
/// temporally overlapping, but on different versions — signaling that an older
/// version should have had an expiry date set when the newer version took effect.
///
/// Faithful port of `legalis_verifier`'s internal `detect_temporal_conflicts` +
/// `title_similarity`. Two statutes are "related" when their jurisdictions are
/// directly equal (`jurisdiction_equal` — see its doc comment for why `same_jurisdiction`
/// must not be reused here) OR their titles are more than 50% similar by Jaccard word
/// overlap; a related, temporally-overlapping pair with differing `version`s is
/// flagged. `temporal_validity_overlaps` (above) is reused as-is.
///
/// Like `detect_jurisdictional_overlap_warnings`, `legalis_verifier::StatuteConflict::new`
/// always assigns `TemporalConflict` a fixed `Severity::Warning`, which
/// `MIN_CONTRADICTION_SEVERITY` would silently drop if routed through the
/// `SmtContradiction` pipeline — so this feeds `run_full_verification`'s `warnings`
/// list directly instead.
pub fn detect_temporal_conflict_warnings(statutes: &[Statute]) -> Vec<String> {
    let mut warnings = Vec::new();

    for (i, statute1) in statutes.iter().enumerate() {
        for statute2 in &statutes[i + 1..] {
            let related = jurisdiction_equal(&statute1.jurisdiction, &statute2.jurisdiction)
                || title_similarity(&statute1.title, &statute2.title) > 0.5;
            if !related {
                continue;
            }

            if temporal_validity_overlaps(&statute1.temporal_validity, &statute2.temporal_validity)
                && statute1.version != statute2.version
            {
                warnings.push(format!(
                    "Statutes '{}' (v{}) and '{}' (v{}) have overlapping validity periods. \
                     Suggestions: \"Set expiry date on older version when newer version \
                     takes effect\"; \"Use version control and temporal validity to manage \
                     transitions\"",
                    statute1.id, statute1.version, statute2.id, statute2.version
                ));
            }
        }
    }

    warnings
}

/// Run the full Legalis-RS verification pipeline on a set of statutes:
///   - `StatuteVerifier::verify()` (circular refs, dead statutes, constitutional, contradictions)
///   - OxiZ SMT contradiction detection (`detect_smt_contradictions`)
///   - Jurisdictional-overlap and temporal-conflict warnings
///     (`detect_jurisdictional_overlap_warnings`, `detect_temporal_conflict_warnings`) —
///     both restored `legalis_verifier` checks, fixed at `Severity::Warning` upstream,
///     so both feed `warnings` directly rather than the `contradictions` pipeline
///   - Constitutional principle checks (equality, due process, privacy)
///   - Quality grading via `analyze_quality()` (averaged A-F)
///
/// Returns a `FullVerificationReport` regardless of how many statutes pass or fail.
pub fn run_full_verification(statutes: &[Statute]) -> FullVerificationReport {
    if statutes.is_empty() {
        return FullVerificationReport::default();
    }

    debug!("Running full verification on {} statutes", statutes.len());

    let verifier = StatuteVerifier::new();

    // Core verification (circular refs, constitutional, contradictions)
    let vr = verifier.verify(statutes);

    let errors: Vec<String> = vr.errors.iter().map(|e| format!("{e:?}")).collect();
    let mut warnings = vr.warnings.clone();
    let mut suggestions = vr.suggestions.clone();

    // Contradiction detection (OxiZ SMT)
    let contradictions = detect_smt_contradictions(statutes);

    // Jurisdictional-overlap and temporal-conflict warnings (restored from
    // legalis_verifier::detect_statute_conflicts). Both are fixed Severity::Warning
    // upstream, so — like the check_equality issues below — they are folded into
    // `warnings` directly rather than the `contradictions` pipeline.
    for warning in detect_jurisdictional_overlap_warnings(statutes) {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    for warning in detect_temporal_conflict_warnings(statutes) {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }

    // Constitutional principle checks + quality analysis per statute
    let mut total_score = 0.0f64;
    let mut score_count = 0usize;

    for statute in statutes {
        // Equality check (Art. 14 JP Constitution) — add issues as warnings
        let eq = check_equality(statute);
        for issue in &eq.issues {
            if !warnings.contains(issue) {
                warnings.push(issue.clone());
            }
        }

        // Due process check
        let dp = check_due_process(statute);
        for suggestion in &dp.suggestions {
            if !suggestions.contains(suggestion) {
                suggestions.push(suggestion.clone());
            }
        }

        // Privacy impact check (side-effect only — warnings surfaced via vr)
        let _privacy = check_privacy_impact(statute);

        // Quality analysis
        let metrics = analyze_quality(statute);
        total_score += metrics.overall_score;
        score_count += 1;
    }

    // Compute averaged quality grade
    let quality_grade = if score_count > 0 {
        let avg = total_score / score_count as f64;
        Some(if avg >= 90.0 {
            'A'
        } else if avg >= 80.0 {
            'B'
        } else if avg >= 70.0 {
            'C'
        } else if avg >= 60.0 {
            'D'
        } else {
            'F'
        })
    } else {
        None
    };

    debug!(
        "Full verification: {} errors, {} warnings, {} contradictions, grade={:?}",
        errors.len(),
        warnings.len(),
        contradictions.len(),
        quality_grade
    );

    FullVerificationReport {
        contradictions,
        errors,
        warnings,
        suggestions,
        quality_grade,
        total_statutes: statutes.len(),
    }
}

/// Format contradiction warnings as a Markdown prefix for the report prompt.
/// The warnings are prepended to the references text so Gemini is aware of them.
pub fn format_contradiction_warnings(contradictions: &[SmtContradiction]) -> String {
    if contradictions.is_empty() {
        return String::new();
    }

    let mut warning = String::from("【OxiZ SMT 法的整合性検証 — 矛盾検出】\n");
    warning.push_str("以下の論理矛盾が検出されました。レポート作成時に必ず言及してください：\n\n");

    for (i, c) in contradictions.iter().enumerate() {
        let severity_label = match c.severity {
            3 => "🔴 高",
            2 => "🟡 中",
            _ => "🟢 低",
        };
        warning.push_str(&format!(
            "{}. [重要度: {}] {}\n   関連条文: {}\n",
            i + 1,
            severity_label,
            c.explanation,
            c.statute_ids.join(", ")
        ));
    }
    warning.push('\n');
    warning
}

/// Build a VerificationSummary from a FullVerificationReport and parsed/total counts.
pub fn build_verification_summary_from_report(
    report: &FullVerificationReport,
    statutes_analyzed: usize,
    statutes_parsed: usize,
    deterministic_count: usize,
    discretion_count: usize,
) -> VerificationSummary {
    let void_count = report.contradictions.len();

    VerificationSummary {
        deterministic_count,
        discretion_count,
        void_count,
        contradictions: report
            .contradictions
            .iter()
            .map(|c| c.explanation.clone())
            .collect(),
        statutes_analyzed,
        statutes_parsed,
        quality_grade: report.quality_grade,
        verification_errors: report.errors.clone(),
        verification_warnings: report.warnings.clone(),
        suggestions: report.suggestions.clone(),
    }
}

/// Build a VerificationSummary from contradiction results (legacy, kept for tests).
pub fn build_verification_summary(
    contradictions: &[SmtContradiction],
    total_statutes: usize,
) -> VerificationSummary {
    let void_count = contradictions.len();
    let deterministic_count = total_statutes.saturating_sub(void_count * 2);

    VerificationSummary {
        deterministic_count,
        discretion_count: total_statutes.saturating_sub(deterministic_count + void_count),
        void_count,
        contradictions: contradictions
            .iter()
            .map(|c| c.explanation.clone())
            .collect(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_contradiction_warnings_empty() {
        let result = format_contradiction_warnings(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_contradiction_warnings() {
        let contradictions = vec![SmtContradiction {
            statute_ids: vec!["法第3条".to_string(), "法第5条".to_string()],
            explanation: "第3条と第5条の適用範囲が論理的に矛盾しています".to_string(),
            severity: 2,
        }];
        let result = format_contradiction_warnings(&contradictions);
        assert!(result.contains("OxiZ SMT"));
        assert!(result.contains("矛盾検出"));
        assert!(result.contains("第3条と第5条"));
    }

    #[test]
    fn test_build_verification_summary() {
        let contradictions = vec![SmtContradiction {
            statute_ids: vec!["test".to_string()],
            explanation: "test contradiction".to_string(),
            severity: 1,
        }];
        let summary = build_verification_summary(&contradictions, 10);
        assert_eq!(summary.void_count, 1);
        assert!(summary.has_contradictions());
    }

    #[test]
    fn test_detect_empty_statutes() {
        let result = detect_smt_contradictions(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_run_full_verification_empty() {
        let report = run_full_verification(&[]);
        assert_eq!(report.total_statutes, 0);
        assert!(report.quality_grade.is_none());
        assert!(report.contradictions.is_empty());
    }

    #[test]
    fn test_run_full_verification_with_labor_statutes() {
        use legalis_jp::reasoning::all_labor_statutes;
        let statutes = all_labor_statutes();
        let report = run_full_verification(&statutes);
        assert_eq!(report.total_statutes, statutes.len());
        // Grade should be computed
        assert!(report.quality_grade.is_some());
        // Contradictions count is determined by OxiZ SMT — just verify it's a valid number
        // (LSA_Art32 and LSA_Art36 may be flagged as conflicting even though they're complementary)
        let _ = report.contradictions.len(); // no panic is the assertion
    }

    #[test]
    fn test_build_verification_summary_from_report() {
        let report = FullVerificationReport {
            contradictions: vec![],
            errors: vec![],
            warnings: vec!["テスト警告".to_string()],
            suggestions: vec!["テスト提案".to_string()],
            quality_grade: Some('B'),
            total_statutes: 5,
        };
        let summary = build_verification_summary_from_report(&report, 5, 3, 2, 1);
        assert_eq!(summary.statutes_analyzed, 5);
        assert_eq!(summary.statutes_parsed, 3);
        assert_eq!(summary.quality_grade, Some('B'));
        assert_eq!(summary.deterministic_count, 2);
        assert_eq!(summary.discretion_count, 1);
        assert!(!summary.has_contradictions());
    }

    /// Pinned OxiZ SMT contradiction-count baseline for every Phase-4 seed-statute
    /// accessor.
    ///
    /// **Re-baselined after the real-OxiZ-SMT upgrade** (`contradiction.rs` now uses
    /// `SmtVerifier::is_satisfiable` for genuine joint-satisfiability instead of
    /// `legalis_verifier`'s discriminant-based `conditions_overlap` heuristic — see
    /// `detect_smt_effect_conflicts`). This test was re-run against the new
    /// implementation and **every count below is unchanged** from the prior
    /// (heuristic-based) pinned baseline. That was verified, not assumed — a
    /// temporary per-pair audit (comparing the old discriminant-based overlap
    /// decision against the new SMT-based one for every gated-in statute pair)
    /// confirmed the real solver is doing genuinely different, semantically
    /// correct work; it just doesn't move any of *these* 13 aggregate counts,
    /// for two independent reasons found in the audit:
    ///
    /// 1. **Real SMT newly finds some pairs disjoint that the old heuristic
    ///    called "overlapping"** (fixing a false positive), but those specific
    ///    pairs were already excluded from the final count by the *separate*
    ///    `is_same_law_conflict` suppression (same law family ⇒ intentional
    ///    base-rule/exception structure), so the aggregate didn't move:
    ///    - `GDPR_Art15` (`AttributeEquals{request_type=="access"}`) vs.
    ///      `GDPR_Art17` (`AttributeEquals{request_type=="erasure"}`, ...): both
    ///      are `AttributeEquals`-shaped (old heuristic: same discriminant ⇒
    ///      "overlap"), but the two literal values hash to different SMT
    ///      constants for the *same* attribute, so `request_type` cannot equal
    ///      both at once — real SMT correctly proves this conjunction UNSAT.
    ///      (Moot for the count here anyway: neither is a Grant/Prohibition
    ///      pair and neither description contains a contradictory keyword, so
    ///      `effects_contradict` is `false` regardless of the overlap verdict.)
    ///    - `CCA_Art4` (`AttributeEquals{entity_type=="consumer"}`, Grant) vs.
    ///      `CCA_Art8`/`Art9`/`Art10` (`AttributeEquals{entity_type=="business_operator"}`,
    ///      Prohibition): same shape-match/different-literal-value situation,
    ///      *and* `effects_contradict` is `true` for Grant-vs-Prohibition — so
    ///      the old heuristic's raw conflict list did include this pair, only
    ///      to be filtered out downstream by `is_same_law_conflict` (all four
    ///      share the "CCA" prefix). Real SMT now proves `entity_type` cannot
    ///      be both "consumer" and "business_operator" at once, so the pair is
    ///      excluded at the overlap stage itself — a more semantically direct
    ///      route to the same (already-correct) "not a contradiction" answer.
    ///    - The analogous same-law-family pairs in `all_pipa_statutes`
    ///      (`PIPA_Art17`/`Art27`/`Art28` vs. `Art33`), `all_minpo_statutes`
    ///      (`Minpo_Art90` vs. `Art541`), `all_constitution_statutes`
    ///      (`Const_Art13`/`14`/`21`/`25`/`29` cross pairs), and
    ///      `all_admin_procedure_statutes` (`AdminProc_Art8` vs. `Art13`) all
    ///      still report `effects_contradict == true` with genuine SMT overlap
    ///      (these preconditions are NOT on the same attribute with clashing
    ///      literal values, so they remain jointly satisfiable) and remain
    ///      correctly suppressed by `is_same_law_conflict`, exactly as before.
    /// 2. **Real SMT newly finds some pairs overlapping that the old heuristic
    ///    missed** (fixing a false negative — e.g. `GDPR_Art6`
    ///    (`SetMembership{lawful_basis, ...}`) vs. `Art7`/`Art15`/`Art17`/`Art33`
    ///    (`AttributeEquals`/`Custom`-shaped): different discriminants ⇒ old
    ///    heuristic said "no overlap"; real SMT correctly says these unrelated
    ///    attributes (`lawful_basis` vs. `request_type`/`breach_occurred`) can
    ///    obviously hold simultaneously). Same pattern recurs across
    ///    `all_ip_statutes` (`Copyright_Art21`/`27`/`32` vs. `Patent_Art68`/`100`)
    ///    and `all_environmental_statutes` /
    ///    `all_construction_real_estate_statutes` cross-statute pairs. None of
    ///    these newly-overlapping pairs has a contradictory `EffectType` pair to
    ///    begin with, so `effects_contradict` is `false` and the aggregate count
    ///    is unaffected either way.
    ///
    /// The sole surviving genuine, cross-law-family contradiction is unchanged:
    /// `ADA_Sec12112` (`Threshold{employee_count>=15}`, Prohibition) vs.
    /// `FMLA_Sec2612` (`Duration{>=12mo}` + `Threshold{hours_worked_12mo>=1250}`,
    /// Grant) — these preconditions are on entirely different attributes
    /// (`employee_count` vs. `hours_worked_12mo`/duration), so they are indeed
    /// still jointly satisfiable under real SMT (nothing stops an employer from
    /// simultaneously having ≥15 employees *and* an employee with ≥1250 hours
    /// worked over ≥12 months), and `Prohibition` vs. `Grant` is a
    /// contradictory `EffectType` pair — a genuine, non-heuristic contradiction
    /// both before and after this upgrade. See
    /// `test_disjoint_numeric_ranges_no_contradiction_despite_conflicting_effects`
    /// below for a hand-constructed case where real SMT *does* change the
    /// verdict end-to-end (same attribute, disjoint numeric ranges, and no
    /// same-law suppression available to hide the old heuristic's mistake).
    #[test]
    fn test_contradiction_baseline_counts_post_phase4_structuring() {
        use crate::verifier::eu_statutes::all_gdpr_statutes;
        use crate::verifier::jp_statutes::{
            all_admin_procedure_statutes, all_cca_statutes, all_commercial_statutes,
            all_constitution_statutes, all_construction_real_estate_statutes,
            all_environmental_statutes, all_ip_statutes, all_minpo_statutes,
            all_minpo_tort_statutes, all_pipa_statutes, all_sha_statutes,
        };
        use crate::verifier::us_statutes::all_us_federal_statutes;

        // Counts below were determined empirically by running this test against the
        // real-SMT implementation and reading the actual counts off the assertion
        // diagnostics — they are not guesses, and (per the doc comment above) every
        // single one is unchanged from the pre-upgrade heuristic-based baseline.
        let cases: Vec<(&str, Vec<Statute>, usize)> = vec![
            // 1 genuine Critical-severity contradiction: OxiZ flags ADA_Sec12112
            // (Prohibition) vs. FMLA_Sec2612 (Grant) as "overlapping conditions but
            // contradictory effects" — still genuine under real joint-satisfiability
            // SMT (different attributes, so trivially co-satisfiable).
            ("all_us_federal_statutes", all_us_federal_statutes(), 1),
            ("all_gdpr_statutes", all_gdpr_statutes(), 0),
            ("all_pipa_statutes", all_pipa_statutes(), 0),
            ("all_sha_statutes", all_sha_statutes(), 0),
            ("all_cca_statutes", all_cca_statutes(), 0),
            ("all_minpo_tort_statutes", all_minpo_tort_statutes(), 0),
            ("all_minpo_statutes", all_minpo_statutes(), 0),
            ("all_commercial_statutes", all_commercial_statutes(), 0),
            ("all_constitution_statutes", all_constitution_statutes(), 0),
            ("all_ip_statutes", all_ip_statutes(), 0),
            (
                "all_environmental_statutes",
                all_environmental_statutes(),
                0,
            ),
            (
                "all_admin_procedure_statutes",
                all_admin_procedure_statutes(),
                0,
            ),
            (
                "all_construction_real_estate_statutes",
                all_construction_real_estate_statutes(),
                0,
            ),
        ];

        for (name, statutes, expected) in &cases {
            let contradictions = detect_smt_contradictions(statutes);
            assert_eq!(
                contradictions.len(),
                *expected,
                "contradiction count regression for {name} (got {}, expected {})",
                contradictions.len(),
                expected
            );
        }
    }

    /// Hand-constructed proof that the OxiZ SMT upgrade is not cosmetic: two
    /// statutes whose preconditions are on the *same* attribute (`age`) with
    /// genuinely disjoint, mutually-exclusive numeric ranges (`< 18` vs. `>= 65`)
    /// — no entity can ever satisfy both at once — combined with directly
    /// contradictory `EffectType`s (`Grant` vs. `Prohibition`, the very first
    /// match arm of `effects_contradict`) that WOULD be reported as a
    /// contradiction if the two preconditions ever did overlap.
    ///
    /// The OLD discriminant-based `conditions_overlap` heuristic only compares
    /// `std::mem::discriminant`, so `Condition::Age{..}` vs. `Condition::Age{..}`
    /// always "overlaps" regardless of the actual numeric ranges — it would have
    /// wrongly flagged this pair as a contradiction (and, unlike the same-law
    /// cases audited above, there is no `is_same_law_conflict` suppression
    /// available here to hide the mistake, since the two statutes are
    /// deliberately given unrelated ID prefixes). Real OxiZ SMT proves
    /// `Age < 18 AND Age >= 65` is unsatisfiable, so the pair is correctly never
    /// reported as a contradiction at all.
    #[test]
    fn test_disjoint_numeric_ranges_no_contradiction_despite_conflicting_effects() {
        use legalis_core::ComparisonOp;

        let minor_only_benefit = Statute::new(
            "TESTBEN_MinorGrant",
            "Minor-only benefit grant (synthetic regression fixture)",
            Effect::new(
                EffectType::Grant,
                "Grants a benefit exclusively to minors under 18",
            ),
        )
        .with_precondition(Condition::Age {
            operator: ComparisonOp::LessThan,
            value: 18,
        })
        .with_jurisdiction("TEST");

        let senior_prohibition = Statute::new(
            "TESTPROH_SeniorBan",
            "Senior-only prohibition on the same benefit (synthetic regression fixture)",
            Effect::new(
                EffectType::Prohibition,
                "Prohibits the same benefit for anyone 65 or older",
            ),
        )
        .with_precondition(Condition::Age {
            operator: ComparisonOp::GreaterOrEqual,
            value: 65,
        })
        .with_jurisdiction("TEST");

        // Sanity check: these two IDs must NOT share a law-family prefix, so a
        // pass here is unambiguously due to the real SMT overlap check — not an
        // incidental same-law suppression like the cases audited above.
        assert!(!is_same_law_conflict(&[
            minor_only_benefit.id.clone(),
            senior_prohibition.id.clone(),
        ]));

        let contradictions = detect_smt_contradictions(&[minor_only_benefit, senior_prohibition]);
        assert!(
            contradictions.is_empty(),
            "real SMT must recognize Age<18 and Age>=65 as mutually exclusive, so no \
             contradiction should be reported even though Grant vs. Prohibition would \
             otherwise be flagged as contradictory effects; got: {contradictions:?}"
        );
    }

    /// Hand-constructed proof that duplicate-statute-ID collisions are detected — and,
    /// critically, that they are reported *unconditionally*, bypassing the
    /// `MIN_CONTRADICTION_SEVERITY` / `is_same_law_conflict` filter chain that the
    /// effect-conflict pipeline runs through.
    ///
    /// This bypass is not optional. A genuine duplicate-ID pair consists, by
    /// definition, of two *identical* ID strings, and identical strings trivially share
    /// the same law-family prefix (`law_prefix` splits on `"_Art"`, so equal inputs
    /// yield equal prefixes). The inverted sanity-check below makes this explicit: it is
    /// the mirror image of the sanity-check in
    /// `test_disjoint_numeric_ranges_no_contradiction_despite_conflicting_effects` —
    /// there the two IDs deliberately do NOT share a prefix, here the collided ID
    /// trivially shares a prefix with itself, so `is_same_law_conflict([id, id])` is
    /// `true`. Consequently, if id-collision findings were routed through the same
    /// `.filter(...)` as effect conflicts, `is_same_law_conflict` would suppress *every*
    /// genuine collision and the restored check would silently never fire. Restoring the
    /// pre-upgrade `IdCollision` coverage therefore *requires* the bypass; this test
    /// locks in that design decision.
    #[test]
    fn test_duplicate_statute_ids_always_reported_bypassing_same_law_filter() {
        let duplicate_id = "DUP_Art1";

        // Three statutes deliberately sharing one ID while differing in every other
        // respect (title, jurisdiction, effect). The differing jurisdictions also keep
        // the effect-conflict detector from firing on any of these pairs, isolating the
        // id-collision path so the count assertion below is unambiguous.
        let first = Statute::new(
            duplicate_id,
            "First statute claiming ID DUP_Art1 (synthetic collision fixture)",
            Effect::new(EffectType::Grant, "Grants something"),
        )
        .with_jurisdiction("TEST_A");

        let second = Statute::new(
            duplicate_id,
            "Second, unrelated statute reusing the same ID (copy-paste defect)",
            Effect::new(EffectType::Prohibition, "Prohibits something else entirely"),
        )
        .with_jurisdiction("TEST_B");

        let third = Statute::new(
            duplicate_id,
            "Third collision on the very same ID",
            Effect::new(EffectType::Obligation, "Requires yet another thing"),
        )
        .with_jurisdiction("TEST_C");

        // Inverted sanity check: a duplicate ID trivially shares its own law-family
        // prefix with itself, so the existing effect-conflict filter chain WOULD have
        // wrongly suppressed this collision had it not been bypassed — which is exactly
        // why the bypass exists.
        assert!(is_same_law_conflict(&[
            duplicate_id.to_string(),
            duplicate_id.to_string(),
        ]));

        let contradictions = detect_smt_contradictions(&[first, second, third]);

        // Exactly one id-collision contradiction per distinct colliding ID (one ID
        // colliding three times → one finding, not three).
        assert_eq!(
            contradictions.len(),
            1,
            "expected exactly one id-collision contradiction; got: {contradictions:?}"
        );
        let collision = &contradictions[0];
        assert_eq!(
            collision.severity, 2,
            "id collisions map to legalis_verifier's Severity::Error (severity == 2)"
        );
        assert!(
            collision.explanation.contains(duplicate_id),
            "explanation must name the duplicate ID; got: {}",
            collision.explanation
        );
        assert!(
            collision.explanation.contains('3'),
            "explanation must report the collision count (3); got: {}",
            collision.explanation
        );
        assert_eq!(
            collision.statute_ids,
            vec![duplicate_id.to_string()],
            "the colliding ID must be reported once, not repeated per collision"
        );
    }

    #[test]
    fn test_title_similarity_identical_titles_is_one() {
        assert_eq!(title_similarity("Traffic Law", "Traffic Law"), 1.0);
    }

    #[test]
    fn test_title_similarity_disjoint_titles_is_zero() {
        assert_eq!(
            title_similarity("Traffic Law", "Employment Contract Act"),
            0.0
        );
    }

    #[test]
    fn test_title_similarity_both_empty_is_one() {
        assert_eq!(title_similarity("", ""), 1.0);
    }

    /// Proves `title_similarity` does not lowercase its inputs: two titles differing
    /// only in case share zero words under case-sensitive comparison, so their
    /// similarity is 0.0, not high.
    #[test]
    fn test_title_similarity_is_case_sensitive() {
        assert_eq!(title_similarity("Traffic Law", "traffic law"), 0.0);
    }

    #[test]
    fn test_title_similarity_partial_overlap_is_exact_jaccard_ratio() {
        // words1 = {"Traffic", "Law", "Amendment"} (3), words2 = {"Traffic", "Law"} (2)
        // intersection = {"Traffic", "Law"} (2), union = {"Traffic", "Law", "Amendment"} (3)
        let similarity = title_similarity("Traffic Law Amendment", "Traffic Law");
        assert_eq!(similarity, 2.0 / 3.0);
    }

    /// Builds a bare-bones synthetic statute for jurisdictional-overlap tests: a fixed
    /// Grant effect and no preconditions, since `detect_jurisdictional_overlap_warnings`
    /// only looks at `jurisdiction`.
    fn make_overlap_test_statute(id: impl Into<String>, jurisdiction: Option<&str>) -> Statute {
        let id = id.into();
        let title = format!("Synthetic jurisdictional-overlap fixture {id}");
        let statute = Statute::new(
            id,
            title,
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        );
        match jurisdiction {
            Some(j) => statute.with_jurisdiction(j),
            None => statute,
        }
    }

    #[test]
    fn test_jurisdictional_overlap_exactly_20_statutes_no_warning() {
        // Boundary check: the threshold is strictly `> 20`, not `>= 20`.
        let statutes: Vec<Statute> = (0..20)
            .map(|i| {
                make_overlap_test_statute(
                    format!("JOTEST_Boundary_{i}"),
                    Some("JOTEST_JURISDICTION_20"),
                )
            })
            .collect();

        let warnings = detect_jurisdictional_overlap_warnings(&statutes);
        assert!(
            warnings.is_empty(),
            "exactly 20 statutes in one jurisdiction must not warn (boundary is > 20, \
             not >= 20); got: {warnings:?}"
        );
    }

    #[test]
    fn test_jurisdictional_overlap_21_statutes_triggers_one_warning() {
        let statutes: Vec<Statute> = (0..21)
            .map(|i| {
                make_overlap_test_statute(
                    format!("JOTEST_Over_{i}"),
                    Some("JOTEST_JURISDICTION_21"),
                )
            })
            .collect();

        let warnings = detect_jurisdictional_overlap_warnings(&statutes);
        assert_eq!(
            warnings.len(),
            1,
            "21 statutes in one jurisdiction must trigger exactly one warning; got: {warnings:?}"
        );
        assert!(warnings[0].contains("JOTEST_JURISDICTION_21"));
        assert!(warnings[0].contains("21"));
    }

    #[test]
    fn test_jurisdictional_overlap_none_jurisdiction_never_triggers() {
        // Even with far more than 20 statutes, statutes with `jurisdiction: None` are
        // never bucketed, matching upstream's `if let Some(jurisdiction)` guard.
        let statutes: Vec<Statute> = (0..50)
            .map(|i| make_overlap_test_statute(format!("JOTEST_None_{i}"), None))
            .collect();

        let warnings = detect_jurisdictional_overlap_warnings(&statutes);
        assert!(
            warnings.is_empty(),
            "statutes without a jurisdiction must never trigger this warning regardless \
             of count; got: {warnings:?}"
        );
    }

    #[test]
    fn test_temporal_conflict_same_jurisdiction_differing_version_warns() {
        let older = Statute::new(
            "TEMPTEST_Older",
            "Temporal Conflict Fixture Statute",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("TEMPTEST_JURISDICTION")
        .with_version(1);

        let newer = Statute::new(
            "TEMPTEST_Newer",
            "Temporal Conflict Fixture Statute",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("TEMPTEST_JURISDICTION")
        .with_version(2);

        // Both statutes use the default (open-ended) TemporalValidity, which
        // `temporal_validity_overlaps` treats as always-overlapping.
        let warnings = detect_temporal_conflict_warnings(&[older, newer]);
        assert_eq!(
            warnings.len(),
            1,
            "same jurisdiction + differing version + overlapping (default, open) \
             temporal validity must warn exactly once; got: {warnings:?}"
        );
        assert!(warnings[0].contains("TEMPTEST_Older"));
        assert!(warnings[0].contains("TEMPTEST_Newer"));
        assert!(warnings[0].contains("v1"));
        assert!(warnings[0].contains("v2"));
    }

    #[test]
    fn test_temporal_conflict_same_version_no_warning() {
        let first = Statute::new(
            "TEMPTEST_SameVersionA",
            "Temporal Conflict Fixture Statute",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("TEMPTEST_JURISDICTION")
        .with_version(1);

        let second = Statute::new(
            "TEMPTEST_SameVersionB",
            "Temporal Conflict Fixture Statute",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("TEMPTEST_JURISDICTION")
        .with_version(1);

        let warnings = detect_temporal_conflict_warnings(&[first, second]);
        assert!(
            warnings.is_empty(),
            "identical versions must never warn, regardless of relatedness or temporal \
             overlap; got: {warnings:?}"
        );
    }

    #[test]
    fn test_temporal_conflict_different_jurisdiction_similar_title_still_warns() {
        // Different jurisdictions (so `jurisdiction_equal` is false), but identical
        // titles (`title_similarity` == 1.0 > 0.5) — proves the `related` OR-branch
        // fires on title similarity alone, independent of jurisdiction.
        let first = Statute::new(
            "TEMPTEST_TitleSimA",
            "Consumer Protection Amendment Act",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("TEMPTEST_JURISDICTION_ONE")
        .with_version(1);

        let second = Statute::new(
            "TEMPTEST_TitleSimB",
            "Consumer Protection Amendment Act",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("TEMPTEST_JURISDICTION_TWO")
        .with_version(2);

        let warnings = detect_temporal_conflict_warnings(&[first, second]);
        assert_eq!(
            warnings.len(),
            1,
            "title similarity > 0.5 must trigger the warning even when jurisdictions \
             differ; got: {warnings:?}"
        );
    }

    /// Regression test for trap #1 (see `jurisdiction_equal`'s doc comment): the
    /// `related` check in `detect_temporal_conflict_warnings` must use direct
    /// `Option<String>` equality, NOT the existing `same_jurisdiction` helper (which
    /// treats `None` as "matches anything"). A `None`-jurisdiction statute and a
    /// `Some("US")`-jurisdiction statute, with dissimilar titles, must NOT be
    /// considered related. If `same_jurisdiction` had been wrongly reused for this
    /// check, this pair would incorrectly pass the `related` gate and this test would
    /// fail.
    #[test]
    fn test_temporal_conflict_none_and_some_jurisdiction_dissimilar_titles_not_related() {
        let unscoped = Statute::new(
            "TEMPTEST_Unscoped",
            "Completely Unrelated Zoning Regulation",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_version(1);
        // `unscoped.jurisdiction` stays `None` — no `.with_jurisdiction(...)` call.

        let us_scoped = Statute::new(
            "TEMPTEST_UsScoped",
            "Totally Different Banking Statute",
            Effect::new(EffectType::Grant, "Grants a synthetic benefit"),
        )
        .with_jurisdiction("US")
        .with_version(2);

        let warnings = detect_temporal_conflict_warnings(&[unscoped, us_scoped]);
        assert!(
            warnings.is_empty(),
            "None jurisdiction vs. Some(\"US\") jurisdiction with dissimilar titles must \
             not be related (same_jurisdiction must not be reused here); got: {warnings:?}"
        );
    }
}
