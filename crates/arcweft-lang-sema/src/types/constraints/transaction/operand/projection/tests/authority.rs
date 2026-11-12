use super::bindings::setup;
use super::*;
use crate::types::MapKind;
use crate::types::constraints::normalization::seal_path;
use crate::types::constraints::{
    ConstraintSourceContainerPolicy, PreparedConstraintSourceProjection,
};

#[test]
fn rebuilt_children_keep_their_own_alias_terminal_and_length_authorities() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, types, lengths) = setup(&cancellation, 2, 1);
    let tuple = context.fixture_operand(TypeKind::Tuple(vec![
        TypeKind::GenericParam(types[1].clone()),
        TypeKind::String,
    ]));
    let array = context.fixture_operand(TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Generic(lengths[0].clone()),
    });
    let length = context.fixture_operand(ArrayLength::Const(7));
    path.bindings.insert(types[0].clone(), tuple);
    path.bindings.insert(types[1].clone(), array);
    path.const_bindings.insert(lengths[0].clone(), length);
    let sealed = seal_path(path, &mut context).unwrap();
    let value = &sealed.bindings[&types[0]];
    let ConstraintOperandOrigin::Rebuilt { children, .. } = value.origin.as_ref() else {
        panic!("rebuilt tuple")
    };
    let mut selected = value.view().children(&mut context).unwrap();
    let array = selected.next().unwrap().unwrap();
    let string = selected.next().unwrap().unwrap();
    assert!(Arc::ptr_eq(&array.value_origin, &children[0].origin));
    assert!(Arc::ptr_eq(&string.value_origin, &children[1].origin));
    assert!(!Arc::ptr_eq(&array.value_origin, &string.value_origin));
    let retained = array.retained();
    let ConstraintOperandOrigin::Projection {
        producer, selected, ..
    } = retained.origin.as_ref()
    else {
        panic!("retained child")
    };
    assert!(Arc::ptr_eq(producer, &value.origin));
    assert!(Arc::ptr_eq(selected, &children[0].origin));

    let ConstraintOperandOrigin::TypeAlias { terminal, .. } = children[0].origin.as_ref() else {
        panic!("array substitution")
    };
    let ConstraintOperandOrigin::Rebuilt {
        children: items,
        length: Some(length),
        ..
    } = terminal.origin.as_ref()
    else {
        panic!("rebuilt array")
    };
    let item = array
        .children(&mut context)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let len = array.array_length().unwrap().unwrap();
    assert!(Arc::ptr_eq(&item.value_origin, &items[0].origin));
    assert!(Arc::ptr_eq(&len.value_origin, &length.origin));
    let retained_array = retained.view();
    let item = retained_array
        .children(&mut context)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&item.value_origin, &items[0].origin));
}

#[test]
fn source_container_ports_select_the_actual_header_and_expected_value_separately() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let actual = context.fixture_operand(TypeKind::Map {
        kind: MapKind::Sorted,
        key: Box::new(TypeKind::String),
        value: Box::new(TypeKind::I64),
    });
    let expected = context.fixture_operand(TypeKind::Tuple(vec![TypeKind::Bool]));
    let projection = CheckedConstraintSourceProjection::derive(
        PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Named,
        },
        actual.value(),
    )
    .unwrap();
    let composed = projection
        .compose_operand(&actual.view(), &expected.view())
        .unwrap();
    let mut children = composed.view().children(&mut context).unwrap();
    let key = children.next().unwrap().unwrap();
    let value = children.next().unwrap().unwrap();
    assert!(Arc::ptr_eq(&key.value_origin, &actual.origin));
    assert!(Arc::ptr_eq(&value.value_origin, &expected.origin));
    let nested = value
        .children(&mut context)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&nested.value_origin, &expected.origin));
    assert!(!Arc::ptr_eq(&nested.value_origin, &actual.origin));

    let actual = context.fixture_operand(TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(7),
    });
    let projection = CheckedConstraintSourceProjection::derive(
        PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Positional,
        },
        actual.value(),
    )
    .unwrap();
    let composed = projection
        .compose_operand(&actual.view(), &expected.view())
        .unwrap();
    let item = composed
        .view()
        .children(&mut context)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let view = composed.view();
    let len = view.array_length().unwrap().unwrap();
    assert!(Arc::ptr_eq(&item.value_origin, &expected.origin));
    assert!(Arc::ptr_eq(&len.value_origin, &actual.origin));
}

#[test]
fn a_missing_rebuilt_port_is_rejected_instead_of_borrowing_the_root_authority() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let root = context.fixture_operand(TypeKind::Tuple(vec![TypeKind::I64]));
    let forged = ConstraintOperand {
        value: Arc::clone(&root.value),
        scope: Arc::clone(&root.scope),
        origin: Arc::new(ConstraintOperandOrigin::Rebuilt {
            source: Arc::clone(&root.origin),
            contents_scope: Arc::clone(&root.scope),
            children: Box::new([]),
            length: None,
            header: None,
        }),
    };
    assert!(matches!(
        forged
            .view()
            .children(&mut context)
            .unwrap()
            .next()
            .unwrap(),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
}
