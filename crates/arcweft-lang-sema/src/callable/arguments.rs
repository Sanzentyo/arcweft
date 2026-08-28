//! Shared argument coordinates used by schemas, facts, and query results.

use std::collections::BTreeSet;

use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use arcweft_lang_hir::{
    dialogue_application::{HirDialogueApplicationMetadataProjection, HirDialogueCoordinateKind},
    expr::{HirCallArgument, HirCallArgumentOrdinal, HirExprKind},
    identity::ExprId,
    module::HirModule,
};

use super::{
    CallableArgumentSlotIndex, CallableCandidateId, CallableGroupIndex, CallableName,
    CallableParameter, CallableParameterConsumer, CallableParameterIndex, CallableParameterPassing,
    CallableParameterPresence, CallableSignatureSchema, CheckedCallArgumentSlotSource,
    DialogueApplicationMetadataCoordinate, OpenArgumentId, SpreadArgumentPolicy,
    UnknownNamedArgumentPolicy,
};
use crate::types::TypeKind;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterCoordinate {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
}

/// Mapper-owned source shape before the candidate-wide type solution is
/// sealed. It describes how one authored expression projects into a logical
/// callable slot; it never claims the expression already has the parameter's
/// item type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedArgumentSourceProjection {
    Scalar,
    InferSpreadContainer { policy: CallableRestContainerPolicy },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallableRestContainerPolicy {
    Positional,
    Named,
}

impl CallableParameterCoordinate {
    pub const fn new(group: CallableGroupIndex, parameter: CallableParameterIndex) -> Self {
        Self { group, parameter }
    }
    pub const fn group(self) -> CallableGroupIndex {
        self.group
    }
    pub const fn parameter(self) -> CallableParameterIndex {
        self.parameter
    }
}

/// Candidate-specific, source-ordered mapping of one authored argument.
///
/// This carrier contains final-HIR expression identities and schema
/// coordinates only. It never reconstructs argument text or creates a second
/// call AST.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MappedCallArgument {
    source: ExprId,
    passing: MappedCallArgumentPassing,
    slots: Vec<MappedCallArgumentSlot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MappedCallArgumentPassing {
    Positional,
    Named,
    Spread,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MappedCallArgumentSlot {
    slot: CallableArgumentSlotIndex,
    source: CheckedCallArgumentSlotSource,
    coordinate: Option<CallableParameterCoordinate>,
    open: Option<OpenArgumentId>,
    source_projection: PreparedArgumentSourceProjection,
    expected: Option<TypeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCallArgumentMapping {
    candidate: Option<CallableCandidateId>,
    schema: super::CallableSignatureSchemaDigest,
    group: CallableGroupIndex,
    arguments: Vec<MappedCallArgument>,
    dialogue_application_metadata: Box<[PreparedDialogueApplicationMetadataArgument]>,
    omitted_parameters: usize,
    unchecked_or_open_slots: usize,
}

/// Accepted identity carried by one immediate Dialogue application metadata
/// coordinate.  The variants deliberately retain the domain identity rather
/// than a string spelling, so an `id` row can never be replayed as a
/// `text_key` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedDialogueApplicationMetadataEvidence {
    Id(DialogueLineId),
    TextKey(DialogueTextKey),
}

/// Application-owner-issued semantic source for one authored metadata
/// argument. The argument ordinal and unchanged expression identity are
/// paired before candidate mapping; the mapper may consume the row only when
/// the selected schema parameter has the exact metadata consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueApplicationMetadataArgument {
    argument: HirCallArgumentOrdinal,
    source: ExprId,
    coordinate: DialogueApplicationMetadataCoordinate,
    actual: TypeKind,
    evidence: PreparedDialogueApplicationMetadataEvidence,
}

/// Complete semantic metadata inventory issued by one outer Dialogue
/// application for its exact inner target Call. It is empty when the
/// application authored no `id`/`text_key` coordinates, but it is never
/// synthesized for an ordinary reusable call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueApplicationMetadataInventory {
    application: ExprId,
    target_call: ExprId,
    arguments: Box<[PreparedDialogueApplicationMetadataArgument]>,
}

