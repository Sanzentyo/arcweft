//! Exact selected-call joins owned by the callable authority.
//!
//! A final semantic consumer may have HIR lookup evidence (for example a
//! typed receiver/method key), but it must not rebuild callable identity or
//! resolve a second catalog.  [`validate_selected_call`] is the sole seam for
//! joining one clean selected call with the current callable authority.

use std::sync::Arc;

use arcweft_lang_hir::expr::HirCallArgumentOrdinal;
use thiserror::Error;

use crate::{effect_row::EffectRow, types::TypeKind};

use super::{
    CallPoison, CallTargetFact, CallTargetFacts, CallableArgumentSlotIndex, CallableCandidateId,
    CallableFamily, CallableGroupIndex, CallableInstantiation, CallableParameterCoordinate,
    CallableParameterType, CallableSignatureSchemaDigest, CheckedCallableCatalog,
    CheckedCallableDigest, CheckedCallableFacts, CheckedCallableId, CheckedCallableLookupError,
    CheckedMethodLookup, ReceiverMethodKey, ResolvedCallable,
};

/// Failure while joining one final call fact with the current callable
/// authority.  Every variant is typed evidence failure; no spelling or
/// source-identity fallback is available.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedCallableJoinError {
    #[error("call target is not a clean selected callable")]
    NotSelected,
    #[error("selected call fact is recovered or rejected")]
    CallPoison,
    #[error("selected callable group does not match the call fact")]
    SelectedGroupMismatch,
    #[error("selected callable has no current parameter group")]
    CurrentGroupMissing,
    #[error("selected call next group does not match the callable schema")]
    NextGroupMismatch,
    #[error("selected call has no typed result")]
    MissingResult,
    #[error("selected call result type does not match its current/full or partial group")]
    ResultMismatch,
    #[error("function-value call has no exact full function type")]
    FunctionValueTypeMismatch,
    #[error("non-function-value call retained a function-value type")]
    UnexpectedFunctionValueType,
    #[error("call argument ordinal is not source contiguous")]
    ArgumentOrdinalMismatch,
    #[error("call argument slot index is not contiguous")]
    ArgumentSlotMismatch,
    #[error("selected call argument is not mapped to the current group")]
    ArgumentGroupMismatch,
    #[error("selected call argument mapping points outside the schema")]
    ArgumentParameterMissing,
    #[error("selected call argument has no inferred typed value")]
    ArgumentTypeMissing,
    #[error("selected call argument retained a non-clean poison state")]
    ArgumentPoison,
    #[error("selected call argument expected type disagrees with its schema")]
    ArgumentExpectedMismatch,
    #[error("selected call generic type observation conflicts")]
    GenericInstantiationMismatch,
    #[error("selected call effects do not match the current group")]
    EffectsMismatch,
    #[error("checked callable ID is missing for a catalog-backed selection")]
    MissingCheckedCallable,
    #[error("checked callable record is missing for a checked selection")]
    MissingCheckedRecord,
    #[error("selected checked callable record does not agree with the catalog row")]
    CatalogRecordMismatch,
    #[error("selected callable signature disagrees with the catalog row")]
    CatalogSignatureMismatch,
    #[error("catalog row effects disagree with the selected callable")]
    CatalogEffectsMismatch,
    #[error("checked callable lookup failed: {0:?}")]
    Catalog(CheckedCallableLookupError),
    #[error("a receiver/method key is required by the selected callable")]
    MissingReceiverKey,
    #[error("receiver/method evidence was supplied for a non-method callable")]
    UnexpectedReceiverKey,
    #[error("receiver type disagrees with the selected callable")]
    ReceiverTypeMismatch,
    #[error("receiver mode disagrees with the selected callable schema")]
    ReceiverModeMismatch,
    #[error("checked method lookup has no accepted candidate")]
    MethodLookupMissing,
    #[error("checked method lookup is ambiguous or inaccessible")]
    MethodLookupAmbiguous,
    #[error("checked method lookup selected a different ID")]
    MethodLookupMismatch,
    #[error("selected callable has no typed intrinsic authority")]
    MissingIntrinsicAuthority,
    #[error("selected callable family disagrees with its typed candidate")]
    IntrinsicFamilyMismatch,
}

/// Closed intrinsic candidate family tag retained by a checked join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntrinsicCallableCandidateTag {
    Fx,
    EnumVariant,
    Result,
    Option,
    Builtin,
    Agent,
    Presentation,
    Dialogue,
    Environment,
    Local,
    FunctionValue,
    Curried,
    CollectionMethod,
    PresentationHandleMethod,
    IntegerMethod,
    DomainMethod,
    CapacityMethod,
    StageMethod,
    LineContextMethod,
    LineSchedule,
    Drop,
    Promotion,
}

