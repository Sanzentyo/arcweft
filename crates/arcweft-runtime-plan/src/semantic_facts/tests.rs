use super::RuntimeFlowFact;
#[path = "tests/character_dialogue.rs"]
mod character_dialogue;
#[path = "tests/dialogue_target.rs"]
mod dialogue_target;
#[path = "tests/scope_continuations.rs"]
mod scope_continuations;
use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::entry::{
    RuntimeCallableId, RuntimeMapKind, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
    RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField,
    RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
    RuntimeSchemaLimits, RuntimeSchemaValueField, RuntimeTypeSchema,
};
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId,
};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeAgentOperationalType, RuntimeBuiltinIteratorFamily, RuntimeLineId,
    RuntimeOperationalType, RuntimePlanTypeProjection,
};
use arcweft_core::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRecordFieldId,
    RuntimeValue,
};
use arcweft_lang_hir::database::HirDatabase;
use arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates;
use arcweft_lang_hir::expr::{HirExprKind, HirSelectedMember};
use arcweft_lang_hir::item::{HirImplMember, HirItemKind};
use arcweft_lang_hir::leaf::HirLiteral;
use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
use arcweft_lang_hir::project::{
    HirProject, HirProjectBuilder, HirProjectModule, HirRuntimeCallCalleeDisposition,
    HirRuntimeEmissionMode, HirRuntimeExecutableOwner, HirRuntimeExpressionProjection,
    HirRuntimeIteratorWitnessMethodRole, HirRuntimeReachabilityEdge,
    HirRuntimeReachabilityEdgeKind, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeReachabilitySite, HirRuntimeSemanticReachability,
    HirRuntimeSemanticReachabilityInput, HirRuntimeValueRetention,
    HirSelectedCallExpressionDisposition, HirSelectedCallExpressionInventory,
};
use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_hir::symbol::{
    CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallablePackageId,
    ImplMethodDeclarationId, ProjectExternalDeclarations, ProjectSymbolRevision,
    ProjectSymbolTable, ProjectSymbolWorldId,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::incremental::SyntaxDatabase;
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    RuntimeAgentTypeShape, RuntimeAssignmentFact, RuntimeBuiltinIteratorFact,
    RuntimeCallResultShape, RuntimeCallableAttachedContentAbi, RuntimeCheckedTypeProjectionError,
    RuntimeDropFadeFact, RuntimeDropPolicyFact, RuntimeEvaluatedEffect, RuntimeEvaluatedEffectFact,
    RuntimeEvaluatedEffectOperandFact, RuntimeIteratorFact, RuntimeIteratorWitnessExecutableFact,
    RuntimeIteratorWitnessFact, RuntimeNormalizedVariantCase, RuntimePlanSemanticFactInput,
    RuntimePlanSemanticFacts, RuntimePositionedAttachedContent, RuntimeProjectCallable,
    RuntimeRecordTypeField, RuntimeRegisteredValueId, RuntimeResolvedAttachedContent,
    RuntimeResolvedCall, RuntimeResolvedCallDispatch, RuntimeResolvedCallError,
    RuntimeResolvedCallOperand, RuntimeResolvedCallOperandBinding,
    RuntimeResolvedCallOperandOrigin, RuntimeResolvedCallOperandProjection,
    RuntimeResolvedCallOperandSource, RuntimeResolvedNominal, RuntimeResolvedPlace,
    RuntimeResolvedSelect, RuntimeResolvedStaticCallTarget, RuntimeResolvedValue,
    RuntimeResolvedVariant, RuntimeResolvedVariantError, RuntimeSemanticFactFamily,
    RuntimeSemanticFactsError, RuntimeSemanticOwnerSet, RuntimeSemanticTypeId, RuntimeSequenceKind,
    RuntimeTraitIdentity, RuntimeTraitMethodFact, RuntimeTraitMethodInstanceKey,
    RuntimeTriggerAdmissionKind, RuntimeTypeProjectionStep, RuntimeTypeShape,
    RuntimeUnsupportedTypeShape, validate_iterator_witness_method_edges,
};

#[test]
fn attached_content_consumes_only_the_terminal_call_abi_position() {
    let project = project_fixture("attached-content-terminal-abi", "fn root() { true }\n");
    let source = boolean_literal(&project);
    let content = RuntimeResolvedAttachedContent::OptionalPresent {
        source,
        ty: option_unit_type(),
    };

    let accepted = RuntimeResolvedCall::try_new(
        RuntimeResolvedCallDispatch::Value { callee: source },
        arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).expect("initial group"),
        Vec::new(),
        Some(RuntimePositionedAttachedContent::new(0, content.clone())),
        None,
        RuntimeCallResultShape::Value,
    )
    .expect("attached content is the terminal ABI member");
    assert_eq!(accepted.attached_content(), Some(&content));
    assert_eq!(accepted.completed_group().get(), 0);
    assert_eq!(
        accepted
            .positioned_attached_content()
            .expect("retained attached-content ABI coordinate")
            .abi_position(),
        0
    );
    assert!(accepted.operands().is_empty());

    let ordinary = RuntimeResolvedCallOperand::new(
        0,
        RuntimeResolvedCallOperandOrigin::Argument {
            argument: 0,
            slot: 0,
        },
        RuntimeResolvedCallOperandSource::Expression(source),
        unit_type(),
        RuntimeResolvedCallOperandBinding::Positional,
        RuntimeResolvedCallOperandProjection::Scalar,
        None,
        Some(request_role_fixture()),
    );
    let accepted = RuntimeResolvedCall::try_new(
        RuntimeResolvedCallDispatch::Value { callee: source },
        arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).expect("initial group"),
        vec![ordinary],
        Some(RuntimePositionedAttachedContent::new(1, content.clone())),
        None,
        RuntimeCallResultShape::Value,
    )
    .expect("attached content follows all ordinary ABI members");
    assert_eq!(accepted.operands().len(), 1);
    assert_eq!(accepted.attached_content(), Some(&content));
    assert_eq!(
        accepted
            .positioned_attached_content()
            .expect("retained attached-content ABI coordinate")
            .abi_position(),
        1
    );

    assert_eq!(
        RuntimeResolvedCall::try_new(
            RuntimeResolvedCallDispatch::Value { callee: source },
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0)
                .expect("initial group"),
            Vec::new(),
            Some(RuntimePositionedAttachedContent::new(1, content)),
            None,
            RuntimeCallResultShape::Value,
        )
        .expect_err("a detached or gapped attached-content slot must be rejected"),
        RuntimeResolvedCallError::NonTerminalAttachedContentPosition {
            expected: 0,
            actual: 1,
        }
    );
}

#[test]
fn call_operands_retain_source_order_and_derive_abi_order() {
    let project = project_fixture("call-source-order", "fn root() { true }\n");
    let source = boolean_literal(&project);
    let operand = |argument, abi_position, binding, projection| {
        RuntimeResolvedCallOperand::new(
            abi_position,
            RuntimeResolvedCallOperandOrigin::Argument { argument, slot: 0 },
            RuntimeResolvedCallOperandSource::Expression(source),
            unit_type(),
            binding,
            projection,
            None,
            Some(request_role_fixture()),
        )
    };
    let source_row = vec![
        operand(
            0,
            1,
            RuntimeResolvedCallOperandBinding::Named(
                arcweft_lang_syntax::name::CallArgumentName::try_new("second").unwrap(),
            ),
            RuntimeResolvedCallOperandProjection::Scalar,
        ),
        operand(
            1,
            0,
            RuntimeResolvedCallOperandBinding::Positional,
            RuntimeResolvedCallOperandProjection::SpreadContainer(
                super::RuntimeResolvedSpreadContainer::Vec,
            ),
        ),
    ];
    let call = RuntimeResolvedCall::try_new(
        RuntimeResolvedCallDispatch::Value { callee: source },
        arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).expect("initial group"),
        source_row.clone(),
        None,
        None,
        RuntimeCallResultShape::Value,
    )
    .expect("source order and ABI order are independent checked axes");
    assert_eq!(call.operands(), source_row);
    assert_eq!(
        call.abi_operands()
            .map(RuntimeResolvedCallOperand::abi_position)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        call.abi_operands()
            .map(RuntimeResolvedCallOperand::origin)
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            RuntimeResolvedCallOperandOrigin::Argument {
                argument: 1,
                slot: 0,
            },
            RuntimeResolvedCallOperandOrigin::Argument {
                argument: 0,
                slot: 0,
            },
        ]
    );

    let mut reversed_source = source_row.clone();
    reversed_source.reverse();
    assert_eq!(
        RuntimeResolvedCall::try_new(
            RuntimeResolvedCallDispatch::Value { callee: source },
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0)
                .expect("initial group"),
            reversed_source,
            None,
            None,
            RuntimeCallResultShape::Value,
        ),
        Err(RuntimeResolvedCallError::NonCanonicalSourceOrder)
    );
    let mut duplicate_abi = source_row;
    duplicate_abi[1] = operand(
        1,
        1,
        RuntimeResolvedCallOperandBinding::Positional,
        RuntimeResolvedCallOperandProjection::Scalar,
    );
    assert_eq!(
        RuntimeResolvedCall::try_new(
            RuntimeResolvedCallDispatch::Value { callee: source },
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0)
                .expect("initial group"),
            duplicate_abi,
            None,
            None,
            RuntimeCallResultShape::Value,
        ),
        Err(RuntimeResolvedCallError::DuplicateAbiPosition { position: 1 })
    );
}

#[test]
fn project_callable_attached_interface_is_consumed_only_by_its_terminal_group() {
    let project = project_fixture(
        "project-attached-interface",
        "fn content(first: String)(second: String)[body: DialogueContent] -> DialogueContent { body }\nfn __runtime_plan_test_probe() -> Unit { () }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let executable = project.analysis_view().expect("clean fixture");
    let item = executable
        .items()
        .find(|item| {
            matches!(
                item.item().kind(),
                HirItemKind::Function(function)
                    if function.name().resolved().is_some_and(|name| name.as_str() == "content")
            )
        })
        .expect("content function");
    let HirItemKind::Function(function) = item.item().kind() else {
        unreachable!("selected content function")
    };
    let attached = function
        .attached_content()
        .expect("attached-content declaration");
    let declaration = CallableDeclarationKey::Existing(
        CallableDeclarationId::try_new(
            executable.package().clone(),
            item.module_path().clone(),
            CallableDeclarationOwner::Function,
            "content",
        )
        .expect("content declaration"),
    );
    let terminal =
        arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(1).expect("terminal group");
    let content_owner = arcweft_core::value::RuntimeDialogueOpaqueRole::Content.exact_owner();
    let content_type = super::RuntimeNormalizedType::new(
        content_owner.semantic_identity(),
        RuntimeTypeShape::Opaque {
            producer: content_owner.producer().clone(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        },
    );
    let string_type = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes(
            *arcweft_lang_sema::types::TypeKind::String
                .semantic_identity_digest()
                .unwrap()
                .as_bytes(),
        ),
        RuntimeTypeShape::String,
    );
    let interface = RuntimeCallableAttachedContentAbi::try_new(
        terminal,
        1,
        arcweft_lang_sema::callable::CallableParameterPresence::Required,
        attached.binding(),
        content_type.clone(),
        content_type.clone(),
        None,
    )
    .expect("runtime attached interface");
    let callable = RuntimeProjectCallable::try_new(
        declaration.clone(),
        analysis
            .accepted_declaration_identity(&declaration)
            .expect("accepted definition identity"),
        item.id(),
        arcweft_lang_hir::source_index::HirCallableSourceOwner::Item,
        RuntimeCallableId::try_new("content").expect("runtime callable"),
        Some(interface),
    )
    .expect("runtime project callable");
    let dispatch = || {
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
            callable.clone(),
        ))
    };
    let initial =
        arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).expect("initial group");
    RuntimeResolvedCall::try_new(
        dispatch(),
        initial,
        Vec::new(),
        None,
        None,
        RuntimeCallResultShape::PartialFunction,
    )
    .expect("nonterminal group does not consume attached content");

    let source = executable
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .map(|(owner, _)| owner)
        .next()
        .expect("fixture expression");
    RuntimeResolvedCall::try_new(
        dispatch(),
        terminal,
        vec![RuntimeResolvedCallOperand::new(
            0,
            RuntimeResolvedCallOperandOrigin::Argument {
                argument: 0,
                slot: 0,
            },
            RuntimeResolvedCallOperandSource::Expression(source),
            string_type,
            RuntimeResolvedCallOperandBinding::Positional,
            RuntimeResolvedCallOperandProjection::Scalar,
            None,
            Some(request_role_fixture()),
        )],
        Some(RuntimePositionedAttachedContent::new(
            1,
            RuntimeResolvedAttachedContent::Required {
                source,
                ty: content_type,
            },
        )),
        None,
        RuntimeCallResultShape::Value,
    )
    .expect("terminal group consumes the exact attached ABI row");
    assert_eq!(
        RuntimeResolvedCall::try_new(
            dispatch(),
            terminal,
            Vec::new(),
            None,
            None,
            RuntimeCallResultShape::Value,
        )
        .expect_err("terminal group cannot omit its attached ABI row"),
        RuntimeResolvedCallError::AttachedContentInterfaceMismatch
    );
}

