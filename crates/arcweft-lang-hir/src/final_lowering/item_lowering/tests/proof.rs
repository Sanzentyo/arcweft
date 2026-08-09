use super::*;

use arcweft_id::PublicId;
use arcweft_lang_syntax::attachment::node::LetStatementKind;
use arcweft_lang_syntax::attachment::{
    AttachedProofBody, BlockTailNode, LetInitializerNode, TypedItemNode,
};

use crate::expr::{HirExprKind, HirGenericExprIssue, HirPoisonState, HirRecoveryIssue};
use crate::item::{HirProof, HirProofBody, ProofTrust};
use crate::pattern::{HirPatternBinding, HirPatternBindingIssue, HirPatternKind};
use crate::project::{HirProjectBuilder, HirProjectExecutionError, HirProjectModule};
use crate::scope::LocalLookup;
use crate::source_index::{
    HirDeclarationSourceRole, HirExprSourceRole, HirItemSourceRole, HirSourceOwnerStatus,
    HirSourcePresence, HirSourceQuery, HirSourceQueryError, HirStmtSourceRole,
};
use crate::stmt::{
    HirAssertionMode, HirStmt, HirStmtChildRole, HirStmtKind, HirStmtPoisonState,
    HirStmtRecoveryIssue,
};
use crate::type_ref::{HirGenericTypeIssue, HirType, HirTypeKind};

fn proof(module: &HirModule, ordinal: usize) -> (crate::identity::ItemId, &HirItem, &HirProof) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Proof(proof) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Proof")
    };
    (owner, item, proof)
}

fn assert_proof_body_scope(
    module: &HirModule,
    parsed: &ParsedSource,
    ordinal: usize,
    owner: crate::identity::ItemId,
    proof: &HirProof,
) -> ScopeId {
    let attached = parsed
        .items()
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            TypedItemNode::Proof(proof) => proof.semantics().ok(),
            _ => None,
        })
        .nth(ordinal)
        .expect("attached Proof body");
    let body_syntax = attached.body().syntax();
    assert_eq!(
        body_syntax.kind(),
        arcweft_lang_syntax::grammar::SyntaxKind::ProofBody
    );
    let body_scope = proof.body().scope();
    let payload = module
        .arenas()
        .scopes()
        .resolve(module.slots(), body_scope)
        .unwrap();
    assert_eq!(payload.kind(), HirScopeKind::Proof);
    assert_eq!(payload.parent(), Some(proof.callable_scope()));
    assert_eq!(payload.owner(), &HirScopeOwner::Item(owner));
    let metadata = module.slots().resolve(body_scope).unwrap();
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Source(source) if source.syntax() == body_syntax.id()
    ));
    assert_eq!(
        metadata.source_site(),
        &HirSourceSite::Span(body_syntax.source_span())
    );
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), proof.callable_scope())
        .unwrap();
    assert_eq!(
        callable.children(),
        [proof.requires_scope(), proof.ensures_scope(), body_scope]
    );
    body_scope
}

