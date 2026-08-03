use super::*;

use std::fmt::Write as _;

use arcweft_lang_syntax::attachment::{
    AttachedEntryDeclaration, AttachedEntryId, AttachedEntryMember, AttachedEntryValue,
    AttachedExpressionNode, SyntaxNodeId, TypedItemNode,
};

use crate::expr::HirExpr;
use crate::identity::{ItemId, TypeId};
use crate::item::{
    HirEntryBody, HirEntryDeclaration, HirEntryId, HirEntryKind, HirEntryMember, HirEntryOption,
    HirEntryOptionValue, HirEntryPathBinding, HirEntryPathValue, HirEntryPunctuationState,
    HirEntryRoute, HirEntryRouteBindings, HirEntryTarget, HirEntryTypeBinding, HirHttpMethod,
    HirHttpMethodIssue, HirHttpMethodValue, HirRoutePath, HirRoutePathIssue, HirRoutePathValue,
};
use crate::leaf::{HirEntityReference, HirIdRef, HirIdRefValue, HirStringIssue};

use super::super::entry::{preflight_entry_members, preflight_entry_route_bindings};

fn entry(module: &HirModule, ordinal: usize) -> (ItemId, &HirItem, &HirEntryDeclaration) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Entry(entry) = item.kind() else {
        panic!("source-ordered item {ordinal} must be an Entry")
    };
    (owner, item, entry)
}

struct CanonicalEntrySyntax {
    inline_expressions: Vec<SyntaxNodeId>,
    types: Vec<SyntaxNodeId>,
    option: SyntaxNodeId,
}

fn canonical_entry_syntax(attached: &AttachedEntryDeclaration) -> CanonicalEntrySyntax {
    let AttachedEntryId::Authored { expression, .. } = attached.id() else {
        panic!("clean Entry ID")
    };
    let mut inline_expressions = vec![expression.id()];
    let mut types = Vec::new();
    let mut option = None;
    for member in attached.body().members() {
        match member {
            AttachedEntryMember::StateType(binding) | AttachedEntryMember::EventType(binding) => {
                types.push(binding.value().value().unwrap().syntax().id());
            }
            AttachedEntryMember::Initializer(binding)
            | AttachedEntryMember::Reducer(binding)
            | AttachedEntryMember::Controller(binding) => {
                inline_expressions.push(binding.value().value().unwrap().syntax().id());
            }
            AttachedEntryMember::Goto { target, .. } => {
                inline_expressions.push(target.value().unwrap().id());
            }
            AttachedEntryMember::Route { path, target, .. } => {
                inline_expressions.push(path.value().unwrap().id());
                inline_expressions.push(target.value().unwrap().id());
            }
            AttachedEntryMember::Option { value, .. } => {
                option = Some(value.value().unwrap().id());
            }
            AttachedEntryMember::Error { .. } => panic!("clean Entry recovery member"),
        }
    }
    CanonicalEntrySyntax {
        inline_expressions,
        types,
        option: option.expect("clean Entry option"),
    }
}

fn assert_canonical_entry_payload(
    module: &HirModule,
    owner: ItemId,
    item: &HirItem,
    declaration: &HirEntryDeclaration,
    source: CanonicalEntrySyntax,
) {
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(declaration.kind(), &HirEntryKind::Server);
    assert!(matches!(
        declaration.id().value(),
        Some(HirIdRefValue::Resolved(_))
    ));
    assert!(!declaration.has_header_trailing_recovery());
    assert!(declaration.body().is_closed());
    assert_eq!(declaration.members().len(), 8);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());

    let [
        HirEntryMember::StateType(state),
        HirEntryMember::Initializer(initializer),
        HirEntryMember::EventType(event),
        HirEntryMember::Reducer(reducer),
        HirEntryMember::Controller(controller),
        HirEntryMember::Goto(goto),
        HirEntryMember::Route(route),
        HirEntryMember::Option(option),
    ] = declaration.members()
    else {
        panic!("Entry final member inventory or source order changed")
    };
    for ty in [state.ty(), event.ty()] {
        assert_eq!(
            module
                .arenas()
                .types()
                .resolve(module.slots(), ty)
                .unwrap()
                .scope(),
            item.scope()
        );
        assert_source_backed_child(module, ty);
    }
    for (syntax, ty) in source.types.into_iter().zip([state.ty(), event.ty()]) {
        assert_eq!(
            module.slots().prepared_source_owner::<TypeId>(syntax),
            Some(ty)
        );
    }
    for (binding, expected) in [
        (initializer, ["server", "initial_state"].as_slice()),
        (reducer, ["server", "reduce"].as_slice()),
        (controller, ["server", "control"].as_slice()),
    ] {
        let HirEntryPathValue::Authored(path) = binding.value() else {
            panic!("clean Entry role path")
        };
        assert_eq!(path_spellings(resolved_path(path)), expected);
        assert_eq!(binding.assignment(), HirEntryPunctuationState::Present);
    }
    assert!(matches!(goto.target(), HirEntryTarget::Authored(_)));
    assert!(matches!(
        route.method(),
        HirHttpMethodValue::Resolved(HirHttpMethod::Get)
    ));
    assert!(matches!(
        route.path(),
        HirRoutePathValue::Resolved(path) if path.as_str() == "/hello/:name"
    ));
    assert!(matches!(route.target(), HirEntryTarget::Authored(_)));
    let HirEntryRouteBindings::Parenthesized {
        items,
        closed: true,
    } = route.bindings()
    else {
        panic!("closed route binding list")
    };
    assert_eq!(items.len(), 1);
    assert!(
        matches!(items[0].parameter(), HirRequiredName::Resolved(name) if name.as_str() == "name")
    );
    assert!(
        matches!(items[0].path_capture(), HirRequiredName::Resolved(name) if name.as_str() == "name")
    );
    assert_eq!(option.assignment(), HirEntryPunctuationState::Present);
    let option_expression = option
        .value()
        .expression()
        .expect("authored Entry option expression");
    let expression = module
        .arenas()
        .expressions()
        .resolve(module.slots(), option_expression)
        .unwrap();
    assert_eq!(expression.scope(), item.scope());
    assert_source_backed_child(module, option_expression);
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(source.option),
        Some(option_expression)
    );
    for syntax in source.inline_expressions {
        assert_eq!(module.slots().prepared_source_owner::<ExprId>(syntax), None);
    }
}