fn request_role_fixture() -> arcweft_lang_sema::callable::CheckedCallRequestRoleIdentity {
    let project = project_fixture(
        "request-role",
        "fn target(value: bool) -> bool { value }\nfn root() -> bool { target(true) }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    analysis
        .calls()
        .find_map(|(_, facts)| {
            facts
                .selected_application()?
                .core()
                .execution()
                .arguments()
                .first()?
                .slots()
                .first()
                .map(|slot| slot.request_role_identity())
        })
        .expect("one real checked parameter role")
}

fn analyze_identity_fixture(
    project: &HirProject,
) -> arcweft_lang_sema::final_analysis::FinalSemanticAnalysis {
    let document = Arc::clone(
        project
            .view()
            .modules()
            .next()
            .unwrap()
            .1
            .provenance()
            .document(),
    );
    let world = ProjectSymbolWorldId::try_new(
        project.package().clone(),
        document.identity().id().clone(),
        "accepted-callable-identity",
    )
    .unwrap();
    let registrations = arcweft_lang_sema::registration::ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let registered = arcweft_lang_sema::registration::CharacterRegistrar::register(
        arcweft_lang_sema::registration::CharacterRegistrationRequest::new(
            Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            project.view(),
            &registrations,
            None,
        ),
    )
    .unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    arcweft_lang_sema::final_analysis::analyze_final_project(
        project.analysis_view().unwrap(),
        registered.symbols(),
        arcweft_lang_sema::final_analysis::FinalSemanticCatalogs::production(&registered),
        arcweft_lang_sema::final_analysis::FinalSemanticAnalysisControl::new(&cancelled),
    )
    .unwrap()
}

#[test]
fn project_callable_keeps_stable_definition_metadata_across_body_and_source_revisions() {
    let build = |source: &str| {
        let project = project_fixture("definition-stability", source);
        let analysis = analyze_identity_fixture(&project);
        let executable = project.analysis_view().unwrap();
        let item = executable
            .items()
            .find(|item| {
                matches!(item.item().kind(), HirItemKind::Function(function)
                if function.name().resolved().is_some_and(|name| name.as_str() == "target"))
            })
            .unwrap();
        let declaration = CallableDeclarationKey::Existing(
            CallableDeclarationId::try_new(
                executable.package().clone(),
                item.module_path().clone(),
                CallableDeclarationOwner::Function,
                "target",
            )
            .unwrap(),
        );
        let checked = analysis
            .checked_callables()
            .project_callable(&declaration)
            .unwrap();
        let definition = analysis
            .accepted_declaration_identity(&declaration)
            .unwrap();
        RuntimeProjectCallable::try_new(
            declaration,
            definition,
            item.id(),
            arcweft_lang_hir::source_index::HirCallableSourceOwner::Item,
            RuntimeCallableId::from_checked_digest(checked.id().semantic_digest().into_bytes()),
            None,
        )
        .unwrap()
    };
    let before =
        build("fn target() -> i64 { 1i64 }\nfn __runtime_plan_test_probe() -> Unit { () }\n");
    let after = build(
        "\n\n// moved source offsets and changed body\nfn target() -> i64 { 2i64 }\nfn __runtime_plan_test_probe() -> Unit { () }\n",
    );
    assert_eq!(before.accepted_identity(), after.accepted_identity());
    assert_ne!(before.runtime(), after.runtime());
}

#[test]
fn project_callable_rejects_an_accepted_identity_from_another_declaration() {
    let project = project_fixture(
        "definition-relation",
        "fn first() -> Unit { () }\nfn second() -> Unit { () }\nfn __runtime_plan_test_probe() -> Unit { () }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let executable = project.analysis_view().unwrap();
    let mut declarations =
        analysis
            .checked_callables()
            .records()
            .filter_map(|facts| match facts.id().declaration() {
                arcweft_lang_sema::callable::CheckedCallableDeclaration::Project(declaration)
                    if declaration.owner() == CallableDeclarationOwner::Function =>
                {
                    Some(declaration.clone())
                }
                _ => None,
            });
    let first = declarations.next().unwrap();
    let second = declarations.next().unwrap();
    let wrong = analysis.accepted_declaration_identity(&second).unwrap();
    let owner = executable.items().next().unwrap().id();
    assert!(matches!(
        RuntimeProjectCallable::try_new(
            first,
            wrong,
            owner,
            arcweft_lang_hir::source_index::HirCallableSourceOwner::Item,
            RuntimeCallableId::try_new("fixture").unwrap(),
            None,
        ),
        Err(super::RuntimeProjectCallableError::DeclarationIdentityMismatch)
    ));
}

#[test]
fn project_callable_identity_catalog_covers_bodyless_trait_requirements_and_rejects_missing_declarations()
 {
    let project = project_fixture(
        "bodyless-definition",
        "pub trait Named { fn name(self) -> String }\nfn __runtime_plan_test_probe() -> Unit { () }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let executable = project.analysis_view().unwrap();
    assert!(executable.items().any(|item| {
        matches!(item.item().kind(), HirItemKind::Trait(trait_item)
        if trait_item.members().iter().any(|member| {
            matches!(member, arcweft_lang_hir::item::HirTraitMember::Function(function)
                if function.body().is_none())
        }))
    }));
    let declaration = analysis
        .checked_callables()
        .records()
        .find_map(|facts| match facts.id().declaration() {
            arcweft_lang_sema::callable::CheckedCallableDeclaration::Project(declaration)
                if declaration.owner() == CallableDeclarationOwner::TraitRequirement =>
            {
                Some(declaration)
            }
            _ => None,
        })
        .unwrap();
    assert!(
        analysis
            .accepted_declaration_identity(declaration)
            .unwrap()
            .matches_declaration(declaration)
    );
    let missing = CallableDeclarationKey::Existing(
        CallableDeclarationId::try_new(
            executable.package().clone(),
            CanonicalModulePath::crate_root(),
            CallableDeclarationOwner::Function,
            "missing",
        )
        .unwrap(),
    );
    assert!(matches!(
        analysis.accepted_declaration_identity(&missing),
        Err(
            arcweft_lang_sema::semantic_coordinate::AcceptedSemanticRootCatalogError::MissingRoot { .. }
        )
    ));
}

fn project_fixture(label: &str, source: &str) -> HirProject {
    let package = CallablePackageId::try_new(format!("runtime-plan-semantic-facts-{label}"))
        .expect("fixture package");
    let path = CanonicalModulePath::crate_root();
    let source_name = SourceName::path(format!("runtime-plan-semantic-facts-{label}.arcw"));
    let probe = if source.contains("fn __runtime_plan_test_probe(") {
        ""
    } else {
        "fn __runtime_plan_test_probe() -> Unit { () }\n"
    };
    let source = format!(
        "{source}\n{probe}flow __runtime_plan_test_root {{ __runtime_plan_test_probe() }}\n"
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-test://runtime-plan/{label}"))
                .expect("fixture document ID"),
            source_name.clone(),
            source.as_str(),
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

fn runtime_reachability_with(
    project: &HirProject,
    selected_postfix: impl FnMut(
        arcweft_lang_hir::identity::ExprId,
    ) -> Option<arcweft_lang_hir::identity::ExprId>,
    expression_projection: impl FnMut(
        arcweft_lang_hir::identity::ExprId,
    ) -> Option<HirRuntimeExpressionProjection>,
) -> HirRuntimeSemanticReachability<'_> {
    let executable = project.analysis_view().expect("clean fixture");
    let (_, first_module) = executable.modules().next().expect("fixture module");
    let world = ProjectSymbolWorldId::try_new(
        executable.package().clone(),
        first_module.provenance().source_identity().id().clone(),
        "runtime-plan-semantic-facts-test",
    )
    .expect("fixture reachability world");
    let revision = ProjectSymbolRevision::try_for_documents(
        executable
            .modules()
            .map(|(_, module)| module.provenance().source_identity()),
    )
    .expect("fixture reachability revision");
    let roots = executable
        .items()
        .filter(|item| {
            matches!(
                item.item().kind(),
                arcweft_lang_hir::item::HirItemKind::Flow(_)
            )
        })
        .map(|item| {
            HirRuntimeReachabilityRoot::new(
                HirRuntimeReachabilityRootKind::CheckedFlow,
                HirRuntimeExecutableOwner::Item(item.id()),
            )
        })
        .collect::<Vec<_>>();
    let probe = executable
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Call(_)).then_some(owner)
        })
        .last()
        .expect("fixture probe call");
    let edges = executable
        .items()
        .filter_map(|item| {
            let arcweft_lang_hir::item::HirItemKind::Function(function) = item.item().kind() else {
                return None;
            };
            let name = function.name().resolved()?;
            let declaration = CallableDeclarationKey::Existing(
                CallableDeclarationId::try_new(
                    executable.package().clone(),
                    item.module_path().clone(),
                    CallableDeclarationOwner::Function,
                    name.as_str(),
                )
                .expect("fixture function declaration"),
            );
            Some(HirRuntimeReachabilityEdge::new(
                HirRuntimeReachabilitySite::Expression(probe),
                HirRuntimeExecutableOwner::Item(item.id()),
                HirRuntimeReachabilityEdgeKind::CheckedProjectCall {
                    call: probe,
                    declaration,
                },
            ))
        })
        .collect::<Vec<_>>();
    let externals = ProjectExternalDeclarations::try_new(world.clone(), revision, Vec::new())
        .expect("fixture external declarations");
    let symbols = ProjectSymbolTable::link(project.view(), &externals)
        .expect("fixture symbols")
        .into_table();
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted fixture symbol generation")
        .into_evaluation_topology()
        .expect("fixture evaluation topology");
    let input = HirRuntimeSemanticReachabilityInput::try_new(
        HirRuntimeEmissionMode::CheckAll,
        world,
        revision,
        roots,
        edges,
    )
    .expect("fixture reachability input");
    executable
        .runtime_semantic_reachability(
            input,
            &topology,
            selected_postfix,
            |owner| selected_call_inventory(executable, owner),
            expression_projection,
        )
        .expect("fixture reachability")
}

fn selected_call_inventory(
    executable: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    owner: arcweft_lang_hir::identity::ExprId,
) -> Option<HirSelectedCallExpressionDisposition> {
    executable.modules().find_map(|(_, module)| {
        let expression = module.resolve_expr(owner).ok()?;
        let HirExprKind::Call(call) = expression.kind() else {
            return None;
        };
        Some(HirSelectedCallExpressionDisposition::Callable(
            HirSelectedCallExpressionInventory::new(
                call.arguments()
                    .iter()
                    .map(|argument| argument.value())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                fixture_runtime_callee(executable, call),
            ),
        ))
    })
}

fn fixture_runtime_callee(
    executable: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    call: &arcweft_lang_hir::expr::HirCallInvocation,
) -> Option<arcweft_lang_hir::identity::ExprId> {
    let value = call.callee().value_expression()?;
    executable.modules().find_map(|(_, module)| {
        let expression = module.resolve_expr(value).ok()?;
        (!matches!(expression.kind(), HirExprKind::Path(_))).then_some(value)
    })
}

fn runtime_reachability(project: &HirProject) -> HirRuntimeSemanticReachability<'_> {
    let executable = project.analysis_view().expect("clean fixture");
    let analysis = executable
        .modules()
        .any(|(_, module)| {
            module
                .expressions()
                .any(|(_, expression)| matches!(expression.kind(), HirExprKind::PostfixBracket(_)))
        })
        .then(|| analyze_identity_fixture(project));
    runtime_reachability_with(
        project,
        |owner| {
            let resolution = analysis.as_ref()?.expression(owner)?.resolution();
            let arcweft_lang_sema::final_analysis::CheckedExpressionResolution::PostfixBracket(
                resolution,
            ) = resolution
            else {
                return None;
            };
            Some(resolution.candidate())
        },
        |owner| retained_runtime_projection(executable, owner),
    )
}

fn retained_runtime_projection(
    executable: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
    owner: arcweft_lang_hir::identity::ExprId,
) -> Option<HirRuntimeExpressionProjection> {
    executable.modules().find_map(|(_, module)| {
        let expression = module.resolve_expr(owner).ok()?;
        Some(match expression.kind() {
            HirExprKind::Call(call) => HirRuntimeExpressionProjection::Call {
                result: HirRuntimeValueRetention::Retain,
                callee: if fixture_runtime_callee(executable, call).is_some() {
                    HirRuntimeCallCalleeDisposition::RuntimeReceiver
                } else {
                    HirRuntimeCallCalleeDisposition::Static
                },
            },
            HirExprKind::AttachedContentApplication(_) => {
                HirRuntimeExpressionProjection::Structural {
                    value: HirRuntimeValueRetention::Omit,
                }
            }
            _ => HirRuntimeExpressionProjection::Structural {
                value: HirRuntimeValueRetention::Retain,
            },
        })
    })
}

fn runtime_facts(
    project: &HirProject,
    input: RuntimePlanSemanticFactInput,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticFactsError> {
    let executable = project.analysis_view().expect("clean fixture");
    let reachability = runtime_reachability(project);
    RuntimePlanSemanticFacts::try_new(executable, &reachability, input)
}

fn boolean_literal(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .analysis_view()
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

fn expression_statement_matching(
    project: &HirProject,
    predicate: impl Fn(&HirExprKind) -> bool,
) -> (
    arcweft_lang_hir::identity::StmtId,
    arcweft_lang_hir::identity::ExprId,
) {
    project
        .analysis_view()
        .expect("clean fixture")
        .modules()
        .flat_map(|(_, module)| {
            module.statements().filter_map(|(statement, body)| {
                let HirStmtKind::Expression { expression } = body.kind() else {
                    return None;
                };
                let expression_kind = module.resolve_expr(*expression).ok()?.kind();
                predicate(expression_kind).then_some((statement, *expression))
            })
        })
        .next()
        .expect("fixture expression statement")
}

fn flow_item(project: &HirProject) -> arcweft_lang_hir::identity::ItemId {
    project
        .analysis_view()
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

fn entity_reference(project: &HirProject) -> arcweft_lang_hir::identity::ExprId {
    project
        .analysis_view()
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

fn option_unit_type() -> super::RuntimeNormalizedType {
    let item = unit_type();
    normalized_type(
        0x12,
        RuntimeTypeShape::Option {
            item: Box::new(item.clone()),
            some_payload: Box::new(tuple_payload(0x13, item)),
        },
    )
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

fn tuple_payload(marker: u8, field: super::RuntimeNormalizedType) -> super::RuntimeNormalizedType {
    normalized_type(marker, RuntimeTypeShape::Tuple(Box::new([field])))
}

fn option_cases(payload: super::RuntimeNormalizedType) -> Box<[RuntimeNormalizedVariantCase]> {
    Box::new([
        RuntimeNormalizedVariantCase::new("Some", Some(payload)),
        RuntimeNormalizedVariantCase::new("None", None),
    ])
}

fn result_cases(
    value_payload: super::RuntimeNormalizedType,
    error_payload: super::RuntimeNormalizedType,
) -> Box<[RuntimeNormalizedVariantCase]> {
    Box::new([
        RuntimeNormalizedVariantCase::new("Ok", Some(value_payload)),
        RuntimeNormalizedVariantCase::new("Err", Some(error_payload)),
    ])
}

pub(crate) fn fixture_local_origin(
    project: &HirProject,
    local: arcweft_lang_hir::identity::LocalId,
) -> arcweft_lang_sema::semantic_coordinate::CheckedLocalBindingOrigin {
    analyze_identity_fixture(project)
        .local_binding_origin(local)
        .unwrap()
}

pub(crate) fn fixture_expression_origin(
    project: &HirProject,
    expression: arcweft_lang_hir::identity::ExprId,
) -> arcweft_lang_sema::semantic_coordinate::CheckedExpressionOrigin {
    analyze_identity_fixture(project)
        .expression_origin(expression)
        .unwrap()
}

fn complete_type_input(project: &HirProject) -> RuntimePlanSemanticFactInput {
    let mut input = RuntimePlanSemanticFactInput::new();
    let runtime_owners = runtime_reachability(project);
    let analysis = analyze_identity_fixture(project);
    for owner in runtime_owners.locals() {
        input.push_local_declaration(
            owner,
            unit_type(),
            analysis.local_binding_origin(owner).unwrap(),
        );
    }
    for owner in runtime_owners.patterns() {
        input.push_pattern_type(owner, unit_type());
    }
    for owner in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(
            owner,
            unit_type(),
            analysis.expression_origin(owner).unwrap(),
        );
    }
    let selected_children = super::RuntimeSemanticOwnerSet {
        runtime: &runtime_owners,
        programs: None,
        program_free_locals: None,
    }
    .selected_expression_children()
    .unwrap();
    for (owner, expression) in analysis.expressions() {
        if selected_children.contains_key(&owner)
            && !input
                .expression_facts
                .iter()
                .any(|(published, _)| *published == owner)
        {
            input.push_expression_origin(owner, analysis.expression_origin(owner).unwrap());
        }
        if runtime_owners.contains_expression(owner) {
            if let arcweft_lang_sema::final_analysis::CheckedExpressionResolution::PostfixBracket(
                resolution,
            ) = expression.resolution()
            {
                input.push_postfix_candidate(owner, resolution.candidate());
            }
        }
    }
    input
}

#[test]
fn local_origins_reject_another_binding_and_hir_allocation_before_publication() {
    let source = "fn root(first: bool, second: bool) -> bool { first }\n";
    let project = project_fixture("local-origin-admission", source);
    let mut wrong_local = complete_type_input(&project);
    let target = wrong_local.local_declarations[0].0;
    wrong_local.local_declarations[0].1.origin = wrong_local.local_declarations[1].1.origin.clone();
    assert_eq!(
        runtime_facts(&project, wrong_local).unwrap_err(),
        RuntimeSemanticFactsError::InvalidLocalOrigin { local: target }
    );

    let foreign = project_fixture("local-origin-admission", source);
    let mut wrong_generation = complete_type_input(&project);
    let foreign_input = complete_type_input(&foreign);
    wrong_generation.local_declarations[0].1.origin =
        foreign_input.local_declarations[0].1.origin.clone();
    assert_eq!(
        runtime_facts(&project, wrong_generation).unwrap_err(),
        RuntimeSemanticFactsError::InvalidLocalOrigin { local: target }
    );
}

#[test]
fn expression_origins_preserve_coordinates_and_reject_wrong_owner_or_generation() {
    let source = "fn root(first: bool) -> bool { first || false }\n";
    let project = project_fixture("expression-origin-admission", source);
    let input = complete_type_input(&project);
    let target = input.expression_facts[0].0;
    let expected = fixture_expression_origin(&project, target);
    let facts = runtime_facts(&project, input).unwrap();
    assert_eq!(
        facts.expression_coordinate(target),
        Some(expected.coordinate())
    );
    let mut wrong_owner = complete_type_input(&project);
    wrong_owner.expression_facts[0].1.origin = wrong_owner.expression_facts[1].1.origin.clone();
    assert_eq!(
        runtime_facts(&project, wrong_owner).unwrap_err(),
        RuntimeSemanticFactsError::InvalidExpressionOrigin { expression: target }
    );
    let foreign = project_fixture("expression-origin-admission", source);
    assert!(!expected.validate_owner(foreign.analysis_view().unwrap(), target));
    let mut wrong_generation = complete_type_input(&project);
    wrong_generation.expression_facts[0].1.origin = complete_type_input(&foreign).expression_facts
        [0]
    .1
    .origin
    .clone();
    assert_eq!(
        runtime_facts(&project, wrong_generation).unwrap_err(),
        RuntimeSemanticFactsError::InvalidExpressionOrigin { expression: target }
    );
}

#[test]
fn type_free_expression_origin_is_required_and_retained_before_publication() {
    let source = "fn root() -> bool { true }\n";
    let project = project_fixture("type-free-expression-origin", source);
    let target = boolean_literal(&project);
    let executable = project.analysis_view().unwrap();
    let reachability = runtime_reachability_with(
        &project,
        |_| None,
        |owner| {
            if owner == target {
                Some(HirRuntimeExpressionProjection::Structural {
                    value: HirRuntimeValueRetention::Omit,
                })
            } else {
                retained_runtime_projection(executable, owner)
            }
        },
    );
    assert!(
        !reachability
            .selected_expression_type_owners()
            .unwrap()
            .contains(&target)
    );
    let origin = fixture_expression_origin(&project, target);
    let complete = || {
        let mut input = complete_type_input(&project);
        input
            .expression_facts
            .iter_mut()
            .find(|(owner, _)| *owner == target)
            .unwrap()
            .1
            .ty = None;
        input
    };
    let facts = RuntimePlanSemanticFacts::try_new(executable, &reachability, complete()).unwrap();
    assert_eq!(
        facts.expression_coordinate(target),
        Some(origin.coordinate())
    );
    assert!(facts.expression_type(target).is_none());

    let mut missing = complete();
    missing
        .expression_facts
        .retain(|(owner, _)| *owner != target);
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, &reachability, missing).unwrap_err(),
        RuntimeSemanticFactsError::MissingExpressionOrigin { expression: target }
    );

    let foreign = project_fixture("type-free-expression-origin", source);
    let foreign_origin = fixture_expression_origin(&foreign, boolean_literal(&foreign));
    let mut wrong_generation = complete();
    wrong_generation
        .expression_facts
        .iter_mut()
        .find(|(owner, _)| *owner == target)
        .unwrap()
        .1
        .origin = foreign_origin;
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, &reachability, wrong_generation).unwrap_err(),
        RuntimeSemanticFactsError::InvalidExpressionOrigin { expression: target }
    );
}

