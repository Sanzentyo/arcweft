use crate::{
    engine::evaluate_runtime_call,
    pure::{RuntimeExternalCallContext, VmRuntimePureCallBackend},
    task::NeedId,
    value::{RuntimeCallTarget, RuntimeIntrinsic, RuntimeIterator, RuntimeSeq, RuntimeValue},
};

fn call(intrinsic: RuntimeIntrinsic, args: Vec<RuntimeValue>) -> RuntimeValue {
    evaluate_runtime_call(
        &RuntimeCallTarget::intrinsic(intrinsic),
        args,
        &RuntimeExternalCallContext::unbound(),
        &mut VmRuntimePureCallBackend::default(),
    )
    .expect("typed core intrinsic accepts its affine argument")
}

fn need_pointer(value: &RuntimeValue) -> *const u8 {
    let RuntimeValue::NeedHandle(need) = value else {
        panic!("expected the affine Need payload")
    };
    need.spec() as *const crate::task::TaskSpec as *const u8
}

#[test]
fn consuming_iterator_intrinsics_move_the_same_affine_payload() {
    let item = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.intrinsic.owner"));
    let original = need_pointer(&item);
    let source = RuntimeValue::Seq(RuntimeSeq::Values(vec![item]));

    let iterator = call(RuntimeIntrinsic::CoreVecIntoIter, vec![source]);
    let RuntimeValue::Iterator(RuntimeIterator::Values { items }) = &iterator else {
        panic!("Vec.into_iter retains an owned value iterator")
    };
    assert_eq!(
        need_pointer(items.front().expect("one remaining item")),
        original
    );

    let next = call(RuntimeIntrinsic::CoreIterNext, vec![iterator]);
    let RuntimeValue::Tuple(mut next) = next else {
        panic!("Iterator.next returns iterator and Option item")
    };
    let option = next.pop().expect("Option result");
    assert!(matches!(next.pop(), Some(RuntimeValue::Iterator(_))));
    let item = call(RuntimeIntrinsic::CoreOptionUnwrap, vec![option]);
    assert_eq!(need_pointer(&item), original);

    let collected = call(
        RuntimeIntrinsic::CoreIterCollect,
        vec![RuntimeValue::Seq(RuntimeSeq::Values(vec![item]))],
    );
    let RuntimeValue::Seq(RuntimeSeq::Values(items)) = collected else {
        panic!("collect returns a value sequence")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(need_pointer(&items[0]), original);
}
