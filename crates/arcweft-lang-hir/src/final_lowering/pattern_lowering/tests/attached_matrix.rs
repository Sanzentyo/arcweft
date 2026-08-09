use std::sync::Arc;

use arcweft_lang_syntax::id_ref::SyntaxIdRefPart;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_lang_syntax::literal::{
    SyntaxCharacterIssue, SyntaxDecimalComponentIssue, SyntaxDecimalIssue, SyntaxDurationIssue,
    SyntaxIntegerIssue, SyntaxLiteralIssue, SyntaxStringIssue, SyntaxUnitNumberIssue,
};
use arcweft_lang_syntax::patterns::{
    PatternComponentRole, PatternFieldPart, PatternLiteralPart, PatternRestPart,
    PatternSyntaxFamily, PatternSyntaxKind,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use super::*;
use crate::diagnostic::{HirDiagnostic, HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::leaf::{
    HirCharacterIssue, HirCharacterLiteral, HirDecimalIssue, HirDurationIssue, HirDurationLiteral,
    HirFloatIssue, HirFloatLiteral, HirIdRef, HirIdRefIssue, HirIdRefValue, HirIntegerIssue,
    HirIntegerLiteral, HirIntegerRadix, HirLiteral, HirPathRoot, HirPathSegment, HirStringIssue,
    HirStringLiteral, HirUnitNumberIssue, HirUnitNumberLiteral,
};
use crate::module::HirModuleStatus;
use crate::pattern::{
    HirPatternBindingIssue, HirPatternChildRole, HirPatternFieldIssue, HirPatternRecordPath,
};
use crate::scope::HirLocalKind;
use crate::source_index::{
    HirIdRefSourcePart, HirLiteralSourcePart, HirPatternFieldSourcePart, HirPatternRestSourcePart,
    HirPatternSourceRole, HirSourceOwnerStatus, HirSourcePresence, HirSourceQuery,
    HirSourceQueryError, HirSourceSite, HirVariantPatternHeadSourcePart,
};

#[derive(Clone, Copy)]
struct FamilyCase {
    name: &'static str,
    source: &'static str,
    family: PatternSyntaxFamily,
    poisoned: bool,
    representative: Option<(PatternComponentRole, HirPatternSourceRole)>,
    absent_optional: Option<HirPatternSourceRole>,
    ordinal_one_over: Option<(HirPatternSourceRole, u32)>,
    inapplicable: HirPatternSourceRole,
}

fn hir_family(kind: &HirPatternKind) -> PatternSyntaxFamily {
    match kind {
        HirPatternKind::Binding(_) => PatternSyntaxFamily::Binding,
        HirPatternKind::MutableBinding(_) => PatternSyntaxFamily::MutableBinding,
        HirPatternKind::Literal(_) => PatternSyntaxFamily::Literal,
        HirPatternKind::EntityReference(_) => PatternSyntaxFamily::EntityReference,
        HirPatternKind::Variant(_) => PatternSyntaxFamily::Variant,
        HirPatternKind::Discard => PatternSyntaxFamily::Discard,
        HirPatternKind::Tuple { .. } => PatternSyntaxFamily::Tuple,
        HirPatternKind::Record { .. } => PatternSyntaxFamily::Record,
        HirPatternKind::BracketSequence { .. } => PatternSyntaxFamily::BracketSequence,
        HirPatternKind::WholeBinding { .. } => PatternSyntaxFamily::WholeBinding,
        HirPatternKind::Or { .. } => PatternSyntaxFamily::Or,
        HirPatternKind::TypedBinding { .. } => PatternSyntaxFamily::TypedBinding,
        HirPatternKind::Error(_) => PatternSyntaxFamily::Error,
    }
}

fn owner_status(poisoned: bool) -> HirSourceOwnerStatus {
    if poisoned {
        HirSourceOwnerStatus::Poisoned
    } else {
        HirSourceOwnerStatus::Clean
    }
}

fn assert_attached_site(
    module: &HirModule,
    parsed: &ParsedSource,
    owner: PatternId,
    attached: &AttachedPatternNode,
    syntax_role: PatternComponentRole,
    hir_role: HirPatternSourceRole,
    expected_status: HirSourceOwnerStatus,
) {
    let expected = attached
        .component(syntax_role)
        .unwrap_or_else(|| panic!("missing attached component {syntax_role:?}"));
    let lookup = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Pattern {
                owner,
                role: hir_role,
            },
        )
        .unwrap_or_else(|error| panic!("source query {hir_role:?} failed: {error:?}"));
    assert_eq!(lookup.owner_status(), expected_status);
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(actual)) => {
            assert_eq!(actual, &expected);
            assert_ne!(expected.range().start(), expected.range().end());
        }
        HirSourcePresence::Present(HirSourceSite::Insertion(actual)) => {
            assert_eq!(expected.range().start(), expected.range().end());
            assert_eq!(actual.source_identity(), expected.source());
            assert_eq!(actual.offset(), expected.range().start());
        }
        HirSourcePresence::AbsentOptional => {
            panic!("attached component {syntax_role:?} was published as absent")
        }
    }
}

fn assert_all_attached_component_sites(
    module: &HirModule,
    parsed: &ParsedSource,
    owner: PatternId,
    attached: &AttachedPatternNode,
    expected_status: HirSourceOwnerStatus,
) {
    for component in attached.components() {
        if component.role() == PatternComponentRole::Whole {
            continue;
        }
        let role = HirPatternSourceRole::from(component.role());
        let lookup = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Pattern { owner, role },
            )
            .unwrap_or_else(|error| panic!("source query {role:?} failed: {error:?}"));
        assert_eq!(lookup.owner_status(), expected_status, "{role:?}");
        match lookup.presence() {
            HirSourcePresence::Present(HirSourceSite::Span(actual)) => {
                assert_eq!(actual, component.source_span(), "{role:?}");
                assert_ne!(actual.range().start(), actual.range().end(), "{role:?}");
            }
            HirSourcePresence::Present(HirSourceSite::Insertion(actual)) => {
                assert_eq!(
                    component.source_span().range().start(),
                    component.source_span().range().end(),
                    "{role:?}"
                );
                assert_eq!(actual.source_identity(), component.source_span().source());
                assert_eq!(actual.offset(), component.source_span().range().start());
            }
            HirSourcePresence::AbsentOptional => {
                panic!("authored attached component {role:?} was published as absent")
            }
        }
    }
}

fn recovery_diagnostics(module: &HirModule, owner: PatternId) -> Vec<&HirRecoveryDiagnostic> {
    module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(recovery)
                if recovery.owner() == SyntheticOwner::Pattern(owner) =>
            {
                Some(recovery)
            }
            HirDiagnostic::Syntax(_)
            | HirDiagnostic::Recovery(_)
            | HirDiagnostic::LineIdentity(_) => None,
        })
        .collect()
}