#[test]
fn canonical_entry_retains_closed_members_and_allocates_only_typed_children() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-clean",
        concat!(
            "/// HTTP launch entry\n",
            "#[launch(primary)]\n",
            "pub entry server @entry.http {\n",
            "    state = ServerState\n",
            "    initializer = server.initial_state\n",
            "    event = ServerEvent\n",
            "    reducer = server.reduce\n",
            "    controller = server.control\n",
            "    goto @flow.start\n",
            "    route GET \"/hello/:name\" -> @flow.hello(name = :name)\n",
            "    budget = policy(1 + 2)\n",
            "}\n",
        ),
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
            TypedItemNode::Entry(entry) => Some(entry.semantics().unwrap()),
            _ => None,
        })
        .expect("typed Entry attachment");
    let source = canonical_entry_syntax(&attached);

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = entry(&module, 0);
    assert_canonical_entry_payload(&module, owner, item, declaration, source);
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn entry_kind_and_http_method_closed_matrices_lower_and_freeze() {
    let kinds = [
        ("game", HirEntryKind::Game),
        ("editor", HirEntryKind::Editor),
        ("cli", HirEntryKind::Cli),
        ("server", HirEntryKind::Server),
        ("activity", HirEntryKind::Activity),
        ("test", HirEntryKind::Test),
        ("bench", HirEntryKind::Bench),
        ("agent", HirEntryKind::Agent),
    ];
    let methods = [
        ("GET", HirHttpMethod::Get),
        ("POST", HirHttpMethod::Post),
        ("PUT", HirHttpMethod::Put),
        ("PATCH", HirHttpMethod::Patch),
        ("DELETE", HirHttpMethod::Delete),
        ("HEAD", HirHttpMethod::Head),
        ("OPTIONS", HirHttpMethod::Options),
    ];
    let mut source = String::new();
    for (ordinal, (spelling, _)) in kinds.iter().enumerate() {
        writeln!(source, "entry {spelling} @entry.kind_{ordinal} {{}}").unwrap();
    }
    writeln!(source, "entry custom_adapter @entry.kind_custom {{}}").unwrap();
    source.push_str("entry server @entry.method_matrix {\n");
    for (ordinal, (spelling, _)) in methods.iter().enumerate() {
        writeln!(
            source,
            "    route {spelling} \"/method/{ordinal}\" -> @flow.method_{ordinal}"
        )
        .unwrap();
    }
    source.push_str("}\n");

    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-closed-matrices",
        &source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    for (ordinal, (_, expected)) in kinds.iter().enumerate() {
        let (_, item, declaration) = entry(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        assert_eq!(declaration.kind(), expected);
    }
    let (_, custom_item, custom) = entry(&module, kinds.len());
    assert_eq!(custom_item.state(), &HirItemPoisonState::Clean);
    assert!(matches!(
        custom.kind(),
        HirEntryKind::Custom(name) if name.as_str() == "custom_adapter"
    ));

    let (_, method_item, method_entry) = entry(&module, kinds.len() + 1);
    assert_eq!(method_item.state(), &HirItemPoisonState::Clean);
    assert_eq!(method_entry.members().len(), methods.len());
    for (member, (_, expected)) in method_entry.members().iter().zip(methods) {
        let HirEntryMember::Route(route) = member else {
            panic!("HTTP method matrix must retain only route members")
        };
        assert_eq!(route.method(), &HirHttpMethodValue::Resolved(expected),);
        assert!(!route.has_recovery());
    }
    assert!(Arc::ptr_eq(
        &module,
        &database
            .current(&key)
            .expect("closed Entry matrices commit through source freeze")
    ));
}

#[derive(Clone, Copy, Debug)]
enum EntryRouteRecoveryCase {
    UnsupportedMethod,
    MissingMethod,
    NonAbsolutePath,
    InvalidString,
    MissingPath,
    InvalidExpressionPath,
    MissingArrow,
    MissingTarget,
    InvalidTarget,
    MissingBindingParameter,
    MissingBindingEquals,
    MissingBindingColon,
    MissingBindingCapture,
    UnclosedBindings,
    TrailingRecovery,
}

const ENTRY_ROUTE_RECOVERY_CASES: &[(&str, &str, EntryRouteRecoveryCase)] = &[
    (
        "unsupported-method",
        "route FETCH \"/ok\" -> @flow.target",
        EntryRouteRecoveryCase::UnsupportedMethod,
    ),
    (
        "missing-method",
        "route \"/ok\" -> @flow.target",
        EntryRouteRecoveryCase::MissingMethod,
    ),
    (
        "non-absolute-path",
        "route GET \"relative\" -> @flow.target",
        EntryRouteRecoveryCase::NonAbsolutePath,
    ),
    (
        "invalid-string",
        "route GET \"/\\q\" -> @flow.target",
        EntryRouteRecoveryCase::InvalidString,
    ),
    (
        "missing-path",
        "route GET",
        EntryRouteRecoveryCase::MissingPath,
    ),
    (
        "invalid-expression-path",
        "route GET 123 -> @flow.target",
        EntryRouteRecoveryCase::InvalidExpressionPath,
    ),
    (
        "missing-arrow",
        "route GET \"/ok\" @flow.target",
        EntryRouteRecoveryCase::MissingArrow,
    ),
    (
        "missing-target",
        "route GET \"/ok\" ->",
        EntryRouteRecoveryCase::MissingTarget,
    ),
    (
        "invalid-target",
        "route GET \"/ok\" -> target",
        EntryRouteRecoveryCase::InvalidTarget,
    ),
    (
        "missing-binding-parameter",
        "route GET \"/:capture\" -> @flow.target(= :capture)",
        EntryRouteRecoveryCase::MissingBindingParameter,
    ),
    (
        "missing-binding-equals",
        "route GET \"/:capture\" -> @flow.target(parameter :capture)",
        EntryRouteRecoveryCase::MissingBindingEquals,
    ),
    (
        "missing-binding-colon",
        "route GET \"/:capture\" -> @flow.target(parameter = capture)",
        EntryRouteRecoveryCase::MissingBindingColon,
    ),
    (
        "missing-binding-capture",
        "route GET \"/:capture\" -> @flow.target(parameter = :)",
        EntryRouteRecoveryCase::MissingBindingCapture,
    ),
    (
        "unclosed-bindings",
        "route GET \"/:capture\" -> @flow.target(parameter = :capture",
        EntryRouteRecoveryCase::UnclosedBindings,
    ),
    (
        "trailing-recovery",
        "route GET \"/ok\" -> @flow.target unexpected",
        EntryRouteRecoveryCase::TrailingRecovery,
    ),
];

fn assert_entry_route_recovery(case: EntryRouteRecoveryCase, route: &HirEntryRoute) {
    match case {
        EntryRouteRecoveryCase::UnsupportedMethod => assert!(matches!(
            route.method(),
            HirHttpMethodValue::Recovered {
                authored: Some(name),
                issue: HirHttpMethodIssue::Unsupported,
            } if name.as_str() == "FETCH"
        )),
        EntryRouteRecoveryCase::MissingMethod => assert!(matches!(
            route.method(),
            HirHttpMethodValue::Recovered {
                authored: None,
                issue: HirHttpMethodIssue::Missing,
            }
        )),
        EntryRouteRecoveryCase::NonAbsolutePath => assert!(matches!(
            route.path(),
            HirRoutePathValue::Recovered {
                decoded: Some(value),
                issue: HirRoutePathIssue::InvalidPath,
            } if value.as_ref() == "relative"
        )),
        EntryRouteRecoveryCase::InvalidString => assert!(matches!(
            route.path(),
            HirRoutePathValue::Recovered {
                decoded: None,
                issue: HirRoutePathIssue::InvalidString(HirStringIssue::InvalidEscape),
            }
        )),
        EntryRouteRecoveryCase::MissingPath => assert!(matches!(
            route.path(),
            HirRoutePathValue::Recovered {
                decoded: None,
                issue: HirRoutePathIssue::Missing,
            }
        )),
        EntryRouteRecoveryCase::InvalidExpressionPath => assert!(matches!(
            route.path(),
            HirRoutePathValue::Recovered {
                decoded: None,
                issue: HirRoutePathIssue::InvalidExpression,
            }
        )),
        EntryRouteRecoveryCase::MissingArrow => {
            assert_eq!(route.arrow(), HirEntryPunctuationState::Missing);
            assert!(matches!(route.target(), HirEntryTarget::Authored(_)));
        }
        EntryRouteRecoveryCase::MissingTarget => {
            assert!(matches!(route.target(), HirEntryTarget::Missing));
        }
        EntryRouteRecoveryCase::InvalidTarget => {
            assert!(matches!(route.target(), HirEntryTarget::Invalid));
        }
        EntryRouteRecoveryCase::MissingBindingParameter
        | EntryRouteRecoveryCase::MissingBindingEquals
        | EntryRouteRecoveryCase::MissingBindingColon
        | EntryRouteRecoveryCase::MissingBindingCapture => {
            let HirEntryRouteBindings::Parenthesized {
                items,
                closed: true,
            } = route.bindings()
            else {
                panic!("recovered binding case must retain one closed binding list")
            };
            let [binding] = items.as_ref() else {
                panic!("recovered binding case must retain one binding")
            };
            match case {
                EntryRouteRecoveryCase::MissingBindingParameter => {
                    assert!(matches!(binding.parameter(), HirRequiredName::Missing));
                }
                EntryRouteRecoveryCase::MissingBindingEquals => {
                    assert_eq!(binding.assignment(), HirEntryPunctuationState::Missing);
                }
                EntryRouteRecoveryCase::MissingBindingColon => {
                    assert_eq!(binding.colon(), HirEntryPunctuationState::Missing);
                }
                EntryRouteRecoveryCase::MissingBindingCapture => {
                    assert!(matches!(binding.path_capture(), HirRequiredName::Missing));
                }
                _ => unreachable!("binding recovery match is exhaustive"),
            }
        }
        EntryRouteRecoveryCase::UnclosedBindings => assert!(matches!(
            route.bindings(),
            HirEntryRouteBindings::Parenthesized { closed: false, .. }
        )),
        EntryRouteRecoveryCase::TrailingRecovery => {
            assert!(route.has_trailing_recovery());
        }
    }
}

#[test]
fn entry_route_recovery_matrix_lowers_and_freezes_exact_typed_states() {
    for &(name, route_source, expected) in ENTRY_ROUTE_RECOVERY_CASES {
        let source = format!("entry server @entry.{name} {{\n    {route_source}\n}}\n");
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-entry-route-{name}"),
            &source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (owner, item, declaration) = entry(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember),
            "{name}"
        );
        let [HirEntryMember::Route(route)] = declaration.members() else {
            panic!("{name}: route recovery must retain the Route semantic family")
        };
        assert_entry_route_recovery(expected, route);
        assert!(route.has_recovery(), "{name}");
        assert_item_owner_whole_recovery(&module, owner);
        assert!(Arc::ptr_eq(
            &module,
            &database
                .current(&key)
                .expect("recovered Entry route commits through source freeze")
        ));
    }
}

