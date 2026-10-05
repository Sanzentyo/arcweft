use arcweft_lang_hir::{
    identity::ExprId, project::HirSemanticPathRoot, symbol::CallableDeclarationKey,
};

use crate::{
    final_analysis::{
        CheckedExecutionContextError, CheckedLocalReadMode, CheckedLocalUseError,
        CheckedLocalUseInstantiation, FinalSemanticAnalysis,
    },
    types::TypeKind,
};

use super::{analyze, fixture, project_specialization::selections};

#[test]
fn monomorphic_producer_plan_excludes_an_incidental_instance_coordinate() {
    let world = fixture(
        "fn identity(value: i64) -> i64 { value }\nfn wrapper(value: i64) -> i64 { identity(value) }\nfn numeric() -> i64 { wrapper(1i64) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let instance = selections(&report, "wrapper")[0]
        .close_instance(None)
        .unwrap();
    let source = report
        .calls()
        .find_map(|(owner, _)| {
            let location = report.hir_topology().semantic_path(owner.into()).unwrap()?;
            (location.root() == &HirSemanticPathRoot::Declaration(instance.declaration().clone()))
                .then_some(owner)
        })
        .unwrap();
    let global = report
        .checked_call_producer_definition(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source,
        )
        .unwrap();
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source,
            Some(CheckedLocalUseInstantiation::ProjectFunction(&instance)),
        )
        .unwrap();
    let bound = context
        .checked_call_producer_definition(&world.symbols, source)
        .unwrap();
    assert_eq!(global.site(), bound.site());
    assert_eq!(global.plan(), bound.plan());
    assert!(!global.matches_authority(source, context.environment().local_uses()));
    assert!(bound.matches_authority(source, context.environment().local_uses()));
}

