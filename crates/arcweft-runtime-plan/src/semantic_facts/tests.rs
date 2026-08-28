use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::entry::{RuntimeNominalTypeId, TypeLayoutHash};
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
    HirProject, HirProjectBuilder, HirProjectModule, HirRuntimeEmissionMode,
    HirRuntimeExecutableOwner, HirRuntimeExpressionTypeDisposition,
    HirRuntimeIteratorWitnessMethodRole, HirRuntimeReachabilityEdge,
    HirRuntimeReachabilityEdgeKind, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeReachabilitySite, HirRuntimeSemanticReachability,
    HirRuntimeSemanticReachabilityInput,
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
    RuntimeCheckedTypeProjectionError, RuntimeIteratorFact, RuntimeIteratorWitnessExecutableFact,
    RuntimeIteratorWitnessFact, RuntimeNormalizedVariantCase, RuntimePlanSemanticFactInput,
    RuntimePlanSemanticFacts, RuntimeRegisteredValueId, RuntimeResolvedNominal,
    RuntimeResolvedSelect, RuntimeResolvedValue, RuntimeResolvedVariant,
    RuntimeResolvedVariantError, RuntimeSemanticFactFamily, RuntimeSemanticFactsError,
    RuntimeSemanticOwnerSet, RuntimeSemanticTypeId, RuntimeSequenceKind, RuntimeTraitIdentity,
    RuntimeTraitMethodFact, RuntimeTypeProjectionStep, RuntimeTypeShape,
    RuntimeUnsupportedTypeShape, validate_iterator_witness_method_edges,
};

fn project_fixture(label: &str, source: &str) -> HirProject {
    let package = CallablePackageId::try_new(format!("runtime-plan-semantic-facts-{label}"))
        .expect("fixture package");
    let path = CanonicalModulePath::crate_root();
    let source_name = SourceName::path(format!("runtime-plan-semantic-facts-{label}.arcw"));
    let source =
        format!("{source}\nflow __runtime_plan_test_root {{ __runtime_plan_test_probe() }}\n");
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
    call_disposition: impl FnMut(
        arcweft_lang_hir::identity::ExprId,
    ) -> HirRuntimeExpressionTypeDisposition,
) -> HirRuntimeSemanticReachability<'_> {
    let executable = project.executable_view().expect("clean fixture");
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
        .runtime_semantic_reachability(input, &topology, selected_postfix, call_disposition)
        .expect("fixture reachability")
}

fn runtime_reachability(project: &HirProject) -> HirRuntimeSemanticReachability<'_> {
    runtime_reachability_with(
        project,
        |_| None,
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    )
}