#[test]
fn entry_error_member_and_unclosed_body_remain_typed_recovery() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-error-member",
        concat!(
            "entry cli @entry.error_member {\n",
            "    unsupported @flow.legacy\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = entry(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert!(matches!(declaration.members(), [HirEntryMember::Error]));
    assert_item_owner_whole_recovery(&module, owner);

    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-unclosed-body",
        "entry game @entry.unclosed_body {\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = entry(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::Recovery)
    );
    assert!(!declaration.body().is_closed());
    assert!(declaration.members().is_empty());
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn entry_missing_type_and_option_value_keep_typed_recovery_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-member-recovery",
        concat!(
            "entry game @entry.recovery {\n",
            "    state =\n",
            "    budget =\n",
            "}\n",
        ),
    );
    let missing_option_syntax = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Entry(entry) => entry
                .semantics()
                .unwrap()
                .body()
                .members()
                .iter()
                .find_map(|member| match member {
                    AttachedEntryMember::Option {
                        value: AttachedEntryValue::Missing(syntax),
                        ..
                    } => Some(syntax.id()),
                    _ => None,
                }),
            _ => None,
        })
        .expect("missing Entry option syntax");
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = entry(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    let [
        HirEntryMember::StateType(state),
        HirEntryMember::Option(option),
    ] = declaration.members()
    else {
        panic!("recovered Entry children")
    };

    let type_record = module
        .arenas()
        .types()
        .resolve(module.slots(), state.ty())
        .unwrap();
    assert_eq!(type_record.scope(), item.scope());
    assert!(module.slots().resolve(state.ty()).unwrap().is_poisoned());
    assert_source_backed_child(&module, state.ty());

    assert_eq!(option.value(), &HirEntryOptionValue::Missing);
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(missing_option_syntax),
        None
    );
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn entry_header_recovery_precedence_retains_the_entry_family() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-header-recovery",
        concat!(
            "entry @entry.no_kind {}\n",
            "entry game {}\n",
            "entry game @flow.wrong {}\n",
            "entry game @entry.trailing junk {}\n",
            "entry game @entry.no_body\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let expected = [
        HirItemIssue::MissingKind,
        HirItemIssue::MissingId,
        HirItemIssue::MalformedHeader,
        HirItemIssue::MalformedHeader,
        HirItemIssue::MissingBody,
    ];
    for (ordinal, issue) in expected.into_iter().enumerate() {
        let (_, item, _) = entry(&module, ordinal);
        assert_eq!(item.state(), &HirItemPoisonState::Poisoned(issue));
    }
}

