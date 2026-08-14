use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId,
};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeLineId, RuntimeOperationalType, RuntimePlanTypeKind,
};
use arcweft_core::value::RuntimeValue;
use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::leaf::HirLiteral;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::project::{
    HirProject, HirProjectBuilder, HirProjectModule, HirRuntimeExpressionTypeDisposition,
};
use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    RuntimeCallResultShape, RuntimeCheckedTypeProjectionError, RuntimeNormalizedVariantCase,
    RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts, RuntimeReductionConstructor,
    RuntimeRegisteredValueId, RuntimeResolvedCall, RuntimeResolvedCallArgument,
    RuntimeResolvedCallTarget, RuntimeResolvedNominalError, RuntimeResolvedValue,
    RuntimeResolvedVariant, RuntimeResolvedVariantError, RuntimeSemanticFactFamily,
    RuntimeSemanticFactsError, RuntimeSemanticTypeId, RuntimeSequenceKind,
    RuntimeTypeProjectionStep, RuntimeTypeShape, RuntimeUnsupportedTypeShape,
};

fn project_fixture(label: &str, source: &str) -> HirProject {
    let package = CallablePackageId::try_new(format!("runtime-plan-semantic-facts-{label}"))
        .expect("fixture package");
    let path = CanonicalModulePath::crate_root();
    let source_name = SourceName::path(format!("runtime-plan-semantic-facts-{label}.arcw"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-test://runtime-plan/{label}"))
                .expect("fixture document ID"),
            source_name.clone(),
            source,
        )
        .expect("fixture document"),
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(source_name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("attached fixture parse");
    let key = HirModuleKey::new(
        package.clone(),
        path.clone(),
        parsed.document().identity().clone(),
    );
    let mut database = HirDatabase::try_new().expect("HIR database");
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        parsed.document().identity().id().clone(),
        "runtime-plan-semantic-facts-test",
    )
    .expect("fixture symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
        .expect("fixture symbol revision");
    let transaction = database
        .stage_proof_return_project(
            [LoweringRequest::try_new(key, &parsed).expect("lower request")],
            world,
            revision,
            [parsed.document().identity()],
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("final HIR project stages");
    let facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("semantic-facts fixture has no authored Proof return headers");
    let mut outputs = transaction
        .publish_with_semantic_facts(&mut database, facts)
        .expect("final HIR project publishes");
    let module = outputs
        .pop()
        .expect("one semantic-facts fixture module")
        .into_module();
    assert!(outputs.is_empty());
    let project_module = HirProjectModule::try_new(
        &database,
        &package,
        &path,
        parsed.document().identity(),
        module,
    )
    .expect("accepted module lease");
    let mut builder = HirProjectBuilder::new(&database, package);
    builder
        .insert_module(project_module)
        .expect("module insertion");
    builder.finish().expect("fixture project")
}

fn boolean_literal(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .executable_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(id, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::Literal(HirLiteral::Boolean(true))
            )
            .then_some(id)
        })
        .expect("fixture boolean literal")
}

fn flow_item(project: &HirProject) -> arcweft_lang_hir::identity::ItemId {
    project
        .executable_view()
        .expect("clean fixture")
        .items()
        .find(|item| {
            matches!(
                item.item().kind(),
                arcweft_lang_hir::item::HirItemKind::Flow(_)
            )
        })
        .map(arcweft_lang_hir::project::HirProjectItemRef::id)
        .expect("fixture Flow item")
}

fn call_expression(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .executable_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(id, expression)| {
            matches!(expression.kind(), HirExprKind::Call(_)).then_some(id)
        })
        .expect("fixture call expression")
}

fn entity_reference(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .executable_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(id, expression)| {
            matches!(expression.kind(), HirExprKind::EntityReference(_)).then_some(id)
        })
        .expect("fixture entity-reference expression")
}

fn unit_type() -> super::RuntimeNormalizedType {
    normalized_type(0x11, RuntimeTypeShape::Unit)
}

fn normalized_type(marker: u8, shape: RuntimeTypeShape) -> super::RuntimeNormalizedType {
    super::RuntimeNormalizedType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
}

fn boxed_unit_type() -> Box<super::RuntimeNormalizedType> {
    Box::new(unit_type())
}

fn unsupported_range_type() -> super::RuntimeNormalizedType {
    normalized_type(0x70, RuntimeTypeShape::Range(boxed_unit_type()))
}

fn complete_type_input(project: &HirProject) -> RuntimePlanSemanticFactInput {
    let mut input = RuntimePlanSemanticFactInput::new();
    let executable = project.executable_view().expect("executable type fixture");
    for (_, module) in executable.modules() {
        for (owner, _) in module.locals() {
            input
                .push_local_declaration(owner, unit_type())
                .expect("fixture local identity");
        }
        for (owner, _) in module.patterns() {
            input.push_pattern_type(owner, unit_type());
        }
    }
    for owner in executable
        .selected_runtime_expression_type_owners(
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(owner, unit_type());
    }
    input
}

