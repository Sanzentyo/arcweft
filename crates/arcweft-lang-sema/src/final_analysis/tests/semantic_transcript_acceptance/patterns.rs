use super::*;
use crate::final_analysis::CheckedRecordPatternOwner;
use arcweft_lang_hir::pattern::{HirPatternChild, HirPatternKind, HirPatternSequenceRest};

fn first_arm_pattern_digest(source: &str) -> [u8; 32] {
    checked_match_pattern_observation(source, 0).arm_pattern_digest
}

fn sequence_pattern_source(pattern: &str) -> String {
    format!(
        "fn root(values: Vec<bool>) -> i64 {{\n    match values {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
    )
}

fn tuple_pattern_source(pattern: &str) -> String {
    format!(
        "fn root(pair: (bool, bool)) -> i64 {{\n    match pair {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
    )
}

fn result_pattern_source(ok_pattern: &str) -> String {
    format!(
        "fn root(value: Result<bool, String>) -> i64 {{\n    match value {{\n        .Ok({ok_pattern}) => 1i64\n        .Err(error) => 2i64\n    }}\n}}\n"
    )
}

fn choice_pattern_source(first: &str, second: &str) -> String {
    format!(
        "fn root(value: String | Bytes) -> i64 {{\n    match value {{\n        text: {first} => 1i64\n        bytes: {second} => 2i64\n    }}\n}}\n"
    )
}

fn record_variant_pattern_source(field_type: &str) -> String {
    format!(
        "enum Event {{\n    Start,\n    Empty {{}},\n    ChoiceSelected {{ id: {field_type} }},\n}}\nfn root(event: Event) -> i64 {{\n    match event {{\n        .Start => 0i64\n        .Empty {{}} => 1i64\n        .ChoiceSelected {{ id }} => 2i64\n    }}\n}}\n"
    )
}

fn mutable_binding_pattern_source(pattern: &str) -> String {
    format!(
        "fn root(value: bool) -> i64 {{\n    match value {{\n        {pattern} => 1i64\n    }}\n}}\n"
    )
}

fn entity_reference_pattern_source(pattern: &str) -> String {
    format!(
        "flow @flow.primary primary {{}}\nflow @flow.alternate alternate {{}}\nfn root(value: Ref<Flow>) -> i64 {{\n    match value {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
    )
}

fn whole_binding_pattern_source(pattern: &str) -> String {
    format!(
        "enum Event {{\n    Start,\n    Empty {{}},\n    ChoiceSelected {{ id: i64 }},\n}}\nfn root(event: Event) -> i64 {{\n    match event {{\n        {pattern} => 2i64\n        .Start => 0i64\n        .Empty {{}} => 1i64\n    }}\n}}\n"
    )
}

struct CheckedMatchPatternFact {
    hir_kind: HirPatternKind,
    checked: CheckedPattern,
}

struct CheckedMatchPatternObservation {
    semantic_digest: [u8; 32],
    arm_pattern_digest: [u8; 32],
    scrutinee_type: TypeKind,
    facts: Vec<CheckedMatchPatternFact>,
}

fn checked_match_pattern_observation(
    source: &str,
    arm_ordinal: usize,
) -> CheckedMatchPatternObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("pattern transcript fixture should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let HirExprKind::Match(authored) = module.resolve_expr(owner).expect("Match owner").kind()
    else {
        panic!("selected expression is a Match");
    };
    let arm = authored
        .arms()
        .get(arm_ordinal)
        .expect("selected Match arm");
    let product = super::checked_match_product(&report, project, module, &world.symbols, owner);
    let arm_product = product
        .arms()
        .get(arm_ordinal)
        .expect("checked Match arm product");
    let mut pending = vec![arm.pattern()];
    let mut visited = std::collections::BTreeSet::new();
    let mut facts = Vec::new();
    while let Some(pattern_owner) = pending.pop() {
        if !visited.insert(pattern_owner) {
            continue;
        }
        let hir = module
            .resolve_pattern(pattern_owner)
            .expect("accepted Match pattern owner");
        let checked = report
            .pattern(pattern_owner)
            .expect("checked Match pattern fact");
        pending.extend(hir.kind().child_edges().into_iter().filter_map(
            |edge| match edge.child() {
                HirPatternChild::Pattern(child) => Some(child),
                HirPatternChild::Local(_) | HirPatternChild::Type(_) => None,
            },
        ));
        facts.push(CheckedMatchPatternFact {
            hir_kind: hir.kind().clone(),
            checked: checked.clone(),
        });
    }

    CheckedMatchPatternObservation {
        semantic_digest: *product.semantic_digest().as_bytes(),
        arm_pattern_digest: *arm_product.pattern().as_bytes(),
        scrutinee_type: report
            .expression(authored.scrutinee())
            .and_then(CheckedExpression::value_type)
            .expect("checked Match scrutinee type")
            .clone(),
        facts,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatternCorpusDisposition {
    Accepted,
    RejectOnly,
    #[allow(
        dead_code,
        reason = "no live pattern family has been proven unreachable"
    )]
    ProvenUnreachable,
}

