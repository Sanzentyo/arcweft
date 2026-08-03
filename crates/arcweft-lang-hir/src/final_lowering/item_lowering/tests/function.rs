use super::*;

use arcweft_lang_syntax::attachment::TypedItemNode;

use crate::expr::HirPoisonState;
use crate::item::{
    HirContractScopes, HirFunctionBody, HirFunctionItem, HirFunctionParameterGroup,
    HirFunctionSignature, HirParameter, HirParameterKind,
};
use crate::stmt::HirStmtKind;
use crate::type_ref::{HirType, HirTypeKind};

fn function(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirFunctionItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Function(function) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Function")
    };
    (owner, item, function)
}

fn assert_function_body_scope(
    module: &HirModule,
    parsed: &ParsedSource,
    ordinal: usize,
    owner: crate::identity::ItemId,
    function: &HirFunctionItem,
) -> ScopeId {
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            TypedItemNode::Function(function) => function.semantics().ok(),
            _ => None,
        })
        .nth(ordinal)
        .expect("attached Function body");
    let body_syntax = attached.body().syntax();
    assert_eq!(
        body_syntax.kind(),
        arcweft_lang_syntax::grammar::SyntaxKind::FunctionBody
    );
    let HirFunctionBody::Block { scope, .. } = function.body() else {
        panic!("Function must retain a block body")
    };
    let payload = module
        .arenas()
        .scopes()
        .resolve(module.slots(), *scope)
        .unwrap();
    assert_eq!(payload.kind(), HirScopeKind::Block);
    assert_eq!(payload.parent(), Some(function.callable_scope()));
    assert_eq!(payload.owner(), &HirScopeOwner::Item(owner));
    let metadata = module.slots().resolve(*scope).unwrap();
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
        .resolve(module.slots(), function.callable_scope())
        .unwrap();
    assert_eq!(
        callable.children(),
        [function.requires_scope(), function.ensures_scope(), *scope]
    );
    *scope
}

fn assert_function_block_payload(
    module: &HirModule,
    function: &HirFunctionItem,
    body_scope: ScopeId,
) {
    let HirFunctionBody::Block {
        scope,
        statements,
        tail,
    } = function.body()
    else {
        panic!("Function block body")
    };
    assert_eq!(*scope, body_scope);
    let [statement] = statements.as_ref() else {
        panic!("one exact ordinary Function statement")
    };
    assert!(matches!(
        module
            .arenas()
            .statements()
            .resolve(module.slots(), *statement)
            .unwrap()
            .kind(),
        HirStmtKind::Let { .. }
    ));
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), *tail)
            .unwrap()
            .scope(),
        body_scope
    );
}

fn assert_function_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-function-{case}"),
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
        "Function freeze accepted {case}"
    );
    assert!(database.current(&key).is_none());
}

fn replace_function_parameter_groups(
    transaction: &mut StagedHirModuleTransaction<'_>,
    owner: crate::identity::ItemId,
    parameter_groups: Box<[HirFunctionParameterGroup]>,
) {
    let (scope, prefix, state, members, name, signature, body, scopes) = {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Function(function) = item.kind() else {
            panic!("final Function item")
        };
        (
            item.scope(),
            item.prefix().clone(),
            *item.state(),
            item.members().into(),
            function.name().clone(),
            (
                function.generic_parameters().into(),
                function.where_predicates().into(),
                function.requires().into(),
                function.ensures().into(),
                function.return_type(),
            ),
            function.body().clone(),
            (
                function.callable_scope(),
                function.requires_scope(),
                function.ensures_scope(),
            ),
        )
    };
    let signature = HirFunctionSignature::try_new(
        owner.module(),
        signature.0,
        parameter_groups,
        signature.1,
        signature.2,
        signature.3,
        signature.4,
    )
    .unwrap();
    let scopes = HirContractScopes::try_new(scopes.0, scopes.1, scopes.2).unwrap();
    let function = HirFunctionItem::try_new(name, signature, body, scopes).unwrap();
    let replacement = HirItem::try_new_with_state(
        owner,
        scope,
        prefix,
        HirItemKind::Function(function),
        members,
        state,
    )
    .unwrap();
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
}

