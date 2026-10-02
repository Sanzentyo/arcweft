//! Intent, complete formal layout and body-relative input acceptance.

use arcweft_lang_hir::project::HirDeclarationBodyRootRole;

use crate::final_analysis::{
    CheckedExecutionBodyOwner, CheckedExecutionContextError, CheckedExecutionCoordinate,
    CheckedExecutionInputRole, CheckedExecutionOperation, CheckedExecutionParameterOrigin,
    CheckedExecutionSource, CheckedExpressionResolution, FinalSemanticAnalysis,
};
use crate::types::TypeKind;

use super::{analyze, fixture};

fn declaration_body(report: &FinalSemanticAnalysis, name: &str) -> CheckedExecutionSource {
    let body = report
        .hir_topology()
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == name)
        .unwrap();
    let [root] = body.roots() else {
        panic!("one declaration body")
    };
    CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
        declaration: body.declaration().clone(),
        role: root.role(),
    })
}

#[test]
fn empty_body_has_an_authenticated_root_without_an_expression_anchor() {
    let world = fixture("flow empty() {}\nfn other() -> i64 { 42i64 }", None);
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "empty");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let abi = context.checked_execution_input_abi(source.clone()).unwrap();
    abi.validate_for(&context).unwrap();
    assert_eq!(abi.source(), &source);
    assert_eq!(abi.result().value_type(), Some(&TypeKind::Unit));
    assert!(matches!(
        abi.coordinate(),
        CheckedExecutionCoordinate::DeclarationBody(_)
    ));
    assert!(abi.expressions().is_empty());
    assert!(abi.statements().is_empty());
    assert!(abi.parameters().is_empty());
    assert!(abi.inputs().is_empty());
    assert!(abi.effects().is_empty());
    assert!(matches!(
        abi.operations(),
        [CheckedExecutionOperation::Body(_)]
    ));
    assert!(matches!(
        context.checked_execution_input_abi(declaration_body(&report, "other")),
        Err(CheckedExecutionContextError::ScopeMismatch { .. })
    ));
    assert!(matches!(
        context.checked_deterministic_program(declaration_body(&report, "other")),
        Err(crate::final_analysis::CheckedProgramAdmissionError::Context(error))
            if matches!(*error, CheckedExecutionContextError::ScopeMismatch { .. })
    ));
    let CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
        declaration,
        ..
    }) = source
    else {
        unreachable!()
    };
    assert!(
        context
            .checked_execution_input_abi(CheckedExecutionSource::InvokeBody(
                CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: HirDeclarationBodyRootRole::PredicateBody
                }
            ))
            .is_err()
    );
}

#[test]
fn body_parameters_keep_unused_arity_and_destructuring() {
    let world = fixture(
        "fn root((left, right): (i64, i64), unused: i64) -> i64 { left + right }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let abi = context.checked_execution_input_abi(source.clone()).unwrap();
    let [pair, unused] = abi.parameters() else {
        panic!("complete two-parameter arity")
    };
    assert_eq!(
        pair.ty(),
        &TypeKind::Tuple(vec![TypeKind::I64, TypeKind::I64])
    );
    assert_eq!(pair.bindings().len(), 2);
    assert!(pair.pattern().is_some());
    assert_eq!(unused.ty(), &TypeKind::I64);
    assert_eq!(unused.bindings().len(), 1);
    assert!(unused.pattern().is_some());
    assert_eq!(abi.inputs().len(), 3);
    assert!(abi.inputs().iter().all(|input| matches!(
        input.role(),
        CheckedExecutionInputRole::Parameter(CheckedExecutionParameterOrigin::Declaration(_))
    )));
    let unused_input = abi
        .inputs()
        .iter()
        .find(|input| unused.bindings().contains(&input.binding().local()))
        .unwrap();
    assert!(unused_input.uses().is_empty());
    assert!(
        abi.inputs()
            .iter()
            .filter(|input| pair.bindings().contains(&input.binding().local()))
            .all(|input| !input.uses().is_empty())
    );
    assert_eq!(abi.result().value_type(), Some(&TypeKind::I64));
}

#[test]
fn invoking_a_view_body_excludes_its_parameter_default_phase() {
    let world = fixture("view Main(first: i64 = 41i64) { Text(\"value\") }", None);
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "Main");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let abi = context.checked_execution_input_abi(source).unwrap();
    let default = report
        .checked_callables()
        .records()
        .find_map(|callable| callable.parameter_defaults().values().next())
        .unwrap();
    assert!(!abi.expressions().contains(&default.source()));
    assert_eq!(abi.parameters().len(), 1);
    assert_eq!(abi.inputs().len(), 1);
    assert!(abi.inputs()[0].uses().is_empty());
}