fn lower_case(
    parsed: &ParsedSource,
    case: &str,
) -> (Arc<HirModule>, PatternId, AttachedPatternNode) {
    let attached = attached_patterns(parsed).remove(0);
    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut transaction = stage(&database, parsed);
    let scope = allocate_module_scope(&mut transaction, parsed);
    let owner = transaction
        .lower_attached_pattern(&attached, scope)
        .unwrap_or_else(|error| panic!("{case}: attached Pattern lowering failed: {error:?}"));
    close_pattern_scope_members(&mut transaction, scope, &[owner]);
    let module = transaction
        .finish(&mut database)
        .unwrap_or_else(|error| panic!("{case}: Pattern publication failed: {error:?}"))
        .into_module();
    (module, owner, attached)
}

fn assert_root_evidence(
    module: &HirModule,
    parsed: &ParsedSource,
    owner: PatternId,
    attached: &AttachedPatternNode,
    poisoned: bool,
) {
    let status = owner_status(poisoned);
    let whole = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Pattern {
                owner,
                role: HirPatternSourceRole::Whole,
            },
        )
        .expect("Pattern Whole source query");
    assert_eq!(whole.owner_status(), status);
    assert_eq!(
        whole.presence(),
        HirSourcePresence::Present(&HirSourceSite::Span(attached.whole_source_span()))
    );

    let diagnostics = recovery_diagnostics(module, owner);
    if poisoned {
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("poisoned root must own exactly one recovery diagnostic")
        };
        assert_eq!(
            diagnostic.primary_role(),
            HirRecoveryPrimary::query(HirSourceQuery::Pattern {
                owner,
                role: HirPatternSourceRole::Whole,
            })
        );
        assert_eq!(
            diagnostic.primary(),
            &HirSourceSite::Span(attached.whole_source_span())
        );
    } else {
        assert!(diagnostics.is_empty());
    }
}

fn assert_source_pattern_child(
    module: &HirModule,
    parent: &AttachedPatternNode,
    step: PatternNodeStep,
    child: PatternId,
    scope: ScopeId,
) -> AttachedPatternNode {
    let attached = attached_pattern_child(parent, step);
    let metadata = module
        .slots()
        .resolve(child)
        .expect("published child Pattern metadata");
    let HirOrigin::Source(source) = metadata.origin() else {
        panic!("authored child Pattern must remain source-backed")
    };
    assert_eq!(source.syntax(), attached.id());
    assert_eq!(pattern(module, child).scope(), scope);
    attached
}

fn assert_bound_local_payload(
    module: &HirModule,
    pattern_owner: PatternId,
    scope: ScopeId,
    binding: &HirPatternBinding,
    expected_name: &str,
    mutable: bool,
    annotation: Option<TypeId>,
) -> LocalId {
    let HirPatternBinding::Bound { name, local: owner } = binding else {
        panic!("clean binding row must own one admitted Local")
    };
    assert_eq!(name.as_str(), expected_name);
    assert_local_id_payload(
        module,
        pattern_owner,
        scope,
        *owner,
        expected_name,
        mutable,
        annotation,
    );
    *owner
}

fn assert_local_id_payload(
    module: &HirModule,
    pattern_owner: PatternId,
    scope: ScopeId,
    owner: LocalId,
    expected_name: &str,
    mutable: bool,
    annotation: Option<TypeId>,
) {
    let payload = local(module, owner);
    assert_eq!(payload.scope(), scope);
    assert_eq!(payload.kind(), HirLocalKind::PatternBinding);
    assert_eq!(payload.name().as_str(), expected_name);
    assert_eq!(payload.generation(), LocalGeneration::FIRST);
    assert_eq!(payload.pattern(), Some(pattern_owner));
    assert_eq!(payload.annotation(), annotation);
    assert_eq!(payload.is_mutable_binding(), mutable);
    assert!(!payload.is_poisoned());
}