fn lower_function_output(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
    key: &HirModuleKey,
) -> crate::database::HirLowerOutput {
    let mut transaction = stage(database, parsed, key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    transaction.finish(database).unwrap()
}

fn expression_ids_in_scope(module: &HirModule, scope: ScopeId) -> Vec<ExprId> {
    module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .unwrap()
        .filter_map(|(owner, expression)| (expression.scope() == scope).then_some(owner))
        .collect()
}

fn assert_retired_at<I: HirTypedId>(
    module: &HirModule,
    owner: I,
    expected: crate::identity::HirRevision,
) {
    assert!(matches!(
        module.slots().resolve(owner),
        Err(crate::slot::HirSlotError::Resolve(
            crate::identity::IdResolveError::Retired {
                id,
                snapshot,
                retired_at,
            }
        )) if id == owner.raw().view()
            && snapshot == module.snapshot_id()
            && retired_at == expected
    ));
}

fn assert_retired<I: HirTypedId>(module: &HirModule, owner: I) {
    assert_retired_at(module, owner, module.snapshot_id().revision());
}

fn assert_not_yet_live_at<I: HirTypedId>(
    module: &HirModule,
    owner: I,
    expected_born: crate::identity::HirRevision,
) {
    assert!(matches!(
        module.slots().resolve(owner),
        Err(crate::slot::HirSlotError::Resolve(
            crate::identity::IdResolveError::NotYetLive { id, snapshot, born }
        )) if id == owner.raw().view()
            && snapshot == module.snapshot_id()
            && born == expected_born
    ));
}

#[test]
fn canonical_function_freezes_curried_signature_contracts_block_and_source_owners() {
    let source = concat!(
        "pub fn ordered<T: Bound>((left, right): (T, T))(next: Mapper<T>) -> Output\n",
        "where T: Other\n",
        "requires ready(left)\n",
        "ensures result == next(right)\n",
        "{\n",
        "    let chosen = next(left);\n",
        "    chosen\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-function-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, function) = function(&module, 0);
    let body_scope = assert_function_body_scope(&module, &parsed, 0, owner, function);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        function.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "ordered"
    ));
    assert_eq!(function.generic_parameters().len(), 1);
    let [first_group, second_group] = function.parameter_groups() else {
        panic!("two exact curried parameter groups")
    };
    let [first] = first_group.parameters() else {
        panic!("one first-group parameter")
    };
    let [second] = second_group.parameters() else {
        panic!("one second-group parameter")
    };
    assert_eq!(first.locals().len(), 2);
    assert_eq!(second.locals().len(), 1);
    assert_eq!(function.where_predicates().len(), 1);
    assert_eq!(function.requires().len(), 1);
    assert_eq!(function.ensures().len(), 1);
    let return_type = function.return_type().expect("authored Function return");
    assert_eq!(
        module
            .arenas()
            .types()
            .resolve(module.slots(), return_type)
            .unwrap()
            .scope(),
        function.callable_scope()
    );

    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), function.callable_scope())
        .unwrap();
    let expected_locals = function
        .parameter_groups()
        .iter()
        .flat_map(crate::item::HirFunctionParameterGroup::parameters)
        .flat_map(|parameter| parameter.locals().iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(callable.locals(), expected_locals);

    let ensures = module
        .arenas()
        .scopes()
        .resolve(module.slots(), function.ensures_scope())
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
    assert_eq!(result.annotation(), Some(return_type));

    assert_function_block_payload(&module, function, body_scope);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn function_retains_curried_group_boundaries_and_freeze_rejects_moved_parameters() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-function-group-boundaries",
        "fn curried(first: A)(second: B) {}\nfn flat(first: A, second: B) {}\n",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, _, curried) = function(&module, 0);
    let (_, _, flat) = function(&module, 1);
    assert_eq!(
        curried
            .parameter_groups()
            .iter()
            .map(|group| group.parameters().len())
            .collect::<Vec<_>>(),
        [1, 1]
    );
    assert_eq!(
        flat.parameter_groups()
            .iter()
            .map(|group| group.parameters().len())
            .collect::<Vec<_>>(),
        [2]
    );

    assert_function_freeze_rejects(
        "moved-group-boundary",
        "fn curried(first: A)(second: B) {}\n",
        |transaction, owner| {
            let (first, second) = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Function(function) = item.kind() else {
                    panic!("final Function item")
                };
                (
                    function.parameter_groups()[0].parameters()[0].clone(),
                    function.parameter_groups()[1].parameters()[0].clone(),
                )
            };
            replace_function_parameter_groups(
                transaction,
                owner,
                vec![
                    HirFunctionParameterGroup::try_new(
                        owner.module(),
                        vec![first, second].into_boxed_slice(),
                    )
                    .unwrap(),
                    HirFunctionParameterGroup::try_new(owner.module(), Box::new([])).unwrap(),
                ]
                .into_boxed_slice(),
            );
        },
    );
}