fn assignment_fact_fixture(
    label: &str,
) -> (
    HirProject,
    RuntimePlanSemanticFactInput,
    arcweft_lang_hir::identity::StmtId,
    arcweft_lang_hir::identity::StmtId,
    RuntimeAssignmentFact,
) {
    let project = project_fixture(
        label,
        concat!(
            "struct Point { x: i64, active: bool }\n",
            "fn update(point: Point) -> bool {\n",
            "    point.active = true\n",
            "    return point.active\n",
            "}\n",
            "fn __runtime_plan_test_probe() -> Unit { () }\n",
        ),
    );
    let executable = project.analysis_view().expect("assignment fixture");
    let (_, module) = executable.modules().next().expect("root assignment module");
    let (statement, target, value) = module
        .statements()
        .find_map(|(owner, statement)| match statement.kind() {
            HirStmtKind::Assign { target, value } => Some((owner, *target, *value)),
            _ => None,
        })
        .expect("assignment fixture statement");
    let extra_statement = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Return { .. }).then_some(owner)
        })
        .expect("assignment fixture return statement");
    let HirExprKind::Select(select) = module
        .resolve_expr(target)
        .expect("assignment target expression")
        .kind()
    else {
        panic!("assignment target is a select")
    };
    let base = select.target();
    let HirSelectedMember::Name(_) = select.member() else {
        panic!("assignment field is resolved")
    };
    let runtime_owners = runtime_reachability(&project);
    let local = runtime_owners
        .locals()
        .next()
        .expect("assignment base local");

    let resolved = assignment_nominal(
        &project,
        module,
        label,
        &[
            ("x", RuntimeTypeSchema::I64),
            ("active", RuntimeTypeSchema::Bool),
        ],
    );
    let identity = resolved.identity();
    let record_type = super::RuntimeNormalizedType::new(
        identity,
        RuntimeTypeShape::Nominal {
            nominal: resolved.clone(),
            arguments: Box::new([]),
        },
    );
    let field_type = normalized_type(0x31, RuntimeTypeShape::Bool);
    let runtime_field = RuntimeRecordFieldId::try_from_zero_based_ordinal(1)
        .expect("assignment fixture field coordinate");
    let fact = RuntimeAssignmentFact::new(
        RuntimeResolvedPlace::Fields {
            base: local,
            fields: vec![runtime_field].into_boxed_slice(),
        },
        field_type.clone(),
    );
    let mut input = complete_type_input(&project);
    input
        .local_declarations
        .iter_mut()
        .find(|(owner, _)| *owner == local)
        .expect("assignment local type")
        .1
        .ty = record_type.clone();
    for (owner, ty) in &mut input.expression_facts {
        if *owner == base {
            ty.ty = Some(record_type.clone());
        } else if *owner == target || *owner == value {
            ty.ty = Some(field_type.clone());
        }
    }
    input.push_value(base, RuntimeResolvedValue::Local(local));
    input.push_select(
        target,
        RuntimeResolvedSelect::Field {
            owner: identity,
            field: runtime_field,
        },
    );
    let document = Arc::clone(module.provenance().document());
    let world = ProjectSymbolWorldId::try_new(
        project.package().clone(),
        document.identity().id().clone(),
        "runtime-plan-semantic-facts-test",
    )
    .unwrap();
    let registrations = arcweft_lang_sema::registration::ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let registered = arcweft_lang_sema::registration::CharacterRegistrar::register(
        arcweft_lang_sema::registration::CharacterRegistrationRequest::new(
            Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            project.view(),
            &registrations,
            None,
        ),
    )
    .unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let analysis = arcweft_lang_sema::final_analysis::analyze_final_project(
        project.analysis_view().unwrap(),
        registered.symbols(),
        arcweft_lang_sema::final_analysis::FinalSemanticCatalogs::production(&registered),
        arcweft_lang_sema::final_analysis::FinalSemanticAnalysisControl::new(&cancelled),
    )
    .unwrap();
    input.attach_checked_local_uses(Arc::clone(analysis.checked_local_uses()));
    (project, input, statement, extra_statement, fact)
}

fn trigger_fact_fixture(
    label: &str,
) -> (
    HirProject,
    RuntimePlanSemanticFactInput,
    arcweft_lang_hir::identity::StmtId,
    arcweft_lang_hir::identity::StmtId,
) {
    let project = project_fixture(
        label,
        "flow trigger_owner {\n    on true => defer ()\n    return ()\n}\n",
    );
    let executable = project.analysis_view().expect("trigger fixture");
    let (_, module) = executable.modules().next().expect("root trigger module");
    let trigger = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::On { .. }).then_some(owner)
        })
        .expect("trigger fixture On statement");
    let non_trigger = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Return { .. }).then_some(owner)
        })
        .expect("trigger fixture non-On statement");
    let input = complete_type_input(&project);
    (project, input, trigger, non_trigger)
}

fn assignment_nominal(
    project: &HirProject,
    module: &arcweft_lang_hir::module::HirModule,
    label: &str,
    fields: &[(&str, RuntimeTypeSchema)],
) -> RuntimeResolvedNominal {
    let document = Arc::clone(module.provenance().document());
    let world = ProjectSymbolWorldId::try_new(
        project.package().clone(),
        document.identity().id().clone(),
        format!("{label}-assignment-facts"),
    )
    .expect("assignment symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([document.identity()])
        .expect("assignment symbol revision");
    let externals = ProjectExternalDeclarations::try_new(world, revision, Vec::new())
        .expect("empty assignment externals");
    let symbols = ProjectSymbolTable::link(project.view(), &externals)
        .expect("assignment symbols link")
        .into_table();
    let nominal = symbols
        .nominal_symbols()
        .find(|nominal| nominal.id().name().as_str() == "Point")
        .expect("Point nominal");
    let identity = RuntimeSemanticTypeId::from_bytes([0x91; 32]);
    let runtime_nominal = RuntimeNominalTypeId::try_new("test::assignment::Point").unwrap();
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            arcweft_core::entry::RuntimeNominalDeclarationId::from_bytes(
                *(RuntimeNominalSchemaIdentity::new(runtime_nominal.clone(), identity))
                    .semantic_identity()
                    .as_bytes(),
            ),
            RuntimeNominalSchemaIdentity::new(runtime_nominal.clone(), identity),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(ordinal, (name, schema))| {
                        RuntimeNominalSchemaField::new(
                            RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                            Some((*name).to_owned()),
                            schema.clone(),
                        )
                    })
                    .collect(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    RuntimeResolvedNominal::project(
        nominal.id().clone(),
        nominal.owner(),
        runtime_nominal,
        identity,
        graph.try_layout_hash(identity).unwrap(),
        Arc::new(graph),
    )
}

#[test]
fn nominal_record_fact_requires_the_project_record_source_shape() {
    use arcweft_core::entry::RuntimeNominalRecordShape;
    use arcweft_core::value::RuntimeNominalRecordLayout;

    let project = project_fixture("nominal-record-shape", "struct Point {}\n");
    let executable = project.analysis_view().expect("project view");
    let (_, module) = executable.modules().next().expect("root module");
    let nominal = assignment_nominal(&project, module, "nominal-record-shape", &[]);
    for shape in [
        RuntimeNominalRecordShape::Record,
        RuntimeNominalRecordShape::Unit,
        RuntimeNominalRecordShape::Tuple,
    ] {
        let layout = RuntimeNominalRecordLayout::try_from_checked_projection(
            nominal.runtime_nominal_id(),
            nominal.identity(),
            nominal.layout(),
            shape,
            Vec::new(),
            Vec::new(),
        )
        .expect("valid empty layout");
        let result = super::RuntimeResolvedNominalRecord::try_new(
            nominal.clone(),
            Arc::new(layout),
            Vec::new(),
        );
        if shape == RuntimeNominalRecordShape::Record {
            assert!(result.is_ok());
        } else {
            assert!(
                matches!(result, Err(super::RuntimeNominalRecordFactError::SourceShape { actual }) if actual == shape)
            );
        }
    }
}

fn local_owners(project: &HirProject) -> Vec<arcweft_lang_hir::identity::LocalId> {
    runtime_reachability(project).locals().collect()
}

#[test]
fn local_declarations_use_one_complete_contiguous_canonical_projection() {
    let project = project_fixture(
        "local-declaration-order",
        "fn root(first: bool, second: bool) -> bool { let third = first; second }\n",
    );
    let owners = local_owners(&project);
    assert!(
        owners.len() >= 3,
        "fixture retains parameters and let binding"
    );

    let facts = runtime_facts(&project, complete_type_input(&project))
        .expect("complete canonical local projection");
    let analysis = analyze_identity_fixture(&project);
    for (owner, _) in facts.local_declarations() {
        let actual = facts.local_origin(owner).unwrap();
        let expected = analysis.local_binding_origin(owner).unwrap();
        assert_eq!(actual.coordinate(), expected.coordinate());
        assert_eq!(actual.declaration(), expected.declaration());
    }

    let locals = facts.local_declarations().collect::<Vec<_>>();
    assert_eq!(locals.len(), owners.len());
    for (owner, (actual, ty)) in owners.into_iter().zip(locals) {
        assert_eq!(
            actual, owner,
            "the canonical final-HIR local remains the sole semantic-fact key"
        );
        assert_eq!(ty, &unit_type());
        assert_eq!(facts.local_type(owner), Some(&unit_type()));
    }
}

#[test]
fn assignment_facts_are_complete_unique_and_bound_to_assignment_statements() {
    let (project, mut input, statement, _, fact) =
        assignment_fact_fixture("assignment-fact-accepted");
    input.push_assignment(statement, fact.clone());
    let facts = runtime_facts(&project, input).expect("complete assignment fact");
    let accepted = facts
        .assignment(statement)
        .expect("assignment accessor returns the sole fact");
    assert_eq!(accepted, &fact);
    assert!(
        matches!(accepted.place(), RuntimeResolvedPlace::Fields { fields, .. } if fields.len() == 1 && fields[0].zero_based() == 1)
    );
    let RuntimeTypeShape::Nominal { nominal, .. } = facts
        .local_type(accepted.place().local())
        .expect("assignment local")
        .shape()
    else {
        panic!("nominal fixture")
    };
    let proof = facts
        .runtime_plan_nominal_schema()
        .expect("source proof reaches plan admission");
    assert_eq!(
        proof.try_layout_hash(nominal.identity()).unwrap(),
        nominal.layout()
    );
    assert_eq!(proof.definitions().len(), 1);

    let (project, input, statement, _, _) = assignment_fact_fixture("assignment-fact-missing");
    assert_eq!(
        runtime_facts(&project, input).expect_err("every live assignment requires one fact"),
        RuntimeSemanticFactsError::MissingAssignmentFact { statement }
    );

    let (project, mut input, statement, _, fact) =
        assignment_fact_fixture("assignment-fact-duplicate");
    input.push_assignment(statement, fact.clone());
    input.push_assignment(statement, fact);
    assert_eq!(
        runtime_facts(&project, input).expect_err("one assignment cannot own duplicate facts"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::Assignment,
        }
    );

    let (project, mut input, statement, extra_statement, fact) =
        assignment_fact_fixture("assignment-fact-extra");
    input.push_assignment(statement, fact.clone());
    input.push_assignment(extra_statement, fact);
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a non-assignment statement cannot own an assignment fact"),
        RuntimeSemanticFactsError::WrongStatementFamily {
            statement: extra_statement,
            expected: RuntimeSemanticFactFamily::Assignment,
        }
    );
}

