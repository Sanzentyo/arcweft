use std::collections::BTreeMap;

use arcweft_dialogue::rich_text::{
    DialogueControlProperty, DialogueHostEventKind, DialogueHostProperty, DialogueRichTextControl,
};
use arcweft_lang_hir::dialogue_application::{
    HirBuiltinRichTextFx, HirBuiltinRichTextTag, HirDialogueContent, HirDialogueContentError,
    HirDialogueNode, HirDialogueNodeKind, HirRichTextArgument, HirRichTextArgumentId,
    HirRichTextArgumentIssue, HirRichTextConditionalTag, HirRichTextDirectStyle,
    HirRichTextHostEvent, HirRichTextLayoutSelector, HirRichTextObjectSelector,
    HirRichTextStyleSelector, HirRichTextTag, HirRichTextTagId, HirRichTextTagIdentity,
    HirRichTextTagPayload, HirRichTextTransformSelector,
};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::source_index::{
    HirDialogueNodeSourcePart, HirExprSourceRole, HirRichTextArgumentSourcePart,
    HirRichTextTagSourcePart, HirSourcePresence, HirSourceQuery, HirSourceQueryError,
    HirSourceSite,
};
use arcweft_presentation::rich_text::{
    BuiltinPropertyDisposition, BuiltinRichTextFx, BuiltinRichTextFxPhase,
    BuiltinRichTextFxProperty, RichTextDirectStyle, RichTextDirectStyleProperty,
    RichTextLayoutProperty, RichTextLayoutSelector, RichTextObjectProperty, RichTextStyleProperty,
    RichTextStyleSelector, RichTextTransformProperty, RichTextTransformSelector,
};
use arcweft_rich_text_schema::{
    Multiplicity, PropertyPresence, RichTextPropertyPredicate, RichTextTagSchema, RichTextValueKind,
};

use super::value::{checked_default, parse_checked_value, parse_public_id};
use super::{
    CheckedDialogueContent, CheckedDialogueControl, CheckedDialogueHostEvent, CheckedDialogueToken,
    CheckedDirectStyleSpan, CheckedField, CheckedFieldOrigin, CheckedLayoutSpan, CheckedObjectSpan,
    CheckedOwnerFields, CheckedRichTextAction, CheckedRichTextClose, CheckedRichTextOwner,
    CheckedRichTextProperty, CheckedRichTextReport, CheckedRichTextTag, CheckedRichTextValue,
    CheckedStyleSpan, CheckedTransformSpan, CheckedVoiceSource, RichTextAttributeDiagnostic,
    RichTextDefaultId, RichTextDiagnosticCode, RichTextDiagnosticOwner, RichTextFailureEffect,
    RichTextRelatedSite,
};

const MAX_CHECKED_SPAN_DEPTH: usize = 64;

/// Sole owner/schema-driven validator for final-HIR `RichText` records.
#[derive(Clone, Copy, Debug, Default)]
pub struct RichTextAttributeChecker;

impl RichTextAttributeChecker {
    /// Validates one final-HIR dialogue-content value.
    ///
    /// All source evidence is obtained through the module's revision-bound
    /// source-role manifest. The checker never reads a source document or
    /// reconstructs syntax from a source range.
    pub fn check(
        module: &HirModule,
        content: &HirDialogueContent,
    ) -> Result<CheckedRichTextReport, HirSourceQueryError> {
        let mut diagnostics = Vec::new();
        let mut tags = BTreeMap::new();

        for tag in content.tags() {
            let result = Self::check_tag(module, tag)?;
            diagnostics.extend(result.diagnostics);
            if let Some(tag) = result.checked {
                tags.insert(tag.id(), tag);
            }
        }

        let tokens = assemble_tokens(module, content, tags, &mut diagnostics)?;
        Ok(CheckedRichTextReport::new(
            CheckedDialogueContent::new(content.id(), tokens, true),
            diagnostics,
        ))
    }

    fn check_tag(
        module: &HirModule,
        tag: &HirRichTextTag,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        match tag.identity() {
            HirRichTextTagIdentity::Builtin(builtin) => Self::check_builtin(module, tag, *builtin),
            HirRichTextTagIdentity::Registered(_) => Ok(TagCheckResult::diagnostic(
                tag_diagnostic(module, tag, RichTextDiagnosticCode::SchemaUnavailable)?,
            )),
            HirRichTextTagIdentity::Unresolved(unresolved) => {
                let code = match unresolved.issue() {
                    arcweft_lang_hir::dialogue_application::HirRichTextIssue::UnknownFx
                    | arcweft_lang_hir::dialogue_application::HirRichTextIssue::UnknownRegisteredTag => {
                        RichTextDiagnosticCode::UnknownSelector
                    }
                    _ => RichTextDiagnosticCode::UnknownTag,
                };
                Ok(TagCheckResult::diagnostic(tag_diagnostic(
                    module, tag, code,
                )?))
            }
        }
    }

    fn check_builtin(
        module: &HirModule,
        tag: &HirRichTextTag,
        builtin: HirBuiltinRichTextTag,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        match builtin {
            HirBuiltinRichTextTag::Page => {
                Self::check_control(module, tag, DialogueRichTextControl::Page, None)
            }
            HirBuiltinRichTextTag::LineWait => {
                Self::check_control(module, tag, DialogueRichTextControl::LineWait, None)
            }
            HirBuiltinRichTextTag::HardBreak => {
                Self::check_control(module, tag, DialogueRichTextControl::HardBreak, None)
            }
            HirBuiltinRichTextTag::TimedWait => Self::check_control(
                module,
                tag,
                DialogueRichTextControl::TimedWait,
                Some(DialogueControlProperty::Time),
            ),
            HirBuiltinRichTextTag::Clear => {
                Self::check_control(module, tag, DialogueRichTextControl::Clear, None)
            }
            HirBuiltinRichTextTag::Reset => {
                Self::check_control(module, tag, DialogueRichTextControl::Reset, None)
            }
            HirBuiltinRichTextTag::Speed => Self::check_control(
                module,
                tag,
                DialogueRichTextControl::RevealRate,
                Some(DialogueControlProperty::Cps),
            ),
            HirBuiltinRichTextTag::Marker => Self::check_marker(module, tag),
            HirBuiltinRichTextTag::DirectStyle(style) => {
                let owner = direct_style(style);
                let positional = match owner {
                    RichTextDirectStyle::Oblique => Some(RichTextDirectStyleProperty::Angle),
                    RichTextDirectStyle::Color
                    | RichTextDirectStyle::Font
                    | RichTextDirectStyle::Size => Some(RichTextDirectStyleProperty::Value),
                    RichTextDirectStyle::Emphasis
                    | RichTextDirectStyle::Strong
                    | RichTextDirectStyle::Italic
                    | RichTextDirectStyle::Ruby => None,
                };
                finish_schema(
                    module,
                    tag,
                    CheckedRichTextOwner::DirectStyle(owner),
                    owner.schema(),
                    positional,
                    false,
                )
            }
            HirBuiltinRichTextTag::Style(style) => {
                let owner = style_selector(style);
                finish_schema(
                    module,
                    tag,
                    CheckedRichTextOwner::Style(owner),
                    owner.schema(),
                    None,
                    true,
                )
            }
            HirBuiltinRichTextTag::Layout(layout) => {
                let owner = layout_selector(layout);
                finish_schema(
                    module,
                    tag,
                    CheckedRichTextOwner::Layout(owner),
                    owner.schema(),
                    None,
                    true,
                )
            }
            HirBuiltinRichTextTag::Transform(transform) => {
                let owner = transform_selector(transform);
                finish_schema(
                    module,
                    tag,
                    CheckedRichTextOwner::Transform(owner),
                    owner.schema(),
                    None,
                    true,
                )
            }
            HirBuiltinRichTextTag::Object(HirRichTextObjectSelector::Object) => {
                Self::check_object(module, tag)
            }
            HirBuiltinRichTextTag::Fx(effect) => Self::check_fx(module, tag, builtin_fx(effect)),
            HirBuiltinRichTextTag::HostEvent(event) => {
                Self::check_host(module, tag, host_event(event))
            }
            HirBuiltinRichTextTag::Conditional(event) => {
                Self::check_host(module, tag, conditional_event(event))
            }
        }
    }