#[test]
fn function_retains_parameter_defaults_and_rest_kind_with_exact_source_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-function-parameter-surface",
        "fn configured(first: Int = 1)(rest: ...String) { first }\n",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Function(function) => function.semantics().ok(),
            _ => None,
        })
        .expect("attached Function");
    assert!(!attached.has_parameter_shape_recovery());

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, function) = function(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let [first_group, second_group] = function.parameter_groups() else {
        panic!("two exact parameter groups")
    };
    let [first] = first_group.parameters() else {
        panic!("one defaulted fixed parameter")
    };
    let [rest] = second_group.parameters() else {
        panic!("one positional rest parameter")
    };
    assert_eq!(first.kind(), HirParameterKind::Fixed);
    let default = first.default().expect("retained default expression");
    assert_eq!(rest.kind(), HirParameterKind::RestPositional);
    assert!(rest.default().is_none());
    assert_eq!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), default)
            .unwrap()
            .scope(),
        function.callable_scope()
    );
    let attached_default = attached.parameter_groups()[0].parameters()[0]
        .default()
        .expect("attached default");
    assert!(matches!(
        module.slots().resolve(default).unwrap().origin(),
        HirOrigin::Source(source) if source.syntax() == attached_default.value().id()
    ));
    assert_eq!(
        module.slots().resolve(default).unwrap().source_site(),
        &HirSourceSite::Span(attached_default.value().whole_source_span())
    );
}

#[test]
fn function_rest_shape_recovery_retains_typed_parameters_and_defaults() {
    let source = concat!(
        "fn misplaced(rest: ...Int)(later: Int) {}\n",
        "fn duplicate(first: ...Int, second: ...Int) {}\n",
        "fn defaulted(rest: ...Int = 1) {}\n",
    );
    let parsed = parse(
        "arcweft-test://proof/final-hir-function-rest-recovery",
        source,
    );
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .filter_map(|item| match item {
            TypedItemNode::Function(function) => function.semantics().ok(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(attached.len(), 3);
    assert!(attached.iter().all(
        arcweft_lang_syntax::attachment::AttachedFunctionDeclaration::has_parameter_shape_recovery
    ));

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    for ordinal in 0..3 {
        let (_, item, _) = function(&module, ordinal);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
        );
    }
    let (_, _, misplaced) = function(&module, 0);
    assert_eq!(
        misplaced.parameter_groups()[0].parameters()[0].kind(),
        HirParameterKind::RestPositional
    );
    let (_, _, duplicate) = function(&module, 1);
    assert!(
        duplicate.parameter_groups()[0]
            .parameters()
            .iter()
            .all(|parameter| parameter.kind() == HirParameterKind::RestPositional)
    );
    let (_, _, defaulted) = function(&module, 2);
    let rest = &defaulted.parameter_groups()[0].parameters()[0];
    assert_eq!(rest.kind(), HirParameterKind::RestPositional);
    assert!(rest.default().is_some());
}

#[test]
fn function_freeze_rejects_parameter_kind_and_default_cardinality_tampering() {
    assert_function_freeze_rejects(
        "parameter-kind",
        "fn configured(value: Int = 1) {}\n",
        |transaction, owner| {
            let (parameter, group_count) = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Function(function) = item.kind() else {
                    panic!("final Function item")
                };
                (
                    function.parameter_groups()[0].parameters()[0].clone(),
                    function.parameter_groups().len(),
                )
            };
            assert_eq!(group_count, 1);
            let replacement = HirParameter::try_new(
                parameter.pattern(),
                parameter.ty(),
                HirParameterKind::RestPositional,
                parameter.default(),
                parameter.locals().into(),
            )
            .unwrap();
            replace_function_parameter_groups(
                transaction,
                owner,
                vec![
                    HirFunctionParameterGroup::try_new(
                        owner.module(),
                        vec![replacement].into_boxed_slice(),
                    )
                    .unwrap(),
                ]
                .into_boxed_slice(),
            );
        },
    );

    assert_function_freeze_rejects(
        "parameter-default-cardinality",
        "fn configured(value: Int = 1) {}\n",
        |transaction, owner| {
            let parameter = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Function(function) = item.kind() else {
                    panic!("final Function item")
                };
                function.parameter_groups()[0].parameters()[0].clone()
            };
            let replacement = HirParameter::try_new(
                parameter.pattern(),
                parameter.ty(),
                parameter.kind(),
                None,
                parameter.locals().into(),
            )
            .unwrap();
            replace_function_parameter_groups(
                transaction,
                owner,
                vec![
                    HirFunctionParameterGroup::try_new(
                        owner.module(),
                        vec![replacement].into_boxed_slice(),
                    )
                    .unwrap(),
                ]
                .into_boxed_slice(),
            );
        },
    );

    assert_function_freeze_rejects(
        "parameter-default-source-owner",
        "fn configured(first: Int = 1, second: Int = 2) {}\n",
        |transaction, owner| {
            let parameters = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Function(function) = item.kind() else {
                    panic!("final Function item")
                };
                function.parameter_groups()[0].parameters().to_vec()
            };
            let replacement = HirParameter::try_new(
                parameters[0].pattern(),
                parameters[0].ty(),
                parameters[0].kind(),
                parameters[1].default(),
                parameters[0].locals().into(),
            )
            .unwrap();
            replace_function_parameter_groups(
                transaction,
                owner,
                vec![
                    HirFunctionParameterGroup::try_new(
                        owner.module(),
                        vec![replacement, parameters[1].clone()].into_boxed_slice(),
                    )
                    .unwrap(),
                ]
                .into_boxed_slice(),
            );
        },
    );
}

