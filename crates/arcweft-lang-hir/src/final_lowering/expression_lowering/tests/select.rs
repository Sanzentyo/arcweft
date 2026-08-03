use super::*;
use arcweft_source::SourceDocumentIdentity;
use serde::Deserialize;
use serde::de::{self, IntoDeserializer};

use crate::source_index::HirInsertionPoint;

enum IdentityValue {
    Text(String),
    Length(u64),
}

impl IntoDeserializer<'_, de::value::Error> for IdentityValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> de::Deserializer<'de> for IdentityValue {
    type Error = de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self {
            Self::Text(value) => visitor.visit_string(value),
            Self::Length(value) => visitor.visit_u64(value),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map
        struct enum identifier ignored_any
    }
}

fn identity_with_length(document: &SourceDocument, source_len: u64) -> SourceDocumentIdentity {
    let fields = [
        (
            "id",
            IdentityValue::Text(document.identity().id().as_str().to_owned()),
        ),
        (
            "revision",
            IdentityValue::Text(document.identity().revision().to_hex()),
        ),
        ("source_len", IdentityValue::Length(source_len)),
    ];
    SourceDocumentIdentity::deserialize(de::value::MapDeserializer::new(fields.into_iter()))
        .expect("test source identity")
}

fn parsed_with_syntax_diagnostics(
    document_id: &str,
    diagnostic_count: usize,
    expression: &str,
) -> ParsedSource {
    let name = SourceName::path(format!("proof/expression-lowering/{document_id}.arcw"));
    let source = format!(
        "fn lower_expressions() {{\n    let value = {expression};\n}}\n{}",
        "use a as\n".repeat(diagnostic_count)
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://lang-hir/expression-lowering/{document_id}.arcw"
            ))
            .expect("diagnostic-prefill document ID"),
            name.clone(),
            source,
        )
        .expect("diagnostic-prefill source"),
    );
    SyntaxDatabase::try_new()
        .expect("diagnostic-prefill syntax database")
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("diagnostic-prefill source parses")
}

fn select(module: &HirModule, owner: ExprId) -> &HirSelectExpr {
    let HirExprKind::Select(select) = expression(module, owner).kind() else {
        panic!("E13 fixture must publish one Select owner");
    };
    select
}

fn recovery_diagnostics(module: &HirModule) -> Vec<&HirRecoveryDiagnostic> {
    module
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(diagnostic) => Some(diagnostic),
            HirDiagnostic::Syntax(_) => None,
        })
        .collect()
}

fn has_recovery_query(module: &HirModule, owner: ExprId, role: HirExprSourceRole) -> bool {
    recovery_diagnostics(module).iter().any(|diagnostic| {
        diagnostic.owner() == SyntheticOwner::Expr(owner)
            && diagnostic.primary_role()
                == HirRecoveryPrimary::query(HirSourceQuery::Expr { owner, role })
    })
}