    fn check_control(
        module: &HirModule,
        tag: &HirRichTextTag,
        owner: DialogueRichTextControl,
        positional: Option<DialogueControlProperty>,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        finish_schema(
            module,
            tag,
            CheckedRichTextOwner::Control(owner),
            owner.schema(),
            positional,
            false,
        )
    }

    fn check_marker(
        module: &HirModule,
        tag: &HirRichTextTag,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        let [HirRichTextArgument::Positional { value, .. }] = tag.arguments() else {
            let diagnostic = tag.arguments().first().map_or_else(
                || tag_diagnostic(module, tag, RichTextDiagnosticCode::RequiredMissing),
                |argument| {
                    argument_diagnostic(
                        module,
                        tag,
                        argument,
                        RichTextDiagnosticCode::PositionalArity,
                    )
                },
            )?;
            return Ok(TagCheckResult::diagnostic(diagnostic));
        };
        let Some(selector) = value.as_str().strip_prefix('.') else {
            return Ok(TagCheckResult::diagnostic(argument_diagnostic(
                module,
                tag,
                &tag.arguments()[0],
                RichTextDiagnosticCode::InvalidSelector,
            )?));
        };
        let marker = match parse_public_id(selector) {
            Ok(marker) => marker,
            Err(code) => {
                return Ok(TagCheckResult::diagnostic(argument_diagnostic(
                    module,
                    tag,
                    &tag.arguments()[0],
                    code,
                )?));
            }
        };
        Ok(TagCheckResult::checked(CheckedRichTextTag::new(
            tag.id(),
            CheckedRichTextOwner::Marker,
            CheckedRichTextAction::Marker(marker),
            tag_site(module, tag.id(), HirRichTextTagSourcePart::Whole)?,
        )))
    }

    fn check_object(
        module: &HirModule,
        tag: &HirRichTextTag,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        let Some(selector) = tag.arguments().first() else {
            return Ok(TagCheckResult::diagnostic(tag_diagnostic(
                module,
                tag,
                RichTextDiagnosticCode::RequiredMissing,
            )?));
        };
        let HirRichTextArgument::Positional { value, .. } = selector else {
            return Ok(TagCheckResult::diagnostic(argument_diagnostic(
                module,
                tag,
                selector,
                RichTextDiagnosticCode::PositionalArity,
            )?));
        };
        let selector_id = match parse_public_id(value.as_str()) {
            Ok(selector) => selector,
            Err(code) => {
                return Ok(TagCheckResult::diagnostic(argument_diagnostic(
                    module, tag, selector, code,
                )?));
            }
        };
        let schema = arcweft_presentation::rich_text::RichTextObjectSelector::Object.schema();
        let result = validate_schema::<RichTextObjectProperty>(
            module,
            tag,
            &tag.arguments()[1..],
            schema,
            None,
        )?;
        let mut result = result;
        result.object_selector = Some(selector_id);
        result.finish(module, tag, CheckedRichTextOwner::Object)
    }

    fn check_host(
        module: &HirModule,
        tag: &HirRichTextTag,
        owner: DialogueHostEventKind,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        let positional = match owner {
            DialogueHostEventKind::Voice => Some(DialogueHostProperty::Source),
            DialogueHostEventKind::Face => Some(DialogueHostProperty::Expression),
            DialogueHostEventKind::Pose => Some(DialogueHostProperty::Pose),
            DialogueHostEventKind::Show | DialogueHostEventKind::Hide => {
                Some(DialogueHostProperty::Entity)
            }
            DialogueHostEventKind::Rotate => Some(DialogueHostProperty::Angle),
            DialogueHostEventKind::Animation => Some(DialogueHostProperty::Animation),
            DialogueHostEventKind::TimedCue => Some(DialogueHostProperty::At),
            DialogueHostEventKind::Signal => Some(DialogueHostProperty::Signal),
            DialogueHostEventKind::Move
            | DialogueHostEventKind::Scale
            | DialogueHostEventKind::Shake
            | DialogueHostEventKind::Call
            | DialogueHostEventKind::ConditionalStart
            | DialogueHostEventKind::ConditionalElse
            | DialogueHostEventKind::ConditionalEnd => None,
        };
        let mut result = validate_schema(module, tag, tag.arguments(), owner.schema(), positional)?;
        if owner == DialogueHostEventKind::Move {
            let authored_axis = result.fields.iter().any(|field| {
                matches!(
                    field.property(),
                    CheckedRichTextProperty::Host(
                        DialogueHostProperty::X | DialogueHostProperty::Y
                    )
                ) && matches!(field.origin(), CheckedFieldOrigin::Authored { .. })
            });
            if !authored_axis {
                result.diagnostics.push(tag_diagnostic(
                    module,
                    tag,
                    RichTextDiagnosticCode::Conflict,
                )?);
            }
        }
        if owner == DialogueHostEventKind::Scale
            && !result.fields.iter().any(|field| {
                field.property() == CheckedRichTextProperty::Host(DialogueHostProperty::Y)
            })
            && let Some(value) = result.fields.iter().find_map(|field| {
                (field.property() == CheckedRichTextProperty::Host(DialogueHostProperty::X))
                    .then(|| field.value().clone())
            })
        {
            result.fields.push(CheckedField::new(
                CheckedRichTextProperty::Host(DialogueHostProperty::Y),
                value,
                CheckedFieldOrigin::Defaulted {
                    default_id: RichTextDefaultId::from_schema_ordinal(1),
                },
            ));
        }
        result.finish(module, tag, CheckedRichTextOwner::Host(owner))
    }

    fn check_fx(
        module: &HirModule,
        tag: &HirRichTextTag,
        effect: BuiltinRichTextFx,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        let arguments = skip_typed_family_selector(tag.arguments());
        let mut phase = effect.default_phase();
        if let Some(argument) = arguments.iter().find(|argument| {
            matches!(argument, HirRichTextArgument::Named { name, .. } if name.as_str() == "phase")
        }) {
            let Some(value) = argument.value() else {
                return Ok(TagCheckResult::diagnostic(argument_diagnostic(
                    module,
                    tag,
                    argument,
                    RichTextDiagnosticCode::InvalidArgument,
                )?));
            };
            let Some(authored_phase) = BuiltinRichTextFxPhase::from_source_name(value.as_str())
            else {
                return Ok(TagCheckResult::diagnostic(argument_diagnostic(
                    module,
                    tag,
                    argument,
                    RichTextDiagnosticCode::InvalidEnum,
                )?));
            };
            phase = authored_phase;
        }
        if !effect.supported_phases().contains(&phase) {
            return Ok(TagCheckResult::diagnostic(tag_diagnostic(
                module,
                tag,
                RichTextDiagnosticCode::PropertyNotInPhase,
            )?));
        }
        let result = validate_fx_schema(module, tag, arguments, effect, phase)?;
        result.finish(
            module,
            tag,
            CheckedRichTextOwner::BuiltinFx { effect, phase },
        )
    }
}