#[test]
fn function_omitted_return_and_missing_body_keep_distinct_typed_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-function-body-matrix",
        concat!(
            "fn unit()\n",
            "ensures result == ()\n",
            "{}\n",
            "fn declared() -> Int {}\n",
            "fn missing()\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (unit_owner, unit_item, unit) = function(&module, 0);
    let unit_scope = assert_function_body_scope(&module, &parsed, 0, unit_owner, unit);
    assert_eq!(unit_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(unit.return_type(), None);
    let [result] = module
        .arenas()
        .scopes()
        .resolve(module.slots(), unit.ensures_scope())
        .unwrap()
        .locals()
    else {
        panic!("omitted return still exposes one postcondition result")
    };
    assert_eq!(
        module
            .arenas()
            .locals()
            .resolve(module.slots(), *result)
            .unwrap()
            .annotation(),
        None
    );
    let HirFunctionBody::Block { tail, .. } = unit.body() else {
        panic!("Unit Function block")
    };
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(unit_scope)
                && key.role() == SyntheticRole::ImplicitUnitTail
    ));
    assert!(!module.slots().resolve(*tail).unwrap().is_poisoned());

    let (declared_owner, declared_item, declared) = function(&module, 1);
    let declared_scope = assert_function_body_scope(&module, &parsed, 1, declared_owner, declared);
    assert_eq!(declared_item.state(), &HirItemPoisonState::Clean);
    assert!(declared.return_type().is_some());
    let HirFunctionBody::Block { tail, .. } = declared.body() else {
        panic!("declared Function block")
    };
    assert!(matches!(
        module.slots().resolve(*tail).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(declared_scope)
                && key.role() == SyntheticRole::ImplicitUnitTail
    ));

    let (missing_owner, missing_item, missing) = function(&module, 2);
    assert_eq!(
        missing_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert_eq!(missing.return_type(), None);
    let HirFunctionBody::Error(error) = missing.body() else {
        panic!("missing Function body must retain a typed error tail")
    };
    assert!(matches!(
        module.slots().resolve(*error).unwrap().origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(missing.callable_scope())
                && key.role() == SyntheticRole::MissingRequiredTail
    ));
    assert!(module.slots().resolve(*error).unwrap().is_poisoned());
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), missing.callable_scope())
        .unwrap();
    assert_eq!(
        callable.children(),
        [missing.requires_scope(), missing.ensures_scope()]
    );
    assert_item_slot_whole(&module, &parsed, missing_owner);
}

