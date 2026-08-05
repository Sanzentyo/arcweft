//! Retained-identity declarations and their secondary member arena.

use arcweft_id::{
    CharacterSurfaceAlias, DeclarationIdentityFamily, DeclarationName, PublicId,
    PublicIdFamilyError,
};
use thiserror::Error;

use crate::identity::{ExprId, HirModuleId, ItemId, LocalId, ScopeId, TypeId};
use crate::leaf::{HirIdRefValue, HirPathValue, HirStringLiteral};

use super::callable::{HirContractScopes, HirParameter};
use super::{
    HirItemFamily, HirItemInvariantError, HirRequiredName, validate_expr, validate_exprs,
    validate_locals, validate_parameters, validate_retained_family, validate_scope, validate_type,
};

/// Retained identity and local name shared by the seven authored families.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRetainedHeader {
    family: DeclarationIdentityFamily,
    public_id: HirRetainedPublicId,
    name: HirRetainedName,
}

impl HirRetainedHeader {
    pub(crate) fn try_new(
        family: DeclarationIdentityFamily,
        public_id: HirRetainedPublicId,
        name: HirRetainedName,
    ) -> Result<Self, HirRetainedHeaderError> {
        if family == DeclarationIdentityFamily::Asset {
            return Err(HirRetainedHeaderError::AssetIsCatalogOwned);
        }
        match &public_id {
            HirRetainedPublicId::Resolved { value, origin } => {
                family.validate_public_id(value)?;
                if *origin == HirPublicIdOrigin::DerivedFromName {
                    let HirRetainedName::Resolved(name) = &name else {
                        return Err(HirRetainedHeaderError::DerivedIdentityWithoutResolvedName);
                    };
                    if family.derive_public_id(name)? != *value {
                        return Err(HirRetainedHeaderError::DerivedIdentityMismatch);
                    }
                }
            }
            HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::WrongFamily(value)) => {
                if family.validate_public_id(value).is_ok() {
                    return Err(HirRetainedHeaderError::RecoveredIdentityMatchesFamily);
                }
            }
            HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::DerivedFromRecoveredName)
                if matches!(name, HirRetainedName::Resolved(_)) =>
            {
                return Err(HirRetainedHeaderError::RecoveredDerivationHasResolvedName);
            }
            HirRetainedPublicId::Recovered(_) => {}
        }
        Ok(Self {
            family,
            public_id,
            name,
        })
    }

    pub const fn family(&self) -> DeclarationIdentityFamily {
        self.family
    }

    pub const fn public_id(&self) -> &HirRetainedPublicId {
        &self.public_id
    }

    pub const fn name(&self) -> &HirRetainedName {
        &self.name
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        matches!(self.public_id, HirRetainedPublicId::Recovered(_))
            || !matches!(self.name, HirRetainedName::Resolved(_))
    }
}

/// Valid or typed-recovered retained declaration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirRetainedPublicId {
    Resolved {
        value: PublicId,
        origin: HirPublicIdOrigin,
    },
    Recovered(HirRetainedPublicIdIssue),
}

impl HirRetainedPublicId {
    pub const fn resolved(&self) -> Option<&PublicId> {
        match self {
            Self::Resolved { value, .. } => Some(value),
            Self::Recovered(_) => None,
        }
    }

    pub const fn origin(&self) -> Option<HirPublicIdOrigin> {
        match self {
            Self::Resolved { origin, .. } => Some(*origin),
            Self::Recovered(_) => None,
        }
    }
}

/// Retained identity recovery without a fabricated valid public ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirRetainedPublicIdIssue {
    WrongFamily(PublicId),
    Malformed,
    Missing,
    DerivedFromRecoveredName,
}

