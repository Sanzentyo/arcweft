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
    let CheckedExecutionSource::InvokeBody(body) = source.clone() else {
        unreachable!()
    };
    let mutation = context
        .checked_execution_input_abi(CheckedExecutionSource::ExportMutation(body))
        .unwrap();
    assert_eq!(abi.coordinate().path(), mutation.coordinate().path());
    assert!(matches!(
        mutation.coordinate(),
        CheckedExecutionCoordinate::DeclarationMutationBody(_)
    ));
    assert_ne!(abi.definition_identity(), mutation.definition_identity());
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
fn formal_parameter_identities_preserve_whole_roles_across_body_and_source_revisions() {
    let observe = |text: &str, name: &str| {
        let world = fixture(text, None);
        let report = analyze(&world).unwrap();
        let source = declaration_body(&report, name);
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                source.clone(),
                None,
            )
            .unwrap();
        let invoked = context.checked_execution_input_abi(source.clone()).unwrap();
        let CheckedExecutionSource::InvokeBody(body) = source else {
            unreachable!()
        };
        let mutation = context
            .checked_execution_input_abi(CheckedExecutionSource::ExportMutation(body))
            .unwrap();
        assert_eq!(
            invoked
                .parameters()
                .iter()
                .map(|row| row.identity())
                .collect::<Vec<_>>(),
            mutation
                .parameters()
                .iter()
                .map(|row| row.identity())
                .collect::<Vec<_>>(),
            "execution intent does not rename the formal's accepted origin"
        );
        assert_eq!(invoked.parameters().len(), 3);
        assert_eq!(invoked.parameters()[0].bindings().len(), 2);
        assert!(invoked.parameters()[1].bindings().is_empty());
        assert_eq!(invoked.parameters()[2].bindings().len(), 1);
        invoked
            .parameters()
            .iter()
            .map(|row| (row.identity(), row.pattern().unwrap()))
            .collect::<Vec<_>>()
    };
    let source = "fn root((left, right): (i64, i64), _: i64, unused: i64) -> i64 { left + right }";
    let original = observe(source, "root");
    let revised = observe(
        &format!(
            "fn unrelated() -> i64 {{ 0i64 }}\n{}",
            source.replace("left + right", "left + right + 1i64")
        ),
        "root",
    );
    assert_eq!(
        original.iter().map(|row| row.0).collect::<Vec<_>>(),
        revised.iter().map(|row| row.0).collect::<Vec<_>>()
    );
    assert_ne!(original[0].1, revised[0].1);
    assert_ne!(original[0].0, original[1].0);
    assert_ne!(original[1].0, original[2].0);
    let other = observe(&source.replace("fn root", "fn other"), "other");
    assert!(
        original
            .iter()
            .zip(&other)
            .all(|(left, right)| left.0 != right.0)
    );
}

#[test]
fn view_formal_identity_is_shared_across_statement_body_roots() {
    let world = fixture(
        "view Main(value: i64) { Text(\"first\")\nText(\"second\") }",
        None,
    );
    let report = analyze(&world).unwrap();
    let body = report
        .hir_topology()
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == "Main")
        .unwrap();
    assert_eq!(body.roots().len(), 2);
    let identities = body
        .roots()
        .iter()
        .map(|root| {
            let source =
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration: body.declaration().clone(),
                    role: root.role(),
                });
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
            abi.parameters()[0].identity()
        })
        .collect::<Vec<_>>();
    assert_eq!(identities[0], identities[1]);
}

#[test]
fn rest_formal_uses_the_materialized_container_type() {
    let world = fixture("fn collect(head: i64, tail: ...i64) -> i64 { head }", None);
    let report = analyze(&world).unwrap();
    let source = declaration_body(&report, "collect");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let abi = context.checked_execution_input_abi(source).unwrap();
    assert_eq!(
        abi.parameters()[1].ty(),
        &TypeKind::Vec(Box::new(TypeKind::I64))
    );
    assert_ne!(
        abi.parameters()[0].identity(),
        abi.parameters()[1].identity()
    );
}