fn assert_proof_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-proof-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&mut transaction, owner);
    assert!(
        matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "Proof freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

#[test]
fn proof_trust_is_semantic_while_exact_coordinates_stay_in_the_source_index() {
    let source = concat!(
        "proof verified() = ()\n",
        "#[verify.trusted(reason = \"  reviewed ✓  \")]\n",
        "proof trusted() = ()\n",
        "#[verify.trusted(reason = 1)]\n",
        "proof recovered() = ()\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-proof-trust", source);
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.proof.trusted.reason_not_string" })
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (verified_owner, verified_item, verified) = proof(&module, 0);
    assert_eq!(verified.trust(), &ProofTrust::Verified);
    assert_eq!(verified_item.state(), &HirItemPoisonState::Clean);

    let (trusted_owner, trusted_item, trusted) = proof(&module, 1);
    let ProofTrust::Trusted { reason } = trusted.trust() else {
        panic!("second Proof must retain typed trust")
    };
    assert_eq!(reason.as_str(), "  reviewed ✓  ");
    assert_eq!(trusted_item.state(), &HirItemPoisonState::Clean);

    let (_, recovered_item, recovered) = proof(&module, 2);
    assert_eq!(recovered.trust(), &ProofTrust::Recovery);
    assert_eq!(
        recovered_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );

    let attached = parsed.items().unwrap();
    let TypedItemNode::Proof(trusted_syntax) = &attached[1] else {
        panic!("second attached item must be a Proof")
    };
    let trusted_syntax = trusted_syntax.semantics().unwrap();
    for (role, expected) in [
        (
            HirDeclarationSourceRole::ProofTrustAttribute,
            trusted_syntax
                .trust_attribute_source_span()
                .expect("trusted attribute source"),
        ),
        (
            HirDeclarationSourceRole::ProofTrustReason,
            trusted_syntax
                .trust_reason_source_span()
                .expect("trusted reason source"),
        ),
    ] {
        let query = HirSourceQuery::Item {
            owner: trusted_owner,
            role: HirItemSourceRole::Declaration(role),
        };
        let lookup = module
            .source_site(parsed.document().identity(), query)
            .expect("trusted Proof source role");
        assert!(matches!(
            lookup.presence(),
            HirSourcePresence::Present(HirSourceSite::Span(actual)) if actual == expected
        ));
    }

    assert!(matches!(
        module.source_site(
            parsed.document().identity(),
            HirSourceQuery::Item {
                owner: verified_owner,
                role: HirItemSourceRole::Declaration(
                    HirDeclarationSourceRole::ProofTrustAttribute,
                ),
            },
        ),
        Err(HirSourceQueryError::ItemRoleNotApplicable { .. })
    ));
}

#[test]
fn proof_parameter_and_let_policy_retain_poisoned_typed_locals() {
    let source = concat!(
        "proof constrained(mut parameter: Bool) {\n",
        "    let result = parameter;\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-proof-local-policy", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, proof) = proof(&module, 0);
    assert_eq!(
        proof.return_semantic_class(),
        crate::proof_return::HirProofReturnSemanticClass::Unit
    );
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );

    let parameter = module
        .arenas()
        .locals()
        .resolve(module.slots(), proof.parameters()[0].locals()[0])
        .unwrap();
    assert!(parameter.is_mutable_binding());
    assert!(parameter.is_poisoned());

    let HirProofBody::Block { statements, .. } = proof.body() else {
        panic!("Proof block body")
    };
    let [statement] = statements.as_ref() else {
        panic!("one exact Proof let")
    };
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), *statement)
        .unwrap();
    let HirStmtKind::Let { locals, .. } = statement.kind() else {
        panic!("Proof let statement")
    };
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Pattern,
        })
    );
    let [local] = locals.as_ref() else {
        panic!("one exact Proof let Local")
    };
    let local = module
        .arenas()
        .locals()
        .resolve(module.slots(), *local)
        .unwrap();
    assert_eq!(local.name().as_str(), "result");
    assert!(local.is_poisoned());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the malformed Proof test asserts one complete recovery, source-query, project, and retry matrix"
)]
fn malformed_proof_body_stays_queryable_while_following_proof_keeps_clean_identity() {
    let source = concat!(
        "proof broken() { let value: Int = ; ??? }\n",
        "proof following() = ()\n",
    );
    let parsed = parse(
        "arcweft-test://proof/final-hir-malformed-body-following-proof",
        source,
    );
    let attached_items = parsed.items().unwrap();
    let [
        TypedItemNode::Proof(broken_syntax),
        TypedItemNode::Proof(following_syntax),
    ] = attached_items.as_slice()
    else {
        panic!("malformed Proof must not consume the following clean Proof")
    };
    let broken_syntax = broken_syntax.semantics().unwrap();
    let AttachedProofBody::Block { block, .. } = broken_syntax.body() else {
        panic!("malformed Proof body must remain a typed block")
    };
    let attached_statements = block.statements().unwrap();
    let [attached_statement] = attached_statements.as_slice() else {
        panic!("malformed Proof body must retain one typed let statement")
    };
    let attached_let = attached_statement.cast::<LetStatementKind>().unwrap();
    let Some(LetInitializerNode::Missing(attached_initializer)) =
        attached_let.initializer().unwrap()
    else {
        panic!("missing initializer must retain its attached insertion owner")
    };
    let BlockTailNode::Expression(attached_tail) = block.tail().unwrap() else {
        panic!("malformed authored tail must not become an omitted tail")
    };

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert!(!module.is_executable());

    let (broken_owner, broken_item, broken) = proof(&module, 0);
    assert_eq!(
        broken_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let HirProofBody::Block {
        statements, tail, ..
    } = broken.body()
    else {
        panic!("malformed Proof must retain its HIR block body")
    };
    let [statement_id] = statements.as_ref() else {
        panic!("malformed Proof must retain one HIR let statement")
    };
    assert!(matches!(
        module.slots().resolve(*statement_id).unwrap().origin(),
        HirOrigin::Source(source) if source.syntax() == attached_statement.id()
    ));
    let statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), *statement_id)
        .unwrap();
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::Initializer,
        })
    );
    let HirStmtKind::Let { initializer, .. } = statement.kind() else {
        panic!("missing initializer must remain a typed HIR let")
    };
    let initializer_payload = module
        .arenas()
        .expressions()
        .resolve(module.slots(), *initializer)
        .unwrap();
    assert!(matches!(
        initializer_payload.kind(),
        HirExprKind::Error(error)
            if error.issue() == HirGenericExprIssue::TransactionalChildFailure
    ));
    assert_eq!(
        initializer_payload.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Operand,
        })
    );

    let tail_payload = module
        .arenas()
        .expressions()
        .resolve(module.slots(), *tail)
        .unwrap();
    assert!(matches!(tail_payload.kind(), HirExprKind::Try(_)));
    assert_eq!(
        tail_payload.state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            crate::expr::HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Operand,
            },
        ))
    );
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Source(source) if source.syntax() == attached_tail.id()
    ));

    for (query, expected_presence) in [
        (
            HirSourceQuery::Stmt {
                owner: *statement_id,
                role: HirStmtSourceRole::Whole,
            },
            HirSourceSite::Span(attached_statement.source_span()),
        ),
        (
            HirSourceQuery::Expr {
                owner: *initializer,
                role: HirExprSourceRole::Whole,
            },
            HirSourceSite::Insertion(
                crate::source_index::HirInsertionPoint::try_new(
                    parsed.document(),
                    attached_initializer.range().start(),
                )
                .unwrap(),
            ),
        ),
        (
            HirSourceQuery::Expr {
                owner: *tail,
                role: HirExprSourceRole::Whole,
            },
            HirSourceSite::Span(attached_tail.source_span()),
        ),
    ] {
        let lookup = module
            .source_site(parsed.document().identity(), query)
            .expect("poisoned typed owner remains source-queryable");
        assert_eq!(lookup.owner_status(), HirSourceOwnerStatus::Poisoned);
        assert_eq!(
            lookup.presence(),
            HirSourcePresence::Present(&expected_presence)
        );
    }

    let (following_owner, following_item, following) = proof(&module, 1);
    assert_ne!(broken_owner, following_owner);
    assert_eq!(following_item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        following.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "following"
    ));
    assert!(matches!(
        module.slots().resolve(following_owner).unwrap().origin(),
        HirOrigin::Source(source) if source.syntax() == following_syntax.id()
    ));
    let following_source = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Item {
                owner: following_owner,
                role: HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
            },
        )
        .expect("following clean Proof remains source-queryable");
    assert_eq!(following_source.owner_status(), HirSourceOwnerStatus::Clean);
    assert_eq!(
        following_source.presence(),
        HirSourcePresence::Present(&HirSourceSite::Span(following_syntax.source_span()))
    );

    let project_module = HirProjectModule::try_new(
        &database,
        key.package(),
        key.path(),
        module.provenance().source_identity(),
        Arc::clone(&module),
    )
    .unwrap();
    let mut builder = HirProjectBuilder::new(&database, key.package().clone());
    builder.insert_module(project_module).unwrap();
    let project = builder.finish().unwrap();
    assert_eq!(project.view().items().count(), 2);
    assert_eq!(
        project.executable_view().err(),
        Some(HirProjectExecutionError::RecoveredModule {
            module: key.path().clone(),
            snapshot: module.snapshot_id(),
        })
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the canonical Proof test asserts one complete signature/contract/body/source owner graph"
)]
fn canonical_proof_freezes_signature_contracts_proof_call_assertion_and_tail() {
    let source = concat!(
        "pub proof ordered<T>((left, right): (T, T), cmp: Comparator<T>) -> Bool\n",
        "where T: Ord\n",
        "requires cmp.is_total()\n",
        "ensures result\n",
        "{\n",
        "    lemma(left);\n",
        "    assert.prove(cmp.ready());\n",
        "    true\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-proof-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower_with_proof_return_classes(
        &mut database,
        &parsed,
        &key,
        [crate::proof_return::HirProofReturnSemanticClass::NonUnit],
    );
    let (owner, item, proof) = proof(&module, 0);
    let body_scope = assert_proof_body_scope(&module, &parsed, 0, owner, proof);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        proof.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "ordered"
    ));
    assert_eq!(proof.generic_parameters().len(), 1);
    assert_eq!(proof.parameters().len(), 2);
    assert_eq!(proof.where_predicates().len(), 1);
    assert_eq!(proof.requires().len(), 1);
    assert_eq!(proof.ensures().len(), 1);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());

    let return_type = module
        .arenas()
        .types()
        .resolve(module.slots(), proof.return_type())
        .unwrap();
    assert_eq!(return_type.scope(), proof.callable_scope());
    assert_eq!(return_type.state(), &HirPoisonState::Clean);
    assert!(matches!(
        return_type.kind(),
        HirTypeKind::Path(path)
            if path.root() == HirPathRoot::ImplicitCrate
                && path_spellings(path) == ["Bool"]
    ));

    let ensures = module
        .arenas()
        .scopes()
        .resolve(module.slots(), proof.ensures_scope())
        .unwrap();
    let [result] = ensures.locals() else {
        panic!("one exact postcondition result local")
    };
    let result = module
        .arenas()
        .locals()
        .resolve(module.slots(), *result)
        .unwrap();
    assert_eq!(result.kind(), HirLocalKind::PostconditionResult);
    assert_eq!(result.name().as_str(), "result");
    assert_eq!(result.annotation(), Some(proof.return_type()));

    let HirProofBody::Block {
        scope,
        statements,
        tail,
    } = proof.body()
    else {
        panic!("Proof block body")
    };
    assert_eq!(*scope, body_scope);
    assert_eq!(statements.len(), 2);
    let proof_call = module
        .arenas()
        .statements()
        .resolve(module.slots(), statements[0])
        .unwrap();
    let HirStmtKind::ProofCall { call } = proof_call.kind() else {
        panic!("first Proof statement must retain the ordinary call expression")
    };
    assert_eq!(proof_call.scope(), *scope);
    assert!(matches!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *call)
            .unwrap()
            .kind(),
        HirExprKind::Call(_)
    ));
    let assertion = module
        .arenas()
        .statements()
        .resolve(module.slots(), statements[1])
        .unwrap();
    assert_eq!(assertion.scope(), *scope);
    assert!(matches!(
        assertion.kind(),
        HirStmtKind::Assertion {
            mode: HirAssertionMode::Resolved(
                arcweft_lang_syntax::assertion::AssertionMode::Prove
            ),
            conditions,
        } if conditions.len() == 1
    ));
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *tail)
            .unwrap()
            .scope(),
        *scope
    );
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn proof_identity_acceptance_matrix_reaches_final_hir_as_typed_public_id() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-explicit-identity",
        concat!(
            "proof bare() = ()\n",
            "proof @proof.explicit explicit() = ()\n",
            "proof @proof:.relative relative() = ()\n",
            "proof @.short short() = ()\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    for (ordinal, expected) in [
        (0, None),
        (1, Some("proof.explicit")),
        (2, Some("proof.relative")),
        (3, Some("proof.short")),
    ] {
        let (_, _, proof) = proof(&module, ordinal);
        assert_eq!(proof.public_id().map(PublicId::as_str), expected);
    }
}

