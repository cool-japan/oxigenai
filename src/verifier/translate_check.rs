//! Translation-consistency checking (Phase 3-3).
//!
//! Verifies that a statute and its translation carry the **same legal meaning**
//! by compiling *both* e-Gov-style documents to Legalis DSL (`legalis_core::Statute`
//! sets) and comparing them **structurally** — never by comparing surface text.
//!
//! The comparison is language-agnostic: it looks only at the compiled
//! [`Statute`] graph (statute count, [`EffectType`] multiset, and the recursive
//! multiset of precondition node kinds), so a Japanese source and an English
//! target that encode the same rules score `1.0` even though no characters match.
//!
//! ## Why structural, not lexical?
//!
//! Two faithful translations of 労働基準法 第32条 ("使用者は…四十時間を超えて…
//! 労働させてはならない" / "An employer shall not have a worker work more than
//! forty hours…") share **no tokens**, yet both compile to a single
//! [`EffectType::Prohibition`] statute gated on the same working-time condition.
//! A meaning-preserving translation therefore yields an identical structural
//! signature; a *divergent* one (dropped exception, flipped obligation, missing
//! article) perturbs the signature and is surfaced as a [`Divergence`].
//!
//! ## `legalis-i18n`
//!
//! `legalis-i18n` 0.1.5 was evaluated and deliberately **not** used here: its
//! public surface is machine-translation engines (`NeuralMachineTranslator`,
//! `TranslationMemory`) and hand-curated equivalence registries
//! (`RegulatoryEquivalenceMapper`, `TermEquivalence`) plus citation formatting —
//! none of which operate on the compiled `legalis_core::Statute` ADT. This module
//! is fully self-contained and offline. See `TODO.md` for the full finding.

use legalis_core::{Condition, EffectType, Statute};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Relative weight of the statute-count dimension in the overall score.
const COUNT_WEIGHT: f64 = 0.25;
/// Relative weight of the effect-type dimension (legal meaning lives here).
const EFFECT_WEIGHT: f64 = 0.45;
/// Relative weight of the precondition-kind dimension.
const CONDITION_WEIGHT: f64 = 0.30;

/// A single structural difference between source and target statute sets.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Divergence {
    /// Machine-readable category: `"statute_count"`, `"effect_type"`,
    /// or `"precondition_kind"`.
    pub kind: String,
    /// Human-readable Japanese explanation of the difference.
    pub detail: String,
    /// The source-side value (count, as text) for the differing key.
    pub source: String,
    /// The target-side value (count, as text) for the differing key.
    pub target: String,
}

/// A language-agnostic structural fingerprint of a compiled statute set.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StructuralSignature {
    /// Number of statutes compiled from the document.
    pub statute_count: usize,
    /// Total number of precondition nodes (recursively counted).
    pub precondition_count: usize,
    /// Multiset of [`EffectType`] names → occurrence count.
    pub effect_types: BTreeMap<String, usize>,
    /// Multiset of recursive precondition node-kind names → occurrence count.
    pub condition_kinds: BTreeMap<String, usize>,
}

/// Result of structurally comparing a source statute set with its translation.
#[derive(Debug, Clone, Serialize)]
pub struct TranslationComparison {
    /// `true` iff the two signatures are structurally identical (no divergences).
    pub equivalent: bool,
    /// Overall weighted similarity in `[0.0, 1.0]` (1.0 = identical structure).
    pub score: f64,
    /// Sub-score for matching statute counts.
    pub count_score: f64,
    /// Sub-score (Sørensen–Dice over the effect-type multiset).
    pub effect_score: f64,
    /// Sub-score (Sørensen–Dice over the precondition-kind multiset).
    pub condition_score: f64,
    /// Every detected structural difference (empty ⟺ `equivalent`).
    pub divergences: Vec<Divergence>,
    /// Structural fingerprint of the source document.
    pub source_signature: StructuralSignature,
    /// Structural fingerprint of the target (translated) document.
    pub target_signature: StructuralSignature,
}

/// Stable name for an [`EffectType`] variant (translation-invariant).
#[must_use]
pub fn effect_type_name(effect_type: &EffectType) -> &'static str {
    match effect_type {
        EffectType::Grant => "Grant",
        EffectType::Revoke => "Revoke",
        EffectType::Obligation => "Obligation",
        EffectType::Prohibition => "Prohibition",
        EffectType::MonetaryTransfer => "MonetaryTransfer",
        EffectType::StatusChange => "StatusChange",
        EffectType::Custom => "Custom",
    }
}