#[test]
fn parameter_passing_is_static_and_keeps_ingress_guarantees_separate() {
    use arcweft_core::plan::RuntimeFunctionParameterPassing;
    let world = fixture(
        "fn root(value: i64, shared: &i64, exclusive: &mut i64, resource: Need<i64>, callback: i64 -> i64 effects {}) -> i64 { callback(value) + callback(value) }",
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
    assert_eq!(
        abi.parameters()
            .iter()
            .map(|parameter| parameter.passing())
            .collect::<Vec<_>>(),
        vec![
            RuntimeFunctionParameterPassing::Value,
            RuntimeFunctionParameterPassing::Shared,
            RuntimeFunctionParameterPassing::Affine,
            RuntimeFunctionParameterPassing::Affine,
            RuntimeFunctionParameterPassing::Affine
        ]
    );
    let callback = &abi.parameters()[4];
    assert!(
        context
            .local_uses()
            .copy_requirement(callback.bindings()[0])
            .is_some(),
        "value-dependent callback Copy still requires the separate exact ingress proof"
    );
}

#[test]
fn method_receiver_passing_retains_borrow_kind_with_value_storage_type() {
    use arcweft_core::plan::RuntimeFunctionParameterPassing;
    let world = fixture(
        r#"
struct Counter { value: i64 }
impl Counter {
    fn owned(self) -> i64 { self.value }
    fn shared(&self) -> i64 { self.value }
    fn exclusive(&mut self) -> i64 { self.value }
}
"#,
        None,
    );
    let report = analyze(&world).unwrap();
    let mut storage_type = None;
    for (name, passing) in [
        ("owned", RuntimeFunctionParameterPassing::Value),
        ("shared", RuntimeFunctionParameterPassing::Shared),
        ("exclusive", RuntimeFunctionParameterPassing::Affine),
    ] {
        let declaration = world
            .symbols
            .callable_symbols()
            .find_map(|symbol| match symbol.declaration() {
                arcweft_lang_hir::symbol::CallableDeclarationKey::ImplMethod(method)
                    if method.method().as_str() == name =>
                {
                    Some(symbol.declaration().clone())
                }
                _ => None,
            })
            .expect("fixture method declaration");
        let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
            declaration,
            role: HirDeclarationBodyRootRole::ImplFunctionBody,
        });
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                source.clone(),
                None,
            )
            .unwrap();
        let abi = context.checked_execution_input_abi(source).unwrap();
        let [receiver] = abi.parameters() else {
            panic!("one receiver formal")
        };
        assert_eq!(receiver.passing(), passing);
        assert_eq!(receiver.bindings().len(), 1);
        match &storage_type {
            Some(ty) => assert_eq!(receiver.ty(), ty),
            None => storage_type = Some(receiver.ty().clone()),
        }
    }
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
    assert_ne!(pair.identity(), unused.identity());
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
    let mut identities = Vec::new();
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
        identities.push(parameter.identity());
        let [input] = abi.inputs() else {
            panic!("one formal binding")
        };
        assert_eq!(parameter.ty(), input.binding().ty());
        assert_eq!(
            parameter.passing(),
            if parameter.ty() == &TypeKind::I64 {
                arcweft_core::plan::RuntimeFunctionParameterPassing::Value
            } else {
                arcweft_core::plan::RuntimeFunctionParameterPassing::Affine
            }
        );
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
    assert_eq!(identities.len(), 2);
    assert_eq!(identities[0], identities[1]);
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

#[test]
fn binding_export_abi_retains_owned_pattern_outputs_and_root_membership() {
    let world = fixture(
        "fn root(value: (String, i64)) { let (label, count) = value; }\nflow other() { return () }",
        None,
    );
    let report = analyze(&world).unwrap();
    let project = world.project.analysis_view().unwrap();
    let binding = project
        .modules()
        .flat_map(|(_, module)| module.statements())
        .find_map(|(owner, statement)| {
            matches!(
                statement.kind(),
                arcweft_lang_hir::stmt::HirStmtKind::Let { .. }
            )
            .then_some(owner)
        })
        .unwrap();
    let source = CheckedExecutionSource::ExportBinding(binding);
    let context = report
        .checked_execution_context(project, &world.symbols, source.clone(), None)
        .unwrap();
    let admission = context.checked_deterministic_program(source).unwrap();
    let abi = admission.input_abi();
    abi.validate_for(&context).unwrap();
    assert!(
        abi.operations()
            .contains(&CheckedExecutionOperation::Statement(binding))
    );
    assert!(matches!(
        abi.coordinate(),
        CheckedExecutionCoordinate::Binding(_)
    ));
    assert!(abi.parameters().is_empty());
    assert_eq!(abi.inputs().len(), 1);
    assert_eq!(
        abi.binding_outputs()
            .iter()
            .map(|output| output.ty())
            .collect::<Vec<_>>(),
        vec![&TypeKind::String, &TypeKind::I64]
    );
    assert_eq!(
        abi.result().value_type(),
        Some(&TypeKind::Tuple(vec![TypeKind::String, TypeKind::I64]))
    );
    for output in abi.binding_outputs() {
        assert!(
            output
                .origin()
                .path()
                .is_at_or_below(abi.coordinate().path())
        );
    }
    let foreign = project
        .modules()
        .flat_map(|(_, module)| module.statements())
        .find_map(|(owner, statement)| {
            matches!(
                statement.kind(),
                arcweft_lang_hir::stmt::HirStmtKind::Return { .. }
            )
            .then_some(owner)
        })
        .unwrap();
    assert!(matches!(
        context.checked_execution_input_abi(CheckedExecutionSource::ExportBinding(foreign)),
        Err(CheckedExecutionContextError::ScopeMismatch { .. })
    ));
}