#[test]
fn wrong_family_proof_identity_survives_as_recovery_without_a_fabricated_id() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-wrong-family",
        "proof @flow.foreign foreign() = ()\n",
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.declaration.wrong_family_id")
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, proof) = proof(&module, 0);
    assert_eq!(proof.public_id(), None);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
}

#[test]
fn proof_lowering_allocates_headers_contracts_and_body_in_the_accepted_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-allocation-order",
        concat!(
            "proof ordered<T: Bound>(value: Input) -> Output\n",
            "where T: Other\n",
            "requires ready(value)\n",
            "ensures result\n",
            "= value\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower_with_proof_return_classes(
        &mut database,
        &parsed,
        &key,
        [crate::proof_return::HirProofReturnSemanticClass::NonUnit],
    );
    let (owner, item, proof) = proof(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);

    let [generic] = proof.generic_parameters() else {
        panic!("one generic parameter")
    };
    let [generic_bound] = generic.bounds() else {
        panic!("one generic bound")
    };
    let [parameter] = proof.parameters() else {
        panic!("one fixed parameter")
    };
    let [parameter_local] = parameter.locals() else {
        panic!("one fixed-parameter local")
    };
    let [where_predicate] = proof.where_predicates() else {
        panic!("one where predicate")
    };
    let [where_bound] = where_predicate.bounds() else {
        panic!("one where bound")
    };
    let [requires] = proof.requires() else {
        panic!("one requires condition")
    };
    let [ensures] = proof.ensures() else {
        panic!("one ensures condition")
    };
    let [result_local] = module
        .arenas()
        .scopes()
        .resolve(module.slots(), proof.ensures_scope())
        .unwrap()
        .locals()
    else {
        panic!("one postcondition result local")
    };
    let HirProofBody::Expression {
        scope: body_scope,
        expression: body,
    } = proof.body()
    else {
        panic!("expression Proof body")
    };

    let slots = [
        owner.raw().slot().get(),
        proof.callable_scope().raw().slot().get(),
        proof.requires_scope().raw().slot().get(),
        proof.ensures_scope().raw().slot().get(),
        body_scope.raw().slot().get(),
        generic_bound.raw().slot().get(),
        parameter.ty().raw().slot().get(),
        parameter.pattern().raw().slot().get(),
        parameter_local.raw().slot().get(),
        proof.return_type().raw().slot().get(),
        where_predicate.subject().raw().slot().get(),
        where_bound.raw().slot().get(),
        requires.raw().slot().get(),
        result_local.raw().slot().get(),
        ensures.raw().slot().get(),
        body.raw().slot().get(),
    ];
    assert!(
        slots.windows(2).all(|pair| pair[0] < pair[1]),
        "Proof allocation order diverged from the accepted transaction: {slots:?}"
    );
}