#[test]
fn function_recovery_matrix_keeps_the_recognized_family_and_primary_issue() {
    let cases = [
        ("missing-name", "fn () {}\n", HirItemIssue::MissingName),
        (
            "malformed-generic",
            "fn malformed<T extra>() {}\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "missing-parameter-type",
            "fn missing(value) {}\n",
            HirItemIssue::MissingType,
        ),
        (
            "invalid-rest-shape",
            "fn invalid(rest: ...Int = 1) {}\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "missing-return-type",
            "fn missing() -> {}\n",
            HirItemIssue::MissingType,
        ),
        (
            "recovered-return-type",
            "fn malformed() -> Int Int {}\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "malformed-where-clause",
            "fn malformed() where T {}\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "recovered-contract",
            "fn recovered()\nrequires\n{}\n",
            HirItemIssue::Recovery,
        ),
        ("missing-body", "fn missing()\n", HirItemIssue::MissingBody),
        (
            "recovered-body",
            "fn recovered() { let value = 1\n",
            HirItemIssue::Recovery,
        ),
        (
            "trailing-recovery",
            "fn recovered() {} trailing\n",
            HirItemIssue::Recovery,
        ),
        (
            "generic-precedes-later-recovery",
            "fn ordered<T extra>(value) ->\nrequires\n",
            HirItemIssue::MalformedHeader,
        ),
        (
            "parameter-precedes-return",
            "fn ordered(value) -> {}\n",
            HirItemIssue::MissingType,
        ),
        (
            "return-precedes-contract-and-body",
            "fn ordered() ->\nrequires\n",
            HirItemIssue::MissingType,
        ),
        (
            "contract-precedes-missing-body",
            "fn ordered()\nrequires\n",
            HirItemIssue::Recovery,
        ),
    ];

    for (case, source, expected_issue) in cases {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-function-{case}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (owner, item, _) = function(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        assert_item_slot_whole(&module, &parsed, owner);
        assert_item_owner_whole_recovery(&module, owner);
    }
}

#[test]
fn function_lowering_allocates_curried_headers_contracts_and_body_in_source_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-function-allocation-order",
        concat!(
            "fn ordered<T: Bound>(value: Input)(next: Mapper<T>) -> Output\n",
            "where T: Other\n",
            "requires ready(value)\n",
            "ensures result\n",
            "{ next(value) }\n",
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
    let (owner, item, function) = function(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);

    let [generic] = function.generic_parameters() else {
        panic!("one generic parameter")
    };
    let [generic_bound] = generic.bounds() else {
        panic!("one generic bound")
    };
    let [first_group, second_group] = function.parameter_groups() else {
        panic!("two curried groups")
    };
    let [first] = first_group.parameters() else {
        panic!("one first-group parameter")
    };
    let [second] = second_group.parameters() else {
        panic!("one second-group parameter")
    };
    let [first_local] = first.locals() else {
        panic!("one first-group local")
    };
    let [second_local] = second.locals() else {
        panic!("one second-group local")
    };
    let return_type = function.return_type().expect("authored return");
    let [where_predicate] = function.where_predicates() else {
        panic!("one where predicate")
    };
    let [where_bound] = where_predicate.bounds() else {
        panic!("one where bound")
    };
    let [requires] = function.requires() else {
        panic!("one requires condition")
    };
    let [ensures] = function.ensures() else {
        panic!("one ensures condition")
    };
    let [result_local] = module
        .arenas()
        .scopes()
        .resolve(module.slots(), function.ensures_scope())
        .unwrap()
        .locals()
    else {
        panic!("one postcondition result local")
    };
    let HirFunctionBody::Block {
        scope: body_scope,
        statements,
        tail,
    } = function.body()
    else {
        panic!("Function block body")
    };
    assert!(statements.is_empty());

    let slots = [
        owner.raw().slot().get(),
        function.callable_scope().raw().slot().get(),
        function.requires_scope().raw().slot().get(),
        function.ensures_scope().raw().slot().get(),
        body_scope.raw().slot().get(),
        generic_bound.raw().slot().get(),
        first.ty().raw().slot().get(),
        first.pattern().raw().slot().get(),
        first_local.raw().slot().get(),
        second.ty().raw().slot().get(),
        second.pattern().raw().slot().get(),
        second_local.raw().slot().get(),
        return_type.raw().slot().get(),
        where_predicate.subject().raw().slot().get(),
        where_bound.raw().slot().get(),
        requires.raw().slot().get(),
        result_local.raw().slot().get(),
        ensures.raw().slot().get(),
        tail.raw().slot().get(),
    ];
    assert!(
        slots.windows(2).all(|pair| pair[0] < pair[1]),
        "Function allocation order diverged from the accepted transaction: {slots:?}"
    );
}

#[test]
fn function_freeze_rejects_callable_child_order_and_omitted_return_annotation_tampering() {
    assert_function_freeze_rejects(
        "callable-child-order",
        "fn scoped(value: Bool) -> Bool { value }\n",
        |transaction, owner| {
            let (callable_scope, reordered) = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Function(function) = item.kind() else {
                    panic!("final Function item")
                };
                let HirFunctionBody::Block { scope, .. } = function.body() else {
                    panic!("Function block")
                };
                (
                    function.callable_scope(),
                    Box::new([*scope, function.requires_scope(), function.ensures_scope()]),
                )
            };
            let (slots, arenas) = transaction.storage_mut();
            let callable = arenas
                .scopes()
                .resolve_staged(slots, callable_scope)
                .unwrap()
                .clone();
            let replacement = callable
                .try_with_members(reordered, callable.locals().into())
                .unwrap();
            arenas
                .scopes()
                .revise_finalized(slots, callable_scope, replacement)
                .unwrap();
        },
    );

    assert_function_freeze_rejects(
        "omitted-return-result-annotation",
        "fn unit(value: Int) ensures result == value {}\n",
        |transaction, owner| {
            let (result, parameter_type) = {
                let (slots, arenas) = transaction.storage_mut();
                let (ensures_scope, parameter_type) = {
                    let item = arenas.items().resolve_staged(slots, owner).unwrap();
                    let HirItemKind::Function(function) = item.kind() else {
                        panic!("final Function item")
                    };
                    (
                        function.ensures_scope(),
                        function.parameter_groups()[0].parameters()[0].ty(),
                    )
                };
                let [result] = arenas
                    .scopes()
                    .resolve_staged(slots, ensures_scope)
                    .unwrap()
                    .locals()
                else {
                    panic!("one exact postcondition result")
                };
                (*result, parameter_type)
            };
            let (slots, arenas) = transaction.storage_mut();
            let original = arenas
                .locals()
                .resolve_staged(slots, result)
                .unwrap()
                .clone();
            let replacement = HirLocal::try_new(
                original.scope(),
                original.kind(),
                original.name().clone(),
                original.generation(),
                original.pattern(),
                Some(parameter_type),
                original.is_mutable_binding(),
                original.is_poisoned(),
            )
            .unwrap();
            arenas
                .locals()
                .revise_finalized(slots, result, replacement)
                .unwrap();
        },
    );
}

