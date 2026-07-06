//! Shared product resource section codec contracts.
//!
//! Product resource sections migrate away from JSON one section family at a
//! time. This module owns the Sans I/O common table/budget/wire model used by
//! those compact binary sections; it does not perform filesystem, network,
//! signing, cache, or platform capability checks.

pub mod budget;
mod codec_io;
pub mod error;
pub mod field;
pub mod header;
pub mod inspection;
pub mod kind;
pub mod product_catalog;
pub mod runtime;
pub mod table;
pub mod types;
pub mod ui;
pub mod wire;

pub use budget::SectionCodecBudget;
pub use error::SectionCodecError;
pub use field::{
    FieldId, FieldRegistry, FieldRequirement, FieldSpec, ResourceField, ResourceWireType,
};
pub use header::{
    PRODUCT_SECTION_HEADER_LEN, PRODUCT_SECTION_SCHEMA_VERSION, PRODUCT_SECTION_WIRE_ALIGNMENT,
    ProductSectionHeader,
};
pub use inspection::{EnumSymbolInspection, ResourceFieldInspection, ResourceInspection};
pub use kind::{ProductResourceMigrationStatus, ProductSectionCodecKind};
pub use product_catalog::{
    AssetCatalogSection as CompactAssetCatalogSection,
    AudioGraphSection as CompactAudioGraphSection,
    ContentCatalogSection as CompactContentCatalogSection,
    DisplayCatalogSection as CompactDisplayCatalogSection,
    SourceMapSection as CompactSourceMapSection,
};
pub use runtime::{
    AdapterRequirementsSection as CompactAdapterRequirementsSection,
    EntrypointsSection as CompactEntrypointsSection,
    RuntimeTypesSection as CompactRuntimeTypesSection,
};
pub use table::{EnumRegistry, EnumSymbol, PublicIdRef, PublicIdTable, StringId, StringTable};
pub use types::{CrossSectionRef, DigestRef, SourceRangeRef, StableId};
pub use ui::{
    CompactUiInputResource, CompactUiStyleResource, CompactUiTextResource, CompactUiThemeResource,
    CompactViewProgramResource, UiInputResource, UiLogicalRect, UiResourceBudget,
    UiResourceCompatibility, UiRuntimeControlBorderStyle, UiRuntimeControlCornerFrameStyle,
    UiRuntimeControlFocusRingStyle, UiRuntimeControlState, UiRuntimeControlStyle,
    UiRuntimeControlStyleDiagnostic, UiRuntimeControlStyleDiagnosticReason,
    UiRuntimeControlStyleDiagnostics, UiRuntimeControlStyleResolution, UiRuntimeControlVisualStyle,
    UiRuntimeShadow, UiRuntimeShadowKind, UiRuntimeStyledControls, UiRuntimeTextControl,
    UiRuntimeTextControlBounds, UiRuntimeTextControlHandler, UiRuntimeTextControlHandlerRuntime,
    UiRuntimeTextControlHandlers, UiRuntimeTextControlOptions, UiRuntimeTextSelection,
    UiStyleResource, UiTextResource, UiThemeResource, ViewActionButtonActionResource,
    ViewActionButtonResource, ViewActionPayloadResource, ViewActionTextControlPayloadField,
    ViewAwaitBranchSpan, ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource,
    ViewFocusInitialPolicy, ViewFocusNavigationEdge, ViewFocusNavigationResource,
    ViewFocusSkipPolicy, ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewLayoutBoundsKind,
    ViewLayoutBoundsResource, ViewProgramResource, ViewRuntimeActionButton,
    ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds, ViewRuntimeFocusGroup,
    ViewRuntimeFocusNavigation, ViewRuntimeFocusNavigationEdge, ViewRuntimeScrollRegion,
    ViewRuntimeScrollRegionBounds, ViewRuntimeTextBlock, ViewRuntimeTextBlockBounds,
    ViewScrollAxis, ViewScrollOverflowPolicy, ViewScrollRegionResource, ViewTextBlockResource,
    migrated_ui_section_compatibility,
};
pub use wire::{DecodedResourceSection, ProductResourceEnvelope};