#[test]
fn proof_body_matrix_distinguishes_unit_nonunit_missing_and_expression_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-body-matrix",
        concat!(
            "proof unit() { lemma(); }\n",
            "proof required() -> Int { lemma(); }\n",
            "proof expression(value: Int) -> Int = value\n",
            "proof missing() -> Int\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower_with_proof_return_classes(
        &mut database,
        &parsed,
        &key,
        [
            crate::proof_return::HirProofReturnSemanticClass::NonUnit,
            crate::proof_return::HirProofReturnSemanticClass::NonUnit,
            crate::proof_return::HirProofReturnSemanticClass::NonUnit,
        ],
    );

    let (unit_owner, unit_item, unit) = proof(&module, 0);
    let unit_body_scope = assert_proof_body_scope(&module, &parsed, 0, unit_owner, unit);
    assert_eq!(unit_item.state(), &HirItemPoisonState::Clean);
    let unit_return = module
        .arenas()
        .types()
        .resolve(module.slots(), unit.return_type())
        .unwrap();
    assert!(matches!(unit_return.kind(), HirTypeKind::Tuple(items) if items.is_empty()));
    assert!(matches!(
        module.slots().resolve(unit.return_type()).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Item(unit_owner)
                && key.role() == SyntheticRole::ProofUnitReturn
    ));
    let HirProofBody::Block { scope, tail, .. } = unit.body() else {
        panic!("Unit Proof block")
    };
    assert_eq!(*scope, unit_body_scope);
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(*scope)
                && key.role() == SyntheticRole::ImplicitUnitTail
    ));
    assert!(!module.slots().resolve(*tail).unwrap().is_poisoned());

    let (required_owner, required_item, required) = proof(&module, 1);
    let required_body_scope =
        assert_proof_body_scope(&module, &parsed, 1, required_owner, required);
    assert_eq!(
        required_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let HirProofBody::Block { scope, tail, .. } = required.body() else {
        panic!("non-Unit Proof block")
    };
    assert_eq!(*scope, required_body_scope);
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(*scope)
                && key.role() == SyntheticRole::MissingRequiredTail
    ));
    assert!(module.slots().resolve(*tail).unwrap().is_poisoned());

    let (expression_owner, expression_item, expression) = proof(&module, 2);
    let expression_body_scope =
        assert_proof_body_scope(&module, &parsed, 2, expression_owner, expression);
    assert_eq!(expression_item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        expression.body(),
        HirProofBody::Expression { scope, .. } if *scope == expression_body_scope
    ));

    let (missing_owner, missing_item, missing) = proof(&module, 3);
    let missing_body_scope = assert_proof_body_scope(&module, &parsed, 3, missing_owner, missing);
    assert_eq!(
        missing_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert!(matches!(
        missing.body(),
        HirProofBody::Error { scope, .. } if *scope == missing_body_scope
    ));
}

