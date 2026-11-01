use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole as Role, PresentationCallableId as Call,
};

#[test]
fn presentation_keys_preserve_metadata_paths_and_target_domains() {
    let keys = ["param.色", "param.user.role", "proxy.param.user.role"].map(|name| {
        Call::Image
            .argument_key(name)
            .expect("metadata destination")
    });
    assert_eq!(keys[0].parameter(), Some("色"));
    assert_eq!(keys[1].parameter(), Some("user.role"));
    assert_eq!(keys[2].parameter(), Some("user.role"));
    assert_ne!(keys[0], keys[1]);
    assert_ne!(keys[1], keys[2]);
    for key in keys {
        let bytes = serde_json::to_vec(&key).unwrap();
        assert_eq!(
            serde_json::from_slice::<PresentationArgumentKey>(&bytes).unwrap(),
            key
        );
    }
    let background = Call::Background.argument_key("target").unwrap();
    let image = Call::Image.argument_key("target").unwrap();
    assert_eq!(background.role(), Role::TargetEntity);
    assert_eq!(image.role(), Role::TargetPublicId);
    assert_ne!(background, image);
    assert_eq!(
        Call::Mount.argument_key("view").unwrap().role(),
        Role::ViewValue
    );
    assert_eq!(Call::View.argument_key("view").unwrap().role(), Role::View);
}

#[test]
fn presentation_keys_reject_malformed_or_misplaced_metadata() {
    for name in [
        "param.",
        "param..role",
        "param.1role",
        "param.role.",
        "proxy.param.",
        "param.a/b",
        "param.a b",
        "proxy.param..role",
    ] {
        assert_eq!(Call::Image.argument_key(name), None, "{name}");
    }
    for call in Call::ALL.into_iter().filter(|call| *call != Call::Image) {
        assert!(call.argument_key("param.role").is_none(), "{call:?}");
        assert!(call.argument_key("proxy.param.role").is_none(), "{call:?}");
    }
    for json in [
        r#"{"role":"parameter"}"#,
        r#"{"role":"proxy_parameter","parameter":null}"#,
        r#"{"role":"parameter","parameter":""}"#,
        r#"{"role":"parameter","parameter":"user..role"}"#,
        r#"{"role":"opacity","parameter":"role"}"#,
        r#"{"role":"parameter","parameter":"role","extra":true}"#,
        r#"{"role":"parameter","parameter":"a","parameter":"b"}"#,
    ] {
        assert!(
            serde_json::from_str::<PresentationArgumentKey>(json).is_err(),
            "{json}"
        );
    }
    for role in Role::ALL {
        assert_eq!(
            PresentationArgumentKey::try_new(*role, None).is_ok(),
            !role.requires_parameter()
        );
        assert_eq!(
            PresentationArgumentKey::try_new(*role, Some("user.role".into())).is_ok(),
            role.requires_parameter()
        );
    }
}

#[test]
fn role_wire_coordinates_roundtrip_without_changing_the_callable_grammar() {
    let mut tags = std::collections::BTreeSet::new();
    for role in Role::ALL {
        assert!(tags.insert(role.semantic_tag()));
        assert_eq!(Role::from_semantic_tag(role.semantic_tag()), Some(*role));
        for call in Call::ALL {
            let name = if role.requires_parameter() {
                format!("{}user.role", role.source_name())
            } else {
                role.source_name().to_owned()
            };
            if call.accepts_argument_role(*role) {
                assert_eq!(
                    call.argument_key(&name).unwrap().role(),
                    *role,
                    "{call:?}: {name}"
                );
            }
        }
    }
    assert_eq!(Role::from_semantic_tag(52), None);
    for call in [Call::RefBackground, Call::RefShow] {
        assert!(call.argument_key("scope").is_none());
    }
}
