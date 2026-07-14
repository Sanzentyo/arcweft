//! Canonical typed Style data, resolution, and reactive property bindings.

use crate::{DirtyFlags, ViewError};
use std::collections::BTreeSet;

/// Stable identifier for a style property slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPropertyId(pub u32);

/// Stable identifier for a dynamic value source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueSourceId(pub u32);

pub mod axis;
pub mod cascade;
pub mod computed;
pub mod property;
pub mod resolver;
pub mod selector;
pub mod sheet;
pub mod trace;
pub mod value;

pub use axis::{
    ViewAxisSign, ViewAxisUsageSet, ViewBoxAxisMode, ViewBoxAxisModeError, ViewBoxAxisRevision,
    ViewBoxAxisSeedSource, ViewBoxAxisSource, ViewInheritedBoxAxes, ViewPhysicalAxis,
    ViewPhysicalBoxStyle, ViewPhysicalEdges, ViewPhysicalSide, ViewResolvedAxis,
    ViewResolvedBoxAxes,
};
pub use cascade::{
    ComputedViewStyleBuilder, ViewStyleContribution, ViewStyleContributionSource, ViewStylePriority,
};
pub use computed::{
    ComputedViewAxes, ComputedViewProperty, ComputedViewStyle, ComputedViewStyleRevision,
    ComputedViewTransition,
};
pub use property::{
    ViewComputedPropertyKind, ViewPropertyExpansion, ViewPropertyKind, ViewPropertyResolution,
    ViewPropertyValueTransform, ViewStyleInvalidationSet, ViewStyleValueKind,
};
pub use resolver::{
    ViewElementStateSet, ViewInteractionStateSet, ViewStyleNodeFacts, ViewStyleNodeKey,
    ViewStyleResolution, ViewStyleResolveContext, ViewStyleResolveError, ViewStyleResolver,
    ViewStyleResolverLimits, ViewStyleRevisionSet,
};
pub use selector::{
    ViewContainerAxis, ViewContainerPredicate, ViewElementState, ViewEnvironmentPredicate,
    ViewInteractionSelector, ViewPartName, ViewStyleCombinator, ViewStyleComparison,
    ViewStylePredicate, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSpecificity,
};
pub use sheet::{
    ViewStyleApplication, ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts,
    ViewStyleDeclaration, ViewStyleModelError, ViewStylePatch, ViewStylePatchId, ViewStyleProgram,
    ViewStyleRule, ViewStyleScopeId, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
    ViewStyleToken, ViewStyleTokenId,
};
pub use trace::{ViewStyleTrace, ViewStyleTraceEntry, ViewStyleTraceMode, ViewStyleTraceRejection};
pub use value::{
    ViewAlignment, ViewAngleMilliDegrees, ViewAxisValueError, ViewBlendMode, ViewBorderRadii,
    ViewClip, ViewColorValue, ViewDisplay, ViewFilter, ViewFlexDirection, ViewFlexWrap,
    ViewFontFamily, ViewFontFamilyList, ViewFontStyle, ViewFontWeight, ViewLengthMilli, ViewMask,
    ViewOverflow, ViewPosition, ViewRatioMilli, ViewScalarMilli, ViewShadow, ViewSpecifiedValue,
    ViewStyleTransition, ViewSystemFontFamily,
};

/// Dynamic property binding emitted by Rust or Arcweft view rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyBinding {
    property: ViewPropertyId,
    kind: ViewPropertyKind,
    source: ValueSourceId,
    invalidation: ViewStyleInvalidationSet,
}

/// Ordered dynamic property binding table for a fragment or node.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PropertyBindingTable {
    bindings: Vec<PropertyBinding>,
}

/// Builder that rejects duplicate property slots deterministically.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropertyBindingTableBuilder {
    bindings: Vec<PropertyBinding>,
    properties: BTreeSet<ViewPropertyId>,
}

impl ViewStyleInvalidationSet {
    /// Maps the canonical invalidation union to retained entity work flags.
    pub fn dirty_flags(self) -> DirtyFlags {
        let mut flags = DirtyFlags::NONE;
        if self.contains(Self::FRAGMENT) {
            flags.insert(DirtyFlags::FRAGMENT);
        }
        if self.contains(Self::LAYOUT) || self.contains(Self::TEXT_LAYOUT) {
            flags.insert(DirtyFlags::LAYOUT);
        }
        if self.contains(Self::SEMANTICS) {
            flags.insert(DirtyFlags::SEMANTICS);
        }
        if self.contains(Self::PAINT)
            || self.contains(Self::COMPOSITE)
            || self.contains(Self::RESOURCE)
        {
            flags.insert(DirtyFlags::PAINT);
        }
        flags
    }
}

impl PropertyBinding {
    pub const fn new(
        property: ViewPropertyId,
        kind: ViewPropertyKind,
        source: ValueSourceId,
    ) -> Self {
        Self {
            property,
            kind,
            source,
            invalidation: kind.default_invalidation(),
        }
    }

    #[must_use]
    pub const fn with_invalidation(mut self, invalidation: ViewStyleInvalidationSet) -> Self {
        self.invalidation = invalidation;
        self
    }

    pub const fn property(self) -> ViewPropertyId {
        self.property
    }

    pub const fn kind(self) -> ViewPropertyKind {
        self.kind
    }

    pub const fn source(self) -> ValueSourceId {
        self.source
    }

    pub const fn invalidation(self) -> ViewStyleInvalidationSet {
        self.invalidation
    }

    pub fn dirty_flags(self) -> DirtyFlags {
        self.invalidation.dirty_flags()
    }
}

impl PropertyBindingTable {
    pub fn as_slice(&self) -> &[PropertyBinding] {
        &self.bindings
    }

    pub fn dirty_flags_for_source(&self, source: ValueSourceId) -> DirtyFlags {
        self.bindings
            .iter()
            .filter(|binding| binding.source() == source)
            .fold(DirtyFlags::NONE, |mut flags, binding| {
                flags.insert(binding.dirty_flags());
                flags
            })
    }
}

impl PropertyBindingTableBuilder {
    pub fn push(&mut self, binding: PropertyBinding) -> Result<(), ViewError> {
        if !self.properties.insert(binding.property()) {
            return Err(ViewError::DuplicatePropertyBinding(binding.property()));
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn finish(self) -> PropertyBindingTable {
        PropertyBindingTable {
            bindings: self.bindings,
        }
    }
}
