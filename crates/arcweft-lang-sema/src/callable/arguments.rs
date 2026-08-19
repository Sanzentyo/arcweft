//! Shared argument coordinates used by schemas, facts, and query results.

use arcweft_lang_hir::{
    expr::{HirCallArgument, HirExprKind},
    identity::ExprId,
    module::HirModule,
};

use super::{
    CallableArgumentSlotIndex, CallableGroupIndex, CallableParameter, CallableParameterIndex,
    CallableParameterPassing, CallableParameterPresence, CallableSignatureSchema,
    CheckedCallArgumentSlotSource, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
};
use crate::types::TypeKind;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterCoordinate {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
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
    slots: Vec<MappedCallArgumentSlot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MappedCallArgumentSlot {
    slot: CallableArgumentSlotIndex,
    source: CheckedCallArgumentSlotSource,
    coordinate: Option<CallableParameterCoordinate>,
    expected: Option<TypeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallArgumentMapping {
    arguments: Vec<MappedCallArgument>,
    omitted_parameters: usize,
    unchecked_or_open_slots: usize,
}

impl MappedCallArgument {
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

    pub(crate) const fn expected(&self) -> Option<&TypeKind> {
        self.expected.as_ref()
    }
}

impl CallArgumentMapping {
    pub(crate) fn arguments(&self) -> &[MappedCallArgument] {
        &self.arguments
    }

    pub(crate) const fn omitted_parameters(&self) -> usize {
        self.omitted_parameters
    }

    pub(crate) const fn unchecked_or_open_slots(&self) -> usize {
        self.unchecked_or_open_slots
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
    group: CallableGroupIndex,
    arguments: &[HirCallArgument],
    implicit: Option<CallableParameterIndex>,
) -> Option<CallArgumentMapping> {
    let group = schema.group(group)?;
    let parameters = group.parameters();
    let mut provided = vec![false; parameters.len()];
    if let Some(implicit) = implicit {
        *provided.get_mut(implicit.get())? = true;
    }
    let mut positional = 0usize;
    let mut unchecked_or_open_slots = 0usize;
    let mut mapped_arguments = Vec::with_capacity(arguments.len());

    for argument in arguments {
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
                    match schema.argument_policy().unknown_named() {
                        UnknownNamedArgumentPolicy::Reject => return None,
                        UnknownNamedArgumentPolicy::OpenChecked
                        | UnknownNamedArgumentPolicy::OpenUnchecked => {
                            unchecked_or_open_slots = unchecked_or_open_slots.checked_add(1)?;
                            slots.push(MappedCallArgumentSlot {
                                slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                                source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                                coordinate: None,
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
                    slots.push(MappedCallArgumentSlot {
                        slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                        source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                        coordinate: parameters
                            .iter()
                            .find(|parameter| {
                                parameter.passing() == CallableParameterPassing::RestPositional
                            })
                            .map(|parameter| {
                                CallableParameterCoordinate::new(group.index(), parameter.index())
                            }),
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
                        let parameter = parameters.iter().find(|parameter| {
                            parameter.passing() == CallableParameterPassing::RestPositional
                        })?;
                        unchecked_or_open_slots = unchecked_or_open_slots.checked_add(1)?;
                        slots.push(MappedCallArgumentSlot {
                            slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                            source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                            coordinate: Some(CallableParameterCoordinate::new(
                                group.index(),
                                parameter.index(),
                            )),
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
        mapped_arguments.push(MappedCallArgument { slots });
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
    Some(CallArgumentMapping {
        arguments: mapped_arguments,
        omitted_parameters,
        unchecked_or_open_slots,
    })
}

/// Builds the deterministic candidate-recovery projection when a schema
/// rejects the authored shape before parameter mapping can complete.
///
/// Each authored argument remains one unmapped expression evaluation. Spread
/// containers are not expanded because no accepting schema supplied logical
/// parameter slots for that candidate.
pub(crate) fn map_unmapped_call_arguments(
    arguments: &[HirCallArgument],
) -> Option<CallArgumentMapping> {
    let arguments = arguments
        .iter()
        .map(|argument| {
            Some(MappedCallArgument {
                slots: vec![MappedCallArgumentSlot {
                    slot: CallableArgumentSlotIndex::try_from_usize(0).ok()?,
                    source: CheckedCallArgumentSlotSource::Expression(argument.value()),
                    coordinate: None,
                    expected: None,
                }],
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CallArgumentMapping {
        unchecked_or_open_slots: arguments.len(),
        arguments,
        omitted_parameters: 0,
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
        let expected = match parameter.ty() {
            super::CallableParameterType::Exact(ty) => Some(ty.clone()),
            super::CallableParameterType::Unchecked => None,
        };
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
