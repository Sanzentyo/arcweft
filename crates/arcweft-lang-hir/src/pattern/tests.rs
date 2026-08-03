use core::num::{NonZeroU32, NonZeroU64};
use std::collections::{BTreeMap, BTreeSet};

use super::{
    HirGenericPatternIssue, HirPattern, HirPatternBinding, HirPatternBindingIssue,
    HirPatternChildRole, HirPatternError, HirPatternField, HirPatternFieldIssue,
    HirPatternInvariantError, HirPatternKind, HirPatternRecordPath, HirPatternRecordPathIssue,
    HirPatternRecoveryIssue, HirPatternResolver, HirPatternSequenceRest,
    HirPatternSequenceRestIssue, HirUnqualifiedVariantForm, HirVariantPattern,
    HirVariantPatternHead, HirVariantPatternHeadIssue, HirVariantPatternHeadValue,
    HirVariantPatternInvariantError, HirVariantPatternName, HirVariantPatternNameIssue,
    HirVariantPatternPayload, HirVariantPatternPayloadIssue,
};
use crate::expr::{HirPoisonState, HirRecoveryIssue};
use crate::identity::{
    HirDatabaseId, HirModuleId, HirTypedId, LocalId, PatternId, RawHirId, ScopeId, TypeId,
};
use crate::leaf::{
    HirEntityReference, HirIdRef, HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue,
    HirLiteral, HirName, HirPath, HirPathIssue, HirPathRoot, HirPathSegment, HirStringIssue,
    HirStringLiteral,
};
use crate::source_index::{
    HirIdRefSourcePart, HirPatternFieldSourcePart, HirPatternSourceRole, HirSourceQueryError,
    HirVariantPatternHeadSourcePart, HirVariantPatternPayloadSourcePart,
};

#[derive(Default)]
struct TestResolver {
    scopes: BTreeSet<ScopeId>,
    locals: BTreeSet<(ScopeId, LocalId)>,
    types: BTreeMap<(ScopeId, TypeId), HirPoisonState>,
    patterns: BTreeMap<(ScopeId, PatternId), HirPattern>,
}

impl TestResolver {
    fn with_scope(scope: ScopeId) -> Self {
        Self {
            scopes: BTreeSet::from([scope]),
            ..Self::default()
        }
    }

    fn admit_local(&mut self, scope: ScopeId, local: LocalId) {
        self.locals.insert((scope, local));
    }

    fn admit_type(&mut self, scope: ScopeId, ty: TypeId) {
        self.types.insert((scope, ty), HirPoisonState::Clean);
    }

    fn admit_poisoned_type(&mut self, scope: ScopeId, ty: TypeId) {
        self.types.insert(
            (scope, ty),
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidTypeRegion(
                crate::leaf::HirTypeRegionIssue::InvalidNamedRegion,
            )),
        );
    }

    fn admit_pattern(&mut self, scope: ScopeId, id: PatternId, pattern: HirPattern) {
        self.patterns.insert((scope, id), pattern);
    }
}

impl HirPatternResolver for TestResolver {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        self.scopes.contains(&scope)
    }

    fn local_is_visible(&self, scope: ScopeId, local: LocalId) -> bool {
        self.locals.contains(&(scope, local))
    }

    fn resolve_type_state(&self, scope: ScopeId, ty: TypeId) -> Option<&HirPoisonState> {
        self.types.get(&(scope, ty))
    }

    fn resolve_pattern(&self, scope: ScopeId, pattern: PatternId) -> Option<&HirPattern> {
        self.patterns.get(&(scope, pattern))
    }
}

fn test_module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).expect("nonzero database")),
        NonZeroU32::new(slot).expect("nonzero module slot"),
    )
}

fn id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("nonzero HIR slot"),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).expect("valid test name")
}

fn path(value: &str) -> HirPath {
    HirPath::try_new(
        HirPathRoot::ImplicitCrate,
        vec![HirPathSegment::Identifier(name(value))].into_boxed_slice(),
    )
    .expect("nonempty test path")
}

fn bound(value: &str, local: LocalId) -> HirPatternBinding {
    HirPatternBinding::Bound {
        name: name(value),
        local,
    }
}

fn variant_head(head: HirVariantPatternHead) -> HirVariantPatternHeadValue {
    HirVariantPatternHeadValue::Resolved(head)
}

fn variant_name(value: &str) -> HirVariantPatternName {
    HirVariantPatternName::Resolved(name(value))
}

fn clean(
    resolver: &TestResolver,
    scope: ScopeId,
    kind: HirPatternKind,
) -> Result<HirPattern, HirPatternInvariantError> {
    HirPattern::try_new(kind, scope, HirPoisonState::Clean, resolver)
}