#[test]
fn creation_and_invocation_have_distinct_regions_and_complete_callback_parameters() {
    for callback in ["|unused: i64| first", "_ + first"] {
        let source = format!(
            "view Main(first: i64, callback: i64 -> i64 = {callback}) {{ Text(\"value\") }}"
        );
        let world = fixture(&source, None);
        let report = analyze(&world).unwrap();
        let owner = report
            .checked_callables()
            .records()
            .find_map(|callable| callable.parameter_defaults().values().next())
            .unwrap()
            .source();
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                owner,
                None,
            )
            .unwrap();
        let creation = context.checked_execution_input_abi(owner).unwrap();
        let body = context
            .checked_execution_input_abi(CheckedExecutionSource::InvokeBody(
                CheckedExecutionBodyOwner::CallableValue(owner),
            ))
            .unwrap();
        assert!(matches!(
            creation.coordinate(),
            CheckedExecutionCoordinate::Value(_)
        ));
        assert!(matches!(
            body.coordinate(),
            CheckedExecutionCoordinate::CallableBody(_)
        ));
        assert_eq!(creation.coordinate().path(), body.coordinate().path());
        assert_eq!(creation.expressions(), [owner]);
        assert!(creation.parameters().is_empty());
        assert!(creation.synthetic_uses().is_empty());
        assert_eq!(creation.inputs().len(), 1);
        assert_eq!(body.result().value_type(), Some(&TypeKind::I64));
        assert_eq!(body.parameters().len(), 1);
        assert_eq!(body.parameters()[0].ty(), &TypeKind::I64);
        assert_eq!(
            body.inputs()
                .iter()
                .filter(|input| matches!(input.role(), CheckedExecutionInputRole::Free))
                .count(),
            1
        );
        match report.expression(owner).unwrap().resolution() {
            CheckedExpressionResolution::Closure(_) => {
                assert!(body.parameters()[0].pattern().is_some());
                let unused = body
                    .inputs()
                    .iter()
                    .find(|input| matches!(input.role(), CheckedExecutionInputRole::Parameter(_)))
                    .unwrap();
                assert!(unused.uses().is_empty());
            }
            CheckedExpressionResolution::ImplicitCallable(_) => {
                assert!(body.parameters()[0].pattern().is_none());
                assert!(body.parameters()[0].bindings().is_empty());
                assert!(matches!(
                    body.parameters()[0].origin(),
                    CheckedExecutionParameterOrigin::Implicit(_)
                ));
                assert!(!body.synthetic_uses().is_empty());
                assert!(body.synthetic_uses().iter().all(|usage| matches!(
                    usage.access().owner(),
                    crate::final_analysis::CheckedSyntheticUseOwner::ImplicitParameter(_)
                )));
            }
            _ => panic!("callback source"),
        }
    }
}

#[test]
fn invocation_preserves_latent_effects_that_creation_does_not_execute() {
    let source = "fn writer(value: i64) -> i64 effects { fs.write } { value }\nfn ignore(callback: i64 -> i64, value: i64) -> i64 { value }\nflow main() -> i64 { return ignore(|value: i64| writer(value), 42i64) }";
    let world = fixture(source, None);
    let report = analyze(&world).unwrap();
    let owner = report
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(
                expression.resolution(),
                CheckedExpressionResolution::Closure(_)
            )
            .then_some(owner)
        })
        .unwrap();
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            owner,
            None,
        )
        .unwrap();
    let creation = context.checked_execution_input_abi(owner).unwrap();
    let body = context
        .checked_execution_input_abi(CheckedExecutionSource::InvokeBody(
            CheckedExecutionBodyOwner::CallableValue(owner),
        ))
        .unwrap();
    assert!(creation.effects().is_empty());
    assert!(!body.effects().is_empty());
    context.checked_deterministic_program(owner).unwrap();
    assert!(matches!(
        context.checked_deterministic_program(CheckedExecutionSource::InvokeBody(
            CheckedExecutionBodyOwner::CallableValue(owner),
        )),
        Err(crate::final_analysis::CheckedProgramAdmissionError::Effects { .. })
    ));
    assert!(
        body.expressions()
            .iter()
            .any(|owner| report.call(*owner).is_some())
    );
}