#[test]
fn trigger_admissions_are_complete_unique_generation_bound_and_opaque() {
    let (project, input, trigger, _) = trigger_fact_fixture("trigger-fact-missing");
    assert_eq!(
        runtime_facts(&project, input).expect_err("every reachable On requires one admission"),
        RuntimeSemanticFactsError::MissingTriggerFact { statement: trigger }
    );

    let (project, mut input, trigger, _) = trigger_fact_fixture("trigger-fact-accepted");
    input
        .push_expression_trigger(trigger)
        .expect("the sole checked trigger row stages");
    let facts = runtime_facts(&project, input).expect("complete trigger projection");
    assert!(facts.trigger(trigger).is_some_and(|admission| matches!(
        &admission.kind,
        RuntimeTriggerAdmissionKind::Expression
    )));

    let (_project, mut input, trigger, _) = trigger_fact_fixture("trigger-fact-duplicate");
    input
        .push_expression_trigger(trigger)
        .expect("first trigger row stages");
    assert_eq!(
        input
            .push_input_trigger(trigger)
            .expect_err("a second variant cannot replace the accepted row"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::Trigger,
        }
    );

    let (project, mut input, trigger, non_trigger) = trigger_fact_fixture("trigger-fact-extra");
    input
        .push_expression_trigger(trigger)
        .expect("required trigger row stages");
    input
        .push_input_trigger(non_trigger)
        .expect("the staging boundary defers owner-set validation to the atomic seal");
    assert_eq!(
        runtime_facts(&project, input).expect_err("a non-On extra is rejected"),
        RuntimeSemanticFactsError::WrongStatementFamily {
            statement: non_trigger,
            expected: RuntimeSemanticFactFamily::Trigger,
        }
    );

    let (project, mut input, trigger, _) = trigger_fact_fixture("trigger-fact-foreign-owner");
    input
        .push_expression_trigger(trigger)
        .expect("required trigger row stages");
    let (foreign, _, foreign_trigger, _) = trigger_fact_fixture("trigger-fact-foreign-generation");
    input
        .push_input_trigger(foreign_trigger)
        .expect("the typed input accepts a qualified owner before generation sealing");
    assert!(matches!(
        runtime_facts(&project, input),
        Err(RuntimeSemanticFactsError::UnknownModule { module })
            if module == foreign_trigger.module()
    ));
    drop(foreign);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one mixed product fixture proves the runtime-domain gate and contiguous local projection across every representative HIR owner family"
)]
fn presentation_owned_facts_are_inactive_and_filtered_local_ids_remain_contiguous() {
    let project = project_fixture(
        "presentation-owner-domain",
        concat!(
            "fn before(first: bool) -> bool { let second: bool = first; second }\n",
            "#[tool.flag(1)]\n",
            "view Card(dialogue: DialogueView, count: i64 = 1i64) { Text(\"x\") }\n",
            "#[tool.flag(2)]\n",
            "style Theme {\n",
            "    token spacing.gap: i64 = 1i64\n",
            "    Button { gap = 2i64 }\n",
            "    when environment(text-scale >= 100%) { Button { gap = 3i64 } }\n",
            "}\n",
            "fn after(third: bool) -> bool { third }\n",
        ),
    );
    let executable = project.analysis_view().expect("executable fixture");
    let runtime_owners = runtime_reachability(&project);
    let module = executable.modules().next().expect("one fixture module").1;
    let all_locals = module.locals().map(|(owner, _)| owner).collect::<Vec<_>>();
    let retained_locals = runtime_owners.locals().collect::<Vec<_>>();
    let presentation_local = all_locals
        .iter()
        .copied()
        .find(|owner| !runtime_owners.contains_local(*owner))
        .expect("View parameter local");
    let removed_position = all_locals
        .iter()
        .position(|owner| *owner == presentation_local)
        .expect("presentation local position");
    assert!(removed_position > 0 && removed_position + 1 < all_locals.len());

    let facts = runtime_facts(&project, complete_type_input(&project))
        .expect("filtered runtime-domain fact set");
    assert_eq!(facts.local_type(presentation_local), None);
    let locals = facts.local_declarations().collect::<Vec<_>>();
    assert_eq!(locals.len(), retained_locals.len());
    for (owner, (actual, ty)) in retained_locals.iter().copied().zip(locals) {
        assert_eq!(actual, owner);
        assert_eq!(ty, &unit_type());
    }

    let presentation_pattern = module
        .patterns()
        .map(|(owner, _)| owner)
        .find(|owner| !runtime_owners.contains_pattern(*owner))
        .expect("View parameter pattern");
    let presentation_type = module
        .types()
        .map(|(owner, _)| owner)
        .find(|owner| !runtime_owners.contains_type(*owner))
        .expect("View or Style type");
    let presentation_literal = module
        .expressions()
        .find_map(|(owner, expression)| {
            (!runtime_owners.contains_expression(owner)
                && matches!(expression.kind(), HirExprKind::Literal(_)))
            .then_some(owner)
        })
        .expect("presentation literal");
    let retained_path = module
        .expressions()
        .find_map(|(owner, expression)| {
            (runtime_owners.contains_expression(owner)
                && matches!(expression.kind(), HirExprKind::Path(_)))
            .then_some(owner)
        })
        .expect("retained local path");

    let mut input = complete_type_input(&project);
    input.push_local_declaration(
        presentation_local,
        unit_type(),
        fixture_local_origin(&project, presentation_local),
    );
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a presentation local cannot extend the runtime domain"),
        RuntimeSemanticFactsError::ExtraLocalDeclaration {
            local: presentation_local,
        }
    );

    let mut input = complete_type_input(&project);
    input.push_expression_type(
        presentation_literal,
        unit_type(),
        fixture_expression_origin(&project, presentation_literal),
    );
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a presentation expression cannot publish a runtime type"),
        RuntimeSemanticFactsError::InactiveExpressionFact {
            expression: presentation_literal,
            family: RuntimeSemanticFactFamily::ExpressionType,
        }
    );

    let mut input = complete_type_input(&project);
    input.push_pattern_literal(presentation_pattern, RuntimeValue::Unit);
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a presentation pattern cannot publish an operational fact"),
        RuntimeSemanticFactsError::InactivePatternFact {
            pattern: presentation_pattern,
            family: RuntimeSemanticFactFamily::PatternLiteral,
        }
    );

    let mut input = complete_type_input(&project);
    input.push_expression_literal(presentation_literal, RuntimeValue::Unit);
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a presentation expression cannot publish a literal fact"),
        RuntimeSemanticFactsError::InactiveExpressionFact {
            expression: presentation_literal,
            family: RuntimeSemanticFactFamily::ExpressionLiteral,
        }
    );

    let mut input = complete_type_input(&project);
    input.push_type(presentation_type, unit_type());
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a presentation type cannot publish a runtime type fact"),
        RuntimeSemanticFactsError::InactiveTypeFact {
            ty: presentation_type,
        }
    );

    let mut input = complete_type_input(&project);
    input.push_value(
        retained_path,
        RuntimeResolvedValue::Local(presentation_local),
    );
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a retained value cannot reference a presentation local"),
        RuntimeSemanticFactsError::InactiveLocalReference {
            local: presentation_local,
        }
    );
}

#[test]
fn missing_extra_duplicate_and_reordered_local_projections_are_rejected() {
    let project = project_fixture(
        "invalid-local-declarations",
        "fn root(first: bool, second: bool) -> bool { first }\n",
    );
    let owners = local_owners(&project);
    assert!(owners.len() >= 2, "fixture retains both parameters");
    let mut missing = complete_type_input(&project);
    let missing_owner = missing
        .local_declarations
        .pop()
        .expect("fixture local declaration")
        .0;
    assert_eq!(
        runtime_facts(&project, missing).expect_err("a local cannot be omitted"),
        RuntimeSemanticFactsError::MissingLocalDeclaration {
            local: missing_owner,
        }
    );

    let foreign = project_fixture("extra-local-declaration", "fn foreign(value: bool) {}\n");
    let foreign_owner = local_owners(&foreign)[0];
    let mut extra = complete_type_input(&project);
    extra.push_local_declaration(
        foreign_owner,
        unit_type(),
        fixture_local_origin(&foreign, foreign_owner),
    );
    assert_eq!(
        runtime_facts(&project, extra).expect_err("a foreign local cannot extend the plan domain"),
        RuntimeSemanticFactsError::ExtraLocalDeclaration {
            local: foreign_owner,
        }
    );

    let mut duplicate = complete_type_input(&project);
    duplicate.push_local_declaration(
        owners[0],
        unit_type(),
        fixture_local_origin(&project, owners[0]),
    );
    assert_eq!(
        runtime_facts(&project, duplicate)
            .expect_err("one HIR local cannot receive two plan identities"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::LocalDeclaration,
        }
    );

    let mut reordered = complete_type_input(&project);
    reordered.local_declarations.swap(0, 1);
    assert_eq!(
        runtime_facts(&project, reordered)
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
    let facts =
        runtime_facts(&first, complete_type_input(&first)).expect("complete checked fact set");

    assert_eq!(
        facts.validate_generation(first.analysis_view().expect("same generation")),
        Ok(())
    );
    assert_eq!(
        facts.validate_generation(second.analysis_view().expect("foreign generation")),
        Err(RuntimeSemanticFactsError::WrongProjectGeneration)
    );
}

#[test]
fn checked_literal_fact_uses_the_qualified_expression_owner() {
    let project = project_fixture("literal-owner", "fn root() {\n    let value = true;\n}\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_literal(owner, RuntimeValue::Bool(true));

    let facts = runtime_facts(&project, input).expect("literal fact");
    assert_eq!(
        facts.expression_literal(owner),
        Some(&RuntimeValue::Bool(true))
    );
}

#[test]
fn checked_flow_identity_uses_the_qualified_item_owner() {
    let project = project_fixture(
        "flow-owner",
        "flow opening {}\nfn __runtime_plan_test_probe() -> Unit { () }\n",
    );
    let owner = flow_item(&project);
    let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
    let mut input = complete_type_input(&project);
    input.push_flow(
        owner,
        accepted_flow_fixture(&project, owner, identity.clone()),
    );

    let facts = runtime_facts(&project, input).expect("Flow identity fact");
    assert_eq!(
        facts.flow(owner).map(RuntimeFlowFact::identity),
        Some(&identity)
    );
}

#[test]
fn flow_definition_rejects_another_owner_and_generation() {
    let source = "flow first {}\nflow second {}\nfn __runtime_plan_test_probe() -> Unit { () }\n";
    let project = project_fixture("flow-definition-owner", source);
    let executable = project.analysis_view().unwrap();
    let owners = executable
        .items()
        .filter_map(|item| matches!(item.item().kind(), HirItemKind::Flow(_)).then_some(item.id()))
        .take(2)
        .collect::<Vec<_>>();
    let first = accepted_flow_fixture(
        &project,
        owners[0],
        FlowRuntimeId::canonical("first").unwrap(),
    );
    let mut foreign_owner = complete_type_input(&project);
    foreign_owner.push_flow(owners[1], first);
    assert_eq!(
        runtime_facts(&project, foreign_owner).unwrap_err(),
        RuntimeSemanticFactsError::InvalidFlowDefinition { item: owners[1] }
    );
    let other = project_fixture("flow-definition-generation", source);
    let other_owner = other
        .analysis_view()
        .unwrap()
        .items()
        .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
        .unwrap()
        .id();
    let foreign = accepted_flow_fixture(
        &other,
        other_owner,
        FlowRuntimeId::canonical("first").unwrap(),
    );
    let mut foreign_generation = complete_type_input(&project);
    foreign_generation.push_flow(owners[0], foreign);
    assert_eq!(
        runtime_facts(&project, foreign_generation).unwrap_err(),
        RuntimeSemanticFactsError::InvalidFlowDefinition { item: owners[0] }
    );
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
        runtime_facts(&project, input).expect_err("literal cannot masquerade as a resolved path"),
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
        "pub character alice {}\nfn opening() { alice(id=@say.story.greeting)[hello]; }\nfn root() -> Ref<DialogueLine> { @say.story.greeting }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let owner = analysis
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(
                expression.resolution(),
                arcweft_lang_sema::final_analysis::CheckedExpressionResolution::DialogueLineReference(_)
            )
            .then_some(owner)
        })
        .expect("reference to the declared Line");
    let line = RuntimeLineId::from_source_entity_body("say.story.greeting")
        .expect("checked dialogue line conversion");
    let mut input = complete_type_input(&project);
    input.push_value(owner, RuntimeResolvedValue::DialogueLine(line.clone()));

    let facts = runtime_facts(&project, input).expect("typed dialogue-line runtime fact");
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
        runtime_facts(&project, input).expect_err("duplicate fact must fail atomically"),
        RuntimeSemanticFactsError::DuplicateFact {
            family: RuntimeSemanticFactFamily::ExpressionLiteral,
        }
    );
}

#[test]
fn accepted_expression_and_pattern_types_are_complete_and_exact() {
    let project = project_fixture(
        "complete-types",
        "fn root(value: bool) -> Unit {\n    match value { true => (), false => () }\n}\n",
    );
    let input = complete_type_input(&project);
    let facts = runtime_facts(&project, input).expect("complete type facts");

    let runtime_owners = runtime_reachability(&project);
    for owner in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        assert_eq!(facts.expression_type(owner), Some(&unit_type()));
    }
    for owner in runtime_owners.patterns() {
        assert_eq!(facts.pattern_type(owner), Some(&unit_type()));
    }
}

#[test]
fn runtime_type_completeness_excludes_effect_metadata_owners() {
    let project = project_fixture(
        "effect-metadata-types",
        "fn root() -> bool effects { fs.read } { true }\n",
    );
    let executable = project.analysis_view().expect("executable fixture");
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
    let facts = runtime_facts(&project, complete_type_input(&project))
        .expect("effect metadata requires no runtime expression type");
    assert!(facts.expression_type(effect).is_none());
    assert_eq!(facts.expression_type(body), Some(&unit_type()));

    let mut input = complete_type_input(&project);
    input.push_expression_type(
        effect,
        unit_type(),
        fixture_expression_origin(&project, effect),
    );
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("effect metadata cannot publish a runtime expression type"),
        RuntimeSemanticFactsError::InactiveExpressionFact {
            expression: effect,
            family: RuntimeSemanticFactFamily::ExpressionType,
        },
    );
}

#[test]
fn evaluated_effect_rejects_an_application_that_is_not_a_call() {
    let project = project_fixture(
        "evaluated-effect-non-call-application",
        "flow opening {\n    true\n}\n",
    );
    let (_, application) =
        expression_statement_matching(&project, |kind| matches!(kind, HirExprKind::Literal(_)));
    let operand = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let mut input = complete_type_input(&project);
    input.push_evaluated_effect(
        application,
        RuntimeEvaluatedEffectFact::new(
            application,
            application,
            unit_type(),
            RuntimeEvaluatedEffect::Log {
                level: super::RuntimeLogLevel::Info,
                message: operand,
                fields: Box::new([]),
            },
        ),
    );

    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("an evaluated effect application must be a HIR Call"),
        RuntimeSemanticFactsError::WrongExpressionFamily {
            expression: application,
            expected: RuntimeSemanticFactFamily::EvaluatedEffect,
        }
    );
}

#[test]
fn evaluated_effect_rejects_a_site_root_different_from_its_expression_owner() {
    let project = project_fixture(
        "evaluated-effect-owner-mismatch",
        "flow opening {\n    true\n    __runtime_plan_test_probe()\n}\n",
    );
    let (_, literal) =
        expression_statement_matching(&project, |kind| matches!(kind, HirExprKind::Literal(_)));
    let (_, application) =
        expression_statement_matching(&project, |kind| matches!(kind, HirExprKind::Call(_)));
    let operand = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let mut input = complete_type_input(&project);
    input.push_evaluated_effect(
        application,
        RuntimeEvaluatedEffectFact::new(
            literal,
            application,
            unit_type(),
            RuntimeEvaluatedEffect::Log {
                level: super::RuntimeLogLevel::Info,
                message: operand,
                fields: Box::new([]),
            },
        ),
    );

    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("an evaluated effect must remain owned by its site root"),
        RuntimeSemanticFactsError::InvalidEvaluatedEffectFact {
            expression: application,
        }
    );
}

#[test]
fn evaluated_effect_rejects_an_ensure_condition_without_bool_type() {
    let project = project_fixture(
        "evaluated-effect-ensure-non-bool",
        "flow opening {\n    __runtime_plan_test_probe()\n}\n",
    );
    let (_, application) =
        expression_statement_matching(&project, |kind| matches!(kind, HirExprKind::Call(_)));
    let condition = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let message = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let mut input = complete_type_input(&project);
    input.push_evaluated_effect(
        application,
        RuntimeEvaluatedEffectFact::new(
            application,
            application,
            unit_type(),
            RuntimeEvaluatedEffect::Ensure { condition, message },
        ),
    );

    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("Ensure condition must carry the checked Bool type"),
        RuntimeSemanticFactsError::InvalidEvaluatedEffectFact {
            expression: application,
        }
    );
}