fn poisoned(
    resolver: &TestResolver,
    scope: ScopeId,
    kind: HirPatternKind,
    issue: HirRecoveryIssue,
) -> Result<HirPattern, HirPatternInvariantError> {
    HirPattern::try_new(kind, scope, HirPoisonState::Poisoned(issue), resolver)
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table-driven test keeps the closed thirteen-family source-role matrix auditable"
)]
fn source_roles_follow_pattern_families_and_exact_ordinals() {
    let owner_module = test_module(1, 1);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let owner = id::<PatternId>(owner_module, 99);
    let resolver = TestResolver::with_scope(owner_scope);

    let tuple = HirPatternKind::Tuple {
        elements: vec![
            id::<PatternId>(owner_module, 2),
            id::<PatternId>(owner_module, 3),
        ]
        .into_boxed_slice(),
    };
    assert_eq!(
        tuple.validate_source_role(owner, HirPatternSourceRole::Element { ordinal: 1 }),
        Ok(())
    );
    let one_over = HirPatternSourceRole::Element { ordinal: 2 };
    assert_eq!(
        tuple.validate_source_role(owner, one_over),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: one_over,
            length: 2,
        })
    );
    assert_eq!(
        tuple.validate_source_role(owner, HirPatternSourceRole::Name),
        Err(HirSourceQueryError::PatternRoleNotApplicable {
            owner,
            role: HirPatternSourceRole::Name,
        })
    );

    let or_pattern = HirPatternKind::Or {
        alternatives: vec![
            id::<PatternId>(owner_module, 4),
            id::<PatternId>(owner_module, 5),
        ]
        .into_boxed_slice(),
    };
    assert_eq!(
        or_pattern.validate_source_role(owner, HirPatternSourceRole::Element { ordinal: 1 }),
        Ok(())
    );
    let absent_alternative = HirPatternSourceRole::Element { ordinal: 2 };
    assert_eq!(
        or_pattern.validate_source_role(owner, absent_alternative),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: absent_alternative,
            length: 2,
        })
    );

    let record = HirPatternKind::Record {
        path: HirPatternRecordPath::Absent,
        fields: vec![HirPatternField::Rest { binding: None }].into_boxed_slice(),
    };
    assert_eq!(
        record.validate_source_role(owner, HirPatternSourceRole::RecordPathRoot),
        Ok(())
    );
    let absent_path_segment = HirPatternSourceRole::RecordPathSegment { ordinal: 0 };
    assert_eq!(
        record.validate_source_role(owner, absent_path_segment),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: absent_path_segment,
            length: 0,
        })
    );
    assert_eq!(
        record.validate_source_role(
            owner,
            HirPatternSourceRole::PatternField {
                field: 0,
                part: HirPatternFieldSourcePart::RestBinding,
            },
        ),
        Ok(())
    );
    let absent_field = HirPatternSourceRole::PatternField {
        field: 1,
        part: HirPatternFieldSourcePart::Whole,
    };
    assert_eq!(
        record.validate_source_role(owner, absent_field),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: absent_field,
            length: 1,
        })
    );

    let entity = HirPatternKind::EntityReference(HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new(Box::<str>::from("scene.opening")).expect("valid entity"),
    )));
    assert_eq!(
        entity.validate_source_role(
            owner,
            HirPatternSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 1 }),
        ),
        Ok(())
    );
    let absent_entity_segment =
        HirPatternSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 2 });
    assert_eq!(
        entity.validate_source_role(owner, absent_entity_segment),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: absent_entity_segment,
            length: 2,
        })
    );

    let qualified = HirPatternKind::Variant(
        HirVariantPattern::try_new(
            variant_head(HirVariantPatternHead::Qualified(path("Choice"))),
            variant_name("Ready"),
            HirVariantPatternPayload::Absent,
            owner_scope,
            &resolver,
        )
        .expect("qualified variant"),
    );
    assert_eq!(
        qualified.validate_source_role(
            owner,
            HirPatternSourceRole::VariantHead(HirVariantPatternHeadSourcePart::QualifiedRoot,),
        ),
        Ok(())
    );
    assert_eq!(
        qualified.validate_source_role(
            owner,
            HirPatternSourceRole::VariantPayload(
                HirVariantPatternPayloadSourcePart::CloseDelimiter,
            ),
        ),
        Ok(())
    );

    let bare = HirPatternKind::Variant(
        HirVariantPattern::try_new(
            variant_head(HirVariantPatternHead::Unqualified(
                HirUnqualifiedVariantForm::BareExpectedType,
            )),
            variant_name("Ready"),
            HirVariantPatternPayload::Absent,
            owner_scope,
            &resolver,
        )
        .expect("expected-type variant"),
    );
    assert_eq!(
        bare.validate_source_role(
            owner,
            HirPatternSourceRole::VariantHead(HirVariantPatternHeadSourcePart::DotShorthandMarker,),
        ),
        Err(HirSourceQueryError::PatternRoleNotApplicable {
            owner,
            role: HirPatternSourceRole::VariantHead(
                HirVariantPatternHeadSourcePart::DotShorthandMarker,
            ),
        })
    );
    let qualified_segment =
        HirPatternSourceRole::VariantHead(HirVariantPatternHeadSourcePart::QualifiedSegment {
            ordinal: 0,
        });
    assert_eq!(
        bare.validate_source_role(owner, qualified_segment),
        Err(HirSourceQueryError::PatternRoleNotApplicable {
            owner,
            role: qualified_segment,
        })
    );

    let error = HirPatternKind::Error(HirPatternError::new(
        HirGenericPatternIssue::UnclassifiedSyntax,
    ));
    assert_eq!(
        error.validate_source_role(owner, HirPatternSourceRole::Recovery),
        Ok(())
    );
}