impl IntrinsicCallableCandidateTag {
    pub const fn semantic_tag(self) -> u16 {
        match self {
            Self::Fx => 0,
            Self::EnumVariant => 1,
            Self::Result => 2,
            Self::Option => 3,
            Self::Builtin => 4,
            Self::Agent => 5,
            Self::Presentation => 6,
            Self::Dialogue => 7,
            Self::Environment => 8,
            Self::Local => 9,
            Self::FunctionValue => 10,
            Self::Curried => 11,
            Self::CollectionMethod => 12,
            Self::PresentationHandleMethod => 13,
            Self::IntegerMethod => 14,
            Self::DomainMethod => 15,
            Self::CapacityMethod => 16,
            Self::StageMethod => 17,
            Self::LineContextMethod => 18,
            Self::LineSchedule => 19,
            Self::Drop => 20,
            Self::Promotion => 21,
        }
    }

    fn from_candidate(candidate: &CallableCandidateId) -> Option<Self> {
        Some(match candidate {
            CallableCandidateId::Fx(_) => Self::Fx,
            CallableCandidateId::EnumVariant(_) => Self::EnumVariant,
            CallableCandidateId::Result(_) => Self::Result,
            CallableCandidateId::Option(_) => Self::Option,
            CallableCandidateId::Builtin(_) => Self::Builtin,
            CallableCandidateId::Agent(_) => Self::Agent,
            CallableCandidateId::Presentation(_) => Self::Presentation,
            CallableCandidateId::Dialogue(_) => Self::Dialogue,
            CallableCandidateId::Environment(_) => Self::Environment,
            CallableCandidateId::Local(_) => Self::Local,
            CallableCandidateId::FunctionValue(_) => Self::FunctionValue,
            CallableCandidateId::Curried(_) => Self::Curried,
            CallableCandidateId::CollectionMethod(_) => Self::CollectionMethod,
            CallableCandidateId::PresentationHandleMethod(_) => Self::PresentationHandleMethod,
            CallableCandidateId::IntegerMethod(_) => Self::IntegerMethod,
            CallableCandidateId::DomainMethod(_) => Self::DomainMethod,
            CallableCandidateId::CapacityMethod(_) => Self::CapacityMethod,
            CallableCandidateId::StageMethod(_) => Self::StageMethod,
            CallableCandidateId::LineContextMethod(_) => Self::LineContextMethod,
            CallableCandidateId::LineSchedule(_) => Self::LineSchedule,
            CallableCandidateId::Drop(_) => Self::Drop,
            CallableCandidateId::Promotion(_) => Self::Promotion,
            CallableCandidateId::Project(_)
            | CallableCandidateId::Detached(_)
            | CallableCandidateId::Standard(_) => return None,
        })
    }
}

/// Stable digest of the selected callable's typed instantiation.  Generic
/// call-site bindings are not reconstructed here: current call facts do not
/// retain a raw substitution map, so the selected resolver-owned
/// [`CallableInstantiation`] is the accepted authority retained in the join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableInstantiationDigest([u8; 32]);

impl CallableInstantiationDigest {
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Semantic receiver mode proven by the selected callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableReceiverMode {
    None,
    Value {
        receiver: TypeKind,
    },
    Type {
        receiver: TypeKind,
    },
    Extension {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: super::CallableParameterIndex,
    },
}

/// One source-order argument slot after exact schema validation.
///
/// Only argument/slot ordinals, accepted coordinates, and typed semantic
/// digests are retained.  The originating `ExprId` is deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableArgumentSlot {
    slot: CallableArgumentSlotIndex,
    mapped: Option<CallableParameterCoordinate>,
    inferred: Option<[u8; 32]>,
    expected: Option<[u8; 32]>,
}

impl CheckedCallableArgumentSlot {
    pub const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }

    pub const fn mapped(&self) -> Option<CallableParameterCoordinate> {
        self.mapped
    }

    pub const fn inferred(&self) -> Option<[u8; 32]> {
        self.inferred
    }

    pub const fn expected(&self) -> Option<[u8; 32]> {
        self.expected
    }
}

/// One source-order argument after exact schema validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallableArgument {
    argument: HirCallArgumentOrdinal,
    slots: Box<[CheckedCallableArgumentSlot]>,
}

impl CheckedCallableArgument {
    pub const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    pub fn slots(&self) -> &[CheckedCallableArgumentSlot] {
        &self.slots
    }
}