fn assert_discard_child(
    module: &HirModule,
    parent: &AttachedPatternNode,
    step: PatternNodeStep,
    child: PatternId,
    scope: ScopeId,
) {
    assert_source_pattern_child(module, parent, step, child, scope);
    assert_eq!(pattern(module, child).kind(), &HirPatternKind::Discard);
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive match is the compact production payload closure for all Pattern families"
)]
fn assert_exact_family_payload(
    module: &HirModule,
    owner: PatternId,
    attached: &AttachedPatternNode,
    family: PatternSyntaxFamily,
) {
    let payload = pattern(module, owner);
    let scope = payload.scope();
    match (family, payload.kind()) {
        (PatternSyntaxFamily::Binding, HirPatternKind::Binding(binding)) => {
            assert_bound_local_payload(module, owner, scope, binding, "binding", false, None);
        }
        (PatternSyntaxFamily::MutableBinding, HirPatternKind::MutableBinding(binding)) => {
            assert_bound_local_payload(module, owner, scope, binding, "binding", true, None);
        }
        (
            PatternSyntaxFamily::Literal,
            HirPatternKind::Literal(HirLiteral::Integer(HirIntegerLiteral::Value {
                magnitude,
                radix,
                suffix,
            })),
        ) => {
            assert_eq!(magnitude.limbs_le(), [42]);
            assert_eq!(*radix, HirIntegerRadix::Decimal);
            assert_eq!(*suffix, None);
        }
        (
            PatternSyntaxFamily::EntityReference,
            HirPatternKind::EntityReference(HirIdRefValue::Resolved(HirIdRef::Absolute(reference))),
        ) => assert_eq!(reference.as_str(), "flow.opening"),
        (PatternSyntaxFamily::Variant, HirPatternKind::Variant(variant)) => {
            let HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path)) =
                variant.head()
            else {
                panic!("qualified Variant row must retain one HirPath")
            };
            assert_eq!(path.root(), HirPathRoot::ImplicitCrate);
            assert!(matches!(
                path.segments(),
                [HirPathSegment::Identifier(segment)] if segment.as_str() == "Choice"
            ));
            assert!(matches!(
                variant.name(),
                HirVariantPatternName::Resolved(name) if name.as_str() == "Ready"
            ));
            let HirVariantPatternPayload::Pattern(child) = variant.payload() else {
                panic!("qualified Variant row must retain its tuple payload")
            };
            let child_attached = assert_source_pattern_child(
                module,
                attached,
                PatternNodeStep::VariantPayload,
                *child,
                scope,
            );
            let HirPatternKind::Tuple { elements } = pattern(module, *child).kind() else {
                panic!("Variant payload delimiters must retain their tuple Pattern")
            };
            let [record] = elements.as_ref() else {
                panic!("Variant tuple payload must preserve its one authored child")
            };
            let record_attached = assert_source_pattern_child(
                module,
                &child_attached,
                PatternNodeStep::Element(0),
                *record,
                scope,
            );
            let HirPatternKind::Record { path, fields } = pattern(module, *record).kind() else {
                panic!("Variant tuple child must retain its record Pattern")
            };
            assert_eq!(path, &HirPatternRecordPath::Absent);
            let [
                HirPatternField::Explicit {
                    name,
                    pattern: value,
                },
            ] = fields.as_ref()
            else {
                panic!("Variant record payload must preserve its one explicit field")
            };
            assert_eq!(name.as_str(), "value");
            assert_discard_child(
                module,
                &record_attached,
                PatternNodeStep::RecordField(0),
                *value,
                scope,
            );
        }
        (PatternSyntaxFamily::Discard, HirPatternKind::Discard) => {}
        (PatternSyntaxFamily::Tuple, HirPatternKind::Tuple { elements }) => {
            let [left, right] = elements.as_ref() else {
                panic!("Tuple row must preserve two ordered children")
            };
            assert_discard_child(module, attached, PatternNodeStep::Element(0), *left, scope);
            assert_discard_child(module, attached, PatternNodeStep::Element(1), *right, scope);
        }
        (PatternSyntaxFamily::Record, HirPatternKind::Record { path, fields }) => {
            assert_eq!(path, &HirPatternRecordPath::Absent);
            let [
                HirPatternField::Explicit {
                    name,
                    pattern: child,
                },
                HirPatternField::Rest { binding: None },
            ] = fields.as_ref()
            else {
                panic!("Record row must retain its explicit field then unbound rest")
            };
            assert_eq!(name.as_str(), "left");
            assert_discard_child(
                module,
                attached,
                PatternNodeStep::RecordField(0),
                *child,
                scope,
            );
        }
        (
            PatternSyntaxFamily::BracketSequence,
            HirPatternKind::BracketSequence { elements, rest },
        ) => {
            let [left, right] = elements.as_ref() else {
                panic!("Sequence row must retain two ordered elements")
            };
            assert_discard_child(module, attached, PatternNodeStep::Element(0), *left, scope);
            assert_discard_child(module, attached, PatternNodeStep::Element(1), *right, scope);
            let HirPatternSequenceRest::Bound(rest) = rest else {
                panic!("Sequence row must retain its bound rest Local")
            };
            let rest_payload = local(module, *rest);
            assert_eq!(rest_payload.scope(), scope);
            assert_eq!(rest_payload.kind(), HirLocalKind::PatternBinding);
            assert_eq!(rest_payload.name().as_str(), "tail");
            assert_eq!(rest_payload.generation(), LocalGeneration::FIRST);
            assert_eq!(rest_payload.pattern(), Some(owner));
            assert_eq!(rest_payload.annotation(), None);
            assert!(!rest_payload.is_mutable_binding());
            assert!(!rest_payload.is_poisoned());
        }
        (
            PatternSyntaxFamily::WholeBinding,
            HirPatternKind::WholeBinding {
                binding,
                pattern: child,
            },
        ) => {
            assert_bound_local_payload(module, owner, scope, binding, "whole", false, None);
            let child_attached = assert_source_pattern_child(
                module,
                attached,
                PatternNodeStep::NestedPattern,
                *child,
                scope,
            );
            let HirPatternKind::Tuple { elements } = pattern(module, *child).kind() else {
                panic!("WholeBinding row must retain its nested tuple")
            };
            let [left, right] = elements.as_ref() else {
                panic!("WholeBinding tuple must retain two ordered children")
            };
            assert_discard_child(
                module,
                &child_attached,
                PatternNodeStep::Element(0),
                *left,
                scope,
            );
            assert_discard_child(
                module,
                &child_attached,
                PatternNodeStep::Element(1),
                *right,
                scope,
            );
        }
        (PatternSyntaxFamily::Or, HirPatternKind::Or { alternatives }) => {
            let [left, right] = alternatives.as_ref() else {
                panic!("Or row must retain two ordered alternatives")
            };
            assert_discard_child(module, attached, PatternNodeStep::Element(0), *left, scope);
            assert_discard_child(module, attached, PatternNodeStep::Element(1), *right, scope);
        }
        (PatternSyntaxFamily::TypedBinding, HirPatternKind::TypedBinding { binding, ty }) => {
            assert_bound_local_payload(module, owner, scope, binding, "typed", false, Some(*ty));
            let attached_type = attached
                .children()
                .expect("TypedBinding attached children")
                .into_iter()
                .find_map(|child| match child {
                    AttachedPatternChild::Type {
                        relation: PatternTypeChildRelation::TypedBinding,
                        node,
                    } => Some(node),
                    AttachedPatternChild::Pattern { .. } => None,
                })
                .expect("TypedBinding attached Type child");
            let metadata = module
                .slots()
                .resolve(*ty)
                .expect("published TypedBinding Type metadata");
            let HirOrigin::Source(source) = metadata.origin() else {
                panic!("authored TypedBinding Type must remain source-backed")
            };
            assert_eq!(source.syntax(), attached_type.id());
            let ty_payload = module
                .arenas()
                .types()
                .resolve(module.slots(), *ty)
                .expect("published TypedBinding Type");
            assert_eq!(ty_payload.scope(), scope);
            let crate::type_ref::HirTypeKind::Path(path) = ty_payload.kind() else {
                panic!("TypedBinding Type must retain its Path payload")
            };
            assert_eq!(path.root(), HirPathRoot::ImplicitCrate);
            assert!(matches!(
                path.segments(),
                [HirPathSegment::Identifier(name)] if name.as_str() == "Value"
            ));
        }
        (PatternSyntaxFamily::Error, HirPatternKind::Error(error)) => {
            assert_eq!(error.issue(), HirGenericPatternIssue::UnclassifiedSyntax);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::UnclassifiedSyntax,
                ))
            );
        }
        _ => panic!("production Pattern family and exact payload diverged: {family:?}"),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table keeps the thirteen attached production Pattern families auditable"
)]
fn all_thirteen_attached_pattern_families_publish_typed_source_and_diagnostics() {
    use HirPatternSourceRole as HirRole;
    use PatternComponentRole as SyntaxRole;

    let cases = [
        FamilyCase {
            name: "binding",
            source: "binding",
            family: PatternSyntaxFamily::Binding,
            poisoned: false,
            representative: Some((SyntaxRole::Name, HirRole::Name)),
            absent_optional: None,
            ordinal_one_over: None,
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "mutable-binding",
            source: "mut binding",
            family: PatternSyntaxFamily::MutableBinding,
            poisoned: false,
            representative: Some((SyntaxRole::MutKeyword, HirRole::MutKeyword)),
            absent_optional: None,
            ordinal_one_over: None,
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "literal",
            source: "42",
            family: PatternSyntaxFamily::Literal,
            poisoned: false,
            representative: Some((
                SyntaxRole::Literal(PatternLiteralPart::Body),
                HirRole::Literal(HirLiteralSourcePart::Body),
            )),
            absent_optional: Some(HirRole::Literal(HirLiteralSourcePart::Prefix)),
            ordinal_one_over: None,
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "entity-reference",
            source: "@flow.opening",
            family: PatternSyntaxFamily::EntityReference,
            poisoned: false,
            representative: Some((
                SyntaxRole::EntityReference(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }),
                HirRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 0 }),
            )),
            absent_optional: None,
            ordinal_one_over: Some((
                HirRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 2 }),
                2,
            )),
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "variant",
            source: "Choice.Ready({value: _})",
            family: PatternSyntaxFamily::Variant,
            poisoned: false,
            representative: Some((SyntaxRole::VariantName, HirRole::VariantName)),
            absent_optional: Some(HirRole::VariantHead(
                HirVariantPatternHeadSourcePart::QualifiedRoot,
            )),
            ordinal_one_over: Some((
                HirRole::VariantHead(HirVariantPatternHeadSourcePart::QualifiedSegment {
                    ordinal: 1,
                }),
                1,
            )),
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "discard",
            source: "_",
            family: PatternSyntaxFamily::Discard,
            poisoned: false,
            representative: None,
            absent_optional: None,
            ordinal_one_over: None,
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "tuple",
            source: "(_, _)",
            family: PatternSyntaxFamily::Tuple,
            poisoned: false,
            representative: Some((
                SyntaxRole::Element { ordinal: 0 },
                HirRole::Element { ordinal: 0 },
            )),
            absent_optional: None,
            ordinal_one_over: Some((HirRole::Element { ordinal: 2 }, 2)),
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "record",
            source: "{left: _, ..}",
            family: PatternSyntaxFamily::Record,
            poisoned: false,
            representative: Some((
                SyntaxRole::PatternField {
                    field: 0,
                    part: PatternFieldPart::Whole,
                },
                HirRole::PatternField {
                    field: 0,
                    part: HirPatternFieldSourcePart::Whole,
                },
            )),
            absent_optional: Some(HirRole::RecordPathRoot),
            ordinal_one_over: Some((
                HirRole::PatternField {
                    field: 2,
                    part: HirPatternFieldSourcePart::Whole,
                },
                2,
            )),
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "bracket-sequence",
            source: "[_, _, ..tail]",
            family: PatternSyntaxFamily::BracketSequence,
            poisoned: false,
            representative: Some((
                SyntaxRole::SequenceRest(PatternRestPart::Marker),
                HirRole::SequenceRest(HirPatternRestSourcePart::Marker),
            )),
            absent_optional: None,
            ordinal_one_over: Some((HirRole::Element { ordinal: 2 }, 2)),
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "whole-binding",
            source: "whole (_, _)",
            family: PatternSyntaxFamily::WholeBinding,
            poisoned: false,
            representative: Some((SyntaxRole::WholeBindingName, HirRole::WholeBindingName)),
            absent_optional: None,
            ordinal_one_over: None,
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "or",
            source: "_ | _",
            family: PatternSyntaxFamily::Or,
            poisoned: false,
            representative: Some((
                SyntaxRole::Element { ordinal: 0 },
                HirRole::Element { ordinal: 0 },
            )),
            absent_optional: None,
            ordinal_one_over: Some((HirRole::Element { ordinal: 2 }, 2)),
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "typed-binding",
            source: "typed: Value",
            family: PatternSyntaxFamily::TypedBinding,
            poisoned: false,
            representative: Some((SyntaxRole::TypedBindingType, HirRole::TypedBindingType)),
            absent_optional: None,
            ordinal_one_over: None,
            inapplicable: HirRole::Recovery,
        },
        FamilyCase {
            name: "error",
            source: "+",
            family: PatternSyntaxFamily::Error,
            poisoned: true,
            representative: Some((SyntaxRole::Recovery, HirRole::Recovery)),
            absent_optional: None,
            ordinal_one_over: None,
            inapplicable: HirRole::Name,
        },
    ];

    for case in cases {
        let parsed = parsed_source(case.name, &[case.source]);
        let (module, owner, attached) = lower_case(&parsed, case.name);
        let payload = pattern(&module, owner);
        assert_eq!(attached.family(), case.family, "{}", case.name);
        assert_eq!(hir_family(payload.kind()), case.family, "{}", case.name);
        assert_eq!(
            payload.state().is_poisoned(),
            case.poisoned,
            "{}",
            case.name
        );
        assert_eq!(
            module.status(),
            if case.poisoned {
                HirModuleStatus::Recovered
            } else {
                HirModuleStatus::Clean
            },
            "{}",
            case.name
        );
        assert_root_evidence(&module, &parsed, owner, &attached, case.poisoned);
        assert_all_attached_component_sites(
            &module,
            &parsed,
            owner,
            &attached,
            owner_status(case.poisoned),
        );
        assert_exact_family_payload(&module, owner, &attached, case.family);

        if let Some((syntax_role, hir_role)) = case.representative {
            assert_attached_site(
                &module,
                &parsed,
                owner,
                &attached,
                syntax_role,
                hir_role,
                owner_status(case.poisoned),
            );
        }
        if let Some(role) = case.absent_optional {
            let lookup = module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Pattern { owner, role },
                )
                .expect("optional Pattern source query");
            assert_eq!(lookup.owner_status(), owner_status(case.poisoned));
            assert_eq!(lookup.presence(), HirSourcePresence::AbsentOptional);
        }
        if let Some((role, length)) = case.ordinal_one_over {
            assert_eq!(
                module.source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Pattern { owner, role },
                ),
                Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
                    owner,
                    role,
                    length,
                }),
                "{}",
                case.name
            );
        }
        assert_eq!(
            module.source_site(
                parsed.document().identity(),
                HirSourceQuery::Pattern {
                    owner,
                    role: case.inapplicable,
                },
            ),
            Err(HirSourceQueryError::PatternRoleNotApplicable {
                owner,
                role: case.inapplicable,
            }),
            "{}",
            case.name
        );
    }
}