#[test]
fn attached_e13_select_publishes_typed_member_poison_and_exact_sources() {
    let parsed = parsed_source(
        "select-matrix",
        &[
            "target.member".into(),
            "target.".into(),
            "target?.member".into(),
            "target?.".into(),
        ],
    );
    let (module, owners, attached) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    for (ordinal, expected_member) in [Some("member"), None, Some("member"), None]
        .into_iter()
        .enumerate()
    {
        let select = select(&module, owners[ordinal]);
        match (select.member(), expected_member) {
            (HirSelectedMember::Name(actual), Some(expected)) => {
                assert_eq!(actual.as_str(), expected);
            }
            (HirSelectedMember::Missing, None) => {}
            _ => panic!("Select member payload disagrees with authored syntax"),
        }

        let whole = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[ordinal],
                    role: HirExprSourceRole::Whole,
                },
            )
            .expect("Select Whole source");
        assert_eq!(
            whole.presence(),
            HirSourcePresence::Present(&HirSourceSite::Span(attached[ordinal].whole_source_span()))
        );

        let target = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[ordinal],
                    role: HirExprSourceRole::Target,
                },
            )
            .expect("Select target source");
        assert_eq!(
            target.presence(),
            HirSourcePresence::Present(&HirSourceSite::Span(
                attached[ordinal]
                    .component(ExpressionComponentRole::Target)
                    .expect("attached Select target"),
            ))
        );

        let member = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[ordinal],
                    role: HirExprSourceRole::SelectedMember,
                },
            )
            .expect("Select member source");
        let attached_member = attached[ordinal]
            .component(ExpressionComponentRole::SelectedMember)
            .expect("attached Select member");
        match (member.presence(), expected_member) {
            (HirSourcePresence::Present(HirSourceSite::Span(actual)), Some(_)) => {
                assert_eq!(actual, &attached_member);
            }
            (HirSourcePresence::Present(HirSourceSite::Insertion(actual)), None) => {
                assert_eq!(actual.offset(), attached_member.range().start());
            }
            _ => panic!("Select member site disagrees with member payload"),
        }

        if ordinal >= 2 {
            let tried = expression(&module, select.target());
            assert!(matches!(
                tried.kind(),
                HirExprKind::Try(tried) if tried.form() == HirTryForm::PostfixQuestion
            ));
            for role in [HirExprSourceRole::Operand, HirExprSourceRole::Operator] {
                let source = module
                    .source_site(
                        parsed.document().identity(),
                        HirSourceQuery::Expr {
                            owner: select.target(),
                            role,
                        },
                    )
                    .expect("postfix Try source remains queryable");
                assert!(matches!(
                    source.presence(),
                    HirSourcePresence::Present(HirSourceSite::Span(_))
                ));
            }
        }
    }

    assert!(matches!(
        expression(&module, owners[0]).state(),
        HirPoisonState::Clean
    ));
    assert_eq!(
        expression(&module, owners[1]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::SelectedMember,
        })
    );
    assert!(matches!(
        expression(&module, owners[2]).state(),
        HirPoisonState::Clean
    ));
    assert_eq!(
        expression(&module, owners[3]).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::SelectedMember,
        })
    );

    let recovery = recovery_diagnostics(&module);
    assert_eq!(recovery.len(), 2);
    assert_eq!(
        recovery
            .iter()
            .map(|diagnostic| diagnostic.owner())
            .collect::<Vec<_>>(),
        vec![
            SyntheticOwner::Expr(owners[1]),
            SyntheticOwner::Expr(owners[3]),
        ]
    );
    assert!(recovery.iter().all(|diagnostic| matches!(
        diagnostic.primary_role(),
        HirRecoveryPrimary::Query(HirSourceQuery::Expr {
            role: HirExprSourceRole::SelectedMember,
            ..
        })
    )));
}

#[test]
fn attached_e13_select_suppresses_target_propagation_but_keeps_missing_member_obligation() {
    let parsed = parsed_source(
        "select-propagation",
        &["(target.).member".into(), "(target.).".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let named_outer = select(&module, owners[0]);
    let missing_outer = select(&module, owners[1]);
    for owner in owners.iter().copied() {
        assert_eq!(
            expression(&module, owner).state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::RecoveredChild {
                    role: HirExprSourceRole::Target,
                },
            ))
        );
    }
    assert!(matches!(
        named_outer.member(),
        HirSelectedMember::Name(member) if member.as_str() == "member"
    ));
    assert!(matches!(missing_outer.member(), HirSelectedMember::Missing));

    let recovery_owners = recovery_diagnostics(&module)
        .into_iter()
        .map(HirRecoveryDiagnostic::owner)
        .collect::<Vec<_>>();
    assert!(!recovery_owners.contains(&SyntheticOwner::Expr(owners[0])));
    assert!(recovery_owners.contains(&SyntheticOwner::Expr(named_outer.target())));
    assert!(recovery_owners.contains(&SyntheticOwner::Expr(missing_outer.target())));
    assert!(recovery_owners.contains(&SyntheticOwner::Expr(owners[1])));
    assert_eq!(recovery_owners.len(), 3);
}

