use super::*;

use arcweft_id::PublicId;
use arcweft_lang_syntax::attachment::{
    AttachedResourceDeclaration, AttachedResourceInitializer, AttachedResourcePublicId,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::expr::HirExprKind;
use crate::item::{HirResourceDeclaration, HirResourceField};
use crate::type_ref::HirTypeKind;

use super::super::nominal::preflight_resource_fields;

fn resource(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirResourceDeclaration) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Resource(resource) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Resource")
    };
    (owner, item, resource)
}

fn attached_resource(parsed: &ParsedSource) -> AttachedResourceDeclaration {
    parsed
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Resource(resource) => Some(resource.semantics().unwrap()),
            _ => None,
        })
        .expect("one attached Resource declaration")
}

fn assert_resource_freeze_rejects(
    case: &str,
    tamper: impl FnOnce(&HirResourceDeclaration) -> HirResourceDeclaration,
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-resource-{case}"),
        concat!(
            "res @image.room room: Image {\n",
            "    asset = @asset.bg.room\n",
            "    visible = true\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.source_ordered_items[0];
    let (slots, arenas) = transaction.storage_mut();
    let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
    let HirItemKind::Resource(resource) = original.kind() else {
        panic!("final Resource item")
    };
    let replacement = HirItem::try_new_with_state(
        owner,
        original.scope(),
        original.prefix().clone(),
        HirItemKind::Resource(tamper(resource)),
        original.members().into(),
        *original.state(),
    )
    .unwrap();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

fn assert_clean_resource_header(item: &HirItem, resource: &HirResourceDeclaration) {
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(
        item.prefix()
            .documentation()
            .expect("Resource documentation")
            .markdown(),
        "Configured room image"
    );
    assert_eq!(
        item.prefix().visibility(),
        Some(crate::item::HirVisibility::Public)
    );
    assert_eq!(item.prefix().attributes().len(), 1);
    assert_eq!(
        resource.public_id().map(PublicId::as_str),
        Some("image.room")
    );
    assert!(matches!(
        resource.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "room"
    ));
}

#[test]
fn clean_resource_freezes_prefix_public_id_type_and_ordered_expression_fields() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-resource-clean",
        concat!(
            "/// Configured room image\n",
            "#[tool.fixture]\n",
            "pub res @image.room room: std.presentation.Image {\n",
            "    asset = @asset.bg.room\n",
            "    visible = true\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let attached = attached_resource(&parsed);
    let public_id_syntax = attached
        .public_id()
        .syntax()
        .expect("explicit Resource PublicId syntax")
        .id();
    let initializer_syntax = attached
        .body()
        .fields()
        .iter()
        .map(|field| field.initializer().authored().unwrap().id())
        .collect::<Vec<_>>();

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, resource) = resource(&module, 0);

    assert_clean_resource_header(item, resource);
    let resource_type = module
        .arenas()
        .types()
        .resolve(module.slots(), resource.resource_type())
        .unwrap();
    assert_eq!(resource_type.scope(), item.scope());
    let HirTypeKind::Path(path) = resource_type.kind() else {
        panic!("Resource nominal head must lower as a typed path")
    };
    assert_eq!(path_spellings(path), ["std", "presentation", "Image"]);

    assert_eq!(resource.fields().len(), 2);
    for (position, (field, syntax)) in resource.fields().iter().zip(initializer_syntax).enumerate()
    {
        let expected = ["asset", "visible"][position];
        assert!(matches!(
            field.name(),
            HirRequiredName::Resolved(name) if name.as_str() == expected
        ));
        let expression = module
            .arenas()
            .expressions()
            .resolve(module.slots(), field.value())
            .unwrap();
        assert_eq!(expression.scope(), item.scope());
        assert_source_backed_child(&module, field.value());
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(syntax),
            Some(field.value())
        );
    }
    assert!(matches!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), resource.fields()[0].value())
            .unwrap()
            .kind(),
        HirExprKind::EntityReference(_)
    ));
    assert!(matches!(
        module
            .arenas()
            .expressions()
            .resolve(module.slots(), resource.fields()[1].value())
            .unwrap()
            .kind(),
        HirExprKind::Literal(_)
    ));
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(public_id_syntax),
        None,
        "declaration PublicId must not become an executable expression"
    );
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn resource_field_recovery_omits_only_unowned_values_without_fabricating_expr_ids() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-resource-field-recovery",
        concat!(
            "res room: Image {\n",
            "    asset @asset.bg.room\n",
            "    opacity =\n",
            "    visible = @\n",
            "}\n",
        ),
    );
    let attached = attached_resource(&parsed);
    let [asset, opacity, visible] = attached.body().fields() else {
        panic!("three source-ordered Resource fields")
    };
    assert!(matches!(
        asset.initializer(),
        AttachedResourceInitializer::Absent
    ));
    let opacity_missing = opacity
        .initializer()
        .authored()
        .expect("missing initializer must retain the shared projected expression owner");
    assert_eq!(
        opacity_missing.syntax().kind(),
        SyntaxKind::MissingExpression
    );
    assert_eq!(opacity_missing.projection(), &ExpressionProjection::Error);
    let opacity_missing = opacity_missing.id();
    let visible_syntax = visible
        .initializer()
        .authored()
        .expect("recovered entity reference remains authored")
        .id();

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, resource) = resource(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    let [opacity, visible] = resource.fields() else {
        panic!("missing and authored Resource values must retain field order")
    };
    assert!(matches!(
        opacity.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "opacity"
    ));
    assert!(matches!(
        visible.name(),
        HirRequiredName::Resolved(name) if name.as_str() == "visible"
    ));
    for field in [opacity, visible] {
        assert!(module.slots().resolve(field.value()).unwrap().is_poisoned());
    }
    assert_source_backed_child(&module, opacity.value());
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(opacity_missing),
        Some(opacity.value()),
        "projected MissingExpression must use the ordinary source-backed expression authority"
    );
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(visible_syntax),
        Some(visible.value())
    );
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn resource_header_recovery_matrix_stays_in_the_resource_family() {
    let cases = [
        ("implicit-id", "res room: Image { visible = true }\n", None),
        (
            "relative-id",
            "res @.room room: Image { visible = true }\n",
            Some(HirItemIssue::MalformedHeader),
        ),
        (
            "malformed-absolute-id",
            "res @image..room room: Image { visible = true }\n",
            Some(HirItemIssue::MalformedHeader),
        ),
        (
            "missing-name",
            "res : Image { visible = true }\n",
            Some(HirItemIssue::MissingName),
        ),
        (
            "missing-colon",
            "res room Image { visible = true }\n",
            Some(HirItemIssue::MalformedHeader),
        ),
        (
            "missing-type",
            "res room: { visible = true }\n",
            Some(HirItemIssue::MissingType),
        ),
        (
            "non-nominal-head",
            "res room: &Image { visible = true }\n",
            Some(HirItemIssue::MalformedHeader),
        ),
        (
            "missing-body",
            "res room: Image\n",
            Some(HirItemIssue::MissingBody),
        ),
        (
            "unclosed-body",
            "res room: Image {\n    visible = true\n",
            Some(HirItemIssue::Recovery),
        ),
        (
            "generic-structural-head",
            "res room: Image<Large> { visible = true }\n",
            None,
        ),
    ];

    for (case, source, expected_issue) in cases {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-resource-{case}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (owner, item, resource) = resource(&module, 0);

        assert_eq!(
            item.state(),
            &expected_issue.map_or(HirItemPoisonState::Clean, |issue| {
                HirItemPoisonState::Poisoned(issue)
            }),
            "{case}",
        );
        if matches!(case, "relative-id" | "malformed-absolute-id") {
            assert!(resource.public_id().is_none());
            assert!(matches!(
                attached_resource(&parsed).public_id(),
                AttachedResourcePublicId::Recovered { .. }
            ));
        }
        if case == "implicit-id" {
            assert!(resource.public_id().is_none());
            assert!(matches!(
                attached_resource(&parsed).public_id(),
                AttachedResourcePublicId::Absent
            ));
        }
        if case == "generic-structural-head" {
            let ty = module
                .arenas()
                .types()
                .resolve(module.slots(), resource.resource_type())
                .unwrap();
            assert!(matches!(ty.kind(), HirTypeKind::Generic(_)));
        }
        assert!(item.members().is_empty());
        assert!(module.declaration_members().arena(owner).is_none());
        assert_item_slot_whole(&module, &parsed, owner);
        if expected_issue.is_some() {
            assert_item_owner_whole_recovery(&module, owner);
        }
    }
}