#[test]
fn binding_export_retains_dynamic_call_suspension_refusal() {
    let world = fixture(
        "fn root(callback: i64 -> i64 effects {}) { let value = callback(41i64); }",
        None,
    );
    let report = analyze(&world).unwrap();
    let project = world.project.analysis_view().unwrap();
    let binding = project
        .modules()
        .flat_map(|(_, module)| module.statements())
        .find_map(|(owner, statement)| {
            matches!(
                statement.kind(),
                arcweft_lang_hir::stmt::HirStmtKind::Let { .. }
            )
            .then_some(owner)
        })
        .unwrap();
    let source = CheckedExecutionSource::ExportBinding(binding);
    let context = report
        .checked_execution_context(project, &world.symbols, source.clone(), None)
        .unwrap();
    assert!(matches!(
        context.checked_deterministic_program(source),
        Err(crate::final_analysis::CheckedProgramAdmissionError::Suspension)
    ));
}

#[test]
fn immutable_project_function_aliases_use_the_checked_body_suspension() {
    let world = fixture(
        "fn pure(value: i64) -> i64 { value }\nfn root() -> i64 { let first = pure; let second = first; second(41i64) }",
        None,
    );
    let report = analyze(&world).unwrap();
    assert!(
        report
            .calls()
            .any(|(_, call)| call
                .selected_application()
                .is_some_and(|application| matches!(
                    application.core().candidates().selected().id(),
                    crate::callable::CallableCandidateId::FunctionValue(_)
                )))
    );
    let source = declaration_body(&report, "root");
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    let admitted = context
        .checked_deterministic_program(source)
        .expect("an exact immutable alias retains its known non-suspending body");
    assert_eq!(
        admitted.input_abi().suspension(),
        crate::final_analysis::CheckedSuspensionRole::NonSuspending
    );
}

#[test]
fn match_selector_inputs_exclude_results_and_keep_exact_arm_outputs() {
    let world = fixture(
        "view Main(value: (bool, String), label: String) { match value { (true, name) => Text(label), (false, name) => Text(name) } }",
        None,
    );
    let report = analyze(&world).unwrap();
    let project = world.project.analysis_view().unwrap();
    let matched = report
        .expressions()
        .find_map(|(owner, expression)| expression.match_fact().map(|matched| (owner, matched)))
        .unwrap();
    let source = CheckedExecutionSource::SelectMatch(matched.0.into());
    let context = report
        .checked_execution_context(project, &world.symbols, source.clone(), None)
        .unwrap();
    let admission = context.checked_deterministic_program(source).unwrap();
    let abi = admission.input_abi();
    assert!(matches!(
        abi.coordinate(),
        CheckedExecutionCoordinate::MatchSelection(_)
    ));
    assert_eq!(
        abi.inputs().len(),
        1,
        "only the selector parameter is free; the label is used only by an arm result"
    );
    assert_eq!(
        abi.inputs()[0].binding().ty(),
        &TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::String])
    );
    assert_eq!(abi.match_selection().unwrap().outputs().len(), 2);
    assert!(
        abi.match_selection()
            .unwrap()
            .outputs()
            .iter()
            .all(|outputs| outputs.len() == 1 && outputs[0].ty() == &TypeKind::String)
    );
    assert_eq!(
        abi.result().value_type(),
        Some(&TypeKind::Tuple(vec![
            TypeKind::U32,
            TypeKind::Tuple(vec![TypeKind::String])
        ]))
    );
    assert!(abi.expressions().contains(&matched.1.scrutinee()));
    for arm in matched.1.arms() {
        assert!(!abi.expressions().contains(&arm.value()));
    }
}

#[test]
fn retained_match_bodies_are_checked_before_nested_bindings() {
    let source = "view Main(value: (bool, String), enabled: bool) { match value { (true, label) when enabled => { let label = label; Text(label) }, (true, _) => Text(\"fallback\"), (false, label) => match enabled { true => Text(label), false => Button(label) } } }";
    let world = fixture(source, None);
    let report =
        analyze(&world).expect("body roots seed Match bindings before nested initializers");
    assert_eq!(
        report
            .expressions()
            .filter(|(_, fact)| fact.match_fact().is_some())
            .count(),
        2
    );
}