fn runtime_facts(
    project: &HirProject,
    input: RuntimePlanSemanticFactInput,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticFactsError> {
    let executable = project.executable_view().expect("clean fixture");
    let reachability = runtime_reachability(project);
    RuntimePlanSemanticFacts::try_new(executable, &reachability, input)
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

fn complete_type_input(project: &HirProject) -> RuntimePlanSemanticFactInput {
    let mut input = RuntimePlanSemanticFactInput::new();
    let runtime_owners = runtime_reachability(project);
    for owner in runtime_owners.locals() {
        input.push_local_declaration(owner, unit_type());
    }
    for owner in runtime_owners.patterns() {
        input.push_pattern_type(owner, unit_type());
    }
    for owner in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(owner, unit_type());
    }
    input
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
        ),
    );
    let executable = project.executable_view().expect("assignment fixture");
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

    let resolved = assignment_nominal(&project, module, label);
    let identity = resolved.identity();
    let record_type = super::RuntimeNormalizedType::new(
        identity,
        RuntimeTypeShape::ProjectNominal {
            nominal: resolved.clone(),
            arguments: Box::new([]),
        },
    );
    let field_type = normalized_type(0x31, RuntimeTypeShape::Bool);
    let runtime_field = RuntimeRecordFieldId::try_from_zero_based_ordinal(1)
        .expect("assignment fixture field coordinate");
    let fact = RuntimeAssignmentFact::new(
        local,
        resolved.clone(),
        runtime_field,
        field_type.clone(),
        field_type.clone(),
    );
    let mut input = complete_type_input(&project);
    input
        .local_declarations
        .iter_mut()
        .find(|(owner, _)| *owner == local)
        .expect("assignment local type")
        .1 = record_type.clone();
    for (owner, ty) in &mut input.expression_types {
        if *owner == base {
            *ty = record_type.clone();
        } else if *owner == target || *owner == value {
            *ty = field_type.clone();
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
    (project, input, statement, extra_statement, fact)
}

fn assignment_nominal(
    project: &HirProject,
    module: &arcweft_lang_hir::module::HirModule,
    label: &str,
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
    RuntimeResolvedNominal::new(
        nominal.id().clone(),
        nominal.owner(),
        RuntimeNominalTypeId::try_new("test::assignment::Point")
            .expect("fixture runtime nominal identity"),
        RuntimeSemanticTypeId::from_bytes([0x91; 32]),
        TypeLayoutHash::from_bytes([0x92; 32]),
    )
}

fn local_owners(project: &HirProject) -> Vec<arcweft_lang_hir::identity::LocalId> {
    runtime_reachability(project).locals().collect()
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

    let facts = runtime_facts(&project, complete_type_input(&project))
        .expect("complete canonical local projection");

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
    assert_eq!(accepted.field().zero_based(), 1);
    assert_eq!(accepted.field_type(), accepted.value_type());

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
#[allow(
    clippy::too_many_lines,
    reason = "one mixed product fixture proves the runtime-domain gate and contiguous local projection across every representative HIR owner family"
)]
fn presentation_owned_facts_are_inactive_and_filtered_local_ids_remain_contiguous() {
    let project = project_fixture(
        "presentation-owner-domain",
        concat!(
            "fn before(first: bool) { let second: bool = first; second }\n",
            "#[tool.flag(1)]\n",
            "view Card(dialogue: DialogueView, count: i64 = 1i64) { Text(\"x\") }\n",
            "#[tool.flag(2)]\n",
            "style Theme {\n",
            "    token color.text: Color = white\n",
            "    Button { color = rgba(10, 20, 30, 255) }\n",
            "    when environment(color-scheme == dark) { Button { color = red } }\n",
            "}\n",
            "fn after(third: bool) { third }\n",
        ),
    );
    let executable = project.executable_view().expect("executable fixture");
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
    input.push_local_declaration(presentation_local, unit_type());
    assert_eq!(
        runtime_facts(&project, input)
            .expect_err("a presentation local cannot extend the runtime domain"),
        RuntimeSemanticFactsError::ExtraLocalDeclaration {
            local: presentation_local,
        }
    );

    let mut input = complete_type_input(&project);
    input.push_expression_type(presentation_literal, unit_type());
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
        "fn root(first: bool, second: bool) { first }\n",
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
    extra.push_local_declaration(foreign_owner, unit_type());
    assert_eq!(
        runtime_facts(&project, extra).expect_err("a foreign local cannot extend the plan domain"),
        RuntimeSemanticFactsError::ExtraLocalDeclaration {
            local: foreign_owner,
        }
    );

    let mut duplicate = complete_type_input(&project);
    duplicate.push_local_declaration(owners[0], unit_type());
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

    let facts = runtime_facts(&project, input).expect("literal fact");
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

    let facts = runtime_facts(&project, input).expect("Flow identity fact");
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
        "fn root() {\n    let line: Ref<DialogueLine> = @say.story.greeting\n}\n",
    );
    let owner = entity_reference(&project);
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
        "fn root(value: bool) {\n    match value { true => (), false => () }\n}\n",
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
    let facts = runtime_facts(&project, complete_type_input(&project))
        .expect("effect metadata requires no runtime expression type");
    assert!(facts.expression_type(effect).is_none());
    assert_eq!(facts.expression_type(body), Some(&unit_type()));

    let mut input = complete_type_input(&project);
    input.push_expression_type(effect, unit_type());
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
        "fn root(value: bool) {\n    match value { true => (), false => () }\n}\n",
    );
    let mut input = RuntimePlanSemanticFactInput::new();
    let runtime_owners = runtime_reachability(&project);
    for owner in runtime_owners.locals() {
        input.push_local_declaration(owner, unit_type());
    }
    for owner in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(owner, unit_type());
    }
    let pattern = runtime_owners.patterns().next().expect("pattern fixture");

    assert_eq!(
        runtime_facts(&project, input).expect_err("an accepted pattern cannot omit its type"),
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
            RuntimeTypeShape::Need(boxed_unit_type()),
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
            normalized
                .runtime_plan_type_seed()
                .map(|seed| seed.projection().operational_type()),
            Ok(Some(operational))
        );
    }
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
            RuntimeAgentTypeShape::DataFormat,
            RuntimeAgentOperationalType::DataFormat,
        ),
        (
            RuntimeAgentTypeShape::DataShape,
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
            RuntimeAgentTypeShape::CaptureFormat,
            RuntimeAgentOperationalType::CaptureFormat,
        ),
        (
            RuntimeAgentTypeShape::CaptureKind,
            RuntimeAgentOperationalType::CaptureKind,
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
            RuntimeAgentTypeShape::PointerButton,
            RuntimeAgentOperationalType::PointerButton,
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
    ];

    for (index, (shape, operational)) in cases.into_iter().enumerate() {
        let marker = u8::try_from(index + 1).expect("bounded Agent type fixture");
        let identity = RuntimeSemanticTypeId::from_bytes([marker; 32]);
        let normalized = normalized_type(marker, RuntimeTypeShape::Agent(shape));
        assert_eq!(
            normalized.checked_type(),
            Err(RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
                semantic_identity: identity,
                path: super::RuntimeTypeProjectionPath::root(),
                shape: RuntimeUnsupportedTypeShape::Agent(operational),
            })
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
        input.push_local_declaration(local, nested.clone());
    }
    for pattern in runtime_owners.patterns() {
        input.push_pattern_type(pattern, nested.clone());
    }
    for expression in runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime expression-type fixture")
    {
        input.push_expression_type(expression, nested.clone());
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
    let runtime_owners = runtime_reachability_with(
        &project,
        |owner| postfix_candidates.get(&owner).copied(),
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    );
    let accepted = runtime_owners
        .selected_expression_type_owners()
        .expect("selected runtime expression-type inventory");
    assert!(!accepted.contains(&postfix_owner));
    assert!(accepted.contains(&target));
    assert!(!accepted.contains(&dialogue));
    assert!(!accepted.contains(&index));

    let complete_selected_input = || {
        let mut input = RuntimePlanSemanticFactInput::new();
        for owner in runtime_owners.locals() {
            input.push_local_declaration(owner, unit_type());
        }
        for owner in &accepted {
            input.push_expression_type(*owner, unit_type());
        }
        for owner in runtime_owners.patterns() {
            input.push_pattern_type(owner, unit_type());
        }
        input.push_postfix_candidate(postfix_owner, dialogue);
        input
    };
    let facts =
        RuntimePlanSemanticFacts::try_new(executable, &runtime_owners, complete_selected_input())
            .expect("the rolled-back expression candidate needs no type fact");
    assert!(facts.expression_type(postfix_owner).is_none());
    assert!(facts.expression_type(target).is_some());
    assert!(facts.expression_type(dialogue).is_none());
    assert!(facts.expression_type(index).is_none());

    let mut input = complete_selected_input();
    input.push_expression_type(dialogue, unit_type());
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, &runtime_owners, input)
            .expect_err("a selected dialogue carrier cannot publish an expression type"),
        RuntimeSemanticFactsError::InactiveExpressionFact {
            expression: dialogue,
            family: RuntimeSemanticFactFamily::ExpressionType,
        },
    );

    let mut input = complete_selected_input();
    input.push_expression_type(index, unit_type());
    assert_eq!(
        RuntimePlanSemanticFacts::try_new(executable, &runtime_owners, input)
            .expect_err("an unselected candidate cannot publish an expression type"),
        RuntimeSemanticFactsError::InactiveExpressionFact {
            expression: index,
            family: RuntimeSemanticFactFamily::ExpressionType,
        },
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

#[derive(Clone)]
struct IteratorMethodFixture {
    implementation: arcweft_lang_hir::identity::ItemId,
    member: u16,
    declaration: ImplMethodDeclarationId,
}

fn iterator_fixture_symbols(project: &HirProject) -> ProjectSymbolTable {
    let executable = project.executable_view().expect("clean iterator fixture");
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
    let executable = project.executable_view().expect("iterator edge fixture");
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
    let executable = project.executable_view().expect("clean iterator fixture");
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
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        )
        .expect("accepted iterator reachability")
}

