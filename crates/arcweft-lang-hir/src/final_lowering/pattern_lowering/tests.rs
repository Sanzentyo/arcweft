use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::node::{FunctionBodyKind, LetStatementKind};
use arcweft_lang_syntax::attachment::{AttachedPatternNode, DeclarationBodyNode, StatementNode};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_lang_syntax::literal::{
    SyntaxCharacterIssue, SyntaxDecimalComponentIssue, SyntaxDecimalIssue, SyntaxDurationIssue,
    SyntaxIntegerIssue, SyntaxLiteralIssue, SyntaxStringIssue, SyntaxUnitNumberIssue,
};
use arcweft_lang_syntax::patterns::{
    PatternComponentRole, PatternOrBindingIssue, PatternRecoveryIssue,
};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::*;
use crate::database::HirDatabase;
use crate::identity::HirLimit;
use crate::leaf::{
    HirCharacterIssue, HirCharacterLiteral, HirDecimalIssue, HirDurationIssue, HirDurationLiteral,
    HirFloatIssue, HirFloatLiteral, HirIntegerIssue, HirIntegerLiteral, HirLiteral, HirStringIssue,
    HirStringLiteral, HirUnitNumberIssue, HirUnitNumberLiteral,
};
use crate::lowering::{HirModuleKey, LoweringRequest};
use crate::module::HirModule;
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::HirOrigin;
use crate::symbol::CallablePackageId;

fn parsed_source(document_id: &str, patterns: &[&str]) -> ParsedSource {
    let name = SourceName::path(format!("proof/pattern-lowering/{document_id}.arcw"));
    let statements = patterns
        .iter()
        .map(|pattern| format!("    let {pattern} = source_value;"))
        .collect::<Vec<_>>()
        .join("\n");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/pattern-lowering/{document_id}.arcw"
            ))
            .expect("pattern-lowering document ID"),
            name.clone(),
            format!("fn lower_patterns() {{\n{statements}\n}}\n"),
        )
        .expect("pattern-lowering source"),
    );
    SyntaxDatabase::try_new()
        .expect("pattern-lowering syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached Pattern source parses")
}

fn statements(parsed: &ParsedSource) -> Vec<StatementNode> {
    let item = parsed
        .items()
        .expect("source item inventory")
        .into_iter()
        .next()
        .expect("function item");
    let Some(DeclarationBodyNode::Body(body)) = item.body().expect("function body access") else {
        panic!("test function must retain an authored body");
    };
    body.cast::<FunctionBodyKind>()
        .expect("function body family")
        .block()
        .expect("function computation block")
        .statements()
        .expect("function statement inventory")
}

fn attached_patterns(parsed: &ParsedSource) -> Vec<AttachedPatternNode> {
    statements(parsed)
        .into_iter()
        .map(|statement| {
            statement
                .cast::<LetStatementKind>()
                .expect("let statement family")
                .pattern()
                .expect("let Pattern")
                .semantic()
                .expect("attached semantic Pattern")
        })
        .collect()
}

fn module_key(parsed: &ParsedSource) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-pattern-lowering-tests").expect("package ID"),
        CanonicalModulePath::crate_root(),
        parsed.document().identity().clone(),
    )
}

fn stage<'source>(
    database: &HirDatabase,
    parsed: &'source ParsedSource,
) -> StagedHirModuleTransaction<'source> {
    super::super::stage_unpublished_module_for_invariant_test(
        database,
        LoweringRequest::try_new(module_key(parsed), parsed).expect("lowering request"),
        crate::lowering::HirLoweringControl::new(),
    )
    .expect("staged HIR module")
}

fn allocate_module_scope(
    transaction: &mut StagedHirModuleTransaction<'_>,
    parsed: &ParsedSource,
) -> ScopeId {
    let module = transaction.snapshot_id().module();
    let root = parsed.root_syntax();
    let site = HirSourceSite::Span(root.source_span().clone());
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .scopes()
        .allocate_source(
            slots,
            root.id(),
            site,
            HirScope::try_new(
                module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(module),
                Box::new([]),
                Box::new([]),
            )
            .expect("module scope"),
        )
        .expect("module scope allocation")
}

fn lower_and_publish(
    parsed: &ParsedSource,
) -> (Arc<HirModule>, Vec<PatternId>, Vec<AttachedPatternNode>) {
    let attached = attached_patterns(parsed);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    let owners = attached
        .iter()
        .map(|pattern| {
            transaction
                .lower_attached_pattern(pattern, scope)
                .expect("attached Pattern lowering")
        })
        .collect::<Vec<_>>();
    close_pattern_scope_members(&mut transaction, scope, &owners);
    let module = transaction
        .finish(&mut database)
        .expect("Pattern module publication")
        .into_module();
    (module, owners, attached)
}