#[test]
fn proof_omitted_return_is_unit() {
    let source = "proof expression() = ()\nproof block() {}\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-omitted-return-unit",
        source,
    );
    let attached = parsed.items().unwrap();
    let TypedItemNode::Proof(expression_syntax) = &attached[0] else {
        panic!("first item must be a Proof")
    };
    let expression_syntax = expression_syntax.semantics().unwrap();
    assert!(expression_syntax.authored_return().is_none());
    assert!(matches!(
        expression_syntax.body(),
        AttachedProofBody::Expression { expression, .. }
            if expression.syntax().source_text() == "()"
    ));
    let TypedItemNode::Proof(block_syntax) = &attached[1] else {
        panic!("second item must be a Proof")
    };
    let block_syntax = block_syntax.semantics().unwrap();
    assert!(block_syntax.authored_return().is_none());
    let AttachedProofBody::Block { block, .. } = block_syntax.body() else {
        panic!("second Proof must have a block body")
    };
    assert!(matches!(block.tail().unwrap(), BlockTailNode::Omitted(_)));

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    for ordinal in 0..2 {
        let (owner, item, proof) = proof(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert_eq!(
            proof.return_semantic_class(),
            crate::proof_return::HirProofReturnSemanticClass::Unit
        );
        let return_type = module
            .arenas()
            .types()
            .resolve(module.slots(), proof.return_type())
            .unwrap();
        assert!(matches!(return_type.kind(), HirTypeKind::Tuple(items) if items.is_empty()));
        assert!(matches!(
            module.slots().resolve(proof.return_type()).unwrap().origin(),
            HirOrigin::Synthetic(synthetic)
                if synthetic.owner() == SyntheticOwner::Item(owner)
                    && synthetic.role() == SyntheticRole::ProofUnitReturn
        ));
    }
    let (_, _, block) = proof(&module, 1);
    let HirProofBody::Block { scope, tail, .. } = block.body() else {
        panic!("second Proof must retain a HIR block")
    };
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(synthetic)
            if synthetic.owner() == SyntheticOwner::Scope(*scope)
                && synthetic.role() == SyntheticRole::ImplicitUnitTail
    ));
}