#[test]
fn entry_recovered_inline_leaves_remain_entry_items_and_delimited_ids_remain_noncanonical() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-inline-recovery",
        concat!(
            "entry game @entry.path_recovery {\n",
            "    initializer = server.\n",
            "}\n",
            "entry game @entry.goto_recovery {\n",
            "    goto @flow.\n",
            "}\n",
            "entry game @<entry.delimited> {}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, path_item, path_entry) = entry(&module, 0);
    assert_eq!(
        path_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    let [HirEntryMember::Initializer(initializer)] = path_entry.members() else {
        panic!("recovered callable path must retain its Entry role")
    };
    let HirEntryPathValue::Authored(initializer_path) = initializer.value() else {
        panic!(
            "recovered authored callable path changed semantic family: {:?}",
            initializer.value()
        )
    };
    assert_eq!(path_spellings(resolved_path(initializer_path)), ["server"]);
    assert!(initializer.has_trailing_recovery());

    let (_, goto_item, goto_entry) = entry(&module, 1);
    assert_eq!(
        goto_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    let [HirEntryMember::Goto(goto)] = goto_entry.members() else {
        panic!("recovered goto ID must retain its Entry member")
    };
    assert!(matches!(
        goto.target(),
        HirEntryTarget::Authored(value) if value.recovery().is_some()
    ));

    let (_, delimited_item, delimited_entry) = entry(&module, 2);
    assert_eq!(
        delimited_item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader)
    );
    let HirEntryId::Authored {
        value: HirIdRefValue::Resolved(reference),
        canonical_entry_family: false,
    } = delimited_entry.id()
    else {
        panic!("delimited Entry ID must remain a resolved noncanonical header leaf")
    };
    assert_eq!(reference.absolute_family(), Some("entry"));
}

fn assert_entry_freeze_rejects(
    case: &str,
    tamper: impl FnOnce(ItemId, &HirEntryDeclaration) -> (HirEntryDeclaration, HirItemPoisonState),
) {
    assert_entry_freeze_rejects_from_source(
        case,
        concat!(
            "entry game @entry.freeze {\n",
            "    state = StateBefore\n",
            "    event = EventAfter\n",
            "    goto @flow.first\n",
            "    route GET \"/:first/:second\" -> @flow.route(first = :first, second = :second)\n",
            "    first_option = 1\n",
            "    second_option = 2\n",
            "}\n",
        ),
        tamper,
    );
}

fn assert_entry_freeze_rejects_from_source(
    case: &str,
    source: &str,
    tamper: impl FnOnce(ItemId, &HirEntryDeclaration) -> (HirEntryDeclaration, HirItemPoisonState),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-entry-freeze-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    let (slots, arenas) = transaction.storage_mut();
    let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
    let HirItemKind::Entry(declaration) = original.kind() else {
        panic!("final Entry item")
    };
    let (declaration, state) = tamper(owner, declaration);
    let replacement = HirItem::try_new_with_state(
        owner,
        original.scope(),
        original.prefix().clone(),
        HirItemKind::Entry(declaration),
        Box::new([]),
        state,
    )
    .unwrap();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    let result = transaction.finish(&mut database);
    assert!(
        matches!(
            &result,
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ),
        "{case}"
    );
    assert!(database.current(&key).is_none());
}

fn assert_entry_route_freeze_rejects(
    case: &str,
    tamper: impl FnOnce(&HirEntryRoute) -> HirEntryRoute,
) {
    assert_entry_freeze_rejects(case, |owner, declaration| {
        let mut members = declaration.members().to_vec();
        let HirEntryMember::Route(route) = &members[3] else {
            panic!("freeze fixture route member")
        };
        let route = tamper(route);
        members[3] = HirEntryMember::Route(route);
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                declaration.kind().clone(),
                declaration.id().clone(),
                declaration.has_header_trailing_recovery(),
                HirEntryBody::braced(members.into_boxed_slice(), declaration.body().is_closed()),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });
}

#[test]
fn entry_freeze_rejects_header_id_and_member_order_substitution() {
    assert_entry_freeze_rejects_from_source(
        "header-recovery",
        "entry game @entry.freeze { missing_option = }\n",
        |owner, declaration| {
            (
                HirEntryDeclaration::try_new(
                    owner.module(),
                    declaration.kind().clone(),
                    declaration.id().clone(),
                    !declaration.has_header_trailing_recovery(),
                    declaration.body().clone(),
                )
                .unwrap(),
                HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember),
            )
        },
    );
    assert_entry_freeze_rejects("id", |owner, declaration| {
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                declaration.kind().clone(),
                HirEntryId::Authored {
                    value: HirIdRefValue::Resolved(HirIdRef::absolute(
                        HirEntityReference::try_new("entry.other".into()).unwrap(),
                    )),
                    canonical_entry_family: true,
                },
                declaration.has_header_trailing_recovery(),
                declaration.body().clone(),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });
    assert_entry_freeze_rejects("member-order", |owner, declaration| {
        let mut members = declaration.members().to_vec();
        members.swap(0, 1);
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                declaration.kind().clone(),
                declaration.id().clone(),
                declaration.has_header_trailing_recovery(),
                HirEntryBody::braced(members.into_boxed_slice(), declaration.body().is_closed()),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });
}