#[test]
fn exact_pattern_families_construct_through_one_typed_owner() {
    let module = test_module(1, 1);
    let scope = id::<ScopeId>(module, 1);
    let local = id::<LocalId>(module, 2);
    let ty = id::<TypeId>(module, 3);
    let child_id = id::<PatternId>(module, 4);
    let second_child_id = id::<PatternId>(module, 5);
    let mut resolver = TestResolver::with_scope(scope);
    resolver.admit_local(scope, local);
    resolver.admit_type(scope, ty);
    let child = clean(&resolver, scope, HirPatternKind::Discard).expect("child pattern");
    resolver.admit_pattern(scope, child_id, child);
    let second_child = clean(&resolver, scope, HirPatternKind::Discard).expect("child pattern");
    resolver.admit_pattern(scope, second_child_id, second_child);

    let variant = HirVariantPattern::try_new(
        variant_head(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::BareExpectedType,
        )),
        variant_name("Some"),
        HirVariantPatternPayload::Absent,
        scope,
        &resolver,
    )
    .expect("pathless variant");
    let entity = HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new("entity".into()).expect("absolute reference"),
    ));

    let families = vec![
        HirPatternKind::Binding(bound("binding", local)),
        HirPatternKind::MutableBinding(bound("mutable_binding", local)),
        HirPatternKind::Literal(HirLiteral::Boolean(true)),
        HirPatternKind::EntityReference(entity),
        HirPatternKind::Variant(variant),
        HirPatternKind::Discard,
        HirPatternKind::Tuple {
            elements: vec![child_id].into_boxed_slice(),
        },
        HirPatternKind::Record {
            path: HirPatternRecordPath::Resolved(path("Record")),
            fields: vec![
                HirPatternField::Explicit {
                    name: name("field"),
                    pattern: child_id,
                },
                HirPatternField::Shorthand {
                    name: name("short"),
                    local,
                },
                HirPatternField::Rest {
                    binding: Some(local),
                },
            ]
            .into_boxed_slice(),
        },
        HirPatternKind::BracketSequence {
            elements: vec![child_id].into_boxed_slice(),
            rest: HirPatternSequenceRest::Bound(local),
        },
        HirPatternKind::WholeBinding {
            binding: bound("whole", local),
            pattern: child_id,
        },
        HirPatternKind::Or {
            alternatives: vec![child_id, second_child_id].into_boxed_slice(),
        },
        HirPatternKind::TypedBinding {
            binding: bound("typed", local),
            ty,
        },
        HirPatternKind::Error(HirPatternError::new(
            HirGenericPatternIssue::UnclassifiedSyntax,
        )),
    ];

    assert_eq!(families.len(), 13);
    for (ordinal, kind) in families.into_iter().enumerate() {
        let pattern = if ordinal == 12 {
            poisoned(
                &resolver,
                scope,
                kind,
                HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::UnclassifiedSyntax),
            )
        } else {
            clean(&resolver, scope, kind)
        }
        .expect("the final pattern family should construct");
        assert_eq!(pattern.scope(), scope);
    }
}

#[test]
fn recovery_free_known_family_rejects_unrelated_poison_state() {
    let module = test_module(15, 1);
    let scope = id::<ScopeId>(module, 1);
    let resolver = TestResolver::with_scope(scope);
    let actual =
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::TransactionalChildFailure);

    assert_eq!(
        poisoned(&resolver, scope, HirPatternKind::Discard, actual.clone(),),
        Err(HirPatternInvariantError::UnexpectedPatternPoison { actual })
    );
}