#[test]
fn proof_non_unit_expression_body_is_typed_once() {
    let source = "proof p() -> Int = 1\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-non-unit-expression",
        source,
    );
    let attached_items = parsed.items().unwrap();
    let TypedItemNode::Proof(attached) = &attached_items[0] else {
        panic!("expected attached Proof")
    };
    let attached = attached.semantics().unwrap();
    let AttachedProofBody::Expression {
        syntax: body_syntax,
        expression,
    } = attached.body()
    else {
        panic!("expected attached expression body")
    };
    assert_ne!(body_syntax.id(), expression.syntax().id());
    assert_eq!(expression.syntax().range(), SourceRange::new(19, 20));

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower_with_proof_return_classes(
        &mut database,
        &parsed,
        &key,
        [crate::proof_return::HirProofReturnSemanticClass::NonUnit],
    );
    let (_, item, proof) = proof(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(
        proof.return_semantic_class(),
        crate::proof_return::HirProofReturnSemanticClass::NonUnit
    );
    let HirProofBody::Expression {
        scope,
        expression: lowered,
    } = proof.body()
    else {
        panic!("expected one lowered expression body")
    };
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *lowered)
            .unwrap()
            .scope(),
        *scope
    );
    assert!(matches!(
        module.slots().resolve(*lowered).unwrap().origin(),
        HirOrigin::Source(source) if source.syntax() == expression.syntax().id()
    ));
}

