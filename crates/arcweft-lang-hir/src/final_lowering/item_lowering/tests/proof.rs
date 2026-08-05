use super::*;

use arcweft_lang_syntax::attachment::TypedItemNode;

use crate::expr::{HirExprKind, HirPoisonState};
use crate::item::{HirProof, HirProofBody};
use crate::pattern::{HirPatternBinding, HirPatternBindingIssue, HirPatternKind};
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
        .tree()
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
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
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
    let module = lower(&mut database, &parsed, &key);
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
        assert_eq!(proof.public_id().map(|id| id.as_str()), expected);
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
    let module = lower(&mut database, &parsed, &key);
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
    let module = lower(&mut database, &parsed, &key);

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