#[test]
fn state_only_pattern_recovery_is_admitted_only_by_its_exact_family() {
    let module = test_module(18, 1);
    let scope = id::<ScopeId>(module, 1);
    let first = id::<PatternId>(module, 2);
    let second = id::<PatternId>(module, 3);
    let mut resolver = TestResolver::with_scope(scope);
    let first_pattern =
        clean(&resolver, scope, HirPatternKind::Discard).expect("first clean alternative");
    let second_pattern =
        clean(&resolver, scope, HirPatternKind::Discard).expect("second clean alternative");
    resolver.admit_pattern(scope, first, first_pattern);
    resolver.admit_pattern(scope, second, second_pattern);

    let cases = [
        (
            HirPatternKind::Tuple {
                elements: Box::new([]),
            },
            HirPatternRecoveryIssue::MissingCloseDelimiter,
        ),
        (
            HirPatternKind::Record {
                path: HirPatternRecordPath::Absent,
                fields: Box::new([]),
            },
            HirPatternRecoveryIssue::MissingCloseDelimiter,
        ),
        (
            HirPatternKind::BracketSequence {
                elements: Box::new([]),
                rest: HirPatternSequenceRest::Absent,
            },
            HirPatternRecoveryIssue::SequenceRest(HirPatternSequenceRestIssue::MultipleRest {
                ordinal: 1,
            }),
        ),
        (
            HirPatternKind::Or {
                alternatives: vec![first, second].into_boxed_slice(),
            },
            HirPatternRecoveryIssue::MissingOrAlternative { ordinal: 2 },
        ),
    ];
    for (kind, issue) in cases {
        poisoned(
            &resolver,
            scope,
            kind,
            HirRecoveryIssue::InvalidPattern(issue),
        )
        .expect("state-only poison must be admitted by its exact known family");
    }

    let wrong_family =
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::MissingOrAlternative {
            ordinal: 1,
        });
    assert_eq!(
        poisoned(
            &resolver,
            scope,
            HirPatternKind::Tuple {
                elements: Box::new([]),
            },
            wrong_family.clone(),
        ),
        Err(HirPatternInvariantError::UnexpectedPatternPoison {
            actual: wrong_family,
        })
    );
}

#[test]
fn variant_heads_preserve_qualified_dot_and_bare_forms_without_placeholder_paths() {
    let module = test_module(2, 1);
    let scope = id::<ScopeId>(module, 1);
    let resolver = TestResolver::with_scope(scope);

    assert_eq!(
        HirPath::try_new(HirPathRoot::ImplicitCrate, Box::new([])),
        Err(HirPathIssue::Empty)
    );

    let dot = HirVariantPattern::try_new(
        variant_head(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::DotShorthand,
        )),
        variant_name("Foo"),
        HirVariantPatternPayload::Absent,
        scope,
        &resolver,
    )
    .expect("dot shorthand");
    let bare = HirVariantPattern::try_new(
        variant_head(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::BareExpectedType,
        )),
        variant_name("UnknownVariant"),
        HirVariantPatternPayload::Absent,
        scope,
        &resolver,
    )
    .expect("bare expected-type variant");
    let qualified = HirVariantPattern::try_new(
        variant_head(HirVariantPatternHead::Qualified(path("Status"))),
        variant_name("Ready"),
        HirVariantPatternPayload::Absent,
        scope,
        &resolver,
    )
    .expect("qualified variant");

    assert!(matches!(
        dot.head(),
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::DotShorthand
        ))
    ));
    assert!(matches!(
        bare.head(),
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::BareExpectedType
        ))
    ));
    assert!(matches!(
        bare.name(),
        HirVariantPatternName::Resolved(name) if name.as_str() == "UnknownVariant"
    ));
    let HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(qualified_path)) =
        qualified.head()
    else {
        panic!("qualified source must retain a qualified head")
    };
    assert_eq!(qualified_path.segments().len(), 1);
}

#[test]
fn variant_payload_requires_a_live_same_scope_tuple_or_record() {
    let module = test_module(3, 1);
    let foreign_module = test_module(4, 1);
    let scope = id::<ScopeId>(module, 1);
    let other_scope = id::<ScopeId>(module, 2);
    let tuple_id = id::<PatternId>(module, 3);
    let record_id = id::<PatternId>(module, 4);
    let wrong_kind_id = id::<PatternId>(module, 5);
    let wrong_scope_id = id::<PatternId>(module, 6);
    let missing_id = id::<PatternId>(module, 7);
    let foreign_id = id::<PatternId>(foreign_module, 1);
    let mut resolver = TestResolver::with_scope(scope);
    resolver.scopes.insert(other_scope);

    let tuple = clean(
        &resolver,
        scope,
        HirPatternKind::Tuple {
            elements: Box::new([]),
        },
    )
    .expect("tuple payload");
    let record = clean(
        &resolver,
        scope,
        HirPatternKind::Record {
            path: HirPatternRecordPath::Absent,
            fields: Box::new([]),
        },
    )
    .expect("record payload");
    let wrong_kind = clean(&resolver, scope, HirPatternKind::Discard).expect("wrong kind");
    let wrong_scope =
        clean(&resolver, other_scope, HirPatternKind::Discard).expect("other-scope payload record");
    resolver.admit_pattern(scope, tuple_id, tuple);
    resolver.admit_pattern(scope, record_id, record);
    resolver.admit_pattern(scope, wrong_kind_id, wrong_kind);
    resolver.admit_pattern(scope, wrong_scope_id, wrong_scope);

    for payload in [tuple_id, record_id] {
        let variant = HirVariantPattern::try_new(
            variant_head(HirVariantPatternHead::Unqualified(
                HirUnqualifiedVariantForm::BareExpectedType,
            )),
            variant_name("Payload"),
            HirVariantPatternPayload::Pattern(payload),
            scope,
            &resolver,
        )
        .expect("tuple and record payloads are accepted");
        assert_eq!(
            variant.payload(),
            &HirVariantPatternPayload::Pattern(payload)
        );
    }

    assert_eq!(
        HirVariantPattern::try_new(
            variant_head(HirVariantPatternHead::Unqualified(
                HirUnqualifiedVariantForm::DotShorthand,
            )),
            variant_name("WrongKind"),
            HirVariantPatternPayload::Pattern(wrong_kind_id),
            scope,
            &resolver,
        ),
        Err(HirVariantPatternInvariantError::InvalidPayloadKind)
    );
    for payload in [wrong_scope_id, missing_id, foreign_id] {
        assert_eq!(
            HirVariantPattern::try_new(
                variant_head(HirVariantPatternHead::Unqualified(
                    HirUnqualifiedVariantForm::DotShorthand,
                )),
                variant_name("Unavailable"),
                HirVariantPatternPayload::Pattern(payload),
                scope,
                &resolver,
            ),
            Err(HirVariantPatternInvariantError::ForeignPayload)
        );
    }
}

