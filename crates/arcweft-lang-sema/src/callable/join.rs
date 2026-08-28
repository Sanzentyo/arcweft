//! Exact selected-call joins owned by the callable authority.
//!
//! A final semantic consumer may have HIR lookup evidence (for example a
//! typed receiver/method key), but it must not rebuild callable identity or
//! resolve a second catalog.  [`validate_selected_application`] is the sole
//! seam for joining one clean selected call with its prepared application and
//! the current callable authority.

use arcweft_lang_hir::expr::HirCallArgumentOrdinal;
use thiserror::Error;

use crate::{effect_row::EffectRow, types::TypeKind};

use super::{
    CallableArgumentSlotIndex, CallableCandidateId, CallableFamily, CallableGroupIndex,
    CallableParameterCoordinate, CallableSignatureSchemaDigest, CheckedCallApplication,
    CheckedCallExecutionArgument, CheckedCallOperandDestination, CheckedCallResult,
    CheckedCallableCatalog, CheckedCallableDigest, CheckedCallableId, CheckedCallableLookupError,
    CheckedMethodLookup, ResolvedCallable, ResolvedCallableBaseInstantiation,
};

/// Failure while joining one final call fact with the current callable
/// authority.  Every variant is typed evidence failure; no spelling or
/// source-identity fallback is available.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedCallableJoinError {
    #[error("call target is not a clean selected callable")]
    NotSelected,
    #[error("selected call fact does not belong to the prepared application authority")]
    ApplicationAuthorityMismatch,
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
    #[error("call argument ordinal is not source contiguous")]
    ArgumentOrdinalMismatch,
    #[error("call argument slot index is not contiguous")]
    ArgumentSlotMismatch,
    #[error("selected call argument is not mapped to the current group")]
    ArgumentGroupMismatch,
    #[error("selected call argument mapping points outside the schema")]
    ArgumentParameterMissing,
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

impl CheckedCallableJoinError {
    pub(crate) fn visit_types<E>(
        &self,
        _visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::NotSelected
            | Self::ApplicationAuthorityMismatch
            | Self::SelectedGroupMismatch
            | Self::CurrentGroupMissing
            | Self::NextGroupMismatch
            | Self::MissingResult
            | Self::ResultMismatch
            | Self::ArgumentOrdinalMismatch
            | Self::ArgumentSlotMismatch
            | Self::ArgumentGroupMismatch
            | Self::ArgumentParameterMissing
            | Self::GenericInstantiationMismatch
            | Self::EffectsMismatch
            | Self::MissingCheckedCallable
            | Self::MissingCheckedRecord
            | Self::CatalogRecordMismatch
            | Self::CatalogSignatureMismatch
            | Self::CatalogEffectsMismatch
            | Self::Catalog(_)
            | Self::MissingReceiverKey
            | Self::UnexpectedReceiverKey
            | Self::ReceiverTypeMismatch
            | Self::ReceiverModeMismatch
            | Self::MethodLookupMissing
            | Self::MethodLookupAmbiguous
            | Self::MethodLookupMismatch
            | Self::MissingIntrinsicAuthority
            | Self::IntrinsicFamilyMismatch => Ok(()),
        }
    }
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
/// [`ResolvedCallableBaseInstantiation`] is the accepted authority retained in
/// the join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableInstantiationDigest([u8; 32]);

impl CallableInstantiationDigest {
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable semantic digest of one fully checked callable-owner join.
///
/// The bytes can only be produced by [`CheckedCallableJoin::semantic_digest`];
/// consumers may borrow them for a parent transcript but cannot mint a second
/// callable authority from raw bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableJoinDigest([u8; 32]);

impl CheckedCallableJoinDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
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

impl CallableReceiverMode {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::None => Ok(()),
            Self::Value { receiver }
            | Self::Type { receiver }
            | Self::Extension { receiver, .. } => visitor(receiver),
        }
    }
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
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Catalog {
                result, receiver, ..
            }
            | Self::Intrinsic {
                result, receiver, ..
            } => {
                visitor(result)?;
                receiver.visit_types(visitor)
            }
        }
    }

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
    pub fn semantic_digest(&self) -> CheckedCallableJoinDigest {
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
        CheckedCallableJoinDigest(*hasher.finalize().as_bytes())
    }
}