// The inventory and its dispositions come from one list. The `of` matches
// below remain exhaustive against the live HIR and checked owner enums.
macro_rules! pattern_family_inventory {
    ($family:ident { $($variant:ident => $disposition:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        enum $family {
            $($variant),+
        }

        impl $family {
            const INVENTORY: &'static [(Self, PatternCorpusDisposition)] = &[
                $((Self::$variant, PatternCorpusDisposition::$disposition)),+
            ];
        }
    };
}

pattern_family_inventory!(PatternShapeFamily {
    Binding => Accepted,
    MutableBinding => Accepted,
    Literal => Accepted,
    EntityReference => Accepted,
    Variant => Accepted,
    Discard => Accepted,
    Tuple => Accepted,
    Record => Accepted,
    BracketSequence => Accepted,
    WholeBinding => Accepted,
    Or => Accepted,
    TypedBinding => Accepted,
    Error => RejectOnly,
});

impl PatternShapeFamily {
    fn of(kind: &HirPatternKind) -> Self {
        match kind {
            HirPatternKind::Binding(_) => Self::Binding,
            HirPatternKind::MutableBinding(_) => Self::MutableBinding,
            HirPatternKind::Literal(_) => Self::Literal,
            HirPatternKind::EntityReference(_) => Self::EntityReference,
            HirPatternKind::Variant(_) => Self::Variant,
            HirPatternKind::Discard => Self::Discard,
            HirPatternKind::Tuple { .. } => Self::Tuple,
            HirPatternKind::Record { .. } => Self::Record,
            HirPatternKind::BracketSequence { .. } => Self::BracketSequence,
            HirPatternKind::WholeBinding { .. } => Self::WholeBinding,
            HirPatternKind::Or { .. } => Self::Or,
            HirPatternKind::TypedBinding { .. } => Self::TypedBinding,
            HirPatternKind::Error(_) => Self::Error,
        }
    }
}

pattern_family_inventory!(PatternResolutionFamily {
    Structural => Accepted,
    Literal => Accepted,
    Entity => Accepted,
    Record => Accepted,
    Variant => Accepted,
    TypedBinding => Accepted,
});

impl PatternResolutionFamily {
    fn of(resolution: &CheckedPatternResolution) -> Self {
        match resolution {
            CheckedPatternResolution::Structural => Self::Structural,
            CheckedPatternResolution::Literal(_) => Self::Literal,
            CheckedPatternResolution::Entity(_)
            | CheckedPatternResolution::ImportedProjectEntity(_) => Self::Entity,
            CheckedPatternResolution::Record(_) => Self::Record,
            CheckedPatternResolution::Variant(_) => Self::Variant,
            CheckedPatternResolution::TypedBinding(_) => Self::TypedBinding,
        }
    }
}

struct MatchPatternCorpusObservation {
    shapes: BTreeSet<PatternShapeFamily>,
    resolutions: BTreeSet<PatternResolutionFamily>,
}

/// Collect checked pattern facts whose accepted owner path descends from the
/// selected Match expression, including all arms and nested pattern children.
fn accepted_match_pattern_corpus_observation(source: &str) -> MatchPatternCorpusObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("pattern corpus source should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = match_owners.as_slice() else {
        panic!("each pattern corpus row has exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    assert!(
        !product.arms().is_empty(),
        "accepted Match has checked arms"
    );

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let root_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match root path");
    let mut shapes = BTreeSet::new();
    let mut resolutions = BTreeSet::new();
    for (owner, hir) in module.patterns() {
        let Some(checked) = report.pattern(owner) else {
            continue;
        };
        let coordinate = coordinates
            .pattern(owner)
            .expect("checked pattern owner path");
        let path = coordinate.path();
        if path.root() != root_path.root() || !path.steps().starts_with(root_path.steps()) {
            continue;
        }
        shapes.insert(PatternShapeFamily::of(hir.kind()));
        resolutions.insert(PatternResolutionFamily::of(checked.resolution()));
    }
    assert!(
        !shapes.is_empty(),
        "Match root has checked pattern descendants"
    );
    MatchPatternCorpusObservation {
        shapes,
        resolutions,
    }
}

#[test]
fn checked_match_pattern_corpus_tracks_accepted_root_families() {
    struct Row {
        name: &'static str,
        source: String,
        shapes: &'static [PatternShapeFamily],
        resolutions: &'static [PatternResolutionFamily],
    }
    let rows = [
        Row {
            name: "tuple and Or",
            source: tuple_pattern_source("(true | false, true)"),
            shapes: &[
                PatternShapeFamily::Tuple,
                PatternShapeFamily::Or,
                PatternShapeFamily::Literal,
                PatternShapeFamily::Discard,
            ],
            resolutions: &[
                PatternResolutionFamily::Structural,
                PatternResolutionFamily::Literal,
            ],
        },
        Row {
            name: "Result variant and binding",
            source: result_pattern_source("value"),
            shapes: &[PatternShapeFamily::Variant, PatternShapeFamily::Binding],
            resolutions: &[PatternResolutionFamily::Variant],
        },
        Row {
            name: "Choice typed binding",
            source: choice_pattern_source("String", "Bytes"),
            shapes: &[PatternShapeFamily::TypedBinding],
            resolutions: &[PatternResolutionFamily::TypedBinding],
        },
        Row {
            name: "sequence rest",
            source: sequence_pattern_source("[true, ..]"),
            shapes: &[PatternShapeFamily::BracketSequence],
            resolutions: &[PatternResolutionFamily::Structural],
        },
        Row {
            name: "project record variant",
            source: record_variant_pattern_source("i64"),
            shapes: &[PatternShapeFamily::Record, PatternShapeFamily::Variant],
            resolutions: &[
                PatternResolutionFamily::Record,
                PatternResolutionFamily::Variant,
            ],
        },
        Row {
            name: "mutable binding",
            source: mutable_binding_pattern_source("mut selected"),
            shapes: &[PatternShapeFamily::MutableBinding],
            resolutions: &[PatternResolutionFamily::Structural],
        },
        Row {
            name: "checked entity reference",
            source: entity_reference_pattern_source("@flow.primary"),
            shapes: &[
                PatternShapeFamily::EntityReference,
                PatternShapeFamily::Discard,
            ],
            resolutions: &[
                PatternResolutionFamily::Entity,
                PatternResolutionFamily::Structural,
            ],
        },
        Row {
            name: "whole binding",
            source: whole_binding_pattern_source("whole .ChoiceSelected { id }"),
            shapes: &[
                PatternShapeFamily::WholeBinding,
                PatternShapeFamily::Variant,
                PatternShapeFamily::Record,
            ],
            resolutions: &[
                PatternResolutionFamily::Structural,
                PatternResolutionFamily::Variant,
                PatternResolutionFamily::Record,
            ],
        },
    ];
    let mut observed_shapes = BTreeSet::new();
    let mut observed_resolutions = BTreeSet::new();
    for row in rows {
        let observation = accepted_match_pattern_corpus_observation(&row.source);
        for required in row.shapes {
            assert!(
                observation.shapes.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        for required in row.resolutions {
            assert!(
                observation.resolutions.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        observed_shapes.extend(observation.shapes);
        observed_resolutions.extend(observation.resolutions);
    }
    for (family, disposition) in PatternShapeFamily::INVENTORY {
        assert_eq!(
            observed_shapes.contains(family),
            *disposition == PatternCorpusDisposition::Accepted,
            "HIR pattern {family:?} corpus disposition {disposition:?}",
        );
    }
    for (family, disposition) in PatternResolutionFamily::INVENTORY {
        assert_eq!(
            observed_resolutions.contains(family),
            *disposition == PatternCorpusDisposition::Accepted,
            "checked pattern {family:?} corpus disposition {disposition:?}",
        );
    }
}

#[test]
fn checked_match_pattern_corpus_excludes_unrelated_declaration_patterns() {
    let source = format!(
        "fn unrelated() -> bool {{ let outside = true; outside }}\n{}",
        tuple_pattern_source("(true | false, true)")
    );
    let world = super::fixture(&source, None);
    let report = super::analyze(&world).expect("unrelated binding should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    assert!(module.patterns().any(|(owner, pattern)| {
        matches!(pattern.kind(), HirPatternKind::Binding(_)) && report.pattern(owner).is_some()
    }));
    let observation = accepted_match_pattern_corpus_observation(&source);
    assert!(observation.shapes.contains(&PatternShapeFamily::Tuple));
    assert!(
        !observation.shapes.contains(&PatternShapeFamily::Binding),
        "a checked binding outside the Match root must not enter its corpus",
    );
}

#[test]
fn checked_match_pattern_transcript_commits_mutable_entity_and_whole_binding_owners() {
    let mutable =
        checked_match_pattern_observation(&mutable_binding_pattern_source("mut selected"), 0);
    let immutable =
        checked_match_pattern_observation(&mutable_binding_pattern_source("selected"), 0);
    assert_ne!(
        mutable.semantic_digest, immutable.semantic_digest,
        "mutable binding mode reaches the Match transcript",
    );
    assert!(mutable.facts.iter().any(|fact| matches!(
        (&fact.hir_kind, fact.checked.resolution()),
        (
            HirPatternKind::MutableBinding(_),
            CheckedPatternResolution::Structural
        )
    )));

    let primary =
        checked_match_pattern_observation(&entity_reference_pattern_source("@flow.primary"), 0);
    let alternate =
        checked_match_pattern_observation(&entity_reference_pattern_source("@flow.alternate"), 0);
    let checked_entity = |observation: &CheckedMatchPatternObservation| {
        observation
            .facts
            .iter()
            .find_map(|fact| match (&fact.hir_kind, fact.checked.resolution()) {
                (HirPatternKind::EntityReference(_), CheckedPatternResolution::Entity(item)) => {
                    Some(item.semantic_id())
                }
                _ => None,
            })
            .expect("entity pattern resolves to an accepted project item")
    };
    assert_ne!(checked_entity(&primary), checked_entity(&alternate));
    assert_ne!(
        primary.semantic_digest, alternate.semantic_digest,
        "checked entity target meaning reaches the Match transcript",
    );

    let whole = checked_match_pattern_observation(
        &whole_binding_pattern_source("whole .ChoiceSelected { id }"),
        0,
    );
    let variant = checked_match_pattern_observation(
        &whole_binding_pattern_source(".ChoiceSelected { id }"),
        0,
    );
    assert_ne!(
        whole.semantic_digest, variant.semantic_digest,
        "whole-pattern binding reaches the Match transcript",
    );
    assert!(whole.facts.iter().any(|fact| matches!(
        (&fact.hir_kind, fact.checked.resolution()),
        (
            HirPatternKind::WholeBinding { .. },
            CheckedPatternResolution::Structural
        )
    )));
}

#[test]
fn checked_match_pattern_transcript_distinguishes_exact_and_rest_sequences() {
    assert_ne!(
        first_arm_pattern_digest(&sequence_pattern_source("[true]")),
        first_arm_pattern_digest(&sequence_pattern_source("[true, ..]")),
    );
    assert_ne!(
        first_arm_pattern_digest(&sequence_pattern_source("[]")),
        first_arm_pattern_digest(&sequence_pattern_source("[..]")),
    );
}

#[test]
fn checked_match_pattern_transcript_carries_tuple_and_or_targets() {
    let tuple_or =
        checked_match_pattern_observation(&tuple_pattern_source("(true | false, true)"), 0);
    let changed =
        checked_match_pattern_observation(&tuple_pattern_source("(true | false, false)"), 0);

    assert_ne!(
        tuple_or.semantic_digest, changed.semantic_digest,
        "a checked tuple element change reaches the Match digest",
    );
    assert!(matches!(
        &tuple_or.scrutinee_type,
        TypeKind::Tuple(elements)
            if elements.as_slice() == [TypeKind::Bool, TypeKind::Bool]
    ));
    assert!(tuple_or.facts.iter().any(|fact| matches!(
        &fact.hir_kind,
        HirPatternKind::Tuple { .. }
    ) && matches!(
        fact.checked.resolution(),
        CheckedPatternResolution::Structural
    ) && matches!(fact.checked.ty(), TypeKind::Tuple(_))));
    assert!(
        tuple_or
            .facts
            .iter()
            .any(|fact| matches!(&fact.hir_kind, HirPatternKind::Or { .. })
                && matches!(
                    fact.checked.resolution(),
                    CheckedPatternResolution::Structural
                )
                && matches!(fact.checked.ty(), TypeKind::Bool))
    );
}

#[test]
fn checked_match_pattern_transcript_carries_result_and_choice_targets() {
    let result = checked_match_pattern_observation(&result_pattern_source("value"), 0);
    let error = checked_match_pattern_observation(&result_pattern_source("value"), 1);
    let changed_result = checked_match_pattern_observation(&result_pattern_source("_"), 0);
    assert_ne!(
        result.semantic_digest, changed_result.semantic_digest,
        "a Result payload binding change reaches the Match digest",
    );
    let selected_result_cases = result
        .facts
        .iter()
        .chain(&error.facts)
        .filter_map(|fact| match (&fact.hir_kind, fact.checked.resolution()) {
            (HirPatternKind::Variant(_), CheckedPatternResolution::Variant(variant))
                if matches!(
                    variant.owner().kind(),
                    CheckedVariantOwnerKind::Result { ok, error }
                        if ok == &TypeKind::Bool && error == &TypeKind::String
                ) =>
            {
                variant.selected().diagnostic_name().map(str::to_owned)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        selected_result_cases,
        ["Err".to_owned(), "Ok".to_owned()].into_iter().collect()
    );

    let choice = checked_match_pattern_observation(&choice_pattern_source("String", "Bytes"), 0);
    let changed_choice =
        checked_match_pattern_observation(&choice_pattern_source("Bytes", "String"), 0);
    assert_ne!(
        choice.semantic_digest, changed_choice.semantic_digest,
        "checked Choice alternative selection reaches the Match digest",
    );
    assert!(matches!(
        &choice.scrutinee_type,
        TypeKind::Choice(alternatives) if alternatives.len() == 2
    ));
    let typed_binding = choice
        .facts
        .iter()
        .find_map(|fact| match (&fact.hir_kind, fact.checked.resolution()) {
            (
                HirPatternKind::TypedBinding { .. },
                CheckedPatternResolution::TypedBinding(binding),
            ) => Some(binding),
            _ => None,
        })
        .expect("checked Choice typed binding reaches the Match arm");
    assert_eq!(typed_binding.annotation(), &TypeKind::String);
    assert_eq!(typed_binding.choice_alternatives().len(), 1);
}

#[test]
fn checked_match_pattern_transcript_carries_sequence_target_and_rest_mode() {
    let source = |first: &str| {
        format!(
            "fn root(items: Vec<bool>) -> i64 {{\n    match items {{\n        [{first}, ..] => 1i64\n        _ => 0i64\n    }}\n}}\n"
        )
    };
    let sequence = checked_match_pattern_observation(&source("true"), 0);
    let changed = checked_match_pattern_observation(&source("false"), 0);

    assert_ne!(
        sequence.semantic_digest, changed.semantic_digest,
        "a checked sequence element change reaches the Match digest",
    );
    assert!(matches!(
        &sequence.scrutinee_type,
        TypeKind::Vec(item) if item.as_ref() == &TypeKind::Bool
    ));
    assert!(sequence.facts.iter().any(|fact| matches!(
        &fact.hir_kind,
        HirPatternKind::BracketSequence {
            rest: HirPatternSequenceRest::Unbound,
            ..
        }
    ) && matches!(
        fact.checked.resolution(),
        CheckedPatternResolution::Structural
    ) && matches!(
        fact.checked.ty(),
        TypeKind::Vec(item) if item.as_ref() == &TypeKind::Bool
    )));
}

#[test]
fn checked_match_pattern_transcript_carries_project_record_variant_owner() {
    let record_variant =
        checked_match_pattern_observation(&record_variant_pattern_source("i64"), 2);
    let changed = checked_match_pattern_observation(&record_variant_pattern_source("i32"), 2);

    assert_ne!(
        record_variant.semantic_digest, changed.semantic_digest,
        "the project record-variant payload schema reaches the Match digest",
    );
    let carries_payload_type = |observation: &CheckedMatchPatternObservation,
                                expected: &TypeKind| {
        observation.facts.iter().any(|fact| {
            matches!(
                (&fact.hir_kind, fact.checked.resolution()),
                (HirPatternKind::Record { .. }, CheckedPatternResolution::Record(record))
                    if matches!(
                        record.owner(),
                        CheckedRecordPatternOwner::VariantPayload { .. }
                    ) && record.fields().len() == 1
                        && record.fields()[0].field_type() == expected
            )
        })
    };
    assert!(carries_payload_type(&record_variant, &TypeKind::I64));
    assert!(carries_payload_type(&changed, &TypeKind::I32));
}