#[test]
fn e13_trivia_preserves_missing_insertions_and_descendant_diagnostics() {
    let parsed = parsed_source(
        "select-trivia",
        &[
            "target.   ".into(),
            "target./*c*/".into(),
            "(target.   ).member".into(),
            "(target./*c*/).member".into(),
        ],
    );
    assert!(parsed.diagnostics().is_empty());
    let (module, owners, attached) = lower_and_publish(&parsed);

    for (ordinal, relative_offset) in [(0, 10), (1, 12)] {
        assert!(matches!(
            select(&module, owners[ordinal]).member(),
            HirSelectedMember::Missing
        ));
        let lookup = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: owners[ordinal],
                    role: HirExprSourceRole::SelectedMember,
                },
            )
            .expect("trivia-retained missing member source");
        assert!(matches!(
            lookup.presence(),
            HirSourcePresence::Present(HirSourceSite::Insertion(insertion))
                if insertion.offset()
                    == attached[ordinal].whole_source_span().range().start() + relative_offset
        ));
    }

    for (ordinal, inner_relative_offset) in [(2, 11), (3, 13)] {
        let outer = select(&module, owners[ordinal]);
        assert!(matches!(
            outer.member(),
            HirSelectedMember::Name(member) if member.as_str() == "member"
        ));
        let inner = select(&module, outer.target());
        assert!(matches!(inner.member(), HirSelectedMember::Missing));
        let lookup = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: outer.target(),
                    role: HirExprSourceRole::SelectedMember,
                },
            )
            .expect("nested trivia-retained missing member source");
        assert!(matches!(
            lookup.presence(),
            HirSourcePresence::Present(HirSourceSite::Insertion(insertion))
                if insertion.offset()
                    == attached[ordinal].whole_source_span().range().start()
                        + inner_relative_offset
        ));
        assert!(
            recovery_diagnostics(&module)
                .iter()
                .any(|diagnostic| diagnostic.owner() == SyntheticOwner::Expr(outer.target()))
        );
        assert!(
            !recovery_diagnostics(&module)
                .iter()
                .any(|diagnostic| diagnostic.owner() == SyntheticOwner::Expr(owners[ordinal]))
        );
    }
}

#[test]
fn e13_negative_forms_never_create_an_invalid_selected_member() {
    for (fixture, expected_root) in [
        (".member", "short-variant"),
        ("target..member", "range"),
        ("target..", "range"),
    ] {
        let parsed = parsed_source(
            &format!("select-negative-{expected_root}"),
            &[fixture.into()],
        );
        let (module, owners, _) = lower_and_publish(&parsed);
        assert!(matches!(
            (expected_root, expression(&module, owners[0]).kind()),
            ("short-variant", HirExprKind::ShortVariant(_)) | ("range", HirExprKind::Range(_))
        ));
        assert!(
            module
                .arenas()
                .expressions()
                .try_iter(module.slots())
                .expect("negative expression inventory")
                .all(|(_, payload)| !matches!(payload.kind(), HirExprKind::Select(_)))
        );
    }
}

