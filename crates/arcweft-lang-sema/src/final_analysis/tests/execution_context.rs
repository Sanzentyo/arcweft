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
