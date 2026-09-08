use std::collections::BTreeSet;

use arcweft_presentation::rich_text::{
    RichTextDirectStyle, RichTextDirectStyleProperty, RichTextLayoutProperty,
    RichTextLayoutSelector, RichTextObjectProperty, RichTextObjectSelector, RichTextStyleProperty,
    RichTextStyleSelector, RichTextTransformProperty, RichTextTransformSelector,
};
use arcweft_rich_text_schema::{
    PropertyPresence, RichTextDefaultValue, RichTextPropertySetSchema, RichTextUnit,
    RichTextValueKind,
};

fn assert_property_set_is_unique<P>(schema: &RichTextPropertySetSchema<P>)
where
    P: Copy + Eq + Ord + std::fmt::Debug,
{
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();

    for property in schema.properties {
        assert!(
            ids.insert(property.id),
            "duplicate property id: {:?}",
            property.id
        );
        assert!(
            names.insert(property.source_name),
            "duplicate property name: {}",
            property.source_name
        );
    }
}

#[test]
fn presentation_owner_inventories_round_trip_through_canonical_names() {
    for owner in RichTextDirectStyle::ALL {
        assert_eq!(
            RichTextDirectStyle::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_property_set_is_unique(owner.property_schema());
    }
    for owner in RichTextStyleSelector::ALL {
        assert_eq!(
            RichTextStyleSelector::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_property_set_is_unique(owner.property_schema());
    }
    for owner in RichTextLayoutSelector::ALL {
        assert_eq!(
            RichTextLayoutSelector::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_property_set_is_unique(owner.property_schema());
    }
    for owner in RichTextTransformSelector::ALL {
        assert_eq!(
            RichTextTransformSelector::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_property_set_is_unique(owner.property_schema());
    }
    for owner in RichTextObjectSelector::ALL {
        assert_eq!(
            RichTextObjectSelector::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_property_set_is_unique(owner.property_schema());
    }

    for property in RichTextDirectStyleProperty::ALL {
        assert_eq!(
            RichTextDirectStyleProperty::from_source_name(property.source_name()),
            Some(property)
        );
    }
    for property in RichTextStyleProperty::ALL {
        assert_eq!(
            RichTextStyleProperty::from_source_name(property.source_name()),
            Some(property)
        );
    }
    for property in RichTextLayoutProperty::ALL {
        assert_eq!(
            RichTextLayoutProperty::from_source_name(property.source_name()),
            Some(property)
        );
    }
    for property in RichTextTransformProperty::ALL {
        assert_eq!(
            RichTextTransformProperty::from_source_name(property.source_name()),
            Some(property)
        );
    }
    for property in RichTextObjectProperty::ALL {
        assert_eq!(
            RichTextObjectProperty::from_source_name(property.source_name()),
            Some(property)
        );
    }
}

#[test]
fn grammar_owned_selector_spellings_resolve_without_property_aliases() {
    assert_eq!(RichTextDirectStyle::from_source_name("i"), None);
    assert_eq!(RichTextDirectStyle::from_source_name("rb"), None);
    assert_eq!(RichTextStyleSelector::from_source_name("alpha"), None);
    assert_eq!(RichTextLayoutSelector::from_source_name("vertical"), None);
    assert_eq!(RichTextTransformSelector::from_source_name("pos"), None);

    assert_eq!(RichTextStyleSelector::from_source_name("meta"), None);
    assert_eq!(RichTextStyleSelector::from_source_name("metadata"), None);
    assert_eq!(RichTextStyleSelector::from_source_name("data"), None);
    assert_eq!(RichTextStyleProperty::from_source_name("alpha"), None);
    assert_eq!(
        RichTextStyleProperty::from_source_name("object_layer"),
        None
    );
    assert_eq!(RichTextLayoutProperty::from_source_name("strictness"), None);
    assert_eq!(RichTextLayoutProperty::from_source_name("gap"), None);
    assert_eq!(RichTextTransformProperty::from_source_name("start"), None);
    assert_eq!(RichTextTransformProperty::from_source_name("glyph"), None);
    assert_eq!(RichTextObjectProperty::from_source_name("struct"), None);
    assert_eq!(RichTextObjectProperty::from_source_name("z_index"), None);
}

#[test]
fn direct_style_schemas_preserve_required_values_defaults_and_limits() {
    let oblique = RichTextDirectStyle::Oblique.property_schema();
    assert_eq!(
        oblique.properties[0].presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::AngleMilliDegrees(0))
    );
    let angle_limits = oblique.properties[0]
        .limits
        .numeric
        .expect("oblique angle limits");
    assert_eq!(angle_limits.inclusive_min_milli, Some(-89_999));
    assert_eq!(angle_limits.inclusive_max_milli, Some(89_999));

    let size = RichTextDirectStyle::Size.property_schema();
    assert_eq!(size.properties[0].id, RichTextDirectStyleProperty::Value);
    assert_eq!(size.properties[0].kind, RichTextValueKind::Length);
    assert_eq!(size.properties[0].presence, PropertyPresence::Required);
    assert_eq!(size.properties[0].limits.units, [RichTextUnit::Pt]);

    let font = RichTextDirectStyle::Font.property_schema();
    assert_eq!(font.properties[0].kind, RichTextValueKind::Text);
    assert_eq!(
        font.properties[0].limits.enum_values,
        ["serif", "sans-serif", "monospace", "cursive", "fantasy"]
    );
    assert_eq!(font.properties[0].limits.max_decoded_bytes, 256);

    let ruby = RichTextDirectStyle::Ruby.property_schema();
    assert_eq!(ruby.properties[0].id, RichTextDirectStyleProperty::RubyText);
    assert_eq!(ruby.properties[0].source_name, "rt");
    assert_eq!(ruby.properties[0].limits.max_decoded_bytes, 4_096);
}

#[test]
fn layout_and_transform_defaults_are_owner_typed_and_ordered() {
    let horizontal = RichTextLayoutSelector::HorizontalTb.property_schema();
    assert_eq!(
        horizontal.properties.len(),
        RichTextLayoutProperty::ALL.len()
    );
    assert_eq!(
        horizontal.properties[0].presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(0))
    );
    assert_eq!(
        horizontal.properties[3].presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::Length {
            milli: 8_000,
            unit: RichTextUnit::Px,
        })
    );
    assert_eq!(
        RichTextLayoutSelector::Direction
            .property_schema()
            .properties[0]
            .presence,
        PropertyPresence::Required
    );
    assert_eq!(
        horizontal.properties[4]
            .limits
            .numeric
            .expect("ruby size limits")
            .inclusive_min_milli,
        Some(1)
    );

    let offset = RichTextTransformSelector::Offset.property_schema();
    assert_eq!(
        offset.properties[2].presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(1))
    );
    assert_eq!(
        RichTextTransformSelector::Rotate
            .property_schema()
            .properties[2]
            .presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::EnumVariant(2))
    );
    assert_eq!(
        RichTextTransformSelector::Scale
            .property_schema()
            .properties[0]
            .presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::Milli(1_000))
    );
}

#[test]
fn object_schema_has_only_canonical_metadata_and_no_fallback_identity() {
    let object = RichTextObjectSelector::Object.property_schema();
    assert_eq!(
        object
            .properties
            .iter()
            .map(|property| property.id)
            .collect::<Vec<_>>(),
        RichTextObjectProperty::ALL
    );
    assert_eq!(
        object.properties[3].presence,
        PropertyPresence::Defaulted(RichTextDefaultValue::Bool(false))
    );
    assert_eq!(RichTextObjectProperty::from_source_name("id"), None);
    assert_eq!(RichTextObjectProperty::from_source_name("name"), None);
    assert_eq!(RichTextObjectProperty::from_source_name("proxy"), None);
    assert_eq!(RichTextObjectProperty::from_source_name("hit"), None);
}