fn close_pattern_scope_members(
    transaction: &mut StagedHirModuleTransaction<'_>,
    scope: ScopeId,
    owners: &[PatternId],
) {
    let locals = owners
        .iter()
        .flat_map(|owner| {
            transaction
                .pattern_locals
                .get(owner)
                .expect("staged Pattern Local inventory")
                .iter()
                .copied()
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    transaction
        .close_scope_members(scope, locals)
        .expect("close Pattern fixture scope");
}

fn pattern(module: &HirModule, owner: PatternId) -> &HirPattern {
    module
        .arenas()
        .patterns()
        .resolve(module.slots(), owner)
        .expect("published Pattern")
}

fn local(module: &HirModule, owner: LocalId) -> &HirLocal {
    module
        .arenas()
        .locals()
        .resolve(module.slots(), owner)
        .expect("published Local")
}

fn attached_pattern_child(
    owner: &AttachedPatternNode,
    step: PatternNodeStep,
) -> AttachedPatternNode {
    owner
        .children()
        .expect("attached Pattern children")
        .into_iter()
        .find_map(|child| match child {
            AttachedPatternChild::Pattern { step: actual, node } if actual == step => Some(node),
            AttachedPatternChild::Pattern { .. } | AttachedPatternChild::Type { .. } => None,
        })
        .expect("required attached Pattern child")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NestedFixtureLocals {
    ordinary: [LocalId; 7],
    rest: LocalId,
    record_owner: PatternId,
    patterns: [PatternId; 11],
}

fn bound_local(binding: &HirPatternBinding) -> LocalId {
    let HirPatternBinding::Bound { local, .. } = binding else {
        panic!("fixture binding must own a Local");
    };
    *local
}

fn nested_fixture_locals_with<'a>(
    owner: PatternId,
    resolve: impl Fn(PatternId) -> &'a HirPattern,
) -> NestedFixtureLocals {
    let HirPatternKind::Tuple { elements } = resolve(owner).kind() else {
        panic!("fixture root must be a tuple");
    };
    let [a_pattern, record_owner, whole_owner] = elements.as_ref() else {
        panic!("fixture tuple must have three elements");
    };
    let HirPatternKind::Binding(binding_a) = resolve(*a_pattern).kind() else {
        panic!("fixture a must be a binding");
    };
    let HirPatternKind::Record { fields, .. } = resolve(*record_owner).kind() else {
        panic!("fixture second element must be a record");
    };
    let [
        HirPatternField::Explicit {
            pattern: b_pattern, ..
        },
        HirPatternField::Explicit {
            pattern: cd_owner, ..
        },
        HirPatternField::Rest {
            binding: Some(rest),
        },
    ] = fields.as_ref()
    else {
        panic!("fixture record must preserve two explicit fields and one rest");
    };
    let HirPatternKind::Binding(binding_b) = resolve(*b_pattern).kind() else {
        panic!("fixture b must be a binding");
    };
    let HirPatternKind::Tuple { elements } = resolve(*cd_owner).kind() else {
        panic!("fixture c/d owner must be a tuple");
    };
    let [c_pattern, d_pattern] = elements.as_ref() else {
        panic!("fixture c/d tuple shape");
    };
    let HirPatternKind::Binding(binding_c) = resolve(*c_pattern).kind() else {
        panic!("fixture c must be a binding");
    };
    let HirPatternKind::Binding(binding_d) = resolve(*d_pattern).kind() else {
        panic!("fixture d must be a binding");
    };
    let HirPatternKind::WholeBinding {
        binding: binding_e,
        pattern: fg_owner,
    } = resolve(*whole_owner).kind()
    else {
        panic!("fixture e must own the nested tuple");
    };
    let HirPatternKind::Tuple { elements } = resolve(*fg_owner).kind() else {
        panic!("fixture f/g owner must be a tuple");
    };
    let [f_pattern, g_pattern] = elements.as_ref() else {
        panic!("fixture f/g tuple shape");
    };
    let HirPatternKind::Binding(binding_f) = resolve(*f_pattern).kind() else {
        panic!("fixture f must be a binding");
    };
    let HirPatternKind::Binding(binding_g) = resolve(*g_pattern).kind() else {
        panic!("fixture g must be a binding");
    };

    NestedFixtureLocals {
        ordinary: [
            bound_local(binding_a),
            bound_local(binding_b),
            bound_local(binding_c),
            bound_local(binding_d),
            bound_local(binding_e),
            bound_local(binding_f),
            bound_local(binding_g),
        ],
        rest: *rest,
        record_owner: *record_owner,
        patterns: [
            owner,
            *a_pattern,
            *record_owner,
            *b_pattern,
            *cd_owner,
            *c_pattern,
            *d_pattern,
            *whole_owner,
            *fg_owner,
            *f_pattern,
            *g_pattern,
        ],
    }
}

fn nested_fixture_locals(module: &HirModule, owner: PatternId) -> NestedFixtureLocals {
    nested_fixture_locals_with(owner, |pattern_id| pattern(module, pattern_id))
}

fn staged_nested_fixture_identity(
    transaction: &StagedHirModuleTransaction<'_>,
    owner: PatternId,
) -> (NestedFixtureLocals, Vec<(LocalId, SyntheticKey)>) {
    let locals = nested_fixture_locals_with(owner, |pattern_id| {
        transaction
            .arenas
            .patterns
            .resolve_staged(&transaction.slots, pattern_id)
            .expect("staged fixture Pattern")
    });
    let identities = locals
        .ordinary
        .into_iter()
        .chain([locals.rest])
        .map(|local_id| {
            let metadata = transaction
                .slots
                .resolve_staged(local_id)
                .expect("staged fixture Local metadata");
            let HirOrigin::Synthetic(key) = metadata.origin() else {
                panic!("staged fixture Local must be synthetic");
            };
            (local_id, *key)
        })
        .collect();
    (locals, identities)
}

fn nested_fixture_attached_nodes(
    owner: &AttachedPatternNode,
) -> ([AttachedPatternNode; 7], AttachedPatternNode) {
    let binding_a = attached_pattern_child(owner, PatternNodeStep::Element(0));
    let record = attached_pattern_child(owner, PatternNodeStep::Element(1));
    let binding_b = attached_pattern_child(&record, PatternNodeStep::RecordField(0));
    let tuple_cd = attached_pattern_child(&record, PatternNodeStep::RecordField(1));
    let binding_c = attached_pattern_child(&tuple_cd, PatternNodeStep::Element(0));
    let binding_d = attached_pattern_child(&tuple_cd, PatternNodeStep::Element(1));
    let binding_e = attached_pattern_child(owner, PatternNodeStep::Element(2));
    let tuple_fg = attached_pattern_child(&binding_e, PatternNodeStep::NestedPattern);
    let binding_f = attached_pattern_child(&tuple_fg, PatternNodeStep::Element(0));
    let binding_g = attached_pattern_child(&tuple_fg, PatternNodeStep::Element(1));
    (
        [
            binding_a, binding_b, binding_c, binding_d, binding_e, binding_f, binding_g,
        ],
        record,
    )
}

type NestedFixtureEvidence = (Box<str>, u32, SyntheticRole, u32, usize);

fn assert_nested_fixture_evidence(
    module: &HirModule,
    structure_owner: PatternId,
    synthetic_owner: PatternId,
    attached: &AttachedPatternNode,
) -> (NestedFixtureLocals, Vec<NestedFixtureEvidence>) {
    let locals = nested_fixture_locals(module, structure_owner);
    let (attached_bindings, attached_record) = nested_fixture_attached_nodes(attached);
    let expected_names = ["a", "b", "c", "d", "e", "f", "g"];
    let mut evidence = Vec::new();
    for (ordinal, ((local_id, attached), expected_name)) in locals
        .ordinary
        .iter()
        .zip(&attached_bindings)
        .zip(expected_names)
        .enumerate()
    {
        let payload = local(module, *local_id);
        assert_eq!(payload.name().as_str(), expected_name);
        assert_eq!(payload.generation(), LocalGeneration::FIRST);
        assert_eq!(payload.pattern(), Some(synthetic_owner));
        assert!(!payload.is_poisoned());

        let metadata = module
            .slots()
            .resolve(*local_id)
            .expect("fixture Local metadata");
        let HirOrigin::Synthetic(key) = metadata.origin() else {
            panic!("fixture Local must be synthetic");
        };
        assert_eq!(key.owner(), SyntheticOwner::Pattern(synthetic_owner));
        assert_eq!(key.role(), SyntheticRole::DestructuredBinding);
        assert_eq!(
            key.ordinal(),
            u32::try_from(ordinal).expect("fixture ordinal")
        );
        let HirSourceSite::Span(binding_span) = metadata.source_site() else {
            panic!("fixture binding must retain its exact authored name span");
        };
        let component = if expected_name == "e" {
            PatternComponentRole::WholeBindingName
        } else {
            PatternComponentRole::Name
        };
        let expected = attached
            .component(component)
            .expect("fixture binding-name component")
            .range();
        assert_eq!(binding_span.range(), expected);
        evidence.push((
            expected_name.into(),
            payload.generation().get(),
            key.role(),
            key.ordinal(),
            binding_span.range().start(),
        ));
    }

    let rest_payload = local(module, locals.rest);
    assert_eq!(rest_payload.name().as_str(), "rest");
    assert_eq!(rest_payload.generation(), LocalGeneration::FIRST);
    assert_eq!(rest_payload.pattern(), Some(locals.record_owner));
    let rest_metadata = module
        .slots()
        .resolve(locals.rest)
        .expect("fixture rest metadata");
    let HirOrigin::Synthetic(rest_key) = rest_metadata.origin() else {
        panic!("fixture rest Local must be synthetic");
    };
    assert_eq!(
        rest_key.owner(),
        SyntheticOwner::Pattern(locals.record_owner)
    );
    assert_eq!(rest_key.role(), SyntheticRole::PatternRest);
    assert_eq!(rest_key.ordinal(), 0);
    let HirSourceSite::Span(rest_span) = rest_metadata.source_site() else {
        panic!("fixture rest must retain its exact authored binding span");
    };
    let rest_binding_start = attached_record
        .component(PatternComponentRole::PatternField {
            field: 2,
            part: PatternFieldPart::RestBinding,
        })
        .expect("fixture rest binding-name component")
        .range()
        .start();
    let rest_binding = attached_record
        .component(PatternComponentRole::PatternField {
            field: 2,
            part: PatternFieldPart::RestBinding,
        })
        .expect("fixture rest binding-name component")
        .range();
    assert_eq!(rest_span.range(), rest_binding);
    evidence.push((
        "rest".into(),
        rest_payload.generation().get(),
        rest_key.role(),
        rest_key.ordinal(),
        rest_binding_start,
    ));
    (locals, evidence)
}

fn small_magnitude(value: &crate::leaf::HirBigUint) -> u128 {
    value
        .limbs_le()
        .iter()
        .rev()
        .fold(0_u128, |result, limb| (result << 32) | u128::from(*limb))
}

#[test]
#[allow(clippy::too_many_lines)]
fn literal_recovery_projection_is_exhaustive_by_typed_family() {
    let attempted = || Box::<str>::from("bad");
    let cases = vec![
        (
            SyntaxLiteralIssue::String(SyntaxStringIssue::InvalidEscape {
                attempted: attempted(),
            }),
            HirLiteral::String(HirStringLiteral::Invalid(HirStringIssue::InvalidEscape)),
        ),
        (
            SyntaxLiteralIssue::String(SyntaxStringIssue::Unterminated {
                attempted: attempted(),
            }),
            HirLiteral::String(HirStringLiteral::Invalid(HirStringIssue::Unterminated)),
        ),
        (
            SyntaxLiteralIssue::Character(SyntaxCharacterIssue::InvalidEscape {
                attempted: attempted(),
            }),
            HirLiteral::Character(HirCharacterLiteral::Invalid(
                HirCharacterIssue::InvalidEscape,
            )),
        ),
        (
            SyntaxLiteralIssue::Character(SyntaxCharacterIssue::Unterminated {
                attempted: attempted(),
            }),
            HirLiteral::Character(HirCharacterLiteral::Invalid(
                HirCharacterIssue::Unterminated,
            )),
        ),
        (
            SyntaxLiteralIssue::Character(SyntaxCharacterIssue::Empty {
                attempted: attempted(),
            }),
            HirLiteral::Character(HirCharacterLiteral::Invalid(HirCharacterIssue::Empty)),
        ),
        (
            SyntaxLiteralIssue::Character(SyntaxCharacterIssue::MultipleScalars {
                attempted: attempted(),
            }),
            HirLiteral::Character(HirCharacterLiteral::Invalid(
                HirCharacterIssue::MultipleScalars,
            )),
        ),
        (
            SyntaxLiteralIssue::Integer(SyntaxIntegerIssue::MissingDigits {
                attempted: attempted(),
            }),
            HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::MissingDigits)),
        ),
        (
            SyntaxLiteralIssue::Integer(SyntaxIntegerIssue::InvalidDigits {
                attempted: attempted(),
            }),
            HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::InvalidDigit)),
        ),
        (
            SyntaxLiteralIssue::Integer(SyntaxIntegerIssue::InvalidSeparator {
                attempted: attempted(),
            }),
            HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::InvalidDigit)),
        ),
        (
            SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                SyntaxDecimalComponentIssue::MissingCoefficient {
                    attempted: attempted(),
                },
            )),
            HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::Decimal(
                HirDecimalIssue::MissingCoefficient,
            ))),
        ),
        (
            SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidDigits {
                    attempted: attempted(),
                },
            )),
            HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::Decimal(
                HirDecimalIssue::InvalidDigit,
            ))),
        ),
        (
            SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidSeparator {
                    attempted: attempted(),
                },
            )),
            HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::Decimal(
                HirDecimalIssue::InvalidDigit,
            ))),
        ),
        (
            SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::InvalidSuffix {
                suffix: attempted(),
            }),
            HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::InvalidSuffix)),
        ),
    ];

    for (issue, expected) in cases {
        assert_eq!(
            super::super::literal_projection::invalid_literal(&issue),
            expected
        );
    }

    for issue in [
        SyntaxDecimalComponentIssue::MissingCoefficient {
            attempted: attempted(),
        },
        SyntaxDecimalComponentIssue::InvalidDigits {
            attempted: attempted(),
        },
        SyntaxDecimalComponentIssue::InvalidSeparator {
            attempted: attempted(),
        },
    ] {
        let expected = match &issue {
            SyntaxDecimalComponentIssue::MissingCoefficient { .. } => {
                HirDecimalIssue::MissingCoefficient
            }
            SyntaxDecimalComponentIssue::InvalidDigits { .. }
            | SyntaxDecimalComponentIssue::InvalidSeparator { .. } => HirDecimalIssue::InvalidDigit,
        };
        assert_eq!(
            super::super::literal_projection::invalid_literal(&SyntaxLiteralIssue::UnitNumber(
                SyntaxUnitNumberIssue::Decimal(issue.clone()),
            )),
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(HirUnitNumberIssue::Decimal(
                expected
            ),))
        );
        assert_eq!(
            super::super::literal_projection::invalid_literal(&SyntaxLiteralIssue::Duration(
                SyntaxDurationIssue::Decimal(issue),
            )),
            HirLiteral::Duration(HirDurationLiteral::Invalid(HirDurationIssue::Decimal(
                expected,
            )))
        );
    }

    assert_eq!(
        super::super::literal_projection::invalid_literal(&SyntaxLiteralIssue::UnitNumber(
            SyntaxUnitNumberIssue::InvalidUnit { unit: attempted() },
        )),
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(
            HirUnitNumberIssue::InvalidUnit,
        ))
    );
    assert_eq!(
        super::super::literal_projection::invalid_literal(&SyntaxLiteralIssue::Duration(
            SyntaxDurationIssue::InvalidUnit { unit: attempted() },
        )),
        HirLiteral::Duration(HirDurationLiteral::Invalid(HirDurationIssue::InvalidUnit,))
    );
}