impl MappedCallArgument {
    /// Returns the one authored HIR expression represented by this mapper
    /// row.  The source remains present even when a fixed empty spread maps to
    /// zero logical slots, so semantic child inventories never reconstruct it
    /// from the expanded slot list.
    pub(crate) const fn source(&self) -> ExprId {
        self.source
    }

    pub(crate) const fn passing(&self) -> MappedCallArgumentPassing {
        self.passing
    }

    pub(crate) fn slots(&self) -> &[MappedCallArgumentSlot] {
        &self.slots
    }
}

impl MappedCallArgumentSlot {
    pub(crate) const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }

    pub(crate) const fn source(&self) -> CheckedCallArgumentSlotSource {
        self.source
    }

    pub(crate) const fn coordinate(&self) -> Option<CallableParameterCoordinate> {
        self.coordinate
    }

    pub(crate) const fn source_projection(&self) -> PreparedArgumentSourceProjection {
        self.source_projection
    }

    pub(crate) const fn declared_expected(&self) -> Option<&TypeKind> {
        self.expected.as_ref()
    }

    pub(crate) const fn open_argument(&self) -> Option<&OpenArgumentId> {
        self.open.as_ref()
    }
}

impl PreparedCallArgumentMapping {
    pub(crate) fn candidate(&self) -> Option<&CallableCandidateId> {
        self.candidate.as_ref()
    }

    pub(crate) const fn schema(&self) -> super::CallableSignatureSchemaDigest {
        self.schema
    }

    pub(crate) const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub(crate) fn arguments(&self) -> &[MappedCallArgument] {
        &self.arguments
    }

    pub(crate) fn dialogue_application_metadata(
        &self,
    ) -> &[PreparedDialogueApplicationMetadataArgument] {
        &self.dialogue_application_metadata
    }