struct TagCheckResult {
    checked: Option<CheckedRichTextTag>,
    diagnostics: Vec<RichTextAttributeDiagnostic>,
}

impl TagCheckResult {
    fn checked(checked: CheckedRichTextTag) -> Self {
        Self {
            checked: Some(checked),
            diagnostics: Vec::new(),
        }
    }

    fn diagnostic(diagnostic: RichTextAttributeDiagnostic) -> Self {
        Self {
            checked: None,
            diagnostics: vec![diagnostic],
        }
    }
}

struct SchemaCheckResult {
    fields: Vec<CheckedField>,
    diagnostics: Vec<RichTextAttributeDiagnostic>,
    object_selector: Option<arcweft_id::PublicId>,
}

impl SchemaCheckResult {
    fn finish(
        self,
        module: &HirModule,
        tag: &HirRichTextTag,
        owner: CheckedRichTextOwner,
    ) -> Result<TagCheckResult, HirSourceQueryError> {
        if self.diagnostics.is_empty() {
            let fields = CheckedOwnerFields::new(self.fields);
            if let Some(action) = checked_action(owner, fields, self.object_selector, tag.payload())
            {
                Ok(TagCheckResult::checked(CheckedRichTextTag::new(
                    tag.id(),
                    owner,
                    action,
                    tag_site(module, tag.id(), HirRichTextTagSourcePart::Whole)?,
                )))
            } else {
                Ok(TagCheckResult::diagnostic(tag_diagnostic(
                    module,
                    tag,
                    RichTextDiagnosticCode::SchemaUnavailable,
                )?))
            }
        } else {
            Ok(TagCheckResult {
                checked: None,
                diagnostics: self.diagnostics,
            })
        }
    }
}

