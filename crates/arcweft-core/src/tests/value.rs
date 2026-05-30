use crate::value::{RuntimeBinding, RuntimeEnv, RuntimeValue};

#[test]
fn root_binding_ref_updates_existing_slots() {
    let mut env = RuntimeEnv::default();
    let first = [RuntimeBinding {
        name: "seed".to_owned(),
        value: RuntimeValue::Int(1),
    }];
    let second = [RuntimeBinding {
        name: "seed".to_owned(),
        value: RuntimeValue::Int(2),
    }];

    env.bind_all_root_ref(&first);
    env.bind_all_root_ref(&second);

    assert_eq!(env.get("seed"), Some(&RuntimeValue::Int(2)));
}

#[test]
fn root_binding_ref_reuses_matching_ordered_slots() {
    let mut env = RuntimeEnv::default();
    let first = [
        RuntimeBinding {
            name: "lhs".to_owned(),
            value: RuntimeValue::Int(1),
        },
        RuntimeBinding {
            name: "rhs".to_owned(),
            value: RuntimeValue::Int(2),
        },
    ];
    let second = [
        RuntimeBinding {
            name: "lhs".to_owned(),
            value: RuntimeValue::Int(3),
        },
        RuntimeBinding {
            name: "rhs".to_owned(),
            value: RuntimeValue::Int(4),
        },
    ];

    env.bind_all_root_ref(&first);
    env.bind_all_root_ref(&second);

    assert_eq!(env.get("lhs"), Some(&RuntimeValue::Int(3)));
    assert_eq!(env.get("rhs"), Some(&RuntimeValue::Int(4)));
}