#[test]
fn entry_freeze_rejects_type_option_and_route_binding_substitution() {
    assert_entry_freeze_rejects("type-id-swap", |owner, declaration| {
        let mut members = declaration.members().to_vec();
        let (HirEntryMember::StateType(state), HirEntryMember::EventType(event)) =
            (&members[0], &members[1])
        else {
            panic!("freeze fixture type members")
        };
        let swapped_state = HirEntryTypeBinding::new(
            state.assignment(),
            event.ty(),
            state.has_trailing_recovery(),
        );
        let swapped_event = HirEntryTypeBinding::new(
            event.assignment(),
            state.ty(),
            event.has_trailing_recovery(),
        );
        members[0] = HirEntryMember::StateType(swapped_state);
        members[1] = HirEntryMember::EventType(swapped_event);
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                declaration.kind().clone(),
                declaration.id().clone(),
                declaration.has_header_trailing_recovery(),
                HirEntryBody::braced(members.into_boxed_slice(), declaration.body().is_closed()),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });

    assert_entry_freeze_rejects("option-expression-swap", |owner, declaration| {
        let mut members = declaration.members().to_vec();
        let (HirEntryMember::Option(first), HirEntryMember::Option(second)) =
            (&members[4], &members[5])
        else {
            panic!("freeze fixture option members")
        };
        let first_expression = first.value().expression().unwrap();
        let second_expression = second.value().expression().unwrap();
        let swapped_first = HirEntryOption::new(
            first.name().clone(),
            first.assignment(),
            HirEntryOptionValue::Expression(second_expression),
            first.has_trailing_recovery(),
        );
        let swapped_second = HirEntryOption::new(
            second.name().clone(),
            second.assignment(),
            HirEntryOptionValue::Expression(first_expression),
            second.has_trailing_recovery(),
        );
        members[4] = HirEntryMember::Option(swapped_first);
        members[5] = HirEntryMember::Option(swapped_second);
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                declaration.kind().clone(),
                declaration.id().clone(),
                declaration.has_header_trailing_recovery(),
                HirEntryBody::braced(members.into_boxed_slice(), declaration.body().is_closed()),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });

    assert_entry_freeze_rejects("route-binding-order", |owner, declaration| {
        let mut members = declaration.members().to_vec();
        let HirEntryMember::Route(route) = &members[3] else {
            panic!("freeze fixture route member")
        };
        let HirEntryRouteBindings::Parenthesized { items, closed } = route.bindings() else {
            panic!("freeze fixture route bindings")
        };
        let mut swapped = items.to_vec();
        swapped.swap(0, 1);
        members[3] = HirEntryMember::Route(HirEntryRoute::new(
            route.method().clone(),
            route.path().clone(),
            route.arrow(),
            route.target().clone(),
            HirEntryRouteBindings::Parenthesized {
                items: swapped.into_boxed_slice(),
                closed: *closed,
            },
            route.has_trailing_recovery(),
        ));
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                declaration.kind().clone(),
                declaration.id().clone(),
                declaration.has_header_trailing_recovery(),
                HirEntryBody::braced(members.into_boxed_slice(), declaration.body().is_closed()),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });
}