#[test]
fn e13_numeric_member_recovery_keeps_the_inner_select_and_generic_outer_error() {
    for (document_id, fixture, insertion) in [
        ("select-numeric-member", "target.42", 7),
        ("select-spaced-numeric-member", "target. 42", 8),
    ] {
        let parsed = parsed_source(document_id, &[fixture.into()]);
        let (module, owners, attached) = lower_and_publish(&parsed);
        let outer = owners[0];
        assert_eq!(module.status(), HirModuleStatus::Recovered);
        assert!(matches!(
            (expression(&module, outer).kind(), expression(&module, outer).state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::Generic(
                        HirGenericExprIssue::UnclassifiedSyntax
                    )
                ))
            ) if error.issue() == HirGenericExprIssue::UnclassifiedSyntax
        ));

        let [prefix] = attached[0].children() else {
            panic!("numeric member recovery must retain one attached prefix: {fixture}");
        };
        let prefix = prefix
            .authored_semantic()
            .expect("numeric member prefix access")
            .expect("numeric member authored prefix");
        let expressions = module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .expect("numeric member expression inventory")
            .collect::<Vec<_>>();
        assert_eq!(expressions.len(), 3, "{fixture}");
        let (inner, inner_payload) = expressions
            .iter()
            .find(|(_, payload)| matches!(payload.kind(), HirExprKind::Select(_)))
            .map(|(owner, payload)| (*owner, *payload))
            .expect("numeric member inner Select owner");
        let HirExprKind::Select(select) = inner_payload.kind() else {
            unreachable!("selected inventory entry")
        };
        assert!(matches!(select.member(), HirSelectedMember::Missing));
        assert!(matches!(
            expression(&module, select.target()).kind(),
            HirExprKind::Path(_)
        ));
        assert_eq!(
            inner_payload.state(),
            &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::SelectedMember,
            })
        );
        assert!(matches!(
            module
                .slots()
                .resolve(inner)
                .expect("numeric member inner Select slot")
                .origin(),
            HirOrigin::Source(source) if source.syntax() == prefix.id()
        ));

        let selected_member = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: inner,
                    role: HirExprSourceRole::SelectedMember,
                },
            )
            .expect("numeric member selected-member source");
        assert!(matches!(
            selected_member.presence(),
            HirSourcePresence::Present(HirSourceSite::Insertion(point))
                if point.offset()
                    == attached[0].whole_source_span().range().start() + insertion
        ));
        let outer_recovery = module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner: outer,
                    role: HirExprSourceRole::Recovery,
                },
            )
            .expect("numeric member outer recovery source");
        assert!(matches!(
            outer_recovery.presence(),
            HirSourcePresence::Present(HirSourceSite::Span(span))
                if span.range().start()
                    == attached[0].whole_source_span().range().start() + insertion
                    && span.range().end() == attached[0].whole_source_span().range().end()
        ));

        let diagnostics = recovery_diagnostics(&module);
        assert_eq!(diagnostics.len(), 2, "{fixture}");
        assert!(has_recovery_query(
            &module,
            inner,
            HirExprSourceRole::SelectedMember
        ));
        assert!(has_recovery_query(
            &module,
            outer,
            HirExprSourceRole::Recovery
        ));
    }
}

#[test]
fn e13_selected_member_name_bytes_exact_and_one_over_is_atomic() {
    let maximum = HirLimit::NameBytes.maximum();
    let exact_name = "a".repeat(maximum);
    let exact = parsed_source("select-name-exact", &[format!("target.{exact_name}")]);
    let (module, owners, _) = lower_and_publish(&exact);
    assert_eq!(module.status(), HirModuleStatus::Clean);
    assert!(matches!(
        select(&module, owners[0]).member(),
        HirSelectedMember::Name(name) if name.as_str().len() == maximum
    ));

    let missing = parsed_source("select-name-missing", &["target.".into()]);
    let (module, owners, _) = lower_and_publish(&missing);
    assert!(matches!(
        select(&module, owners[0]).member(),
        HirSelectedMember::Missing
    ));

    let one_over_name = "a".repeat(maximum + 1);
    let one_over = parsed_source("select-name-one-over", &[format!("target.{one_over_name}")]);
    let attached = attached_expressions(&one_over).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 NameBytes database");
    let mut transaction = stage(&database, &one_over);
    let scope = allocate_module_scope(&mut transaction, &one_over);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::NameBytes
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&one_over)).is_none());
}

