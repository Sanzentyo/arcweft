//! Checker for the final-HIR dialogue content stream.
//!
//! The checker consumes only typed HIR nodes.  Point actions are validated
//! against the dialogue-owned control/host schemas; body-bearing operations
//! arrive as `ContentApplication` nodes and are sealed by the callable
//! resolver.  There is intentionally no delimiter, tag, or nesting stack in
//! this module.

use std::collections::BTreeMap;

use arcweft_dialogue::rich_text::{
    DialogueControlProperty, DialogueHostEventKind, DialogueHostProperty, DialogueRichTextControl,
};
use arcweft_lang_hir::dialogue_application::{
    HirDialogueContent, HirDialogueContentError, HirDialogueMarkId, HirDialogueNode,
    HirDialogueNodeKind, HirDialoguePointAction, HirDialoguePointActionArgument,
    HirDialoguePointActionArgumentId, HirDialoguePointActionIdentity,
    HirDialoguePointActionPayload, HirRichTextArgumentIssue, HirRichTextHostEvent,
};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::source_index::{
    HirDialogueNodeSourcePart, HirDialoguePointActionArgumentSourcePart,
    HirDialoguePointActionSourcePart, HirExprSourceRole, HirSourcePresence, HirSourceQuery,
    HirSourceQueryError, HirSourceSite,
};
use arcweft_rich_text_schema::{
    Multiplicity, PropertyPresence, RichTextPointActionSchema, RichTextValueKind,
};

use super::value::{checked_default, parse_checked_value};
use super::{
    CheckedDialogueControl, CheckedDialogueHostEvent, CheckedField, CheckedFieldOrigin,
    CheckedOwnerFields, CheckedRichTextProperty, CheckedVoiceSource,
    PreparedCheckedDialogueContent, PreparedCheckedDialogueMark,
    PreparedCheckedDialogueMarkCatalog, PreparedCheckedDialogueToken,
    PreparedCheckedRichTextAction, PreparedCheckedRichTextCheck, PreparedCheckedRichTextReport,
    PreparedContentApplicationRef, RichTextDefaultId, RichTextDiagnostic, RichTextDiagnosticCode,
    RichTextDiagnosticOwner, RichTextFailureEffect, RichTextRelatedSite,
};

/// Sole owner/schema-driven validator for final-HIR dialogue content.
#[derive(Clone, Copy, Debug, Default)]
pub struct RichTextContentChecker;

impl RichTextContentChecker {
    /// Validates one final-HIR dialogue-content value.
    pub(crate) fn check(
        module: &HirModule,
        content: &HirDialogueContent,
    ) -> Result<PreparedCheckedRichTextCheck, HirSourceQueryError> {
        let mut diagnostics = Vec::new();
        let mut markers = BTreeMap::new();
        let mut tokens = Vec::new();

        // Raw is an opaque typed body.  It has no nodes, and in particular is
        // never reparsed for a closing marker or a nested action.
        if let Some(raw) = content.raw_literal() {
            tokens.push(PreparedCheckedDialogueToken::RawLiteral(
                raw.as_str().into(),
            ));
        }

        for node in content.nodes() {
            match node.kind() {
                HirDialogueNodeKind::Text(text) => {
                    tokens.push(PreparedCheckedDialogueToken::Text(text.as_str().into()));
                }
                HirDialogueNodeKind::Escape(value) => {
                    tokens.push(PreparedCheckedDialogueToken::Escape(*value));
                }
                HirDialogueNodeKind::Interpolation(expression) => {
                    tokens.push(PreparedCheckedDialogueToken::Interpolation(*expression));
                }
                HirDialogueNodeKind::ContentApplication(expression) => {
                    tokens.push(PreparedCheckedDialogueToken::ContentApplication(
                        PreparedContentApplicationRef::new(node.id(), *expression),
                    ));
                }
                HirDialogueNodeKind::PointAction(action) => {
                    match Self::check_point_action(
                        module,
                        content,
                        node,
                        action,
                        &mut markers,
                        &mut diagnostics,
                    )? {
                        Some(action) => {
                            tokens.push(PreparedCheckedDialogueToken::PointAction(action));
                        }
                        None => {
                            // A rejected point event has a diagnostic; no
                            // success token is emitted for it.
                        }
                    }
                }
                HirDialogueNodeKind::LineBreak(kind) => {
                    tokens.push(PreparedCheckedDialogueToken::LineBreak(*kind));
                }
                HirDialogueNodeKind::Error(issue) => {
                    diagnostics.push(node_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        content_error_code(issue),
                    )?);
                }
            }
        }

