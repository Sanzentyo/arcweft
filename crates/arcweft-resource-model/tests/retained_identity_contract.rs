use arcweft_id::{EntityId, PublicId};
use arcweft_manifest_model::RawDigest;
use arcweft_resource_model::{
    canonical::ResourceCanonicalEncodingError,
    retained::{
        PresentationTargetScope, ResolvedRetainedIdentityRef, ResourceValuePath,
        ResourceValuePathSegment, RetainedIdentityDependency, RetainedIdentityKind,
    },
    value::{
        ResourceConstValue, ResourceReferenceRequirementKind, ResourceScalarType,
        ResourceValueType, ResourceValueTypePathSegment, ResourceValueValidationError,
    },
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

#[test]
fn retained_identity_kind_is_closed_and_uses_canonical_tokens() {
    let cases = [
        (RetainedIdentityKind::Character, "character"),
        (RetainedIdentityKind::View, "view"),
        (RetainedIdentityKind::Action, "action"),
        (RetainedIdentityKind::Layer, "layer"),
        (RetainedIdentityKind::Signal, "signal"),
        (
            RetainedIdentityKind::PresentationTarget,
            "presentation_target",
        ),
        (RetainedIdentityKind::ScrollRegion, "scroll_region"),
    ];

    assert_eq!(
        RetainedIdentityKind::ALL,
        cases.map(|(identity, _)| identity)
    );
    for (kind, token) in cases {
        assert_eq!(kind.as_str(), token);
        assert_eq!(RetainedIdentityKind::from_manifest_token(token), Some(kind));
    }
    assert_eq!(RetainedIdentityKind::from_manifest_token("Character"), None);
    assert_eq!(RetainedIdentityKind::from_manifest_token("metric"), None);
}

#[test]
fn retained_identity_expected_types_match_only_the_exact_kind() {
    let character = ResourceConstValue::RetainedIdentityRef {
        value: ResolvedRetainedIdentityRef::Character {
            entity_id: entity_id("entity.character.alice"),
        },
    };
    ResourceValueType::RetainedIdentityRef {
        identity: RetainedIdentityKind::Character,
    }
    .validate_const(&character)
    .unwrap();
    assert!(
        ResourceValueType::RetainedIdentityRef {
            identity: RetainedIdentityKind::Character,
        }
        .accepts_const_value(&character)
    );

    assert_eq!(
        ResourceValueType::RetainedIdentityRef {
            identity: RetainedIdentityKind::View,
        }
        .validate_const(&character),
        Err(ResourceValueValidationError::RetainedIdentityKindMismatch {
            expected: RetainedIdentityKind::View,
            actual: RetainedIdentityKind::Character,
        })
    );
    assert!(matches!(
        ResourceValueType::Scalar(ResourceScalarType::String).validate_const(&character),
        Err(ResourceValueValidationError::TypeMismatch { .. })
    ));
}

#[test]
fn retained_identity_value_type_transcripts_match_the_contract_vectors() {
    let vectors = [
        (
            RetainedIdentityKind::Character,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f72656609000000636861726163746572",
        ),
        (
            RetainedIdentityKind::View,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f7265660400000076696577",
        ),
        (
            RetainedIdentityKind::Action,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f72656606000000616374696f6e",
        ),
        (
            RetainedIdentityKind::Layer,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f726566050000006c61796572",
        ),
        (
            RetainedIdentityKind::Signal,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f726566060000007369676e616c",
        ),
        (
            RetainedIdentityKind::PresentationTarget,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f7265661300000070726573656e746174696f6e5f746172676574",
        ),
        (
            RetainedIdentityKind::ScrollRegion,
            "617263776566742e7265736f757263652e76616c75652d747970652e763100010000001500000072657461696e65645f6964656e746974795f7265660d0000007363726f6c6c5f726567696f6e",
        ),
    ];

    for (identity, expected_hex) in vectors {
        let value_type = ResourceValueType::RetainedIdentityRef { identity };
        let expected = decode_hex(expected_hex);
        assert_eq!(value_type.canonical_bytes_v1().unwrap(), expected);
        assert_eq!(
            value_type.canonical_digest_v1().unwrap(),
            RawDigest::for_bytes(&expected)
        );
    }
}

#[test]
fn retained_identity_constant_transcripts_match_the_contract_vectors() {
    let vectors = [
        (
            ResolvedRetainedIdentityRef::Character {
                entity_id: entity_id("entity.character.alice"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f7265660900000063686172616374657216000000656e746974792e6368617261637465722e616c696365",
        ),
        (
            ResolvedRetainedIdentityRef::View {
                entity_id: entity_id("entity.view.dialogue"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f726566040000007669657714000000656e746974792e766965772e6469616c6f677565",
        ),
        (
            ResolvedRetainedIdentityRef::Action {
                entity_id: entity_id("entity.action.submit"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f72656606000000616374696f6e14000000656e746974792e616374696f6e2e7375626d6974",
        ),
        (
            ResolvedRetainedIdentityRef::Layer {
                entity_id: entity_id("entity.layer.dialogue"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f726566050000006c6179657215000000656e746974792e6c617965722e6469616c6f677565",
        ),
        (
            ResolvedRetainedIdentityRef::Signal {
                entity_id: entity_id("entity.signal.mood"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f726566060000007369676e616c12000000656e746974792e7369676e616c2e6d6f6f64",
        ),
        (
            ResolvedRetainedIdentityRef::PresentationTarget {
                scope: PresentationTargetScope::Global,
                target_id: public_id("target.open_menu"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f7265661300000070726573656e746174696f6e5f74617267657406000000676c6f62616c100000007461726765742e6f70656e5f6d656e75",
        ),
        (
            ResolvedRetainedIdentityRef::PresentationTarget {
                scope: PresentationTargetScope::View {
                    owner_view_entity_id: entity_id("entity.view.dialogue"),
                },
                target_id: public_id("target.next"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f7265661300000070726573656e746174696f6e5f746172676574040000007669657714000000656e746974792e766965772e6469616c6f6775650b0000007461726765742e6e657874",
        ),
        (
            ResolvedRetainedIdentityRef::ScrollRegion {
                owner_view_entity_id: entity_id("entity.view.log"),
                region_id: public_id("scroll.log"),
            },
            "617263776566742e7265736f757263652e636f6e73742d76616c75652e763100010000001500000072657461696e65645f6964656e746974795f7265660d0000007363726f6c6c5f726567696f6e0f000000656e746974792e766965772e6c6f670a0000007363726f6c6c2e6c6f67",
        ),
    ];

    for (value, expected_hex) in vectors {
        let value = ResourceConstValue::RetainedIdentityRef { value };
        let expected = decode_hex(expected_hex);
        assert_eq!(value.canonical_bytes_v1().unwrap(), expected);
        assert_eq!(
            value.canonical_digest_v1().unwrap(),
            RawDigest::for_bytes(&expected)
        );
    }
}

#[test]
fn canonical_retained_encoder_does_not_claim_unfrozen_value_shapes() {
    assert_eq!(
        ResourceValueType::Scalar(ResourceScalarType::Bool).canonical_bytes_v1(),
        Err(ResourceCanonicalEncodingError::UnsupportedValueType)
    );
}

#[test]
fn retained_dependencies_keep_canonical_target_and_exact_source_occurrence() {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("main.arcw").unwrap(),
        SourceName::path("src/main.arcw"),
        "target = @action.submit",
    )
    .unwrap();
    let source = document.span(SourceRange::new(9, 23)).unwrap();
    let target = ResolvedRetainedIdentityRef::Action {
        entity_id: entity_id("entity.action.submit"),
    };
    let path = ResourceValuePath::new([
        ResourceValuePathSegment::Field(
            arcweft_resource_model::identity::ResourceFieldId::try_new(5).unwrap(),
        ),
        ResourceValuePathSegment::ListIndex(0),
    ]);
    let dependency = RetainedIdentityDependency::new(
        entity_id("entity.resource.image.logo"),
        path,
        target.clone(),
        source.clone(),
    );

    assert_eq!(dependency.target(), &target);
    assert_eq!(dependency.source(), &source);
    assert_eq!(dependency.value_path().segments().len(), 2);
    assert_eq!(
        dependency.from_resource().as_str(),
        "entity.resource.image.logo"
    );
}

#[test]
fn reference_requirement_inventory_owns_nested_structural_traversal() {
    let value_type = ResourceValueType::map(
        ResourceValueType::Scalar(ResourceScalarType::String),
        ResourceValueType::option(ResourceValueType::RetainedIdentityRef {
            identity: RetainedIdentityKind::Layer,
        }),
    );
    let requirements = value_type.reference_requirements().unwrap();

    assert_eq!(requirements.len(), 1);
    assert_eq!(
        requirements[0].path().segments(),
        [
            ResourceValueTypePathSegment::MapValue,
            ResourceValueTypePathSegment::OptionValue,
        ]
    );
    assert!(matches!(
        requirements[0].kind(),
        ResourceReferenceRequirementKind::Retained {
            identity: RetainedIdentityKind::Layer,
        }
    ));

    let mut too_deep = ResourceValueType::RetainedIdentityRef {
        identity: RetainedIdentityKind::Layer,
    };
    for _ in 0..=64 {
        too_deep = ResourceValueType::option(too_deep);
    }
    assert_eq!(
        too_deep
            .reference_requirements()
            .unwrap_err()
            .path()
            .segments()
            .len(),
        65
    );
}

fn entity_id(value: &str) -> EntityId {
    EntityId::try_new(value).unwrap()
}

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_digit(pair[0]) << 4) | hex_digit(pair[1]))
        .collect()
}

fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("test vector contains non-hex input"),
    }
}