fn finish_schema<P: CheckedPropertyDomain>(
    module: &HirModule,
    tag: &HirRichTextTag,
    owner: CheckedRichTextOwner,
    schema: &'static RichTextTagSchema<P>,
    positional: Option<P>,
    typed_family: bool,
) -> Result<TagCheckResult, HirSourceQueryError> {
    let arguments = if typed_family {
        skip_typed_family_selector(tag.arguments())
    } else {
        tag.arguments()
    };
    let result = validate_schema(module, tag, arguments, schema, positional)?;
    result.finish(module, tag, owner)
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed RichText property-schema matrix validates source, arity, aliases, duplicates, and typed values in one deterministic pass"
)]
fn validate_schema<P: CheckedPropertyDomain>(
    module: &HirModule,
    tag: &HirRichTextTag,
    arguments: &[HirRichTextArgument],
    schema: &'static RichTextTagSchema<P>,
    positional: Option<P>,
) -> Result<SchemaCheckResult, HirSourceQueryError> {
    let mut diagnostics = Vec::new();
    let mut authored: BTreeMap<P, Vec<(CheckedRichTextValue, CheckedFieldOrigin)>> =
        BTreeMap::new();
    let mut first_sites: BTreeMap<P, HirSourceSite> = BTreeMap::new();
    let mut positional_consumed = false;

    for argument in arguments {
        let (property, value) = match argument {
            HirRichTextArgument::Positional { value, .. } => {
                let Some(property) = positional.filter(|_| !positional_consumed) else {
                    diagnostics.push(argument_diagnostic(
                        module,
                        tag,
                        argument,
                        RichTextDiagnosticCode::PositionalForbidden,
                    )?);
                    continue;
                };
                positional_consumed = true;
                (property, value)
            }
            HirRichTextArgument::Named { name, value, .. } => {
                let Some(property) = P::from_source_name(name.as_str()) else {
                    diagnostics.push(argument_diagnostic(
                        module,
                        tag,
                        argument,
                        RichTextDiagnosticCode::UnknownProperty,
                    )?);
                    continue;
                };
                (property, value)
            }
            HirRichTextArgument::Invalid { issue, .. } => {
                diagnostics.push(argument_diagnostic(
                    module,
                    tag,
                    argument,
                    argument_issue_code(*issue),
                )?);
                continue;
            }
        };

        let Some(spec) = schema
            .properties
            .iter()
            .find(|candidate| candidate.id == property)
        else {
            diagnostics.push(argument_diagnostic(
                module,
                tag,
                argument,
                RichTextDiagnosticCode::UnknownProperty,
            )?);
            continue;
        };
        let value_site = argument_site(
            module,
            tag.id(),
            argument.id(),
            HirRichTextArgumentSourcePart::Value,
        )?;
        let key_site = optional_argument_site(
            module,
            tag.id(),
            argument.id(),
            HirRichTextArgumentSourcePart::Name,
        )?;
        let entries = authored.entry(property).or_default();
        let over_limit = match spec.multiplicity {
            Multiplicity::Single => !entries.is_empty(),
            Multiplicity::Repeated { max } => entries.len() >= usize::from(max),
        };
        if over_limit {
            let code = match spec.multiplicity {
                Multiplicity::Single => RichTextDiagnosticCode::Duplicate,
                Multiplicity::Repeated { .. } => RichTextDiagnosticCode::ResourceLimit,
            };
            let mut diagnostic = argument_diagnostic(module, tag, argument, code)?;
            if let Some(first) = first_sites.get(&property) {
                diagnostic = diagnostic.with_related(RichTextRelatedSite::new(
                    first.clone(),
                    "first authored value",
                ));
            }
            diagnostics.push(diagnostic);
            continue;
        }
        match parse_checked_value(value.as_str(), spec) {
            Ok(value) => {
                first_sites
                    .entry(property)
                    .or_insert_with(|| key_site.clone().unwrap_or_else(|| value_site.clone()));
                entries.push((
                    value,
                    CheckedFieldOrigin::Authored {
                        argument: argument.id(),
                        key: key_site,
                        value: value_site,
                    },
                ));
            }
            Err(code) => diagnostics.push(argument_diagnostic(module, tag, argument, code)?),
        }
    }

    let mut fields = Vec::new();
    let values = authored
        .iter()
        .map(|(&property, entries)| {
            (
                property,
                entries
                    .iter()
                    .map(|(value, _)| value.clone())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (ordinal, spec) in schema.properties.iter().enumerate() {
        if let Some(entries) = authored.remove(&spec.id) {
            fields.extend(entries.into_iter().map(|(value, origin)| {
                CheckedField::new(spec.id.checked_property(), value, origin)
            }));
            continue;
        }
        match spec.presence {
            PropertyPresence::Required => diagnostics.push(missing_property_diagnostic(
                module,
                tag,
                RichTextDiagnosticCode::RequiredMissing,
            )?),
            PropertyPresence::Optional => {}
            PropertyPresence::Defaulted(default) => {
                let value = checked_default(default, enum_schema_id(spec.kind))
                    .expect("owner schemas contain valid closed defaults");
                fields.push(CheckedField::new(
                    spec.id.checked_property(),
                    value,
                    CheckedFieldOrigin::Defaulted {
                        default_id: RichTextDefaultId::from_schema_ordinal(
                            u16::try_from(ordinal).expect("schema property count fits u16"),
                        ),
                    },
                ));
            }
            PropertyPresence::Conditional { predicate } => {
                if predicate_holds(predicate, &values) {
                    diagnostics.push(missing_property_diagnostic(
                        module,
                        tag,
                        RichTextDiagnosticCode::RequiredMissing,
                    )?);
                }
            }
        }
    }
    Ok(SchemaCheckResult {
        fields,
        diagnostics,
        object_selector: None,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "the closed RichText Fx matrix validates phase-specific fields and source ownership together"
)]
fn validate_fx_schema(
    module: &HirModule,
    tag: &HirRichTextTag,
    arguments: &[HirRichTextArgument],
    effect: BuiltinRichTextFx,
    phase: BuiltinRichTextFxPhase,
) -> Result<SchemaCheckResult, HirSourceQueryError> {
    let mut diagnostics = Vec::new();
    let mut authored: BTreeMap<
        BuiltinRichTextFxProperty,
        (CheckedRichTextValue, CheckedFieldOrigin),
    > = BTreeMap::new();
    let mut first_sites: BTreeMap<BuiltinRichTextFxProperty, HirSourceSite> = BTreeMap::new();

    for argument in arguments {
        let HirRichTextArgument::Named { name, value, .. } = argument else {
            let code = if let Some(issue) = argument.issue() {
                argument_issue_code(issue)
            } else {
                RichTextDiagnosticCode::PositionalForbidden
            };
            diagnostics.push(argument_diagnostic(module, tag, argument, code)?);
            continue;
        };
        let Some(property) = BuiltinRichTextFxProperty::from_source_name(name.as_str()) else {
            diagnostics.push(argument_diagnostic(
                module,
                tag,
                argument,
                RichTextDiagnosticCode::UnknownProperty,
            )?);
            continue;
        };
        let BuiltinPropertyDisposition::Accepted(spec) = effect.property_spec(phase, property)
        else {
            diagnostics.push(argument_diagnostic(
                module,
                tag,
                argument,
                RichTextDiagnosticCode::PropertyNotInPhase,
            )?);
            continue;
        };
        if let Some(first) = first_sites.get(&property) {
            let diagnostic =
                argument_diagnostic(module, tag, argument, RichTextDiagnosticCode::Duplicate)?
                    .with_related(RichTextRelatedSite::new(
                        first.clone(),
                        "first authored value",
                    ));
            diagnostics.push(diagnostic);
            continue;
        }
        match parse_checked_value(value.as_str(), spec) {
            Ok(value) => {
                let value_site = argument_site(
                    module,
                    tag.id(),
                    argument.id(),
                    HirRichTextArgumentSourcePart::Value,
                )?;
                let key_site = optional_argument_site(
                    module,
                    tag.id(),
                    argument.id(),
                    HirRichTextArgumentSourcePart::Name,
                )?;
                first_sites.insert(
                    property,
                    key_site.clone().unwrap_or_else(|| value_site.clone()),
                );
                authored.insert(
                    property,
                    (
                        value,
                        CheckedFieldOrigin::Authored {
                            argument: argument.id(),
                            key: key_site,
                            value: value_site,
                        },
                    ),
                );
            }
            Err(code) => diagnostics.push(argument_diagnostic(module, tag, argument, code)?),
        }
    }

    let values = authored
        .iter()
        .map(|(&property, (value, _))| (property, vec![value.clone()]))
        .collect::<BTreeMap<_, _>>();
    let mut fields = Vec::new();
    for (ordinal, property) in effect
        .properties_for_phase(phase)
        .iter()
        .copied()
        .enumerate()
    {
        let BuiltinPropertyDisposition::Accepted(spec) = effect.property_spec(phase, property)
        else {
            unreachable!("effect phase property inventory is self-consistent")
        };
        if let Some((value, origin)) = authored.remove(&property) {
            fields.push(CheckedField::new(
                CheckedRichTextProperty::BuiltinFx(property),
                value,
                origin,
            ));
            continue;
        }
        let default = match spec.presence {
            PropertyPresence::Defaulted(default) => Some(default),
            PropertyPresence::Conditional { predicate } if predicate_holds(predicate, &values) => {
                effect.conditional_default(phase, property)
            }
            PropertyPresence::Required => None,
            PropertyPresence::Optional | PropertyPresence::Conditional { .. } => continue,
        };
        if let Some(default) = default {
            let value = checked_default(default, enum_schema_id(spec.kind))
                .expect("builtin Fx schemas contain valid closed defaults");
            fields.push(CheckedField::new(
                CheckedRichTextProperty::BuiltinFx(property),
                value,
                CheckedFieldOrigin::Defaulted {
                    default_id: RichTextDefaultId::from_schema_ordinal(
                        u16::try_from(ordinal).expect("Fx property count fits u16"),
                    ),
                },
            ));
        } else {
            diagnostics.push(missing_property_diagnostic(
                module,
                tag,
                RichTextDiagnosticCode::RequiredMissing,
            )?);
        }
    }
    Ok(SchemaCheckResult {
        fields,
        diagnostics,
        object_selector: None,
    })
}

fn checked_action(
    owner: CheckedRichTextOwner,
    fields: CheckedOwnerFields,
    object_selector: Option<arcweft_id::PublicId>,
    payload: &HirRichTextTagPayload,
) -> Option<CheckedRichTextAction> {
    match owner {
        CheckedRichTextOwner::Control(owner) => Some(CheckedRichTextAction::Control {
            action: checked_control(owner, &fields)?,
            fields,
        }),
        CheckedRichTextOwner::DirectStyle(owner) => Some(CheckedRichTextAction::DirectStyle {
            owner,
            action: checked_direct_style(owner, &fields)?,
            fields,
        }),
        CheckedRichTextOwner::Style(owner) => Some(CheckedRichTextAction::Style {
            owner,
            action: checked_style(owner, &fields)?,
            fields,
        }),
        CheckedRichTextOwner::Layout(owner) => Some(CheckedRichTextAction::Layout {
            owner,
            action: checked_layout(owner, &fields)?,
            fields,
        }),
        CheckedRichTextOwner::Transform(owner) => Some(CheckedRichTextAction::Transform {
            owner,
            action: checked_transform(owner, &fields)?,
            fields,
        }),
        CheckedRichTextOwner::Object => Some(CheckedRichTextAction::Object {
            action: checked_object(object_selector?, &fields)?,
            fields,
        }),
        CheckedRichTextOwner::BuiltinFx { effect, phase } => {
            Some(CheckedRichTextAction::BuiltinFx {
                effect,
                phase,
                fields,
            })
        }
        CheckedRichTextOwner::Host(owner) => Some(CheckedRichTextAction::Host {
            owner,
            action: checked_host_event(owner, &fields, payload)?,
            fields,
        }),
        CheckedRichTextOwner::Marker => None,
    }
}

fn checked_control(
    owner: DialogueRichTextControl,
    fields: &CheckedOwnerFields,
) -> Option<CheckedDialogueControl> {
    Some(match owner {
        DialogueRichTextControl::Page => CheckedDialogueControl::Page,
        DialogueRichTextControl::LineWait => CheckedDialogueControl::LineWait,
        DialogueRichTextControl::HardBreak => CheckedDialogueControl::HardBreak,
        DialogueRichTextControl::TimedWait => {
            let CheckedRichTextValue::Duration(duration) = fields.value(
                CheckedRichTextProperty::Control(DialogueControlProperty::Time),
            )?
            else {
                return None;
            };
            CheckedDialogueControl::TimedWait {
                duration: *duration,
            }
        }
        DialogueRichTextControl::Clear => CheckedDialogueControl::Clear,
        DialogueRichTextControl::Reset => CheckedDialogueControl::Reset,
        DialogueRichTextControl::RevealRate => {
            let CheckedRichTextValue::Milli(milli_cps) = fields.value(
                CheckedRichTextProperty::Control(DialogueControlProperty::Cps),
            )?
            else {
                return None;
            };
            CheckedDialogueControl::RevealRate {
                milli_cps: *milli_cps,
            }
        }
        DialogueRichTextControl::Marker => return None,
    })
}

fn checked_direct_style(
    owner: RichTextDirectStyle,
    fields: &CheckedOwnerFields,
) -> Option<CheckedDirectStyleSpan> {
    let property = |id| fields.value(CheckedRichTextProperty::DirectStyle(id));
    Some(match owner {
        RichTextDirectStyle::Emphasis => CheckedDirectStyleSpan::Emphasis,
        RichTextDirectStyle::Strong => CheckedDirectStyleSpan::Strong,
        RichTextDirectStyle::Italic => CheckedDirectStyleSpan::Italic,
        RichTextDirectStyle::Oblique => {
            let CheckedRichTextValue::Angle(angle) = property(RichTextDirectStyleProperty::Angle)?
            else {
                return None;
            };
            CheckedDirectStyleSpan::Oblique { angle: *angle }
        }
        RichTextDirectStyle::Color => {
            let CheckedRichTextValue::Color(value) = property(RichTextDirectStyleProperty::Value)?
            else {
                return None;
            };
            CheckedDirectStyleSpan::Color {
                value: value.clone(),
            }
        }
        RichTextDirectStyle::Font => {
            let CheckedRichTextValue::Text(family) = property(RichTextDirectStyleProperty::Value)?
            else {
                return None;
            };
            CheckedDirectStyleSpan::Font {
                family: family.clone(),
            }
        }
        RichTextDirectStyle::Size => {
            let CheckedRichTextValue::Length(value) = property(RichTextDirectStyleProperty::Value)?
            else {
                return None;
            };
            CheckedDirectStyleSpan::Size { value: *value }
        }
        RichTextDirectStyle::Ruby => {
            let CheckedRichTextValue::Text(annotation) =
                property(RichTextDirectStyleProperty::RubyText)?
            else {
                return None;
            };
            CheckedDirectStyleSpan::Ruby {
                annotation: annotation.clone(),
            }
        }
    })
}

fn checked_style(
    owner: RichTextStyleSelector,
    fields: &CheckedOwnerFields,
) -> Option<CheckedStyleSpan> {
    let property = |id| fields.value(CheckedRichTextProperty::Style(id));
    Some(match owner {
        RichTextStyleSelector::Italic => CheckedStyleSpan::Italic,
        RichTextStyleSelector::Oblique => {
            let CheckedRichTextValue::Angle(angle) = property(RichTextStyleProperty::Angle)? else {
                return None;
            };
            CheckedStyleSpan::Oblique { angle: *angle }
        }
        RichTextStyleSelector::Opacity => {
            let CheckedRichTextValue::Ratio(value) = property(RichTextStyleProperty::Opacity)?
            else {
                return None;
            };
            CheckedStyleSpan::Opacity { value: *value }
        }
        RichTextStyleSelector::Layer => {
            let CheckedRichTextValue::PublicId(value) = property(RichTextStyleProperty::Layer)?
            else {
                return None;
            };
            CheckedStyleSpan::Layer {
                value: value.clone(),
            }
        }
        RichTextStyleSelector::ZIndex => {
            let CheckedRichTextValue::Int(value) = property(RichTextStyleProperty::ZIndex)? else {
                return None;
            };
            CheckedStyleSpan::ZIndex {
                value: i16::try_from(*value).ok()?,
            }
        }
    })
}

fn checked_layout(
    selector: RichTextLayoutSelector,
    fields: &CheckedOwnerFields,
) -> Option<CheckedLayoutSpan> {
    let property = |id| fields.value(CheckedRichTextProperty::Layout(id));
    let CheckedRichTextValue::Enum(direction) = property(RichTextLayoutProperty::Direction)? else {
        return None;
    };
    let CheckedRichTextValue::Enum(vertical_latin) = property(RichTextLayoutProperty::Latin)?
    else {
        return None;
    };
    let CheckedRichTextValue::Enum(jlreq_strictness) = property(RichTextLayoutProperty::Jlreq)?
    else {
        return None;
    };
    let CheckedRichTextValue::Length(column_gap) = property(RichTextLayoutProperty::ColumnGap)?
    else {
        return None;
    };
    Some(CheckedLayoutSpan::new(
        selector,
        *direction,
        *vertical_latin,
        *jlreq_strictness,
        *column_gap,
        optional_layout_length(fields, RichTextLayoutProperty::RubySize).ok()?,
        optional_layout_length(fields, RichTextLayoutProperty::RubyGap).ok()?,
        optional_layout_length(fields, RichTextLayoutProperty::RubyOverhang).ok()?,
        optional_layout_length(fields, RichTextLayoutProperty::RubyCollisionGap).ok()?,
    ))
}

fn optional_layout_length(
    fields: &CheckedOwnerFields,
    property: RichTextLayoutProperty,
) -> Result<Option<super::CheckedLength>, ()> {
    match fields.value(CheckedRichTextProperty::Layout(property)) {
        Some(CheckedRichTextValue::Length(value)) => Ok(Some(*value)),
        None => Ok(None),
        Some(_) => Err(()),
    }
}

fn checked_transform(
    selector: RichTextTransformSelector,
    fields: &CheckedOwnerFields,
) -> Option<CheckedTransformSpan> {
    let property = |id| fields.value(CheckedRichTextProperty::Transform(id));
    let enum_property = |id| {
        let CheckedRichTextValue::Enum(value) = property(id)? else {
            return None;
        };
        Some(*value)
    };
    let target = enum_property(RichTextTransformProperty::Target)?;
    let origin = enum_property(RichTextTransformProperty::Origin)?;
    Some(match selector {
        RichTextTransformSelector::Offset => {
            let CheckedRichTextValue::Length(x) = property(RichTextTransformProperty::X)? else {
                return None;
            };
            let CheckedRichTextValue::Length(y) = property(RichTextTransformProperty::Y)? else {
                return None;
            };
            CheckedTransformSpan::Offset {
                x: *x,
                y: *y,
                target,
                origin,
            }
        }
        RichTextTransformSelector::Rotate => {
            let CheckedRichTextValue::Angle(angle) = property(RichTextTransformProperty::Angle)?
            else {
                return None;
            };
            CheckedTransformSpan::Rotate {
                angle: *angle,
                target,
                origin,
            }
        }
        RichTextTransformSelector::Scale => {
            let CheckedRichTextValue::Milli(x) = property(RichTextTransformProperty::X)? else {
                return None;
            };
            let CheckedRichTextValue::Milli(y) = property(RichTextTransformProperty::Y)? else {
                return None;
            };
            CheckedTransformSpan::Scale {
                x: *x,
                y: *y,
                target,
                origin,
            }
        }
        RichTextTransformSelector::Skew => {
            let CheckedRichTextValue::Angle(x) = property(RichTextTransformProperty::X)? else {
                return None;
            };
            let CheckedRichTextValue::Angle(y) = property(RichTextTransformProperty::Y)? else {
                return None;
            };
            CheckedTransformSpan::Skew {
                x: *x,
                y: *y,
                target,
                origin,
            }
        }
    })
}

fn checked_object(
    selector: arcweft_id::PublicId,
    fields: &CheckedOwnerFields,
) -> Option<CheckedObjectSpan> {
    let property = |id| fields.value(CheckedRichTextProperty::Object(id));
    let public_id = |id| match property(id) {
        Some(CheckedRichTextValue::PublicId(value)) => Some(Some(value.clone())),
        None => Some(None),
        Some(_) => None,
    };
    let depth = match property(RichTextObjectProperty::Depth) {
        Some(CheckedRichTextValue::Length(value)) => Some(Some(*value)),
        None => Some(None),
        Some(_) => None,
    }?;
    let hit_test = match property(RichTextObjectProperty::HitTest) {
        Some(CheckedRichTextValue::Bool(value)) => *value,
        None => false,
        Some(_) => return None,
    };
    Some(CheckedObjectSpan::new(
        selector,
        public_id(RichTextObjectProperty::Role)?,
        public_id(RichTextObjectProperty::Layer)?,
        depth,
        hit_test,
    ))
}

fn checked_host_event(
    owner: DialogueHostEventKind,
    fields: &CheckedOwnerFields,
    payload: &HirRichTextTagPayload,
) -> Option<CheckedDialogueHostEvent> {
    let property = |id| fields.value(CheckedRichTextProperty::Host(id));
    let public_id = |id| {
        let CheckedRichTextValue::PublicId(value) = property(id)? else {
            return None;
        };
        Some(value.clone())
    };
    Some(match owner {
        DialogueHostEventKind::Voice => {
            let value = public_id(DialogueHostProperty::Source)?;
            let source = if value.as_str() == "auto" {
                CheckedVoiceSource::Auto
            } else {
                CheckedVoiceSource::Identity(value)
            };
            CheckedDialogueHostEvent::Voice { source }
        }
        DialogueHostEventKind::Face => CheckedDialogueHostEvent::Face {
            expression: public_id(DialogueHostProperty::Expression)?,
        },
        DialogueHostEventKind::Pose => CheckedDialogueHostEvent::Pose {
            pose: public_id(DialogueHostProperty::Pose)?,
        },
        DialogueHostEventKind::Show => CheckedDialogueHostEvent::Show {
            entity: public_id(DialogueHostProperty::Entity)?,
        },
        DialogueHostEventKind::Hide => CheckedDialogueHostEvent::Hide {
            entity: public_id(DialogueHostProperty::Entity)?,
        },
        DialogueHostEventKind::Move => {
            let CheckedRichTextValue::Length(x) = property(DialogueHostProperty::X)? else {
                return None;
            };
            let CheckedRichTextValue::Length(y) = property(DialogueHostProperty::Y)? else {
                return None;
            };
            CheckedDialogueHostEvent::Move { x: *x, y: *y }
        }
        DialogueHostEventKind::Scale => {
            let CheckedRichTextValue::Milli(x) = property(DialogueHostProperty::X)? else {
                return None;
            };
            let CheckedRichTextValue::Milli(y) = property(DialogueHostProperty::Y)? else {
                return None;
            };
            CheckedDialogueHostEvent::Scale { x: *x, y: *y }
        }
        DialogueHostEventKind::Rotate => {
            let CheckedRichTextValue::Angle(angle) = property(DialogueHostProperty::Angle)? else {
                return None;
            };
            CheckedDialogueHostEvent::Rotate { angle: *angle }
        }
        DialogueHostEventKind::Animation => CheckedDialogueHostEvent::Animation {
            animation: public_id(DialogueHostProperty::Animation)?,
        },
        DialogueHostEventKind::Shake => {
            let CheckedRichTextValue::Length(amplitude) = property(DialogueHostProperty::Amp)?
            else {
                return None;
            };
            CheckedDialogueHostEvent::Shake {
                amplitude: *amplitude,
            }
        }
        DialogueHostEventKind::TimedCue => {
            let CheckedRichTextValue::Duration(at) = property(DialogueHostProperty::At)? else {
                return None;
            };
            let HirRichTextTagPayload::DialogueCall(call) = payload else {
                return None;
            };
            CheckedDialogueHostEvent::TimedCue {
                at: *at,
                call: *call,
            }
        }
        DialogueHostEventKind::Call => {
            let HirRichTextTagPayload::DialogueCall(call) = payload else {
                return None;
            };
            CheckedDialogueHostEvent::Call { call: *call }
        }
        DialogueHostEventKind::Signal => CheckedDialogueHostEvent::Signal {
            signal: public_id(DialogueHostProperty::Signal)?,
        },
        DialogueHostEventKind::ConditionalStart => {
            let HirRichTextTagPayload::Condition(condition) = payload else {
                return None;
            };
            CheckedDialogueHostEvent::ConditionalStart {
                condition: *condition,
            }
        }
        DialogueHostEventKind::ConditionalElse => CheckedDialogueHostEvent::ConditionalElse,
        DialogueHostEventKind::ConditionalEnd => CheckedDialogueHostEvent::ConditionalEnd,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "token assembly is one ordered zip of retained HIR text, checked tags, and dialogue host events"
)]
fn assemble_tokens(
    module: &HirModule,
    content: &HirDialogueContent,
    mut checked_tags: BTreeMap<HirRichTextTagId, CheckedRichTextTag>,
    diagnostics: &mut Vec<RichTextAttributeDiagnostic>,
) -> Result<Vec<CheckedDialogueToken>, HirSourceQueryError> {
    let tags = content
        .tags()
        .iter()
        .map(|tag| (tag.id(), tag))
        .collect::<BTreeMap<_, _>>();
    let mut stack = Vec::new();
    let mut tokens = Vec::new();
    for node in content.nodes() {
        match node.kind() {
            HirDialogueNodeKind::Text(text) => {
                tokens.push(CheckedDialogueToken::Text(text.as_str().into()));
            }
            HirDialogueNodeKind::Raw(text) => {
                tokens.push(CheckedDialogueToken::RawText(text.as_str().into()));
            }
            HirDialogueNodeKind::Escape(value) => tokens.push(CheckedDialogueToken::Escape(*value)),
            HirDialogueNodeKind::Ruby(ruby) => tokens.push(CheckedDialogueToken::Ruby {
                base: ruby.base().into(),
                ruby: ruby.ruby().into(),
            }),
            HirDialogueNodeKind::AuthoredStartTag(tag)
            | HirDialogueNodeKind::InferredStartTag(tag) => {
                let source_tag = tags
                    .get(tag)
                    .expect("dialogue content validates every start-tag reference");
                let opens_span = tag_opens_span(source_tag.identity());
                if opens_span && stack.len() >= MAX_CHECKED_SPAN_DEPTH {
                    diagnostics.push(node_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::NestingLimit,
                    )?);
                    continue;
                }
                let accepted = checked_tags.contains_key(tag);
                if opens_span {
                    stack.push((*tag, accepted));
                }
                if let Some(tag) = checked_tags.remove(tag) {
                    tokens.push(CheckedDialogueToken::Open(tag));
                } else {
                    tokens.push(CheckedDialogueToken::InvalidTag {
                        tag: *tag,
                        source: tag_site(module, *tag, HirRichTextTagSourcePart::Whole)?,
                    });
                }
            }
            HirDialogueNodeKind::AuthoredEndTag(end) | HirDialogueNodeKind::InferredEndTag(end) => {
                let Some(paired_start) = end.paired_start() else {
                    diagnostics.push(node_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::UnmatchedClose,
                    )?);
                    continue;
                };
                let matching = stack.iter().rposition(|(open, _)| *open == paired_start);
                match matching {
                    Some(index) if index + 1 == stack.len() => {
                        let (open, accepted) = stack.pop().expect("matching stack top exists");
                        if accepted {
                            tokens.push(CheckedDialogueToken::Close(CheckedRichTextClose::new(
                                open,
                                node_site(module, content.id().owner(), node)?,
                                node.id().ordinal(),
                                end.is_inferred(),
                            )));
                        }
                    }
                    Some(_) => diagnostics.push(node_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::CrossingSpan,
                    )?),
                    None => diagnostics.push(node_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::UnmatchedClose,
                    )?),
                }
            }
            HirDialogueNodeKind::Interpolation(expression) => {
                tokens.push(CheckedDialogueToken::Interpolation(*expression));
            }
            HirDialogueNodeKind::LineBreak(kind) => {
                tokens.push(CheckedDialogueToken::LineBreak(*kind));
            }
            HirDialogueNodeKind::Error(issue) => {
                if !matches!(issue, HirDialogueContentError::UnclosedTag) {
                    let code = match issue {
                        HirDialogueContentError::UnmatchedEndTag => {
                            RichTextDiagnosticCode::UnmatchedClose
                        }
                        HirDialogueContentError::UnclosedTag => unreachable!(),
                        HirDialogueContentError::UnclassifiedToken
                        | HirDialogueContentError::InvalidEscape
                        | HirDialogueContentError::InvalidRuby => {
                            RichTextDiagnosticCode::InvalidArgument
                        }
                    };
                    diagnostics.push(node_diagnostic(module, content.id().owner(), node, code)?);
                }
            }
        }
    }
    for (open, _) in stack {
        let tag = tags
            .get(&open)
            .expect("dialogue content validates every start-tag reference");
        diagnostics.push(tag_diagnostic(
            module,
            tag,
            RichTextDiagnosticCode::UnclosedSpan,
        )?);
    }
    Ok(tokens)
}

fn tag_opens_span(identity: &HirRichTextTagIdentity) -> bool {
    match identity {
        HirRichTextTagIdentity::Builtin(builtin) => matches!(
            builtin,
            HirBuiltinRichTextTag::DirectStyle(_)
                | HirBuiltinRichTextTag::Style(_)
                | HirBuiltinRichTextTag::Layout(_)
                | HirBuiltinRichTextTag::Transform(_)
                | HirBuiltinRichTextTag::Object(_)
                | HirBuiltinRichTextTag::Fx(_)
        ),
        HirRichTextTagIdentity::Registered(_) | HirRichTextTagIdentity::Unresolved(_) => true,
    }
}

fn skip_typed_family_selector(arguments: &[HirRichTextArgument]) -> &[HirRichTextArgument] {
    if matches!(
        arguments.first(),
        Some(HirRichTextArgument::Positional { value, .. }) if value.as_str().starts_with('.')
    ) {
        &arguments[1..]
    } else {
        arguments
    }
}

fn tag_diagnostic(
    module: &HirModule,
    tag: &HirRichTextTag,
    code: RichTextDiagnosticCode,
) -> Result<RichTextAttributeDiagnostic, HirSourceQueryError> {
    Ok(RichTextAttributeDiagnostic::new(
        code,
        RichTextDiagnosticOwner::Tag(tag.id()),
        tag_site(module, tag.id(), HirRichTextTagSourcePart::Name)?,
        RichTextFailureEffect::RejectTag,
    ))
}

fn missing_property_diagnostic(
    module: &HirModule,
    tag: &HirRichTextTag,
    code: RichTextDiagnosticCode,
) -> Result<RichTextAttributeDiagnostic, HirSourceQueryError> {
    tag_diagnostic(module, tag, code)
}

fn argument_diagnostic(
    module: &HirModule,
    tag: &HirRichTextTag,
    argument: &HirRichTextArgument,
    code: RichTextDiagnosticCode,
) -> Result<RichTextAttributeDiagnostic, HirSourceQueryError> {
    Ok(RichTextAttributeDiagnostic::new(
        code,
        RichTextDiagnosticOwner::Argument(argument.id()),
        argument_site(
            module,
            tag.id(),
            argument.id(),
            HirRichTextArgumentSourcePart::Whole,
        )?,
        RichTextFailureEffect::RejectTag,
    ))
}

fn node_diagnostic(
    module: &HirModule,
    owner: ExprId,
    node: &HirDialogueNode,
    code: RichTextDiagnosticCode,
) -> Result<RichTextAttributeDiagnostic, HirSourceQueryError> {
    Ok(RichTextAttributeDiagnostic::new(
        code,
        RichTextDiagnosticOwner::Node(node.id()),
        node_site(module, owner, node)?,
        RichTextFailureEffect::RejectCompilation,
    ))
}

fn node_site(
    module: &HirModule,
    owner: ExprId,
    node: &HirDialogueNode,
) -> Result<HirSourceSite, HirSourceQueryError> {
    required_expr_site(
        module,
        owner,
        HirExprSourceRole::DialogueNode {
            ordinal: node.id().ordinal(),
            part: HirDialogueNodeSourcePart::Whole,
        },
    )
}

fn tag_site(
    module: &HirModule,
    tag: HirRichTextTagId,
    part: HirRichTextTagSourcePart,
) -> Result<HirSourceSite, HirSourceQueryError> {
    required_expr_site(
        module,
        tag.content().owner(),
        HirExprSourceRole::RichTextTag {
            tag: tag.ordinal(),
            part,
        },
    )
}

fn argument_site(
    module: &HirModule,
    tag: HirRichTextTagId,
    argument: HirRichTextArgumentId,
    part: HirRichTextArgumentSourcePart,
) -> Result<HirSourceSite, HirSourceQueryError> {
    required_expr_site(
        module,
        tag.content().owner(),
        HirExprSourceRole::RichTextArgument {
            tag: tag.ordinal(),
            argument: argument.ordinal(),
            part,
        },
    )
}

fn optional_argument_site(
    module: &HirModule,
    tag: HirRichTextTagId,
    argument: HirRichTextArgumentId,
    part: HirRichTextArgumentSourcePart,
) -> Result<Option<HirSourceSite>, HirSourceQueryError> {
    optional_expr_site(
        module,
        tag.content().owner(),
        HirExprSourceRole::RichTextArgument {
            tag: tag.ordinal(),
            argument: argument.ordinal(),
            part,
        },
    )
}

fn required_expr_site(
    module: &HirModule,
    owner: ExprId,
    role: HirExprSourceRole,
) -> Result<HirSourceSite, HirSourceQueryError> {
    let lookup = module.source_site(
        module.provenance().source_identity(),
        HirSourceQuery::Expr { owner, role },
    )?;
    match lookup.presence() {
        HirSourcePresence::Present(site) => Ok(site.clone()),
        HirSourcePresence::AbsentOptional => {
            unreachable!("required final-HIR source role is present in a published module")
        }
    }
}

fn optional_expr_site(
    module: &HirModule,
    owner: ExprId,
    role: HirExprSourceRole,
) -> Result<Option<HirSourceSite>, HirSourceQueryError> {
    let lookup = match module.source_site(
        module.provenance().source_identity(),
        HirSourceQuery::Expr { owner, role },
    ) {
        Ok(lookup) => lookup,
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner: actual_owner,
            role: actual_role,
        }) if actual_owner == owner && actual_role == role => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(match lookup.presence() {
        HirSourcePresence::Present(site) => Some(site.clone()),
        HirSourcePresence::AbsentOptional => None,
    })
}