fn local_owners(project: &HirProject) -> Vec<arcweft_lang_hir::identity::LocalId> {
    project
        .executable_view()
        .expect("executable local fixture")
        .modules()
        .flat_map(|(_, module)| module.locals().map(|(owner, _)| owner))
        .collect()
}

#[test]
fn local_declarations_use_one_complete_contiguous_canonical_projection() {
    let project = project_fixture(
        "local-declaration-order",
        "fn root(first: bool, second: bool) { let third = first; second }\n",
    );
    let owners = local_owners(&project);
    assert!(
        owners.len() >= 3,
        "fixture retains parameters and let binding"
    );

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        complete_type_input(&project),
    )
    .expect("complete canonical local projection");

    assert_eq!(
        usize::try_from(facts.local_declaration_table().len()).ok(),
        Some(owners.len())
    );
    for (position, owner) in owners.into_iter().enumerate() {
        assert_eq!(
            facts
                .local_declaration(owner)
                .expect("every executable HIR local has one plan-local ID")
                .get()
                .get(),
            u32::try_from(position).expect("bounded fixture ordinal") + 1
        );
        assert_eq!(
            facts.local_type(owner),
            Some(&unit_type()),
            "the local identity and accepted type are published by one row"
        );
    }
}

#[test]
fn missing_extra_duplicate_and_reordered_local_projections_are_rejected() {
    let project = project_fixture(
        "invalid-local-declarations",
        "fn root(first: bool, second: bool) { first }\n",
    );
    let owners = local_owners(&project);
    assert!(owners.len() >= 2, "fixture retains both parameters");
    let executable = project.executable_view().expect("executable fixture");

    let mut missing = complete_type_input(&project);
    let missing_owner = missing
        .local_declarations
        .pop()
        .expect("fixture local declaration")
        .0;
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, missing)
            .expect_err("a local cannot be omitted"),
        RuntimeSemanticFactsError::MissingLocalDeclaration {
            local: missing_owner,
        }
    );

    let foreign = project_fixture("extra-local-declaration", "fn foreign(value: bool) {}\n");
    let foreign_owner = local_owners(&foreign)[0];
    let mut extra = complete_type_input(&project);
    extra
        .push_local_declaration(foreign_owner, unit_type())
        .expect("bounded extra local identity");
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, extra)
            .expect_err("a foreign local cannot extend the plan domain"),
        RuntimeSemanticFactsError::ExtraLocalDeclaration {
            local: foreign_owner,
        }
    );

    let mut duplicate = complete_type_input(&project);
    duplicate
        .push_local_declaration(owners[0], unit_type())
        .expect("bounded duplicate local identity");
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, duplicate)
            .expect_err("one HIR local cannot receive two plan identities"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::LocalDeclaration,
        }
    );

    let mut reordered = complete_type_input(&project);
    reordered.local_declarations.swap(0, 1);
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, reordered)
            .expect_err("the same local set in a noncanonical order is invalid"),
        RuntimeSemanticFactsError::NonCanonicalLocalDeclarationOrder {
            expected: owners[0],
            actual: owners[1],
        }
    );
}

#[test]
fn semantic_facts_are_bound_to_the_exact_accepted_generation() {
    let first = project_fixture("generation-first", "fn root() {}\n");
    let second = project_fixture("generation-second", "fn root() {}\n");
    let facts = RuntimePlanSemanticFacts::try_new(
        first.executable_view().expect("first executable view"),
        complete_type_input(&first),
    )
    .expect("complete checked fact set");

    assert_eq!(
        facts.validate_generation(first.executable_view().expect("same generation")),
        Ok(())
    );
    assert_eq!(
        facts.validate_generation(second.executable_view().expect("foreign generation")),
        Err(RuntimeSemanticFactsError::WrongProjectGeneration)
    );
}