#[test]
fn variant_heads_and_absent_payloads_keep_exact_production_forms() {
    let parsed = parsed_source(
        "variant-head-forms",
        &[
            ".Foo",
            "Some",
            "None",
            "Ok",
            "Err",
            "vendor-pack::model::Choice.Ready",
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    let expected = [
        (
            HirVariantPatternHead::Unqualified(HirUnqualifiedVariantForm::DotShorthand),
            "Foo",
        ),
        (
            HirVariantPatternHead::Unqualified(HirUnqualifiedVariantForm::BareExpectedType),
            "Some",
        ),
        (
            HirVariantPatternHead::Unqualified(HirUnqualifiedVariantForm::BareExpectedType),
            "None",
        ),
        (
            HirVariantPatternHead::Unqualified(HirUnqualifiedVariantForm::BareExpectedType),
            "Ok",
        ),
        (
            HirVariantPatternHead::Unqualified(HirUnqualifiedVariantForm::BareExpectedType),
            "Err",
        ),
    ];

    for (((owner, attached), (expected_head, expected_name)), ordinal) in owners
        .iter()
        .zip(&attached)
        .take(expected.len())
        .zip(expected)
        .zip(0_u32..)
    {
        let payload = pattern(&module, *owner);
        let HirPatternKind::Variant(variant) = payload.kind() else {
            panic!("variant form {ordinal} must remain a Variant")
        };
        assert_eq!(
            variant.head(),
            &HirVariantPatternHeadValue::Resolved(expected_head)
        );
        assert!(matches!(
            variant.name(),
            HirVariantPatternName::Resolved(name) if name.as_str() == expected_name
        ));
        assert_eq!(variant.payload(), &HirVariantPatternPayload::Absent);
        assert_eq!(payload.state(), &HirPoisonState::Clean);
        assert_all_attached_component_sites(
            &module,
            &parsed,
            *owner,
            attached,
            HirSourceOwnerStatus::Clean,
        );
    }

    let owner = owners[5];
    let payload = pattern(&module, owner);
    let HirPatternKind::Variant(variant) = payload.kind() else {
        panic!("external-capable qualified form must remain a Variant")
    };
    let HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path)) =
        variant.head()
    else {
        panic!("qualified Variant must retain a HirPath")
    };
    assert_eq!(path.root(), HirPathRoot::ImplicitCrate);
    assert!(matches!(
        path.segments(),
        [HirPathSegment::ProjectSymbol(project),
         HirPathSegment::Identifier(module_name),
         HirPathSegment::Identifier(owner_name)]
            if project.as_str() == "vendor-pack"
                && module_name.as_str() == "model"
                && owner_name.as_str() == "Choice"
    ));
    assert!(matches!(
        variant.name(),
        HirVariantPatternName::Resolved(name) if name.as_str() == "Ready"
    ));
    assert_eq!(variant.payload(), &HirVariantPatternPayload::Absent);
    assert_eq!(payload.state(), &HirPoisonState::Clean);
    assert_all_attached_component_sites(
        &module,
        &parsed,
        owner,
        &attached[5],
        HirSourceOwnerStatus::Clean,
    );
}