fn argument_issue_code(issue: HirRichTextArgumentIssue) -> RichTextDiagnosticCode {
    match issue {
        HirRichTextArgumentIssue::KeyTooLong | HirRichTextArgumentIssue::ValueTooLong => {
            RichTextDiagnosticCode::ResourceLimit
        }
        HirRichTextArgumentIssue::EmptyKey
        | HirRichTextArgumentIssue::InvalidKey
        | HirRichTextArgumentIssue::InvalidEscape
        | HirRichTextArgumentIssue::UnterminatedQuote
        | HirRichTextArgumentIssue::MissingValue
        | HirRichTextArgumentIssue::DecoderFailure => RichTextDiagnosticCode::InvalidArgument,
    }
}

fn enum_schema_id(
    kind: RichTextValueKind,
) -> Option<arcweft_rich_text_schema::RichTextEnumSchemaId> {
    match kind {
        RichTextValueKind::ClosedEnum(id) => Some(id),
        _ => None,
    }
}

fn predicate_holds<P: Copy + Eq + Ord + 'static>(
    predicate: RichTextPropertyPredicate<P>,
    values: &BTreeMap<P, Vec<CheckedRichTextValue>>,
) -> bool {
    match predicate {
        RichTextPropertyPredicate::Present(property) => values.contains_key(&property),
        RichTextPropertyPredicate::BoolEquals { property, value } => values
            .get(&property)
            .and_then(|values| values.first())
            .is_some_and(|actual| matches!(actual, CheckedRichTextValue::Bool(actual) if *actual == value)),
        RichTextPropertyPredicate::EnumEquals { property, variant } => values
            .get(&property)
            .and_then(|values| values.first())
            .is_some_and(|actual| matches!(actual, CheckedRichTextValue::Enum(actual) if actual.variant == variant)),
    }
}

