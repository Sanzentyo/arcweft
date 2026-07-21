use std::collections::BTreeSet;

use arcweft_dialogue::rich_text::{
    DialogueControlProperty, DialogueHostEventKind, DialogueHostProperty, DialogueRichTextControl,
};
use arcweft_rich_text_schema::{
    CheckedOutputKind, PropertyPresence, RichTextDefaultValue, RichTextSourceForm,
    RichTextTagSchema, RichTextUnit, RichTextValueKind, SelectorContract, UnknownPropertyPolicy,
};

fn assert_schema_properties_are_unique<P>(schema: &RichTextTagSchema<P>)
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
fn dialogue_control_inventory_round_trips_and_rejects_removed_property_names() {
    for owner in DialogueRichTextControl::ALL {
        assert_eq!(
            DialogueRichTextControl::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_eq!(owner.schema().unknown_policy, UnknownPropertyPolicy::Reject);
        assert_schema_properties_are_unique(owner.schema());
    }

    for property in DialogueControlProperty::ALL {
        assert_eq!(
            DialogueControlProperty::from_source_name(property.source_name()),
            Some(property)
        );
    }

    assert_eq!(
        DialogueRichTextControl::from_source_name("page"),
        Some(DialogueRichTextControl::Page)
    );
    assert_eq!(
        DialogueRichTextControl::from_source_name("wait"),
        Some(DialogueRichTextControl::LineWait)
    );
    assert_eq!(
        DialogueRichTextControl::from_source_name("nl"),
        Some(DialogueRichTextControl::HardBreak)
    );
    assert_eq!(
        DialogueRichTextControl::from_source_name("cm"),
        Some(DialogueRichTextControl::Clear)
    );
    assert_eq!(DialogueControlProperty::from_source_name("speed"), None);
    assert_eq!(DialogueControlProperty::from_source_name("value"), None);
}

#[test]
fn dialogue_control_schemas_preserve_exact_limits_and_outputs() {
    let timed_wait = DialogueRichTextControl::TimedWait.schema();
    assert_eq!(timed_wait.output, CheckedOutputKind::PointControl);
    assert_eq!(timed_wait.selector, SelectorContract::None);
    assert_eq!(timed_wait.properties.len(), 1);
    assert_eq!(timed_wait.properties[0].id, DialogueControlProperty::Time);
    assert_eq!(
        timed_wait.properties[0].presence,
        PropertyPresence::Required
    );
    assert_eq!(timed_wait.properties[0].kind, RichTextValueKind::Duration);
    assert_eq!(
        timed_wait.properties[0].limits.units,
        [RichTextUnit::Ms, RichTextUnit::S]
    );
    assert_eq!(
        timed_wait.properties[0]
            .limits
            .numeric
            .expect("duration limits")
            .inclusive_max_milli,
        Some(86_400_000_000)
    );

    let reveal = DialogueRichTextControl::RevealRate.schema();
    assert_eq!(reveal.properties[0].id, DialogueControlProperty::Cps);
    assert_eq!(
        reveal.properties[0].limits.enum_values,
        ["slow", "normal", "fast"]
    );

    let marker = DialogueRichTextControl::Marker.schema();
    assert_eq!(marker.output, CheckedOutputKind::Marker);
    assert!(matches!(
        marker.selector,
        SelectorContract::RequiredPositional { .. }
    ));
    assert!(marker.properties.is_empty());
}

#[test]
fn dialogue_host_inventory_owns_only_current_names() {
    for owner in DialogueHostEventKind::ALL {
        assert_eq!(
            DialogueHostEventKind::from_source_name(owner.canonical_name()),
            Some(owner)
        );
        assert_eq!(owner.schema().output, CheckedOutputKind::Host);
        assert_eq!(owner.schema().unknown_policy, UnknownPropertyPolicy::Reject);
        assert_schema_properties_are_unique(owner.schema());
    }

    for property in DialogueHostProperty::ALL {
        if property != DialogueHostProperty::At && property != DialogueHostProperty::Call {
            assert_eq!(
                DialogueHostProperty::from_source_name(property.source_name()),
                Some(property)
            );
        }
    }

    assert_eq!(
        DialogueHostEventKind::from_source_name("!"),
        Some(DialogueHostEventKind::Call)
    );
    assert_eq!(DialogueHostProperty::from_source_name("at"), None);
    assert_eq!(DialogueHostProperty::from_source_name("call"), None);
    assert_eq!(DialogueHostProperty::from_source_name("attrs"), None);
    assert_eq!(DialogueHostProperty::from_source_name("value"), None);
}

#[test]
fn dialogue_host_schemas_retain_cross_field_inputs_without_guessing_them() {
    let voice = DialogueHostEventKind::Voice.schema();
    assert_eq!(voice.properties[0].kind, RichTextValueKind::PublicId);
    assert_eq!(voice.properties[0].limits.enum_values, ["auto"]);

    let move_schema = DialogueHostEventKind::Move.schema();
    assert_eq!(move_schema.properties.len(), 2);
    for property in move_schema.properties {
        assert_eq!(
            property.presence,
            PropertyPresence::Defaulted(RichTextDefaultValue::Length {
                milli: 0,
                unit: RichTextUnit::Px,
            })
        );
    }

    let scale = DialogueHostEventKind::Scale.schema();
    assert_eq!(scale.properties[0].presence, PropertyPresence::Required);
    assert_eq!(scale.properties[1].presence, PropertyPresence::Optional);

    let call = DialogueHostEventKind::Call.schema();
    assert!(call.properties.is_empty());
    assert!(
        call.source_forms
            .contains(&RichTextSourceForm::DedicatedPayload)
    );

    let conditional = DialogueHostEventKind::ConditionalStart.schema();
    assert!(
        conditional
            .source_forms
            .contains(&RichTextSourceForm::DedicatedPayload)
    );

    let timed_cue = DialogueHostEventKind::TimedCue.schema();
    assert!(
        timed_cue
            .source_forms
            .contains(&RichTextSourceForm::DedicatedPayload)
    );
    assert_eq!(
        timed_cue
            .properties
            .iter()
            .map(|property| property.id)
            .collect::<Vec<_>>(),
        [DialogueHostProperty::At]
    );
}