#[test]
fn fractional_nanosecond_duration_commits_typed_pattern_poison() {
    let parsed = parsed_source("fractional-duration", &["0.1ns"]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert!(matches!(
        pattern(&module, owners[0]).kind(),
        HirPatternKind::Literal(HirLiteral::Duration(HirDurationLiteral::Invalid(
            HirDurationIssue::FractionalNanosecond
        )))
    ));
    assert!(!module.is_executable());
    assert_eq!(
        module
            .diagnostics()
            .iter()
            .filter(|diagnostic| matches!(
                diagnostic,
                crate::diagnostic::HirDiagnostic::Recovery(_)
            ))
            .count(),
        1
    );
}

#[test]
fn every_duration_unit_lowers_to_exact_whole_nanoseconds() {
    let parsed = parsed_source(
        "duration-units",
        &["1ns", "1us", "1ms", "1s", "1min", "1h", "0.1us"],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let expected = [
        1_u128,
        1_000,
        1_000_000,
        1_000_000_000,
        60_000_000_000,
        3_600_000_000_000,
        100,
    ];
    for (owner, expected) in owners.iter().zip(expected) {
        let HirPatternKind::Literal(HirLiteral::Duration(HirDurationLiteral::Value(value))) =
            pattern(&module, *owner).kind()
        else {
            panic!("expected valid Duration Pattern");
        };
        assert_eq!(
            small_magnitude(value.semantic_value().nanoseconds()),
            expected
        );
    }
}

#[test]
fn maximum_duration_decimal_power_uses_bounded_bigint_scaling() {
    let parsed = parsed_source("duration-power-boundary", &["1e1000000ns"]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let HirPatternKind::Literal(HirLiteral::Duration(HirDurationLiteral::Value(value))) =
        pattern(&module, owners[0]).kind()
    else {
        panic!("expected maximum-power Duration Pattern");
    };
    assert!(
        value.semantic_value().nanoseconds().limbs_le().len() > 100_000,
        "10^1,000,000 must retain its complete arbitrary-width magnitude"
    );
}

#[test]
fn integer_radices_share_exact_base_two_limbs() {
    let parsed = parsed_source(
        "integer-radices",
        &[
            "4294967296",
            "0x1_0000_0000",
            "0o40000000000",
            "0b1_00000000_00000000_00000000_00000000",
        ],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let magnitudes = owners
        .iter()
        .map(|owner| {
            let HirPatternKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
                magnitude,
                ..
            })) = pattern(&module, *owner).kind()
            else {
                panic!("expected integer Pattern");
            };
            magnitude
        })
        .collect::<Vec<_>>();
    assert!(magnitudes.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(magnitudes[0].limbs_le(), [0, 1]);
}

#[test]
fn maximum_decimal_integer_digits_commit_without_host_narrowing() {
    let literal = "9".repeat(HirLimit::NumericDigitsPerLiteral.maximum());
    let parsed = parsed_source("integer-digits-exact", &[&literal]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let HirPatternKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
        magnitude, ..
    })) = pattern(&module, owners[0]).kind()
    else {
        panic!("expected maximum-digit Integer Pattern");
    };
    assert!(magnitude.limbs_le().len() > 6_000);
}