trait CheckedPropertyDomain: Copy + Eq + Ord + 'static {
    fn from_source_name(source: &str) -> Option<Self>;
    fn checked_property(self) -> CheckedRichTextProperty;
}

macro_rules! property_domain {
    ($type:ty, $variant:ident) => {
        impl CheckedPropertyDomain for $type {
            fn from_source_name(source: &str) -> Option<Self> {
                <$type>::from_source_name(source)
            }

            fn checked_property(self) -> CheckedRichTextProperty {
                CheckedRichTextProperty::$variant(self)
            }
        }
    };
}

property_domain!(DialogueControlProperty, Control);
property_domain!(DialogueHostProperty, Host);
property_domain!(RichTextDirectStyleProperty, DirectStyle);
property_domain!(RichTextStyleProperty, Style);
property_domain!(RichTextLayoutProperty, Layout);
property_domain!(RichTextTransformProperty, Transform);
property_domain!(RichTextObjectProperty, Object);

fn direct_style(value: HirRichTextDirectStyle) -> RichTextDirectStyle {
    match value {
        HirRichTextDirectStyle::Emphasis => RichTextDirectStyle::Emphasis,
        HirRichTextDirectStyle::Strong => RichTextDirectStyle::Strong,
        HirRichTextDirectStyle::Italic => RichTextDirectStyle::Italic,
        HirRichTextDirectStyle::Oblique => RichTextDirectStyle::Oblique,
        HirRichTextDirectStyle::Color => RichTextDirectStyle::Color,
        HirRichTextDirectStyle::Font => RichTextDirectStyle::Font,
        HirRichTextDirectStyle::Size => RichTextDirectStyle::Size,
        HirRichTextDirectStyle::Ruby => RichTextDirectStyle::Ruby,
    }
}

