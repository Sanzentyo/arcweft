use arcweft_character::id::CharacterId;
use arcweft_core::pattern::RuntimeSemanticTypeIdentityEncoder;
use arcweft_lang_syntax::{
    ast::{
        module_path::ModulePathRoot,
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
    },
    types::TypePath,
};

use crate::{
    env::{
        identity::EnvironmentBindingId,
        nominal::{AcceptedNominalId, AcceptedNominalOwnerId},
    },
    registration::StandardStatementIngressTypeId,
    types::{AcceptedNominalType, CharacterDialogueType, TypeKind},
};

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
enum EncodingStop {
    #[error("encoding cancelled")]
    Cancelled,
    #[error("encoding node limit reached")]
    Nodes,
    #[error("encoding depth {0} exceeds the bound")]
    Depth(u64),
}

struct EncodingBudget {
    nodes: Vec<(crate::types::TypeProjectionNodeKind, u64)>,
    limit: usize,
    depth: u64,
    bindings: usize,
    cancelled: bool,
}

impl EncodingBudget {
    fn new(limit: usize, depth: u64) -> Self {
        Self {
            nodes: Vec::new(),
            limit,
            depth,
            bindings: 0,
            cancelled: false,
        }
    }
}

impl crate::types::TypeProjectionControl for EncodingBudget {
    type Error = EncodingStop;

    fn check(&mut self) -> Result<(), Self::Error> {
        if self.cancelled {
            Err(EncodingStop::Cancelled)
        } else {
            Ok(())
        }
    }

    fn visit_node(
        &mut self,
        kind: crate::types::TypeProjectionNodeKind,
        depth: u64,
    ) -> Result<(), Self::Error> {
        if depth > self.depth {
            return Err(EncodingStop::Depth(depth));
        }
        if self.nodes.len() == self.limit {
            return Err(EncodingStop::Nodes);
        }
        self.nodes.push((kind, depth));
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.bindings += 1;
        Ok(())
    }
}

#[test]
fn encoding_visits_constants_effects_and_exact_depth_before_finishing() {
    use crate::types::{ArrayLength, GenericScope, TypeProjectionError, TypeProjectionNodeKind::*};
    let ty = TypeKind::function_with_effects(
        [TypeKind::I32, TypeKind::I64],
        TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::Const(3),
        },
        crate::effect_row::EffectRow::closed(
            crate::effects::EffectSet::from_labels(["fs.read", "fs.write"]).expect("effects"),
        ),
    );
    let mut exact = EncodingBudget::new(9, 3);
    assert_eq!(
        ty.semantic_identity_digest_in_scope_with_control(&GenericScope::default(), &mut exact)
            .expect("exact bound"),
        ty.semantic_identity_digest().expect("unmetered identity"),
    );
    assert_eq!(
        exact.nodes,
        [
            (Type, 1),
            (Type, 2),
            (Type, 2),
            (Type, 2),
            (Type, 3),
            (Const, 3),
            (Effect, 2),
            (Effect, 3),
            (Effect, 3)
        ]
    );
    let mut limited = EncodingBudget::new(8, 3);
    assert!(matches!(
        ty.semantic_identity_digest_in_scope_with_control(&GenericScope::default(), &mut limited),
        Err(TypeProjectionError::Control(EncodingStop::Nodes)),
    ));
    assert_eq!(limited.nodes.len(), 8);
    assert_eq!(limited.bindings, 0);
}

#[test]
fn scoped_constant_encoding_preserves_bytes_and_reports_typed_failure() {
    use crate::types::{
        ArrayLength, GenericBinder, GenericScope, GenericScopeError, TypeInstantiationError,
        TypeProjectionError, TypeProjectionNodeKind,
    };
    let scope = GenericScope::default().with_binder(GenericBinder::new(1, 1, 0));
    let length = ArrayLength::Generic(scope.bound_const(0, 0).expect("constant slot"));
    let mut expected = vec![3];
    expected.extend_from_slice(&1_u64.to_le_bytes());
    expected.extend_from_slice(&1_u16.to_le_bytes());
    expected.extend_from_slice(&1_u16.to_le_bytes());
    expected.extend_from_slice(&0_u32.to_le_bytes());
    expected.extend_from_slice(&[1, 1]);
    expected.extend_from_slice(&0_u32.to_le_bytes());
    expected.extend_from_slice(&0_u16.to_le_bytes());
    let mut budget = EncodingBudget::new(1, 1);
    assert_eq!(
        length
            .canonical_checked_bytes_in_scope_with_control(&scope, &mut budget)
            .expect("encoded constant"),
        expected
    );
    assert_eq!(
        length
            .canonical_checked_bytes_in_scope(&scope)
            .expect("default encoding"),
        expected
    );
    assert_eq!(budget.bindings, 1);
    assert_eq!(budget.nodes, [(TypeProjectionNodeKind::Const, 1)]);
    assert!(matches!(
        length.canonical_checked_bytes_in_scope_with_control(&scope, &mut budget),
        Err(TypeProjectionError::Control(EncodingStop::Nodes)),
    ));
    assert!(matches!(
        length.canonical_checked_bytes(),
        Err(TypeInstantiationError::Scope(
            GenericScopeError::UnknownDepth { depth: 0 }
        )),
    ));
    assert!(matches!(
        ArrayLength::Inferred.canonical_checked_bytes(),
        Err(TypeInstantiationError::UnresolvedType)
    ));
}