#[test]
fn equivalent_decimal_spellings_share_one_canonical_payload() {
    let parsed = parsed_source("decimal-canonical", &["100.0", "1e2", "1.00e2"]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let literals = owners
        .iter()
        .map(|owner| {
            let HirPatternKind::Literal(literal) = pattern(&module, *owner).kind() else {
                panic!("expected literal Pattern");
            };
            literal
        })
        .collect::<Vec<_>>();
    assert_eq!(literals[0], literals[1]);
    assert_eq!(literals[1], literals[2]);
    let HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }) = literals[1] else {
        panic!("expected canonical decimal float");
    };
    assert_eq!(decimal.coefficient().digits(), [1]);
    assert_eq!(decimal.scale(), 0);
    assert_eq!(decimal.exponent10(), 2);
}

#[test]
fn decimal_authored_limits_preflight_before_zero_tail_canonicalization() {
    let exact_literal = format!("1.{}", "0".repeat(65_535));
    let exact = parsed_source("decimal-zero-tail-exact", &[&exact_literal]);
    let (module, owners, _) = lower_and_publish(&exact);
    let HirPatternKind::Literal(HirLiteral::Float(HirFloatLiteral::Value { decimal, .. })) =
        pattern(&module, owners[0]).kind()
    else {
        panic!("expected exact-boundary decimal");
    };
    assert_eq!(decimal.coefficient().digits(), [1]);
    assert_eq!(decimal.scale(), 0);

    let over_literal = format!("1.{}", "0".repeat(65_536));
    let over = parsed_source("decimal-zero-tail-over", &[&over_literal]);
    let attached = attached_patterns(&over);
    let database = HirDatabase::try_new().expect("HIR database");
    let key = module_key(&over);
    let mut transaction = stage(&database, &over);
    let scope = allocate_module_scope(&mut transaction, &over);
    let error = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect_err("one-over typed numeric digit count must fail before canonicalization");
    let HirLowerFailure::Limit(error) = error else {
        panic!("expected numeric digit limit, got {error:?}");
    };
    assert_eq!(error.limit(), HirLimit::NumericDigitsPerLiteral);
    assert_eq!(error.observed(), 65_537);
    drop(transaction);
    assert!(database.current(&key).is_none());
}