fn style_selector(value: HirRichTextStyleSelector) -> RichTextStyleSelector {
    match value {
        HirRichTextStyleSelector::Italic => RichTextStyleSelector::Italic,
        HirRichTextStyleSelector::Oblique => RichTextStyleSelector::Oblique,
        HirRichTextStyleSelector::Opacity => RichTextStyleSelector::Opacity,
        HirRichTextStyleSelector::Layer => RichTextStyleSelector::Layer,
        HirRichTextStyleSelector::ZIndex => RichTextStyleSelector::ZIndex,
    }
}

fn layout_selector(value: HirRichTextLayoutSelector) -> RichTextLayoutSelector {
    match value {
        HirRichTextLayoutSelector::HorizontalTb => RichTextLayoutSelector::HorizontalTb,
        HirRichTextLayoutSelector::VerticalRl => RichTextLayoutSelector::VerticalRl,
        HirRichTextLayoutSelector::VerticalLr => RichTextLayoutSelector::VerticalLr,
        HirRichTextLayoutSelector::Direction => RichTextLayoutSelector::Direction,
        HirRichTextLayoutSelector::RubyOver => RichTextLayoutSelector::RubyOver,
        HirRichTextLayoutSelector::RubyUnder => RichTextLayoutSelector::RubyUnder,
        HirRichTextLayoutSelector::RubyInterCharacter => RichTextLayoutSelector::RubyInterCharacter,
    }
}