#[test]
fn evaluated_effect_rejects_a_stop_fade_without_duration_type() {
    let project = project_fixture(
        "evaluated-effect-stop-non-duration",
        "flow opening {\n    __runtime_plan_test_probe()\n}\n",
    );
    let (_, application) =
        expression_statement_matching(&project, |kind| matches!(kind, HirExprKind::Call(_)));
    let target = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let fade = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let mut input = complete_type_input(&project);
    input.push_evaluated_effect(
        application,
        RuntimeEvaluatedEffectFact::new(
            application,
            application,
            unit_type(),
            RuntimeEvaluatedEffect::Drop {
                target,
                policy: RuntimeDropPolicyFact::Stop {
                    fade: RuntimeDropFadeFact::Operand(fade),
                },
            },
        ),
    );

    assert_eq!(
        runtime_facts(&project, input).expect_err("Stop fade must carry the checked Duration type"),
        RuntimeSemanticFactsError::InvalidEvaluatedEffectFact {
            expression: application,
        }
    );
}

#[test]
fn evaluated_effect_rejects_a_stop_fade_with_an_invalid_source() {
    let project = project_fixture(
        "evaluated-effect-stop-invalid-source",
        "flow opening {\n    __runtime_plan_test_probe()\n}\n",
    );
    let (_, application) =
        expression_statement_matching(&project, |kind| matches!(kind, HirExprKind::Call(_)));
    let target = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::Expression(application),
        unit_type(),
    );
    let fade = RuntimeEvaluatedEffectOperandFact::new(
        RuntimeResolvedCallOperandSource::CompactNumericElement {
            sequence: application,
            ordinal: 0,
        },
        unit_type(),
    );
    let mut input = complete_type_input(&project);
    input.push_evaluated_effect(
        application,
        RuntimeEvaluatedEffectFact::new(
            application,
            application,
            unit_type(),
            RuntimeEvaluatedEffect::Drop {
                target,
                policy: RuntimeDropPolicyFact::Stop {
                    fade: RuntimeDropFadeFact::Operand(fade),
                },
            },
        ),
    );

    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("Stop fade must reference a valid checked operand source"),
        RuntimeSemanticFactsError::InvalidEvaluatedEffectFact {
            expression: application,
        }
    );
}

#[test]
fn missing_expression_type_is_rejected_before_publication() {
    let project = project_fixture("missing-expression-type", "fn root() { true }\n");
    let owner = boolean_literal(&project);

    assert_eq!(
        runtime_facts(&project, RuntimePlanSemanticFactInput::new())
            .expect_err("an accepted expression cannot omit its type"),
        RuntimeSemanticFactsError::MissingExpressionType { expression: owner },
    );
}

#[test]
fn missing_pattern_type_is_rejected_before_publication() {
    let project = project_fixture(
        "missing-pattern-type",
        "fn root(value: bool) -> Unit {\n    match value { true => (), false => () }\n}\n",
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    let runtime_owners = runtime_reachability(&project);
    for owner in runtime_owners.locals() {
        input.push_local_declaration(owner, unit_type(), fixture_local_origin(&project, owner));
    }
    for owner in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(
            owner,
            unit_type(),
            fixture_expression_origin(&project, owner),
        );
    }
    let pattern = runtime_owners.patterns().next().expect("pattern fixture");

    assert_eq!(
        runtime_facts(&project, input).expect_err("an accepted pattern cannot omit its type"),
        RuntimeSemanticFactsError::MissingPatternType { pattern },
    );
}

#[test]
fn duplicate_expression_types_are_rejected_before_publication() {
    let project = project_fixture("duplicate-expression-type", "fn root() -> bool { true }\n");
    let owner = boolean_literal(&project);
    let mut input = complete_type_input(&project);
    input.push_expression_type(
        owner,
        unit_type(),
        fixture_expression_origin(&project, owner),
    );

    assert_eq!(
        runtime_facts(&project, input).expect_err("one expression cannot own two accepted types"),
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
        .analysis_view()
        .expect("executable fixture")
        .modules()
        .flat_map(|(_, module)| module.patterns())
        .map(|(owner, _)| owner)
        .next()
        .expect("pattern fixture");
    let mut input = complete_type_input(&project);
    input.push_pattern_type(pattern, unit_type());

    assert_eq!(
        runtime_facts(&project, input).expect_err("one pattern cannot own two accepted types"),
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
            Some(RuntimeUnsupportedTypeShape::Range),
            RuntimeOperationalType::Range,
        ),
        (
            RuntimeTypeShape::Iterator(boxed_unit_type()),
            Some(RuntimeUnsupportedTypeShape::Iterator),
            RuntimeOperationalType::Iterator,
        ),
        (
            RuntimeTypeShape::Need(boxed_unit_type()),
            None,
            RuntimeOperationalType::Need,
        ),
        (
            RuntimeTypeShape::Stream {
                item: boxed_unit_type(),
                error: boxed_unit_type(),
            },
            Some(RuntimeUnsupportedTypeShape::Stream),
            RuntimeOperationalType::Stream,
        ),
        (
            RuntimeTypeShape::ThreadHandle(boxed_unit_type()),
            Some(RuntimeUnsupportedTypeShape::ThreadHandle),
            RuntimeOperationalType::ThreadHandle,
        ),
        (
            RuntimeTypeShape::Shared(boxed_unit_type()),
            Some(RuntimeUnsupportedTypeShape::Shared),
            RuntimeOperationalType::Shared,
        ),
        (
            RuntimeTypeShape::Reference(boxed_unit_type()),
            Some(RuntimeUnsupportedTypeShape::Reference),
            RuntimeOperationalType::Reference,
        ),
        (
            RuntimeTypeShape::Function {
                contract: Default::default(),
                parameters: vec![unit_type()].into_boxed_slice(),
                result: boxed_unit_type(),
            },
            None,
            RuntimeOperationalType::Function,
        ),
    ];

    for (index, (shape, unsupported, operational)) in cases.into_iter().enumerate() {
        let marker = 0x30_u8 + u8::try_from(index).expect("bounded operational fixture");
        let identity = RuntimeSemanticTypeId::from_bytes([marker; 32]);
        let normalized = super::RuntimeNormalizedType::new(identity, shape);
        if unsupported.is_none() {
            let checked = normalized.checked_type().unwrap();
            assert_eq!(checked, RuntimeCheckedType::ExecutableRef(identity));
            assert_eq!(checked.semantic_identity_digest(), identity);
            assert!(!checked.accepts_value(&RuntimeValue::Unit));
        } else {
            assert_eq!(
                normalized.checked_type(),
                Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
                    semantic_identity: identity,
                    path: super::RuntimeTypeProjectionPath::root(),
                    shape: unsupported.expect("unsupported operational shape"),
                })
            );
        }
        assert_eq!(
            normalized
                .runtime_plan_type_seed()
                .map(|seed| seed.projection().operational_type()),
            Ok(Some(operational))
        );
    }

    let identity = RuntimeSemanticTypeId::from_bytes([0x46; 32]);
    let map = super::RuntimeNormalizedType::new(
        identity,
        RuntimeTypeShape::Map {
            kind: RuntimeMapKind::BTree,
            key: boxed_unit_type(),
            value: boxed_unit_type(),
        },
    );
    assert_eq!(
        map.checked_type(),
        Ok(RuntimeCheckedType::Map {
            kind: RuntimeMapKind::BTree,
            key: Box::new(RuntimeCheckedType::Unit),
            value: Box::new(RuntimeCheckedType::Unit),
        })
    );
    assert_eq!(
        map.runtime_plan_type_seed()
            .map(|seed| seed.projection().operational_type()),
        Ok(Some(RuntimeOperationalType::Map))
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive table proves every closed Agent type mapping"
)]
fn every_agent_shape_selects_its_closed_operational_family() {
    let cases = vec![
        (
            RuntimeAgentTypeShape::DebugStatePath,
            RuntimeAgentOperationalType::DebugStatePath,
        ),
        (
            RuntimeAgentTypeShape::ObservationFieldPath,
            RuntimeAgentOperationalType::ObservationFieldPath,
        ),
        (
            RuntimeAgentTypeShape::Probe(boxed_unit_type()),
            RuntimeAgentOperationalType::Probe,
        ),
        (
            RuntimeAgentTypeShape::Predicate,
            RuntimeAgentOperationalType::Predicate,
        ),
        (
            RuntimeAgentTypeShape::Observation,
            RuntimeAgentOperationalType::Observation,
        ),
        (
            RuntimeAgentTypeShape::ObservedObject,
            RuntimeAgentOperationalType::ObservedObject,
        ),
        (
            RuntimeAgentTypeShape::BoundingBox,
            RuntimeAgentOperationalType::BoundingBox,
        ),
        (
            RuntimeAgentTypeShape::ActionName,
            RuntimeAgentOperationalType::ActionName,
        ),
        (
            RuntimeAgentTypeShape::ActionTarget,
            RuntimeAgentOperationalType::ActionTarget,
        ),
        (
            RuntimeAgentTypeShape::ActionResult,
            RuntimeAgentOperationalType::ActionResult,
        ),
        (
            RuntimeAgentTypeShape::DataShape(boxed_unit_type()),
            RuntimeAgentOperationalType::DataShape,
        ),
        (
            RuntimeAgentTypeShape::EntityMetadata,
            RuntimeAgentOperationalType::EntityMetadata,
        ),
        (
            RuntimeAgentTypeShape::SourceAnchor,
            RuntimeAgentOperationalType::SourceAnchor,
        ),
        (
            RuntimeAgentTypeShape::ProjectGraphNeighborhood,
            RuntimeAgentOperationalType::ProjectGraphNeighborhood,
        ),
        (
            RuntimeAgentTypeShape::ProjectGraphSymbol,
            RuntimeAgentOperationalType::ProjectGraphSymbol,
        ),
        (
            RuntimeAgentTypeShape::ProjectGraphEdge,
            RuntimeAgentOperationalType::ProjectGraphEdge,
        ),
        (
            RuntimeAgentTypeShape::CaptureTarget,
            RuntimeAgentOperationalType::CaptureTarget,
        ),
        (
            RuntimeAgentTypeShape::CaptureReference,
            RuntimeAgentOperationalType::CaptureReference,
        ),
        (
            RuntimeAgentTypeShape::Resource,
            RuntimeAgentOperationalType::Resource,
        ),
        (
            RuntimeAgentTypeShape::RagContextPack,
            RuntimeAgentOperationalType::RagContextPack,
        ),
        (
            RuntimeAgentTypeShape::ObservedObjectId,
            RuntimeAgentOperationalType::ObservedObjectId,
        ),
        (
            RuntimeAgentTypeShape::Diagnostics,
            RuntimeAgentOperationalType::Diagnostics,
        ),
        (
            RuntimeAgentTypeShape::WaitError,
            RuntimeAgentOperationalType::WaitError,
        ),
        (
            RuntimeAgentTypeShape::ViewportPoint,
            RuntimeAgentOperationalType::ViewportPoint,
        ),
        (
            RuntimeAgentTypeShape::RagError,
            RuntimeAgentOperationalType::RagError,
        ),
        (
            RuntimeAgentTypeShape::SourcePosition,
            RuntimeAgentOperationalType::SourcePosition,
        ),
        (
            RuntimeAgentTypeShape::ProjectFlowControlSummary,
            RuntimeAgentOperationalType::ProjectFlowControlSummary,
        ),
        (
            RuntimeAgentTypeShape::ProjectGraphSummary,
            RuntimeAgentOperationalType::ProjectGraphSummary,
        ),
        (
            RuntimeAgentTypeShape::BinaryResourceBody,
            RuntimeAgentOperationalType::BinaryResourceBody,
        ),
        (
            RuntimeAgentTypeShape::BinaryData,
            RuntimeAgentOperationalType::BinaryData,
        ),
    ];

    for (index, (shape, operational)) in cases.into_iter().enumerate() {
        let marker = u8::try_from(index + 1).expect("bounded Agent type fixture");
        let normalized = normalized_type(marker, RuntimeTypeShape::Agent(shape));
        let expected = match operational {
            RuntimeAgentOperationalType::Probe => {
                arcweft_core::plan::RuntimeAgentTypeProjection::Probe(Box::new(
                    RuntimeCheckedType::Unit,
                ))
            }
            RuntimeAgentOperationalType::DataShape => {
                arcweft_core::plan::RuntimeAgentTypeProjection::DataShape(Box::new(
                    RuntimeCheckedType::Unit,
                ))
            }
            leaf => arcweft_core::plan::RuntimeAgentTypeProjection::try_leaf(leaf).unwrap(),
        };
        assert_eq!(
            normalized.checked_type(),
            Ok(RuntimeCheckedType::Agent(expected))
        );
        assert_eq!(
            normalized
                .runtime_plan_type_seed()
                .map(|seed| seed.projection().operational_type()),
            Ok(Some(RuntimeOperationalType::Agent(operational)))
        );
    }
}

#[test]
fn nested_operational_descendants_select_their_outer_composite_family() {
    let result_value = unsupported_range_type();
    let result_error = unit_type();
    let option_item = unsupported_range_type();
    let cases = vec![
        (
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Probe(Box::new(
                unsupported_range_type(),
            ))),
            RuntimeTypeProjectionStep::AgentProbeValue,
            RuntimeOperationalType::Agent(RuntimeAgentOperationalType::Probe),
        ),
        (
            RuntimeTypeShape::Agent(RuntimeAgentTypeShape::DataShape(Box::new(
                unsupported_range_type(),
            ))),
            RuntimeTypeProjectionStep::AgentDataShapeValue,
            RuntimeOperationalType::Agent(RuntimeAgentOperationalType::DataShape),
        ),
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
                length: 1.into(),
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
                value: Box::new(result_value.clone()),
                error: Box::new(result_error.clone()),
                value_payload: Box::new(tuple_payload(0x81, result_value)),
                error_payload: Box::new(tuple_payload(0x82, result_error)),
            },
            RuntimeTypeProjectionStep::ResultOk,
            RuntimeOperationalType::Result,
        ),
        (
            RuntimeTypeShape::Option {
                item: Box::new(option_item.clone()),
                some_payload: Box::new(tuple_payload(0x83, option_item)),
            },
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
            normalized
                .runtime_plan_type_seed()
                .map(|seed| seed.projection().operational_type()),
            Ok(Some(operational))
        );
    }
}

#[test]
fn complete_checked_composites_retain_their_exact_checked_predicate() {
    let option_item = unit_type();
    let option_payload = tuple_payload(0x94, option_item.clone());
    let result_value = normalized_type(
        0x91,
        RuntimeTypeShape::Option {
            item: Box::new(option_item),
            some_payload: Box::new(option_payload),
        },
    );
    let result_error = normalized_type(
        0x92,
        RuntimeTypeShape::Sequence {
            kind: RuntimeSequenceKind::Seq,
            item: Box::new(normalized_type(0x93, RuntimeTypeShape::Bool)),
        },
    );
    let result_value_payload = tuple_payload(0x95, result_value.clone());
    let result_error_payload = tuple_payload(0x96, result_error.clone());
    let normalized = normalized_type(
        0x90,
        RuntimeTypeShape::Result {
            value: Box::new(result_value),
            error: Box::new(result_error),
            value_payload: Box::new(result_value_payload),
            error_payload: Box::new(result_error_payload),
        },
    );

    assert_eq!(
        normalized
            .runtime_plan_type_seed()
            .map(|seed| seed.projection().clone()),
        Ok(RuntimePlanTypeProjection::Result {
            value: RuntimeSemanticTypeId::from_bytes([0x91; 32]),
            error: RuntimeSemanticTypeId::from_bytes([0x92; 32]),
            value_payload: RuntimeSemanticTypeId::from_bytes([0x95; 32]),
            error_payload: RuntimeSemanticTypeId::from_bytes([0x96; 32]),
        })
    );
}