#[allow(
    clippy::too_many_lines,
    reason = "the production negative matrix keeps each known Pattern family and exact recovery payload auditable"
)]
fn assert_exact_recovery_payload(
    case: &str,
    module: &HirModule,
    owner: PatternId,
    attached: &AttachedPatternNode,
) {
    let payload = pattern(module, owner);
    let scope = payload.scope();
    match case {
        "binding-recovery" => {
            assert_eq!(
                payload.kind(),
                &HirPatternKind::Binding(HirPatternBinding::Recovered {
                    issue: HirPatternBindingIssue::UnexpectedTrailingInput { token_count: 2 },
                })
            );
        }
        "mutable-binding-recovery" => {
            assert_eq!(
                payload.kind(),
                &HirPatternKind::MutableBinding(HirPatternBinding::Recovered {
                    issue: HirPatternBindingIssue::UnexpectedTrailingInput { token_count: 2 },
                })
            );
        }
        "entity-reference-recovery" => {
            let HirPatternKind::EntityReference(HirIdRefValue::Recovered(recovery)) =
                payload.kind()
            else {
                panic!("entity-reference recovery must retain its exact family payload")
            };
            assert_eq!(recovery.issue(), HirIdRefIssue::Missing);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::EntityReference(recovery.issue()),
                ))
            );
        }
        "variant-recovery" => {
            let HirPatternKind::Variant(variant) = payload.kind() else {
                panic!("variant recovery must retain its Variant payload")
            };
            assert_eq!(
                variant.head(),
                &HirVariantPatternHeadValue::Recovered(
                    HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count: 1 },
                )
            );
            assert_eq!(
                variant.name(),
                &HirVariantPatternName::Recovered(HirVariantPatternNameIssue::Missing)
            );
            assert_eq!(variant.payload(), &HirVariantPatternPayload::Absent);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::VariantHead(
                        HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count: 1 },
                    ),
                ))
            );
        }
        "tuple-recovery" => {
            let HirPatternKind::Tuple { elements } = payload.kind() else {
                panic!("tuple recovery must retain its Tuple payload")
            };
            let [left, right] = elements.as_ref() else {
                panic!("tuple recovery must preserve its two ordered children")
            };
            assert_discard_child(module, attached, PatternNodeStep::Element(0), *left, scope);
            assert_discard_child(module, attached, PatternNodeStep::Element(1), *right, scope);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::MissingCloseDelimiter,
                ))
            );
        }
        "record-recovery" => {
            let HirPatternKind::Record { path, fields } = payload.kind() else {
                panic!("record recovery must retain its Record payload")
            };
            assert_eq!(path, &HirPatternRecordPath::Absent);
            let [
                HirPatternField::Shorthand {
                    name: first_name,
                    local: first,
                },
                HirPatternField::Invalid {
                    issue: HirPatternFieldIssue::DuplicateName,
                },
                HirPatternField::Rest {
                    binding: Some(rest),
                },
                HirPatternField::Invalid {
                    issue: HirPatternFieldIssue::MultipleRest,
                },
            ] = fields.as_ref()
            else {
                panic!("record recovery must retain every authored field disposition")
            };
            assert_eq!(first_name.as_str(), "a");
            assert_local_id_payload(module, owner, scope, *first, "a", false, None);
            assert_local_id_payload(module, owner, scope, *rest, "tail", false, None);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::InvalidField {
                        field: 1,
                        issue: HirPatternFieldIssue::DuplicateName,
                    },
                ))
            );
        }
        "sequence-recovery" => {
            let HirPatternKind::BracketSequence { elements, rest } = payload.kind() else {
                panic!("sequence recovery must retain its BracketSequence payload")
            };
            let [left, right] = elements.as_ref() else {
                panic!("sequence recovery must preserve two ordered elements")
            };
            assert_discard_child(module, attached, PatternNodeStep::Element(0), *left, scope);
            assert_discard_child(module, attached, PatternNodeStep::Element(1), *right, scope);
            assert_eq!(*rest, HirPatternSequenceRest::Absent);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::MissingCloseDelimiter,
                ))
            );
        }
        "whole-binding-recovery" => {
            let HirPatternKind::WholeBinding {
                binding,
                pattern: child,
            } = payload.kind()
            else {
                panic!("whole-binding recovery must retain its exact payload")
            };
            assert_bound_local_payload(module, owner, scope, binding, "whole", false, None);
            assert_source_pattern_child(
                module,
                attached,
                PatternNodeStep::NestedPattern,
                *child,
                scope,
            );
            assert!(pattern(module, *child).state().is_poisoned());
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::RecoveredChild {
                        role: HirPatternChildRole::NestedPattern,
                    },
                ))
            );
        }
        "or-recovery" => {
            let HirPatternKind::Or { alternatives } = payload.kind() else {
                panic!("Or recovery must retain its alternatives")
            };
            let [left, right] = alternatives.as_ref() else {
                panic!("Or recovery must preserve two alternatives")
            };
            assert_discard_child(module, attached, PatternNodeStep::Element(0), *left, scope);
            let error_attached = assert_source_pattern_child(
                module,
                attached,
                PatternNodeStep::Element(1),
                *right,
                scope,
            );
            assert!(matches!(
                pattern(module, *right).kind(),
                HirPatternKind::Error(error)
                    if error.issue() == HirGenericPatternIssue::UnclassifiedSyntax
            ));
            assert_eq!(error_attached.family(), PatternSyntaxFamily::Error);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::RecoveredChild {
                        role: HirPatternChildRole::Element { ordinal: 1 },
                    },
                ))
            );
        }
        "typed-binding-recovery" => {
            let HirPatternKind::TypedBinding { binding, ty } = payload.kind() else {
                panic!("typed-binding recovery must retain its binding and TypeId")
            };
            assert_bound_local_payload(module, owner, scope, binding, "typed", false, Some(*ty));
            let type_payload = module
                .arenas()
                .types()
                .resolve(module.slots(), *ty)
                .expect("recovered TypedBinding Type");
            assert_eq!(type_payload.scope(), scope);
            assert!(type_payload.state().is_poisoned());
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::RecoveredChild {
                        role: HirPatternChildRole::TypedBindingType,
                    },
                ))
            );
        }
        "error-recovery" => {
            let HirPatternKind::Error(error) = payload.kind() else {
                panic!("generic recovery must remain Error")
            };
            assert_eq!(error.issue(), HirGenericPatternIssue::UnclassifiedSyntax);
            assert_eq!(
                payload.state(),
                &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                    HirPatternRecoveryIssue::UnclassifiedSyntax,
                ))
            );
        }
        _ => panic!("unhandled production Pattern recovery row: {case}"),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one known-family recovery matrix proves family source retention and root diagnostics"
)]
fn known_family_recovery_keeps_family_source_and_root_diagnostic() {
    let cases = [
        (
            "binding-recovery",
            "binding extra",
            PatternSyntaxFamily::Binding,
            PatternComponentRole::Name,
            HirPatternSourceRole::Name,
        ),
        (
            "mutable-binding-recovery",
            "mut binding extra",
            PatternSyntaxFamily::MutableBinding,
            PatternComponentRole::Name,
            HirPatternSourceRole::Name,
        ),
        (
            "entity-reference-recovery",
            "@...",
            PatternSyntaxFamily::EntityReference,
            PatternComponentRole::EntityReference(SyntaxIdRefPart::SuffixSegment { ordinal: 0 }),
            HirPatternSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 0 }),
        ),
        (
            "variant-recovery",
            "Choice.",
            PatternSyntaxFamily::Variant,
            PatternComponentRole::VariantName,
            HirPatternSourceRole::VariantName,
        ),
        (
            "tuple-recovery",
            "(_, _",
            PatternSyntaxFamily::Tuple,
            PatternComponentRole::Element { ordinal: 0 },
            HirPatternSourceRole::Element { ordinal: 0 },
        ),
        (
            "record-recovery",
            "{a, a, ..tail, ..other}",
            PatternSyntaxFamily::Record,
            PatternComponentRole::PatternField {
                field: 0,
                part: PatternFieldPart::Whole,
            },
            HirPatternSourceRole::PatternField {
                field: 0,
                part: HirPatternFieldSourcePart::Whole,
            },
        ),
        (
            "sequence-recovery",
            "[_, _",
            PatternSyntaxFamily::BracketSequence,
            PatternComponentRole::Element { ordinal: 0 },
            HirPatternSourceRole::Element { ordinal: 0 },
        ),
        (
            "whole-binding-recovery",
            "whole (_, _",
            PatternSyntaxFamily::WholeBinding,
            PatternComponentRole::WholeBindingName,
            HirPatternSourceRole::WholeBindingName,
        ),
        (
            "or-recovery",
            "_ | +",
            PatternSyntaxFamily::Or,
            PatternComponentRole::Element { ordinal: 1 },
            HirPatternSourceRole::Element { ordinal: 1 },
        ),
        (
            "typed-binding-recovery",
            "typed: [Value; 4]",
            PatternSyntaxFamily::TypedBinding,
            PatternComponentRole::TypedBindingType,
            HirPatternSourceRole::TypedBindingType,
        ),
        (
            "error-recovery",
            "+",
            PatternSyntaxFamily::Error,
            PatternComponentRole::Recovery,
            HirPatternSourceRole::Recovery,
        ),
    ];

    for (name, source, family, syntax_role, hir_role) in cases {
        let parsed = parsed_source(name, &[source]);
        let (module, owner, attached) = lower_case(&parsed, name);
        assert_eq!(attached.family(), family, "{name}");
        assert_eq!(hir_family(pattern(&module, owner).kind()), family, "{name}");
        assert!(pattern(&module, owner).state().is_poisoned(), "{name}");
        assert_eq!(module.status(), HirModuleStatus::Recovered, "{name}");
        assert_root_evidence(&module, &parsed, owner, &attached, true);
        assert_all_attached_component_sites(
            &module,
            &parsed,
            owner,
            &attached,
            HirSourceOwnerStatus::Poisoned,
        );
        assert_exact_recovery_payload(name, &module, owner, &attached);
        assert_attached_site(
            &module,
            &parsed,
            owner,
            &attached,
            syntax_role,
            hir_role,
            HirSourceOwnerStatus::Poisoned,
        );
    }
}