#[test]
fn e13_recovery_diagnostic_deltas_enforce_exact_and_one_over_limits() {
    for (document_id, prefill, expression, expected_delta) in [
        ("select-diagnostic-delta1-exact", 1_023, "target.", 1),
        ("select-diagnostic-delta2-exact", 1_022, "(target.).", 2),
        ("select-diagnostic-delta0-exact", 1_024, "target.member", 0),
    ] {
        let parsed = parsed_with_syntax_diagnostics(document_id, prefill, expression);
        assert_eq!(parsed.diagnostics().len(), prefill);
        let (module, _, _) = lower_and_publish(&parsed);
        assert_eq!(module.diagnostics().len(), prefill + expected_delta);
    }

    for (document_id, prefill, expression) in [
        ("select-diagnostic-delta1-over", 1_024, "target."),
        ("select-diagnostic-delta2-over", 1_023, "(target.)."),
    ] {
        let parsed = parsed_with_syntax_diagnostics(document_id, prefill, expression);
        assert_eq!(parsed.diagnostics().len(), prefill);
        let attached = attached_expressions(&parsed).pop().unwrap();
        let mut database = HirDatabase::try_new().expect("E13 diagnostic-limit database");
        let mut transaction = stage(&database, &parsed);
        let scope = allocate_module_scope(&mut transaction, &parsed);
        transaction
            .lower_attached_expression(&attached, scope)
            .expect("E13 diagnostic one-over stages before freeze");
        assert!(matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Limit(error))
                if error.limit() == HirLimit::Diagnostics
                    && error.observed() == HirLimit::Diagnostics.maximum() + 1
                    && error.maximum() == HirLimit::Diagnostics.maximum()
        ));
        assert!(database.current(&module_key(&parsed)).is_none());
    }
}

#[test]
fn e13_source_query_role_precedence_and_revision_checks_use_the_select_owner() {
    let parsed = parsed_source("select-query-precedence", &["target.member".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    let owner = owners[0];
    let foreign = parsed_source("select-query-foreign", &["target.member".into()]);

    for role in [HirExprSourceRole::Recovery, HirExprSourceRole::Index] {
        assert_eq!(
            module.source_site(
                foreign.document().identity(),
                HirSourceQuery::Expr { owner, role },
            ),
            Err(HirSourceQueryError::ExprRoleNotApplicable { owner, role })
        );
    }

    assert!(matches!(
        module.source_site(
            foreign.document().identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Target,
            },
        ),
        Err(HirSourceQueryError::WrongSourceDocument { expected, actual })
            if expected == *parsed.document().identity().id()
                && actual == *foreign.document().identity().id()
    ));

    let retained_len = parsed.document().text().len();
    let stale = SourceDocument::try_new(
        parsed.document().identity().id().clone(),
        parsed.document().display_name().clone(),
        "x".repeat(retained_len),
    )
    .expect("same-document stale revision");
    assert!(matches!(
        module.source_site(
            stale.identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Target,
            },
        ),
        Err(HirSourceQueryError::StaleSourceRevision { expected, actual })
            if expected == parsed.document().identity().revision()
                && actual == stale.identity().revision()
    ));

    let wrong_length = identity_with_length(
        parsed.document(),
        parsed
            .document()
            .identity()
            .source_len()
            .checked_add(1)
            .expect("E13 test source length"),
    );
    assert_eq!(
        module.source_site(
            &wrong_length,
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Target,
            },
        ),
        Err(HirSourceQueryError::SourceLengthMismatch {
            expected: parsed.document().identity().source_len(),
            actual: wrong_length.source_len(),
        })
    );

    let target = select(&module, owner).target();
    let role = HirExprSourceRole::PathSegment { ordinal: 1 };
    assert_eq!(
        module.source_site(
            foreign.document().identity(),
            HirSourceQuery::Expr {
                owner: target,
                role,
            },
        ),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
            owner: target,
            role,
            length: 1,
        })
    );
}