/// Complete typed result of the callable-owner join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallableJoin {
    Catalog {
        id: Box<CheckedCallableId>,
        digest: CheckedCallableDigest,
        signature: CallableSignatureSchemaDigest,
        catalog_effects: EffectRow,
        effects: EffectRow,
        result: TypeKind,
        current_group: CallableGroupIndex,
        next_group: Option<CallableGroupIndex>,
        arguments: Box<[CheckedCallableArgument]>,
        receiver: CallableReceiverMode,
        instantiation: CallableInstantiationDigest,
    },
    Intrinsic {
        candidate: IntrinsicCallableCandidateTag,
        family: CallableFamily,
        signature: CallableSignatureSchemaDigest,
        effects: EffectRow,
        result: TypeKind,
        current_group: CallableGroupIndex,
        next_group: Option<CallableGroupIndex>,
        arguments: Box<[CheckedCallableArgument]>,
        receiver: CallableReceiverMode,
        instantiation: CallableInstantiationDigest,
    },
}

impl CheckedCallableJoin {
    pub const fn checked_id(&self) -> Option<&CheckedCallableId> {
        match self {
            Self::Catalog { id, .. } => Some(id),
            Self::Intrinsic { .. } => None,
        }
    }

    pub const fn digest(&self) -> Option<CheckedCallableDigest> {
        match self {
            Self::Catalog { digest, .. } => Some(*digest),
            Self::Intrinsic { .. } => None,
        }
    }

    pub const fn signature(&self) -> CallableSignatureSchemaDigest {
        match self {
            Self::Catalog { signature, .. } | Self::Intrinsic { signature, .. } => *signature,
        }
    }

    pub const fn instantiation(&self) -> CallableInstantiationDigest {
        match self {
            Self::Catalog { instantiation, .. } | Self::Intrinsic { instantiation, .. } => {
                *instantiation
            }
        }
    }

    pub const fn current_group(&self) -> CallableGroupIndex {
        match self {
            Self::Catalog { current_group, .. } | Self::Intrinsic { current_group, .. } => {
                *current_group
            }
        }
    }

    pub const fn next_group(&self) -> Option<CallableGroupIndex> {
        match self {
            Self::Catalog { next_group, .. } | Self::Intrinsic { next_group, .. } => *next_group,
        }
    }

    pub const fn result(&self) -> &TypeKind {
        match self {
            Self::Catalog { result, .. } | Self::Intrinsic { result, .. } => result,
        }
    }

    pub const fn effects(&self) -> &EffectRow {
        match self {
            Self::Catalog { effects, .. } | Self::Intrinsic { effects, .. } => effects,
        }
    }

    pub fn arguments(&self) -> &[CheckedCallableArgument] {
        match self {
            Self::Catalog { arguments, .. } | Self::Intrinsic { arguments, .. } => arguments,
        }
    }

    pub const fn receiver(&self) -> &CallableReceiverMode {
        match self {
            Self::Catalog { receiver, .. } | Self::Intrinsic { receiver, .. } => receiver,
        }
    }

    /// Stable semantic transcript for the fully checked join.
    pub fn semantic_digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.checked-callable-authority-join.v1\0");
        match self {
            Self::Catalog {
                id,
                digest,
                signature,
                catalog_effects,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            } => {
                hasher.update(&[0]);
                hasher.update(id.semantic_digest().as_bytes());
                hasher.update(digest.as_bytes());
                hasher.update(signature.as_bytes());
                write_effect(&mut hasher, catalog_effects);
                write_effect(&mut hasher, effects);
                write_type(&mut hasher, result);
                write_group(&mut hasher, *current_group);
                write_optional_group(&mut hasher, *next_group);
                write_arguments(&mut hasher, arguments);
                write_receiver(&mut hasher, receiver);
                hasher.update(instantiation.bytes());
            }
            Self::Intrinsic {
                candidate,
                family,
                signature,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            } => {
                hasher.update(&[1]);
                hasher.update(&candidate.semantic_tag().to_le_bytes());
                hasher.update(&[callable_family_tag(*family)]);
                hasher.update(signature.as_bytes());
                write_effect(&mut hasher, effects);
                write_type(&mut hasher, result);
                write_group(&mut hasher, *current_group);
                write_optional_group(&mut hasher, *next_group);
                write_arguments(&mut hasher, arguments);
                write_receiver(&mut hasher, receiver);
                hasher.update(instantiation.bytes());
            }
        }
        *hasher.finalize().as_bytes()
    }
}