#[test]
fn duplicate_resource_fields_preserve_source_order_for_later_semantic_rejection() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-resource-duplicate-fields",
        concat!(
            "res room: Image {\n",
            "    visible = true\n",
            "    visible = false\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, resource) = resource(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let [first, second] = resource.fields() else {
        panic!("both duplicate Resource fields must remain in HIR")
    };
    for field in [first, second] {
        assert!(matches!(
            field.name(),
            HirRequiredName::Resolved(name) if name.as_str() == "visible"
        ));
    }
    assert_ne!(first.value(), second.value());
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn resource_freeze_rejects_public_id_field_order_and_initializer_owner_tampering() {
    assert_resource_freeze_rejects("public-id-tamper", |resource| {
        HirResourceDeclaration::new(
            Some(PublicId::try_new("image.other").unwrap()),
            resource.name().clone(),
            resource.resource_type(),
            resource.fields().into(),
        )
    });
    assert_resource_freeze_rejects("field-order-tamper", |resource| {
        let mut fields = resource.fields().to_vec();
        fields.swap(0, 1);
        HirResourceDeclaration::new(
            resource.public_id().cloned(),
            resource.name().clone(),
            resource.resource_type(),
            fields.into_boxed_slice(),
        )
    });
    assert_resource_freeze_rejects("initializer-owner-tamper", |resource| {
        let [first, second] = resource.fields() else {
            panic!("two Resource fields")
        };
        let mut fields = resource.fields().to_vec();
        fields[0] = HirResourceField::new(first.name().clone(), second.value());
        HirResourceDeclaration::new(
            resource.public_id().cloned(),
            resource.name().clone(),
            resource.resource_type(),
            fields.into_boxed_slice(),
        )
    });
}

#[test]
fn resource_field_preflight_accepts_exact_and_rejects_one_over() {
    assert!(preflight_resource_fields(HirLimit::DeclarationMembers.maximum()).is_ok());
    let observed = HirLimit::DeclarationMembers.maximum() + 1;
    let Err(HirLowerFailure::Limit(error)) = preflight_resource_fields(observed) else {
        panic!("one-over Resource field inventory must fail before expression lowering")
    };
    assert_eq!(error.limit(), HirLimit::DeclarationMembers);
    assert_eq!(error.observed(), observed);
    assert_eq!(error.maximum(), HirLimit::DeclarationMembers.maximum());
}