#[test]
fn entry_freeze_rejects_kind_role_and_route_leaf_substitution() {
    assert_entry_freeze_rejects("kind", |owner, declaration| {
        (
            HirEntryDeclaration::try_new(
                owner.module(),
                HirEntryKind::Server,
                declaration.id().clone(),
                declaration.has_header_trailing_recovery(),
                declaration.body().clone(),
            )
            .unwrap(),
            HirItemPoisonState::Clean,
        )
    });

    assert_entry_freeze_rejects_from_source(
        "role-path",
        concat!(
            "entry game @entry.freeze_role {\n",
            "    initializer = server.initial\n",
            "    reducer = server.reduce\n",
            "}\n",
        ),
        |owner, declaration| {
            let mut members = declaration.members().to_vec();
            let (HirEntryMember::Initializer(initializer), HirEntryMember::Reducer(reducer)) =
                (&members[0], &members[1])
            else {
                panic!("freeze role-path fixture")
            };
            let swapped_initializer = HirEntryPathBinding::new(
                initializer.assignment(),
                reducer.value().clone(),
                initializer.has_trailing_recovery(),
            );
            let swapped_reducer = HirEntryPathBinding::new(
                reducer.assignment(),
                initializer.value().clone(),
                reducer.has_trailing_recovery(),
            );
            members[0] = HirEntryMember::Initializer(swapped_initializer);
            members[1] = HirEntryMember::Reducer(swapped_reducer);
            (
                HirEntryDeclaration::try_new(
                    owner.module(),
                    declaration.kind().clone(),
                    declaration.id().clone(),
                    declaration.has_header_trailing_recovery(),
                    HirEntryBody::braced(
                        members.into_boxed_slice(),
                        declaration.body().is_closed(),
                    ),
                )
                .unwrap(),
                HirItemPoisonState::Clean,
            )
        },
    );

    assert_entry_route_freeze_rejects("route-method", |route| {
        HirEntryRoute::new(
            HirHttpMethodValue::Resolved(HirHttpMethod::Post),
            route.path().clone(),
            route.arrow(),
            route.target().clone(),
            route.bindings().clone(),
            route.has_trailing_recovery(),
        )
    });
    assert_entry_route_freeze_rejects("route-path", |route| {
        HirEntryRoute::new(
            route.method().clone(),
            HirRoutePathValue::Resolved(HirRoutePath::try_new("/changed".into()).unwrap()),
            route.arrow(),
            route.target().clone(),
            route.bindings().clone(),
            route.has_trailing_recovery(),
        )
    });
    assert_entry_route_freeze_rejects("route-target", |route| {
        HirEntryRoute::new(
            route.method().clone(),
            route.path().clone(),
            route.arrow(),
            HirEntryTarget::Authored(HirIdRefValue::Resolved(HirIdRef::absolute(
                HirEntityReference::try_new("flow.changed".into()).unwrap(),
            ))),
            route.bindings().clone(),
            route.has_trailing_recovery(),
        )
    });
}