#[test]
fn authored_decimal_exponent_exact_and_one_over_are_checked_before_canonicalization() {
    let exact = parsed_source("decimal-exponent-exact", &["1e1000000"]);
    let (module, owners, _) = lower_and_publish(&exact);
    let HirPatternKind::Literal(HirLiteral::Float(HirFloatLiteral::Value { decimal, .. })) =
        pattern(&module, owners[0]).kind()
    else {
        panic!("expected exact exponent decimal");
    };
    assert_eq!(decimal.exponent10(), 1_000_000);

    let over = parsed_source("decimal-exponent-over", &["1e1000001"]);
    let attached = attached_patterns(&over);
    let database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &over);
    let scope = allocate_module_scope(&mut transaction, &over);
    let error = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect_err("one-over authored exponent must fail");
    let HirLowerFailure::Limit(error) = error else {
        panic!("expected decimal exponent limit, got {error:?}");
    };
    assert_eq!(error.limit(), HirLimit::DecimalExponentAbs);
    assert_eq!(error.observed(), 1_000_001);
}

#[test]
fn canonical_decimal_exponent_rechecks_integral_zero_reduction() {
    let exact = parsed_source("canonical-exponent-exact", &["10e999999"]);
    let (module, owners, _) = lower_and_publish(&exact);
    let HirPatternKind::Literal(HirLiteral::Float(HirFloatLiteral::Value { decimal, .. })) =
        pattern(&module, owners[0]).kind()
    else {
        panic!("expected exact canonical exponent decimal");
    };
    assert_eq!(decimal.coefficient().digits(), [1]);
    assert_eq!(decimal.exponent10(), 1_000_000);

    let over = parsed_source("canonical-exponent-over", &["100e999999"]);
    let attached = attached_patterns(&over);
    let database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &over);
    let scope = allocate_module_scope(&mut transaction, &over);
    let error = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect_err("canonical exponent one-over must fail");
    let HirLowerFailure::Limit(error) = error else {
        panic!("expected canonical exponent limit, got {error:?}");
    };
    assert_eq!(error.limit(), HirLimit::DecimalExponentAbs);
    assert_eq!(error.observed(), 1_000_001);
}