#[test]
fn declaration_body_inputs_close_under_the_same_instance_as_value_transfers() {
    use crate::final_analysis::{CheckedLocalReadMode, CheckedLocalUseInstantiation};
    let world = fixture(
        "fn identity<T>(value: T) -> T { value }\nfn numeric() -> i64 { identity(1i64) }\nfn pending(value: Need<i64>) -> Need<i64> { identity(value) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "identity");
    assert!(matches!(
        report.checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None
        ),
        Err(CheckedExecutionContextError::OpenDeclaration { .. })
    ));
    let instances = super::project_specialization::selections(&report, "identity")
        .into_iter()
        .map(|selection| selection.close_instance(None).unwrap())
        .collect::<Vec<_>>();
    for instance in &instances {
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                source.clone(),
                Some(CheckedLocalUseInstantiation::ProjectFunction(instance)),
            )
            .unwrap();
        let abi = context.checked_execution_input_abi(source.clone()).unwrap();
        abi.validate_for(&context).unwrap();
        let [parameter] = abi.parameters() else {
            panic!("one full formal")
        };
        let [input] = abi.inputs() else {
            panic!("one formal binding")
        };
        assert_eq!(parameter.ty(), input.binding().ty());
        assert_eq!(abi.result().value_type(), Some(parameter.ty()));
        let [usage] = input.uses() else {
            panic!("one use")
        };
        assert_eq!(
            usage.access().value_transfer().unwrap().mode(),
            if parameter.ty() == &TypeKind::I64 {
                CheckedLocalReadMode::Copy
            } else {
                CheckedLocalReadMode::Move
            }
        );
    }
}

#[test]
fn admission_retains_closed_environment_after_context_and_solution_are_dropped() {
    use crate::final_analysis::{CheckedLocalReadMode, CheckedLocalUseInstantiation};
    let world = fixture(
        "fn identity<T>(value: T) -> T { value }\nfn numeric() -> i64 { identity(1i64) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "identity");
    let (program, generic, expected_instantiation) = {
        let instance = super::project_specialization::selections(&report, "identity")
            .remove(0)
            .close_instance(None)
            .unwrap();
        let expected_instantiation = instance.instantiation();
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                source.clone(),
                Some(CheckedLocalUseInstantiation::ProjectFunction(&instance)),
            )
            .unwrap();
        let program = context.checked_deterministic_program(source).unwrap();
        let local = program.input_abi().parameters()[0].bindings()[0];
        let generic = report.local(local).unwrap().ty().clone();
        (program, generic, expected_instantiation)
    };
    let environment = program.input_abi().environment();
    assert_eq!(
        environment.instantiate_type(&generic).unwrap(),
        TypeKind::I64
    );
    let Some(CheckedLocalUseInstantiation::ProjectFunction(instance)) = environment.instantiation()
    else {
        panic!("owned function environment");
    };
    assert_eq!(instance.instantiation(), expected_instantiation);
    let input = &program.input_abi().inputs()[0];
    let transfer = environment
        .local_uses()
        .value_transfer_at(input.uses()[0].site())
        .unwrap();
    assert_eq!(transfer.mode(), CheckedLocalReadMode::Copy);
    assert_eq!(transfer, input.uses()[0].access().value_transfer().unwrap());
}

#[test]
fn global_admissions_share_the_report_catalog_and_one_context_environment() {
    use crate::final_analysis::CheckedLocalUseAuthority;
    use std::sync::Arc;
    let world = fixture("fn root(value: i64) -> i64 { value }", None);
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let first = context
        .checked_deterministic_program(source.clone())
        .unwrap();
    let second = context.checked_deterministic_program(source).unwrap();
    assert!(Arc::ptr_eq(
        first.input_abi().environment(),
        second.input_abi().environment()
    ));
    let CheckedLocalUseAuthority::Global(catalog) = first.input_abi().environment().local_uses()
    else {
        panic!("global authority");
    };
    assert!(Arc::ptr_eq(catalog, report.checked_local_uses()));
    drop(context);
    first
        .input_abi()
        .environment()
        .validate_analysis(&report)
        .unwrap();
    drop(report);
    assert_eq!(
        first
            .input_abi()
            .environment()
            .instantiate_type(&TypeKind::I64)
            .unwrap(),
        TypeKind::I64
    );
}