struct MalformedLiteralCase {
    name: &'static str,
    source: &'static str,
    syntax_issue: Option<SyntaxLiteralIssue>,
    hir: HirLiteral,
    representative: (PatternComponentRole, HirPatternSourceRole),
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table preserves every source-reachable malformed literal family and issue"
)]
fn source_reachable_malformed_literals_keep_exact_syntax_hir_source_and_diagnostic() {
    let cases = vec![
        MalformedLiteralCase {
            name: "string-invalid-escape",
            source: "\"\\q\"",
            syntax_issue: Some(SyntaxLiteralIssue::String(
                SyntaxStringIssue::InvalidEscape {
                    attempted: "\\q".into(),
                },
            )),
            hir: HirLiteral::String(HirStringLiteral::Invalid(HirStringIssue::InvalidEscape)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Body),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Body),
            ),
        },
        MalformedLiteralCase {
            name: "string-unterminated",
            source: "\"unterminated",
            syntax_issue: Some(SyntaxLiteralIssue::String(
                SyntaxStringIssue::Unterminated {
                    attempted: "\"unterminated = source_value;".into(),
                },
            )),
            hir: HirLiteral::String(HirStringLiteral::Invalid(HirStringIssue::Unterminated)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Body),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Body),
            ),
        },
        MalformedLiteralCase {
            name: "character-invalid-escape",
            source: "\"\\q\"c",
            syntax_issue: Some(SyntaxLiteralIssue::Character(
                SyntaxCharacterIssue::InvalidEscape {
                    attempted: "\\q".into(),
                },
            )),
            hir: HirLiteral::Character(HirCharacterLiteral::Invalid(
                HirCharacterIssue::InvalidEscape,
            )),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Suffix),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Suffix),
            ),
        },
        MalformedLiteralCase {
            name: "character-empty",
            source: "\"\"c",
            syntax_issue: Some(SyntaxLiteralIssue::Character(SyntaxCharacterIssue::Empty {
                attempted: "".into(),
            })),
            hir: HirLiteral::Character(HirCharacterLiteral::Invalid(HirCharacterIssue::Empty)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Suffix),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Suffix),
            ),
        },
        MalformedLiteralCase {
            name: "character-multiple-scalars",
            source: "\"ab\"c",
            syntax_issue: Some(SyntaxLiteralIssue::Character(
                SyntaxCharacterIssue::MultipleScalars {
                    attempted: "ab".into(),
                },
            )),
            hir: HirLiteral::Character(HirCharacterLiteral::Invalid(
                HirCharacterIssue::MultipleScalars,
            )),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Suffix),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Suffix),
            ),
        },
        MalformedLiteralCase {
            name: "integer-missing-digits",
            source: "0x",
            syntax_issue: Some(SyntaxLiteralIssue::Integer(
                SyntaxIntegerIssue::MissingDigits {
                    attempted: "".into(),
                },
            )),
            hir: HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::MissingDigits)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Prefix),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Prefix),
            ),
        },
        MalformedLiteralCase {
            name: "integer-invalid-digits",
            source: "970milli",
            syntax_issue: Some(SyntaxLiteralIssue::Integer(
                SyntaxIntegerIssue::InvalidDigits {
                    attempted: "milli".into(),
                },
            )),
            hir: HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::InvalidDigit)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Suffix),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Suffix),
            ),
        },
        MalformedLiteralCase {
            name: "integer-invalid-separator",
            source: "1__",
            syntax_issue: Some(SyntaxLiteralIssue::Integer(
                SyntaxIntegerIssue::InvalidSeparator {
                    attempted: "1__".into(),
                },
            )),
            hir: HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::InvalidDigit)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Body),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Body),
            ),
        },
        MalformedLiteralCase {
            name: "decimal-invalid-digits",
            source: "1e",
            syntax_issue: Some(SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidDigits {
                    attempted: "".into(),
                },
            ))),
            hir: HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::Decimal(
                HirDecimalIssue::InvalidDigit,
            ))),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Body),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Body),
            ),
        },
        MalformedLiteralCase {
            name: "decimal-invalid-separator",
            source: "1._0",
            syntax_issue: Some(SyntaxLiteralIssue::Decimal(SyntaxDecimalIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidSeparator {
                    attempted: "_0".into(),
                },
            ))),
            hir: HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::Decimal(
                HirDecimalIssue::InvalidDigit,
            ))),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Body),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Body),
            ),
        },
        MalformedLiteralCase {
            name: "decimal-invalid-suffix",
            source: "1.0milli",
            syntax_issue: Some(SyntaxLiteralIssue::Decimal(
                SyntaxDecimalIssue::InvalidSuffix {
                    suffix: "milli".into(),
                },
            )),
            hir: HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::InvalidSuffix)),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Suffix),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Suffix),
            ),
        },
        MalformedLiteralCase {
            name: "unit-number-invalid-digits",
            source: "1epx",
            syntax_issue: Some(SyntaxLiteralIssue::UnitNumber(
                SyntaxUnitNumberIssue::Decimal(SyntaxDecimalComponentIssue::InvalidDigits {
                    attempted: "".into(),
                }),
            )),
            hir: HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(
                HirUnitNumberIssue::Decimal(HirDecimalIssue::InvalidDigit),
            )),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Unit),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Unit),
            ),
        },
        MalformedLiteralCase {
            name: "unit-number-invalid-separator",
            source: "1__px",
            syntax_issue: Some(SyntaxLiteralIssue::UnitNumber(
                SyntaxUnitNumberIssue::Decimal(SyntaxDecimalComponentIssue::InvalidSeparator {
                    attempted: "1__".into(),
                }),
            )),
            hir: HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(
                HirUnitNumberIssue::Decimal(HirDecimalIssue::InvalidDigit),
            )),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Unit),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Unit),
            ),
        },
        MalformedLiteralCase {
            name: "duration-invalid-digits",
            source: "1ems",
            syntax_issue: Some(SyntaxLiteralIssue::Duration(SyntaxDurationIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidDigits {
                    attempted: "".into(),
                },
            ))),
            hir: HirLiteral::Duration(HirDurationLiteral::Invalid(HirDurationIssue::Decimal(
                HirDecimalIssue::InvalidDigit,
            ))),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Unit),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Unit),
            ),
        },
        MalformedLiteralCase {
            name: "duration-invalid-separator",
            source: "1__ms",
            syntax_issue: Some(SyntaxLiteralIssue::Duration(SyntaxDurationIssue::Decimal(
                SyntaxDecimalComponentIssue::InvalidSeparator {
                    attempted: "1__".into(),
                },
            ))),
            hir: HirLiteral::Duration(HirDurationLiteral::Invalid(HirDurationIssue::Decimal(
                HirDecimalIssue::InvalidDigit,
            ))),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Unit),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Unit),
            ),
        },
        MalformedLiteralCase {
            name: "duration-fractional-nanosecond",
            source: "0.1ns",
            syntax_issue: None,
            hir: HirLiteral::Duration(HirDurationLiteral::Invalid(
                HirDurationIssue::FractionalNanosecond,
            )),
            representative: (
                PatternComponentRole::Literal(PatternLiteralPart::Unit),
                HirPatternSourceRole::Literal(HirLiteralSourcePart::Unit),
            ),
        },
    ];

    for case in cases {
        let parsed = parsed_source(case.name, &[case.source]);
        let attached = attached_patterns(&parsed).remove(0);
        let PatternSyntaxKind::Literal(literal) = attached.value().kind() else {
            panic!("{} must remain the attached Literal family", case.name);
        };
        assert_eq!(
            literal.value().issue(),
            case.syntax_issue.as_ref(),
            "{}",
            case.name
        );

        let (module, owner, _) = lower_case(&parsed, case.name);
        let payload = pattern(&module, owner);
        assert_eq!(payload.kind(), &HirPatternKind::Literal(case.hir.clone()));
        let issue = crate::expr::literal_recovery_issue(&case.hir)
            .expect("malformed final HIR literal issue");
        assert_eq!(
            payload.state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::MalformedLiteral(issue)),
            "{}",
            case.name
        );
        assert_root_evidence(&module, &parsed, owner, &attached, true);
        assert_all_attached_component_sites(
            &module,
            &parsed,
            owner,
            &attached,
            HirSourceOwnerStatus::Poisoned,
        );
        assert_attached_site(
            &module,
            &parsed,
            owner,
            &attached,
            case.representative.0,
            case.representative.1,
            HirSourceOwnerStatus::Poisoned,
        );
    }
}