#[test]
fn recovered_variant_payload_retains_its_typed_child_and_exact_issue() {
    let module = test_module(16, 1);
    let scope = id::<ScopeId>(module, 1);
    let payload = id::<PatternId>(module, 2);
    let mut resolver = TestResolver::with_scope(scope);
    let tuple = poisoned(
        &resolver,
        scope,
        HirPatternKind::Tuple {
            elements: Box::new([]),
        },
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::MissingCloseDelimiter),
    )
    .expect("known tuple recovery remains in the tuple family");
    resolver.admit_pattern(scope, payload, tuple);

    let payload_issue = HirVariantPatternPayloadIssue::InvalidPattern;
    let variant = HirVariantPattern::try_new(
        variant_head(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::DotShorthand,
        )),
        variant_name("Payload"),
        HirVariantPatternPayload::Recovered {
            pattern: Some(payload),
            issue: payload_issue,
        },
        scope,
        &resolver,
    )
    .expect("recovered payload keeps its tuple child");
    assert_eq!(
        variant.payload(),
        &HirVariantPatternPayload::Recovered {
            pattern: Some(payload),
            issue: payload_issue,
        }
    );

    let kind = HirPatternKind::Variant(variant);
    assert_eq!(
        clean(&resolver, scope, kind.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        kind,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::VariantPayload(payload_issue)),
    )
    .expect("parent poison must retain the exact payload issue");
}

#[test]
fn outer_pattern_construction_revalidates_variant_payload_evidence() {
    let module = test_module(5, 1);
    let scope = id::<ScopeId>(module, 1);
    let payload = id::<PatternId>(module, 2);
    let mut staging = TestResolver::with_scope(scope);
    let tuple = clean(
        &staging,
        scope,
        HirPatternKind::Tuple {
            elements: Box::new([]),
        },
    )
    .expect("tuple payload");
    staging.admit_pattern(scope, payload, tuple);
    let variant = HirVariantPattern::try_new(
        variant_head(HirVariantPatternHead::Unqualified(
            HirUnqualifiedVariantForm::DotShorthand,
        )),
        variant_name("Some"),
        HirVariantPatternPayload::Pattern(payload),
        scope,
        &staging,
    )
    .expect("staged payload");

    let publication = TestResolver::with_scope(scope);
    assert_eq!(
        clean(&publication, scope, HirPatternKind::Variant(variant),),
        Err(HirPatternInvariantError::InvalidVariant(
            HirVariantPatternInvariantError::ForeignPayload
        ))
    );
}

#[test]
fn pattern_children_require_typed_transaction_visibility_not_module_equality_alone() {
    let module = test_module(6, 1);
    let foreign_module = test_module(7, 1);
    let scope = id::<ScopeId>(module, 1);
    let local = id::<LocalId>(module, 2);
    let foreign_local = id::<LocalId>(foreign_module, 1);
    let pattern = id::<PatternId>(module, 3);
    let foreign_type = id::<TypeId>(foreign_module, 2);
    let mut resolver = TestResolver::with_scope(scope);

    assert_eq!(
        clean(
            &resolver,
            scope,
            HirPatternKind::Binding(bound("local", local)),
        ),
        Err(HirPatternInvariantError::LocalNotVisible { scope, local })
    );
    assert_eq!(
        clean(
            &resolver,
            scope,
            HirPatternKind::Binding(bound("foreign", foreign_local)),
        ),
        Err(HirPatternInvariantError::ForeignLocal {
            expected: module,
            actual: foreign_module,
        })
    );
    assert_eq!(
        clean(
            &resolver,
            scope,
            HirPatternKind::Tuple {
                elements: vec![pattern].into_boxed_slice(),
            },
        ),
        Err(HirPatternInvariantError::PatternNotVisible { scope, pattern })
    );
    assert_eq!(
        clean(
            &resolver,
            scope,
            HirPatternKind::Or {
                alternatives: vec![pattern].into_boxed_slice(),
            },
        ),
        Err(HirPatternInvariantError::OrPatternAlternativeCount { observed: 1 })
    );
    resolver.admit_local(scope, local);
    assert_eq!(
        clean(
            &resolver,
            scope,
            HirPatternKind::TypedBinding {
                binding: bound("typed", local),
                ty: foreign_type,
            },
        ),
        Err(HirPatternInvariantError::ForeignType {
            expected: module,
            actual: foreign_module,
        })
    );

    let dead_scope = id::<ScopeId>(module, 9);
    assert_eq!(
        clean(&resolver, dead_scope, HirPatternKind::Discard),
        Err(HirPatternInvariantError::ScopeNotLive { scope: dead_scope })
    );
}