#[test]
fn proof_non_unit_block_requires_tail() {
    let source = "proof p() -> Int { let x: Int = 1; }\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-required-block-tail",
        source,
    );
    let attached_items = parsed.items().unwrap();
    let TypedItemNode::Proof(attached) = &attached_items[0] else {
        panic!("expected attached Proof")
    };
    let attached = attached.semantics().unwrap();
    let AttachedProofBody::Block { block, .. } = attached.body() else {
        panic!("expected attached Proof block")
    };
    let BlockTailNode::Omitted(omitted) = block.tail().unwrap() else {
        panic!("missing non-Unit tail must remain queryable")
    };
    let close_start = source.find('}').unwrap();
    assert_eq!(omitted.range(), SourceRange::new(close_start, close_start));

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower_with_proof_return_classes(
        &mut database,
        &parsed,
        &key,
        [crate::proof_return::HirProofReturnSemanticClass::NonUnit],
    );
    let (_, item, proof) = proof(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    let HirProofBody::Block { scope, tail, .. } = proof.body() else {
        panic!("expected lowered Proof block")
    };
    assert!(module.slots().resolve(*tail).unwrap().is_poisoned());
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(synthetic)
            if synthetic.owner() == SyntheticOwner::Scope(*scope)
                && synthetic.role() == SyntheticRole::MissingRequiredTail
    ));
}

#[test]
fn requires_must_precede_ensures() {
    let source = "proof ordered()\nensures true\nrequires true\n= ()\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-contract-order",
        source,
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.contract.invalid_clause_order")
    );
    let attached_items = parsed.items().unwrap();
    let TypedItemNode::Proof(attached) = &attached_items[0] else {
        panic!("expected attached Proof")
    };
    let attached = attached.semantics().unwrap();
    assert_eq!(attached.contracts().len(), 2);
    assert!(attached.contracts()[0].is_ensures());
    assert!(attached.contracts()[1].is_requires());
    assert!(attached.contracts()[1].has_recovery());

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, proof) = proof(&module, 0);
    assert_eq!(proof.ensures().len(), 1);
    assert_eq!(proof.requires().len(), 1);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the pure-let test asserts one ordered initializer, binding, assertion, and tail owner graph"
)]
fn pure_let_initializer_precedes_binding_scope() {
    const SOURCE: &str = "proof shadow(x: Int) -> Int { let x: Int = x; x }\n";
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-let-binding-point",
        SOURCE,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower_with_proof_return_classes(
        &mut database,
        &parsed,
        &key,
        [crate::proof_return::HirProofReturnSemanticClass::NonUnit],
    );
    let (_, item, proof) = proof(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let parameter = proof.parameters()[0].locals()[0];
    let HirProofBody::Block {
        statements, tail, ..
    } = proof.body()
    else {
        panic!("expected Proof block")
    };
    let [statement] = statements.as_ref() else {
        panic!("expected one pure let")
    };
    let statement = module.resolve_stmt(*statement).unwrap();
    let HirStmtKind::Let {
        pattern,
        annotation: None,
        initializer,
        locals,
    } = statement.kind()
    else {
        panic!("expected typed pure let payload")
    };
    let [binding_local] = locals.as_ref() else {
        panic!("expected one let binding")
    };
    let pattern_payload = module.resolve_pattern(*pattern).unwrap();
    let HirPatternKind::TypedBinding {
        binding: typed_binding,
        ty: annotation,
    } = pattern_payload.kind()
    else {
        panic!("pure let annotation must remain owned once by TypedBinding")
    };
    let HirPatternBinding::Bound {
        name,
        local: pattern_local,
    } = typed_binding
    else {
        panic!("clean pure let must retain its typed binding local")
    };
    assert_eq!(name.as_str(), "x");
    assert_eq!(pattern_local, binding_local);
    assert_eq!(pattern_payload.scope(), statement.scope());
    assert_eq!(
        module.resolve_type(*annotation).unwrap().scope(),
        statement.scope()
    );
    assert_eq!(
        module.resolve_expr(*initializer).unwrap().scope(),
        statement.scope()
    );
    assert_source_backed_child(&module, *pattern);
    assert_source_backed_child(&module, *annotation);
    assert_source_backed_child(&module, *initializer);
    let binding_payload = module.resolve_local(*binding_local).unwrap();
    assert_eq!(binding_payload.pattern(), Some(*pattern));
    assert_eq!(binding_payload.annotation(), Some(*annotation));
    let reservation_slots = [
        initializer.raw().slot().get(),
        pattern.raw().slot().get(),
        annotation.raw().slot().get(),
        binding_local.raw().slot().get(),
        tail.raw().slot().get(),
    ];
    assert!(
        reservation_slots.windows(2).all(|pair| pair[0] < pair[1]),
        "pure-let allocation must lower the initializer before its one typed-binding owner: {reservation_slots:?}"
    );
    let initializer_span = match module.slots().resolve(*initializer).unwrap().source_site() {
        HirSourceSite::Span(span) => span.clone(),
        HirSourceSite::Insertion(_) => panic!("initializer must be source-backed"),
    };
    let tail_span = match module.slots().resolve(*tail).unwrap().source_site() {
        HirSourceSite::Span(span) => span.clone(),
        HirSourceSite::Insertion(_) => panic!("tail must be source-backed"),
    };
    assert_eq!(
        module.lookup_local(
            binding_payload.scope(),
            binding_payload.name(),
            initializer_span,
        ),
        Ok(LocalLookup::Found(parameter))
    );
    assert_eq!(
        module.lookup_local(binding_payload.scope(), binding_payload.name(), tail_span),
        Ok(LocalLookup::Found(*binding_local))
    );
}

#[test]
fn receiver_shaped_proof_parameter_commits_typed_retained_recovery() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-proof-receiver-parameter-recovery",
        "proof recovered(self) = ()\n",
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, proof) = proof(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingType)
    );
    let [parameter] = proof.parameters() else {
        panic!("receiver-shaped Proof parameter must remain in HIR")
    };
    assert!(parameter.locals().is_empty());
    let pattern = module
        .arenas()
        .patterns()
        .resolve(module.slots(), parameter.pattern())
        .unwrap();
    assert!(matches!(
        pattern.kind(),
        HirPatternKind::Binding(HirPatternBinding::Recovered {
            issue: HirPatternBindingIssue::InvalidName(
                crate::leaf::HirNameInvariantError::InvalidIdentifier
            )
        })
    ));
    assert!(pattern.state().is_poisoned());
    let ty = module
        .arenas()
        .types()
        .resolve(module.slots(), parameter.ty())
        .unwrap();
    assert!(matches!(
        ty.kind(),
        HirTypeKind::Recovery(error)
            if error.issue() == HirGenericTypeIssue::UnclassifiedSyntax
    ));
    assert!(ty.state().is_poisoned());
    assert_source_backed_child(&module, parameter.pattern());
    assert_source_backed_child(&module, parameter.ty());
    assert!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), proof.callable_scope())
            .unwrap()
            .locals()
            .is_empty()
    );
}