#[test]
fn e13_failed_freeze_retries_with_stable_owner_and_deduplicates_repeated_lowering() {
    let parsed = parsed_source("select-retry-dedup", &["target.".into()]);
    let attached = attached_expressions(&parsed).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 retry database");

    let mut failed = stage(&database, &parsed);
    let failed_scope = allocate_module_scope(&mut failed, &parsed);
    let failed_owner = failed
        .lower_attached_expression(&attached, failed_scope)
        .expect("valid E13 prefix before diagnostic tamper");
    let duplicate = failed
        .diagnostics
        .iter()
        .find_map(|diagnostic| match diagnostic {
            HirDiagnostic::Recovery(diagnostic) => Some(diagnostic.clone()),
            HirDiagnostic::Syntax(_) => None,
        })
        .expect("missing-member recovery diagnostic");
    failed.stage_recovery_diagnostic(duplicate);
    assert!(matches!(
        failed.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
    assert!(database.current(&module_key(&parsed)).is_none());

    let mut retry = stage(&database, &parsed);
    let retry_scope = allocate_module_scope(&mut retry, &parsed);
    let first = retry
        .lower_attached_expression(&attached, retry_scope)
        .expect("E13 retry");
    let reused = retry
        .lower_attached_expression(&attached, retry_scope)
        .expect("repeated E13 lowering reuses the owner");
    assert_eq!(failed_owner, first);
    assert_eq!(first, reused);

    let module = retry
        .finish(&mut database)
        .expect("E13 retry publication")
        .into_module();
    assert_eq!(
        recovery_diagnostics(&module)
            .into_iter()
            .map(HirRecoveryDiagnostic::owner)
            .collect::<Vec<_>>(),
        vec![SyntheticOwner::Expr(first)]
    );
    for role in [HirExprSourceRole::Target, HirExprSourceRole::SelectedMember] {
        assert!(
            module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner: first, role },
                )
                .is_ok()
        );
    }
}

#[test]
fn e13_component_insertion_order_is_canonical_at_projection_and_freeze() {
    let parsed = parsed_source("select-component-order", &["target.member".into()]);
    let (canonical, canonical_owners, _) = lower_and_publish(&parsed);
    let canonical_owner = canonical_owners[0];
    let canonical_target = select(&canonical, canonical_owner).target();

    let attached = attached_expressions(&parsed)
        .pop()
        .expect("attached Select expression");
    let [target_child] = attached.children() else {
        panic!("Select retains exactly one target child");
    };
    let target_attached = target_child
        .authored_semantic()
        .expect("attached Select target access")
        .expect("authored Select target");

    let mut database = HirDatabase::try_new().expect("component-order HIR database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    let reservation = {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .reserve_source(
                slots,
                attached.id(),
                HirSourceSite::Span(attached.whole_source_span()),
            )
            .expect("Select owner reservation")
    };
    let owner = reservation.id();
    assert_eq!(owner.raw().slot(), canonical_owner.raw().slot());

    let selected_member_query = HirSourceQuery::Expr {
        owner,
        role: HirExprSourceRole::SelectedMember,
    };
    let selected_member_site = HirSourceSite::Span(
        attached
            .component(ExpressionComponentRole::SelectedMember)
            .expect("selected-member source"),
    );
    let selected_member = match attached.projection() {
        ExpressionProjection::Select(SyntaxSelectedMember::Name(member)) => {
            HirSelectedMember::Name(name(member).expect("parser-owned selected member projection"))
        }
        _ => panic!("fixture retains one named Select projection"),
    };
    transaction
        .source_components()
        .inject_component_for_test(&selected_member_query, selected_member_site.clone())
        .expect("SelectedMember can be staged before Target");

    let target = transaction
        .lower_attached_expression(&target_attached, scope)
        .expect("Select target lowering after SelectedMember staging");
    assert_eq!(target.raw().slot(), canonical_target.raw().slot());
    let kind = HirExprKind::Select(HirSelectExpr::new(target, selected_member));
    transaction
        .source_components()
        .stage_attached_expression(&parsed, owner, &attached, &kind)
        .expect("central manifest accepts the reversed component insertion order");
    let payload = HirExpr::try_new(scope, kind, HirPoisonState::Clean)
        .expect("valid reversed-order Select payload");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .finalize(slots, reservation, payload)
            .expect("reversed-order Select finalization");
    }

    let perturbed = transaction
        .finish(&mut database)
        .expect("central freeze accepts the reversed component insertion order")
        .into_module();
    assert_eq!(
        select(&perturbed, owner).member(),
        select(&canonical, canonical_owner).member()
    );
    assert_eq!(
        expression(&perturbed, owner).state(),
        expression(&canonical, canonical_owner).state()
    );
    assert_eq!(
        expression(&perturbed, target).kind(),
        expression(&canonical, canonical_target).kind()
    );
    assert_eq!(
        expression(&perturbed, target).state(),
        expression(&canonical, canonical_target).state()
    );
    for role in [HirExprSourceRole::Target, HirExprSourceRole::SelectedMember] {
        let perturbed_query = HirSourceQuery::Expr { owner, role };
        let canonical_query = HirSourceQuery::Expr {
            owner: canonical_owner,
            role,
        };
        assert_eq!(
            perturbed.source_components().requirement(&perturbed_query),
            canonical.source_components().requirement(&canonical_query)
        );
        assert_eq!(
            perturbed
                .source_site(parsed.document().identity(), perturbed_query)
                .expect("perturbed Select source"),
            canonical
                .source_site(parsed.document().identity(), canonical_query)
                .expect("canonical Select source")
        );
    }
    assert_eq!(
        perturbed
            .source_site(parsed.document().identity(), selected_member_query)
            .expect("reversed-order selected-member source")
            .presence(),
        HirSourcePresence::Present(&selected_member_site)
    );
}

