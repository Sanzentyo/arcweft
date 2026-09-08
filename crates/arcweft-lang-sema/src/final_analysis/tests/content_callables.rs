use crate::{
    callable::{
        CallableAttachedContentExecution, CallableAttachedContentPolicy, CallableParameterPresence,
        CheckedAttachedContentAdmission, CheckedCallAttachedContentOperand,
        CheckedCallRuntimeOperand, CheckedContentRole,
    },
    checked_rich_text::{CheckedContentEmission, CheckedContentInsertion, CheckedDialogueToken},
    final_analysis::{
        CheckedCompileTimeValue, CheckedContentFxApplication, CheckedContentFxBinding,
        CheckedExpressionResolution, CheckedFxBindingDecision, CheckedFxConstructorArgumentValue,
        CheckedFxDefinitionRef, CheckedFxGraphExpression, CheckedFxSourceParameter,
        CheckedFxSymbolicValue, CheckedProjectFxDefinition, FinalSemanticAnalysis,
    },
    semantic_coordinate::{SemanticCoordinateIndex, StableCheckedValueCoordinate},
};
use arcweft_presentation::{
    fx::{BuiltinFxCallableRowId, BuiltinFxParameterId},
    rich_text::{
        PresentationContentCallableDefinitionId, PresentationContentCallableParameterId,
        RichTextStyleSelector,
    },
};

use super::{analyze, fixture, fixture_registration_error};

fn content_insertions(report: &FinalSemanticAnalysis) -> Vec<&CheckedContentInsertion> {
    fn visit<'a>(
        tokens: &'a [CheckedDialogueToken],
        insertions: &mut Vec<&'a CheckedContentInsertion>,
    ) {
        for token in tokens {
            let CheckedDialogueToken::ContentInsert(insertion) = token else {
                continue;
            };
            insertions.push(insertion);
            if let Some(content) = insertion.argument().checked_content() {
                visit(content.content().tokens(), insertions);
            }
        }
    }

    let mut insertions = Vec::new();
    for (_, expression) in report.expressions() {
        if let CheckedExpressionResolution::DialogueApplication { rich_text, .. } =
            expression.resolution()
        {
            visit(rich_text.content().tokens(), &mut insertions);
        }
    }
    insertions
}