#[test]
fn checked_literal_fact_uses_the_qualified_expression_owner() {
    let project = project_fixture("literal-owner", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_literal(owner, RuntimeValue::Bool(true));

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("literal fact");
    assert_eq!(
        facts.expression_literal(owner),
        Some(&RuntimeValue::Bool(true))
    );
}

#[test]
fn checked_flow_identity_uses_the_qualified_item_owner() {
    let project = project_fixture("flow-owner", "flow opening {}\n");
    let owner = flow_item(&project);
    let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
    let mut input = complete_type_input(&project);
    input.push_flow(owner, identity.clone());

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("Flow identity fact");
    assert_eq!(facts.flow(owner), Some(&identity));
}

#[test]
fn wrong_expression_family_is_not_reinterpreted() {
    let project = project_fixture("wrong-family", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_value(
        owner,
        RuntimeResolvedValue::Constant(RuntimeValue::Bool(true)),
    );

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("literal cannot masquerade as a resolved path"),
        RuntimeSemanticFactsError::WrongExpressionFamily {
            expression: owner,
            expected: RuntimeSemanticFactFamily::Value,
        }
    );
}

#[test]
fn dialogue_line_fact_owns_the_checked_path_only_runtime_identity() {
    let project = project_fixture(
        "dialogue-line",
        "fn root() {\n    let line: Ref<DialogueLine> = @say.story.greeting\n}\n",
    );
    let owner = entity_reference(&project);
    let line = RuntimeLineId::from_source_entity_body("say.story.greeting")
        .expect("checked dialogue line conversion");
    let mut input = complete_type_input(&project);
    input.push_value(owner, RuntimeResolvedValue::DialogueLine(line.clone()));

    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("typed dialogue-line runtime fact");
    assert_eq!(line.canonical_label(), "story.greeting");
    assert_eq!(
        facts.value(owner),
        Some(&RuntimeResolvedValue::DialogueLine(line))
    );
}

#[test]
fn duplicate_facts_are_rejected_before_publication() {
    let project = project_fixture("duplicate", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_literal(owner, RuntimeValue::Bool(true));
    input.push_expression_literal(owner, RuntimeValue::Bool(false));

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("duplicate fact must fail atomically"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::ExpressionLiteral,
        }
    );
}

#[test]
fn accepted_expression_and_pattern_types_are_complete_and_exact() {
    let project = project_fixture(
        "complete-types",
        "fn root(value: bool) {\n    match value { true => (), false => () }\n}\n",
    );
    let input = complete_type_input(&project);
    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("complete type facts");

    let executable = project.executable_view().expect("executable fixture");
    for owner in executable
        .selected_runtime_expression_type_owners(
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
        .expect("postfix-free runtime expression-type fixture")
    {
        assert_eq!(facts.expression_type(owner), Some(&unit_type()));
    }
    for (_, module) in executable.modules() {
        for (owner, _) in module.patterns() {
            assert_eq!(facts.pattern_type(owner), Some(&unit_type()));
        }
    }
}

#[test]
fn runtime_type_completeness_excludes_effect_metadata_owners() {
    let project = project_fixture(
        "effect-metadata-types",
        "fn root() effects { fs.read } { true }\n",
    );
    let executable = project.executable_view().expect("executable fixture");
    let effect = executable
        .items()
        .find_map(|item| {
            item.item()
                .kind()
                .effect_expression_roots()
                .into_iter()
                .next()
        })
        .expect("fixture effect expression");
    let body = boolean_literal(&project);
    let facts = RuntimePlanSemanticFacts::try_new(executable, complete_type_input(&project))
        .expect("effect metadata requires no runtime expression type");
    assert!(facts.expression_type(effect).is_none());
    assert_eq!(facts.expression_type(body), Some(&unit_type()));

    let mut input = complete_type_input(&project);
    input.push_expression_type(effect, unit_type());
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, input)
            .expect_err("effect metadata cannot publish a runtime expression type"),
        RuntimeSemanticFactsError::InactiveExpressionType { expression: effect },
    );
}

#[test]
fn runtime_type_completeness_uses_the_selected_call_carrier_disposition() {
    let project = project_fixture(
        "agent-call-carrier-types",
        "fn helper(value: bool) -> bool { value }\nfn root(value: bool) { helper(value) }\n",
    );
    let executable = project.executable_view().expect("executable fixture");
    let (call, callee, argument) = executable
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(owner, expression)| {
            let HirExprKind::Call(call) = expression.kind() else {
                return None;
            };
            Some((
                owner,
                call.callee()
                    .value_expression()
                    .expect("fixture value callee"),
                call.arguments()[0].value(),
            ))
        })
        .expect("fixture call expression");
    let call_fact = RuntimeResolvedCall::new(
        RuntimeResolvedCallTarget::Agent(crate::agent::RuntimeAgentIntrinsic::Observation),
        [RuntimeResolvedCallArgument::Authored { ordinal: 0 }],
        RuntimeCallResultShape::Value,
    );
    let accepted = executable
        .selected_runtime_expression_type_owners(
            |_| None,
            |owner| {
                if owner == call {
                    call_fact.expression_type_disposition()
                } else {
                    HirRuntimeExpressionTypeDisposition::Retain
                }
            },
        )
        .expect("postfix-free call-carrier inventory");
    let mut input = RuntimePlanSemanticFactInput::new();
    for (_, module) in executable.modules() {
        for (owner, _) in module.locals() {
            input
                .push_local_declaration(owner, unit_type())
                .expect("fixture local identity");
        }
        for (owner, _) in module.patterns() {
            input.push_pattern_type(owner, unit_type());
        }
    }
    for owner in accepted {
        input.push_expression_type(owner, unit_type());
    }
    input.push_call(call, call_fact);

    let facts = RuntimePlanSemanticFacts::try_new(executable, input)
        .expect("selected Agent carrier requires only operand types");
    assert!(facts.expression_type(call).is_none());
    assert!(facts.expression_type(callee).is_none());
    assert_eq!(facts.expression_type(argument), Some(&unit_type()));
}