#[test]
fn normalized_array_projection_retains_its_exact_length() {
    let normalized = normalized_type(
        0x95,
        RuntimeTypeShape::Array {
            item: Box::new(unit_type()),
            length: 2.into(),
        },
    );
    let expected = RuntimeCheckedType::Array {
        item: Box::new(RuntimeCheckedType::Unit),
        length: 2,
    };
    assert_eq!(normalized.checked_type().unwrap(), expected);
    assert!(matches!(
        normalized.runtime_plan_type_seed().unwrap().projection(),
        RuntimePlanTypeProjection::Array {
            length: arcweft_core::plan::RuntimeArrayLength::Constant(2),
            ..
        }
    ));
}

#[test]
fn normalized_map_projection_retains_its_exact_ordering_kind() {
    for (marker, kind) in [
        (0xa1, RuntimeMapKind::Ordered),
        (0xa2, RuntimeMapKind::Sorted),
        (0xa3, RuntimeMapKind::BTree),
    ] {
        let normalized = normalized_type(
            marker,
            RuntimeTypeShape::Map {
                kind,
                key: boxed_unit_type(),
                value: boxed_unit_type(),
            },
        );
        assert_eq!(
            normalized
                .runtime_plan_type_seed()
                .expect("map type projects into RuntimePlan")
                .projection(),
            &RuntimePlanTypeProjection::Map {
                kind,
                key: RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                value: RuntimeSemanticTypeId::from_bytes([0x11; 32]),
            }
        );
    }
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
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        },
    );
    assert_eq!(
        opaque
            .runtime_plan_type_seed()
            .map(|seed| seed.projection().clone()),
        Ok(RuntimePlanTypeProjection::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        })
    );
}

#[test]
fn affine_snapshot_only_opaque_shape_preserves_owner_through_plan_projection() {
    let identity = RuntimeSemanticTypeId::from_bytes([0xa1; 32]);
    let producer = RuntimeHandleKind::Cue
        .try_producer()
        .expect("standard cue producer");
    let normalized = super::RuntimeNormalizedType::new(
        identity,
        RuntimeTypeShape::Opaque {
            producer: producer.clone(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Box::new([]),
        },
    );
    assert_eq!(
        normalized
            .runtime_plan_type_seed()
            .map(|seed| seed.projection().clone()),
        Ok(RuntimePlanTypeProjection::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Box::new([]),
        })
    );
    let RuntimeCheckedType::Opaque { owner } = normalized
        .checked_type()
        .expect("snapshot-only opaque owner projects")
    else {
        panic!("snapshot-only handle remains opaque")
    };
    assert_eq!(owner.semantic_identity(), identity);
    assert_eq!(
        owner.value_class(),
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue)
    );
    assert_eq!(owner.persistence(), RuntimeOpaquePersistence::SnapshotOnly);
}

#[test]
fn nested_operational_expression_type_is_retained_without_reconstruction() {
    let project = project_fixture(
        "nested-operational-type",
        "fn root(value: bool) -> bool { true }\n",
    );
    let owner = boolean_literal(&project);
    let leaf = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x22; 32]),
        RuntimeTypeShape::Unit,
    );
    let range = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x33; 32]),
        RuntimeTypeShape::Range(Box::new(leaf)),
    );
    let range_payload = tuple_payload(0x45, range.clone());
    let nested = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x44; 32]),
        RuntimeTypeShape::Option {
            item: Box::new(range),
            some_payload: Box::new(range_payload),
        },
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    let runtime_owners = runtime_reachability(&project);
    for local in runtime_owners.locals() {
        input.push_local_declaration(local, nested.clone(), fixture_local_origin(&project, local));
    }
    for pattern in runtime_owners.patterns() {
        input.push_pattern_type(pattern, nested.clone());
    }
    for expression in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(
            expression,
            nested.clone(),
            fixture_expression_origin(&project, expression),
        );
    }
    let facts = runtime_facts(&project, input)
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
        "fn root(items: Vec<i64>, subject: i64) -> i64 {\n    items[{ match subject { value => value }; 0i64 }]\n}\n",
    );
    let executable = project.analysis_view().expect("executable fixture");
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

    let postfix_candidates = BTreeMap::from([(postfix_owner, index)]);
    let runtime_owners = runtime_reachability_with(
        &project,
        |owner| postfix_candidates.get(&owner).copied(),
        |owner| retained_runtime_projection(executable, owner),
    );
    let accepted = runtime_owners
        .selected_expression_type_owners()
        .expect("selected runtime expression-type inventory");
    assert!(accepted.contains(&postfix_owner));
    assert!(accepted.contains(&target));
    assert!(!accepted.contains(&dialogue));
    assert!(accepted.contains(&index));

    let complete_selected_input = || {
        let mut input = RuntimePlanSemanticFactInput::new();
        for owner in runtime_owners.locals() {
            input.push_local_declaration(owner, unit_type(), fixture_local_origin(&project, owner));
        }
        for owner in &accepted {
            input.push_expression_type(
                *owner,
                unit_type(),
                fixture_expression_origin(&project, *owner),
            );
        }
        for owner in runtime_owners.patterns() {
            input.push_pattern_type(owner, unit_type());
        }
        input.push_postfix_candidate(postfix_owner, index);
        input
    };
    let facts =
        RuntimePlanSemanticFacts::try_new(executable, &runtime_owners, complete_selected_input())
            .expect("the rolled-back dialogue candidate needs no type fact");
    assert!(facts.expression_type(postfix_owner).is_some());
    assert!(facts.expression_type(target).is_some());
    assert!(facts.expression_type(dialogue).is_none());
    assert!(facts.expression_type(index).is_some());

    let mut missing_selection = complete_selected_input();
    missing_selection.postfix_candidates.clear();
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, &runtime_owners, missing_selection)
            .expect_err("the selector requires its exact accepted choice"),
        RuntimeSemanticFactsError::MissingPostfixCandidate {
            expression: postfix_owner
        }
    );

    assert!(
        analyze_identity_fixture(&project)
            .expression_origin(dialogue)
            .is_err(),
        "the unselected candidate cannot obtain mandatory accepted-origin evidence"
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
fn opaque_composite_projection_preserves_complete_owner_and_first_error_path() {
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.runtime-plan.opaque")
        .expect("valid fixture producer");
    let opaque = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([1; 32]),
        RuntimeTypeShape::Opaque {
            producer: producer.clone(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::Plain,
            persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments: Box::new([]),
        },
    );
    let opaque_value_payload = tuple_payload(0xa2, opaque.clone());
    let opaque_error_payload = tuple_payload(0xa3, opaque.clone());
    let closed = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([2; 32]),
        RuntimeTypeShape::Result {
            value: Box::new(opaque.clone()),
            error: Box::new(opaque),
            value_payload: Box::new(opaque_value_payload),
            error_payload: Box::new(opaque_error_payload),
        },
    );
    assert!(matches!(
        closed.checked_type().expect("complete opaque Result owner"),
        RuntimeCheckedType::Result { ok, error }
            if matches!(*ok, RuntimeCheckedType::Opaque { .. })
                && matches!(*error, RuntimeCheckedType::Opaque { .. })
    ));

    let unsupported_value = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([4; 32]),
        RuntimeTypeShape::Range(Box::new(super::RuntimeNormalizedType::new(
            RuntimeSemanticTypeId::from_bytes([5; 32]),
            RuntimeTypeShape::Unit,
        ))),
    );
    let unsupported_error = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([6; 32]),
        RuntimeTypeShape::Function {
            contract: Default::default(),
            parameters: Box::new([]),
            result: Box::new(super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes([7; 32]),
                RuntimeTypeShape::Unit,
            )),
        },
    );
    let unsupported = super::RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([3; 32]),
        RuntimeTypeShape::Result {
            value: Box::new(unsupported_value.clone()),
            error: Box::new(unsupported_error.clone()),
            value_payload: Box::new(tuple_payload(0x08, unsupported_value)),
            error_payload: Box::new(tuple_payload(0x09, unsupported_error)),
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
    let ok_payload = tuple_payload(0x0a, ok.clone());
    let error_payload = tuple_payload(0x0b, error.clone());
    let variant = RuntimeResolvedVariant::result(
        RuntimeSemanticTypeId::from_bytes([0x0c; 32]),
        ok.clone(),
        error,
        result_cases(ok_payload.clone(), error_payload),
        0,
        "Ok",
    )
    .expect("accepted Result case");
    assert_eq!(
        variant
            .selected_payload_type()
            .expect("selected normalized Result payload"),
        Some(&ok_payload)
    );
    let selection = variant
        .checked_selection()
        .expect("complete Result selection");
    assert_eq!(selection.ordinal(), 0);
    assert_eq!(selection.name(), "Ok");
    assert_eq!(
        selection.payload(),
        Some(&RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::Unit]))
    );
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
    let payload = tuple_payload(0x73, item.clone());
    let identity = RuntimeSemanticTypeId::from_bytes([0x74; 32]);
    let some = RuntimeResolvedVariant::option(
        identity,
        item.clone(),
        option_cases(payload.clone()),
        0,
        "Some",
    )
    .expect("accepted Option Some case");
    assert_eq!(some.selected_name(), Ok("Some"));
    assert_eq!(some.selected_payload_type(), Ok(Some(&payload)));
    assert_eq!(
        some.checked_selection()
            .expect("Some checked selection")
            .payload(),
        Some(&RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::Unit]))
    );

    let none = RuntimeResolvedVariant::option(identity, item, option_cases(payload), 1, "None")
        .expect("accepted Option None case");
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
        variant_source_graph(
            0x72,
            "fixture.CharacterState",
            &[("Idle", None), ("Speaking", None)],
        ),
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
    let payload = normalized_type(
        0x81,
        RuntimeTypeShape::Tuple(vec![normalized_type(0x80, RuntimeTypeShape::String)].into()),
    );
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
    let variant = RuntimeResolvedVariant::builtin_closed(
        identity,
        nominal.clone(),
        variant_source_graph(
            0x82,
            "fixture.NormalizedVariant",
            &[
                ("Empty", None),
                (
                    "Payload",
                    Some(RuntimeTypeSchema::Tuple(
                        vec![RuntimeTypeSchema::String].into(),
                    )),
                ),
            ],
        ),
        cases(),
        1,
        "Payload",
    )
    .expect("name and ordinal select the normalized row");
    assert_eq!(variant.selected_name(), Ok("Payload"));
    assert_eq!(variant.selected_payload_type(), Ok(Some(&payload)));

    let selection = variant
        .checked_selection()
        .expect("checked view derives from the normalized table");
    assert_eq!(selection.name(), "Payload");
    assert_eq!(
        selection.payload(),
        Some(&RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::String]))
    );
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
        Some(&RuntimeCheckedType::Tuple(vec![RuntimeCheckedType::String]))
    );

    assert!(matches!(
        RuntimeResolvedVariant::builtin_closed(identity, nominal, variant_source_graph(0x82, "fixture.NormalizedVariant", &[("Empty", None), ("Payload", Some(RuntimeTypeSchema::Tuple(vec![RuntimeTypeSchema::String].into())))]), cases(), 1, "Other"),
        Err(RuntimeResolvedVariantError::CaseName {
            ordinal: 1,
            expected,
            actual,
        }) if expected == "Payload" && actual == "Other"
    ));
}

fn variant_source_graph(
    marker: u8,
    nominal: &str,
    cases: &[(&str, Option<RuntimeTypeSchema>)],
) -> Arc<RuntimeNominalSchemaGraph> {
    Arc::new(
        RuntimeNominalSchemaGraph::try_new(
            vec![RuntimeNominalSchemaDefinition::new(
                arcweft_core::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(RuntimeNominalSchemaIdentity::new(
                        RuntimeNominalTypeId::try_new(nominal).unwrap(),
                        RuntimeSemanticTypeId::from_bytes([marker; 32]),
                    ))
                    .semantic_identity()
                    .as_bytes(),
                ),
                RuntimeNominalSchemaIdentity::new(
                    RuntimeNominalTypeId::try_new(nominal).unwrap(),
                    RuntimeSemanticTypeId::from_bytes([marker; 32]),
                ),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: cases
                        .iter()
                        .enumerate()
                        .map(|(ordinal, (name, payload))| {
                            RuntimeNominalSchemaCase::new(
                                u32::try_from(ordinal).unwrap(),
                                (*name).to_owned(),
                                payload.clone(),
                            )
                        })
                        .collect(),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap(),
    )
}

#[test]
fn operational_variant_payload_is_not_admitted_through_raw_facts() {
    let variant = RuntimeResolvedVariant::builtin_closed(
        RuntimeSemanticTypeId::from_bytes([0x83; 32]),
        RuntimeNominalTypeId::try_new("fixture.OperationalVariant").expect("valid fixture nominal"),
        variant_source_graph(
            0x83,
            "fixture.OperationalVariant",
            &[(
                "Payload",
                Some(RuntimeTypeSchema::Tuple(
                    vec![RuntimeTypeSchema::Unit].into(),
                )),
            )],
        ),
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

#[test]
fn project_record_variant_admission_rejects_swapped_field_types_and_order() {
    let project = project_fixture(
        "record-variant-admission",
        "enum Event { ChoiceSelected { id: i64, active: bool } }\n",
    );
    let executable = project.analysis_view().expect("record variant HIR");
    let (_, module) = executable.modules().next().expect("root module");
    let document = Arc::clone(module.provenance().document());
    let world = ProjectSymbolWorldId::try_new(
        project.package().clone(),
        document.identity().id().clone(),
        "record-variant-admission",
    )
    .expect("project symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([document.identity()])
        .expect("project symbol revision");
    let externals = ProjectExternalDeclarations::try_new(world, revision, Vec::new())
        .expect("empty project externals");
    let symbols = ProjectSymbolTable::link(project.view(), &externals)
        .expect("project symbols link")
        .into_table();
    let event = symbols
        .nominal_symbols()
        .find(|nominal| nominal.id().name().as_str() == "Event")
        .expect("Event nominal");
    let identity = RuntimeSemanticTypeId::from_bytes([0x84; 32]);
    let runtime_nominal =
        RuntimeNominalTypeId::try_new("test::variant::Event").expect("runtime Event nominal");
    let graph = variant_source_graph(
        0x84,
        "test::variant::Event",
        &[(
            "ChoiceSelected",
            Some(RuntimeTypeSchema::RecordValue {
                fields: [
                    RuntimeSchemaValueField::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                        "id".to_owned(),
                        RuntimeTypeSchema::I64,
                    ),
                    RuntimeSchemaValueField::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap(),
                        "active".to_owned(),
                        RuntimeTypeSchema::Bool,
                    ),
                ]
                .into(),
            }),
        )],
    );
    let nominal = RuntimeResolvedNominal::project(
        event.id().clone(),
        event.owner(),
        runtime_nominal,
        identity,
        graph
            .try_layout_hash(identity)
            .expect("sealed source layout"),
        graph,
    );
    let modules = executable
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let scalar = |marker, shape| {
        super::RuntimeNormalizedType::new(RuntimeSemanticTypeId::from_bytes([marker; 32]), shape)
    };
    let record = |fields: [(&str, RuntimeTypeShape); 2]| {
        super::RuntimeNormalizedType::new(
            RuntimeSemanticTypeId::from_bytes([0x85; 32]),
            RuntimeTypeShape::Record(
                fields
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, shape))| {
                        RuntimeRecordTypeField::new(name, scalar(0x86 + index as u8, shape))
                    })
                    .collect(),
            ),
        )
    };
    for (label, fields, admitted) in [
        (
            "declared",
            [
                (
                    "id",
                    RuntimeTypeShape::Signed(arcweft_core::value::RuntimeSignedIntWidth::I64),
                ),
                ("active", RuntimeTypeShape::Bool),
            ],
            true,
        ),
        (
            "swapped types",
            [
                ("id", RuntimeTypeShape::Bool),
                (
                    "active",
                    RuntimeTypeShape::Signed(arcweft_core::value::RuntimeSignedIntWidth::I64),
                ),
            ],
            false,
        ),
        (
            "swapped order",
            [
                ("active", RuntimeTypeShape::Bool),
                (
                    "id",
                    RuntimeTypeShape::Signed(arcweft_core::value::RuntimeSignedIntWidth::I64),
                ),
            ],
            false,
        ),
    ] {
        let variant = RuntimeResolvedVariant::project(
            nominal.clone(),
            Box::new([]),
            0,
            "ChoiceSelected",
            [RuntimeNormalizedVariantCase::new(
                "ChoiceSelected",
                Some(record(fields)),
            )]
            .into(),
        )
        .expect("normalized case has a coherent checked shape");
        let result = super::validate_variant(&modules, &variant);
        if admitted {
            assert_eq!(result, Ok(()), "{label}");
        } else {
            assert_eq!(
                result,
                Err(RuntimeSemanticFactsError::WrongVariantIdentity),
                "{label}"
            );
        }
    }
}