/// The one callable-owner validation seam for final semantic consumers.
///
/// `method_key` is optional HIR-derived lookup evidence.  It is compared to
/// the accepted typed receiver key and is never retained as transcript
/// identity.  All other authority comes from `facts`, the selected resolver
/// product, and the supplied current checked catalog.
pub fn validate_selected_call(
    facts: &CallTargetFacts,
    catalog: &CheckedCallableCatalog,
    method_key: Option<&ReceiverMethodKey>,
) -> Result<CheckedCallableJoin, CheckedCallableJoinError> {
    let selected = match facts.target() {
        CallTargetFact::Selected { selected, .. } => selected,
        CallTargetFact::Ambiguous { .. }
        | CallTargetFact::Rejected { .. }
        | CallTargetFact::NonCallable { .. }
        | CallTargetFact::Missing { .. } => return Err(CheckedCallableJoinError::NotSelected),
    };
    if facts.poison() != CallPoison::Clean {
        return Err(CheckedCallableJoinError::CallPoison);
    }

    let current_group = facts.current_group();
    let function_value = matches!(selected.id(), CallableCandidateId::FunctionValue(_));
    let next_group =
        validate_selected_groups(selected, current_group, facts.next_group(), function_value)?;

    let checked_row = checked_row(selected, catalog)?;
    let accepted_receiver_key = checked_row
        .facts
        .and_then(|row| row.record().receiver_method_key());
    validate_method_lookup(
        selected,
        checked_row.id.as_ref(),
        checked_row.facts,
        method_key,
        catalog,
    )?;
    let receiver = receiver_mode(selected, accepted_receiver_key.as_ref(), method_key)?;
    let (arguments, substitutions) =
        validate_arguments(selected, facts, current_group, checked_row.facts)?;

    let raw_result = selected
        .result_type_for_group(current_group)
        .ok_or(CheckedCallableJoinError::ResultMismatch)?;
    let result = substitutions.apply(&raw_result);
    validate_result_type(facts.result(), &result)?;
    if function_value {
        let full = callable_schema_type(selected.schema())
            .map(|ty| substitutions.apply(&ty))
            .ok_or(CheckedCallableJoinError::FunctionValueTypeMismatch)?;
        if facts.function_value_type() != Some(&full) {
            return Err(CheckedCallableJoinError::FunctionValueTypeMismatch);
        }
    } else if facts.function_value_type().is_some() {
        return Err(CheckedCallableJoinError::UnexpectedFunctionValueType);
    }

    let expected_effects =
        expected_call_effects(selected, current_group, checked_row.effects.as_ref());
    if facts.effects() != &expected_effects {
        return Err(CheckedCallableJoinError::EffectsMismatch);
    }
    let signature = selected.schema().semantic_digest();
    let instantiation = callable_instantiation_digest(selected.instantiation());
    match (checked_row.id, checked_row.facts, checked_row.effects) {
        (Some(id), Some(_row), Some(catalog_effects)) => Ok(CheckedCallableJoin::Catalog {
            digest: id.semantic_digest(),
            id: Box::new(id),
            signature,
            catalog_effects,
            effects: facts.effects().clone(),
            result,
            current_group,
            next_group,
            arguments,
            receiver,
            instantiation,
        }),
        (None, None, None) => {
            let candidate = IntrinsicCallableCandidateTag::from_candidate(selected.id())
                .ok_or(CheckedCallableJoinError::MissingIntrinsicAuthority)?;
            if selected.family() != selected.id().intrinsic_family() {
                return Err(CheckedCallableJoinError::IntrinsicFamilyMismatch);
            }
            Ok(CheckedCallableJoin::Intrinsic {
                candidate,
                family: selected.family(),
                signature,
                effects: facts.effects().clone(),
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            })
        }
        _ => Err(CheckedCallableJoinError::MissingCheckedRecord),
    }
}

struct CheckedRow<'a> {
    id: Option<CheckedCallableId>,
    facts: Option<&'a CheckedCallableFacts>,
    effects: Option<EffectRow>,
}