#[test]
fn producer_definitions_retain_exact_closed_instance_and_source_authority() {
    let world = fixture(
        "fn identity<T>(value: T) -> T { value }\nfn wrapper<T>(value: T) -> T { identity(value) }\nfn numeric() -> i64 { wrapper(1i64) }\nfn pending(value: Need<i64>) -> Need<i64> { wrapper(value) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let instances = selections(&report, "wrapper")
        .into_iter()
        .map(|selection| selection.close_instance(None).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    let source = report
        .calls()
        .find_map(|(owner, _)| {
            let location = report.hir_topology().semantic_path(owner.into()).unwrap()?;
            (location.root()
                == &HirSemanticPathRoot::Declaration(instances[0].declaration().clone()))
                .then_some(owner)
        })
        .unwrap();
    let contexts = instances
        .iter()
        .map(|instance| {
            report
                .checked_execution_context(
                    world.project.analysis_view().unwrap(),
                    &world.symbols,
                    source,
                    Some(CheckedLocalUseInstantiation::ProjectFunction(instance)),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let first = contexts[0]
        .checked_call_producer_definition(&world.symbols, source)
        .unwrap();
    let second = contexts[1]
        .checked_call_producer_definition(&world.symbols, source)
        .unwrap();
    assert_eq!(
        first,
        contexts[0]
            .checked_call_producer_definition(&world.symbols, source)
            .unwrap()
    );
    assert_eq!(first.site(), second.site());
    assert_ne!(first.plan(), second.plan());
    assert!(first.matches_authority(source, contexts[0].environment().local_uses()));
    assert!(!first.matches_authority(source, contexts[1].environment().local_uses()));
    let other_source = local_source(&report, instances[0].declaration());
    assert_ne!(source, other_source);
    assert!(!first.matches_authority(other_source, contexts[0].environment().local_uses()));
    let foreign_world = fixture("fn wrapper(value: i64) -> i64 { value }", None);
    let foreign_report = analyze(&foreign_world).unwrap();
    assert!(matches!(
        contexts[0].environment().producer_definition(
            &foreign_report,
            foreign_world.project.analysis_view().unwrap(),
            &foreign_world.symbols,
            source,
            crate::CheckedExpressionProducerKind::Call,
        ),
        Err(CheckedExecutionContextError::ForeignAuthority)
    ));
}

fn local_source(report: &FinalSemanticAnalysis, declaration: &CallableDeclarationKey) -> ExprId {
    report
        .expressions()
        .find_map(|(owner, expression)| {
            expression.execution_local_use()?;
            let location = report.hir_topology().semantic_path(owner.into()).unwrap()?;
            (location.root() == &HirSemanticPathRoot::Declaration(declaration.clone()))
                .then_some(owner)
        })
        .unwrap()
}

#[test]
fn context_closes_types_and_transfers_under_one_exact_instance() {
    let world = fixture(
        "fn identity<T>(value: T) -> T { value }\nfn numeric() -> i64 { identity(1i64) }\nfn pending(value: Need<i64>) -> Need<i64> { identity(value) }",
        None,
    );
    let report = analyze(&world).unwrap();
    let instances = selections(&report, "identity")
        .into_iter()
        .map(|selection| selection.close_instance(None).unwrap())
        .collect::<Vec<_>>();
    let source = local_source(&report, instances[0].declaration());
    assert!(matches!(
        report.checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source,
            None,
        ),
        Err(CheckedExecutionContextError::OpenDeclaration { .. })
    ));
    let mut modes = Vec::new();
    for instance in &instances {
        let context = report
            .checked_execution_context(
                world.project.analysis_view().unwrap(),
                &world.symbols,
                source,
                Some(CheckedLocalUseInstantiation::ProjectFunction(instance)),
            )
            .unwrap();
        context.admit_root(&source.into()).unwrap();
        let closed = context
            .instantiate_type(report.expression(source).unwrap().value_type().unwrap())
            .unwrap();
        let abi = context.checked_execution_input_abi(source).unwrap();
        abi.validate_for(&context).unwrap();
        assert_eq!(abi.result().value_type(), Some(&closed));
        assert_eq!(abi.inputs().len(), 1);
        assert_eq!(abi.inputs()[0].binding().ty(), &closed);
        modes.push(match closed {
            TypeKind::I64 => {
                assert_eq!(
                    context.local_uses().read_at(source).unwrap().mode(),
                    CheckedLocalReadMode::Copy
                );
                CheckedLocalReadMode::Copy
            }
            TypeKind::Need(_) => {
                assert_eq!(
                    context.local_uses().read_at(source).unwrap().mode(),
                    CheckedLocalReadMode::Move
                );
                CheckedLocalReadMode::Move
            }
            other => panic!("unexpected closed type {other:?}"),
        });
    }
    modes.sort();
    assert_eq!(
        modes,
        [CheckedLocalReadMode::Copy, CheckedLocalReadMode::Move]
    );
    let first = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source,
            Some(CheckedLocalUseInstantiation::ProjectFunction(&instances[0])),
        )
        .unwrap();
    let second = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            source,
            Some(CheckedLocalUseInstantiation::ProjectFunction(&instances[1])),
        )
        .unwrap();
    let abi = first.checked_execution_input_abi(source).unwrap();
    assert!(matches!(
        abi.validate_for(&second),
        Err(CheckedExecutionContextError::InstanceMismatch)
    ));
}

#[test]
fn context_rejects_foreign_instance_and_same_world_wrong_declaration() {
    let source = "fn identity<T>(value: T) -> T { value }\nfn other<T>(value: T) -> T { value }\nfn caller() -> i64 { identity(1i64) + other(2i64) }";
    let first = fixture(source, None);
    let second = fixture(source, None);
    let first_report = analyze(&first).unwrap();
    let report = analyze(&second).unwrap();
    let foreign = selections(&first_report, "identity")
        .remove(0)
        .close_instance(None)
        .unwrap();
    let current = selections(&report, "identity")
        .remove(0)
        .close_instance(None)
        .unwrap();
    let other = selections(&report, "other")
        .remove(0)
        .close_instance(None)
        .unwrap();
    let source = local_source(&report, current.declaration());
    assert!(matches!(
        report.checked_execution_context(
            second.project.analysis_view().unwrap(),
            &second.symbols,
            source,
            Some(CheckedLocalUseInstantiation::ProjectFunction(&foreign)),
        ),
        Err(CheckedExecutionContextError::LocalUse(
            CheckedLocalUseError::ForeignInstance
        ))
    ));
    assert!(matches!(
        report.checked_execution_context(
            second.project.analysis_view().unwrap(),
            &second.symbols,
            source,
            Some(CheckedLocalUseInstantiation::ProjectFunction(&other)),
        ),
        Err(CheckedExecutionContextError::ScopeMismatch { .. })
    ));
}

#[test]
fn monomorphic_context_is_bound_to_one_lexical_owner() {
    let world = fixture(
        "fn first(value: i64) -> i64 { value }\nfn second(value: i64) -> i64 { value }",
        None,
    );
    let report = analyze(&world).unwrap();
    let sources = report
        .expressions()
        .filter_map(|(owner, expression)| expression.execution_local_use().map(|_| owner))
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    let context = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            sources[0],
            None,
        )
        .unwrap();
    context.admit_root(&sources[0].into()).unwrap();
    assert!(matches!(
        context.admit_root(&sources[1].into()),
        Err(CheckedExecutionContextError::ScopeMismatch { .. })
    ));
    let abi = context.checked_execution_input_abi(sources[0]).unwrap();
    let other = report
        .checked_execution_context(
            world.project.analysis_view().unwrap(),
            &world.symbols,
            sources[1],
            None,
        )
        .unwrap();
    assert!(matches!(
        abi.validate_for(&other),
        Err(CheckedExecutionContextError::ScopeMismatch { .. })
    ));
    assert_eq!(
        context.instantiate_type(&TypeKind::I64).unwrap(),
        TypeKind::I64
    );
}

#[test]
fn input_snapshot_rejects_an_equivalent_rebuilds_context() {
    let source = "fn identity(value: i64) -> i64 { value }";
    let first = fixture(source, None);
    let second = fixture(source, None);
    let first_report = analyze(&first).unwrap();
    let second_report = analyze(&second).unwrap();
    let first_source = first_report
        .expressions()
        .find_map(|(owner, expression)| expression.execution_local_use().map(|_| owner))
        .unwrap();
    let second_source = second_report
        .expressions()
        .find_map(|(owner, expression)| expression.execution_local_use().map(|_| owner))
        .unwrap();
    let first_context = first_report
        .checked_execution_context(
            first.project.analysis_view().unwrap(),
            &first.symbols,
            first_source,
            None,
        )
        .unwrap();
    let second_context = second_report
        .checked_execution_context(
            second.project.analysis_view().unwrap(),
            &second.symbols,
            second_source,
            None,
        )
        .unwrap();
    let abi = first_context
        .checked_execution_input_abi(first_source)
        .unwrap();
    let second_abi = second_context
        .checked_execution_input_abi(second_source)
        .unwrap();
    assert_eq!(abi.coordinate(), second_abi.coordinate());
    assert_eq!(abi.result(), second_abi.result());
    assert!(matches!(
        abi.validate_for(&second_context),
        Err(CheckedExecutionContextError::ForeignAuthority)
    ));
}