#[test]
fn e13_freeze_rejects_payload_diagnostic_and_source_tampering() {
    let payload = parsed_source("select-payload-tamper", &["target.member".into()]);
    let attached = attached_expressions(&payload).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 payload-tamper database");
    let mut transaction = stage(&database, &payload);
    let scope = allocate_module_scope(&mut transaction, &payload);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid E13 payload prefix");
    let target = {
        let (slots, arenas) = transaction.storage_mut();
        let staged = arenas
            .expressions()
            .resolve_staged(slots, owner)
            .expect("staged E13 payload");
        let HirExprKind::Select(select) = staged.kind() else {
            panic!("staged E13 Select payload");
        };
        select.target()
    };
    let substituted = crate::leaf::HirName::try_new("different".into())
        .expect("test-only valid E13 member substitution");
    let replacement = HirExpr::try_new(
        scope,
        HirExprKind::Select(HirSelectExpr::new(
            target,
            HirSelectedMember::Name(substituted),
        )),
        HirPoisonState::Clean,
    )
    .expect("same-module forged E13 payload");
    {
        let (slots, arenas) = transaction.storage_mut();
        arenas
            .expressions()
            .revise_finalized(slots, owner, replacement)
            .expect("test-only E13 payload substitution");
    }
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&payload)).is_none());

    let diagnostic = parsed_source("select-diagnostic-tamper", &["target.".into()]);
    let attached = attached_expressions(&diagnostic).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 diagnostic-tamper database");
    let mut transaction = stage(&database, &diagnostic);
    let scope = allocate_module_scope(&mut transaction, &diagnostic);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid E13 diagnostic prefix");
    transaction.diagnostics.clear();
    transaction.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(owner),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::Target,
        }),
        HirSourceSite::Span(
            attached
                .component(ExpressionComponentRole::Target)
                .expect("E13 target source"),
        ),
    ));
    let error = transaction
        .finish(&mut database)
        .err()
        .expect("E13 diagnostic tamper must fail freeze");
    assert!(
        matches!(
            error,
            HirLowerFailure::Invariant(HirInvariantFailure::InvalidModuleDiagnostics)
        ),
        "unexpected E13 diagnostic-tamper failure: {error:?}"
    );
    assert!(database.current(&module_key(&diagnostic)).is_none());

    let source = parsed_source("select-source-tamper", &["target.member".into()]);
    let attached = attached_expressions(&source).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 source-tamper database");
    let mut transaction = stage(&database, &source);
    let scope = allocate_module_scope(&mut transaction, &source);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid E13 source prefix");
    let query = HirSourceQuery::Expr {
        owner,
        role: HirExprSourceRole::SelectedMember,
    };
    assert_eq!(
        transaction
            .source_components()
            .inject_component_for_test(&query, HirSourceSite::Span(attached.whole_source_span()),),
        Err(HirSourceCommitInvariantError::ConflictingComponent { query })
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&module_key(&source)).is_none());
}