fn checked_row<'a>(
    selected: &ResolvedCallable,
    catalog: &'a CheckedCallableCatalog,
) -> Result<CheckedRow<'a>, CheckedCallableJoinError> {
    match (selected.checked(), selected.record()) {
        (Some(id), Some(record)) => {
            let row = catalog
                .callable(id)
                .map_err(CheckedCallableJoinError::Catalog)?;
            if row.id() != id || row.record().id() != selected.id() {
                return Err(CheckedCallableJoinError::CatalogRecordMismatch);
            }
            if !Arc::ptr_eq(row.record(), record) {
                return Err(CheckedCallableJoinError::CatalogRecordMismatch);
            }
            if row.signature().semantic_digest() != selected.schema().semantic_digest() {
                return Err(CheckedCallableJoinError::CatalogSignatureMismatch);
            }
            if selected
                .schema()
                .effects()
                .fixed_row()
                .is_some_and(|expected| row.exposed_row() != expected)
            {
                return Err(CheckedCallableJoinError::CatalogEffectsMismatch);
            }
            Ok(CheckedRow {
                id: Some(id.clone()),
                facts: Some(row),
                effects: Some(row.exposed_row().clone()),
            })
        }
        (None, None) if selected.schema().effects().fixed_row().is_some() => Ok(CheckedRow {
            id: None,
            facts: None,
            effects: None,
        }),
        (None, None | Some(_)) => Err(CheckedCallableJoinError::MissingIntrinsicAuthority),
        (Some(_), None) => Err(CheckedCallableJoinError::MissingCheckedRecord),
    }
}

fn validate_selected_groups(
    selected: &ResolvedCallable,
    current_group: CallableGroupIndex,
    actual_next_group: Option<CallableGroupIndex>,
    function_value: bool,
) -> Result<Option<CallableGroupIndex>, CheckedCallableJoinError> {
    if !function_value && selected.call_group() != current_group {
        return Err(CheckedCallableJoinError::SelectedGroupMismatch);
    }
    if selected.schema().group(current_group).is_none() {
        return Err(CheckedCallableJoinError::CurrentGroupMissing);
    }
    let next_group = selected.next_group_for(current_group);
    if next_group != actual_next_group {
        return Err(CheckedCallableJoinError::NextGroupMismatch);
    }
    Ok(next_group)
}

fn validate_result_type(
    actual: Option<&TypeKind>,
    expected: &TypeKind,
) -> Result<(), CheckedCallableJoinError> {
    let actual = actual.ok_or(CheckedCallableJoinError::MissingResult)?;
    (actual == expected)
        .then_some(())
        .ok_or(CheckedCallableJoinError::ResultMismatch)
}

fn receiver_mode(
    selected: &ResolvedCallable,
    accepted_key: Option<&ReceiverMethodKey>,
    method_key: Option<&ReceiverMethodKey>,
) -> Result<CallableReceiverMode, CheckedCallableJoinError> {
    match (accepted_key, method_key) {
        (Some(accepted), Some(actual)) if accepted != actual => {
            return Err(CheckedCallableJoinError::ReceiverTypeMismatch);
        }
        _ => {}
    }

    match selected.instantiation() {
        CallableInstantiation::None => {
            if method_key.is_some() {
                return Err(CheckedCallableJoinError::UnexpectedReceiverKey);
            }
            Ok(CallableReceiverMode::None)
        }
        CallableInstantiation::ExpectedEnum { .. }
        | CallableInstantiation::Result { .. }
        | CallableInstantiation::Option { .. }
        | CallableInstantiation::Character { .. }
        | CallableInstantiation::Curried { .. } => {
            if accepted_key.is_some() || method_key.is_some() {
                return Err(CheckedCallableJoinError::ReceiverModeMismatch);
            }
            Ok(CallableReceiverMode::None)
        }
        CallableInstantiation::Receiver { receiver } => {
            let key = method_key.ok_or(CheckedCallableJoinError::MissingReceiverKey)?;
            if key.receiver() != receiver {
                return Err(CheckedCallableJoinError::ReceiverTypeMismatch);
            }
            Ok(CallableReceiverMode::Value {
                receiver: receiver.clone(),
            })
        }
        CallableInstantiation::TypeReceiver { receiver } => {
            let key = method_key.ok_or(CheckedCallableJoinError::MissingReceiverKey)?;
            if key.receiver() != receiver.receiver() {
                return Err(CheckedCallableJoinError::ReceiverTypeMismatch);
            }
            Ok(CallableReceiverMode::Type {
                receiver: receiver.receiver().clone(),
            })
        }
        CallableInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => {
            let key = method_key.ok_or(CheckedCallableJoinError::MissingReceiverKey)?;
            if key.receiver() != receiver {
                return Err(CheckedCallableJoinError::ReceiverTypeMismatch);
            }
            if selected.schema().extension_receiver()
                != Some(super::CallableExtensionReceiver::new(*group, *parameter))
            {
                return Err(CheckedCallableJoinError::ReceiverModeMismatch);
            }
            Ok(CallableReceiverMode::Extension {
                receiver: receiver.clone(),
                group: *group,
                parameter: *parameter,
            })
        }
    }
}