fn reparsed_pattern_source(
    document_id: &str,
    pattern_source: &str,
    inserted: &str,
) -> (ParsedSource, ParsedSource) {
    let name = SourceName::path(format!("proof/pattern-lowering/{document_id}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/pattern-lowering/{document_id}.arcw"
            ))
            .expect("pattern relower document ID"),
            name.clone(),
            format!("fn lower_patterns() {{\n    let {pattern_source} = source_value;\n}}\n"),
        )
        .expect("pattern relower source"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("initial attached Pattern source");
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, 0))
                    .expect("trivia insertion span"),
                inserted,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("trivia-only Pattern reparse");
    (initial, revised)
}

fn nested_synthetic_identities(
    module: &HirModule,
    locals: &NestedFixtureLocals,
) -> Vec<(LocalId, SyntheticKey)> {
    locals
        .ordinary
        .iter()
        .copied()
        .chain([locals.rest])
        .map(|local| {
            let metadata = module
                .slots()
                .resolve(local)
                .expect("nested fixture Local metadata");
            let HirOrigin::Synthetic(key) = metadata.origin() else {
                panic!("nested fixture Local must remain synthetic")
            };
            (local, *key)
        })
        .collect()
}

fn queried_span(
    module: &HirModule,
    parsed: &ParsedSource,
    owner: PatternId,
    role: HirPatternSourceRole,
) -> SourceRange {
    let lookup = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Pattern { owner, role },
        )
        .expect("Pattern span query");
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        panic!("Pattern span query must retain authored bytes")
    };
    span.range()
}