#[test]
fn entry_freeze_rejects_arena_allocation_for_inline_expression_leaves() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-inline-allocation",
        concat!(
            "entry server @entry.inline_allocation {\n",
            "    goto @flow.goto_target\n",
            "    route GET \"/route\" -> @flow.route_target\n",
            "}\n",
        ),
    );
    let attached = parsed
        .tree()
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Entry(entry) => Some(entry.semantics().unwrap()),
            _ => None,
        })
        .expect("typed Entry attachment");
    let mut inline = Vec::<(&str, AttachedExpressionNode)>::new();
    let AttachedEntryId::Authored { expression, .. } = attached.id() else {
        panic!("authored Entry header ID")
    };
    inline.push(("header-id", expression.as_ref().clone()));
    for member in attached.body().members() {
        match member {
            AttachedEntryMember::Goto { target, .. } => {
                inline.push(("goto-target", target.value().unwrap().clone()));
            }
            AttachedEntryMember::Route { path, target, .. } => {
                inline.push(("route-path", path.value().unwrap().clone()));
                inline.push(("route-target", target.value().unwrap().clone()));
            }
            _ => {}
        }
    }
    assert_eq!(inline.len(), 4);

    for (case, expression) in inline {
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let mut transaction = stage(&database, &parsed, &key);
        transaction
            .lower_attached_source_file_items(&parsed.tree())
            .unwrap();
        let owner = transaction.source_ordered_items[0];
        let scope = {
            let (slots, arenas) = transaction.storage_mut();
            arenas.items().resolve_staged(slots, owner).unwrap().scope()
        };
        transaction
            .lower_attached_expression(&expression, scope)
            .unwrap_or_else(|error| panic!("{case}: inline expression allocation failed: {error}"));
        assert!(matches!(
            transaction.finish(&mut database),
            Err(HirLowerFailure::Invariant(
                HirInvariantFailure::InvalidSourceIndex
            ))
        ));
        assert!(database.current(&key).is_none(), "{case}");
    }
}