#[test]
#[allow(clippy::too_many_lines)]
fn exact_nested_binding_fixture_is_stable_and_or_reuses_every_local() {
    const FIXTURE: &str = "(a, {left: b, right: (c, d), ..rest}, e (f, g))";

    let parsed = parsed_source("nested-binding-generator", &[FIXTURE]);
    let (module, owners, attached) = lower_and_publish(&parsed);
    let (_, normal_evidence) =
        assert_nested_fixture_evidence(&module, owners[0], owners[0], &attached[0]);
    assert_eq!(
        module
            .arenas()
            .locals()
            .try_iter(module.slots())
            .expect("nested fixture Local arena")
            .len(),
        8
    );

    let mut perturb_database = HirDatabase::try_new().expect("perturbation HIR database");
    let perturb_key = module_key(&parsed);
    let (normal_snapshot, normal_scope, normal_owner, normal_staged_locals, normal_staged_identity) = {
        let mut normal_transaction = stage(&perturb_database, &parsed);
        let normal_snapshot = normal_transaction.snapshot_id();
        let normal_scope = allocate_module_scope(&mut normal_transaction, &parsed);
        let normal_owner = normal_transaction
            .lower_attached_pattern(&attached[0], normal_scope)
            .expect("normal staged fixture lowering");
        let (normal_staged_locals, normal_staged_identity) =
            staged_nested_fixture_identity(&normal_transaction, normal_owner);
        (
            normal_snapshot,
            normal_scope,
            normal_owner,
            normal_staged_locals,
            normal_staged_identity,
        )
    };
    assert!(
        perturb_database.current(&perturb_key).is_none(),
        "unpublished normal perturbation transaction must leave no module"
    );

    let mut reversed_transaction = stage(&perturb_database, &parsed);
    assert_eq!(reversed_transaction.snapshot_id(), normal_snapshot);
    reversed_transaction.reverse_pattern_child_insertion_for_test();
    let reversed_scope = allocate_module_scope(&mut reversed_transaction, &parsed);
    assert_eq!(reversed_scope, normal_scope);
    let reversed_owner = reversed_transaction
        .lower_attached_pattern(&attached[0], reversed_scope)
        .expect("reversed-child staged fixture lowering");
    let (reversed_staged_locals, reversed_staged_identity) =
        staged_nested_fixture_identity(&reversed_transaction, reversed_owner);
    assert_eq!(reversed_owner, normal_owner);
    assert_eq!(reversed_staged_locals, normal_staged_locals);
    assert_eq!(
        reversed_staged_identity, normal_staged_identity,
        "typed child-map insertion order must preserve exact LocalIds and SyntheticKeys"
    );
    close_pattern_scope_members(&mut reversed_transaction, reversed_scope, &[reversed_owner]);
    let reversed_module = reversed_transaction
        .finish(&mut perturb_database)
        .expect("reversed perturbation publication")
        .into_module();
    let (_, reversed_evidence) = assert_nested_fixture_evidence(
        &reversed_module,
        reversed_owner,
        reversed_owner,
        &attached[0],
    );
    assert_eq!(
        reversed_evidence, normal_evidence,
        "typed child-map insertion order must not affect generator evidence"
    );

    let paired_source = format!("{FIXTURE} | {FIXTURE}");
    let paired = parsed_source("nested-binding-generator-or", &[&paired_source]);
    let (paired_module, paired_owners, paired_attached) = lower_and_publish(&paired);
    let outer = paired_owners[0];
    let HirPatternKind::Or { alternatives } = pattern(&paired_module, outer).kind() else {
        panic!("paired fixture must lower as an Or Pattern");
    };
    let [first, second] = alternatives.as_ref() else {
        panic!("paired fixture must have two alternatives");
    };
    let first_attached = attached_pattern_child(&paired_attached[0], PatternNodeStep::Element(0));
    let (first_locals, _) =
        assert_nested_fixture_evidence(&paired_module, *first, outer, &first_attached);
    let second_locals = nested_fixture_locals(&paired_module, *second);
    assert_eq!(
        first_locals.ordinary, second_locals.ordinary,
        "later Or alternatives must reuse all seven ordinary LocalIds"
    );
    assert_eq!(
        first_locals.rest, second_locals.rest,
        "later Or alternatives must reuse the first PatternRest LocalId"
    );
    assert_eq!(
        paired_module
            .arenas()
            .locals()
            .try_iter(paired_module.slots())
            .expect("paired fixture Local arena")
            .len(),
        8,
        "paired Or must not advance Local or synthetic generation"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn or_alternatives_reuse_outer_ordinals_and_first_rest_local() {
    let parsed = parsed_source("or-generator", &["(a, [b, ..tail]) | (a, [b, ..tail])"]);
    let (module, owners, attached) = lower_and_publish(&parsed);
    let outer = owners[0];
    let HirPatternKind::Or { alternatives } = pattern(&module, outer).kind() else {
        panic!("expected Or Pattern");
    };
    let [first, second] = alternatives.as_ref() else {
        panic!("expected two alternatives");
    };
    let tuple_bindings = |owner| {
        let HirPatternKind::Tuple { elements } = pattern(&module, owner).kind() else {
            panic!("expected tuple alternative");
        };
        let HirPatternKind::Binding(HirPatternBinding::Bound { local: a, .. }) =
            pattern(&module, elements[0]).kind()
        else {
            panic!("expected first tuple binding");
        };
        let HirPatternKind::BracketSequence { elements, rest } =
            pattern(&module, elements[1]).kind()
        else {
            panic!("expected sequence Pattern");
        };
        let HirPatternKind::Binding(HirPatternBinding::Bound { local: b, .. }) =
            pattern(&module, elements[0]).kind()
        else {
            panic!("expected sequence binding");
        };
        let HirPatternSequenceRest::Bound(tail) = rest else {
            panic!("expected named sequence rest");
        };
        (*a, *b, *tail)
    };
    let first_locals = tuple_bindings(*first);
    let second_locals = tuple_bindings(*second);
    assert_eq!(first_locals, second_locals, "Or must reuse exact LocalIds");

    let locals = module
        .arenas()
        .locals()
        .try_iter(module.slots())
        .expect("Local arena iteration");
    assert_eq!(locals.len(), 3, "later Or alternative adds no Local");
    let mut ordinary_ordinals = Vec::new();
    let mut rest_owner = None;
    for (id, payload) in locals {
        let metadata = module.slots().resolve(id).expect("Local slot metadata");
        let HirOrigin::Synthetic(key) = metadata.origin() else {
            panic!("Pattern locals are synthetic");
        };
        match key.role() {
            SyntheticRole::DestructuredBinding => {
                assert_eq!(key.owner(), SyntheticOwner::Pattern(outer));
                assert_eq!(payload.pattern(), Some(outer));
                ordinary_ordinals.push(key.ordinal());
            }
            SyntheticRole::PatternRest => {
                let SyntheticOwner::Pattern(owner) = key.owner() else {
                    panic!("PatternRest owner must be Pattern");
                };
                assert_eq!(key.ordinal(), 0);
                assert_eq!(payload.pattern(), Some(owner));
                rest_owner = Some(owner);
            }
            role => panic!("unexpected Pattern Local role {role:?}"),
        }
    }
    ordinary_ordinals.sort_unstable();
    assert_eq!(ordinary_ordinals, [0, 1]);
    assert!(rest_owner.is_some());

    let expected_rest_binding = attached[0]
        .children()
        .expect("Or children")
        .into_iter()
        .find_map(|child| {
            child.pattern().and_then(|pattern| {
                matches!(pattern.value().kind(), PatternSyntaxKind::Tuple(_))
                    .then(|| pattern.clone())
            })
        })
        .and_then(|tuple| {
            tuple.children().ok()?.into_iter().find_map(|child| {
                child.pattern().and_then(|pattern| {
                    matches!(
                        pattern.value().kind(),
                        PatternSyntaxKind::BracketSequence(_)
                    )
                    .then(|| pattern.clone())
                })
            })
        })
        .and_then(|sequence| {
            sequence
                .component(PatternComponentRole::SequenceRest(
                    arcweft_lang_syntax::patterns::PatternRestPart::Binding,
                ))
                .map(|span| span.range())
        })
        .expect("first sequence rest source");
    let rest_metadata = module
        .slots()
        .resolve(first_locals.2)
        .expect("rest Local metadata");
    let HirSourceSite::Span(rest_span) = rest_metadata.source_site() else {
        panic!("rest Local must retain its exact authored binding span");
    };
    assert_eq!(rest_span.range(), expected_rest_binding);
}

#[test]
fn record_cross_field_recovery_does_not_allocate_duplicate_or_second_rest() {
    let parsed = parsed_source("record-recovery", &["{a, a, ..tail, ..other}"]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let HirPatternKind::Record { fields, .. } = pattern(&module, owners[0]).kind() else {
        panic!("expected record Pattern");
    };
    assert!(matches!(
        fields[1],
        HirPatternField::Invalid {
            issue: crate::pattern::HirPatternFieldIssue::DuplicateName
        }
    ));
    assert!(matches!(
        fields[3],
        HirPatternField::Invalid {
            issue: crate::pattern::HirPatternFieldIssue::MultipleRest
        }
    ));
    assert_eq!(
        module
            .arenas()
            .locals()
            .try_iter(module.slots())
            .expect("Local arena")
            .len(),
        2
    );
}

#[test]
fn underscore_allocates_no_local() {
    let parsed = parsed_source("discard-bindings", &["_", "(left, _, right)"]);
    let (module, owners, _) = lower_and_publish(&parsed);

    assert!(matches!(
        pattern(&module, owners[0]).kind(),
        HirPatternKind::Discard
    ));
    let HirPatternKind::Tuple { elements } = pattern(&module, owners[1]).kind() else {
        panic!("mixed discard fixture must retain its tuple Pattern");
    };
    let [left, discard, right] = elements.as_ref() else {
        panic!("mixed discard tuple must retain all three PatternIds");
    };
    assert!(matches!(
        pattern(&module, *left).kind(),
        HirPatternKind::Binding(_)
    ));
    assert!(matches!(
        pattern(&module, *discard).kind(),
        HirPatternKind::Discard
    ));
    assert!(matches!(
        pattern(&module, *right).kind(),
        HirPatternKind::Binding(_)
    ));

    let scope = pattern(&module, owners[0]).scope();
    let scope = module.resolve_scope(scope).expect("discard fixture scope");
    let names = scope
        .locals()
        .iter()
        .map(|local| module.resolve_local(*local).unwrap().name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["left", "right"]);
    assert!(module.captures().next().is_none());
}

#[test]
fn generations_follow_current_scope_source_order() {
    let parsed = parsed_source("generation-order", &["same", "same", "other", "same"]);
    let (module, _, _) = lower_and_publish(&parsed);
    let mut same = module
        .arenas()
        .locals()
        .try_iter(module.slots())
        .expect("Local arena")
        .filter_map(|(_, local)| {
            (local.name().as_str() == "same").then_some(local.generation().get())
        })
        .collect::<Vec<_>>();
    same.sort_unstable();
    assert_eq!(same, [1, 2, 3]);
}

#[test]
fn generation_ledger_rejects_reversed_root_lowering_without_publication() {
    let parsed = parsed_source("generation-reversed", &["same", "same"]);
    let attached = attached_patterns(&parsed);
    let first_start = attached[0]
        .component(PatternComponentRole::Name)
        .expect("first binding name")
        .range()
        .start();
    let second_start = attached[1]
        .component(PatternComponentRole::Name)
        .expect("second binding name")
        .range()
        .start();
    assert!(first_start < second_start);

    let mut database = HirDatabase::try_new().expect("HIR database");
    let key = module_key(&parsed);
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    transaction
        .lower_attached_pattern(&attached[1], scope)
        .expect("later source root can stage first only until order is contradicted");
    let name = crate::leaf::HirName::try_new("same".into()).expect("valid HIR name");
    let timeline = transaction
        .local_timelines
        .get(&(scope, name.clone()))
        .expect("same-name publication timeline");
    let [entry] = timeline.entries() else {
        panic!("one lowered binding must publish one timeline entry");
    };
    assert_eq!(entry.generation, LocalGeneration::FIRST);
    assert_eq!(entry.binding_name_start, second_start);
    let diagnostic_count = transaction.diagnostics.len();

    assert_eq!(
        transaction.lower_attached_pattern(&attached[0], scope),
        Err(HirLowerFailure::LocalBindingSourceOrderViolation {
            scope,
            name: name.clone(),
            previous_start: second_start,
            attempted_start: first_start,
        })
    );
    assert_eq!(transaction.local_timelines.len(), 1);
    assert_eq!(transaction.diagnostics.len(), diagnostic_count);
    assert!(
        transaction.finish(&mut database).is_err(),
        "poisoned reverse-order transaction must not freeze"
    );
    assert!(database.current(&key).is_none());
}

#[test]
fn duplicate_binding_gets_distinct_poisoned_local_without_advancing_generation() {
    let parsed = parsed_source("duplicate-generation", &["(same, same)", "same"]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let HirPatternKind::Tuple { elements } = pattern(&module, owners[0]).kind() else {
        panic!("expected destructuring tuple");
    };
    let tuple_local = |owner| {
        let HirPatternKind::Binding(HirPatternBinding::Bound { local, .. }) =
            pattern(&module, owner).kind()
        else {
            panic!("expected binding Pattern");
        };
        *local
    };
    let first = tuple_local(elements[0]);
    let duplicate = tuple_local(elements[1]);
    let HirPatternKind::Binding(HirPatternBinding::Bound { local: later, .. }) =
        pattern(&module, owners[1]).kind()
    else {
        panic!("expected later binding Pattern");
    };

    assert_ne!(first, duplicate, "a duplicate owns a distinct LocalId");
    assert_eq!(local(&module, first).generation(), LocalGeneration::FIRST);
    assert!(!local(&module, first).is_poisoned());
    assert_eq!(
        local(&module, duplicate).generation(),
        LocalGeneration::FIRST
    );
    assert!(local(&module, duplicate).is_poisoned());
    assert_eq!(local(&module, *later).generation().get(), 2);
    assert!(!local(&module, *later).is_poisoned());

    let duplicate_diagnostics = module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            crate::diagnostic::HirDiagnostic::Recovery(recovery) => Some(recovery.owner()),
            crate::diagnostic::HirDiagnostic::Syntax(_)
            | crate::diagnostic::HirDiagnostic::LineIdentity(_) => None,
        })
        .filter(|owner| *owner == SyntheticOwner::Local(duplicate))
        .count();
    assert_eq!(duplicate_diagnostics, 1);
}

#[test]
fn exact_1024_ordinary_bindings_publish_and_1025_rolls_back() {
    let names = |count: usize| {
        format!(
            "({})",
            (0..count)
                .map(|index| format!("binding_{index}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let exact_source = names(1_024);
    let exact = parsed_source("binding-limit-exact", &[&exact_source]);
    let exact_attached = attached_patterns(&exact);
    let mut exact_database = HirDatabase::try_new().expect("HIR database");
    let mut exact_transaction = stage(&exact_database, &exact);
    let exact_scope = allocate_module_scope(&mut exact_transaction, &exact);
    let outer = exact_transaction
        .lower_attached_pattern(&exact_attached[0], exact_scope)
        .expect("1,024 ordinary bindings must stage");
    assert_eq!(exact_transaction.local_timelines.len(), 1_024);
    assert!(exact_transaction.local_timelines.values().all(|timeline| {
        matches!(
            timeline.entries(),
            [entry]
                if entry.generation == LocalGeneration::FIRST
                    && entry.binding_name_start < exact.document().text().len()
        )
    }));
    close_pattern_scope_members(&mut exact_transaction, exact_scope, &[outer]);
    let module = exact_transaction
        .finish(&mut exact_database)
        .expect("exact binding boundary publication")
        .into_module();
    let locals = module
        .arenas()
        .locals()
        .try_iter(module.slots())
        .expect("exact Local arena")
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 1_024);
    let HirPatternKind::Tuple { elements } = pattern(&module, outer).kind() else {
        panic!("expected tuple Pattern");
    };
    assert_eq!(elements.len(), 1_024);
    let mut ordinals = Vec::with_capacity(1_024);
    for (local_id, payload) in locals {
        assert_eq!(payload.generation(), LocalGeneration::FIRST);
        assert_eq!(payload.pattern(), Some(outer));
        let metadata = module
            .slots()
            .resolve(local_id)
            .expect("exact Local metadata");
        let HirOrigin::Synthetic(key) = metadata.origin() else {
            panic!("exact binding Local must be synthetic");
        };
        assert_eq!(key.owner(), SyntheticOwner::Pattern(outer));
        assert_eq!(key.role(), SyntheticRole::DestructuredBinding);
        ordinals.push(key.ordinal());
    }
    ordinals.sort_unstable();
    assert_eq!(ordinals, (0_u32..1_024).collect::<Vec<_>>());

    let over_source = names(1_025);
    let over = parsed_source("binding-limit-over", &[&over_source]);
    let attached = attached_patterns(&over);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let key = module_key(&over);
    let mut transaction = stage(&database, &over);
    let scope = allocate_module_scope(&mut transaction, &over);
    let diagnostic_count = transaction.diagnostics.len();
    let error = transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect_err("1,025 ordinary bindings must fail preflight");
    let HirLowerFailure::Limit(error) = error else {
        panic!("expected typed binding limit, got {error:?}");
    };
    assert_eq!(error.limit(), HirLimit::SyntheticDescendantsPerOwner);
    assert_eq!(error.observed(), 1_025);
    assert_eq!(error.maximum(), 1_024);
    assert!(transaction.local_timelines.is_empty());
    assert_eq!(transaction.diagnostics.len(), diagnostic_count);
    assert!(transaction.finish(&mut database).is_err());
    assert!(
        database.current(&key).is_none(),
        "failed transaction published state"
    );
}

#[test]
fn or_mismatch_is_fatal_before_any_local_allocation() {
    let parsed = parsed_source("or-mismatch", &["left | (left, extra)"]);
    let attached = attached_patterns(&parsed);
    let database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    assert!(attached[0].value().state().issues().iter().any(|issue| {
        matches!(
            issue,
            PatternRecoveryIssue::OrBindings(PatternOrBindingIssue::CountMismatch {
                alternative: 1,
                expected: 1,
                actual: 2,
            })
        )
    }));
    assert_eq!(
        transaction.lower_attached_pattern(&attached[0], scope),
        Err(HirLowerFailure::OrAlternativeBindingsMismatch {
            owner: attached[0].id(),
            issue: PatternOrBindingIssue::CountMismatch {
                alternative: 1,
                expected: 1,
                actual: 2,
            },
        })
    );
    assert!(transaction.local_timelines.is_empty());
}

#[test]
fn recovered_qualified_variant_paths_share_all_path_preflight_limits() {
    let mut too_many_segments = vec!["bad+name".to_owned()];
    too_many_segments.extend((0..256).map(|_| "x".to_owned()));

    let too_long_name = vec!["n".repeat(1_025), "bad+name".to_owned()];

    let mut too_many_semantic_bytes = vec![format!("{}+b", "a".repeat(1_022))];
    too_many_semantic_bytes.extend((0..63).map(|_| "b".repeat(1_024)));
    too_many_semantic_bytes.push("c".to_owned());

    for (document_id, segments, expected_limit, expected_observed) in [
        (
            "recovered-path-segments-over",
            too_many_segments,
            HirLimit::PathSegments,
            257,
        ),
        (
            "recovered-path-name-over",
            too_long_name,
            HirLimit::NameBytes,
            1_025,
        ),
        (
            "recovered-path-semantic-over",
            too_many_semantic_bytes,
            HirLimit::PathSemanticBytes,
            65_537,
        ),
    ] {
        let source = format!("{}.Ready", segments.join("::"));
        let parsed = parsed_source(document_id, &[&source]);
        let attached = attached_patterns(&parsed);
        let PatternSyntaxKind::Variant(variant) = attached[0].value().kind() else {
            panic!("limit fixture must remain a typed Variant Pattern");
        };
        assert!(matches!(
            variant.head(),
            PatternVariantHeadSyntax::Recovered(_)
        ));

        let mut database = HirDatabase::try_new().expect("HIR database");
        let key = module_key(&parsed);
        let mut transaction = stage(&database, &parsed);
        let scope = allocate_module_scope(&mut transaction, &parsed);
        let error = transaction
            .lower_attached_pattern(&attached[0], scope)
            .expect_err("recovered path one-over fixture must fail before publication");
        let HirLowerFailure::Limit(error) = error else {
            panic!("expected typed path limit, got {error:?}");
        };
        assert_eq!(error.limit(), expected_limit);
        assert_eq!(error.observed(), expected_observed);
        assert!(transaction.local_timelines.is_empty());
        assert!(transaction.finish(&mut database).is_err());
        assert!(database.current(&key).is_none());
    }
}

#[test]
fn foreign_attached_pattern_is_rejected_before_reservation() {
    let current = parsed_source("foreign-current", &["current"]);
    let foreign = parsed_source("foreign-supplied", &["foreign"]);
    let foreign_pattern = attached_patterns(&foreign).remove(0);
    let database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, &current);
    let scope = allocate_module_scope(&mut transaction, &current);
    assert!(matches!(
        transaction.lower_attached_pattern(&foreign_pattern, scope),
        Err(HirLowerFailure::StaleSource { .. } | HirLowerFailure::SourceIdentityMismatch { .. })
    ));
}

#[test]
fn local_generation_exhaustion_is_atomic() {
    let parsed = parsed_source("generation-exhaustion", &["overflowed", "overflowed"]);
    let attached = attached_patterns(&parsed);
    let database = HirDatabase::try_new().expect("HIR database");
    let key = module_key(&parsed);
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let name = crate::leaf::HirName::try_new("overflowed".into()).expect("valid HIR name");
    transaction
        .lower_attached_pattern(&attached[0], scope)
        .expect("first binding establishes a real local timeline");
    let timeline = transaction
        .local_timelines
        .get_mut(&(scope, name.clone()))
        .expect("overflowed publication timeline");
    let entry = timeline
        .entries
        .last_mut()
        .expect("first binding timeline entry");
    entry.generation = LocalGeneration::try_new(u32::MAX).expect("maximum non-zero generation");

    assert_eq!(
        transaction.lower_attached_pattern(&attached[1], scope),
        Err(HirLowerFailure::LocalGenerationExhausted { scope, name })
    );
    drop(transaction);
    assert!(
        database.current(&key).is_none(),
        "fatal generation exhaustion published a module"
    );
}

#[path = "tests/attached_matrix.rs"]
mod attached_matrix;
#[path = "tests/payload_freeze.rs"]
mod payload_freeze;