fn validate_method_lookup(
    selected: &ResolvedCallable,
    checked: Option<&CheckedCallableId>,
    row: Option<&CheckedCallableFacts>,
    method_key: Option<&ReceiverMethodKey>,
    catalog: &CheckedCallableCatalog,
) -> Result<(), CheckedCallableJoinError> {
    if !matches!(
        selected.instantiation(),
        CallableInstantiation::Receiver { .. }
            | CallableInstantiation::TypeReceiver { .. }
            | CallableInstantiation::Extension { .. }
    ) {
        return Ok(());
    }
    let Some(row) = row else {
        return Ok(());
    };
    let accepted_key = row
        .record()
        .receiver_method_key()
        .ok_or(CheckedCallableJoinError::MissingReceiverKey)?;
    let key = method_key.ok_or(CheckedCallableJoinError::MissingReceiverKey)?;
    if key != &accepted_key {
        return Err(CheckedCallableJoinError::ReceiverTypeMismatch);
    }
    let selected = checked.ok_or(CheckedCallableJoinError::MissingCheckedCallable)?;
    validate_method_lookup_result(selected, catalog.method(key))
}

fn validate_method_lookup_result(
    selected: &CheckedCallableId,
    lookup: CheckedMethodLookup,
) -> Result<(), CheckedCallableJoinError> {
    match lookup {
        CheckedMethodLookup::Unique(candidate) if candidate.as_ref() == selected => Ok(()),
        CheckedMethodLookup::Unique(_) => Err(CheckedCallableJoinError::MethodLookupMismatch),
        CheckedMethodLookup::Absent => Err(CheckedCallableJoinError::MethodLookupMissing),
        CheckedMethodLookup::Ambiguous(_) | CheckedMethodLookup::Inaccessible(_) => {
            Err(CheckedCallableJoinError::MethodLookupAmbiguous)
        }
    }
}

fn validate_arguments(
    selected: &ResolvedCallable,
    facts: &CallTargetFacts,
    current_group: CallableGroupIndex,
    row: Option<&CheckedCallableFacts>,
) -> Result<
    (
        Box<[CheckedCallableArgument]>,
        crate::types::TypeParameterSubstitutions,
    ),
    CheckedCallableJoinError,
> {
    let mut substitutions = crate::types::TypeParameterSubstitutions::default();
    if let Some((coordinate, receiver)) = receiver_binding(selected, row) {
        let parameter = selected
            .schema()
            .group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .ok_or(CheckedCallableJoinError::ArgumentParameterMissing)?;
        let CallableParameterType::Exact(declared) = parameter.ty() else {
            return Err(CheckedCallableJoinError::GenericInstantiationMismatch);
        };
        if !substitutions.observe(declared, receiver) {
            return Err(CheckedCallableJoinError::GenericInstantiationMismatch);
        }
    }
    let mut arguments = Vec::with_capacity(facts.arguments().len());
    for (argument_index, argument) in facts.arguments().iter().enumerate() {
        let expected = HirCallArgumentOrdinal::try_from_usize(argument_index)
            .map_err(|_| CheckedCallableJoinError::ArgumentOrdinalMismatch)?;
        if argument.argument() != expected {
            return Err(CheckedCallableJoinError::ArgumentOrdinalMismatch);
        }
        if argument.poison() != CallPoison::Clean {
            return Err(CheckedCallableJoinError::ArgumentPoison);
        }
        let mut slots = Vec::with_capacity(argument.slots().len());
        for (slot_index, slot) in argument.slots().iter().enumerate() {
            let expected_slot = CallableArgumentSlotIndex::try_from_usize(slot_index)
                .map_err(|_| CheckedCallableJoinError::ArgumentSlotMismatch)?;
            if slot.slot() != expected_slot {
                return Err(CheckedCallableJoinError::ArgumentSlotMismatch);
            }
            if slot.poison() != CallPoison::Clean {
                return Err(CheckedCallableJoinError::ArgumentPoison);
            }
            let inferred = slot
                .inferred()
                .ok_or(CheckedCallableJoinError::ArgumentTypeMissing)?;
            let expected_type = if let Some(coordinate) = slot.mapped() {
                if coordinate.group() != current_group {
                    return Err(CheckedCallableJoinError::ArgumentGroupMismatch);
                }
                let parameter = selected
                    .schema()
                    .group(coordinate.group())
                    .and_then(|group| group.parameter(coordinate.parameter()))
                    .ok_or(CheckedCallableJoinError::ArgumentParameterMissing)?;
                match parameter.ty() {
                    CallableParameterType::Exact(declared) => {
                        if !substitutions.observe(declared, inferred) {
                            return Err(CheckedCallableJoinError::GenericInstantiationMismatch);
                        }
                        let expected = substitutions.apply(declared);
                        if slot.expected() != Some(&expected) {
                            return Err(CheckedCallableJoinError::ArgumentExpectedMismatch);
                        }
                        Some(expected)
                    }
                    CallableParameterType::Unchecked => {
                        if slot.expected().is_some() {
                            return Err(CheckedCallableJoinError::ArgumentExpectedMismatch);
                        }
                        None
                    }
                }
            } else {
                if slot.expected().is_some() {
                    return Err(CheckedCallableJoinError::ArgumentExpectedMismatch);
                }
                None
            };
            slots.push(CheckedCallableArgumentSlot {
                slot: slot.slot(),
                mapped: slot.mapped(),
                inferred: Some(*inferred.semantic_identity_digest().as_bytes()),
                expected: expected_type.map(|ty| *ty.semantic_identity_digest().as_bytes()),
            });
        }
        arguments.push(CheckedCallableArgument {
            argument: argument.argument(),
            slots: slots.into_boxed_slice(),
        });
    }
    Ok((arguments.into_boxed_slice(), substitutions))
}