#[test]
fn match_bindings_are_available_to_independent_thread_bodies() {
    let world = fixture(
        r#"
flow main(value: (bool, String)) {
    let handle = match value {
        (true, label) => thread { let retained = label },
        (false, _) => thread {}
    }
}

"#,
        None,
    );
    let report = analyze(&world).expect("Thread roots retain their lexical Match inputs");
    assert!(
        report
            .locals()
            .any(|(_, local)| local.ty() == &TypeKind::String)
    );
}

#[test]
fn authored_view_statement_match_retains_its_statement_owner() {
    let world = fixture(
        r#"
view Main(value: (bool, String)) {
    { match value {
        (true, label) => Text(label),
        (false, label) => Button(label)
    }; }
}
"#,
        None,
    );
    let report = analyze(&world).expect("authored View statement Match checks");
    let project = world.project.analysis_view().unwrap();
    let module = project
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .unwrap();
    let matches = module
        .statements()
        .filter(|(_, row)| matches!(row.kind(), arcweft_lang_hir::stmt::HirStmtKind::Match(_)))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "semicolon Match retains a statement owner"
    );
    assert!(report.statement(matches[0].0).is_some());
    let source = CheckedExecutionSource::SelectMatch(matches[0].0.into());
    let context = report
        .checked_execution_context(project, &world.symbols, source.clone(), None)
        .unwrap();
    let admission = context.checked_deterministic_program(source).unwrap();
    let abi = admission.input_abi();
    assert_eq!(abi.inputs().len(), 1);
    assert_eq!(abi.match_selection().unwrap().outputs().len(), 2);
    assert!(
        abi.match_selection()
            .unwrap()
            .outputs()
            .iter()
            .all(|outputs| outputs.len() == 1 && outputs[0].ty() == &TypeKind::String)
    );
    let product = report
        .checked_match(
            project,
            &world.symbols,
            matches[0].0,
            super::super::CheckedMatchLimits::PRODUCTION,
        )
        .unwrap();
    assert!(product.coverage().exhaustive());
    assert_eq!(product.arms().len(), 2);
}

#[test]
fn statement_match_transcript_commits_owned_arm_bodies() {
    use crate::final_analysis::{CheckedMatchArmResult, CheckedMatchLimits};
    let digest = |number: i64| {
        let source = format!(
            "flow main(value: bool) {{ match value {{ true => {{ let result = {number}i64 }}, false => {{ let result = 0i64 }} }} }}"
        );
        let world = fixture(&source, None);
        let report = analyze(&world).unwrap();
        let project = world.project.analysis_view().unwrap();
        let module = project
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .unwrap();
        let owner = module
            .statements()
            .find_map(|(owner, row)| {
                matches!(row.kind(), arcweft_lang_hir::stmt::HirStmtKind::Match(_)).then_some(owner)
            })
            .unwrap();
        let product = report
            .checked_match(
                project,
                &world.symbols,
                owner,
                CheckedMatchLimits::PRODUCTION,
            )
            .unwrap();
        assert!(product.coverage().exhaustive());
        assert!(
            product
                .arms()
                .iter()
                .all(|arm| matches!(arm.result(), CheckedMatchArmResult::Body(_)))
        );
        product.semantic_digest()
    };
    assert_ne!(
        digest(1),
        digest(2),
        "body-only edits change the complete Match meaning"
    );
}

#[test]
fn empty_loop_requires_flow_without_a_nested_transfer() {
    let world = fixture("fn root() -> Never { loop {} }", None);
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
    assert_eq!(
        program.input_abi().control(),
        crate::final_analysis::CheckedExecutableControlRole::FlowRequired
    );
}