#[test]
fn function_omitted_return_freeze_rejects_foreign_synthetic_return_inventory() {
    assert_function_freeze_rejects(
        "foreign-synthetic-return-inventory",
        "fn unit() {}\n",
        |transaction, owner| {
            let (callable_scope, item_site) = {
                let (slots, arenas) = transaction.storage_mut();
                let item = arenas.items().resolve_staged(slots, owner).unwrap();
                let HirItemKind::Function(function) = item.kind() else {
                    panic!("final Function item")
                };
                (
                    function.callable_scope(),
                    slots.resolve_staged(owner).unwrap().source_site().clone(),
                )
            };
            let key = SyntheticKey::try_new(
                SyntheticOwner::Item(owner),
                SyntheticRole::PredicateBoolReturn,
                0,
            )
            .unwrap();
            let reservation = {
                let (slots, arenas) = transaction.storage_mut();
                arenas
                    .types()
                    .reserve_synthetic(slots, key, item_site)
                    .unwrap()
            };
            let synthetic_return = reservation.id();
            let path = HirPath::try_new(
                HirPathRoot::ImplicitCrate,
                Box::new([HirPathSegment::Identifier(
                    HirName::try_new("Bool".into()).unwrap(),
                )]),
            )
            .unwrap();
            let payload = HirType::try_new(
                synthetic_return,
                HirTypeKind::Path(path),
                callable_scope,
                HirPoisonState::Clean,
                transaction,
            )
            .unwrap();
            let (slots, arenas) = transaction.storage_mut();
            arenas
                .types()
                .finalize(slots, reservation, payload)
                .unwrap();
        },
    );
}