    /// Returns the expression children owned by the inner Call after the
    /// enclosing Dialogue application has retained its metadata coordinates.
    /// Metadata rows are reference-only from the inner call and therefore do
    /// not re-enter its semantic child inventory.
    pub(crate) fn owned_expression_sources(&self) -> Box<[ExprId]> {
        self.arguments
            .iter()
            .filter(|argument| {
                !self
                    .dialogue_application_metadata
                    .iter()
                    .any(|metadata| metadata.source == argument.source)
            })
            .map(MappedCallArgument::source)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    /// Consumes the one outer-application metadata inventory into the
    /// candidate mapper seal. Every row must name the exact authored argument
    /// and the exact schema consumer selected for that slot; metadata
    /// consumers without issued rows and issued rows without consumers both
    /// fail closed.
    pub(crate) fn seal_dialogue_application_metadata(
        mut self,
        call: ExprId,
        schema: &CallableSignatureSchema,
        inventory: Option<&PreparedDialogueApplicationMetadataInventory>,
    ) -> Result<Self, super::CallConstraintInvariant> {
        if schema.semantic_digest() != self.schema {
            return Err(super::CallConstraintInvariant::MalformedMapperSeal);
        }
        let supplied = inventory.map_or(
            &[][..],
            PreparedDialogueApplicationMetadataInventory::arguments,
        );
        if inventory.is_some_and(|inventory| {
            inventory.target_call != call
                || inventory.application == call
                || inventory.application.module() != call.module()
        }) {
            return Err(super::CallConstraintInvariant::MalformedMapperSeal);
        }
        let group = schema
            .group(self.group)
            .ok_or(super::CallConstraintInvariant::MalformedSchemaInventory)?;
        let mut consumed = vec![false; supplied.len()];
        let mut metadata = Vec::new();
        for (argument_index, argument) in self.arguments.iter().enumerate() {
            let ordinal = HirCallArgumentOrdinal::try_from_usize(argument_index)
                .map_err(|_| super::CallConstraintInvariant::MalformedMapperSeal)?;
            for slot in &argument.slots {
                let Some(coordinate) = slot.coordinate else {
                    continue;
                };
                let parameter = group
                    .parameter(coordinate.parameter())
                    .filter(|_| coordinate.group() == group.index())
                    .ok_or(super::CallConstraintInvariant::MalformedSchemaInventory)?;
                let CallableParameterConsumer::DialogueApplicationMetadata(expected) =
                    parameter.consumer()
                else {
                    continue;
                };
                if argument.slots.len() != 1
                    || argument.passing != MappedCallArgumentPassing::Named
                    || slot.slot.get() != 0
                    || slot.source_projection != PreparedArgumentSourceProjection::Scalar
                    || slot.source != CheckedCallArgumentSlotSource::Expression(argument.source)
                {
                    return Err(super::CallConstraintInvariant::MalformedMapperSeal);
                }
                let mut matches = supplied.iter().enumerate().filter(|(_, row)| {
                    row.argument == ordinal
                        && row.source == argument.source
                        && row.coordinate == *expected
                });
                let (row_index, row) = matches
                    .next()
                    .ok_or(super::CallConstraintInvariant::MalformedMapperSeal)?;
                if matches.next().is_some() || consumed[row_index] {
                    return Err(super::CallConstraintInvariant::MalformedMapperSeal);
                }
                consumed[row_index] = true;
                metadata.push(row.clone());
            }
        }
        if consumed.iter().any(|consumed| !consumed) {
            return Err(super::CallConstraintInvariant::MalformedMapperSeal);
        }
        self.dialogue_application_metadata = metadata.into_boxed_slice();
        Ok(self)
    }

    pub(crate) const fn omitted_parameters(&self) -> usize {
        self.omitted_parameters
    }

    pub(crate) const fn unchecked_or_open_slots(&self) -> usize {
        self.unchecked_or_open_slots
    }
}

impl PreparedDialogueApplicationMetadataArgument {
    pub(crate) fn seal(
        argument: HirCallArgumentOrdinal,
        source: ExprId,
        coordinate: DialogueApplicationMetadataCoordinate,
        actual: TypeKind,
        evidence: PreparedDialogueApplicationMetadataEvidence,
    ) -> Result<Self, super::CallConstraintInvariant> {
        let expected = match (&coordinate, &evidence) {
            (
                DialogueApplicationMetadataCoordinate::Id,
                PreparedDialogueApplicationMetadataEvidence::Id(_),
            ) => TypeKind::entity_ref(crate::types::EntityKind::DialogueLine),
            (
                DialogueApplicationMetadataCoordinate::TextKey,
                PreparedDialogueApplicationMetadataEvidence::TextKey(_),
            ) => TypeKind::entity_ref(crate::types::EntityKind::Text),
            _ => return Err(super::CallConstraintInvariant::MalformedMapperSeal),
        };
        if actual != expected {
            return Err(super::CallConstraintInvariant::MalformedMapperSeal);
        }
        Ok(Self {
            argument,
            source,
            coordinate,
            actual,
            evidence,
        })
    }

    pub(crate) const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    pub(crate) const fn source(&self) -> ExprId {
        self.source
    }

    pub(crate) const fn coordinate(&self) -> DialogueApplicationMetadataCoordinate {
        self.coordinate
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }

    pub(crate) const fn evidence(&self) -> &PreparedDialogueApplicationMetadataEvidence {
        &self.evidence
    }
}

impl PreparedDialogueApplicationMetadataInventory {
    pub(crate) fn seal(
        projection: &HirDialogueApplicationMetadataProjection,
        arguments: Box<[PreparedDialogueApplicationMetadataArgument]>,
    ) -> Result<Self, super::CallConstraintInvariant> {
        let application = projection.application();
        let target_call = projection.target_call();
        let mut sources = BTreeSet::new();
        let mut coordinates = BTreeSet::new();
        if application.module() != target_call.module()
            || projection.coordinates().len() != arguments.len()
            || projection
                .coordinates()
                .iter()
                .zip(arguments.iter())
                .any(|(projected, prepared)| {
                    projected.argument() != prepared.argument
                        || projected.value() != prepared.source
                        || !matches!(
                            (projected.kind(), prepared.coordinate),
                            (
                                HirDialogueCoordinateKind::Id,
                                DialogueApplicationMetadataCoordinate::Id,
                            ) | (
                                HirDialogueCoordinateKind::TextKey,
                                DialogueApplicationMetadataCoordinate::TextKey,
                            )
                        )
                })
            || arguments
                .iter()
                .any(|row| row.source.module() != application.module())
            || arguments
                .windows(2)
                .any(|rows| rows[0].argument >= rows[1].argument)
            || arguments
                .iter()
                .any(|row| !sources.insert(row.source) || !coordinates.insert(row.coordinate))
        {
            return Err(super::CallConstraintInvariant::MalformedMapperSeal);
        }
        Ok(Self {
            application,
            target_call,
            arguments,
        })
    }