fn receiver_binding<'a>(
    selected: &'a ResolvedCallable,
    row: Option<&CheckedCallableFacts>,
) -> Option<(CallableParameterCoordinate, &'a TypeKind)> {
    match selected.instantiation() {
        CallableInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => Some((
            CallableParameterCoordinate::new(*group, *parameter),
            receiver,
        )),
        CallableInstantiation::Receiver { .. } | CallableInstantiation::TypeReceiver { .. }
            if row.is_some_and(|row| row.record().method_role().is_some()) =>
        {
            let receiver = match selected.instantiation() {
                CallableInstantiation::Receiver { receiver } => receiver,
                CallableInstantiation::TypeReceiver { receiver } => receiver.receiver(),
                _ => unreachable!("receiver binding arm is restricted to receiver modes"),
            };
            Some((
                CallableParameterCoordinate::new(
                    CallableGroupIndex::try_from_usize(0)
                        .expect("zero callable group is representable"),
                    super::CallableParameterIndex::try_from_usize(0)
                        .expect("zero callable parameter is representable"),
                ),
                receiver,
            ))
        }
        _ => None,
    }
}

fn callable_schema_type(schema: &super::CallableSignatureSchema) -> Option<TypeKind> {
    let mut result = schema.result().clone();
    for group in schema.groups().iter().rev() {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| match parameter.ty() {
                CallableParameterType::Exact(ty) => Some(ty.clone()),
                CallableParameterType::Unchecked => None,
            })
            .collect::<Option<Vec<_>>>()?;
        result = TypeKind::function_with_effects(
            parameters,
            result,
            schema
                .effects()
                .fixed_row()
                .cloned()
                .unwrap_or_else(EffectRow::unknown),
        );
    }
    Some(result)
}

fn expected_call_effects(
    selected: &ResolvedCallable,
    current_group: CallableGroupIndex,
    catalog_effects: Option<&EffectRow>,
) -> EffectRow {
    if selected.next_group_for(current_group).is_some() {
        return EffectRow::closed(crate::effects::EffectSet::new());
    }
    selected
        .schema()
        .effects()
        .fixed_row()
        .cloned()
        .or_else(|| catalog_effects.cloned())
        .unwrap_or_else(EffectRow::unknown)
}

fn callable_instantiation_digest(
    instantiation: &CallableInstantiation,
) -> CallableInstantiationDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.lang.callable-instantiation.v1\0");
    match instantiation {
        CallableInstantiation::None => {
            hasher.update(&[0]);
        }
        CallableInstantiation::ExpectedEnum { expected } => {
            hasher.update(&[1]);
            write_type(&mut hasher, expected);
        }
        CallableInstantiation::Result { kind, expected } => {
            hasher.update(&[
                2,
                u8::from(matches!(kind, super::ResultConstructorKind::Err)),
            ]);
            write_optional_type(&mut hasher, expected.as_ref());
        }
        CallableInstantiation::Option { expected } => {
            hasher.update(&[3]);
            write_optional_type(&mut hasher, expected.as_ref());
        }
        CallableInstantiation::Character { owner } => {
            hasher.update(&[4]);
            write_bytes(&mut hasher, owner.character().as_str().as_bytes());
        }
        CallableInstantiation::Receiver { receiver } => {
            hasher.update(&[5]);
            write_type(&mut hasher, receiver);
        }
        CallableInstantiation::TypeReceiver { receiver } => {
            hasher.update(&[6]);
            write_type(&mut hasher, receiver.receiver());
        }
        CallableInstantiation::Curried { base, group } => {
            hasher.update(&[7]);
            let tag = IntrinsicCallableCandidateTag::from_candidate(base)
                .map_or(u16::MAX, IntrinsicCallableCandidateTag::semantic_tag);
            hasher.update(&tag.to_le_bytes());
            write_group(&mut hasher, *group);
        }
        CallableInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => {
            hasher.update(&[8]);
            write_type(&mut hasher, receiver);
            write_group(&mut hasher, *group);
            hasher.update(
                &u32::try_from(parameter.get())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
        }
    }
    CallableInstantiationDigest(*hasher.finalize().as_bytes())
}