#[test]
fn missing_expression_type_is_rejected_before_publication() {
    let project = project_fixture("missing-expression-type", "fn root() { true }\n");
    let owner = boolean_literal(&project);

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            RuntimePlanSemanticFactInput::new(),
        )
        .expect_err("an accepted expression cannot omit its type"),
        RuntimeSemanticFactsError::MissingExpressionType { expression: owner },
    );
}

#[test]
fn missing_pattern_type_is_rejected_before_publication() {
    let project = project_fixture(
        "missing-pattern-type",
        "fn root(value: bool) {\n    match value { true => (), false => () }\n}\n",
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    let executable = project.executable_view().expect("executable fixture");
    for (_, module) in executable.modules() {
        for (owner, _) in module.locals() {
            input
                .push_local_declaration(owner, unit_type())
                .expect("fixture local identity");
        }
    }
    for owner in executable
        .selected_runtime_expression_type_owners(
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(owner, unit_type());
    }
    let pattern = executable
        .modules()
        .flat_map(|(_, module)| module.patterns())
        .map(|(owner, _)| owner)
        .next()
        .expect("pattern fixture");

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, input)
            .expect_err("an accepted pattern cannot omit its type"),
        RuntimeSemanticFactsError::MissingPatternType { pattern },
    );
}

#[test]
fn duplicate_expression_types_are_rejected_before_publication() {
    let project = project_fixture("duplicate-expression-type", "fn root() { true }\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_type(owner, unit_type());

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("one expression cannot own two accepted types"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::ExpressionType,
        },
    );
}

#[test]
fn duplicate_pattern_types_are_rejected_before_publication() {
    let project = project_fixture(
        "duplicate-pattern-type",
        "fn root(value: bool) { match value { true => (), false => () } }\n",
    );
    let pattern = project
        .executable_view()
        .expect("executable fixture")
        .modules()
        .flat_map(|(_, module)| module.patterns())
        .map(|(owner, _)| owner)
        .next()
        .expect("pattern fixture");
    let mut input = complete_type_input(&project);
    input.push_pattern_type(pattern, unit_type());

    assert_eq!(
        RuntimePlanSemanticFacts::try_new(
            project.executable_view().expect("executable fixture"),
            input,
        )
        .expect_err("one pattern cannot own two accepted types"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::PatternType,
        },
    );
}

#[test]
fn every_direct_operational_shape_selects_its_closed_plan_family() {
    let cases = vec![
        (
            RuntimeTypeShape::Range(boxed_unit_type()),
            RuntimeUnsupportedTypeShape::Range,
            RuntimeOperationalType::Range,
        ),
        (
            RuntimeTypeShape::Iterator(boxed_unit_type()),
            RuntimeUnsupportedTypeShape::Iterator,
            RuntimeOperationalType::Iterator,
        ),
        (
            RuntimeTypeShape::Map {
                key: boxed_unit_type(),
                value: boxed_unit_type(),
            },
            RuntimeUnsupportedTypeShape::Map,
            RuntimeOperationalType::Map,
        ),
        (
            RuntimeTypeShape::Need {
                ready: boxed_unit_type(),
                error: boxed_unit_type(),
            },
            RuntimeUnsupportedTypeShape::Need,
            RuntimeOperationalType::Need,
        ),
        (
            RuntimeTypeShape::Stream {
                item: boxed_unit_type(),
                error: boxed_unit_type(),
            },
            RuntimeUnsupportedTypeShape::Stream,
            RuntimeOperationalType::Stream,
        ),
        (
            RuntimeTypeShape::Source {
                item: boxed_unit_type(),
                error: boxed_unit_type(),
            },
            RuntimeUnsupportedTypeShape::Source,
            RuntimeOperationalType::Source,
        ),
        (
            RuntimeTypeShape::ThreadHandle(boxed_unit_type()),
            RuntimeUnsupportedTypeShape::ThreadHandle,
            RuntimeOperationalType::ThreadHandle,
        ),
        (
            RuntimeTypeShape::Shared(boxed_unit_type()),
            RuntimeUnsupportedTypeShape::Shared,
            RuntimeOperationalType::Shared,
        ),
        (
            RuntimeTypeShape::Reference(boxed_unit_type()),
            RuntimeUnsupportedTypeShape::Reference,
            RuntimeOperationalType::Reference,
        ),
        (
            RuntimeTypeShape::Function {
                parameters: vec![unit_type()].into_boxed_slice(),
                result: boxed_unit_type(),
            },
            RuntimeUnsupportedTypeShape::Function,
            RuntimeOperationalType::Function,
        ),
    ];

    for (index, (shape, unsupported, operational)) in cases.into_iter().enumerate() {
        let marker = 0x30_u8 + u8::try_from(index).expect("bounded operational fixture");
        let identity = RuntimeSemanticTypeId::from_bytes([marker; 32]);
        let normalized = super::RuntimeNormalizedType::new(identity, shape);
        assert_eq!(
            normalized.checked_type(),
            Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
                semantic_identity: identity,
                path: super::RuntimeTypeProjectionPath::root(),
                shape: unsupported,
            })
        );
        assert_eq!(
            normalized.runtime_plan_type_kind(),
            Ok(RuntimePlanTypeKind::Operational(operational))
        );
    }
}