/// Stable name for the outermost variant of a [`Condition`] node.
///
/// Used as the per-node "kind" when building the recursive condition multiset.
#[must_use]
pub fn condition_kind(condition: &Condition) -> &'static str {
    match condition {
        Condition::Age { .. } => "Age",
        Condition::Income { .. } => "Income",
        Condition::HasAttribute { .. } => "HasAttribute",
        Condition::AttributeEquals { .. } => "AttributeEquals",
        Condition::DateRange { .. } => "DateRange",
        Condition::Geographic { .. } => "Geographic",
        Condition::EntityRelationship { .. } => "EntityRelationship",
        Condition::ResidencyDuration { .. } => "ResidencyDuration",
        Condition::Duration { .. } => "Duration",
        Condition::Percentage { .. } => "Percentage",
        Condition::SetMembership { .. } => "SetMembership",
        Condition::Pattern { .. } => "Pattern",
        Condition::Calculation { .. } => "Calculation",
        Condition::Composite { .. } => "Composite",
        Condition::Threshold { .. } => "Threshold",
        Condition::Fuzzy { .. } => "Fuzzy",
        Condition::Probabilistic { .. } => "Probabilistic",
        Condition::Temporal { .. } => "Temporal",
        Condition::And(..) => "And",
        Condition::Or(..) => "Or",
        Condition::Not(..) => "Not",
        Condition::Custom { .. } => "Custom",
    }
}

/// Recursively tallies every condition node-kind into `tally`.
///
/// Compound nodes (`And`/`Or`/`Not`/`Composite`/`Probabilistic`) are counted
/// themselves *and* descended into, so dropping a branch in translation changes
/// the multiset.
fn collect_condition_kinds(condition: &Condition, tally: &mut BTreeMap<String, usize>) {
    *tally
        .entry(condition_kind(condition).to_string())
        .or_insert(0) += 1;
    match condition {
        Condition::And(left, right) | Condition::Or(left, right) => {
            collect_condition_kinds(left, tally);
            collect_condition_kinds(right, tally);
        }
        Condition::Not(inner)
        | Condition::Probabilistic {
            condition: inner, ..
        } => {
            collect_condition_kinds(inner, tally);
        }
        Condition::Composite { conditions, .. } => {
            for (_weight, inner) in conditions {
                collect_condition_kinds(inner, tally);
            }
        }
        _ => {}
    }
}

/// Builds the language-agnostic structural fingerprint of a statute set.
#[must_use]
pub fn summarize(statutes: &[Statute]) -> StructuralSignature {
    let mut effect_types: BTreeMap<String, usize> = BTreeMap::new();
    let mut condition_kinds: BTreeMap<String, usize> = BTreeMap::new();

    for statute in statutes {
        *effect_types
            .entry(effect_type_name(&statute.effect.effect_type).to_string())
            .or_insert(0) += 1;
        for precondition in &statute.preconditions {
            collect_condition_kinds(precondition, &mut condition_kinds);
        }
    }

    let precondition_count = condition_kinds.values().sum();

    StructuralSignature {
        statute_count: statutes.len(),
        precondition_count,
        effect_types,
        condition_kinds,
    }
}

/// Sørensen–Dice coefficient over two integer multisets in `[0.0, 1.0]`.
///
/// `1.0` when the multisets are identical (including both empty); shrinks as the
/// symmetric difference grows.
fn multiset_dice(a: &BTreeMap<String, usize>, b: &BTreeMap<String, usize>) -> f64 {
    let total: usize = a.values().sum::<usize>() + b.values().sum::<usize>();
    if total == 0 {
        return 1.0;
    }
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    let intersection: usize = keys
        .iter()
        .map(|k| {
            a.get(*k)
                .copied()
                .unwrap_or(0)
                .min(b.get(*k).copied().unwrap_or(0))
        })
        .sum();
    (2.0 * intersection as f64) / total as f64
}

/// Closeness of two counts in `[0.0, 1.0]` (`1.0` when equal).
fn count_similarity(a: usize, b: usize) -> f64 {
    let max = a.max(b);
    if max == 0 {
        return 1.0;
    }
    1.0 - (a.abs_diff(b) as f64 / max as f64)
}