#[test]
fn effect_row_identity_uses_the_same_borrowed_version_one_encoding() {
    use crate::effect_row::EffectRow;
    use crate::types::{GenericScope, TypeProjectionError};
    for row in [
        EffectRow::closed(crate::effects::EffectSet::new()),
        EffectRow::closed(
            crate::effects::EffectSet::from_labels(["fs.read", "fs.write"]).expect("effects"),
        ),
    ] {
        let ty = TypeKind::function_with_effects([], TypeKind::Unit, row.clone());
        let mut type_budget = EncodingBudget::new(100, 3);
        let mut row_budget = EncodingBudget::new(100, 3);
        assert_eq!(
            row.semantic_identity_digest(),
            ty.semantic_identity_digest().expect("canonical function")
        );
        assert_eq!(
            row.semantic_identity_digest_with_control(&mut row_budget)
                .expect("controlled row"),
            ty.semantic_identity_digest_in_scope_with_control(
                &GenericScope::default(),
                &mut type_budget
            )
            .expect("controlled function"),
        );
        assert_eq!(row_budget.nodes, type_budget.nodes);
        let mut limited = EncodingBudget::new(2, 3);
        assert!(matches!(
            row.semantic_identity_digest_with_control(&mut limited),
            Err(TypeProjectionError::Control(EncodingStop::Nodes))
        ));
    }
}

#[test]
fn nested_payload_hashes_keep_their_insertion_depth() {
    use crate::types::{
        GenericScope, TypeProjectionError, VariantPayloadOwnerFamily, VariantPayloadType,
        VariantPayloadTypeShape,
    };
    let payload = |value: TypeKind| {
        TypeKind::VariantPayload(Box::new(VariantPayloadType::from_prepared_case(
            VariantPayloadOwnerFamily::Option,
            TypeKind::Option(Box::new(value.clone())),
            1,
            VariantPayloadTypeShape::Tuple(vec![value].into_boxed_slice()),
        )))
    };
    let ty = payload(payload(TypeKind::Bool));
    let mut limited = EncodingBudget::new(100, 3);
    assert!(matches!(
        ty.semantic_identity_digest_in_scope_with_control(&GenericScope::default(), &mut limited),
        Err(TypeProjectionError::Control(EncodingStop::Depth(4))),
    ));
    let mut complete = EncodingBudget::new(100, 5);
    assert_eq!(
        ty.semantic_identity_digest_in_scope_with_control(&GenericScope::default(), &mut complete)
            .expect("nested hashes"),
        ty.semantic_identity_digest().expect("same identity"),
    );
    assert_eq!(
        complete.nodes.iter().map(|(_, depth)| *depth).max(),
        Some(5)
    );
}

#[test]
fn deep_encoding_is_iterative_and_cancellation_returns_no_identity() {
    use crate::types::{GenericScope, TypeProjectionError};
    let mut ty = (0..10_000).fold(TypeKind::Bool, |ty, _| TypeKind::Vec(Box::new(ty)));
    let complete = ty.semantic_identity_digest();
    let mut budget = EncodingBudget::new(100_000, 32);
    let limited =
        ty.semantic_identity_digest_in_scope_with_control(&GenericScope::default(), &mut budget);
    // Drop this deliberately extreme input without recursive TypeKind drop.
    while let TypeKind::Vec(inner) = ty {
        ty = *inner;
    }
    assert!(complete.is_ok());
    assert!(matches!(
        limited,
        Err(TypeProjectionError::Control(EncodingStop::Depth(33)))
    ));
    assert_eq!(budget.nodes.len(), 32);
    budget.cancelled = true;
    assert!(matches!(
        TypeKind::Unit
            .semantic_identity_digest_in_scope_with_control(&GenericScope::default(), &mut budget),
        Err(TypeProjectionError::Control(EncodingStop::Cancelled)),
    ));
    assert_eq!(budget.nodes.len(), 32);
}

fn path(name: &str) -> TypePath {
    ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        [ProjectSymbolSegment::try_new(name).expect("segment")],
    )
    .expect("path")
    .into()
}