fn iterator_method_fact(
    method: &IteratorMethodFixture,
    trait_identity: RuntimeTraitIdentity,
) -> RuntimeTraitMethodFact {
    RuntimeTraitMethodFact::new(
        method.declaration.clone(),
        method.implementation,
        method.member,
        trait_identity,
        unit_type(),
    )
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
    methods: &BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodFact>,
) -> RuntimeSemanticFactsError {
    let reachability = iterator_reachability_with_edges(project, edges);
    validate_iterator_witness_method_edges(
        RuntimeSemanticOwnerSet::runtime_only(&reachability),
        statement,
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
            "flow iterator_edge_root() {\n",
            "    let counter = Counter { end: 1 }\n",
            "    for value in counter { value }\n",
            "}\n",
        ),
    );
    let statement = project
        .executable_view()
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
    let methods = BTreeMap::from([
        (
            into_iter.declaration.clone(),
            iterator_method_fact(&into_iter, RuntimeTraitIdentity::StandardIntoIterator),
        ),
        (
            next.declaration.clone(),
            iterator_method_fact(&next, RuntimeTraitIdentity::StandardIterator),
        ),
    ]);
    let identity = identity_iterator_fact(&next);
    let trait_calls = trait_call_iterator_fact(&into_iter, &next);

    let accepted_identity = iterator_reachability_with_edges(&project, vec![next_edge()]);
    assert_eq!(
        validate_iterator_witness_method_edges(
            RuntimeSemanticOwnerSet::runtime_only(&accepted_identity),
            statement,
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
    wrong_implementation.insert(
        next.declaration.clone(),
        RuntimeTraitMethodFact::new(
            next.declaration.clone(),
            other_next.implementation,
            next.member,
            RuntimeTraitIdentity::StandardIterator,
            unit_type(),
        ),
    );
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
    wrong_member.insert(
        next.declaration.clone(),
        RuntimeTraitMethodFact::new(
            next.declaration.clone(),
            next.implementation,
            next.member
                .checked_add(1)
                .expect("fixture member coordinate"),
            RuntimeTraitIdentity::StandardIterator,
            unit_type(),
        ),
    );
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
    wrong_trait.insert(
        next.declaration.clone(),
        iterator_method_fact(&next, RuntimeTraitIdentity::StandardIntoIterator),
    );
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