/// Rounds to 4 decimal places for stable, presentation-friendly scores.
fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// Collects per-key divergences between two multisets into `out`.
fn diff_multisets(
    source: &BTreeMap<String, usize>,
    target: &BTreeMap<String, usize>,
    kind: &str,
    label: &str,
    out: &mut Vec<Divergence>,
) {
    let keys: BTreeSet<&String> = source.keys().chain(target.keys()).collect();
    for key in keys {
        let src = source.get(key).copied().unwrap_or(0);
        let tgt = target.get(key).copied().unwrap_or(0);
        if src != tgt {
            out.push(Divergence {
                kind: kind.to_string(),
                detail: format!(
                    "{label}「{key}」の件数が一致しません（源文 {src} / 訳文 {tgt}）。"
                ),
                source: src.to_string(),
                target: tgt.to_string(),
            });
        }
    }
}

/// Structurally compares a source statute set against its translation.
///
/// Pure and offline: identical structure ⇒ `equivalent = true`, `score = 1.0`;
/// any structural perturbation (count, effect type, or precondition kind) lowers
/// the score and is itemised in `divergences`.
#[must_use]
pub fn compare_statutes(source: &[Statute], target: &[Statute]) -> TranslationComparison {
    let source_signature = summarize(source);
    let target_signature = summarize(target);

    let count_score = count_similarity(
        source_signature.statute_count,
        target_signature.statute_count,
    );
    let effect_score = multiset_dice(
        &source_signature.effect_types,
        &target_signature.effect_types,
    );
    let condition_score = multiset_dice(
        &source_signature.condition_kinds,
        &target_signature.condition_kinds,
    );

    let score = COUNT_WEIGHT * count_score
        + EFFECT_WEIGHT * effect_score
        + CONDITION_WEIGHT * condition_score;

    let mut divergences = Vec::new();
    if source_signature.statute_count != target_signature.statute_count {
        divergences.push(Divergence {
            kind: "statute_count".to_string(),
            detail: format!(
                "条文数が一致しません（源文 {} 件 / 訳文 {} 件）。",
                source_signature.statute_count, target_signature.statute_count
            ),
            source: source_signature.statute_count.to_string(),
            target: target_signature.statute_count.to_string(),
        });
    }
    diff_multisets(
        &source_signature.effect_types,
        &target_signature.effect_types,
        "effect_type",
        "法的効果",
        &mut divergences,
    );
    diff_multisets(
        &source_signature.condition_kinds,
        &target_signature.condition_kinds,
        "precondition_kind",
        "適用条件",
        &mut divergences,
    );

    TranslationComparison {
        equivalent: divergences.is_empty(),
        score: round4(score),
        count_score: round4(count_score),
        effect_score: round4(effect_score),
        condition_score: round4(condition_score),
        divergences,
        source_signature,
        target_signature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legalis_core::{ComparisonOp, Effect};

    /// A working-time prohibition gated on an age condition (labour-law shaped).
    fn working_time_statute(id: &str) -> Statute {
        Statute::new(
            id,
            "労働時間",
            Effect::new(
                EffectType::Prohibition,
                "週40時間を超える労働をさせてはならない",
            ),
        )
        .with_precondition(Condition::Age {
            operator: ComparisonOp::GreaterOrEqual,
            value: 18,
        })
    }

    /// A leave-entitlement grant gated on an employment-duration condition.
    fn leave_statute(id: &str) -> Statute {
        Statute::new(
            id,
            "年次有給休暇",
            Effect::new(EffectType::Grant, "年次有給休暇を付与する"),
        )
        .with_precondition(Condition::And(
            Box::new(Condition::Age {
                operator: ComparisonOp::GreaterOrEqual,
                value: 18,
            }),
            Box::new(Condition::HasAttribute {
                key: "continuous_service".to_string(),
            }),
        ))
    }

    #[test]
    fn identical_translation_is_equivalent() {
        let source = vec![working_time_statute("a"), leave_statute("b")];
        // The "translation" carries the same structure with different ids/text.
        let target = vec![working_time_statute("a_en"), leave_statute("b_en")];

        let result = compare_statutes(&source, &target);
        assert!(result.equivalent, "identical structure must be equivalent");
        assert!(result.divergences.is_empty());
        assert_eq!(result.score, 1.0);
        assert_eq!(result.effect_score, 1.0);
        assert_eq!(result.condition_score, 1.0);
        assert_eq!(result.count_score, 1.0);
    }

    #[test]
    fn flipped_effect_type_is_detected() {
        let source = vec![working_time_statute("a")];
        // Translation mistranslates the prohibition as a permission/grant.
        let target = vec![
            Statute::new(
                "a_en",
                "Working hours",
                Effect::new(EffectType::Grant, "may work over 40h/week"),
            )
            .with_precondition(Condition::Age {
                operator: ComparisonOp::GreaterOrEqual,
                value: 18,
            }),
        ];

        let result = compare_statutes(&source, &target);
        assert!(!result.equivalent, "flipped effect must diverge");
        assert!(result.score < 1.0);
        let effect_div: Vec<_> = result
            .divergences
            .iter()
            .filter(|d| d.kind == "effect_type")
            .collect();
        // Both Prohibition (source-only) and Grant (target-only) are reported.
        assert_eq!(effect_div.len(), 2);
        assert!(result.divergences.iter().all(|d| d.kind == "effect_type"));
        // Statute count and condition kinds still match.
        assert_eq!(result.count_score, 1.0);
        assert_eq!(result.condition_score, 1.0);
    }

    #[test]
    fn dropped_precondition_branch_is_detected() {
        let source = vec![leave_statute("b")];
        // Translation drops the HasAttribute branch (and the AND wrapper).
        let target = vec![
            Statute::new(
                "b_en",
                "Annual leave",
                Effect::new(EffectType::Grant, "grant annual paid leave"),
            )
            .with_precondition(Condition::Age {
                operator: ComparisonOp::GreaterOrEqual,
                value: 18,
            }),
        ];

        let result = compare_statutes(&source, &target);
        assert!(!result.equivalent);
        // Effect types still match exactly (both Grant).
        assert_eq!(result.effect_score, 1.0);
        // The And and HasAttribute nodes are lost → precondition divergences.
        let kinds: BTreeSet<&str> = result.divergences.iter().map(|d| d.kind.as_str()).collect();
        assert!(kinds.contains("precondition_kind"));
        assert!(result.condition_score < 1.0);
        // Source had And+Age+HasAttribute (3 nodes); target only Age (1 node).
        assert_eq!(result.source_signature.precondition_count, 3);
        assert_eq!(result.target_signature.precondition_count, 1);
    }

    #[test]
    fn missing_statute_changes_count() {
        let source = vec![working_time_statute("a"), leave_statute("b")];
        // Translation omits the second article entirely.
        let target = vec![working_time_statute("a_en")];

        let result = compare_statutes(&source, &target);
        assert!(!result.equivalent);
        assert!(result.count_score < 1.0);
        assert!(
            result.divergences.iter().any(|d| d.kind == "statute_count"),
            "missing statute must raise a statute_count divergence"
        );
    }

    #[test]
    fn both_empty_is_trivially_equivalent() {
        let result = compare_statutes(&[], &[]);
        assert!(result.equivalent);
        assert_eq!(result.score, 1.0);
        assert!(result.divergences.is_empty());
    }

    #[test]
    fn partial_overlap_scores_between_zero_and_one() {
        // Two source effects {Obligation, Prohibition}; target {Grant, Prohibition}.
        let source = vec![
            Statute::new("o", "義務", Effect::new(EffectType::Obligation, "x")),
            Statute::new("p", "禁止", Effect::new(EffectType::Prohibition, "y")),
        ];
        let target = vec![
            Statute::new("g", "権利", Effect::new(EffectType::Grant, "x")),
            Statute::new("p", "禁止", Effect::new(EffectType::Prohibition, "y")),
        ];
        let result = compare_statutes(&source, &target);
        // Dice over {Obligation,Prohibition} vs {Grant,Prohibition} = 2*1/4 = 0.5.
        assert_eq!(result.effect_score, 0.5);
        assert!(result.score > 0.0 && result.score < 1.0);
        assert!(!result.equivalent);
    }

    #[test]
    fn condition_kind_helper_covers_compound_nodes() {
        // A NOT wrapping an OR of two leaves: 4 nodes total (Not, Or, Income, Age).
        let nested = Condition::Not(Box::new(Condition::Or(
            Box::new(Condition::Income {
                operator: ComparisonOp::LessThan,
                value: 100,
            }),
            Box::new(Condition::Age {
                operator: ComparisonOp::GreaterOrEqual,
                value: 65,
            }),
        )));
        let mut tally = BTreeMap::new();
        collect_condition_kinds(&nested, &mut tally);
        assert_eq!(tally.get("Not"), Some(&1));
        assert_eq!(tally.get("Or"), Some(&1));
        assert_eq!(tally.get("Income"), Some(&1));
        assert_eq!(tally.get("Age"), Some(&1));
        assert_eq!(tally.values().sum::<usize>(), 4);
    }
}