#[test]
fn clean_patterns_reject_typed_recovery_payloads_while_poisoned_patterns_retain_them() {
    let module = test_module(8, 1);
    let scope = id::<ScopeId>(module, 1);
    let resolver = TestResolver::with_scope(scope);

    let invalid_literal = HirPatternKind::Literal(HirLiteral::String(HirStringLiteral::Invalid(
        HirStringIssue::InvalidEscape,
    )));
    assert_eq!(
        clean(&resolver, scope, invalid_literal.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    assert_eq!(
        HirPattern::try_new(
            invalid_literal.clone(),
            scope,
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                HirPatternRecoveryIssue::UnclassifiedSyntax,
            )),
            &resolver,
        ),
        Err(HirPatternInvariantError::PatternRecoveryIssueMismatch {
            expected: HirRecoveryIssue::MalformedLiteral(crate::leaf::HirLiteralIssue::String(
                HirStringIssue::InvalidEscape,
            )),
        })
    );
    poisoned(
        &resolver,
        scope,
        invalid_literal,
        HirRecoveryIssue::MalformedLiteral(crate::leaf::HirLiteralIssue::String(
            HirStringIssue::InvalidEscape,
        )),
    )
    .expect("literal poison retains its exact issue");
    let invalid_field = HirPatternKind::Record {
        path: HirPatternRecordPath::Absent,
        fields: vec![HirPatternField::Invalid {
            issue: HirPatternFieldIssue::MissingPattern,
        }]
        .into_boxed_slice(),
    };
    assert_eq!(
        clean(&resolver, scope, invalid_field.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        invalid_field,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::InvalidField {
            field: 0,
            issue: HirPatternFieldIssue::MissingPattern,
        }),
    )
    .expect("field poison retains its exact issue");
    let generic_error = HirPatternKind::Error(HirPatternError::new(
        HirGenericPatternIssue::TransactionalChildFailure,
    ));
    assert_eq!(
        clean(&resolver, scope, generic_error.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    assert_eq!(
        poisoned(
            &resolver,
            scope,
            generic_error.clone(),
            HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::UnclassifiedSyntax),
        ),
        Err(HirPatternInvariantError::PatternRecoveryIssueMismatch {
            expected: HirRecoveryIssue::InvalidPattern(
                HirPatternRecoveryIssue::TransactionalChildFailure,
            ),
        })
    );

    let retained = poisoned(
        &resolver,
        scope,
        generic_error,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::TransactionalChildFailure),
    )
    .expect("poisoned generic recovery");
    assert!(matches!(retained.kind(), HirPatternKind::Error(_)));
    assert!(matches!(retained.state(), HirPoisonState::Poisoned(_)));
}

#[test]
fn record_cross_field_recovery_keeps_first_value_and_poisons_the_later_field() {
    let module = test_module(19, 1);
    let scope = id::<ScopeId>(module, 1);
    let local = id::<LocalId>(module, 2);
    let mut resolver = TestResolver::with_scope(scope);
    resolver.admit_local(scope, local);

    let cases = [
        (
            vec![
                HirPatternField::Shorthand {
                    name: name("field"),
                    local,
                },
                HirPatternField::Invalid {
                    issue: HirPatternFieldIssue::DuplicateName,
                },
            ]
            .into_boxed_slice(),
            HirPatternFieldIssue::DuplicateName,
        ),
        (
            vec![
                HirPatternField::Rest {
                    binding: Some(local),
                },
                HirPatternField::Invalid {
                    issue: HirPatternFieldIssue::MultipleRest,
                },
            ]
            .into_boxed_slice(),
            HirPatternFieldIssue::MultipleRest,
        ),
    ];

    for (fields, issue) in cases {
        let kind = HirPatternKind::Record {
            path: HirPatternRecordPath::Absent,
            fields,
        };
        assert_eq!(
            clean(&resolver, scope, kind.clone()),
            Err(HirPatternInvariantError::CleanRecoveryPayload)
        );
        poisoned(
            &resolver,
            scope,
            kind,
            HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::InvalidField {
                field: 1,
                issue,
            }),
        )
        .expect("cross-field recovery must retain the exact later-field ordinal and issue");
    }
}