#[test]
fn pipe_reads_retain_synthetic_transfer_evidence_without_becoming_formal_parameters() {
    let world = fixture(
        "fn root(value: i64) -> (i64, i64) { value |> (^, ^) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let abi = context.checked_execution_input_abi(source).unwrap();
    assert_eq!(abi.parameters().len(), 1);
    assert_eq!(abi.synthetic_uses().len(), 2);
    assert!(abi.synthetic_uses().iter().all(|usage| matches!(
        usage.access().owner(),
        crate::final_analysis::CheckedSyntheticUseOwner::Pipe(_)
    ) && usage.access().mode()
        == crate::final_analysis::CheckedLocalReadMode::Copy));
}

#[test]
fn deterministic_body_admits_local_mutation_loop_control_return_and_pure_calls() {
    let world = fixture(
        "fn increment(value: i64) -> i64 { value + 1i64 }\nflow root(condition: bool) -> i64 { let mut value = 0i64\nwhile condition { value = increment(value)\nif condition { continue }\nbreak }\nreturn value }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let program = context.checked_deterministic_program(source).unwrap();
    assert!(program.input_abi().effects().is_empty());
    assert_eq!(
        program.input_abi().control(),
        crate::final_analysis::CheckedExecutableControlRole::FlowRequired
    );
    assert!(
        program
            .input_abi()
            .statements()
            .iter()
            .filter_map(|statement| report.statement(*statement))
            .any(|statement| matches!(
                statement.payload(),
                crate::final_analysis::CheckedStatementPayload::ControlTransfer(_)
            ))
    );
    program.input_abi().validate_for(&context).unwrap();

    let foreign = fixture("fn root() -> i64 { 1i64 }", None);
    assert!(matches!(
        program
            .input_abi()
            .validate_project(foreign.project.analysis_view().unwrap()),
        Err(CheckedExecutionContextError::ForeignAuthority)
    ));
}

#[test]
fn extracting_a_value_cannot_return_to_its_original_declaration() {
    use crate::final_analysis::{CheckedProgramAdmissionError, CheckedStatementPayload};
    let world = fixture("fn root() -> i64 { { return 42i64 } }", None);
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let program = context.checked_deterministic_program(source).unwrap();
    let statement = program
        .input_abi()
        .statements()
        .iter()
        .copied()
        .find(|statement| {
            matches!(
                report.statement(*statement).unwrap().payload(),
                CheckedStatementPayload::ControlTransfer(_)
            )
        })
        .unwrap();
    let owners = report
        .expressions()
        .filter_map(|(owner, _)| {
            let abi = context.checked_execution_input_abi(owner).ok()?;
            abi.statements().contains(&statement).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert!(!owners.is_empty());
    for owner in owners {
        assert!(matches!(context.checked_deterministic_program(owner),
            Err(CheckedProgramAdmissionError::ExternalControl { statement: rejected }) if rejected == statement));
    }
}

#[test]
fn extracting_external_assignment_is_rejected_but_the_complete_owned_body_is_admitted() {
    use crate::final_analysis::{CheckedLocalPlaceMode, CheckedProgramAdmissionError};
    let world = fixture(
        "fn root(input: Vec<Need<i64>>, replacement: Vec<Need<i64>>) -> i64 { let mut items = input; { let marker = 0i64; items = replacement; 42i64 } }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    context.checked_deterministic_program(source).unwrap();
    let rejected = report
        .expressions()
        .filter_map(
            |(owner, _)| match context.checked_deterministic_program(owner) {
                Err(CheckedProgramAdmissionError::ExternalPlace {
                    local,
                    mode: CheckedLocalPlaceMode::Assign,
                }) => Some(local),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert!(
        !rejected.is_empty(),
        "the nested assignment cannot write its original external binding"
    );
}

#[test]
fn extracted_try_requires_its_carrier_boundary_inside_the_program() {
    use crate::final_analysis::CheckedProgramAdmissionError;
    let world = fixture(
        "fn root(first: Result<i64, String>, second: Result<i64, String>, flag: bool) -> Result<i64, String> { result { match flag { true => try first\nfalse => try second } } }",
        None,
    );
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    context.checked_deterministic_program(source).unwrap();
    let tried = report
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.resolution(), CheckedExpressionResolution::Try(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert_eq!(tried.len(), 2);
    for owner in tried {
        assert!(matches!(context.checked_deterministic_program(owner),
            Err(CheckedProgramAdmissionError::ExternalTry { expression }) if expression == owner));
    }
}

#[test]
fn deterministic_defer_checks_control_in_its_own_cleanup_frame() {
    use crate::final_analysis::CheckedProgramAdmissionError;
    for (cleanup, admitted) in [
        ("let copied = value; ()", true),
        (
            "let ignored = if value == 0i64 { return value } else { () }; ()",
            false,
        ),
    ] {
        let world = fixture(
            &format!("fn root(value: i64) -> i64 {{ {{ defer {{ {cleanup} }}; value }} }}"),
            None,
        );
        let report = analyze(&world).unwrap();
        let source = declaration_body(&report, "root");
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                source.clone(),
                None,
            )
            .unwrap();
        let result = context.checked_deterministic_program(source);
        if admitted {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(CheckedProgramAdmissionError::ExternalControl { .. })
            ));
        }
    }
}