#[test]
fn nested_scopes_and_payload_hashes_keep_version_one_identity() {
    use crate::types::{
        ArrayLength, GenericBinder, GenericScope, VariantPayloadOwnerFamily, VariantPayloadType,
        VariantPayloadTypeShape,
    };
    let binder = GenericBinder::new(1, 1, 0);
    let scope = GenericScope::default().with_binder(binder);
    let local = TypeKind::GenericParam(scope.bound_type(0, 0).expect("type slot"));
    let payload = TypeKind::VariantPayload(Box::new(VariantPayloadType::from_prepared_case(
        VariantPayloadOwnerFamily::Option,
        TypeKind::Option(Box::new(local.clone())),
        1,
        VariantPayloadTypeShape::Tuple(
            vec![
                local.clone(),
                TypeKind::Array {
                    item: Box::new(TypeKind::Bool),
                    len: ArrayLength::Generic(scope.bound_const(0, 0).expect("constant slot")),
                },
            ]
            .into_boxed_slice(),
        ),
    )));
    let nested = TypeKind::function_with_binder(
        GenericBinder::new(1, 0, 0),
        [local.clone()],
        local,
        crate::effect_row::EffectRow::closed(crate::effects::EffectSet::new()),
    );
    let ty = TypeKind::function_with_binder(
        binder,
        [payload.clone(), nested],
        TypeKind::Projection {
            subject: Box::new(TypeKind::Vec(Box::new(TypeKind::I32))),
            trait_name: Some("Sequence".into()),
            assoc: "Item".into(),
        },
        crate::effect_row::EffectRow::closed(
            crate::effects::EffectSet::from_labels(["fs.read", "fs.write"]).expect("effects"),
        ),
    );
    assert_eq!(
        ty.semantic_identity_digest()
            .expect("root identity")
            .as_bytes(),
        &[
            32, 198, 75, 26, 128, 87, 70, 74, 191, 29, 239, 213, 206, 229, 68, 122, 224, 117, 46,
            52, 192, 4, 25, 235, 212, 168, 137, 17, 115, 98, 232, 153,
        ]
    );
    assert_eq!(
        payload
            .semantic_identity_digest_in_scope(&scope)
            .expect("scoped identity")
            .as_bytes(),
        &[
            170, 18, 151, 22, 239, 57, 52, 8, 64, 177, 251, 146, 110, 178, 127, 25, 49, 88, 22, 37,
            98, 34, 125, 55, 78, 153, 246, 71, 176, 111, 176, 109,
        ]
    );
}

#[test]
fn accepted_owner_and_nested_arguments_participate_in_identity() {
    let first = TypeKind::AcceptedNominal(AcceptedNominalType::new(
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::Environment(
                EnvironmentBindingId::try_new("adapter:first").expect("owner"),
            ),
            path("Value"),
        ),
        [TypeKind::Vec(Box::new(TypeKind::I32))],
    ));
    let owner_changed = TypeKind::AcceptedNominal(AcceptedNominalType::new(
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::Environment(
                EnvironmentBindingId::try_new("adapter:second").expect("owner"),
            ),
            path("Value"),
        ),
        [TypeKind::Vec(Box::new(TypeKind::I32))],
    ));
    let argument_changed = TypeKind::AcceptedNominal(AcceptedNominalType::new(
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::Environment(
                EnvironmentBindingId::try_new("adapter:first").expect("owner"),
            ),
            path("Value"),
        ),
        [TypeKind::Vec(Box::new(TypeKind::I64))],
    ));

    assert_eq!(
        first.semantic_identity_digest().expect("stable test type"),
        first
            .clone()
            .semantic_identity_digest()
            .expect("stable test type")
    );
    assert_ne!(
        first.semantic_identity_digest().expect("stable test type"),
        owner_changed
            .semantic_identity_digest()
            .expect("stable test type")
    );
    assert_ne!(
        first.semantic_identity_digest().expect("stable test type"),
        argument_changed
            .semantic_identity_digest()
            .expect("stable test type")
    );
}

#[test]
fn character_dialogue_producer_and_type_kind_share_one_identity_authority() {
    let exact = CharacterDialogueType::exact(
        CharacterId::try_new("character.alice").expect("character ID"),
    );
    let any = CharacterDialogueType::any();
    assert_eq!(
        TypeKind::CharacterDialogue(exact.clone())
            .semantic_identity_digest()
            .expect("stable test type")
            .as_bytes(),
        exact.runtime_semantic_identity().as_bytes()
    );
    assert_eq!(
        TypeKind::CharacterDialogue(any.clone())
            .semantic_identity_digest()
            .expect("stable test type")
            .as_bytes(),
        any.runtime_semantic_identity().as_bytes()
    );
}

#[test]
fn statement_ingress_uses_the_reserved_outer_and_exact_inner_tags() {
    for (ingress, inner_tag) in [
        (StandardStatementIngressTypeId::TaskEvent, 0),
        (StandardStatementIngressTypeId::ScopeExit, 1),
        (StandardStatementIngressTypeId::FrameBoundary, 2),
    ] {
        let mut expected = RuntimeSemanticTypeIdentityEncoder::new();
        expected.write_tag(88);
        expected.write_u8(inner_tag);
        assert_eq!(
            TypeKind::StatementIngress(ingress)
                .semantic_identity_digest()
                .expect("stable test type")
                .as_bytes(),
            expected.finish().as_bytes()
        );
    }
}