#[test]
fn known_binding_recovery_needs_no_name_or_local_sentinel() {
    let module = test_module(9, 1);
    let scope = id::<ScopeId>(module, 1);
    let resolver = TestResolver::with_scope(scope);
    let issue = HirPatternBindingIssue::MissingName;
    let recovered = HirPatternKind::Binding(HirPatternBinding::Recovered { issue });

    assert_eq!(
        clean(&resolver, scope, recovered.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    assert_eq!(
        HirPattern::try_new(
            recovered.clone(),
            scope,
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                HirPatternRecoveryIssue::VariantName(HirVariantPatternNameIssue::Missing),
            )),
            &resolver,
        ),
        Err(HirPatternInvariantError::PatternRecoveryIssueMismatch {
            expected: HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::Binding(issue)),
        })
    );

    let retained = poisoned(
        &resolver,
        scope,
        recovered,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::Binding(issue)),
    )
    .expect("known Binding recovery");
    assert!(matches!(
        retained.kind(),
        HirPatternKind::Binding(HirPatternBinding::Recovered {
            issue: HirPatternBindingIssue::MissingName,
        })
    ));

    let invalid_name = HirPatternKind::MutableBinding(HirPatternBinding::Recovered {
        issue: HirPatternBindingIssue::InvalidName(
            crate::leaf::HirNameInvariantError::InvalidIdentifier,
        ),
    });
    poisoned(
        &resolver,
        scope,
        invalid_name,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::Binding(
            HirPatternBindingIssue::InvalidName(
                crate::leaf::HirNameInvariantError::InvalidIdentifier,
            ),
        )),
    )
    .expect("known MutableBinding invalid-name recovery");
}

#[test]
fn sequence_rest_preserves_absent_unbound_bound_and_recovered_states() {
    let module = test_module(17, 1);
    let scope = id::<ScopeId>(module, 1);
    let local = id::<LocalId>(module, 2);
    let mut resolver = TestResolver::with_scope(scope);
    resolver.admit_local(scope, local);

    for rest in [
        HirPatternSequenceRest::Absent,
        HirPatternSequenceRest::Unbound,
        HirPatternSequenceRest::Bound(local),
    ] {
        clean(
            &resolver,
            scope,
            HirPatternKind::BracketSequence {
                elements: Box::new([]),
                rest,
            },
        )
        .expect("each non-recovered rest state is semantically distinct and clean");
    }

    let binding_issue = HirPatternBindingIssue::UnexpectedTrailingInput { token_count: 1 };
    let rest_issue = HirPatternSequenceRestIssue::InvalidBinding(binding_issue);
    let recovered = HirPatternKind::BracketSequence {
        elements: Box::new([]),
        rest: HirPatternSequenceRest::Recovered(rest_issue),
    };
    assert_eq!(
        clean(&resolver, scope, recovered.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        recovered,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::SequenceRest(rest_issue)),
    )
    .expect("invalid authored rest binding allocates no local and retains exact poison");

    let multiple = HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::SequenceRest(
        HirPatternSequenceRestIssue::MultipleRest { ordinal: 1 },
    ));
    poisoned(
        &resolver,
        scope,
        HirPatternKind::BracketSequence {
            elements: Box::new([]),
            rest: HirPatternSequenceRest::Bound(local),
        },
        multiple.clone(),
    )
    .expect("multiple rest keeps the first admitted semantic rest");
    assert_eq!(
        poisoned(&resolver, scope, HirPatternKind::Discard, multiple.clone()),
        Err(HirPatternInvariantError::UnexpectedPatternPoison { actual: multiple })
    );
    assert_eq!(
        clean(
            &resolver,
            scope,
            HirPatternKind::BracketSequence {
                elements: Box::new([]),
                rest: HirPatternSequenceRest::Recovered(
                    HirPatternSequenceRestIssue::MultipleRest { ordinal: 1 },
                ),
            },
        ),
        Err(HirPatternInvariantError::MultipleRestCannotReplaceFirstRest)
    );
}

#[test]
fn variant_recovery_distinguishes_absent_payload_from_recovered_absence() {
    let module = test_module(10, 1);
    let scope = id::<ScopeId>(module, 1);
    let resolver = TestResolver::with_scope(scope);
    let head = variant_head(HirVariantPatternHead::Unqualified(
        HirUnqualifiedVariantForm::DotShorthand,
    ));

    let absent = HirVariantPattern::try_new(
        head.clone(),
        variant_name("Some"),
        HirVariantPatternPayload::Absent,
        scope,
        &resolver,
    )
    .expect("legitimately absent payload");
    clean(&resolver, scope, HirPatternKind::Variant(absent)).expect("payload absence is clean");

    let payload_issue = HirVariantPatternPayloadIssue::MissingPattern;
    let recovered_payload = HirPatternKind::Variant(
        HirVariantPattern::try_new(
            head,
            variant_name("Some"),
            HirVariantPatternPayload::Recovered {
                pattern: None,
                issue: payload_issue,
            },
            scope,
            &resolver,
        )
        .expect("source recovery is not a transaction invariant failure"),
    );
    assert_eq!(
        clean(&resolver, scope, recovered_payload.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        recovered_payload,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::VariantPayload(payload_issue)),
    )
    .expect("recovered payload absence");

    let head_issue = HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count: 2 };
    let recovered_head = HirPatternKind::Variant(
        HirVariantPattern::try_new(
            HirVariantPatternHeadValue::Recovered(head_issue),
            HirVariantPatternName::Recovered(HirVariantPatternNameIssue::Missing),
            HirVariantPatternPayload::Absent,
            scope,
            &resolver,
        )
        .expect("known malformed Variant"),
    );
    poisoned(
        &resolver,
        scope,
        recovered_head,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::VariantHead(head_issue)),
    )
    .expect("head is the deterministic first recovery issue");
}