#[test]
fn entry_freeze_rejects_an_option_expression_moved_to_a_foreign_scope() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-option-foreign-scope",
        concat!(
            "entry game @entry.foreign_scope { option = 1 }\n",
            "fn scope_donor() = 0\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let entry_owner = transaction.source_ordered_items[0];
    let function_owner = transaction.source_ordered_items[1];
    let (expression, entry_scope, foreign_scope, replacement) = {
        let (slots, arenas) = transaction.storage_mut();
        let entry_item = arenas.items().resolve_staged(slots, entry_owner).unwrap();
        let entry_scope = entry_item.scope();
        let HirItemKind::Entry(entry) = entry_item.kind() else {
            panic!("first fixture item must remain Entry")
        };
        let [HirEntryMember::Option(option)] = entry.members() else {
            panic!("one authored Entry option")
        };
        let expression = option.value().expression().unwrap();
        let function = arenas
            .items()
            .resolve_staged(slots, function_owner)
            .unwrap();
        let HirItemKind::Function(function) = function.kind() else {
            panic!("second fixture item must remain Function")
        };
        let foreign_scope = function.callable_scope();
        let original = arenas
            .expressions()
            .resolve_staged(slots, expression)
            .unwrap();
        (
            expression,
            entry_scope,
            foreign_scope,
            HirExpr::try_new(
                foreign_scope,
                original.kind().clone(),
                original.state().clone(),
            )
            .unwrap(),
        )
    };
    assert_ne!(entry_scope, foreign_scope);
    let (slots, arenas) = transaction.storage_mut();
    arenas
        .expressions()
        .revise_finalized(slots, expression, replacement)
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
fn entry_member_limit_accepts_exact_source_and_rejects_one_over_atomically() {
    let source = |member_count: usize| {
        let mut source = String::from("entry game @entry.member_limit {\n");
        for ordinal in 0..member_count {
            writeln!(source, "    option_{ordinal} = {ordinal}").unwrap();
        }
        source.push_str("}\n");
        source
    };
    let maximum = HirLimit::DeclarationMembers.maximum();
    assert!(preflight_entry_members(maximum).is_ok());

    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-member-limit-exact",
        &source(maximum),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, declaration) = entry(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(declaration.members().len(), maximum);
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert!(Arc::ptr_eq(
        &module,
        &database.current(&key).expect("exact-limit Entry commits")
    ));

    let observed = maximum + 1;
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-member-limit-one-over",
        &source(observed),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    let Err(HirLowerFailure::Limit(error)) =
        transaction.lower_attached_source_file_items(&parsed.tree())
    else {
        panic!("first one-over Entry inventory must fail before child lowering")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), observed);
    assert_eq!(error.maximum(), maximum);
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&key).is_none());
}

#[test]
fn entry_route_binding_limit_accepts_exact_source_and_rejects_one_over_atomically() {
    let source = |binding_count: usize| {
        let mut source = String::from(
            "entry server @entry.route_binding_limit {\n    route GET \"/\" -> @flow.next(",
        );
        for ordinal in 0..binding_count {
            if ordinal != 0 {
                source.push_str(", ");
            }
            write!(source, "parameter_{ordinal} = :capture_{ordinal}").unwrap();
        }
        source.push_str(")\n}\n");
        source
    };
    let maximum = HirLimit::CallArguments.maximum();
    assert!(preflight_entry_route_bindings(maximum).is_ok());

    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-route-binding-limit-exact",
        &source(maximum),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, declaration) = entry(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let [HirEntryMember::Route(route)] = declaration.members() else {
        panic!("exact-limit route remains one Entry member")
    };
    assert_eq!(route.bindings().items().len(), maximum);

    let observed = maximum + 1;
    let parsed = parse(
        "arcweft-test://proof/final-hir-entry-route-binding-limit-one-over",
        &source(observed),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    let Err(HirLowerFailure::Limit(error)) =
        transaction.lower_attached_source_file_items(&parsed.tree())
    else {
        panic!("first one-over route binding inventory must fail before binding lowering")
    };
    assert_eq!(error.limit(), HirLimit::CallArguments);
    assert_eq!(error.observed(), observed);
    assert_eq!(error.maximum(), maximum);
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&key).is_none());
}