    pub(crate) fn arguments(&self) -> &[PreparedDialogueApplicationMetadataArgument] {
        &self.arguments
    }
}

/// Maps one authored argument list to one candidate schema without checking
/// expression types. `None` is a terminal shape rejection for that candidate.
#[allow(
    clippy::too_many_lines,
    reason = "one source-ordered mapping transaction owns named, positional, recovered, and spread slot accounting"
)]
pub(crate) fn map_call_arguments(
    module: &HirModule,
    schema: &CallableSignatureSchema,
    candidate: &CallableCandidateId,
    group: CallableGroupIndex,
    arguments: &[HirCallArgument],
    implicit: Option<CallableParameterIndex>,
) -> Option<PreparedCallArgumentMapping> {
    let group = schema.group(group)?;
    let schema_digest = schema.semantic_digest();
    let parameters = group.parameters();
    let mut provided = vec![false; parameters.len()];
    if let Some(implicit) = implicit {
        *provided.get_mut(implicit.get())? = true;
    }
    let mut positional = 0usize;
    let mut unchecked_or_open_slots = 0usize;
    let mut mapped_arguments = Vec::with_capacity(arguments.len());

    for argument in arguments {
        let passing = match argument {
            HirCallArgument::Positional { .. } => MappedCallArgumentPassing::Positional,
            HirCallArgument::Named { .. } => MappedCallArgumentPassing::Named,
            HirCallArgument::Spread { .. } => MappedCallArgumentPassing::Spread,
        };
        let mut slots = Vec::new();
        match argument {
            HirCallArgument::Positional { .. } => {
                let parameter =
                    take_positional_parameter(parameters, &mut provided, &mut positional)?;
                push_mapped_slot(
                    &mut slots,
                    argument.value(),
                    group.index(),
                    Some(parameter),
                    &mut unchecked_or_open_slots,
                )?;
            }
            HirCallArgument::Named { .. } => {
                let name = argument.resolved_name()?;
                if let Some(parameter) = named_parameter(parameters, name) {
                    if parameter.passing() != CallableParameterPassing::RestNamed {
                        let index = parameter.index().get();
                        if provided[index] {
                            return None;
                        }
                        provided[index] = true;
                    }
                    push_mapped_slot(
                        &mut slots,
                        argument.value(),
                        group.index(),
                        Some(parameter),
                        &mut unchecked_or_open_slots,
                    )?;
                } else {
                    let name = CallableName::try_new(name.as_str()).ok()?;
                    let policy = schema.argument_policy();
                    if !schema.allows_open_name(&name) {
                        return None;
                    }
                    match policy.unknown_named() {
                        UnknownNamedArgumentPolicy::Reject => return None,
                        UnknownNamedArgumentPolicy::OpenSupply => {
                            unchecked_or_open_slots = unchecked_or_open_slots.checked_add(1)?;
                            slots.push(MappedCallArgumentSlot {
                                slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                                source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                                coordinate: None,
                                open: Some(OpenArgumentId::new(schema.semantic_digest(), name)),
                                source_projection: PreparedArgumentSourceProjection::Scalar,
                                expected: None,
                            });
                        }
                    }
                }
            }
            HirCallArgument::Spread { .. } => match schema.argument_policy().spread() {
                SpreadArgumentPolicy::Reject => return None,
                SpreadArgumentPolicy::Unchecked => {
                    unchecked_or_open_slots = unchecked_or_open_slots.checked_add(1)?;
                    let rest = parameters.iter().find(|parameter| {
                        parameter.passing() == CallableParameterPassing::RestPositional
                    });
                    slots.push(MappedCallArgumentSlot {
                        slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                        source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                        coordinate: rest.map(|parameter| {
                            CallableParameterCoordinate::new(group.index(), parameter.index())
                        }),
                        open: None,
                        source_projection: PreparedArgumentSourceProjection::InferSpreadContainer {
                            policy: CallableRestContainerPolicy::Positional,
                        },
                        expected: None,
                    });
                }
                SpreadArgumentPolicy::FixedLiteralOnly => {
                    let sources = fixed_literal_spread_sources(module, argument.value())?;
                    for source in sources {
                        let parameter =
                            take_positional_parameter(parameters, &mut provided, &mut positional)?;
                        push_mapped_slot(
                            &mut slots,
                            source,
                            group.index(),
                            Some(parameter),
                            &mut unchecked_or_open_slots,
                        )?;
                    }
                }
                SpreadArgumentPolicy::TypedRest => {
                    if let Some(sources) = fixed_literal_spread_sources(module, argument.value()) {
                        for source in sources {
                            let parameter = take_positional_parameter(
                                parameters,
                                &mut provided,
                                &mut positional,
                            )?;
                            push_mapped_slot(
                                &mut slots,
                                source,
                                group.index(),
                                Some(parameter),
                                &mut unchecked_or_open_slots,
                            )?;
                        }
                    } else {
                        let parameter = typed_rest_parameter(parameters)?;
                        unchecked_or_open_slots = unchecked_or_open_slots.checked_add(1)?;
                        slots.push(MappedCallArgumentSlot {
                            slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                            source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                            coordinate: Some(CallableParameterCoordinate::new(
                                group.index(),
                                parameter.index(),
                            )),
                            open: None,
                            source_projection:
                                PreparedArgumentSourceProjection::InferSpreadContainer {
                                    policy: if parameter.passing()
                                        == CallableParameterPassing::RestNamed
                                    {
                                        CallableRestContainerPolicy::Named
                                    } else {
                                        CallableRestContainerPolicy::Positional
                                    },
                                },
                            // The runtime container family determines how the
                            // rest item type is projected. Retain the typed
                            // parameter coordinate, but do not pretend that
                            // the container expression has the item type.
                            expected: None,
                        });
                    }
                }
            },
        }
        mapped_arguments.push(MappedCallArgument {
            source: argument.value(),
            passing,
            slots,
        });
    }

    if required_fixed_parameter_is_missing(parameters, &provided) {
        return None;
    }
    let omitted_parameters = parameters
        .iter()
        .filter(|parameter| {
            !provided[parameter.index().get()]
                && !matches!(
                    parameter.passing(),
                    CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
                )
        })
        .count();
    Some(PreparedCallArgumentMapping {
        candidate: Some(candidate.clone()),
        schema: schema_digest,
        group: group.index(),
        arguments: mapped_arguments,
        dialogue_application_metadata: Box::new([]),
        omitted_parameters,
        unchecked_or_open_slots,
    })
}

fn take_positional_parameter<'a>(
    parameters: &'a [CallableParameter],
    provided: &mut [bool],
    positional: &mut usize,
) -> Option<&'a CallableParameter> {
    let parameter = next_positional_parameter(parameters, provided, positional)?;
    if parameter.passing() != CallableParameterPassing::RestPositional {
        let index = parameter.index().get();
        provided[index] = true;
        *positional = index.checked_add(1)?;
    }
    Some(parameter)
}