fn transform_selector(value: HirRichTextTransformSelector) -> RichTextTransformSelector {
    match value {
        HirRichTextTransformSelector::Offset => RichTextTransformSelector::Offset,
        HirRichTextTransformSelector::Rotate => RichTextTransformSelector::Rotate,
        HirRichTextTransformSelector::Scale => RichTextTransformSelector::Scale,
        HirRichTextTransformSelector::Skew => RichTextTransformSelector::Skew,
    }
}

fn builtin_fx(value: HirBuiltinRichTextFx) -> BuiltinRichTextFx {
    match value {
        HirBuiltinRichTextFx::Wave => BuiltinRichTextFx::Wave,
        HirBuiltinRichTextFx::Shake => BuiltinRichTextFx::Shake,
        HirBuiltinRichTextFx::Jitter => BuiltinRichTextFx::Jitter,
        HirBuiltinRichTextFx::Arc => BuiltinRichTextFx::Arc,
        HirBuiltinRichTextFx::Spin => BuiltinRichTextFx::Spin,
        HirBuiltinRichTextFx::Pulse => BuiltinRichTextFx::Pulse,
        HirBuiltinRichTextFx::Motion => BuiltinRichTextFx::Motion,
        HirBuiltinRichTextFx::Typewriter => BuiltinRichTextFx::Typewriter,
        HirBuiltinRichTextFx::Sparkle => BuiltinRichTextFx::Sparkle,
        HirBuiltinRichTextFx::Shader => BuiltinRichTextFx::Shader,
    }
}

fn host_event(value: HirRichTextHostEvent) -> DialogueHostEventKind {
    match value {
        HirRichTextHostEvent::Voice => DialogueHostEventKind::Voice,
        HirRichTextHostEvent::Face => DialogueHostEventKind::Face,
        HirRichTextHostEvent::Pose => DialogueHostEventKind::Pose,
        HirRichTextHostEvent::Show => DialogueHostEventKind::Show,
        HirRichTextHostEvent::Hide => DialogueHostEventKind::Hide,
        HirRichTextHostEvent::Move => DialogueHostEventKind::Move,
        HirRichTextHostEvent::Scale => DialogueHostEventKind::Scale,
        HirRichTextHostEvent::Rotate => DialogueHostEventKind::Rotate,
        HirRichTextHostEvent::Animation => DialogueHostEventKind::Animation,
        HirRichTextHostEvent::StageShake => DialogueHostEventKind::Shake,
        HirRichTextHostEvent::TimedCue => DialogueHostEventKind::TimedCue,
        HirRichTextHostEvent::Call => DialogueHostEventKind::Call,
        HirRichTextHostEvent::Signal => DialogueHostEventKind::Signal,
    }
}

fn conditional_event(value: HirRichTextConditionalTag) -> DialogueHostEventKind {
    match value {
        HirRichTextConditionalTag::If => DialogueHostEventKind::ConditionalStart,
        HirRichTextConditionalTag::Else => DialogueHostEventKind::ConditionalElse,
        HirRichTextConditionalTag::EndIf => DialogueHostEventKind::ConditionalEnd,
    }
}
