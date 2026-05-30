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

#[test]
fn spare_scopes_do_not_affect_runtime_env_semantics() {
    let mut env = RuntimeEnv::default();
    env.push_scope_with_capacity(2);
    env.set("scoped", RuntimeValue::Int(1));
    env.pop_scope();

    let baseline = RuntimeEnv::default();
    assert_eq!(env, baseline);
    assert_eq!(env.clone(), baseline);

    env.push_scope_with_capacity(1);
    assert!(env.get("scoped").is_none());
}