fn write_type(hasher: &mut blake3::Hasher, ty: &TypeKind) {
    hasher.update(ty.semantic_identity_digest().as_bytes());
}

fn write_optional_type(hasher: &mut blake3::Hasher, ty: Option<&TypeKind>) {
    match ty {
        Some(ty) => {
            hasher.update(&[1]);
            write_type(hasher, ty);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn write_group(hasher: &mut blake3::Hasher, group: CallableGroupIndex) {
    hasher.update(&u32::try_from(group.get()).unwrap_or(u32::MAX).to_le_bytes());
}

fn write_optional_group(hasher: &mut blake3::Hasher, group: Option<CallableGroupIndex>) {
    match group {
        Some(group) => {
            hasher.update(&[1]);
            write_group(hasher, group);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn write_effect(hasher: &mut blake3::Hasher, effect: &EffectRow) {
    let ty = TypeKind::function_with_effects([], TypeKind::Unit, effect.clone());
    write_type(hasher, &ty);
}

fn write_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn write_arguments(hasher: &mut blake3::Hasher, arguments: &[CheckedCallableArgument]) {
    hasher.update(
        &u32::try_from(arguments.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for argument in arguments {
        hasher.update(&u32::from(argument.argument().get()).to_le_bytes());
        hasher.update(
            &u32::try_from(argument.slots().len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for slot in argument.slots() {
            hasher.update(
                &u32::try_from(slot.slot().get())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
            match slot.mapped() {
                Some(coordinate) => {
                    hasher.update(&[1]);
                    write_group(hasher, coordinate.group());
                    hasher.update(
                        &u32::try_from(coordinate.parameter().get())
                            .unwrap_or(u32::MAX)
                            .to_le_bytes(),
                    );
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            write_optional_digest(hasher, slot.inferred());
            write_optional_digest(hasher, slot.expected());
        }
    }
}

fn write_optional_digest(hasher: &mut blake3::Hasher, digest: Option<[u8; 32]>) {
    match digest {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(&digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn write_receiver(hasher: &mut blake3::Hasher, receiver: &CallableReceiverMode) {
    match receiver {
        CallableReceiverMode::None => {
            hasher.update(&[0]);
        }
        CallableReceiverMode::Value { receiver } => {
            hasher.update(&[1]);
            write_type(hasher, receiver);
        }
        CallableReceiverMode::Type { receiver } => {
            hasher.update(&[2]);
            write_type(hasher, receiver);
        }
        CallableReceiverMode::Extension {
            receiver,
            group,
            parameter,
        } => {
            hasher.update(&[3]);
            write_type(hasher, receiver);
            write_group(hasher, *group);
            hasher.update(
                &u32::try_from(parameter.get())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
        }
    }
}

fn callable_family_tag(family: CallableFamily) -> u8 {
    match family {
        CallableFamily::Fx => 0,
        CallableFamily::EnumConstructor => 1,
        CallableFamily::ResultConstructor => 2,
        CallableFamily::OptionConstructor => 3,
        CallableFamily::Builtin => 4,
        CallableFamily::Agent => 5,
        CallableFamily::Presentation => 6,
        CallableFamily::Dialogue => 7,
        CallableFamily::Project => 8,
        CallableFamily::Environment => 9,
        CallableFamily::Lexical => 10,
        CallableFamily::FunctionValue => 11,
        CallableFamily::CollectionMethod => 12,
        CallableFamily::PresentationHandleMethod => 13,
        CallableFamily::IntegerMethod => 14,
        CallableFamily::DomainMethod => 15,
        CallableFamily::TraitMethod => 16,
        CallableFamily::CapacityMethod => 17,
        CallableFamily::StageMethod => 18,
        CallableFamily::LineContextMethod => 19,
        CallableFamily::LineSchedule => 20,
        CallableFamily::Drop => 21,
        CallableFamily::Promotion => 22,
    }
}

#[cfg(test)]
mod tests;