#[test]
fn retained_iteration_header_exports_bindings_without_key_or_body_execution() {
    let world = fixture(
        r#"fn items(reverse: bool) -> Vec<(i32, String)> { if reverse { [(2, "second"), (1, "first")] } else { [(1, "first"), (2, "second")] } }
fn item_key(id: i32) -> String { if id == 1 { "one" } else { "two" } }
view Main(reverse: bool) { {
        for (id, label) in items(reverse) key = item_key(id) { Text(label) }
        Text("tail")
    } }"#,
        None,
    );
    let report = analyze(&world).unwrap();
    let project = world.project.analysis_view().unwrap();
    let module = project
        .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
        .unwrap();
    let (owner, iteration) = module
        .statements()
        .find_map(|(owner, row)| match row.kind() {
            arcweft_lang_hir::stmt::HirStmtKind::For(iteration) => Some((owner, iteration)),
            _ => None,
        })
        .unwrap();
    let key = iteration.key().unwrap();
    assert_eq!(
        report.expression(key).unwrap().value_type(),
        Some(&TypeKind::String)
    );
    let source = CheckedExecutionSource::ExportIteration(owner);
    let context = report
        .checked_execution_context(project, &world.symbols, source.clone(), None)
        .unwrap();
    let program = context.checked_deterministic_program(source).unwrap();
    let abi = program.input_abi();
    assert_eq!(abi.statements(), [owner]);
    assert!(
        abi.operations()
            .contains(&CheckedExecutionOperation::Iteration(owner))
    );
    assert!(!abi.contains_expression(key));
    assert_eq!(
        abi.binding_outputs()
            .iter()
            .map(|binding| binding.local())
            .collect::<Vec<_>>(),
        iteration.locals()
    );
    assert_eq!(
        abi.result().value_type(),
        Some(&TypeKind::Vec(Box::new(TypeKind::Tuple(vec![
            TypeKind::I32,
            TypeKind::String
        ]))))
    );
    assert!(abi.effects().is_empty());
    assert_eq!(abi.inputs().len(), 1);
    assert_eq!(
        abi.control(),
        crate::final_analysis::CheckedExecutableControlRole::FlowRequired
    );
}

#[test]
fn mutation_body_exports_checked_free_roots_without_admitting_value_extraction_writes() {
    use crate::final_analysis::CheckedProgramAdmissionError;
    let world = fixture(
        r#"fn root() -> i64 {
            let mut caption: String = "first"
            let handler = || { caption = "second"; () }
            42i64
        }"#,
        None,
    );
    let report = analyze(&world).unwrap();
    let (owner, _) = report
        .expressions()
        .find(|(_, checked)| {
            matches!(
                checked.resolution(),
                CheckedExpressionResolution::Closure(_)
            )
        })
        .unwrap();
    let source =
        CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner));
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    assert!(matches!(
        context.checked_deterministic_program(source),
        Err(CheckedProgramAdmissionError::ExternalPlace { .. })
    ));
    let invoked = context
        .checked_execution_input_abi(CheckedExecutionSource::InvokeBody(
            CheckedExecutionBodyOwner::CallableValue(owner),
        ))
        .unwrap();
    let admission = context
        .checked_deterministic_program(CheckedExecutionSource::ExportMutation(
            CheckedExecutionBodyOwner::CallableValue(owner),
        ))
        .unwrap();
    let abi = admission.input_abi();
    assert_eq!(invoked.coordinate().path(), abi.coordinate().path());
    assert_ne!(invoked.definition_identity(), abi.definition_identity());
    assert!(matches!(
        abi.coordinate(),
        CheckedExecutionCoordinate::MutationBody(_)
    ));
    assert_eq!(abi.inputs().len(), 1);
    assert_eq!(abi.binding_outputs().len(), 1);
    assert_eq!(abi.inputs()[0].binding(), &abi.binding_outputs()[0]);
    assert!(
        abi.inputs()[0]
            .uses()
            .iter()
            .any(|usage| usage.access().place_access().is_some())
    );
    assert_eq!(
        abi.result().value_type(),
        Some(&TypeKind::Tuple(vec![TypeKind::Unit, TypeKind::String]))
    );
    assert!(abi.effects().is_empty());
}

#[test]
fn mutation_body_does_not_export_writes_from_independent_cleanup_frame() {
    use crate::final_analysis::CheckedProgramAdmissionError;
    let world = fixture(
        r#"fn root() -> i64 {
            let mut caption: String = "first"
            let handler = || { { defer { let copied = caption; caption = "cleanup"; () }; caption = "body"; () } };
            42i64
        }"#,
        None,
    );
    let (_, lowered) = world.project.view().modules().next().unwrap();
    assert_eq!(
        lowered.status(),
        arcweft_lang_hir::module::HirModuleStatus::Clean,
        "cleanup fixture must lower cleanly: {:?}",
        lowered.diagnostics()
    );
    let report = analyze(&world).unwrap();
    let (owner, _) = report
        .expressions()
        .find(|(_, checked)| {
            matches!(
                checked.resolution(),
                CheckedExpressionResolution::Closure(_)
            )
        })
        .unwrap();
    let source =
        CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::CallableValue(owner));
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source.clone(),
            None,
        )
        .unwrap();
    assert!(matches!(
        context.checked_deterministic_program(source),
        Err(CheckedProgramAdmissionError::ExternalPlace { .. })
    ));
}