/// The one callable-owner validation seam for final semantic consumers.
///
/// Every execution, result, group, effect, receiver, and instantiation row is
/// projected from the already sealed application.  The join performs only the
/// remaining checked-catalog identity lookup; it never observes HIR receiver
/// spelling or reruns lower substitution.
pub(crate) fn validate_selected_application(
    application: &CheckedCallApplication,
    catalog: &CheckedCallableCatalog,
) -> Result<CheckedCallableJoin, CheckedCallableJoinError> {
    let core = application.core();
    let selected = core.candidates().selected();
    let current_group = core.current_group();
    let next_group = match application.result() {
        CheckedCallResult::Value(_) => None,
        CheckedCallResult::Continuation(continuation) => Some(continuation.next_group()),
    };
    let arguments = checked_join_arguments(selected, current_group, core.execution().arguments())?;
    let receiver = checked_receiver_mode(selected)?;
    let signature = selected.schema().semantic_digest();
    let instantiation = callable_instantiation_digest(selected.instantiation());
    let result = application.result().ty().clone();
    let effects = core.effects().clone();

    match selected.checked() {
        Some(id) => {
            let row = catalog
                .callable(id)
                .map_err(CheckedCallableJoinError::Catalog)?;
            if row.id() != id
                || row.signature().semantic_digest() != signature
                || row.record().id() != selected.id()
            {
                return Err(CheckedCallableJoinError::CatalogRecordMismatch);
            }
            if let Some(schema_key) = row.record().receiver_method_key() {
                let key = match selected.instantiation() {
                    ResolvedCallableBaseInstantiation::Extension { receiver, .. } => {
                        super::ReceiverMethodKey::new(
                            receiver.clone(),
                            row.record()
                                .extension_method_name()
                                .ok_or(CheckedCallableJoinError::MethodLookupMismatch)?
                                .clone(),
                        )
                    }
                    _ => schema_key,
                };
                match catalog.method(&key) {
                    CheckedMethodLookup::Candidates(candidates)
                        if candidates.iter().any(|candidate| candidate == id) => {}
                    CheckedMethodLookup::Candidates(_) => {
                        return Err(CheckedCallableJoinError::MethodLookupMismatch);
                    }
                    CheckedMethodLookup::Absent => {
                        return Err(CheckedCallableJoinError::MethodLookupMissing);
                    }
                    CheckedMethodLookup::Inaccessible(_) => {
                        return Err(CheckedCallableJoinError::MethodLookupAmbiguous);
                    }
                }
            }
            let catalog_effects = row.exposed_row().clone();
            Ok(CheckedCallableJoin::Catalog {
                id: Box::new(id.clone()),
                digest: id.semantic_digest(),
                signature,
                catalog_effects,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            })
        }
        None => {
            let candidate = IntrinsicCallableCandidateTag::from_candidate(selected.id())
                .ok_or(CheckedCallableJoinError::MissingIntrinsicAuthority)?;
            if selected.family() != selected.id().intrinsic_family() {
                return Err(CheckedCallableJoinError::IntrinsicFamilyMismatch);
            }
            Ok(CheckedCallableJoin::Intrinsic {
                candidate,
                family: selected.family(),
                signature,
                effects,
                result,
                current_group,
                next_group,
                arguments,
                receiver,
                instantiation,
            })
        }
    }
}