#[test]
fn e13_freeze_enforces_exact_missing_and_descendant_diagnostic_obligations() {
    let missing = parsed_source("select-missing-diagnostic", &["target.".into()]);
    let attached = attached_expressions(&missing).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 missing-diagnostic database");
    let mut transaction = stage(&database, &missing);
    let scope = allocate_module_scope(&mut transaction, &missing);
    transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid missing-member prefix");
    transaction.diagnostics.clear();
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
    assert!(database.current(&module_key(&missing)).is_none());

    let clean = parsed_source("select-extra-diagnostic", &["target.member".into()]);
    let attached = attached_expressions(&clean).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 extra-diagnostic database");
    let mut transaction = stage(&database, &clean);
    let scope = allocate_module_scope(&mut transaction, &clean);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid clean Select prefix");
    transaction.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(owner),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::SelectedMember,
        }),
        HirSourceSite::Span(
            attached
                .component(ExpressionComponentRole::SelectedMember)
                .expect("clean selected-member source"),
        ),
    ));
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
    assert!(database.current(&module_key(&clean)).is_none());

    let wrong_site = parsed_source("select-wrong-diagnostic-site", &["target.".into()]);
    let attached = attached_expressions(&wrong_site).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 wrong-site database");
    let mut transaction = stage(&database, &wrong_site);
    let scope = allocate_module_scope(&mut transaction, &wrong_site);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid wrong-site Select prefix");
    transaction.diagnostics.clear();
    let wrong_insertion = HirInsertionPoint::try_new(
        wrong_site.document(),
        attached.whole_source_span().range().start(),
    )
    .expect("wrong but valid E13 insertion");
    transaction.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(owner),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::SelectedMember,
        }),
        HirSourceSite::Insertion(wrong_insertion),
    ));
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
    assert!(database.current(&module_key(&wrong_site)).is_none());

    let named = parsed_source(
        "select-unexpected-propagation-diagnostic",
        &["(target.).member".into()],
    );
    let attached = attached_expressions(&named).pop().unwrap();
    let mut database = HirDatabase::try_new().expect("E13 named-propagation database");
    let mut transaction = stage(&database, &named);
    let scope = allocate_module_scope(&mut transaction, &named);
    let owner = transaction
        .lower_attached_expression(&attached, scope)
        .expect("valid recovered-target named Select prefix");
    transaction.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
        SyntheticOwner::Expr(owner),
        HirRecoveryPrimary::query(HirSourceQuery::Expr {
            owner,
            role: HirExprSourceRole::Target,
        }),
        HirSourceSite::Span(
            attached
                .component(ExpressionComponentRole::Target)
                .expect("outer Select target source"),
        ),
    ));
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleDiagnostics
        ))
    ));
    assert!(database.current(&module_key(&named)).is_none());

    for keep_outer in [false, true] {
        let parsed = parsed_source(
            if keep_outer {
                "select-missing-descendant-diagnostic"
            } else {
                "select-missing-outer-diagnostic"
            },
            &["(target.).".into()],
        );
        let attached = attached_expressions(&parsed).pop().unwrap();
        let mut database = HirDatabase::try_new().expect("E13 nested-missing database");
        let mut transaction = stage(&database, &parsed);
        let scope = allocate_module_scope(&mut transaction, &parsed);
        let outer = transaction
            .lower_attached_expression(&attached, scope)
            .expect("valid nested missing Select prefix");
        transaction
            .diagnostics
            .retain(|diagnostic| match diagnostic {
                HirDiagnostic::Syntax(_) => true,
                HirDiagnostic::Recovery(diagnostic) => {
                    (diagnostic.owner() == SyntheticOwner::Expr(outer)) == keep_outer
                }
            });
        assert!(matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidModuleDiagnostics
            ))
        ));
        assert!(database.current(&module_key(&parsed)).is_none());
    }
}