#[test]
fn incremental_function_missing_block_missing_retires_old_body_and_readmits_fresh_error() {
    let name = SourceName::path("proof/function-body-incremental.arcw");
    let document_id = "arcweft-test://proof/function-body-incremental";
    let missing_source = concat!(
        "fn evolving(value: Int)\n",
        "requires ready(value)\n",
        "ensures result == value\n",
    );
    let block_source = concat!(
        "fn evolving(value: Int)\n",
        "requires ready(value)\n",
        "ensures result == value\n",
        "{\n",
        "    let chosen = value;\n",
        "    chosen\n",
        "}\n",
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, missing_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let first_snapshot = first.snapshot_id();
    let (owner, first_item, first_function) = function(&first, 0);
    assert_eq!(
        first_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    let callable_scope = first_function.callable_scope();
    let requires_scope = first_function.requires_scope();
    let ensures_scope = first_function.ensures_scope();
    let first_requires = first_function.requires().to_vec();
    let first_ensures = first_function.ensures().to_vec();
    let HirFunctionBody::Error(first_error) = first_function.body() else {
        panic!("initial Function must own its missing-body expression")
    };

    let with_block = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, missing_source.len()))
                    .unwrap(),
                block_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second_output = lower_function_output(&mut database, &with_block, &key);
    let second = Arc::clone(second_output.module());
    let (second_owner, second_item, second_function) = function(&second, 0);
    assert_eq!(second_owner, owner);
    assert_eq!(second_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(second_function.callable_scope(), callable_scope);
    assert_eq!(second_function.requires_scope(), requires_scope);
    assert_eq!(second_function.ensures_scope(), ensures_scope);
    assert_eq!(second_function.requires(), first_requires);
    assert_eq!(second_function.ensures(), first_ensures);
    assert_eq!(second_output.invalidations().changed_items(), [owner]);
    assert_retired(&second, *first_error);
    let HirFunctionBody::Block {
        scope: block_scope,
        statements,
        tail: block_tail,
    } = second_function.body()
    else {
        panic!("second Function must own the authored block")
    };
    let [block_statement] = statements.as_ref() else {
        panic!("one exact block statement")
    };

    let missing_again = syntax
        .reparse(
            &with_block,
            &[SourceEdit::new(
                with_block
                    .document()
                    .span(SourceRange::new(0, block_source.len()))
                    .unwrap(),
                missing_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let third_output = lower_function_output(&mut database, &missing_again, &key);
    let third = third_output.module();
    let (third_owner, third_item, third_function) = function(third, 0);
    assert_eq!(third_owner, owner);
    assert_eq!(
        third_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert_eq!(third_function.callable_scope(), callable_scope);
    assert_eq!(third_function.requires_scope(), requires_scope);
    assert_eq!(third_function.ensures_scope(), ensures_scope);
    assert_eq!(third_function.requires(), first_requires);
    assert_eq!(third_function.ensures(), first_ensures);
    assert_eq!(third_output.invalidations().changed_items(), [owner]);
    let HirFunctionBody::Error(third_error) = third_function.body() else {
        panic!("third Function must own its replacement missing-body expression")
    };
    assert_ne!(third_error, first_error);
    let error_key = SyntheticKey::try_new(
        SyntheticOwner::Scope(callable_scope),
        SyntheticRole::MissingRequiredTail,
        0,
    )
    .unwrap();
    assert_eq!(
        third
            .slots()
            .resolve_prepared_synthetic::<ExprId>(error_key)
            .unwrap(),
        Some(*third_error)
    );
    assert_retired_at(third, *first_error, second.snapshot_id().revision());
    assert_retired(third, *block_scope);
    assert_retired(third, *block_statement);
    assert_retired(third, *block_tail);
    assert!(third.slots().resolve(*third_error).is_ok());
    assert!(second.slots().resolve(*block_scope).is_ok());
    assert!(second.slots().resolve(*block_statement).is_ok());
    assert!(second.slots().resolve(*block_tail).is_ok());
    assert_not_yet_live_at(&second, *third_error, third.snapshot_id().revision());

    let historical = database.snapshot(first_snapshot).unwrap();
    assert!(historical.slots().resolve(*first_error).is_ok());
    assert_not_yet_live_at(&historical, *third_error, third.snapshot_id().revision());
}

#[test]
fn incremental_function_contract_cardinality_change_retires_the_changed_item_graph() {
    let name = SourceName::path("proof/function-contract-incremental.arcw");
    let document_id = "arcweft-test://proof/function-contract-incremental";
    let initial_source = concat!(
        "fn contracted(value: Int)\n",
        "requires first(value)\n",
        "requires second(value)\n",
        "ensures result == value\n",
        "{ value }\n",
    );
    let revised_source = concat!(
        "fn contracted(value: Int)\n",
        "requires first(value)\n",
        "ensures result == value\n",
        "{ value }\n",
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let (owner, _, first_function) = function(&first, 0);
    let callable_scope = first_function.callable_scope();
    let requires_scope = first_function.requires_scope();
    let ensures_scope = first_function.ensures_scope();
    let HirFunctionBody::Block {
        scope: body_scope, ..
    } = first_function.body()
    else {
        panic!("initial Function block")
    };
    let [_, removed_requires] = first_function.requires() else {
        panic!("two exact preconditions")
    };
    let initial_requires = expression_ids_in_scope(&first, requires_scope);
    let initial_ensures = expression_ids_in_scope(&first, ensures_scope);
    let initial_body = expression_ids_in_scope(&first, *body_scope);

    let removed_clause = "requires second(value)\n";
    let removed_start = initial_source.find(removed_clause).unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(
                        removed_start,
                        removed_start + removed_clause.len(),
                    ))
                    .unwrap(),
                "",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    assert_eq!(revised.document().text(), revised_source);
    let output = lower_function_output(&mut database, &revised, &key);
    let second = output.module();
    let (second_owner, second_item, second_function) = function(second, 0);
    assert_ne!(second_owner, owner);
    assert_eq!(second_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(second_function.requires().len(), 1);
    assert_eq!(output.invalidations().changed_items(), [second_owner]);
    assert_eq!(output.invalidations().retired_items(), [owner]);

    assert_retired(second, owner);
    assert_retired(second, callable_scope);
    assert_retired(second, requires_scope);
    assert_retired(second, ensures_scope);
    assert_retired(second, *body_scope);
    for retired in initial_requires
        .iter()
        .chain(&initial_ensures)
        .chain(&initial_body)
    {
        assert_retired(second, *retired);
    }
    assert_retired(second, *removed_requires);
    assert!(first.slots().resolve(*removed_requires).is_ok());
}

#[test]
fn incremental_function_contract_replacement_retains_scopes_and_retires_replaced_subtree() {
    let name = SourceName::path("proof/function-contract-replacement.arcw");
    let document_id = "arcweft-test://proof/function-contract-replacement";
    let initial_source = concat!(
        "fn contracted(value: Int)\n",
        "requires second(value)\n",
        "ensures result == value\n",
        "{ value }\n",
    );
    let revised_source = concat!(
        "fn contracted(value: Int)\n",
        "requires value == value\n",
        "ensures result == value\n",
        "{ value }\n",
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let (owner, _, first_function) = function(&first, 0);
    let callable_scope = first_function.callable_scope();
    let requires_scope = first_function.requires_scope();
    let ensures_scope = first_function.ensures_scope();
    let HirFunctionBody::Block {
        scope: body_scope, ..
    } = first_function.body()
    else {
        panic!("initial Function block")
    };
    let [old_requires_root] = first_function.requires() else {
        panic!("one exact precondition")
    };
    let initial_requires = expression_ids_in_scope(&first, requires_scope);
    let initial_ensures = expression_ids_in_scope(&first, ensures_scope);
    let initial_body = expression_ids_in_scope(&first, *body_scope);

    let old_condition = "second(value)";
    let new_condition = "value == value";
    let condition_start = initial_source.find(old_condition).unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(
                        condition_start,
                        condition_start + old_condition.len(),
                    ))
                    .unwrap(),
                new_condition,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    assert_eq!(revised.document().text(), revised_source);
    let output = lower_function_output(&mut database, &revised, &key);
    let second = output.module();
    let (second_owner, second_item, second_function) = function(second, 0);
    assert_eq!(second_owner, owner);
    assert_eq!(second_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(second_function.callable_scope(), callable_scope);
    assert_eq!(second_function.requires_scope(), requires_scope);
    assert_eq!(second_function.ensures_scope(), ensures_scope);
    let HirFunctionBody::Block {
        scope: second_body_scope,
        ..
    } = second_function.body()
    else {
        panic!("revised Function block")
    };
    assert_eq!(*second_body_scope, *body_scope);
    assert_eq!(output.invalidations().changed_items(), [owner]);

    let [new_requires_root] = second_function.requires() else {
        panic!("one revised precondition")
    };
    assert_ne!(new_requires_root, old_requires_root);
    assert_retired(second, *old_requires_root);
    let revised_requires = expression_ids_in_scope(second, requires_scope);
    for retired in initial_requires
        .iter()
        .filter(|id| !revised_requires.contains(id))
    {
        assert_retired(second, *retired);
    }
    assert_eq!(
        expression_ids_in_scope(second, ensures_scope),
        initial_ensures
    );
    assert_eq!(expression_ids_in_scope(second, *body_scope), initial_body);
}