fn checked_join_arguments(
    selected: &ResolvedCallable,
    current_group: CallableGroupIndex,
    execution: &[CheckedCallExecutionArgument],
) -> Result<Box<[CheckedCallableArgument]>, CheckedCallableJoinError> {
    let mut arguments = Vec::with_capacity(execution.len());
    for (argument_index, argument) in execution.iter().enumerate() {
        let expected = HirCallArgumentOrdinal::try_from_usize(argument_index)
            .map_err(|_| CheckedCallableJoinError::ArgumentOrdinalMismatch)?;
        if argument.argument() != expected {
            return Err(CheckedCallableJoinError::ArgumentOrdinalMismatch);
        }
        let mut slots = Vec::with_capacity(argument.slots().len());
        for (slot_index, slot) in argument.slots().iter().enumerate() {
            let expected_slot = CallableArgumentSlotIndex::try_from_usize(slot_index)
                .map_err(|_| CheckedCallableJoinError::ArgumentSlotMismatch)?;
            if slot.slot() != expected_slot {
                return Err(CheckedCallableJoinError::ArgumentSlotMismatch);
            }
            let mapped = match slot.destination() {
                CheckedCallOperandDestination::Parameter(coordinate) => {
                    if coordinate.group() != current_group {
                        return Err(CheckedCallableJoinError::ArgumentGroupMismatch);
                    }
                    if selected
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameters().get(coordinate.parameter().get()))
                        .is_none()
                    {
                        return Err(CheckedCallableJoinError::ArgumentParameterMissing);
                    }
                    Some(*coordinate)
                }
                CheckedCallOperandDestination::Open(_) => None,
            };
            slots.push(CheckedCallableArgumentSlot {
                slot: slot.slot(),
                mapped,
                inferred: Some(*slot.inferred().semantic_identity_digest().as_bytes()),
                expected: slot
                    .expected()
                    .map(|ty| *ty.semantic_identity_digest().as_bytes()),
            });
        }
        arguments.push(CheckedCallableArgument {
            argument: argument.argument(),
            slots: slots.into_boxed_slice(),
        });
    }
    Ok(arguments.into_boxed_slice())
}

fn checked_receiver_mode(
    selected: &ResolvedCallable,
) -> Result<CallableReceiverMode, CheckedCallableJoinError> {
    Ok(match selected.instantiation() {
        ResolvedCallableBaseInstantiation::None
        | ResolvedCallableBaseInstantiation::ExpectedEnum { .. }
        | ResolvedCallableBaseInstantiation::Result { .. }
        | ResolvedCallableBaseInstantiation::Option
        | ResolvedCallableBaseInstantiation::Character { .. } => CallableReceiverMode::None,
        ResolvedCallableBaseInstantiation::Receiver { receiver } => CallableReceiverMode::Value {
            receiver: receiver.clone(),
        },
        ResolvedCallableBaseInstantiation::TypeReceiver { receiver } => {
            CallableReceiverMode::Type {
                receiver: receiver.receiver().clone(),
            }
        }
        ResolvedCallableBaseInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => CallableReceiverMode::Extension {
            receiver: receiver.clone(),
            group: *group,
            parameter: *parameter,
        },
    })
}

fn callable_instantiation_digest(
    instantiation: &ResolvedCallableBaseInstantiation,
) -> CallableInstantiationDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.lang.callable-instantiation.v1\0");
    match instantiation {
        ResolvedCallableBaseInstantiation::None => {
            hasher.update(&[0]);
        }
        ResolvedCallableBaseInstantiation::ExpectedEnum { expected } => {
            hasher.update(&[1]);
            write_type(&mut hasher, expected);
        }
        ResolvedCallableBaseInstantiation::Result { kind } => {
            hasher.update(&[
                2,
                u8::from(matches!(kind, super::ResultConstructorKind::Err)),
            ]);
        }
        ResolvedCallableBaseInstantiation::Option => {
            hasher.update(&[3]);
        }
        ResolvedCallableBaseInstantiation::Character { owner } => {
            hasher.update(&[4]);
            write_bytes(&mut hasher, owner.character().as_str().as_bytes());
        }
        ResolvedCallableBaseInstantiation::Receiver { receiver } => {
            hasher.update(&[5]);
            write_type(&mut hasher, receiver);
        }
        ResolvedCallableBaseInstantiation::TypeReceiver { receiver } => {
            hasher.update(&[6]);
            write_type(&mut hasher, receiver.receiver());
        }
        ResolvedCallableBaseInstantiation::Extension {
            receiver,
            group,
            parameter,
        } => {
            hasher.update(&[7]);
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