#[derive(Clone)]
struct IteratorMethodFixture {
    implementation: arcweft_lang_hir::identity::ItemId,
    member: u16,
    declaration: ImplMethodDeclarationId,
}

fn iterator_fixture_symbols(project: &HirProject) -> ProjectSymbolTable {
    let executable = project.analysis_view().expect("clean iterator fixture");
    let (_, first_module) = executable
        .modules()
        .next()
        .expect("iterator fixture module");
    let world = ProjectSymbolWorldId::try_new(
        executable.package().clone(),
        first_module.provenance().source_identity().id().clone(),
        "runtime-plan-iterator-witness-edge-test",
    )
    .expect("iterator symbol world");
    let revision = ProjectSymbolRevision::try_for_documents(
        executable
            .modules()
            .map(|(_, module)| module.provenance().source_identity()),
    )
    .expect("iterator symbol revision");
    let externals = ProjectExternalDeclarations::try_new(world, revision, Vec::new())
        .expect("iterator fixture external declarations");
    ProjectSymbolTable::link(project.view(), &externals)
        .expect("iterator fixture symbols")
        .into_table()
}

fn iterator_method_fixture(
    project: &HirProject,
    implementation_ordinal: usize,
    method_name: &str,
) -> IteratorMethodFixture {
    let executable = project.analysis_view().expect("iterator edge fixture");
    let (_, module) = executable.modules().next().expect("root fixture module");
    let (implementation, declaration) = module
        .items()
        .filter_map(|(owner, item)| match item.kind() {
            HirItemKind::Impl(declaration) => Some((owner, declaration)),
            _ => None,
        })
        .nth(implementation_ordinal)
        .expect("fixture Impl declaration");
    let member = declaration
        .members()
        .iter()
        .position(|member| {
            matches!(
                member,
                HirImplMember::Function(function)
                    if function
                        .name()
                        .resolved()
                        .is_some_and(|name| name.as_str() == method_name)
            )
        })
        .and_then(|member| u16::try_from(member).ok())
        .expect("fixture method member");
    let symbols = iterator_fixture_symbols(project);
    let declaration = symbols
        .callable_symbols()
        .find_map(|symbol| {
            if symbol.source_item() != implementation {
                return None;
            }
            let CallableDeclarationKey::ImplMethod(method) = symbol.declaration() else {
                return None;
            };
            (method.method().as_str() == method_name).then(|| method.clone())
        })
        .expect("linked fixture method identity");
    IteratorMethodFixture {
        implementation,
        member,
        declaration,
    }
}

fn iterator_method_edge(
    statement: arcweft_lang_hir::identity::StmtId,
    role: HirRuntimeIteratorWitnessMethodRole,
    method: &IteratorMethodFixture,
) -> HirRuntimeReachabilityEdge {
    HirRuntimeReachabilityEdge::new(
        HirRuntimeReachabilitySite::Statement(statement),
        HirRuntimeExecutableOwner::ImplMethod(method.declaration.clone()),
        HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod {
            role,
            implementation: method.implementation,
            member: method.member,
            method: method.declaration.clone(),
        },
    )
}

fn iterator_reachability_with_edges<'project>(
    project: &'project HirProject,
    edges: Vec<HirRuntimeReachabilityEdge>,
) -> HirRuntimeSemanticReachability<'project> {
    let executable = project.analysis_view().expect("clean iterator fixture");
    let symbols = iterator_fixture_symbols(project);
    let world = symbols.world().clone();
    let revision = *symbols.revision();
    let roots = executable
        .items()
        .filter(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
        .map(|item| {
            HirRuntimeReachabilityRoot::new(
                HirRuntimeReachabilityRootKind::CheckedFlow,
                HirRuntimeExecutableOwner::Item(item.id()),
            )
        })
        .collect::<Vec<_>>();
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted iterator symbol generation")
        .into_evaluation_topology()
        .expect("iterator evaluation topology");
    let input = HirRuntimeSemanticReachabilityInput::try_new(
        HirRuntimeEmissionMode::CheckAll,
        world,
        revision,
        roots,
        edges,
    )
    .expect("accepted iterator reachability input");
    executable
        .runtime_semantic_reachability(
            input,
            &topology,
            |_| None,
            |owner| selected_call_inventory(executable, owner),
            |owner| retained_runtime_projection(executable, owner),
        )
        .expect("accepted iterator reachability")
}

fn iterator_method_fact(
    project: &HirProject,
    method: &IteratorMethodFixture,
    trait_identity: RuntimeTraitIdentity,
) -> RuntimeTraitMethodFact {
    let definition = iterator_method_definition(project, method);
    RuntimeTraitMethodFact::try_new(
        method.declaration.clone(),
        method.implementation,
        method.member,
        trait_identity,
        unit_type(),
        definition,
    )
    .unwrap()
}

fn identity_iterator_fact(method: &IteratorMethodFixture) -> RuntimeIteratorFact {
    RuntimeIteratorFact::Witness(Box::new(RuntimeIteratorWitnessFact::new(
        unit_type(),
        unit_type(),
        RuntimeIteratorWitnessExecutableFact::IdentityIntoIterator {
            next: method.declaration.clone(),
        },
    )))
}

fn trait_call_iterator_fact(
    into_iter: &IteratorMethodFixture,
    next: &IteratorMethodFixture,
) -> RuntimeIteratorFact {
    RuntimeIteratorFact::Witness(Box::new(RuntimeIteratorWitnessFact::new(
        unit_type(),
        unit_type(),
        RuntimeIteratorWitnessExecutableFact::TraitCalls {
            into_iter: into_iter.declaration.clone(),
            next: next.declaration.clone(),
        },
    )))
}

fn iterator_edge_error(
    project: &HirProject,
    statement: arcweft_lang_hir::identity::StmtId,
    edges: Vec<HirRuntimeReachabilityEdge>,
    iteration: &RuntimeIteratorFact,
    methods: &BTreeMap<RuntimeTraitMethodInstanceKey, RuntimeTraitMethodFact>,
) -> RuntimeSemanticFactsError {
    let reachability = iterator_reachability_with_edges(project, edges);
    validate_iterator_witness_method_edges(
        RuntimeSemanticOwnerSet::runtime_only(&reachability),
        statement,
        unit_type().identity(),
        iteration,
        methods,
    )
    .expect_err("tampered iterator witness edge")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed tamper matrix proves every field of the statement-owned iterator edge and both witness variants"
)]
fn iterator_witness_method_edges_are_exact_and_fail_closed() {
    let project = project_fixture(
        "iterator-witness-edges",
        concat!(
            "struct Counter { end: i64 }\n",
            "struct CounterIter { current: i64, end: i64 }\n",
            "impl IntoIterator for Counter {\n",
            "    type Item = i64\n",
            "    type IntoIter = CounterIter\n",
            "    fn into_iter(self) -> CounterIter {\n",
            "        CounterIter { current: 0, end: self.end }\n",
            "    }\n",
            "}\n",
            "impl Iterator for CounterIter {\n",
            "    type Item = i64\n",
            "    fn next(&mut self) -> Option<i64> { None }\n",
            "}\n",
            "struct OtherIter {}\n",
            "impl Iterator for OtherIter {\n",
            "    type Item = i64\n",
            "    fn next(&mut self) -> Option<i64> { None }\n",
            "}\n",
            "fn __runtime_plan_test_probe() -> Unit { () }\n",
            "flow iterator_edge_root() {\n",
            "    let counter = Counter { end: 1 }\n",
            "    for value in counter { value }\n",
            "}\n",
        ),
    );
    let statement = project
        .analysis_view()
        .expect("iterator edge fixture")
        .modules()
        .flat_map(|(_, module)| module.statements())
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::For(_)).then_some(owner)
        })
        .expect("fixture for statement");
    let into_iter = iterator_method_fixture(&project, 0, "into_iter");
    let next = iterator_method_fixture(&project, 1, "next");
    let other_next = iterator_method_fixture(&project, 2, "next");
    let next_edge = || {
        iterator_method_edge(
            statement,
            HirRuntimeIteratorWitnessMethodRole::IteratorNext,
            &next,
        )
    };
    let into_iter_edge = || {
        iterator_method_edge(
            statement,
            HirRuntimeIteratorWitnessMethodRole::IntoIterator,
            &into_iter,
        )
    };
    let into_fact = iterator_method_fact(
        &project,
        &into_iter,
        RuntimeTraitIdentity::StandardIntoIterator,
    );
    let next_fact = iterator_method_fact(&project, &next, RuntimeTraitIdentity::StandardIterator);
    let methods = BTreeMap::from([
        (into_fact.key().clone(), into_fact),
        (next_fact.key().clone(), next_fact),
    ]);
    let identity = identity_iterator_fact(&next);
    let trait_calls = trait_call_iterator_fact(&into_iter, &next);

    let accepted_identity = iterator_reachability_with_edges(&project, vec![next_edge()]);
    assert_eq!(
        validate_iterator_witness_method_edges(
            RuntimeSemanticOwnerSet::runtime_only(&accepted_identity),
            statement,
            unit_type().identity(),
            &identity,
            &methods,
        ),
        Ok(())
    );
    let accepted_trait_calls =
        iterator_reachability_with_edges(&project, vec![into_iter_edge(), next_edge()]);
    assert_eq!(
        validate_iterator_witness_method_edges(
            RuntimeSemanticOwnerSet::runtime_only(&accepted_trait_calls),
            statement,
            unit_type().identity(),
            &trait_calls,
            &methods,
        ),
        Ok(())
    );

    let wrong_role = iterator_method_edge(
        statement,
        HirRuntimeIteratorWitnessMethodRole::IntoIterator,
        &next,
    );
    assert_eq!(
        iterator_edge_error(&project, statement, vec![wrong_role], &identity, &methods),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );
    assert_eq!(
        iterator_edge_error(&project, statement, Vec::new(), &identity, &methods),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );
    assert_eq!(
        iterator_edge_error(
            &project,
            statement,
            vec![into_iter_edge(), next_edge()],
            &identity,
            &methods,
        ),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IntoIterator,
        }
    );
    let builtin = RuntimeIteratorFact::Builtin(Box::new(RuntimeBuiltinIteratorFact::new(
        RuntimeBuiltinIteratorFamily::Range,
        unit_type(),
        unit_type(),
        unit_type(),
        unit_type(),
    )));
    assert_eq!(
        iterator_edge_error(&project, statement, vec![next_edge()], &builtin, &methods,),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );

    let alternate_declaration = iterator_method_edge(
        statement,
        HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        &other_next,
    );
    assert_eq!(
        iterator_edge_error(
            &project,
            statement,
            vec![alternate_declaration],
            &identity,
            &methods,
        ),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );

    let mut wrong_implementation = methods.clone();
    let wrong = RuntimeTraitMethodFact::try_new(
        next.declaration.clone(),
        other_next.implementation,
        next.member,
        RuntimeTraitIdentity::StandardIterator,
        unit_type(),
        iterator_method_definition(&project, &next),
    )
    .unwrap();
    wrong_implementation.insert(wrong.key().clone(), wrong);
    assert_eq!(
        iterator_edge_error(
            &project,
            statement,
            vec![next_edge()],
            &identity,
            &wrong_implementation,
        ),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );
    let mut wrong_member = methods.clone();
    let wrong = RuntimeTraitMethodFact::try_new(
        next.declaration.clone(),
        next.implementation,
        next.member
            .checked_add(1)
            .expect("fixture member coordinate"),
        RuntimeTraitIdentity::StandardIterator,
        unit_type(),
        iterator_method_definition(&project, &next),
    )
    .unwrap();
    wrong_member.insert(wrong.key().clone(), wrong);
    assert_eq!(
        iterator_edge_error(
            &project,
            statement,
            vec![next_edge()],
            &identity,
            &wrong_member,
        ),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );
    let mut wrong_trait = methods;
    let wrong = iterator_method_fact(&project, &next, RuntimeTraitIdentity::StandardIntoIterator);
    wrong_trait.insert(wrong.key().clone(), wrong);
    assert_eq!(
        iterator_edge_error(
            &project,
            statement,
            vec![next_edge()],
            &identity,
            &wrong_trait,
        ),
        RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
            statement,
            role: HirRuntimeIteratorWitnessMethodRole::IteratorNext,
        }
    );
}

fn accepted_flow_definition_fixture(
    project: &arcweft_lang_hir::project::HirProject,
    owner: arcweft_lang_hir::identity::ItemId,
) -> Arc<arcweft_lang_sema::final_analysis::CheckedFlowExecutionDefinition> {
    use arcweft_lang_sema::{final_analysis::*, registration::*};
    let executable = project.analysis_view().unwrap();
    let documents = executable
        .modules()
        .map(|(_, module)| Arc::clone(module.provenance().document()))
        .collect::<Vec<_>>();
    let world = ProjectSymbolWorldId::try_new(
        executable.package().clone(),
        documents[0].identity().id().clone(),
        "runtime-plan-semantic-facts-test",
    )
    .unwrap();
    let registration =
        ProjectRegistrationFacts::try_new(world, documents, vec![], vec![], vec![]).unwrap();
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
        project.view(),
        &registration,
        None,
    ))
    .unwrap();
    let cancellation = std::sync::atomic::AtomicBool::new(false);
    let analysis = analyze_final_project(
        executable,
        registered.symbols(),
        FinalSemanticCatalogs::production(&registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .unwrap();
    Arc::new(
        analysis
            .checked_flow_execution_definition(executable, registered.symbols(), owner)
            .unwrap(),
    )
}

fn accepted_flow_fixture(
    project: &arcweft_lang_hir::project::HirProject,
    owner: arcweft_lang_hir::identity::ItemId,
    identity: arcweft_core::plan::FlowRuntimeId,
) -> crate::semantic_facts::RuntimeFlowFact {
    let definition = accepted_flow_definition_fixture(project, owner);
    {
        assert!(definition.body().parameters().is_empty());
        assert!(definition.body().inputs().is_empty());
        let unit = crate::semantic_facts::RuntimeNormalizedType::new(
            crate::semantic_facts::RuntimeSemanticTypeId::from_bytes([0x11; 32]),
            crate::semantic_facts::RuntimeTypeShape::Unit,
        );
        let signature = crate::semantic_facts::RuntimeNormalizedType::new(
            crate::semantic_facts::RuntimeSemanticTypeId::from_bytes([0x70; 32]),
            crate::semantic_facts::RuntimeTypeShape::Function {
                contract: arcweft_core::plan::RuntimeFunctionTypeContract::monomorphic(
                    definition.effects().clone(),
                ),
                parameters: Box::new([]),
                result: Box::new(unit),
            },
        );
        crate::semantic_facts::RuntimeFlowFact::try_new(identity, definition, signature).unwrap()
    }
}

fn iterator_method_definition(
    project: &HirProject,
    method: &IteratorMethodFixture,
) -> Arc<arcweft_lang_sema::final_analysis::CheckedExecutionInputAbi> {
    use arcweft_lang_sema::{final_analysis::*, registration::*};
    let executable = project.analysis_view().unwrap();
    let registration = ProjectRegistrationFacts::try_new(
        iterator_fixture_symbols(project).world().clone(),
        executable
            .modules()
            .map(|(_, module)| Arc::clone(module.provenance().document()))
            .collect(),
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
        project.view(),
        &registration,
        None,
    ))
    .unwrap();
    let cancellation = std::sync::atomic::AtomicBool::new(false);
    let analysis = analyze_final_project(
        executable,
        registered.symbols(),
        FinalSemanticCatalogs::production(&registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .unwrap();
    let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
        declaration: CallableDeclarationKey::ImplMethod(method.declaration.clone()),
        role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::ImplFunctionBody,
    });
    let context = analysis
        .checked_execution_context(executable, registered.symbols(), source.clone(), None)
        .unwrap();
    Arc::new(context.checked_execution_input_abi(source).unwrap())
}

