use super::bindings::setup;
use super::*;
use crate::types::constraints::normalization::seal_path;

#[test]
fn structural_sealing_keeps_corresponding_children_aliases_and_array_length_proofs() {
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
    let tuple_owner = Arc::downgrade(&tuple.value);
    let array_owner = Arc::downgrade(&array.value);
    let length_owner = Arc::downgrade(&length.value);
    path.bindings.insert(types[0].clone(), tuple.clone());
    path.bindings.insert(types[1].clone(), array.clone());
    path.const_bindings
        .insert(lengths[0].clone(), length.clone());
    let sealed = seal_path(path, &mut context).unwrap();
    let value = &sealed.bindings[&types[0]];
    assert_eq!(
        value.value(),
        &TypeKind::Tuple(vec![
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Const(7)
            },
            TypeKind::String,
        ])
    );
    let ConstraintOperandOrigin::Rebuilt {
        source,
        children,
        length: None,
        ..
    } = value.origin.as_ref()
    else {
        panic!("the tuple keeps its own source and corresponding children")
    };
    let ConstraintOperandOrigin::Projection {
        root,
        address: None,
        ..
    } = source.as_ref()
    else {
        panic!("the exact tuple root is retained")
    };
    assert!(Arc::ptr_eq(root, &tuple.value));
    assert_eq!(children.len(), 2);
    let ConstraintOperandOrigin::TypeAlias {
        steps: aliases,
        terminal,
    } = children[0].origin.as_ref()
    else {
        panic!("only the first tuple child substitutes the array")
    };
    assert_eq!(aliases.len(), 1);
    assert_eq!(
        steps(&aliases[0].occurrence),
        [ConstraintOperandProjectionStep::TypeChild(0)]
    );
    assert!(Arc::ptr_eq(&aliases[0].replacement.value, &array.value));
    let ConstraintOperandOrigin::Rebuilt {
        children: items,
        length: Some(len),
        ..
    } = terminal.origin.as_ref()
    else {
        panic!("the substituted array keeps distinct item and length proofs")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(
        source_steps(&items[0].origin),
        [ConstraintOperandProjectionStep::TypeChild(0)]
    );
    let ConstraintOperandOrigin::ConstAlias {
        steps: aliases,
        terminal,
    } = len.origin.as_ref()
    else {
        panic!("the length uses its own constant substitution")
    };
    assert_eq!(aliases.len(), 1);
    assert_eq!(
        steps(&aliases[0].occurrence),
        [ConstraintOperandProjectionStep::ArrayLength]
    );
    assert!(Arc::ptr_eq(&aliases[0].replacement.value, &length.value));
    assert!(
        matches!(terminal.origin.as_ref(), ConstraintOperandOrigin::ConstOccurrence(root) if Arc::ptr_eq(&root.value, &length.value))
    );
    assert_eq!(
        source_steps(&children[1].origin),
        [ConstraintOperandProjectionStep::TypeChild(1)]
    );
    drop(tuple);
    drop(array);
    drop(length);
    assert!(tuple_owner.upgrade().is_some());
    assert!(array_owner.upgrade().is_some());
    assert!(length_owner.upgrade().is_some());
    drop(sealed);
    assert!(tuple_owner.upgrade().is_none());
    assert!(array_owner.upgrade().is_none());
    assert!(length_owner.upgrade().is_none());
}

#[test]
fn sealing_keeps_a_function_header_on_its_actual_local_frame() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, types, _) = setup(&cancellation, 1, 0);
    let binder = GenericBinder::new(1, 1, 1);
    let shape = context.lexical_scope().with_binder(binder);
    let item = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(shape.bound_type(0, 0).unwrap())),
        len: ArrayLength::Generic(shape.bound_const(0, 0).unwrap()),
    };
    let effect = shape.bound_effect(0, 0).unwrap();
    let predicate = crate::effect_row::EffectFormula::variable(effect.clone(), &mut context)
        .unwrap()
        .subset(&crate::effect_row::EffectFormula::empty(), &mut context)
        .unwrap();
    assert!(!predicate.is_unconstrained());
    let function = context.fixture_operand(TypeKind::function_with_contract(
        binder,
        predicate,
        [item],
        TypeKind::String,
        crate::effect_row::EffectRow::open(crate::effects::EffectSet::new(), effect),
    ));
    path.bindings.insert(types[0].clone(), function.clone());
    let sealed = seal_path(path, &mut context).unwrap();
    let value = &sealed.bindings[&types[0]];
    assert_eq!(value.value(), function.value());
    let ConstraintOperandOrigin::Rebuilt {
        source,
        contents_scope,
        children,
        length: None,
        header: None,
    } = value.origin.as_ref()
    else {
        panic!("the header retains its source and actual local frame")
    };
    let ConstraintOperandOrigin::Projection {
        root,
        occurrence_scope,
        ..
    } = source.as_ref()
    else {
        panic!("the source function owns the unchanged header")
    };
    assert!(Arc::ptr_eq(root, &function.value));
    assert_eq!(occurrence_scope, &function.scope);
    assert_eq!(contents_scope.shape(), &shape);
    assert_ne!(contents_scope, &function.scope);
    assert_eq!(children.len(), 2);
    assert!(children.iter().all(|child| child.scope == *contents_scope));
    assert!(context.lexical_scope().binders().is_empty());
    let foreign = context.enter_binder_scope(binder);
    let foreign_scope = context.capture_lexical_scope().unwrap();
    assert_eq!(foreign_scope.shape(), contents_scope.shape());
    assert_ne!(&foreign_scope, contents_scope.as_ref());
    context.restore_binder_scope(foreign);
}

#[test]
fn sealing_rejects_a_forged_stored_root_before_adopting_its_scope() {
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path, types, _) = setup(&cancellation, 1, 0);
    let outer = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let foreign = context.fixture_operand(TypeKind::I64);
    context.restore_binder_scope(outer);
    path.bindings.insert(types[0].clone(), foreign);
    assert!(matches!(
        seal_path(path, &mut context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
    assert!(context.lexical_scope().binders().is_empty());
}