/// Valid or typed-recovered retained declaration name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirRetainedName {
    Resolved(DeclarationName),
    Missing,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPublicIdOrigin {
    Explicit,
    DerivedFromName,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirRetainedHeaderError {
    #[error("asset identities are catalog-owned and cannot form authored retained HIR items")]
    AssetIsCatalogOwned,
    #[error(transparent)]
    PublicIdFamily(#[from] PublicIdFamilyError),
    #[error("derived retained identity does not match its family and declaration name")]
    DerivedIdentityMismatch,
    #[error("a derived retained identity requires a resolved declaration name")]
    DerivedIdentityWithoutResolvedName,
    #[error("a wrong-family recovery identity unexpectedly belongs to the declaration family")]
    RecoveredIdentityMatchesFamily,
    #[error("derived-name recovery cannot accompany a resolved declaration name")]
    RecoveredDerivationHasResolvedName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCharacterDeclaration {
    header: HirRetainedHeader,
    surface_alias: HirCharacterSurfaceAlias,
    display_name: Option<HirDeclarationMemberId>,
}

impl HirCharacterDeclaration {
    pub(crate) const fn new(
        header: HirRetainedHeader,
        surface_alias: HirCharacterSurfaceAlias,
        display_name: Option<HirDeclarationMemberId>,
    ) -> Self {
        Self {
            header,
            surface_alias,
            display_name,
        }
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn surface_alias(&self) -> &HirCharacterSurfaceAlias {
        &self.surface_alias
    }

    pub const fn display_name(&self) -> Option<HirDeclarationMemberId> {
        self.display_name
    }

    pub(super) fn validate_module(
        &self,
        _expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::Character)
    }
}

/// Optional Character surface alias with missing syntax kept distinct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirCharacterSurfaceAlias {
    Absent,
    Resolved(CharacterSurfaceAlias),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirViewDeclaration {
    header: HirRetainedHeader,
    callable_scope: ScopeId,
    parameters: Box<[HirParameter]>,
    exports: Box<[HirDeclarationMemberId]>,
    values: Box<[ExprId]>,
}

impl HirViewDeclaration {
    pub(crate) fn try_new(
        owner: ItemId,
        header: HirRetainedHeader,
        callable_scope: ScopeId,
        parameters: Box<[HirParameter]>,
        exports: Box<[HirDeclarationMemberId]>,
        values: Box<[ExprId]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_retained_family(&header, DeclarationIdentityFamily::View)?;
        let expected = owner.module();
        validate_scope(expected, callable_scope)?;
        validate_parameters(expected, &parameters)?;
        if parameters
            .iter()
            .any(|parameter| parameter.kind() != super::callable::HirParameterKind::Fixed)
        {
            return Err(HirItemInvariantError::ViewParameterShape);
        }
        validate_declaration_member_references(owner, &exports)?;
        validate_exprs(expected, &values)?;
        Ok(Self {
            header,
            callable_scope,
            parameters,
            exports,
            values,
        })
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn parameters(&self) -> &[HirParameter] {
        &self.parameters
    }

    pub const fn callable_scope(&self) -> ScopeId {
        self.callable_scope
    }

    pub const fn exports(&self) -> &[HirDeclarationMemberId] {
        &self.exports
    }

    pub const fn values(&self) -> &[ExprId] {
        &self.values
    }

    pub(crate) fn validate_member_row(
        &self,
        owner: ItemId,
        expected: &[HirDeclarationMemberId],
    ) -> Result<(), HirItemInvariantError> {
        if self.exports.as_ref() != expected {
            return Err(HirItemInvariantError::ItemPayloadMemberRowMismatch { owner });
        }
        Ok(())
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        self.header.has_recovery()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::View)?;
        validate_scope(expected, self.callable_scope)?;
        validate_parameters(expected, &self.parameters)?;
        for member in &self.exports {
            if member.module() != expected {
                return Err(HirItemInvariantError::ForeignChild {
                    expected,
                    actual: member.module(),
                });
            }
        }
        validate_exprs(expected, &self.values)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirActionDeclaration {
    header: HirRetainedHeader,
    callable_scope: ScopeId,
    parameters: Box<[HirParameter]>,
}

impl HirActionDeclaration {
    pub(crate) fn try_new(
        header: HirRetainedHeader,
        callable_scope: ScopeId,
        parameters: Box<[HirParameter]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_retained_family(&header, DeclarationIdentityFamily::Action)?;
        let expected = callable_scope.module();
        validate_parameters(expected, &parameters)?;
        if parameters
            .iter()
            .any(|parameter| parameter.default().is_some())
        {
            return Err(HirItemInvariantError::ActionParameterDefault);
        }
        Ok(Self {
            header,
            callable_scope,
            parameters,
        })
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn callable_scope(&self) -> ScopeId {
        self.callable_scope
    }

    pub const fn parameters(&self) -> &[HirParameter] {
        &self.parameters
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::Action)?;
        validate_scope(expected, self.callable_scope)?;
        validate_parameters(expected, &self.parameters)?;
        if self
            .parameters
            .iter()
            .any(|parameter| parameter.default().is_some())
        {
            return Err(HirItemInvariantError::ActionParameterDefault);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirActivityDeclaration {
    header: HirRetainedHeader,
    scopes: HirContractScopes,
    mode: HirActivityMode,
    lifecycle: HirActivityLifecycle,
    inputs: Box<[HirDeclarationMemberId]>,
    outputs: Box<[HirDeclarationMemberId]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
}

impl HirActivityDeclaration {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        owner: ItemId,
        header: HirRetainedHeader,
        scopes: HirContractScopes,
        mode: HirActivityMode,
        lifecycle: HirActivityLifecycle,
        inputs: Box<[HirDeclarationMemberId]>,
        outputs: Box<[HirDeclarationMemberId]>,
        requires: Box<[ExprId]>,
        ensures: Box<[ExprId]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_retained_family(&header, DeclarationIdentityFamily::Activity)?;
        scopes.validate_module(owner.module())?;
        for member in inputs.iter().chain(outputs.iter()) {
            if member.item() != owner {
                return Err(HirItemInvariantError::ActivityPortOwner {
                    expected: owner,
                    actual: member.item(),
                });
            }
        }
        if inputs
            .iter()
            .enumerate()
            .any(|(index, input)| inputs[..index].contains(input))
            || outputs
                .iter()
                .enumerate()
                .any(|(index, output)| outputs[..index].contains(output))
            || inputs.iter().any(|input| outputs.contains(input))
        {
            return Err(HirItemInvariantError::DuplicateActivityPortMember);
        }
        validate_exprs(owner.module(), &requires)?;
        validate_exprs(owner.module(), &ensures)?;
        Ok(Self {
            header,
            scopes,
            mode,
            lifecycle,
            inputs,
            outputs,
            requires,
            ensures,
        })
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn scopes(&self) -> HirContractScopes {
        self.scopes
    }

    pub const fn mode(&self) -> HirActivityMode {
        self.mode
    }

    pub const fn lifecycle(&self) -> HirActivityLifecycle {
        self.lifecycle
    }

    pub const fn inputs(&self) -> &[HirDeclarationMemberId] {
        &self.inputs
    }

    pub const fn outputs(&self) -> &[HirDeclarationMemberId] {
        &self.outputs
    }

    pub const fn requires(&self) -> &[ExprId] {
        &self.requires
    }

    pub const fn ensures(&self) -> &[ExprId] {
        &self.ensures
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::Activity)?;
        self.scopes.validate_module(expected)?;
        for member in self.inputs.iter().chain(self.outputs.iter()) {
            if member.module() != expected {
                return Err(HirItemInvariantError::ForeignChild {
                    expected,
                    actual: member.module(),
                });
            }
        }
        validate_exprs(expected, &self.requires)?;
        validate_exprs(expected, &self.ensures)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirActivityMode {
    Deterministic,
    CheckpointedRealtime,
    ExternalRealtime,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirActivityLifecycle {
    Stateless,
    Snapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSignalDeclaration {
    header: HirRetainedHeader,
    observable_type: TypeId,
}

impl HirSignalDeclaration {
    pub(crate) fn try_new(
        header: HirRetainedHeader,
        observable_type: TypeId,
    ) -> Result<Self, HirItemInvariantError> {
        validate_retained_family(&header, DeclarationIdentityFamily::Signal)?;
        Ok(Self {
            header,
            observable_type,
        })
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn observable_type(&self) -> TypeId {
        self.observable_type
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::Signal)?;
        validate_type(expected, self.observable_type)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMetricDeclaration {
    header: HirRetainedHeader,
    kind: HirMetricKind,
    value_type: TypeId,
    unit: Option<HirDeclarationMemberId>,
    labels: Box<[HirDeclarationMemberId]>,
    buckets: Option<HirDeclarationMemberId>,
}

impl HirMetricDeclaration {
    pub(crate) fn try_new(
        owner: ItemId,
        header: HirRetainedHeader,
        kind: HirMetricKind,
        value_type: TypeId,
        unit: Option<HirDeclarationMemberId>,
        labels: Box<[HirDeclarationMemberId]>,
        buckets: Option<HirDeclarationMemberId>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_retained_family(&header, DeclarationIdentityFamily::Metric)?;
        validate_type(owner.module(), value_type)?;
        let references = unit
            .iter()
            .chain(labels.iter())
            .chain(buckets.iter())
            .copied()
            .collect::<Vec<_>>();
        for (position, member) in references.iter().copied().enumerate() {
            if member.item() != owner {
                return Err(HirItemInvariantError::DeclarationMemberOwner {
                    expected: owner,
                    actual: member.item(),
                });
            }
            if references[..position].contains(&member) {
                return Err(HirItemInvariantError::DuplicateDeclarationMemberReference { member });
            }
        }
        Ok(Self {
            header,
            kind,
            value_type,
            unit,
            labels,
            buckets,
        })
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn kind(&self) -> HirMetricKind {
        self.kind
    }

    pub const fn value_type(&self) -> TypeId {
        self.value_type
    }

    pub const fn unit(&self) -> Option<HirDeclarationMemberId> {
        self.unit
    }

    pub const fn labels(&self) -> &[HirDeclarationMemberId] {
        &self.labels
    }

    pub const fn buckets(&self) -> Option<HirDeclarationMemberId> {
        self.buckets
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::Metric)?;
        validate_type(expected, self.value_type)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMetricKind {
    Counter,
    Gauge,
    Histogram,
    Recovered(HirMetricKindIssue),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMetricKindIssue {
    Missing,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLayerDeclaration {
    header: HirRetainedHeader,
    kind: HirLayerKind,
    members: Box<[HirDeclarationMemberId]>,
}

impl HirLayerDeclaration {
    pub(crate) fn try_new(
        owner: ItemId,
        header: HirRetainedHeader,
        kind: HirLayerKind,
        members: Box<[HirDeclarationMemberId]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_retained_family(&header, DeclarationIdentityFamily::Layer)?;
        validate_declaration_member_references(owner, &members)?;
        Ok(Self {
            header,
            kind,
            members,
        })
    }

    pub const fn header(&self) -> &HirRetainedHeader {
        &self.header
    }

    pub const fn kind(&self) -> HirLayerKind {
        self.kind
    }

    pub const fn members(&self) -> &[HirDeclarationMemberId] {
        &self.members
    }

    pub(crate) fn validate_member_row(
        &self,
        owner: ItemId,
        expected: &[HirDeclarationMemberId],
    ) -> Result<(), HirItemInvariantError> {
        if self.members.as_ref() != expected {
            return Err(HirItemInvariantError::ItemPayloadMemberRowMismatch { owner });
        }
        Ok(())
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        self.header.has_recovery() || matches!(self.kind, HirLayerKind::Recovered(_))
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_retained_family(&self.header, DeclarationIdentityFamily::Layer)?;
        for member in &self.members {
            if member.module() != expected {
                return Err(HirItemInvariantError::ForeignChild {
                    expected,
                    actual: member.module(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerKind {
    Background,
    World2d,
    Character,
    Effects,
    Dialogue,
    GameView,
    HtmlView,
    Activity,
    Modal,
    Overlay,
    Debug,
    Agent,
    Offscreen,
    Custom,
    Recovered(HirLayerKindIssue),
}

impl HirLayerKind {
    pub const fn default_phase(self) -> Option<HirRenderPhase> {
        match self {
            Self::Background | Self::Offscreen => Some(HirRenderPhase::Background),
            Self::World2d | Self::Custom => Some(HirRenderPhase::World),
            Self::Character => Some(HirRenderPhase::Characters),
            Self::Effects => Some(HirRenderPhase::Effects),
            Self::Dialogue => Some(HirRenderPhase::Dialogue),
            Self::GameView | Self::Activity => Some(HirRenderPhase::GameView),
            Self::HtmlView => Some(HirRenderPhase::HtmlView),
            Self::Modal | Self::Overlay => Some(HirRenderPhase::Modal),
            Self::Debug => Some(HirRenderPhase::Debug),
            Self::Agent => Some(HirRenderPhase::AgentOverlay),
            Self::Recovered(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerKindIssue {
    Missing,
    Invalid,
}

/// Composite same-item declaration-member identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDeclarationMemberId {
    item: ItemId,
    ordinal: u32,
}

impl HirDeclarationMemberId {
    pub(crate) const fn new(item: ItemId, ordinal: u32) -> Self {
        Self { item, ordinal }
    }

    pub const fn item(self) -> ItemId {
        self.item
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub const fn module(self) -> HirModuleId {
        self.item.module()
    }
}

/// One record in the secondary declaration-member arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationMember {
    id: HirDeclarationMemberId,
    kind: HirDeclarationMemberKind,
    state: HirDeclarationMemberPoisonState,
}

impl HirDeclarationMember {
    pub(crate) fn try_new(
        id: HirDeclarationMemberId,
        kind: HirDeclarationMemberKind,
        state: HirDeclarationMemberPoisonState,
    ) -> Result<Self, HirItemInvariantError> {
        kind.validate_module(id.module())?;
        if !member_state_matches_kind(&kind, state) {
            return Err(HirItemInvariantError::InvalidPoisonState);
        }
        Ok(Self { id, kind, state })
    }

    pub const fn id(&self) -> HirDeclarationMemberId {
        self.id
    }

    pub const fn kind(&self) -> &HirDeclarationMemberKind {
        &self.kind
    }

    pub const fn state(&self) -> HirDeclarationMemberPoisonState {
        self.state
    }

    pub const fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }
}

/// Executability state of one declaration-member record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationMemberPoisonState {
    Clean,
    Poisoned(HirDeclarationMemberIssue),
}

impl HirDeclarationMemberPoisonState {
    pub const fn is_poisoned(self) -> bool {
        matches!(self, Self::Poisoned(_))
    }
}

/// Canonical primary issue for a recognized declaration member.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationMemberIssue {
    Duplicate,
    MissingAssignment,
    MissingInitializer,
    RecoveredChild,
    UnclassifiedSyntax,
}

/// Per-item secondary arena preserving source order exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationMemberArena {
    owner: ItemId,
    family: HirItemFamily,
    members: Box<[HirDeclarationMember]>,
}

impl HirDeclarationMemberArena {
    pub(crate) fn try_new(
        owner: ItemId,
        family: HirItemFamily,
        members: Box<[HirDeclarationMember]>,
    ) -> Result<Self, HirItemInvariantError> {
        for (position, member) in members.iter().enumerate() {
            let ordinal = u32::try_from(position)
                .map_err(|_| HirItemInvariantError::MemberOrdinalOverflow { position })?;
            let expected = HirDeclarationMemberId::new(owner, ordinal);
            if member.id != expected {
                return Err(HirItemInvariantError::NonContiguousMember {
                    expected,
                    actual: member.id,
                });
            }
            if !member.kind.accepts_family(family) {
                return Err(HirItemInvariantError::WrongMemberFamily {
                    member: member.id,
                    family,
                });
            }
        }
        Ok(Self {
            owner,
            family,
            members,
        })
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn family(&self) -> HirItemFamily {
        self.family
    }

    pub const fn members(&self) -> &[HirDeclarationMember] {
        &self.members
    }

    pub fn resolve(
        &self,
        id: HirDeclarationMemberId,
    ) -> Result<&HirDeclarationMember, HirDeclarationMemberResolveError> {
        if id.item != self.owner {
            return Err(HirDeclarationMemberResolveError::ForeignOwner {
                expected: self.owner,
                actual: id.item,
            });
        }
        let index = usize::try_from(id.ordinal)
            .map_err(|_| HirDeclarationMemberResolveError::UnknownOrdinal(id.ordinal))?;
        self.members
            .get(index)
            .ok_or(HirDeclarationMemberResolveError::UnknownOrdinal(id.ordinal))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationMemberResolveError {
    #[error("declaration member belongs to a foreign item")]
    ForeignOwner { expected: ItemId, actual: ItemId },
    #[error("declaration member ordinal {0} is not allocated")]
    UnknownOrdinal(u32),
}

/// Closed declaration-member payload inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDeclarationMemberKind {
    ViewExport(HirViewExportMember),
    ActivityInput(HirActivityPortMember),
    ActivityOutput(HirActivityPortMember),
    MetricUnit(HirMetricUnitMember),
    MetricLabel(HirMetricLabelMember),
    MetricBuckets(HirMetricBucketsMember),
    CharacterDisplayName(HirCharacterDisplayNameMember),
    CharacterRecovery(HirCharacterMemberRecovery),
    LayerReference(HirLayerReferenceMember),
    LayerPolicy(HirLayerPolicyMember),
    LayerExpression(HirLayerExpressionMember),
}

impl HirDeclarationMemberKind {
    const fn accepts_family(&self, family: HirItemFamily) -> bool {
        matches!(
            (self, family),
            (Self::ViewExport(_), HirItemFamily::View)
                | (
                    Self::ActivityInput(_) | Self::ActivityOutput(_),
                    HirItemFamily::Activity
                )
                | (
                    Self::MetricUnit(_) | Self::MetricLabel(_) | Self::MetricBuckets(_),
                    HirItemFamily::Metric
                )
                | (
                    Self::CharacterDisplayName(_) | Self::CharacterRecovery(_),
                    HirItemFamily::Character
                )
                | (
                    Self::LayerReference(_) | Self::LayerPolicy(_) | Self::LayerExpression(_),
                    HirItemFamily::Layer
                )
        )
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        match self {
            Self::ViewExport(_) | Self::LayerReference(_) | Self::LayerPolicy(_) => Ok(()),
            Self::ActivityInput(port) | Self::ActivityOutput(port) => {
                port.validate_module(expected)
            }
            Self::MetricUnit(member) => member.validate_module(expected),
            Self::MetricLabel(label) => validate_type(expected, label.ty),
            Self::MetricBuckets(member) => member.validate_module(expected),
            Self::CharacterDisplayName(member) => {
                if let Some(value) = member.initializer() {
                    validate_expr(expected, value)?;
                }
                Ok(())
            }
            Self::CharacterRecovery(_) => Ok(()),
            Self::LayerExpression(expression) => match expression.payload().value() {
                HirLayerMemberValue::Present(value)
                | HirLayerMemberValue::Recovered(Some(value)) => validate_expr(expected, *value),
                HirLayerMemberValue::Recovered(None) | HirLayerMemberValue::Missing => Ok(()),
            },
        }
    }
}

fn member_state_matches_kind(
    kind: &HirDeclarationMemberKind,
    state: HirDeclarationMemberPoisonState,
) -> bool {
    match kind {
        HirDeclarationMemberKind::ViewExport(member) => {
            if member.has_recovery() {
                state
                    == HirDeclarationMemberPoisonState::Poisoned(
                        HirDeclarationMemberIssue::RecoveredChild,
                    )
            } else {
                matches!(
                    state,
                    HirDeclarationMemberPoisonState::Clean
                        | HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::RecoveredChild
                        )
                )
            }
        }
        HirDeclarationMemberKind::CharacterDisplayName(member) => {
            let structural_issue = if member.duplicate {
                Some(HirDeclarationMemberIssue::Duplicate)
            } else if member.assignment == HirCharacterAssignmentState::Missing {
                Some(HirDeclarationMemberIssue::MissingAssignment)
            } else if member.initializer.is_none() {
                Some(HirDeclarationMemberIssue::MissingInitializer)
            } else {
                None
            };
            match (structural_issue, state) {
                (None, HirDeclarationMemberPoisonState::Clean)
                | (
                    None,
                    HirDeclarationMemberPoisonState::Poisoned(
                        HirDeclarationMemberIssue::RecoveredChild,
                    ),
                ) => true,
                (Some(expected), HirDeclarationMemberPoisonState::Poisoned(actual)) => {
                    expected == actual
                }
                _ => false,
            }
        }
        HirDeclarationMemberKind::CharacterRecovery(_) => matches!(
            state,
            HirDeclarationMemberPoisonState::Poisoned(
                HirDeclarationMemberIssue::UnclassifiedSyntax
            )
        ),
        HirDeclarationMemberKind::ActivityInput(port)
        | HirDeclarationMemberKind::ActivityOutput(port) => {
            if port.name.is_recovered() {
                matches!(
                    state,
                    HirDeclarationMemberPoisonState::Poisoned(
                        HirDeclarationMemberIssue::RecoveredChild
                    )
                )
            } else {
                matches!(
                    state,
                    HirDeclarationMemberPoisonState::Clean
                        | HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::Duplicate
                                | HirDeclarationMemberIssue::RecoveredChild
                        )
                )
            }
        }
        HirDeclarationMemberKind::MetricUnit(member) => {
            let structural_issue = if member.duplicate {
                Some(HirDeclarationMemberIssue::Duplicate)
            } else if member.assignment == HirMetricAssignmentState::Missing {
                Some(HirDeclarationMemberIssue::MissingAssignment)
            } else {
                match &member.value {
                    HirMetricUnitValue::Missing => {
                        Some(HirDeclarationMemberIssue::MissingInitializer)
                    }
                    HirMetricUnitValue::NonString(_)
                    | HirMetricUnitValue::String(HirStringLiteral::Invalid(_)) => {
                        Some(HirDeclarationMemberIssue::RecoveredChild)
                    }
                    HirMetricUnitValue::String(HirStringLiteral::Value(_)) => None,
                }
            };
            state_matches_structural_issue(structural_issue, state)
        }
        HirDeclarationMemberKind::MetricLabel(member) => {
            let structural_issue = if member.duplicate {
                Some(HirDeclarationMemberIssue::Duplicate)
            } else if member.name.resolved().is_none() {
                Some(HirDeclarationMemberIssue::RecoveredChild)
            } else {
                None
            };
            match structural_issue {
                Some(issue) => state == HirDeclarationMemberPoisonState::Poisoned(issue),
                None => matches!(
                    state,
                    HirDeclarationMemberPoisonState::Clean
                        | HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::RecoveredChild
                        )
                ),
            }
        }
        HirDeclarationMemberKind::MetricBuckets(member) => {
            let structural_issue = if member.duplicate {
                Some(HirDeclarationMemberIssue::Duplicate)
            } else if member.assignment == HirMetricAssignmentState::Missing {
                Some(HirDeclarationMemberIssue::MissingAssignment)
            } else {
                match &member.value {
                    HirMetricBucketsValue::Missing => {
                        Some(HirDeclarationMemberIssue::MissingInitializer)
                    }
                    HirMetricBucketsValue::NonSequence(_) => {
                        Some(HirDeclarationMemberIssue::RecoveredChild)
                    }
                    HirMetricBucketsValue::Sequence(values) if values.is_empty() => {
                        Some(HirDeclarationMemberIssue::RecoveredChild)
                    }
                    HirMetricBucketsValue::Sequence(_) => None,
                }
            };
            match structural_issue {
                Some(issue) => state == HirDeclarationMemberPoisonState::Poisoned(issue),
                None => matches!(
                    state,
                    HirDeclarationMemberPoisonState::Clean
                        | HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::RecoveredChild
                        )
                ),
            }
        }
        HirDeclarationMemberKind::LayerReference(member) => member.poison_state() == state,
        HirDeclarationMemberKind::LayerPolicy(member) => match member {
            HirLayerPolicyMember::Phase(payload) => payload.poison_state() == state,
            HirLayerPolicyMember::Input(payload) => payload.poison_state() == state,
            HirLayerPolicyMember::HitTest(payload) => payload.poison_state() == state,
            HirLayerPolicyMember::Capture(payload) => payload.poison_state() == state,
            HirLayerPolicyMember::Accessibility(payload) => payload.poison_state() == state,
        },
        HirDeclarationMemberKind::LayerExpression(member) => {
            layer_member_state_matches(&member.payload(), state)
        }
    }
}

fn layer_member_state_matches<T>(
    payload: &HirLayerMemberPayload<T>,
    state: HirDeclarationMemberPoisonState,
) -> bool {
    payload.poison_state() == state
}

const fn layer_member_issue<T>(
    payload: &HirLayerMemberPayload<T>,
) -> Option<HirDeclarationMemberIssue> {
    if payload.duplicate {
        Some(HirDeclarationMemberIssue::Duplicate)
    } else if matches!(payload.assignment, HirLayerAssignmentState::Missing) {
        Some(HirDeclarationMemberIssue::MissingAssignment)
    } else {
        match &payload.value {
            HirLayerMemberValue::Missing => Some(HirDeclarationMemberIssue::MissingInitializer),
            HirLayerMemberValue::Recovered(_) => Some(HirDeclarationMemberIssue::RecoveredChild),
            HirLayerMemberValue::Present(_) => None,
        }
    }
}

fn state_matches_structural_issue(
    structural_issue: Option<HirDeclarationMemberIssue>,
    state: HirDeclarationMemberPoisonState,
) -> bool {
    match (structural_issue, state) {
        (None, HirDeclarationMemberPoisonState::Clean) => true,
        (Some(expected), HirDeclarationMemberPoisonState::Poisoned(actual)) => expected == actual,
        _ => false,
    }
}

/// Semantic payload of a Character `display_name` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCharacterDisplayNameMember {
    assignment: HirCharacterAssignmentState,
    initializer: Option<ExprId>,
    duplicate: bool,
}

impl HirCharacterDisplayNameMember {
    pub(crate) const fn new(
        assignment: HirCharacterAssignmentState,
        initializer: Option<ExprId>,
        duplicate: bool,
    ) -> Self {
        Self {
            assignment,
            initializer,
            duplicate,
        }
    }

    pub const fn assignment(&self) -> HirCharacterAssignmentState {
        self.assignment
    }

    pub const fn initializer(&self) -> Option<ExprId> {
        self.initializer
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCharacterAssignmentState {
    Present,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCharacterMemberRecovery {
    Unknown,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirViewExportMember {
    local_part: HirPathValue,
    public_part: HirPathValue,
}

impl HirViewExportMember {
    pub(crate) const fn new(local_part: HirPathValue, public_part: HirPathValue) -> Self {
        Self {
            local_part,
            public_part,
        }
    }

    pub const fn local_part(&self) -> &HirPathValue {
        &self.local_part
    }

    pub const fn public_part(&self) -> &HirPathValue {
        &self.public_part
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self.local_part, HirPathValue::Recovered(_))
            || matches!(self.public_part, HirPathValue::Recovered(_))
    }
}

fn validate_declaration_member_references(
    owner: ItemId,
    members: &[HirDeclarationMemberId],
) -> Result<(), HirItemInvariantError> {
    for (position, member) in members.iter().copied().enumerate() {
        if member.item() != owner {
            return Err(HirItemInvariantError::DeclarationMemberOwner {
                expected: owner,
                actual: member.item(),
            });
        }
        if members[..position].contains(&member) {
            return Err(HirItemInvariantError::DuplicateDeclarationMemberReference { member });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirActivityPortMember {
    name: HirRequiredName,
    ty: TypeId,
    local: Option<LocalId>,
}

impl HirActivityPortMember {
    pub(crate) fn try_new(
        name: HirRequiredName,
        ty: TypeId,
        local: Option<LocalId>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_locals(ty.module(), local.as_slice())?;
        if name.resolved().is_some() != local.is_some() {
            return Err(HirItemInvariantError::ActivityPortLocalMismatch);
        }
        Ok(Self { name, ty, local })
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    pub const fn local(&self) -> Option<LocalId> {
        self.local
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_type(expected, self.ty)?;
        validate_locals(expected, self.local.as_slice())?;
        if self.name.resolved().is_some() != self.local.is_some() {
            return Err(HirItemInvariantError::ActivityPortLocalMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMetricLabelMember {
    name: HirRequiredName,
    ty: TypeId,
    duplicate: bool,
}

impl HirMetricLabelMember {
    pub(crate) const fn new(name: HirRequiredName, ty: TypeId, duplicate: bool) -> Self {
        Self {
            name,
            ty,
            duplicate,
        }
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }
}

/// Source-preserving Metric `unit` member payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMetricUnitMember {
    assignment: HirMetricAssignmentState,
    value: HirMetricUnitValue,
    duplicate: bool,
}

impl HirMetricUnitMember {
    pub(crate) const fn new(
        assignment: HirMetricAssignmentState,
        value: HirMetricUnitValue,
        duplicate: bool,
    ) -> Self {
        Self {
            assignment,
            value,
            duplicate,
        }
    }

    pub const fn assignment(&self) -> HirMetricAssignmentState {
        self.assignment
    }

    pub const fn value(&self) -> &HirMetricUnitValue {
        &self.value
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        if let HirMetricUnitValue::NonString(expression) = &self.value {
            validate_expr(expected, *expression)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirMetricUnitValue {
    String(HirStringLiteral),
    NonString(ExprId),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMetricBucketsMember {
    assignment: HirMetricAssignmentState,
    value: HirMetricBucketsValue,
    duplicate: bool,
}

impl HirMetricBucketsMember {
    pub(crate) const fn new(
        assignment: HirMetricAssignmentState,
        value: HirMetricBucketsValue,
        duplicate: bool,
    ) -> Self {
        Self {
            assignment,
            value,
            duplicate,
        }
    }

    pub const fn assignment(&self) -> HirMetricAssignmentState {
        self.assignment
    }

    pub const fn value(&self) -> &HirMetricBucketsValue {
        &self.value
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match &self.value {
            HirMetricBucketsValue::Sequence(values) => validate_exprs(expected, values),
            HirMetricBucketsValue::NonSequence(expression) => validate_expr(expected, *expression),
            HirMetricBucketsValue::Missing => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirMetricBucketsValue {
    Sequence(Box<[ExprId]>),
    NonSequence(ExprId),
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMetricAssignmentState {
    Present,
    Missing,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerReferenceMember {
    Parent(HirLayerMemberPayload<HirIdRefValue>),
    View(HirLayerMemberPayload<HirIdRefValue>),
    Activity(HirLayerMemberPayload<HirIdRefValue>),
}

impl HirLayerReferenceMember {
    pub(crate) const fn payload(&self) -> &HirLayerMemberPayload<HirIdRefValue> {
        match self {
            Self::Parent(payload) | Self::View(payload) | Self::Activity(payload) => payload,
        }
    }

    pub(crate) const fn poison_state(&self) -> HirDeclarationMemberPoisonState {
        let payload = self.payload();
        let issue = if payload.duplicate {
            Some(HirDeclarationMemberIssue::Duplicate)
        } else if matches!(payload.assignment, HirLayerAssignmentState::Missing) {
            Some(HirDeclarationMemberIssue::MissingAssignment)
        } else {
            match &payload.value {
                HirLayerMemberValue::Missing => Some(HirDeclarationMemberIssue::MissingInitializer),
                HirLayerMemberValue::Recovered(_) => {
                    Some(HirDeclarationMemberIssue::RecoveredChild)
                }
                HirLayerMemberValue::Present(value) if value.is_recovered() => {
                    Some(HirDeclarationMemberIssue::RecoveredChild)
                }
                HirLayerMemberValue::Present(_) => None,
            }
        };
        match issue {
            Some(issue) => HirDeclarationMemberPoisonState::Poisoned(issue),
            None => HirDeclarationMemberPoisonState::Clean,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerPolicyMember {
    Phase(HirLayerMemberPayload<HirRenderPhase>),
    Input(HirLayerMemberPayload<HirInputPolicy>),
    HitTest(HirLayerMemberPayload<HirHitTestPolicy>),
    Capture(HirLayerMemberPayload<HirCapturePolicy>),
    Accessibility(HirLayerMemberPayload<HirAccessibilityPolicy>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerExpressionMember {
    Z(HirLayerMemberPayload<ExprId>),
    Visible(HirLayerMemberPayload<ExprId>),
    Transform(HirLayerMemberPayload<ExprId>),
}

impl HirLayerExpressionMember {
    pub const fn payload(self) -> HirLayerMemberPayload<ExprId> {
        match self {
            Self::Z(payload) | Self::Visible(payload) | Self::Transform(payload) => payload,
        }
    }
}

/// Assignment and recovery retained for one typed Layer member value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirLayerMemberPayload<T> {
    assignment: HirLayerAssignmentState,
    value: HirLayerMemberValue<T>,
    duplicate: bool,
}

impl<T> HirLayerMemberPayload<T> {
    pub(crate) const fn new(
        assignment: HirLayerAssignmentState,
        value: HirLayerMemberValue<T>,
        duplicate: bool,
    ) -> Self {
        Self {
            assignment,
            value,
            duplicate,
        }
    }

    pub const fn assignment(&self) -> HirLayerAssignmentState {
        self.assignment
    }

    pub const fn value(&self) -> &HirLayerMemberValue<T> {
        &self.value
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    pub(crate) const fn poison_state(&self) -> HirDeclarationMemberPoisonState {
        match layer_member_issue(self) {
            Some(issue) => HirDeclarationMemberPoisonState::Poisoned(issue),
            None => HirDeclarationMemberPoisonState::Clean,
        }
    }
}

/// Exact semantic value state of a recognized Layer member.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerMemberValue<T> {
    Present(T),
    Recovered(Option<T>),
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerAssignmentState {
    Present,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRenderPhase {
    Background,
    World,
    Characters,
    Effects,
    Dialogue,
    GameView,
    HtmlView,
    Modal,
    Debug,
    AgentOverlay,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirInputPolicy {
    Ignore,
    PassThrough,
    HitTest,
    Modal,
    Capture,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirHitTestPolicy {
    None,
    Bounds,
    ViewTree,
    ObjectIdMask,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCapturePolicy {
    None,
    Color,
    ObjectId,
    Mask,
    All,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAccessibilityPolicy {
    Hidden,
    Exposed,
    Container,
}