        // `has_recovery` is intentionally not reconstructed from source.  The
        // final node algebra exposes all recoverable families needed here.
        let diagnostics_complete = content.nodes().iter().all(|node| {
            !matches!(node.kind(), HirDialogueNodeKind::Error(_))
                && !matches!(
                    node.kind(),
                    HirDialogueNodeKind::PointAction(action)
                        if action.arguments().iter().any(|argument| argument.issue().is_some())
                )
        });

        Ok(PreparedCheckedRichTextCheck::new(
            PreparedCheckedRichTextReport::new(
                PreparedCheckedDialogueContent::new(content.id(), tokens, diagnostics_complete),
                diagnostics,
            ),
            PreparedCheckedDialogueMarkCatalog::new(content.id(), markers),
        ))
    }

    fn check_point_action(
        module: &HirModule,
        content: &HirDialogueContent,
        node: &HirDialogueNode,
        action: &HirDialoguePointAction,
        markers: &mut BTreeMap<HirDialogueMarkId, PreparedCheckedDialogueMark>,
        diagnostics: &mut Vec<RichTextDiagnostic>,
    ) -> Result<Option<PreparedCheckedRichTextAction>, HirSourceQueryError> {
        match action.identity() {
            HirDialoguePointActionIdentity::Control(control) => {
                let owner = dialogue_control(*control);
                let positional = match owner {
                    DialogueRichTextControl::TimedWait => Some(DialogueControlProperty::Time),
                    DialogueRichTextControl::RevealRate => Some(DialogueControlProperty::Cps),
                    DialogueRichTextControl::Page
                    | DialogueRichTextControl::LineWait
                    | DialogueRichTextControl::HardBreak
                    | DialogueRichTextControl::Clear
                    | DialogueRichTextControl::Reset
                    | DialogueRichTextControl::Marker => None,
                };
                let checked = validate_schema(
                    module,
                    content.id().owner(),
                    node,
                    action.arguments(),
                    owner.schema(),
                    positional,
                )?;
                if !checked.diagnostics.is_empty() {
                    diagnostics.extend(checked.diagnostics);
                    return Ok(None);
                }
                let Some(action) = checked_control(owner, &checked.fields) else {
                    diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::SchemaUnavailable,
                    )?);
                    return Ok(None);
                };
                Ok(Some(PreparedCheckedRichTextAction::Control {
                    action,
                    fields: CheckedOwnerFields::new(checked.fields),
                }))
            }
            HirDialoguePointActionIdentity::Mark(name) => {
                if !action.arguments().is_empty()
                    || !matches!(action.payload(), HirDialoguePointActionPayload::None)
                {
                    diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::InvalidSelector,
                    )?);
                    return Ok(None);
                }
                let Some(mark) = content
                    .marks()
                    .iter()
                    .find(|mark| mark.action() == node.id() && mark.name() == name)
                else {
                    diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::InvalidSelector,
                    )?);
                    return Ok(None);
                };
                let id = mark.id();
                if markers
                    .insert(
                        id,
                        PreparedCheckedDialogueMark::new(id, mark.name().clone()),
                    )
                    .is_some()
                {
                    diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::Duplicate,
                    )?);
                    return Ok(None);
                }
                Ok(Some(PreparedCheckedRichTextAction::Marker { mark: id }))
            }
            HirDialoguePointActionIdentity::Host(host) => {
                let owner = dialogue_host(*host);
                let positional = match owner {
                    DialogueHostEventKind::Voice => Some(DialogueHostProperty::Source),
                    DialogueHostEventKind::Face => Some(DialogueHostProperty::Expression),
                    DialogueHostEventKind::Pose => Some(DialogueHostProperty::Pose),
                    DialogueHostEventKind::Show | DialogueHostEventKind::Hide => {
                        Some(DialogueHostProperty::Entity)
                    }
                    DialogueHostEventKind::Rotate => Some(DialogueHostProperty::Angle),
                    DialogueHostEventKind::Animation => Some(DialogueHostProperty::Animation),
                    DialogueHostEventKind::Signal => Some(DialogueHostProperty::Signal),
                    DialogueHostEventKind::TimedCue => Some(DialogueHostProperty::At),
                    DialogueHostEventKind::Move
                    | DialogueHostEventKind::Scale
                    | DialogueHostEventKind::Shake
                    | DialogueHostEventKind::Call => None,
                };
                let mut checked = validate_schema(
                    module,
                    content.id().owner(),
                    node,
                    action.arguments(),
                    owner.schema(),
                    positional,
                )?;
                if owner == DialogueHostEventKind::Move
                    && !checked.fields.iter().any(|field| {
                        matches!(
                            field.property(),
                            CheckedRichTextProperty::Host(
                                DialogueHostProperty::X | DialogueHostProperty::Y
                            )
                        ) && matches!(field.origin(), CheckedFieldOrigin::Authored { .. })
                    })
                {
                    checked.diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::Conflict,
                    )?);
                }
                if owner == DialogueHostEventKind::Scale
                    && !checked.fields.iter().any(|field| {
                        field.property() == CheckedRichTextProperty::Host(DialogueHostProperty::Y)
                    })
                    && let Some(value) = checked.fields.iter().find_map(|field| {
                        (field.property() == CheckedRichTextProperty::Host(DialogueHostProperty::X))
                            .then(|| field.value().clone())
                    })
                {
                    checked.fields.push(CheckedField::new(
                        CheckedRichTextProperty::Host(DialogueHostProperty::Y),
                        value,
                        CheckedFieldOrigin::Defaulted {
                            default_id: RichTextDefaultId::from_schema_ordinal(1),
                        },
                    ));
                }
                let invalid_payload =
                    !matches!(action.payload(), HirDialoguePointActionPayload::None)
                        && !matches!(
                            owner,
                            DialogueHostEventKind::TimedCue | DialogueHostEventKind::Call
                        );
                if invalid_payload {
                    checked.diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::InvalidArgument,
                    )?);
                }
                let invalid_schema = !checked.diagnostics.is_empty();
                diagnostics.extend(checked.diagnostics);
                if invalid_schema {
                    return Ok(None);
                }
                let Some(event) = checked_host_event(owner, &checked.fields, action.payload())
                else {
                    diagnostics.push(point_action_diagnostic(
                        module,
                        content.id().owner(),
                        node,
                        RichTextDiagnosticCode::InvalidArgument,
                    )?);
                    return Ok(None);
                };
                Ok(Some(PreparedCheckedRichTextAction::Host {
                    owner,
                    action: event,
                    fields: CheckedOwnerFields::new(checked.fields),
                }))
            }
        }
    }
}