#[test]
fn nested_operational_descendants_select_their_outer_composite_family() {
    let cases = vec![
        (
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Vec,
                item: Box::new(unsupported_range_type()),
            },
            RuntimeTypeProjectionStep::SequenceItem,
            RuntimeOperationalType::Sequence,
        ),
        (
            RuntimeTypeShape::Array {
                item: Box::new(unsupported_range_type()),
                length: 1,
            },
            RuntimeTypeProjectionStep::SequenceItem,
            RuntimeOperationalType::Sequence,
        ),
        (
            RuntimeTypeShape::Tuple(vec![unsupported_range_type()].into_boxed_slice()),
            RuntimeTypeProjectionStep::TupleItem(0),
            RuntimeOperationalType::Tuple,
        ),
        (
            RuntimeTypeShape::Choice(vec![unsupported_range_type()].into_boxed_slice()),
            RuntimeTypeProjectionStep::ChoiceAlternative(0),
            RuntimeOperationalType::Choice,
        ),
        (
            RuntimeTypeShape::Result {
                value: Box::new(unsupported_range_type()),
                error: boxed_unit_type(),
            },
            RuntimeTypeProjectionStep::ResultOk,
            RuntimeOperationalType::Result,
        ),
        (
            RuntimeTypeShape::Option(Box::new(unsupported_range_type())),
            RuntimeTypeProjectionStep::OptionItem,
            RuntimeOperationalType::Option,
        ),
    ];

    for (index, (shape, step, operational)) in cases.into_iter().enumerate() {
        let marker = 0x80_u8 + u8::try_from(index).expect("bounded composite fixture");
        let normalized = normalized_type(marker, shape);
        assert_eq!(
            normalized.checked_type(),
            Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
                semantic_identity: RuntimeSemanticTypeId::from_bytes([0x70; 32]),
                path: super::RuntimeTypeProjectionPath::root().pushed(step),
                shape: RuntimeUnsupportedTypeShape::Range,
            })
        );
        assert_eq!(
            normalized.runtime_plan_type_kind(),
            Ok(RuntimePlanTypeKind::Operational(operational))
        );
    }
}

#[test]
fn complete_checked_composites_retain_their_exact_checked_predicate() {
    let normalized = normalized_type(
        0x90,
        RuntimeTypeShape::Result {
            value: Box::new(normalized_type(
                0x91,
                RuntimeTypeShape::Option(boxed_unit_type()),
            )),
            error: Box::new(normalized_type(
                0x92,
                RuntimeTypeShape::Sequence {
                    kind: RuntimeSequenceKind::Seq,
                    item: Box::new(normalized_type(0x93, RuntimeTypeShape::Bool)),
                },
            )),
        },
    );

    assert_eq!(
        normalized.runtime_plan_type_kind(),
        Ok(RuntimePlanTypeKind::Checked(RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Option(Box::new(
                RuntimeCheckedType::Unit
            ))),
            error: Box::new(RuntimeCheckedType::Sequence(Box::new(
                RuntimeCheckedType::Bool
            ))),
        }))
    );
}