#[test]
fn presentation_content_calls_publish_typed_emissions_and_operands() {
    let fixture = fixture(
        r#"
pub character alice {}

fn opening() {
    alice[#strong()[#style(.italic)[#fx(wave(amplitude=1px, direction=vec2(0.0, 1.0), phase=.glyph_transform))[#ruby("reading")[base]]]]#raw()[raw[p]literal]];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("typed presentation content calls");
    let insertions = content_insertions(&report);
    assert_eq!(insertions.len(), 5);

    let [strong, style, fx, ruby, raw] = insertions.as_slice() else {
        panic!("expected one insertion for every presentation family");
    };
    let owners = insertions
        .iter()
        .map(|insertion| insertion.site().raw())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(owners.len(), 5, "each nested call has a distinct HIR site");
    for insertion in &insertions {
        let owner = insertion.site().raw();
        let call = report
            .calls()
            .find(|(expression, _)| *expression == owner)
            .map(|(_, call)| call)
            .expect("each content insertion has a final call fact");
        let application = call
            .selected_application()
            .expect("each content insertion selects one call application");
        assert_eq!(
            application.core().site(),
            crate::callable::CheckedCallSite::AttachedContentApplication {
                expression: owner,
                family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
            }
        );
        assert_eq!(
            application
                .core()
                .candidates()
                .selected()
                .schema()
                .attached_content()
                .map(crate::callable::CallableAttachedContentParameter::execution),
            Some(CallableAttachedContentExecution::Structural),
        );
        let Some(CheckedCallAttachedContentOperand::StructuralPresent { source }) =
            application.core().execution().attached_content()
        else {
            panic!("each body-bearing content call seals one attached operand");
        };
        assert_eq!(source.raw().owner(), owner);
        assert_eq!(source.application(), application.core().stable_site());
    }
    assert_eq!(
        strong.argument().admission(),
        Some(CheckedAttachedContentAdmission::Role(
            CheckedContentRole::Dialogue,
        )),
    );
    assert_eq!(
        style.argument().admission(),
        Some(CheckedAttachedContentAdmission::Role(
            CheckedContentRole::Dialogue,
        )),
    );
    assert_eq!(
        fx.argument().admission(),
        Some(CheckedAttachedContentAdmission::Role(
            CheckedContentRole::Dialogue,
        )),
    );
    assert_eq!(
        ruby.argument().admission(),
        Some(CheckedAttachedContentAdmission::Role(
            CheckedContentRole::Inline,
        )),
    );
    assert_eq!(
        raw.argument().admission(),
        Some(CheckedAttachedContentAdmission::Literal),
    );
    let CheckedContentEmission::Modifier(strong) = strong.emission() else {
        panic!("strong must publish a modifier emission");
    };
    assert_eq!(
        strong.definition(),
        PresentationContentCallableDefinitionId::Strong
    );
    assert!(strong.parameters().is_empty());

    let CheckedContentEmission::Modifier(style) = style.emission() else {
        panic!("style must publish a modifier emission");
    };
    assert_eq!(
        style.definition(),
        PresentationContentCallableDefinitionId::Style(RichTextStyleSelector::Italic)
    );
    assert!(matches!(
        style.parameters(),
        [parameter]
            if parameter.id() == PresentationContentCallableParameterId::Selector
                && matches!(parameter.value(), CheckedCompileTimeValue::Enum(value)
                    if value.variant() == RichTextStyleSelector::Italic.ordinal())
    ));

    let CheckedContentEmission::Fx(fx) = fx.emission() else {
        panic!("fx must publish a modifier emission");
    };
    assert_builtin_row(fx, BuiltinFxCallableRowId::WaveGlyphTransform);

    let CheckedContentEmission::Ruby(ruby) = ruby.emission() else {
        panic!("ruby must publish a ruby emission");
    };
    assert_eq!(ruby.reading(), "reading");

    let CheckedContentEmission::Raw(raw) = raw.emission() else {
        panic!("raw must publish a raw emission");
    };
    assert_eq!(raw.body(), "raw[p]literal");
}

#[test]
fn checked_content_reports_publish_report_local_stable_fragment_coordinates() {
    let fixture = fixture(
        r#"
pub character alice {}

fn opening() {
    alice[root #strong()[nested]];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("checked content fragment coordinates");
    let (_, expression) = report
        .expressions()
        .find(|(_, expression)| {
            matches!(
                expression.resolution(),
                CheckedExpressionResolution::DialogueApplication { .. }
            )
        })
        .expect("one checked dialogue application");
    let CheckedExpressionResolution::DialogueApplication { rich_text, .. } =
        expression.resolution()
    else {
        unreachable!("dialogue application resolution was selected")
    };
    assert!(rich_text.fragment_coordinate().path().segments().is_empty());
    let index = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    assert_eq!(
        rich_text.fragment_coordinate().owner(),
        &StableCheckedValueCoordinate::Expression(
            index
                .expression(rich_text.content().id().owner())
                .expect("dialogue root has one accepted semantic coordinate"),
        )
    );

    let insertions = content_insertions(&report);
    let [insertion] = insertions.as_slice() else {
        panic!("one structural insertion")
    };
    assert!(matches!(
        insertion.emission(),
        CheckedContentEmission::Modifier(_)
    ));
    assert_eq!(
        insertion.fragment_coordinate(),
        rich_text.fragment_coordinate()
    );
    let nested = insertion
        .argument()
        .checked_content()
        .expect("structural insertion carries its checked body");
    assert_eq!(
        nested.fragment_coordinate(),
        rich_text.fragment_coordinate()
    );
    assert_eq!(
        nested
            .fragment_coordinate()
            .semantic_digest()
            .expect("nested structural fragment identity"),
        rich_text
            .fragment_coordinate()
            .semantic_digest()
            .expect("dialogue root fragment identity"),
    );
}

#[test]
fn project_runtime_content_call_seals_terminal_abi_and_child_fragment() {
    let fixture = fixture(
        r#"
pub character alice {}

fn passthrough()[body: DialogueContent] -> DialogueContent {
    body
}

fn opening() {
    alice[root #passthrough()[nested]];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("project runtime attached content");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let static_callee = module
        .expressions()
        .find_map(|(_, expression)| {
            let arcweft_lang_hir::expr::HirExprKind::AttachedContentApplication(application) =
                expression.kind()
            else {
                return None;
            };
            let arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::ContentCall {
                invocation,
                ..
            } = application.family()
            else {
                return None;
            };
            invocation.callee().value_expression()
        })
        .expect("runtime content call static callee");
    assert!(
        report.expression(static_callee).is_none(),
        "static content callees are owned by the attached call site, not the selected expression graph"
    );
    let (item_id, binding) = module
        .items()
        .find_map(|(item_id, item)| {
            let arcweft_lang_hir::item::HirItemKind::Function(function) = item.kind() else {
                return None;
            };
            (function.name().resolved().map(|name| name.as_str()) == Some("passthrough")).then(
                || {
                    (
                        item_id,
                        function
                            .attached_content()
                            .expect("passthrough attached declaration")
                            .binding(),
                    )
                },
            )
        })
        .expect("passthrough function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item_id)
        .map(|symbol| symbol.declaration().clone())
        .expect("passthrough callable declaration");
    let facts = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked passthrough callable");
    let attached = facts
        .signature()
        .attached_content()
        .expect("project schema publishes attached content");
    assert_eq!(
        attached.group(),
        facts.signature().groups().last().unwrap().index()
    );
    assert_eq!(attached.presence(), CallableParameterPresence::Required);
    assert_eq!(
        attached.policy(),
        CallableAttachedContentPolicy::Declared(CheckedContentRole::Dialogue)
    );
    assert_eq!(
        attached.execution(),
        CallableAttachedContentExecution::RuntimeContent
    );
    let checked_attached = facts
        .attached_content()
        .expect("final callable interface owns the attached ABI row");
    assert_eq!(checked_attached.binding(), binding);
    assert_eq!(
        checked_attached.presence(),
        CallableParameterPresence::Required
    );
    assert_eq!(checked_attached.abi_position(), 0);
    assert!(checked_attached.default().is_none());
    let dialogue_content = fixture
        .registered
        .environment()
        .typecheck_env()
        .standard_dialogue_content_type()
        .expect("standard DialogueContent type");
    assert_eq!(
        report
            .local(binding)
            .expect("checked attached binding")
            .ty(),
        &dialogue_content
    );
    assert_eq!(checked_attached.binding_type(), &dialogue_content);
    assert_eq!(checked_attached.abi_type(), &dialogue_content);

    let insertions = content_insertions(&report);
    let [insertion] = insertions.as_slice() else {
        panic!("one project ContentResult insertion")
    };
    assert!(matches!(
        insertion.emission(),
        CheckedContentEmission::ContentResult
    ));
    let nested = insertion
        .argument()
        .checked_content()
        .expect("required runtime body is checked once");
    assert_eq!(
        insertion.fragment_coordinate(),
        nested.fragment_coordinate()
    );
    assert_eq!(
        insertion.fragment_coordinate().path().segments()[0].content_result_ordinal(),
        0
    );

    let call = report
        .call(insertion.site().raw())
        .and_then(crate::callable::CallTargetFacts::selected_application)
        .expect("selected project content call");
    let Some(CheckedCallAttachedContentOperand::RuntimePresent {
        source,
        abi_position,
    }) = call.core().execution().attached_content()
    else {
        panic!("project body seals one runtime-present operand")
    };
    assert_eq!(*abi_position, 0);
    assert_eq!(source.raw(), nested.content().id());
    let runtime_operands = call.core().runtime_operands();
    let [
        CheckedCallRuntimeOperand::AttachedContent {
            source: Some(runtime_source),
            presence,
            ty,
            abi_position: runtime_position,
        },
    ] = runtime_operands.as_ref()
    else {
        panic!("attached content is the sole ABI operand")
    };
    assert_eq!(*runtime_source, source);
    assert_eq!(*presence, CallableParameterPresence::Required);
    assert_eq!(*ty, &dialogue_content);
    assert_eq!(*runtime_position, *abi_position);
}

#[test]
fn defaulted_project_content_seals_an_acyclic_interface_row() {
    let fixture = fixture(
        r#"
pub character alice {}

fn fallback()[body: DialogueContent = fallback()] -> DialogueContent {
    body
}

fn opening() {
    alice[#fallback()];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("self-recursive attached default is finite");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let (item_id, binding, default_source) = module
        .items()
        .find_map(|(item_id, item)| {
            let arcweft_lang_hir::item::HirItemKind::Function(function) = item.kind() else {
                return None;
            };
            if function.name().resolved().map(|name| name.as_str()) != Some("fallback") {
                return None;
            }
            let attached = function
                .attached_content()
                .expect("fallback attached declaration");
            Some((
                item_id,
                attached.binding(),
                attached
                    .presence()
                    .default_value()
                    .expect("fallback default source"),
            ))
        })
        .expect("fallback function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item_id)
        .map(|symbol| symbol.declaration().clone())
        .expect("fallback callable declaration");
    let facts = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked fallback callable");
    let attached = facts
        .attached_content()
        .expect("defaulted callable interface row");
    assert_eq!(attached.binding(), binding);
    assert_eq!(attached.presence(), CallableParameterPresence::Defaulted);
    let default = attached.default().expect("checked default row");
    assert_eq!(default.source(), default_source);
    assert!(default.effects().concrete().is_empty());
    assert_eq!(
        default.suspension(),
        crate::final_analysis::CheckedSuspensionRole::NonSuspending,
        "a recursion SCC with no direct suspension remains non-suspending"
    );
    assert_eq!(
        default.control(),
        crate::final_analysis::CheckedExecutableControlRole::FlowRequired,
        "the recursive ProjectCall still requires Flow control independently of suspension"
    );
    assert_ne!(default.expression().as_bytes(), &[0; 32]);
    assert_ne!(facts.interface_digest().as_bytes(), &[0; 32]);
    let default_call = report
        .call(default_source)
        .and_then(crate::callable::CallTargetFacts::selected_application)
        .expect("default call seals against the callable resolution draft");
    assert!(matches!(
        default_call.core().execution().attached_content(),
        Some(CheckedCallAttachedContentOperand::RuntimeOmitted { .. })
    ));
}

#[test]
fn attached_default_captures_the_exact_logical_parameter_binding_row() {
    let fixture = fixture(
        r#"
fn fallback(first: DialogueContent)[body: DialogueContent = first] -> DialogueContent {
    body
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("parameter-reading attached default is checked");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let (item, function) = module
        .items()
        .find_map(|(item, hir)| match hir.kind() {
            arcweft_lang_hir::item::HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("fallback") =>
            {
                Some((item, function))
            }
            _ => None,
        })
        .expect("fallback function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item)
        .map(|symbol| symbol.declaration().clone())
        .expect("fallback declaration");
    let default = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked fallback")
        .attached_content()
        .and_then(|attached| attached.default())
        .expect("checked attached default");
    let [capture] = default.captures() else {
        panic!("one logical parameter capture")
    };
    let parameter = &function.parameter_groups()[0].parameters()[0];
    assert_eq!(capture.parameter().group().get(), 0);
    assert_eq!(capture.parameter().parameter().get(), 0);
    assert_eq!(capture.pattern(), parameter.pattern());
    assert_ne!(capture.pattern_digest().as_bytes(), &[0; 32]);
    assert_eq!(capture.bindings(), parameter.locals());
    assert_eq!(capture.binding_evidence().len(), parameter.locals().len());
    assert_eq!(capture.used_locals().len(), 1);
    assert_eq!(
        capture.used_locals()[0].local(),
        *parameter.locals().first().expect("first binding")
    );
    assert_eq!(
        default.suspension(),
        crate::final_analysis::CheckedSuspensionRole::NonSuspending
    );
}

#[test]
fn attached_default_capture_retains_the_complete_destructuring_binding_row() {
    let fixture = fixture(
        r#"
fn choose(first: DialogueContent, second: DialogueContent) -> DialogueContent { first }

fn fallback((first, second): (DialogueContent, DialogueContent))[body: DialogueContent = choose(second, first)] -> DialogueContent {
    body
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("destructuring attached default is checked");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let (item, function) = module
        .items()
        .find_map(|(item, hir)| match hir.kind() {
            arcweft_lang_hir::item::HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("fallback") =>
            {
                Some((item, function))
            }
            _ => None,
        })
        .expect("fallback function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item)
        .map(|symbol| symbol.declaration().clone())
        .expect("fallback declaration");
    let default = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked fallback")
        .attached_content()
        .and_then(|attached| attached.default())
        .expect("checked attached default");
    let [capture] = default.captures() else {
        panic!("one logical destructuring capture")
    };
    let parameter = &function.parameter_groups()[0].parameters()[0];
    assert_eq!(parameter.locals().len(), 2);
    assert_eq!(capture.pattern(), parameter.pattern());
    assert_eq!(capture.bindings(), parameter.locals());
    assert_eq!(capture.binding_evidence().len(), 2);
    assert_eq!(capture.used_locals().len(), 2);
    assert_eq!(
        capture
            .used_locals()
            .iter()
            .map(|local| local.local())
            .collect::<Vec<_>>(),
        parameter.locals(),
        "used locals are canonicalized by checked binding origin, not reference traversal order"
    );
    for evidence in capture.binding_evidence() {
        assert_eq!(
            report.local(evidence.local()).map(|local| local.ty()),
            Some(evidence.ty()),
            "each child binding retains its independently checked component type"
        );
    }
}

#[test]
fn attached_default_captures_prefix_and_current_group_parameters() {
    let fixture = fixture(
        r#"
fn choose(first: DialogueContent, second: DialogueContent) -> DialogueContent { first }

fn fallback(prefix: DialogueContent)(current: DialogueContent)[body: DialogueContent = choose(prefix, current)] -> DialogueContent {
    body
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("prefix/current attached-default capture analysis");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let item = module
        .items()
        .find_map(|(item, hir)| match hir.kind() {
            arcweft_lang_hir::item::HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("fallback") =>
            {
                Some(item)
            }
            _ => None,
        })
        .expect("fallback function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item)
        .map(|symbol| symbol.declaration().clone())
        .expect("fallback declaration");
    let default = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked fallback")
        .attached_content()
        .and_then(|attached| attached.default())
        .expect("checked attached default");

    let [prefix, current] = default.captures() else {
        panic!("one prefix and one current-group capture")
    };
    assert_eq!(
        (
            prefix.parameter().group().get(),
            prefix.parameter().parameter().get()
        ),
        (0, 0)
    );
    assert_eq!(
        (
            current.parameter().group().get(),
            current.parameter().parameter().get()
        ),
        (1, 0)
    );
    assert_eq!(prefix.used_locals().len(), 1);
    assert_eq!(current.used_locals().len(), 1);
}

#[test]
fn attached_default_rest_capture_uses_vec_binding_type() {
    let fixture = fixture(
        r#"
fn choose_rest(items: Vec<DialogueContent>, seed: DialogueContent) -> DialogueContent { seed }

fn fallback(seed: DialogueContent, items: ...DialogueContent)[body: DialogueContent = choose_rest(items, seed)] -> DialogueContent {
    body
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("rest attached-default capture analysis");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let item = module
        .items()
        .find_map(|(item, hir)| match hir.kind() {
            arcweft_lang_hir::item::HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("fallback") =>
            {
                Some(item)
            }
            _ => None,
        })
        .expect("fallback function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item)
        .map(|symbol| symbol.declaration().clone())
        .expect("fallback declaration");
    let default = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked fallback")
        .attached_content()
        .and_then(|attached| attached.default())
        .expect("checked attached default");

    let rest = default
        .captures()
        .iter()
        .find(|capture| capture.parameter().parameter().get() == 1)
        .expect("rest logical capture");
    assert!(matches!(
        rest.binding_type(),
        crate::types::TypeKind::Vec(_)
    ));
    assert_eq!(rest.binding_evidence().len(), 1);
    assert!(matches!(
        rest.binding_evidence()[0].ty(),
        crate::types::TypeKind::Vec(_)
    ));
}

#[test]
fn attached_default_rejects_a_free_nonparameter_name_upstream() {
    let fixture = fixture(
        r#"
fn fallback()[body: DialogueContent = missing] -> DialogueContent {
    body
}
"#,
        None,
    );
    assert!(
        analyze(&fixture).is_err(),
        "an attached default cannot publish a capture for an unresolved or nonparameter free local"
    );
}

#[test]
fn ordinary_project_parameter_default_is_rejected_before_runtime_materialization() {
    const SOURCE: &str = r#"
fn invalid(value: i64 = 1i64) -> i64 { value }
"#;
    let report = fixture_registration_error(SOURCE);
    let [diagnostic] = report.diagnostics() else {
        panic!("ordinary parameter default must have one typed registration rejection")
    };
    assert!(matches!(
        diagnostic.kind(),
        crate::registration::CharacterRegistrationDiagnosticKind::CallableCatalog {
            code: crate::callable::CallableDiagnosticCode::UnsupportedProjectParameterDefault,
        }
    ));
    assert_eq!(
        &SOURCE[diagnostic.primary().range().as_range()],
        "1i64",
        "the callable owner must retain the authored default-expression source"
    );
}

#[test]
fn optional_project_content_seals_one_option_abi_for_presence_and_omission() {
    let fixture = fixture(
        r#"
pub character alice {}

fn maybe()[body?: DialogueContent] -> DialogueContent {
    match body {
        .Some(content) => content
        .None => panic("missing optional content")
    }
}

fn opening() {
    alice[#maybe()];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("optional attached content");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    let (item_id, binding) = module
        .items()
        .find_map(|(item_id, item)| {
            let arcweft_lang_hir::item::HirItemKind::Function(function) = item.kind() else {
                return None;
            };
            (function.name().resolved().map(|name| name.as_str()) == Some("maybe")).then(|| {
                (
                    item_id,
                    function
                        .attached_content()
                        .expect("maybe attached declaration")
                        .binding(),
                )
            })
        })
        .expect("maybe function");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == item_id)
        .map(|symbol| symbol.declaration().clone())
        .expect("maybe callable declaration");
    let facts = report
        .checked_callables()
        .project_callable(&declaration)
        .expect("checked maybe callable");
    let attached = facts
        .attached_content()
        .expect("optional callable interface row");
    assert_eq!(attached.binding(), binding);
    assert_eq!(attached.presence(), CallableParameterPresence::Optional);
    assert!(attached.default().is_none());
    assert_eq!(attached.binding_type(), attached.abi_type());
    assert!(matches!(
        attached.abi_type(),
        crate::types::TypeKind::Option(_)
    ));
    let omitted = report.calls().find_map(|(_, call)| {
        let application = call.selected_application()?;
        matches!(
            application.core().candidates().selected().origin(),
            crate::callable::ResolvedCallableOrigin::Project {
                declaration: selected,
                ..
            } if selected == &declaration
        )
        .then_some(application)
    });
    assert!(matches!(
        omitted.and_then(|application| application.core().execution().attached_content()),
        Some(CheckedCallAttachedContentOperand::RuntimeOmitted { .. })
    ));
}

#[test]
fn inline_attached_content_rejects_dialogue_controls() {
    let fixture = fixture(
        r#"
pub character alice {}

fn opening() {
    alice[#ruby("reading")[base[p]]];
}
"#,
        None,
    );
    assert!(
        analyze(&fixture).is_err(),
        "Ruby's Inline admission must reject a page control",
    );
}

#[test]
fn fx_callable_omitted_phase_selects_the_default_phase_row() {
    let fixture = fixture(
        r#"
pub character alice {}

fn opening() {
    alice[#fx(shake(amplitude=1px))[shake]];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("defaulted Fx phase");
    let insertions = content_insertions(&report);
    let [insertion] = insertions.as_slice() else {
        panic!("expected one Fx insertion");
    };
    let CheckedContentEmission::Fx(fx) = insertion.emission() else {
        panic!("Fx must publish a modifier emission");
    };
    assert_builtin_row(fx, BuiltinFxCallableRowId::ShakeGlyphTransform);
}

fn typewriter_fixture(arguments: &str) -> super::Fixture {
    let source = format!(
        r#"
pub character alice {{}}

fn opening() {{
    alice[#fx(typewriter({arguments}))[text]];
}}
"#
    );
    fixture(&source, None)
}

fn typewriter_effect(report: &FinalSemanticAnalysis) -> &CheckedContentFxApplication {
    let insertions = content_insertions(report);
    let [insertion] = insertions.as_slice() else {
        panic!("expected one typewriter insertion");
    };
    let CheckedContentEmission::Fx(application) = insertion.emission() else {
        panic!("typewriter must publish an Fx emission");
    };
    application
}

fn assert_builtin_row(application: &CheckedContentFxApplication, expected: BuiltinFxCallableRowId) {
    assert!(matches!(
        application.definition(),
        CheckedFxDefinitionRef::Builtin { row, .. } if *row == expected
    ));
}

fn fx_decision(
    application: &CheckedContentFxApplication,
    expected: BuiltinFxParameterId,
) -> &CheckedFxBindingDecision<CheckedContentFxBinding> {
    application
        .arguments()
        .iter()
        .find_map(|argument| {
            matches!(
                argument.parameter(),
                CheckedFxSourceParameter::Builtin(parameter) if parameter == expected
            )
            .then(|| argument.decision())
        })
        .expect("builtin Fx parameter decision")
}

#[test]
fn typewriter_conditional_cursor_alpha_follows_typed_predicate() {
    let cursor_false = analyze(&typewriter_fixture(
        "characters_per_second=20, cursor=false",
    ))
    .expect("false cursor without inactive alpha");
    let false_effect = typewriter_effect(&cursor_false);
    assert_builtin_row(false_effect, BuiltinFxCallableRowId::TypewriterGlyphMask);
    assert!(matches!(
        fx_decision(false_effect, BuiltinFxParameterId::Cursor),
        CheckedFxBindingDecision::Explicit(binding)
            if matches!(binding, CheckedContentFxBinding::Abi(
                arcweft_presentation::fx::FxDefinitionArgumentValue::Runtime(
                    arcweft_presentation::fx::FxRuntimeValue::Bool(false)
                )
            ))
    ));
    assert!(matches!(
        fx_decision(false_effect, BuiltinFxParameterId::CursorAlpha),
        CheckedFxBindingDecision::Omitted
    ));

    assert!(
        analyze(&typewriter_fixture(
            "characters_per_second=20, cursor=false, cursor_alpha=0.4"
        ))
        .is_err(),
        "explicit inactive conditional parameter must reject"
    );

    let cursor_true = analyze(&typewriter_fixture("characters_per_second=20, cursor=true"))
        .expect("true cursor materializes conditional default");
    let true_effect = typewriter_effect(&cursor_true);
    assert_builtin_row(true_effect, BuiltinFxCallableRowId::TypewriterGlyphMask);
    assert!(matches!(
        fx_decision(true_effect, BuiltinFxParameterId::CursorAlpha),
        CheckedFxBindingDecision::Defaulted
    ));

    let cursor_true_explicit = analyze(&typewriter_fixture(
        "characters_per_second=20, cursor=true, cursor_alpha=0.4",
    ))
    .expect("true cursor retains explicit conditional value");
    let explicit_effect = typewriter_effect(&cursor_true_explicit);
    assert_builtin_row(explicit_effect, BuiltinFxCallableRowId::TypewriterGlyphMask);
    assert!(matches!(
        fx_decision(explicit_effect, BuiltinFxParameterId::CursorAlpha),
        CheckedFxBindingDecision::Explicit(binding)
            if matches!(binding, CheckedContentFxBinding::Abi(
                arcweft_presentation::fx::FxDefinitionArgumentValue::Runtime(
                    arcweft_presentation::fx::FxRuntimeValue::F32(value)
                )
            ) if value.get() == 0.4)
    ));
}

#[test]
fn project_fx_application_seals_definition_schema_defaults_and_closed_bindings() {
    let fixture = fixture(
        r##"
pub character alice {}

#[fx]
fn emphasis(accent: Color = rgb("#ffd060")) -> Fx {
    Fx.text(color = accent)
}

fn opening() {
    alice[#fx(emphasis(accent=rgb("#ff6b8a")))[text]];
}
"##,
        None,
    );
    let report = analyze(&fixture).expect("project Fx application");
    let insertions = content_insertions(&report);
    let [insertion] = insertions.as_slice() else {
        panic!("expected one project Fx insertion");
    };
    let CheckedContentEmission::Fx(application) = insertion.emission() else {
        panic!("project Fx must publish an Fx emission");
    };
    let CheckedFxDefinitionRef::Project { definition, .. } = application.definition() else {
        panic!("project Fx definition identity");
    };
    let parameter_schema = report
        .checked_fx_definitions()
        .get(definition)
        .and_then(crate::final_analysis::CheckedFxDefinition::project)
        .map(CheckedProjectFxDefinition::parameter_schema)
        .expect("project Fx parameter schema lives only in the definition catalog");
    let [parameter] = parameter_schema.parameters() else {
        panic!("project Fx parameter schema");
    };
    assert_eq!(parameter.name(), "accent");
    assert!(matches!(
        parameter.default(),
        Some(arcweft_presentation::fx::FxDefinitionArgumentValue::Runtime(
            arcweft_presentation::fx::FxRuntimeValue::Color(value)
        )) if *value == arcweft_presentation::fx::FxColor::from_rgba8([0xff, 0xd0, 0x60, 0xff])
    ));
    assert!(matches!(
        application.arguments(),
        [argument]
            if matches!(argument.parameter(), CheckedFxSourceParameter::Project(index)
                if index.get() == 0)
                && matches!(argument.decision(), CheckedFxBindingDecision::Explicit(
                    CheckedContentFxBinding::Abi(
                        arcweft_presentation::fx::FxDefinitionArgumentValue::Runtime(
                            arcweft_presentation::fx::FxRuntimeValue::Color(value)
                        )
                    )
                ) if *value == arcweft_presentation::fx::FxColor::from_rgba8([0xff, 0x6b, 0x8a, 0xff]))
    ));
    assert!(report.checked_fx_definitions().get(definition).is_some());
    let checked_definition = report
        .checked_fx_definitions()
        .get(definition)
        .and_then(crate::final_analysis::CheckedFxDefinition::project)
        .expect("project Fx body carrier");
    let CheckedFxGraphExpression::Constructor(call) = checked_definition.body().root() else {
        panic!("Fx.text wrapper must retain one constructor body");
    };
    assert_eq!(
        call.constructor(),
        arcweft_presentation::fx::FxSourceConstructor::Text
    );
    assert!(call.arguments().iter().any(|argument| {
        matches!(
            argument.value(),
            CheckedFxConstructorArgumentValue::Value(CheckedFxSymbolicValue::Parameter(
                parameter
            )) if parameter.index().get() == 0
        )
    }));
}

#[test]
fn project_fx_builtin_wave_body_seals_default_row_and_short_phase() {
    let fixture = fixture(
        r#"
pub character alice {}

#[fx]
fn gentle_wave() -> Fx {
    wave(phase=.glyph_transform)
}

fn opening() {
    alice[#fx(gentle_wave())[text]];
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("project Fx builtin wave wrapper");
    let project = report
        .checked_fx_definitions()
        .definitions()
        .find_map(|(_, definition)| definition.project())
        .expect("project wave definition");
    let CheckedFxGraphExpression::Builtin(call) = project.body().root() else {
        panic!("wave wrapper must retain a builtin body call");
    };
    assert!(matches!(
        call.definition(),
        CheckedFxDefinitionRef::Builtin {
            row: BuiltinFxCallableRowId::WaveGlyphTransform,
            ..
        }
    ));
    assert!(
        report
            .checked_fx_definitions()
            .definitions()
            .any(|(_, definition)| matches!(
                definition,
                crate::final_analysis::CheckedFxDefinition::Builtin {
                    row: BuiltinFxCallableRowId::WaveGlyphTransform,
                    ..
                }
            ))
    );
}

#[test]
fn project_fx_builtin_range_rejects_unproven_symbolic_parameter() {
    let fixture = fixture(
        r#"
#[fx]
fn parameterized_wave(amplitude: Length) -> Fx {
    wave(phase=.glyph_transform, amplitude=amplitude)
}
"#,
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(crate::final_analysis::FinalSemanticAnalysisError::FxDefinition {
            cause: crate::final_analysis::CheckedFxDefinitionSealError::UnprovableBuiltinConstraint {
                parameter: BuiltinFxParameterId::Amplitude,
            },
            ..
        })
    ));
}