#[test]
fn trivia_relower_returns_stable_source_ids_with_new_spans() {
    const TRIVIA: &str = "// retained Pattern trivia\n";
    const FIXTURE: &str = "(a, {left: b, right: (c, d), ..rest}, e (f, g))";

    let (initial, revised) = reparsed_pattern_source("trivia-identity", FIXTURE, TRIVIA);
    let initial_attached = attached_patterns(&initial).remove(0);
    let revised_attached = revised
        .attached_pattern(initial_attached.id())
        .expect("retained attached Pattern in revised snapshot");
    assert_eq!(initial_attached.id(), revised_attached.id());

    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut first = stage(&database, &initial);
    let first_scope = allocate_module_scope(&mut first, &initial);
    let first_owner = first
        .lower_attached_pattern(&initial_attached, first_scope)
        .expect("initial attached Pattern lowering");
    close_pattern_scope_members(&mut first, first_scope, &[first_owner]);
    let first_module = first
        .finish(&mut database)
        .expect("initial Pattern publication")
        .into_module();
    let first_locals = nested_fixture_locals(&first_module, first_owner);
    let first_synthetics = nested_synthetic_identities(&first_module, &first_locals);
    let first_whole = queried_span(
        &first_module,
        &initial,
        first_owner,
        HirPatternSourceRole::Whole,
    );
    let first_name = queried_span(
        &first_module,
        &initial,
        first_locals.patterns[1],
        HirPatternSourceRole::Name,
    );

    let mut second = stage(&database, &revised);
    let second_scope = allocate_module_scope(&mut second, &revised);
    let second_owner = second
        .lower_attached_pattern(&revised_attached, second_scope)
        .expect("revised attached Pattern lowering");
    close_pattern_scope_members(&mut second, second_scope, &[second_owner]);
    let second_module = second
        .finish(&mut database)
        .expect("revised Pattern publication")
        .into_module();
    let second_locals = nested_fixture_locals(&second_module, second_owner);
    let second_synthetics = nested_synthetic_identities(&second_module, &second_locals);

    assert_eq!(second_scope, first_scope);
    assert_eq!(second_owner, first_owner);
    assert_eq!(second_locals.patterns, first_locals.patterns);
    assert_eq!(second_locals.ordinary, first_locals.ordinary);
    assert_eq!(second_locals.rest, first_locals.rest);
    assert_eq!(second_synthetics, first_synthetics);

    let second_whole = queried_span(
        &second_module,
        &revised,
        second_owner,
        HirPatternSourceRole::Whole,
    );
    let second_name = queried_span(
        &second_module,
        &revised,
        second_locals.patterns[1],
        HirPatternSourceRole::Name,
    );
    assert_eq!(second_whole.start(), first_whole.start() + TRIVIA.len());
    assert_eq!(second_whole.end(), first_whole.end() + TRIVIA.len());
    assert_eq!(second_name.start(), first_name.start() + TRIVIA.len());
    assert_eq!(second_name.end(), first_name.end() + TRIVIA.len());
}

#[test]
fn stale_failed_relower_keeps_old_publication_and_retry_reuses_every_identity() {
    const TRIVIA: &str = "// failed Pattern proposal\n";
    const FIXTURE: &str = "(a, {left: b, right: (c, d), ..rest}, e (f, g))";

    let (initial, revised) = reparsed_pattern_source("failed-revision", FIXTURE, TRIVIA);
    let initial_attached = attached_patterns(&initial).remove(0);
    let revised_attached = revised
        .attached_pattern(initial_attached.id())
        .expect("retained revised Pattern");
    let key = module_key(&initial);
    let revised_key = module_key(&revised);
    assert_eq!(key.package(), revised_key.package());
    assert_eq!(key.path(), revised_key.path());
    assert_ne!(key.source(), revised_key.source());

    let mut database = HirDatabase::try_new().expect("HIR database");
    let mut accepted = stage(&database, &initial);
    let accepted_scope = allocate_module_scope(&mut accepted, &initial);
    let accepted_owner = accepted
        .lower_attached_pattern(&initial_attached, accepted_scope)
        .expect("accepted initial Pattern");
    close_pattern_scope_members(&mut accepted, accepted_scope, &[accepted_owner]);
    let accepted = accepted
        .finish(&mut database)
        .expect("initial Pattern publication")
        .into_module();

    let (failed_scope, failed_owner, failed_locals, failed_synthetics) = {
        let mut failed = stage(&database, &revised);
        let failed_scope = allocate_module_scope(&mut failed, &revised);
        let failed_owner = failed
            .lower_attached_pattern(&revised_attached, failed_scope)
            .expect("valid prefix of failed relower");
        let (failed_locals, failed_synthetics) =
            staged_nested_fixture_identity(&failed, failed_owner);
        assert!(matches!(
            failed.lower_attached_pattern(&initial_attached, failed_scope),
            Err(HirLowerFailure::StaleSource { .. })
        ));
        assert!(failed.finish(&mut database).is_err());
        (failed_scope, failed_owner, failed_locals, failed_synthetics)
    };
    assert!(Arc::ptr_eq(
        &accepted,
        &database
            .current(&key)
            .expect("old accepted module remains current")
    ));

    let mut retry = stage(&database, &revised);
    let retry_scope = allocate_module_scope(&mut retry, &revised);
    let retry_owner = retry
        .lower_attached_pattern(&revised_attached, retry_scope)
        .expect("replacement Pattern relower");
    let (retry_locals, retry_synthetics) = staged_nested_fixture_identity(&retry, retry_owner);
    assert_eq!(retry_scope, failed_scope);
    assert_eq!(retry_owner, failed_owner);
    assert_eq!(retry_locals.patterns, failed_locals.patterns);
    assert_eq!(retry_locals.ordinary, failed_locals.ordinary);
    assert_eq!(retry_locals.rest, failed_locals.rest);
    assert_eq!(retry_synthetics, failed_synthetics);

    close_pattern_scope_members(&mut retry, retry_scope, &[retry_owner]);
    let replacement = retry
        .finish(&mut database)
        .expect("replacement Pattern publication")
        .into_module();
    assert!(Arc::ptr_eq(
        &replacement,
        &database
            .current(&revised_key)
            .expect("replacement becomes current")
    ));
}