#[test]
fn opaque_and_nominal_checked_results_remain_atomic_checked_types() {
    let opaque_identity = RuntimeSemanticTypeId::from_bytes([0xa0; 32]);
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.runtime-plan.atomic-opaque")
        .expect("valid fixture producer");
    let opaque = super::RuntimeNormalizedType::new(
        opaque_identity,
        RuntimeTypeShape::Opaque {
            producer: producer.clone(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
        },
    );
    assert_eq!(
        opaque.runtime_plan_type_kind(),
        Ok(RuntimePlanTypeKind::Checked(RuntimeCheckedType::Opaque {
            owner: RuntimeOpaqueTypeOwner::exact(producer, opaque_identity),
        }))
    );

    let nominal = RuntimeCheckedType::Nominal {
        nominal: RuntimeNominalTypeId::try_new("fixture.runtime-plan.AtomicNominal")
            .expect("valid fixture nominal"),
        semantic_identity: RuntimeSemanticTypeId::from_bytes([0xa1; 32]),
        layout: TypeLayoutHash::from_bytes([0xa2; 32]),
    };
    // Project nominal declaration IDs are intentionally issued outside this
    // crate. Exercise the exact successful projection result here without
    // adding a forgeable nominal constructor solely for a unit test.
    assert_eq!(
        unit_type().classify_runtime_plan_type_projection(Ok(nominal.clone())),
        Ok(RuntimePlanTypeKind::Checked(nominal))
    );
}

#[test]
fn non_unsupported_projection_errors_are_returned_unchanged() {
    let identity = RuntimeSemanticTypeId::from_bytes([0xb0; 32]);
    let errors = [
        RuntimeCheckedTypeProjectionError::MissingOpaqueProducerEvidence {
            semantic_identity: identity,
            path: super::RuntimeTypeProjectionPath::root(),
            type_label: "fixture.missing-opaque".to_owned(),
        },
        RuntimeCheckedTypeProjectionError::InvalidProjectNominal {
            semantic_identity: identity,
            path: super::RuntimeTypeProjectionPath::root(),
            reason: RuntimeResolvedNominalError::InvalidIdentity(
                RuntimeNominalTypeId::try_new("").expect_err("empty nominal is invalid"),
            ),
        },
    ];

    for error in errors {
        assert_eq!(
            unit_type().classify_runtime_plan_type_projection(Err(error.clone())),
            Err(error)
        );
    }
}

#[test]
fn nested_operational_expression_type_is_retained_without_reconstruction() {
    let project = project_fixture("nested-operational-type", "fn root(value: bool) { true }\n");
    let owner = boolean_literal(&project);
    let leaf = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x22; 32]),
        RuntimeTypeShape::Unit,
    );
    let range = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x33; 32]),
        RuntimeTypeShape::Range(Box::new(leaf)),
    );
    let nested = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x44; 32]),
        RuntimeTypeShape::Option(Box::new(range)),
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    let executable = project.executable_view().expect("executable fixture");
    for (_, module) in executable.modules() {
        for (local, _) in module.locals() {
            input
                .push_local_declaration(local, nested.clone())
                .expect("fixture local identity");
        }
        for (pattern, _) in module.patterns() {
            input.push_pattern_type(pattern, nested.clone());
        }
    }
    for expression in executable
        .selected_runtime_expression_type_owners(
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(expression, nested.clone());
    }
    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("nested operational fact remains accepted semantic data");

    assert_eq!(facts.expression_type(owner), Some(&nested));
    let local = local_owners(&project)
        .into_iter()
        .next()
        .expect("root fixture retains a local");
    assert_eq!(facts.local_type(local), Some(&nested));
    assert_eq!(
        facts
            .expression_type(owner)
            .expect("exact retained type")
            .checked_type(),
        Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
            semantic_identity: RuntimeSemanticTypeId::from_bytes([0x33; 32]),
            path: super::RuntimeTypeProjectionPath::root()
                .pushed(RuntimeTypeProjectionStep::OptionItem),
            shape: RuntimeUnsupportedTypeShape::Range,
        }),
    );
}

#[test]
fn postfix_type_completeness_keeps_only_the_selected_candidate_expression_tree() {
    let project = project_fixture(
        "postfix-selected-types",
        "fn root(items: Vec<i64>, subject: i64) {\n    items[{ match subject { value => value }; 0 }]\n}\n",
    );
    let executable = project.executable_view().expect("executable fixture");
    let modules = executable
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let (postfix_owner, target, index, dialogue) = modules
        .values()
        .flat_map(|module| module.expressions())
        .find_map(|(owner, expression)| {
            let HirExprKind::PostfixBracket(postfix) = expression.kind() else {
                return None;
            };
            let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
            else {
                return None;
            };
            Some((owner, postfix.target(), *index, *dialogue))
        })
        .expect("ambiguous postfix fixture");
    assert!(
        modules
            .values()
            .flat_map(|module| module.patterns())
            .next()
            .is_some(),
        "the ordinary candidate retains a Match pattern"
    );

    let postfix_candidates = BTreeMap::from([(postfix_owner, dialogue)]);
    let semantic = executable
        .selected_expression_owners(|owner| postfix_candidates.get(&owner).copied())
        .expect("selected semantic candidate inventory");
    assert!(semantic.contains(&postfix_owner));
    assert!(semantic.contains(&target));
    assert!(semantic.contains(&dialogue));
    assert!(!semantic.contains(&index));
    let accepted = executable
        .selected_runtime_expression_type_owners(
            |owner| postfix_candidates.get(&owner).copied(),
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
        .expect("selected runtime expression-type inventory");
    assert!(!accepted.contains(&postfix_owner));
    assert!(accepted.contains(&target));
    assert!(!accepted.contains(&dialogue));
    assert!(!accepted.contains(&index));

    let complete_selected_input = || {
        let mut input = RuntimePlanSemanticFactInput::new();
        for (_, module) in executable.modules() {
            for (owner, _) in module.locals() {
                input
                    .push_local_declaration(owner, unit_type())
                    .expect("fixture local identity");
            }
        }
        for owner in &accepted {
            input.push_expression_type(*owner, unit_type());
        }
        for module in modules.values() {
            for (owner, _) in module.patterns() {
                input.push_pattern_type(owner, unit_type());
            }
        }
        input.push_postfix_candidate(postfix_owner, dialogue);
        input
    };
    let facts = RuntimePlanSemanticFacts::try_new(executable, complete_selected_input())
        .expect("the rolled-back expression candidate needs no type fact");
    assert!(facts.expression_type(postfix_owner).is_none());
    assert!(facts.expression_type(target).is_some());
    assert!(facts.expression_type(dialogue).is_none());
    assert!(facts.expression_type(index).is_none());

    let mut input = complete_selected_input();
    input.push_expression_type(dialogue, unit_type());
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, input)
            .expect_err("a selected dialogue carrier cannot publish an expression type"),
        RuntimeSemanticFactsError::InactiveExpressionType {
            expression: dialogue
        },
    );

    let mut input = complete_selected_input();
    input.push_expression_type(index, unit_type());
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, input)
            .expect_err("an unselected candidate cannot publish an expression type"),
        RuntimeSemanticFactsError::InactiveExpressionType { expression: index },
    );
}