#[test]
fn call_operand_request_role_presence_is_checked_before_publication() {
    let project = project_fixture("request-role-rejection", "fn root() { true }\n");
    let source = boolean_literal(&project);
    let group = arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(0).unwrap();
    let build = |origin, role| {
        RuntimeResolvedCall::try_new(
            RuntimeResolvedCallDispatch::Value { callee: source },
            group,
            vec![RuntimeResolvedCallOperand::new(
                0,
                origin,
                RuntimeResolvedCallOperandSource::Expression(source),
                unit_type(),
                RuntimeResolvedCallOperandBinding::Positional,
                RuntimeResolvedCallOperandProjection::Scalar,
                None,
                role,
            )],
            None,
            None,
            RuntimeCallResultShape::Value,
        )
    };
    assert_eq!(
        build(
            RuntimeResolvedCallOperandOrigin::Argument {
                argument: 0,
                slot: 0
            },
            None
        ),
        Err(RuntimeResolvedCallError::RequestRoleIdentity { position: 0 })
    );
    assert_eq!(
        build(
            RuntimeResolvedCallOperandOrigin::Receiver,
            Some(request_role_fixture())
        ),
        Err(RuntimeResolvedCallError::RequestRoleIdentity { position: 0 })
    );
    let identity = request_role_fixture();
    let admitted = build(
        RuntimeResolvedCallOperandOrigin::Argument {
            argument: 0,
            slot: 0,
        },
        Some(identity),
    )
    .unwrap();
    assert_eq!(
        admitted.operands()[0].request_role_identity(),
        Some(identity)
    );
}

#[test]
fn flow_invocation_projection_rejects_nonfunction_and_incomplete_arity() {
    use super::{RuntimeFlowFactError, RuntimeNormalizedType, RuntimeTypeShape};
    let project = project_fixture(
        "flow-invocation-contract",
        "flow root {}\nfn __runtime_plan_test_probe() -> Unit { () }\n",
    );
    let owner = flow_item(&project);
    let definition = accepted_flow_definition_fixture(&project, owner);
    let identity = FlowRuntimeId::canonical("root").unwrap();
    assert!(matches!(
        RuntimeFlowFact::try_new(identity.clone(), Arc::clone(&definition), unit_type()),
        Err(RuntimeFlowFactError::NotFunction)
    ));
    let extra_input = RuntimeNormalizedType::new(
        RuntimeSemanticTypeId::from_bytes([0x71; 32]),
        RuntimeTypeShape::Function {
            contract: arcweft_core::plan::RuntimeFunctionTypeContract::monomorphic(
                definition.effects().clone(),
            ),
            parameters: Box::new([unit_type()]),
            result: Box::new(unit_type()),
        },
    );
    assert!(matches!(
        RuntimeFlowFact::try_new(identity, definition, extra_input),
        Err(RuntimeFlowFactError::InputArity {
            expected: 0,
            actual: 1
        })
    ));
}

#[test]
fn closed_entity_value_constructor_rejects_foreign_owner_and_type_before_publication() {
    use super::project_function::{
        RuntimeProjectFunctionExpressionPayload, RuntimeProjectFunctionExpressionSemanticFact,
        RuntimeProjectFunctionFactError, RuntimeProjectFunctionInstanceSemanticFacts,
        RuntimeProjectFunctionTypeOwner, RuntimeProjectFunctionTypeProjection,
    };
    use arcweft_lang_sema::final_analysis::{
        CheckedExecutableRuntimeExpressionFactFamily, CheckedLocalUseAuthority,
    };
    let source = "fn selected() -> Ref<Asset> { @asset.bg.pulse }\n";
    let project = project_fixture("closed-entity-value-admission", source);
    let analysis = analyze_identity_fixture(&project);
    let owner = entity_reference(&project);
    let root = project
        .analysis_view()
        .expect("HIR")
        .items()
        .find(|item| {
            matches!(item.item().kind(), HirItemKind::Function(function)
            if function.name().resolved().is_some_and(|name| name.as_str() == "selected"))
        })
        .expect("actual function owner")
        .id();
    let reachability = runtime_reachability(&project);
    let partition = analysis
        .execution_projection()
        .runtime_fact_partition(&reachability, &HirRuntimeExecutableOwner::Item(root))
        .expect("final Sema seals the source body partition");
    assert!(
        partition.locals().is_empty()
            && partition.patterns().is_empty()
            && partition.statements().is_empty()
            && partition.captures().is_empty()
    );
    let entity = analysis
        .entity_value_projection(owner)
        .expect("accepted source address");
    let normalized = |identity| {
        super::RuntimeNormalizedType::new(
            RuntimeSemanticTypeId::from_bytes(identity),
            RuntimeTypeShape::EntityReference,
        )
    };
    let assemble = |value: arcweft_lang_sema::final_analysis::CheckedEntityValueProjection,
                    identity: [u8; 32]| {
        let mut types = partition
            .expressions()
            .iter()
            .map(|row| RuntimeProjectFunctionTypeProjection::Value {
                owner: RuntimeProjectFunctionTypeOwner::Expression(row.owner()),
                ty: normalized(identity),
            })
            .collect::<Vec<_>>();
        types.extend(partition.types().iter().map(|row| {
            if row.has_runtime_type() {
                RuntimeProjectFunctionTypeProjection::Value {
                    owner: RuntimeProjectFunctionTypeOwner::Type(row.owner()),
                    ty: normalized(
                        *analysis
                            .ty(row.owner())
                            .expect("accepted type")
                            .semantic_identity_digest()
                            .expect("type key")
                            .as_bytes(),
                    ),
                }
            } else {
                RuntimeProjectFunctionTypeProjection::SemanticOnlyType {
                    owner: row.owner(),
                    purpose: row.purpose().clone(),
                }
            }
        }));
        types.sort_by_key(RuntimeProjectFunctionTypeProjection::owner);
        let expressions = partition
            .expressions()
            .iter()
            .map(|row| {
                assert_eq!(
                    row.family(),
                    CheckedExecutableRuntimeExpressionFactFamily::Value,
                    "the source fixture owns only its admitted entity expression"
                );
                RuntimeProjectFunctionExpressionSemanticFact::new(
                    row.owner(),
                    row.children().into(),
                    RuntimeProjectFunctionExpressionPayload::Value(RuntimeResolvedValue::Entity(
                        value.clone(),
                    )),
                )
            })
            .collect::<Box<[_]>>();
        RuntimeProjectFunctionInstanceSemanticFacts::try_new(
            partition.clone(),
            CheckedLocalUseAuthority::Global(Arc::clone(analysis.checked_local_uses())),
            types.into_boxed_slice(),
            expressions,
            Box::new([]),
            Box::new([]),
            Box::new([]),
        )
    };
    assemble(entity.clone(), *entity.type_identity().as_bytes())
        .expect("actual source projection passes closed admission");
    let foreign = project_fixture("closed-entity-value-admission", source);
    let foreign_analysis = analyze_identity_fixture(&foreign);
    let foreign_value = foreign_analysis
        .entity_value_projection(entity_reference(&foreign))
        .expect("foreign actual source address");
    assert!(
        matches!(assemble(foreign_value, *entity.type_identity().as_bytes()),
        Err(RuntimeProjectFunctionFactError::InvalidEntityValueOrigin { expression }) if expression == owner)
    );
    let wrong_family =
        arcweft_lang_sema::types::TypeKind::entity_ref(arcweft_lang_sema::types::EntityKind::Flow)
            .semantic_identity_digest()
            .expect("other closed Ref");
    assert!(matches!(assemble(entity, *wrong_family.as_bytes()),
        Err(RuntimeProjectFunctionFactError::InvalidEntityValueType { expression }) if expression == owner));
}

fn checked_entity_value_input(
    project: &HirProject,
    analysis: &arcweft_lang_sema::final_analysis::FinalSemanticAnalysis,
    mut select: impl FnMut(
        arcweft_lang_hir::identity::ExprId,
        arcweft_lang_sema::final_analysis::CheckedEntityValueProjection,
    ) -> arcweft_lang_sema::final_analysis::CheckedEntityValueProjection,
) -> RuntimePlanSemanticFactInput {
    let mut input = complete_type_input(project);
    for (owner, row) in analysis.expressions() {
        if matches!(
            row.resolution(),
            arcweft_lang_sema::final_analysis::CheckedExpressionResolution::Value(
                arcweft_lang_sema::final_analysis::CheckedValueResolution::CatalogAsset(_)
                    | arcweft_lang_sema::final_analysis::CheckedValueResolution::ProjectItem(_)
            )
        ) {
            let entity = analysis
                .entity_value_projection(owner)
                .expect("accepted source entity projection");
            let ty = super::RuntimeNormalizedType::new(
                RuntimeSemanticTypeId::from_bytes(*entity.type_identity().as_bytes()),
                RuntimeTypeShape::EntityReference,
            );
            input
                .expression_facts
                .iter_mut()
                .find(|(row, _)| *row == owner)
                .expect("runtime expression owner")
                .1
                .ty = Some(ty);
            input.push_value(owner, RuntimeResolvedValue::Entity(select(owner, entity)));
        }
    }
    input
}

#[test]
fn entity_value_public_admission_retains_exact_source_asset_and_rejects_swapped_addresses_or_generation()
 {
    let source = "fn root(flag: bool) -> Ref<Asset> { if flag { @asset.bg.pulse } else { @asset.bg.poster } }\n";
    let project = project_fixture("entity-value-public-admission", source);
    let analysis = analyze_identity_fixture(&project);
    let entities = analysis
        .expressions()
        .filter_map(|(owner, row)| {
            matches!(
                row.resolution(),
                arcweft_lang_sema::final_analysis::CheckedExpressionResolution::Value(
                    arcweft_lang_sema::final_analysis::CheckedValueResolution::CatalogAsset(_)
                )
            )
            .then(|| {
                (
                    owner,
                    analysis
                        .entity_value_projection(owner)
                        .expect("sealed source address"),
                )
            })
        })
        .collect::<Vec<_>>();
    let [(first, pulse), (second, poster)] = entities.as_slice() else {
        panic!("two accepted Asset addresses")
    };
    assert_ne!(pulse.runtime_reference(), poster.runtime_reference());
    let facts = runtime_facts(
        &project,
        checked_entity_value_input(&project, &analysis, |_, value| value),
    )
    .expect("actual source values admitted");
    let Some(RuntimeResolvedValue::Entity(actual)) = facts.value(*first) else {
        panic!("sealed entity value retained")
    };
    assert_eq!(actual, pulse);
    assert_eq!(actual.origin().expression(), *first);
    assert_eq!(
        facts
            .expression_type(*first)
            .expect("Ref type")
            .identity()
            .as_bytes(),
        pulse.type_identity().as_bytes()
    );
    let swapped = checked_entity_value_input(&project, &analysis, |owner, value| {
        if owner == *first {
            poster.clone()
        } else {
            value
        }
    });
    assert_eq!(
        runtime_facts(&project, swapped).unwrap_err(),
        RuntimeSemanticFactsError::InvalidEntityValueOrigin { expression: *first }
    );
    let swapped_owner = checked_entity_value_input(&project, &analysis, |owner, value| {
        if owner == *second {
            pulse.clone()
        } else {
            value
        }
    });
    assert_eq!(
        runtime_facts(&project, swapped_owner).unwrap_err(),
        RuntimeSemanticFactsError::InvalidEntityValueOrigin {
            expression: *second
        }
    );
    let foreign = project_fixture("entity-value-public-admission", source);
    let foreign_analysis = analyze_identity_fixture(&foreign);
    let foreign_owner = entity_reference(&foreign);
    let foreign_value = foreign_analysis
        .entity_value_projection(foreign_owner)
        .expect("same source address in foreign allocation");
    let foreign_input = checked_entity_value_input(&project, &analysis, |owner, value| {
        if owner == *first {
            foreign_value.clone()
        } else {
            value
        }
    });
    assert_eq!(
        runtime_facts(&project, foreign_input).unwrap_err(),
        RuntimeSemanticFactsError::InvalidEntityValueOrigin { expression: *first }
    );
}

#[test]
fn entity_value_public_admission_rejects_wrong_ref_family_or_physical_shape_for_the_same_owner() {
    let project = project_fixture(
        "entity-value-type-admission",
        "fn root() -> Ref<Asset> { @asset.bg.pulse }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let owner = entity_reference(&project);
    let entity = analysis
        .entity_value_projection(owner)
        .expect("sealed Asset");
    let wrong_family =
        arcweft_lang_sema::types::TypeKind::entity_ref(arcweft_lang_sema::types::EntityKind::Flow)
            .semantic_identity_digest()
            .expect("closed Ref<Flow>");
    for ty in [
        super::RuntimeNormalizedType::new(
            RuntimeSemanticTypeId::from_bytes(*wrong_family.as_bytes()),
            RuntimeTypeShape::EntityReference,
        ),
        super::RuntimeNormalizedType::new(
            RuntimeSemanticTypeId::from_bytes(*entity.type_identity().as_bytes()),
            RuntimeTypeShape::Unit,
        ),
    ] {
        let mut input = checked_entity_value_input(&project, &analysis, |_, value| value);
        input
            .expression_facts
            .iter_mut()
            .find(|(row, _)| *row == owner)
            .expect("same expression owner")
            .1
            .ty = Some(ty);
        assert_eq!(
            runtime_facts(&project, input).unwrap_err(),
            RuntimeSemanticFactsError::InvalidEntityValueType { expression: owner }
        );
    }
}

#[test]
fn source_project_entity_values_use_the_same_sealed_admission_as_catalog_assets() {
    let project = project_fixture(
        "project-entity-value-admission",
        "pub character alice {}\nfn root() -> Ref<Character> { @character.alice }\n",
    );
    let analysis = analyze_identity_fixture(&project);
    let owner = entity_reference(&project);
    let projection = analysis
        .entity_value_projection(owner)
        .expect("accepted Character source value");
    assert!(
        matches!(projection.resolution(), arcweft_lang_sema::final_analysis::CheckedValueResolution::ProjectItem(item)
        if item.family() == arcweft_id::DeclarationIdentityFamily::Character)
    );
    let facts = runtime_facts(
        &project,
        checked_entity_value_input(&project, &analysis, |_, value| value),
    )
    .expect("retained source owner");
    assert_eq!(
        facts.value(owner),
        Some(&RuntimeResolvedValue::Entity(projection))
    );
}