fn push_mapped_slot(
    slots: &mut Vec<MappedCallArgumentSlot>,
    source: impl Into<CheckedCallArgumentSlotSource>,
    group: CallableGroupIndex,
    parameter: Option<&CallableParameter>,
    unchecked_or_open_slots: &mut usize,
) -> Option<()> {
    let slot = CallableArgumentSlotIndex::try_from_usize(slots.len()).ok()?;
    let (coordinate, expected) = parameter.map_or((None, None), |parameter| {
        let expected = parameter.declared_type().and_then(|declared| {
            if parameter.passing() == CallableParameterPassing::RestNamed {
                match declared {
                    TypeKind::Map { value, .. } => Some(value.as_ref().clone()),
                    _ => None,
                }
            } else {
                Some(declared.clone())
            }
        });
        (
            Some(CallableParameterCoordinate::new(group, parameter.index())),
            expected,
        )
    });
    if expected.is_none() {
        *unchecked_or_open_slots = unchecked_or_open_slots.checked_add(1)?;
    }
    slots.push(MappedCallArgumentSlot {
        slot,
        source: source.into(),
        coordinate,
        open: None,
        source_projection: PreparedArgumentSourceProjection::Scalar,
        expected,
    });
    Some(())
}

fn next_positional_parameter<'a>(
    parameters: &'a [CallableParameter],
    provided: &[bool],
    positional: &mut usize,
) -> Option<&'a CallableParameter> {
    while let Some(parameter) = parameters.get(*positional) {
        if provided[*positional]
            || matches!(
                parameter.passing(),
                CallableParameterPassing::NamedOnly | CallableParameterPassing::RestNamed
            )
        {
            *positional += 1;
        } else {
            break;
        }
    }
    parameters.get(*positional).or_else(|| {
        parameters
            .iter()
            .find(|parameter| parameter.passing() == CallableParameterPassing::RestPositional)
    })
}

fn named_parameter<'a>(
    parameters: &'a [CallableParameter],
    name: &arcweft_lang_hir::leaf::HirName,
) -> Option<&'a CallableParameter> {
    parameters
        .iter()
        .find(|parameter| {
            parameter
                .name()
                .is_some_and(|candidate| candidate.as_str() == name.as_str())
                && matches!(
                    parameter.passing(),
                    CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::NamedOnly
                )
        })
        .or_else(|| {
            parameters
                .iter()
                .find(|parameter| parameter.passing() == CallableParameterPassing::RestNamed)
        })
}

fn typed_rest_parameter<'a>(parameters: &'a [CallableParameter]) -> Option<&'a CallableParameter> {
    let mut rest = parameters.iter().filter(|parameter| {
        matches!(
            parameter.passing(),
            CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
        )
    });
    let parameter = rest.next()?;
    rest.next().is_none().then_some(parameter)
}

fn required_fixed_parameter_is_missing(
    parameters: &[CallableParameter],
    provided: &[bool],
) -> bool {
    parameters.iter().any(|parameter| {
        parameter.presence() == CallableParameterPresence::Required
            && !matches!(
                parameter.passing(),
                CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
            )
            && !provided[parameter.index().get()]
    })
}

fn fixed_literal_spread_sources(
    module: &HirModule,
    value: ExprId,
) -> Option<Vec<CheckedCallArgumentSlotSource>> {
    match module.resolve_expr(value).ok()?.kind() {
        HirExprKind::BracketSequence(sequence) => Some(
            sequence
                .elements()
                .iter()
                .copied()
                .map(CheckedCallArgumentSlotSource::Expression)
                .collect(),
        ),
        HirExprKind::NumericBracketSequence(sequence) => sequence
            .elements()
            .iter()
            .enumerate()
            .map(|(ordinal, _)| {
                Some(CheckedCallArgumentSlotSource::CompactNumericElement {
                    sequence: value,
                    ordinal: u32::try_from(ordinal).ok()?,
                })
            })
            .collect(),
        _ => None,
    }
}

impl From<ExprId> for CheckedCallArgumentSlotSource {
    fn from(value: ExprId) -> Self {
        Self::Expression(value)
    }
}