#[test]
fn semantic_identities_round_trip_without_display_labels() {
    let type_bytes = [0x5a; 32];
    let registered_bytes = [0xa5; 32];
    assert_eq!(
        RuntimeSemanticTypeId::from_bytes(type_bytes).as_bytes(),
        &type_bytes
    );
    assert_eq!(
        RuntimeRegisteredValueId::from_bytes(registered_bytes).as_bytes(),
        &registered_bytes
    );
}

#[test]
fn reduction_constructor_fact_requires_one_authored_value_argument() {
    let project = project_fixture(
        "reduction-constructor",
        "fn root(value: i32) {\n    identity(value)\n}\n",
    );
    let owner = call_expression(&project);
    let valid = RuntimeResolvedCall::new(
        RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged),
        [RuntimeResolvedCallArgument::Authored { ordinal: 0 }],
        RuntimeCallResultShape::Value,
    );
    let mut input = complete_type_input(&project);
    input.push_call(owner, valid.clone());
    let facts = RuntimePlanSemanticFacts::try_new(
        project.executable_view().expect("executable fixture"),
        input,
    )
    .expect("typed Reduction constructor fact");
    assert_eq!(facts.call(owner), Some(&valid));

    for invalid in [
        RuntimeResolvedCall::new(
            RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged),
            [],
            RuntimeCallResultShape::Value,
        ),
        RuntimeResolvedCall::new(
            RuntimeResolvedCallTarget::Reduction(RuntimeReductionConstructor::Unchanged),
            [RuntimeResolvedCallArgument::Authored { ordinal: 0 }],
            RuntimeCallResultShape::PartialFunction,
        ),
    ] {
        let mut input = complete_type_input(&project);
        input.push_call(owner, invalid);
        assert_eq!(
            RuntimePlanSemanticFacts::try_new(
                project.executable_view().expect("executable fixture"),
                input,
            )
            .expect_err("fabricated Reduction constructor fact must fail"),
            RuntimeSemanticFactsError::InvalidReductionConstructorCall,
        );
    }
}

#[test]
fn opaque_composite_projection_preserves_complete_owner_and_first_error_path() {
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.runtime-plan.opaque")
        .expect("valid fixture producer");
    let opaque = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([1; 32]),
        RuntimeTypeShape::Opaque {
            producer: producer.clone(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
        },
    );
    let closed = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([2; 32]),
        RuntimeTypeShape::Result {
            value: Box::new(opaque.clone()),
            error: Box::new(opaque),
        },
    );
    assert!(matches!(
        closed.checked_type().expect("complete opaque Result owner"),
        RuntimeCheckedType::Result { ok, error }
            if matches!(*ok, RuntimeCheckedType::Opaque { .. })
                && matches!(*error, RuntimeCheckedType::Opaque { .. })
    ));

    let unsupported = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([3; 32]),
        RuntimeTypeShape::Result {
            value: Box::new(super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes([4; 32]),
                RuntimeTypeShape::Range(Box::new(super::RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([5; 32]),
                    RuntimeTypeShape::Unit,
                ))),
            )),
            error: Box::new(super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes([6; 32]),
                RuntimeTypeShape::Function {
                    parameters: Box::new([]),
                    result: Box::new(super::RuntimeNormalizedType::new(
                        RuntimeSemanticTypeId::from_bytes([7; 32]),
                        RuntimeTypeShape::Unit,
                    )),
                },
            )),
        },
    );
    assert_eq!(
        unsupported.checked_type(),
        Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
            semantic_identity: RuntimeSemanticTypeId::from_bytes([4; 32]),
            path: super::RuntimeTypeProjectionPath::root()
                .pushed(RuntimeTypeProjectionStep::ResultOk),
            shape: RuntimeUnsupportedTypeShape::Range,
        })
    );
}