#[test]
fn entity_reference_recovery_retains_family_and_exact_source_shape() {
    let module = test_module(11, 1);
    let scope = id::<ScopeId>(module, 1);
    let owner = id::<PatternId>(module, 2);
    let resolver = TestResolver::with_scope(scope);
    let issue = HirIdRefIssue::Invalid(crate::leaf::HirIdRefInvariantError::InvalidFamily);
    let recovery = HirIdRefRecovery::new(
        HirIdRefShape::FamilyRelative {
            parent_depth: 1,
            suffix_segment_count: 2,
        },
        issue,
    );
    let kind = HirPatternKind::EntityReference(HirIdRefValue::Recovered(recovery));

    assert_eq!(
        clean(&resolver, scope, kind.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        kind.clone(),
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::EntityReference(issue)),
    )
    .expect("known entity-reference recovery");
    assert_eq!(
        kind.validate_source_role(
            owner,
            HirPatternSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 1 }),
        ),
        Ok(())
    );
    let one_over =
        HirPatternSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 2 });
    assert_eq!(
        kind.validate_source_role(owner, one_over),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: one_over,
            length: 2,
        })
    );
}

#[test]
fn record_path_recovery_is_distinct_from_authored_path_absence() {
    let module = test_module(12, 1);
    let scope = id::<ScopeId>(module, 1);
    let owner = id::<PatternId>(module, 2);
    let resolver = TestResolver::with_scope(scope);
    let absent = HirPatternKind::Record {
        path: HirPatternRecordPath::Absent,
        fields: Box::new([]),
    };
    clean(&resolver, scope, absent).expect("authored path absence is clean");

    let path_issue = HirPatternRecordPathIssue::new(HirPathIssue::InvalidSegment { ordinal: 0 }, 1);
    let recovered = HirPatternKind::Record {
        path: HirPatternRecordPath::Recovered(path_issue.clone()),
        fields: Box::new([]),
    };
    assert_eq!(
        clean(&resolver, scope, recovered.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        recovered.clone(),
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::RecordPath(path_issue)),
    )
    .expect("record path poison");
    assert_eq!(
        recovered.validate_source_role(
            owner,
            HirPatternSourceRole::RecordPathSegment { ordinal: 0 },
        ),
        Ok(())
    );
    let one_over = HirPatternSourceRole::RecordPathSegment { ordinal: 1 };
    assert_eq!(
        recovered.validate_source_role(owner, one_over),
        Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
            owner,
            role: one_over,
            length: 1,
        })
    );
}

#[test]
fn typed_binding_propagates_poisoned_type_child() {
    let module = test_module(13, 1);
    let scope = id::<ScopeId>(module, 1);
    let local = id::<LocalId>(module, 2);
    let ty = id::<TypeId>(module, 3);
    let mut resolver = TestResolver::with_scope(scope);
    resolver.admit_local(scope, local);
    resolver.admit_poisoned_type(scope, ty);
    let kind = HirPatternKind::TypedBinding {
        binding: bound("value", local),
        ty,
    };

    assert_eq!(
        clean(&resolver, scope, kind.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        kind,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::RecoveredChild {
            role: HirPatternChildRole::TypedBindingType,
        }),
    )
    .expect("typed binding parent retains Type child recovery");
}

#[test]
fn clean_parent_rejects_poisoned_pattern_children() {
    let module = test_module(14, 1);
    let scope = id::<ScopeId>(module, 1);
    let child_id = id::<PatternId>(module, 2);
    let mut resolver = TestResolver::with_scope(scope);
    let child_issue = HirPatternBindingIssue::MissingName;
    let child = poisoned(
        &resolver,
        scope,
        HirPatternKind::Binding(HirPatternBinding::Recovered { issue: child_issue }),
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::Binding(child_issue)),
    )
    .expect("poisoned child");
    resolver.admit_pattern(scope, child_id, child);
    let tuple = HirPatternKind::Tuple {
        elements: vec![child_id].into_boxed_slice(),
    };

    assert_eq!(
        clean(&resolver, scope, tuple.clone()),
        Err(HirPatternInvariantError::CleanRecoveryPayload)
    );
    poisoned(
        &resolver,
        scope,
        tuple,
        HirRecoveryIssue::InvalidPattern(HirPatternRecoveryIssue::RecoveredChild {
            role: HirPatternChildRole::Element { ordinal: 0 },
        }),
    )
    .expect("parent retains deterministic child recovery");
}
