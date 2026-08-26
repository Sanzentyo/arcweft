//! Final semantic Entry declaration and closed member inventory.

use core::cmp::Ordering;

use crate::identity::{ExprId, HirModuleId, TypeId};
use crate::leaf::{HirIdRef, HirIdRefValue, HirName, HirPathValue, HirStringIssue};

use super::{HirItemInvariantError, HirRequiredName, validate_expr, validate_type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirEntryDeclaration {
    kind: HirEntryKind,
    id: HirEntryId,
    header_trailing_recovery: bool,
    body: HirEntryBody,
}

impl HirEntryDeclaration {
    pub(crate) fn try_new(
        expected: HirModuleId,
        kind: HirEntryKind,
        id: HirEntryId,
        header_trailing_recovery: bool,
        body: HirEntryBody,
    ) -> Result<Self, HirItemInvariantError> {
        let declaration = Self {
            kind,
            id,
            header_trailing_recovery,
            body,
        };
        declaration.validate_module(expected)?;
        Ok(declaration)
    }

    pub const fn kind(&self) -> &HirEntryKind {
        &self.kind
    }

    pub const fn id(&self) -> &HirEntryId {
        &self.id
    }

    pub const fn has_header_trailing_recovery(&self) -> bool {
        self.header_trailing_recovery
    }

    pub const fn body(&self) -> &HirEntryBody {
        &self.body
    }

    pub const fn members(&self) -> &[HirEntryMember] {
        self.body.members()
    }

    pub fn has_structural_recovery(&self) -> bool {
        self.kind.has_recovery()
            || self.id.has_recovery()
            || self.header_trailing_recovery
            || self.body.has_structural_recovery()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        self.id.validate()?;
        for member in self.body.members() {
            match member {
                HirEntryMember::StateType(binding) | HirEntryMember::EventType(binding) => {
                    validate_type(expected, binding.ty())?;
                }
                HirEntryMember::Option(option) => {
                    if let Some(expression) = option.value().expression() {
                        validate_expr(expected, expression)?;
                    }
                }
                HirEntryMember::Route(route) => route.validate()?,
                HirEntryMember::Initializer(_)
                | HirEntryMember::Reducer(_)
                | HirEntryMember::Controller(_)
                | HirEntryMember::Goto(_)
                | HirEntryMember::Error => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryKind {
    Game,
    Editor,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Agent,
    Custom(HirName),
    Recovered(HirEntryKindIssue),
}

impl HirEntryKind {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryKindIssue {
    Missing,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryId {
    Authored {
        value: HirIdRefValue,
        canonical_entry_family: bool,
    },
    Missing,
}

impl HirEntryId {
    pub const fn value(&self) -> Option<&HirIdRefValue> {
        match self {
            Self::Authored { value, .. } => Some(value),
            Self::Missing => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Authored {
                value,
                canonical_entry_family,
            } => !canonical_entry_family || value.recovery_issue().is_some(),
            Self::Missing => true,
        }
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        let Self::Authored {
            value,
            canonical_entry_family,
        } = self
        else {
            return Ok(());
        };
        if *canonical_entry_family
            && value
                .as_resolved()
                .and_then(HirIdRef::absolute_family)
                .is_none_or(|family| family != "entry")
        {
            return Err(HirItemInvariantError::InvalidEntryRecovery);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryPunctuationState {
    Present,
    Missing,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEntryTypeBinding {
    assignment: HirEntryPunctuationState,
    ty: TypeId,
    trailing_recovery: bool,
}

impl HirEntryTypeBinding {
    pub(crate) const fn new(
        assignment: HirEntryPunctuationState,
        ty: TypeId,
        trailing_recovery: bool,
    ) -> Self {
        Self {
            assignment,
            ty,
            trailing_recovery,
        }
    }

    pub const fn assignment(&self) -> HirEntryPunctuationState {
        self.assignment
    }

    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery
    }

    pub const fn has_structural_recovery(&self) -> bool {
        matches!(self.assignment, HirEntryPunctuationState::Missing) || self.trailing_recovery
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryPathValue {
    Authored(HirPathValue),
    Invalid,
    Missing,
}

impl HirEntryPathValue {
    pub const fn has_recovery(&self) -> bool {
        match self {
            Self::Authored(value) => value.recovery().is_some(),
            Self::Invalid | Self::Missing => true,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEntryPathBinding {
    assignment: HirEntryPunctuationState,
    value: HirEntryPathValue,
    trailing_recovery: bool,
}

impl HirEntryPathBinding {
    pub(crate) const fn new(
        assignment: HirEntryPunctuationState,
        value: HirEntryPathValue,
        trailing_recovery: bool,
    ) -> Self {
        Self {
            assignment,
            value,
            trailing_recovery,
        }
    }

    pub const fn assignment(&self) -> HirEntryPunctuationState {
        self.assignment
    }

    pub const fn value(&self) -> &HirEntryPathValue {
        &self.value
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self.assignment, HirEntryPunctuationState::Missing)
            || self.value.has_recovery()
            || self.trailing_recovery
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryTarget {
    Authored(HirIdRefValue),
    Invalid,
    Missing,
}

impl HirEntryTarget {
    pub const fn has_recovery(&self) -> bool {
        match self {
            Self::Authored(value) => value.recovery_issue().is_some(),
            Self::Invalid | Self::Missing => true,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEntryGoto {
    target: HirEntryTarget,
    trailing_recovery: bool,
}

impl HirEntryGoto {
    pub(crate) const fn new(target: HirEntryTarget, trailing_recovery: bool) -> Self {
        Self {
            target,
            trailing_recovery,
        }
    }

    pub const fn target(&self) -> &HirEntryTarget {
        &self.target
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery
    }

    pub const fn has_recovery(&self) -> bool {
        self.target.has_recovery() || self.trailing_recovery
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirEntryBody {
    Missing,
    Braced {
        members: Box<[HirEntryMember]>,
        closed: bool,
    },
}

impl HirEntryBody {
    pub(crate) const fn braced(members: Box<[HirEntryMember]>, closed: bool) -> Self {
        Self::Braced { members, closed }
    }

    pub const fn members(&self) -> &[HirEntryMember] {
        match self {
            Self::Missing => &[],
            Self::Braced { members, .. } => members,
        }
    }

    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Braced { closed: true, .. })
    }

    pub fn has_structural_recovery(&self) -> bool {
        match self {
            Self::Missing => true,
            Self::Braced { members, closed } => {
                !closed || members.iter().any(HirEntryMember::has_structural_recovery)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirEntryMember {
    StateType(HirEntryTypeBinding),
    Initializer(HirEntryPathBinding),
    EventType(HirEntryTypeBinding),
    Reducer(HirEntryPathBinding),
    Controller(HirEntryPathBinding),
    Goto(HirEntryGoto),
    Route(HirEntryRoute),
    Option(HirEntryOption),
    Error,
}

impl HirEntryMember {
    pub fn has_structural_recovery(&self) -> bool {
        match self {
            Self::StateType(value) | Self::EventType(value) => value.has_structural_recovery(),
            Self::Initializer(value) | Self::Reducer(value) | Self::Controller(value) => {
                value.has_recovery()
            }
            Self::Goto(value) => value.has_recovery(),
            Self::Route(value) => value.has_recovery(),
            Self::Option(value) => value.has_structural_recovery(),
            Self::Error => true,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEntryRoute {
    method: HirHttpMethodValue,
    path: HirRoutePathValue,
    arrow: HirEntryPunctuationState,
    target: HirEntryTarget,
    bindings: HirEntryRouteBindings,
    trailing_recovery: bool,
}

impl HirEntryRoute {
    pub(crate) const fn new(
        method: HirHttpMethodValue,
        path: HirRoutePathValue,
        arrow: HirEntryPunctuationState,
        target: HirEntryTarget,
        bindings: HirEntryRouteBindings,
        trailing_recovery: bool,
    ) -> Self {
        Self {
            method,
            path,
            arrow,
            target,
            bindings,
            trailing_recovery,
        }
    }

    pub const fn method(&self) -> &HirHttpMethodValue {
        &self.method
    }

    pub const fn path(&self) -> &HirRoutePathValue {
        &self.path
    }

    pub const fn arrow(&self) -> HirEntryPunctuationState {
        self.arrow
    }

    pub const fn target(&self) -> &HirEntryTarget {
        &self.target
    }

    pub const fn bindings(&self) -> &HirEntryRouteBindings {
        &self.bindings
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery
    }

    pub fn has_recovery(&self) -> bool {
        self.method.has_recovery()
            || self.path.has_recovery()
            || matches!(self.arrow, HirEntryPunctuationState::Missing)
            || self.target.has_recovery()
            || self.bindings.has_recovery()
            || self.trailing_recovery
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        self.method.validate()?;
        self.path.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HirHttpMethod {
    /// Canonical adapter payload spelling for this checked method.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirHttpMethodValue {
    Resolved(HirHttpMethod),
    Recovered {
        authored: Option<HirName>,
        issue: HirHttpMethodIssue,
    },
}

impl HirHttpMethodValue {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        let valid = match self {
            Self::Resolved(_) => true,
            Self::Recovered {
                authored,
                issue: HirHttpMethodIssue::Unsupported,
            } => authored.is_some(),
            Self::Recovered {
                authored,
                issue: HirHttpMethodIssue::Missing | HirHttpMethodIssue::InvalidName,
            } => authored.is_none(),
        };
        if valid {
            Ok(())
        } else {
            Err(HirItemInvariantError::InvalidEntryRecovery)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirHttpMethodIssue {
    Missing,
    Unsupported,
    InvalidName,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRoutePath {
    value: Box<str>,
    segments: Box<[HirRoutePathSegment]>,
    captures: Box<[HirRoutePathCapture]>,
}

/// One validated route-matching segment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRoutePathSegment {
    Literal(Box<str>),
    Capture(HirRouteCaptureCoordinate),
}

/// Stable source-order coordinate of one path capture.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRouteCaptureCoordinate(u32);

impl HirRouteCaptureCoordinate {
    pub const fn position(self) -> u32 {
        self.0
    }
}

/// One capture coordinate and its diagnostic authored name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRoutePathCapture {
    coordinate: HirRouteCaptureCoordinate,
    name: HirName,
}

impl HirRoutePathCapture {
    pub const fn coordinate(&self) -> HirRouteCaptureCoordinate {
        self.coordinate
    }

    pub const fn name(&self) -> &HirName {
        &self.name
    }
}

impl HirRoutePath {
    pub(crate) fn try_new(value: Box<str>) -> Result<Self, HirItemInvariantError> {
        if !value.starts_with('/') || value.chars().any(char::is_control) {
            return Err(HirItemInvariantError::InvalidRoutePath);
        }
        let mut segments = Vec::new();
        let mut captures = Vec::new();
        for segment in value.split('/').skip(1) {
            if segment.is_empty() {
                if value.as_ref() == "/" {
                    continue;
                }
                return Err(HirItemInvariantError::InvalidRoutePath);
            }
            let Some(capture) = segment.strip_prefix(':') else {
                segments.push(HirRoutePathSegment::Literal(segment.into()));
                continue;
            };
            let capture = HirName::try_new(capture.into())
                .map_err(|_| HirItemInvariantError::InvalidRoutePath)?;
            if captures
                .iter()
                .any(|existing: &HirRoutePathCapture| existing.name == capture)
            {
                return Err(HirItemInvariantError::InvalidRoutePath);
            }
            let coordinate = u32::try_from(captures.len())
                .map(HirRouteCaptureCoordinate)
                .map_err(|_| HirItemInvariantError::InvalidRoutePath)?;
            captures.push(HirRoutePathCapture {
                coordinate,
                name: capture,
            });
            segments.push(HirRoutePathSegment::Capture(coordinate));
        }
        Ok(Self {
            value,
            segments: segments.into_boxed_slice(),
            captures: captures.into_boxed_slice(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Validated path-capture names in exact path-segment order.
    ///
    /// Entry checking joins route bindings to this inventory without parsing
    /// the retained path string a second time.
    pub fn captures(&self) -> &[HirRoutePathCapture] {
        &self.captures
    }

    pub fn segments(&self) -> &[HirRoutePathSegment] {
        &self.segments
    }

    /// Whether both paths can accept at least one identical request path.
    /// Capture names are source identities and never affect dispatch shape.
    pub fn overlaps_dispatch(&self, other: &Self) -> bool {
        self.segments.len() == other.segments.len()
            && self
                .segments
                .iter()
                .zip(other.segments.iter())
                .all(|(left, right)| match (left, right) {
                    (HirRoutePathSegment::Literal(left), HirRoutePathSegment::Literal(right)) => {
                        left == right
                    }
                    (
                        HirRoutePathSegment::Literal(_) | HirRoutePathSegment::Capture(_),
                        HirRoutePathSegment::Capture(_),
                    )
                    | (HirRoutePathSegment::Capture(_), HirRoutePathSegment::Literal(_)) => true,
                })
    }

    /// Canonical ordering of route dispatch shapes, ignoring capture names.
    pub fn dispatch_cmp(&self, other: &Self) -> Ordering {
        for (left, right) in self.segments.iter().zip(other.segments.iter()) {
            let ordering = match (left, right) {
                (HirRoutePathSegment::Literal(left), HirRoutePathSegment::Literal(right)) => {
                    left.cmp(right)
                }
                (HirRoutePathSegment::Literal(_), HirRoutePathSegment::Capture(_)) => {
                    Ordering::Less
                }
                (HirRoutePathSegment::Capture(_), HirRoutePathSegment::Literal(_)) => {
                    Ordering::Greater
                }
                (HirRoutePathSegment::Capture(_), HirRoutePathSegment::Capture(_)) => {
                    Ordering::Equal
                }
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        self.segments.len().cmp(&other.segments.len())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRoutePathValue {
    Resolved(HirRoutePath),
    Recovered {
        decoded: Option<Box<str>>,
        issue: HirRoutePathIssue,
    },
}

impl HirRoutePathValue {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        let valid = match self {
            Self::Resolved(_) => true,
            Self::Recovered {
                decoded,
                issue: HirRoutePathIssue::InvalidPath,
            } => decoded.is_some(),
            Self::Recovered {
                decoded,
                issue:
                    HirRoutePathIssue::Missing
                    | HirRoutePathIssue::InvalidExpression
                    | HirRoutePathIssue::InvalidString(_),
            } => decoded.is_none(),
        };
        if valid {
            Ok(())
        } else {
            Err(HirItemInvariantError::InvalidEntryRecovery)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRoutePathIssue {
    Missing,
    InvalidExpression,
    InvalidString(HirStringIssue),
    InvalidPath,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirEntryRouteBindings {
    Absent,
    Parenthesized {
        items: Box<[HirEntryRouteBinding]>,
        closed: bool,
    },
}

impl HirEntryRouteBindings {
    pub const fn items(&self) -> &[HirEntryRouteBinding] {
        match self {
            Self::Absent => &[],
            Self::Parenthesized { items, .. } => items,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Absent => false,
            Self::Parenthesized { items, closed } => {
                !closed || items.iter().any(HirEntryRouteBinding::has_recovery)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEntryRouteBinding {
    parameter: HirRequiredName,
    assignment: HirEntryPunctuationState,
    colon: HirEntryPunctuationState,
    path_capture: HirRequiredName,
    trailing_recovery: bool,
}

impl HirEntryRouteBinding {
    pub(crate) const fn new(
        parameter: HirRequiredName,
        assignment: HirEntryPunctuationState,
        colon: HirEntryPunctuationState,
        path_capture: HirRequiredName,
        trailing_recovery: bool,
    ) -> Self {
        Self {
            parameter,
            assignment,
            colon,
            path_capture,
            trailing_recovery,
        }
    }

    pub const fn parameter(&self) -> &HirRequiredName {
        &self.parameter
    }

    pub const fn assignment(&self) -> HirEntryPunctuationState {
        self.assignment
    }

    pub const fn colon(&self) -> HirEntryPunctuationState {
        self.colon
    }

    pub const fn path_capture(&self) -> &HirRequiredName {
        &self.path_capture
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self.parameter, HirRequiredName::Resolved(_))
            || matches!(self.assignment, HirEntryPunctuationState::Missing)
            || matches!(self.colon, HirEntryPunctuationState::Missing)
            || !matches!(self.path_capture, HirRequiredName::Resolved(_))
            || self.trailing_recovery
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirEntryOption {
    name: HirRequiredName,
    assignment: HirEntryPunctuationState,
    value: HirEntryOptionValue,
    trailing_recovery: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirEntryOptionValue {
    Expression(ExprId),
    Missing,
    Invalid,
}

impl HirEntryOptionValue {
    pub const fn expression(&self) -> Option<ExprId> {
        match self {
            Self::Expression(value) => Some(*value),
            Self::Missing | Self::Invalid => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing | Self::Invalid)
    }
}

impl HirEntryOption {
    pub(crate) const fn new(
        name: HirRequiredName,
        assignment: HirEntryPunctuationState,
        value: HirEntryOptionValue,
        trailing_recovery: bool,
    ) -> Self {
        Self {
            name,
            assignment,
            value,
            trailing_recovery,
        }
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn assignment(&self) -> HirEntryPunctuationState {
        self.assignment
    }

    pub const fn value(&self) -> &HirEntryOptionValue {
        &self.value
    }

    pub const fn has_trailing_recovery(&self) -> bool {
        self.trailing_recovery
    }

    pub const fn has_structural_recovery(&self) -> bool {
        !matches!(self.name, HirRequiredName::Resolved(_))
            || matches!(self.assignment, HirEntryPunctuationState::Missing)
            || self.value.has_recovery()
            || self.trailing_recovery
    }
}