struct SchemaCheckResult {
    fields: Vec<CheckedField>,
    diagnostics: Vec<RichTextDiagnostic>,
}

fn validate_schema<P: CheckedPropertyDomain>(
    module: &HirModule,
    owner: ExprId,
    node: &HirDialogueNode,
    arguments: &[HirDialoguePointActionArgument],
    schema: &'static RichTextPointActionSchema<P>,
    positional: Option<P>,
) -> Result<SchemaCheckResult, HirSourceQueryError> {
    let mut diagnostics = Vec::new();
    let mut authored: BTreeMap<P, Vec<(super::CheckedRichTextValue, CheckedFieldOrigin)>> =
        BTreeMap::new();
    let mut first_sites: BTreeMap<P, HirSourceSite> = BTreeMap::new();
    let mut positional_consumed = false;

    for argument in arguments {
        let (property, value) = match argument {
            HirDialoguePointActionArgument::Positional { value, .. } => {
                let Some(property) = positional.filter(|_| !positional_consumed) else {
                    diagnostics.push(argument_diagnostic(
                        module,
                        owner,
                        node,
                        argument.id(),
                        RichTextDiagnosticCode::PositionalForbidden,
                    )?);
                    continue;
                };
                positional_consumed = true;
                (property, value)
            }
            HirDialoguePointActionArgument::Named { name, value, .. } => {
                let Some(property) = P::from_source_name(name) else {
                    diagnostics.push(argument_diagnostic(
                        module,
                        owner,
                        node,
                        argument.id(),
                        RichTextDiagnosticCode::UnknownProperty,
                    )?);
                    continue;
                };
                (property, value)
            }
            HirDialoguePointActionArgument::Invalid { issue, .. } => {
                diagnostics.push(argument_diagnostic(
                    module,
                    owner,
                    node,
                    argument.id(),
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
                owner,
                node,
                argument.id(),
                RichTextDiagnosticCode::UnknownProperty,
            )?);
            continue;
        };
        let value_site = argument_site(
            module,
            owner,
            node.id(),
            argument.id(),
            HirDialoguePointActionArgumentSourcePart::Value,
        )?;
        let key_site = optional_argument_site(
            module,
            owner,
            node.id(),
            argument.id(),
            HirDialoguePointActionArgumentSourcePart::Name,
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
            let mut diagnostic = argument_diagnostic(module, owner, node, argument.id(), code)?;
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
            Err(code) => diagnostics.push(argument_diagnostic(
                module,
                owner,
                node,
                argument.id(),
                code,
            )?),
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
            if let PropertyPresence::Conditional { predicate } = spec.presence
                && !predicate
                    .holds(|property| values.get(&property).and_then(|entries| entries.first()))
            {
                if let Some(argument) = entries.iter().find_map(|(_, origin)| match origin {
                    CheckedFieldOrigin::Authored { argument, .. } => Some(*argument),
                    CheckedFieldOrigin::Defaulted { .. }
                    | CheckedFieldOrigin::TextProxyDefault { .. } => None,
                }) {
                    diagnostics.push(argument_diagnostic(
                        module,
                        owner,
                        node,
                        argument,
                        RichTextDiagnosticCode::PropertyNotInPhase,
                    )?);
                }
                continue;
            }
            fields.extend(entries.into_iter().map(|(value, origin)| {
                CheckedField::new(spec.id.checked_property(), value, origin)
            }));
            continue;
        }
        match spec.presence {
            PropertyPresence::Required => diagnostics.push(point_action_diagnostic(
                module,
                owner,
                node,
                RichTextDiagnosticCode::RequiredMissing,
            )?),
            PropertyPresence::Optional => {}
            PropertyPresence::Defaulted(default) => {
                let value = checked_default(default, enum_schema_id(spec.kind))
                    .expect("dialogue owner schemas contain valid defaults");
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
                if predicate
                    .holds(|property| values.get(&property).and_then(|entries| entries.first()))
                {
                    diagnostics.push(point_action_diagnostic(
                        module,
                        owner,
                        node,
                        RichTextDiagnosticCode::RequiredMissing,
                    )?);
                }
            }
        }
    }
    Ok(SchemaCheckResult {
        fields,
        diagnostics,
    })
}

fn checked_control(
    owner: DialogueRichTextControl,
    fields: &[CheckedField],
) -> Option<CheckedDialogueControl> {
    let value = |property| {
        fields
            .iter()
            .find(|field| field.property() == CheckedRichTextProperty::Control(property))
            .map(CheckedField::value)
    };
    Some(match owner {
        DialogueRichTextControl::Page => CheckedDialogueControl::Page,
        DialogueRichTextControl::LineWait => CheckedDialogueControl::LineWait,
        DialogueRichTextControl::HardBreak => CheckedDialogueControl::HardBreak,
        DialogueRichTextControl::TimedWait => {
            let super::CheckedRichTextValue::Duration(duration) =
                value(DialogueControlProperty::Time)?
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
            let super::CheckedRichTextValue::Milli(milli_cps) =
                value(DialogueControlProperty::Cps)?
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

fn checked_host_event(
    owner: DialogueHostEventKind,
    fields: &[CheckedField],
    payload: HirDialoguePointActionPayload,
) -> Option<CheckedDialogueHostEvent> {
    let property = |id| {
        fields
            .iter()
            .find(|field| field.property() == CheckedRichTextProperty::Host(id))
            .map(CheckedField::value)
    };
    let public_id = |id| {
        let super::CheckedRichTextValue::PublicId(value) = property(id)? else {
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
            let super::CheckedRichTextValue::Length(x) = property(DialogueHostProperty::X)? else {
                return None;
            };
            let super::CheckedRichTextValue::Length(y) = property(DialogueHostProperty::Y)? else {
                return None;
            };
            CheckedDialogueHostEvent::Move { x: *x, y: *y }
        }
        DialogueHostEventKind::Scale => {
            let super::CheckedRichTextValue::Milli(x) = property(DialogueHostProperty::X)? else {
                return None;
            };
            let super::CheckedRichTextValue::Milli(y) = property(DialogueHostProperty::Y)? else {
                return None;
            };
            CheckedDialogueHostEvent::Scale { x: *x, y: *y }
        }
        DialogueHostEventKind::Rotate => {
            let super::CheckedRichTextValue::Angle(angle) = property(DialogueHostProperty::Angle)?
            else {
                return None;
            };
            CheckedDialogueHostEvent::Rotate { angle: *angle }
        }
        DialogueHostEventKind::Animation => CheckedDialogueHostEvent::Animation {
            animation: public_id(DialogueHostProperty::Animation)?,
        },
        DialogueHostEventKind::Shake => {
            let super::CheckedRichTextValue::Length(amplitude) =
                property(DialogueHostProperty::Amp)?
            else {
                return None;
            };
            CheckedDialogueHostEvent::Shake {
                amplitude: *amplitude,
            }
        }
        DialogueHostEventKind::TimedCue => {
            let super::CheckedRichTextValue::Duration(at) = property(DialogueHostProperty::At)?
            else {
                return None;
            };
            let HirDialoguePointActionPayload::TimedCue(call) = payload else {
                return None;
            };
            CheckedDialogueHostEvent::TimedCue { at: *at, call }
        }
        DialogueHostEventKind::Call => {
            let HirDialoguePointActionPayload::Call(call) = payload else {
                return None;
            };
            CheckedDialogueHostEvent::Call { call }
        }
        DialogueHostEventKind::Signal => CheckedDialogueHostEvent::Signal {
            signal: public_id(DialogueHostProperty::Signal)?,
        },
    })
}

trait CheckedPropertyDomain: Copy + Eq + Ord + 'static {
    fn from_source_name(source: &str) -> Option<Self>;
    fn checked_property(self) -> CheckedRichTextProperty;
}

impl CheckedPropertyDomain for DialogueControlProperty {
    fn from_source_name(source: &str) -> Option<Self> {
        Self::from_source_name(source)
    }

    fn checked_property(self) -> CheckedRichTextProperty {
        CheckedRichTextProperty::Control(self)
    }
}

impl CheckedPropertyDomain for DialogueHostProperty {
    fn from_source_name(source: &str) -> Option<Self> {
        Self::from_source_name(source)
    }

    fn checked_property(self) -> CheckedRichTextProperty {
        CheckedRichTextProperty::Host(self)
    }
}

fn enum_schema_id(kind: RichTextValueKind) -> Option<arcweft_id::closed_enum::ClosedEnumDomainId> {
    match kind {
        RichTextValueKind::ClosedEnum(id) => Some(id),
        _ => None,
    }
}

fn dialogue_control(
    value: arcweft_lang_hir::dialogue_application::HirDialogueControl,
) -> DialogueRichTextControl {
    match value {
        arcweft_lang_hir::dialogue_application::HirDialogueControl::Page => {
            DialogueRichTextControl::Page
        }
        arcweft_lang_hir::dialogue_application::HirDialogueControl::LineWait => {
            DialogueRichTextControl::LineWait
        }
        arcweft_lang_hir::dialogue_application::HirDialogueControl::HardBreak => {
            DialogueRichTextControl::HardBreak
        }
        arcweft_lang_hir::dialogue_application::HirDialogueControl::TimedWait => {
            DialogueRichTextControl::TimedWait
        }
        arcweft_lang_hir::dialogue_application::HirDialogueControl::Clear => {
            DialogueRichTextControl::Clear
        }
        arcweft_lang_hir::dialogue_application::HirDialogueControl::Reset => {
            DialogueRichTextControl::Reset
        }
        arcweft_lang_hir::dialogue_application::HirDialogueControl::Speed => {
            DialogueRichTextControl::RevealRate
        }
    }
}

fn dialogue_host(value: HirRichTextHostEvent) -> DialogueHostEventKind {
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

fn content_error_code(issue: &HirDialogueContentError) -> RichTextDiagnosticCode {
    match issue {
        HirDialogueContentError::UnclassifiedToken
        | HirDialogueContentError::InvalidPointAction => RichTextDiagnosticCode::InvalidArgument,
    }
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

fn point_action_diagnostic(
    module: &HirModule,
    owner: ExprId,
    node: &HirDialogueNode,
    code: RichTextDiagnosticCode,
) -> Result<RichTextDiagnostic, HirSourceQueryError> {
    Ok(RichTextDiagnostic::new(
        code,
        RichTextDiagnosticOwner::PointAction(node.id()),
        point_action_site(
            module,
            owner,
            node.id(),
            HirDialoguePointActionSourcePart::Whole,
        )?,
        RichTextFailureEffect::RejectPointEvent,
    ))
}

fn node_diagnostic(
    module: &HirModule,
    owner: ExprId,
    node: &HirDialogueNode,
    code: RichTextDiagnosticCode,
) -> Result<RichTextDiagnostic, HirSourceQueryError> {
    Ok(RichTextDiagnostic::new(
        code,
        RichTextDiagnosticOwner::Node(node.id()),
        required_expr_site(
            module,
            owner,
            HirExprSourceRole::DialogueNode {
                ordinal: node.id().ordinal(),
                part: HirDialogueNodeSourcePart::Whole,
            },
        )?,
        RichTextFailureEffect::RejectCompilation,
    ))
}

fn argument_diagnostic(
    module: &HirModule,
    owner: ExprId,
    node: &HirDialogueNode,
    argument: HirDialoguePointActionArgumentId,
    code: RichTextDiagnosticCode,
) -> Result<RichTextDiagnostic, HirSourceQueryError> {
    Ok(RichTextDiagnostic::new(
        code,
        RichTextDiagnosticOwner::Argument(argument),
        argument_site(
            module,
            owner,
            node.id(),
            argument,
            HirDialoguePointActionArgumentSourcePart::Whole,
        )?,
        RichTextFailureEffect::RejectPointEvent,
    ))
}

fn point_action_site(
    module: &HirModule,
    owner: ExprId,
    action: arcweft_lang_hir::dialogue_application::HirDialogueNodeId,
    part: HirDialoguePointActionSourcePart,
) -> Result<HirSourceSite, HirSourceQueryError> {
    required_expr_site(
        module,
        owner,
        HirExprSourceRole::DialoguePointAction {
            ordinal: action.ordinal(),
            part,
        },
    )
}

fn argument_site(
    module: &HirModule,
    owner: ExprId,
    action: arcweft_lang_hir::dialogue_application::HirDialogueNodeId,
    argument: HirDialoguePointActionArgumentId,
    part: HirDialoguePointActionArgumentSourcePart,
) -> Result<HirSourceSite, HirSourceQueryError> {
    required_expr_site(
        module,
        owner,
        HirExprSourceRole::DialoguePointActionArgument {
            action: action.ordinal(),
            argument: argument.ordinal(),
            part,
        },
    )
}

fn optional_argument_site(
    module: &HirModule,
    owner: ExprId,
    action: arcweft_lang_hir::dialogue_application::HirDialogueNodeId,
    argument: HirDialoguePointActionArgumentId,
    part: HirDialoguePointActionArgumentSourcePart,
) -> Result<Option<HirSourceSite>, HirSourceQueryError> {
    optional_expr_site(
        module,
        owner,
        HirExprSourceRole::DialoguePointActionArgument {
            action: action.ordinal(),
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