#[test]
fn checked_variant_selection_retains_both_result_branches() {
    let ok = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([8; 32]),
        RuntimeTypeShape::Unit,
    );
    let error = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([9; 32]),
        RuntimeTypeShape::String,
    );
    let variant =
        RuntimeResolvedVariant::result(ok.clone(), error, 0, "Ok").expect("accepted Result case");
    assert_eq!(
        variant
            .selected_payload_type()
            .expect("selected normalized Result payload"),
        Some(&ok)
    );
    let selection = variant
        .checked_selection()
        .expect("complete Result selection");
    assert_eq!(selection.ordinal(), 0);
    assert_eq!(selection.name(), "Ok");
    assert_eq!(selection.payload(), Some(&RuntimeCheckedType::Unit));
    assert_eq!(
        selection.owner(),
        &RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Unit),
            error: Box::new(RuntimeCheckedType::String),
        }
    );
}

#[test]
fn option_and_character_cases_use_the_shared_normalized_selection_path() {
    let item = normalized_type(0x71, RuntimeTypeShape::Unit);
    let some =
        RuntimeResolvedVariant::option(item.clone(), 0, "Some").expect("accepted Option Some case");
    assert_eq!(some.selected_name(), Ok("Some"));
    assert_eq!(some.selected_payload_type(), Ok(Some(&item)));
    assert_eq!(
        some.checked_selection()
            .expect("Some checked selection")
            .payload(),
        Some(&RuntimeCheckedType::Unit)
    );

    let none = RuntimeResolvedVariant::option(item, 1, "None").expect("accepted Option None case");
    assert_eq!(none.selected_name(), Ok("None"));
    assert_eq!(none.selected_payload_type(), Ok(None));
    assert!(
        none.checked_selection()
            .expect("None checked selection")
            .payload()
            .is_none()
    );

    let character = RuntimeResolvedVariant::character(
        RuntimeSemanticTypeId::from_bytes([0x72; 32]),
        RuntimeNominalTypeId::try_new("fixture.CharacterState")
            .expect("valid Character fixture nominal"),
        [
            RuntimeNormalizedVariantCase::new("Idle", None),
            RuntimeNormalizedVariantCase::new("Speaking", None),
        ]
        .into(),
        1,
        "Speaking",
    )
    .expect("accepted payload-free Character case");
    assert_eq!(character.selected_name(), Ok("Speaking"));
    assert_eq!(character.selected_payload_type(), Ok(None));
    assert_eq!(
        character
            .checked_selection()
            .expect("Character checked selection")
            .name(),
        "Speaking"
    );
}

#[test]
fn normalized_variant_case_table_is_the_only_selected_payload_authority() {
    let payload = normalized_type(0x81, RuntimeTypeShape::String);
    let cases = || {
        vec![
            RuntimeNormalizedVariantCase::new("Empty", None),
            RuntimeNormalizedVariantCase::new("Payload", Some(payload.clone())),
        ]
        .into_boxed_slice()
    };
    let identity = RuntimeSemanticTypeId::from_bytes([0x82; 32]);
    let nominal =
        RuntimeNominalTypeId::try_new("fixture.NormalizedVariant").expect("valid fixture nominal");
    let variant =
        RuntimeResolvedVariant::builtin_closed(identity, nominal.clone(), cases(), 1, "Payload")
            .expect("name and ordinal select the normalized row");
    assert_eq!(variant.selected_name(), Ok("Payload"));
    assert_eq!(variant.selected_payload_type(), Ok(Some(&payload)));

    let selection = variant
        .checked_selection()
        .expect("checked view derives from the normalized table");
    assert_eq!(selection.name(), "Payload");
    assert_eq!(selection.payload(), Some(&RuntimeCheckedType::String));
    let RuntimeCheckedType::Variant {
        cases: checked_cases,
        ..
    } = selection.owner()
    else {
        panic!("base-environment owner projects as a checked variant");
    };
    assert_eq!(checked_cases.len(), 2);
    assert!(checked_cases[0].payload.is_none());
    assert_eq!(
        checked_cases[1].payload.as_deref(),
        Some(&RuntimeCheckedType::String)
    );

    assert!(matches!(
        RuntimeResolvedVariant::builtin_closed(identity, nominal, cases(), 1, "Other"),
        Err(RuntimeResolvedVariantError::CaseName {
            ordinal: 1,
            expected,
            actual,
        }) if expected == "Payload" && actual == "Other"
    ));
}

#[test]
fn operational_variant_payload_is_not_admitted_through_raw_facts() {
    let variant = RuntimeResolvedVariant::builtin_closed(
        RuntimeSemanticTypeId::from_bytes([0x83; 32]),
        RuntimeNominalTypeId::try_new("fixture.OperationalVariant").expect("valid fixture nominal"),
        [RuntimeNormalizedVariantCase::new(
            "Payload",
            Some(unsupported_range_type()),
        )]
        .into(),
        0,
        "Payload",
    )
    .expect("normalized selection itself is structurally complete");
    assert_eq!(
        super::validate_variant(&BTreeMap::new(), &variant),
        Err(RuntimeSemanticFactsError::WrongVariantIdentity)
    );
}