#[test]
fn proof_freeze_rejects_unit_return_and_proof_call_payload_substitution_atomically() {
    assert_proof_freeze_rejects(
        "unit-return-kind",
        "proof unit() = ()\n",
        |transaction, owner| {
            let (return_type, scope) = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Proof(proof) = item.kind() else {
                    panic!("final Proof item")
                };
                (proof.return_type(), proof.callable_scope())
            };
            let replacement = HirType::try_new(
                return_type,
                HirTypeKind::Path(
                    HirPath::try_new(
                        HirPathRoot::ImplicitCrate,
                        Box::new([HirPathSegment::Identifier(
                            HirName::try_new("NotUnit".into()).unwrap(),
                        )]),
                    )
                    .unwrap(),
                ),
                scope,
                HirPoisonState::Clean,
                transaction,
            )
            .unwrap();
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .types()
                .revise_finalized(slots, return_type, replacement)
                .unwrap();
        },
    );

    assert_proof_freeze_rejects(
        "proof-call-kind",
        "proof calls() { lemma(); }\n",
        |transaction, owner| {
            let statement = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Proof(proof) = item.kind() else {
                    panic!("final Proof item")
                };
                let HirProofBody::Block { statements, .. } = proof.body() else {
                    panic!("Proof block")
                };
                statements[0]
            };
            let replacement = {
                let (slots, arenas) = transaction.storage_mut();
                let original = arenas
                    .statements()
                    .resolve_staged(slots, statement)
                    .unwrap();
                let HirStmtKind::ProofCall { call } = original.kind() else {
                    panic!("ProofCall statement")
                };
                HirStmt::try_new(
                    original.scope(),
                    HirStmtKind::Expression { expression: *call },
                )
                .unwrap()
            };
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .statements()
                .revise_finalized(slots, statement, replacement)
                .unwrap();
        },
    );
}
