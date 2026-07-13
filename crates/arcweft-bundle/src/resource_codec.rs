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
pub mod view;
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
pub use kind::ProductSectionCodecKind;
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
pub use view::{
    SystemColorOverride, ViewActionButtonActionResource, ViewActionButtonResource,
    ViewActionPayloadResource, ViewActionTextControlPayloadField, ViewAwaitBranchSpan,
    ViewCallArgumentBindingRef, ViewDefinitionResource, ViewDisplayFrameResource,
    ViewFocusAutoScrollPolicy, ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource,
    ViewFocusInitialPolicy, ViewFocusNavigationEdge, ViewFocusNavigationResource,
    ViewFocusSkipPolicy, ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewFxArgumentBindingRef,
    ViewInputResource, ViewInstructionSpan, ViewLayoutBoundsKind, ViewLayoutBoundsResource,
    ViewLocalizedTextResource, ViewLogicalRect, ViewParameterResource, ViewProgramResource,
    ViewProgramStyleResources, ViewResourceBudget, ViewResourceCompatibility,
    ViewResourceMergeError, ViewRichTextDocumentResource, ViewRuntimeActionButton,
    ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds, ViewRuntimeControlBorderStyle,
    ViewRuntimeControlCornerFrameStyle, ViewRuntimeControlFocusRingStyle,
    ViewRuntimeControlVisualStyle, ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation,
    ViewRuntimeFocusNavigationEdge, ViewRuntimeNodeStyle, ViewRuntimeScrollRegion,
    ViewRuntimeScrollRegionBounds, ViewRuntimeShadow, ViewRuntimeShadowKind,
    ViewRuntimeStyleProjectionError, ViewRuntimeStyleProperties, ViewRuntimeSurface,
    ViewRuntimeSurfaceBounds, ViewRuntimeTextControl, ViewRuntimeTextControlBounds,
    ViewRuntimeTextControlHandler, ViewRuntimeTextControlHandlerRuntime,
    ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions, ViewRuntimeTextSelection,
    ViewScrollAxis, ViewScrollIndicatorsPolicy, ViewScrollOverflowPolicy,
    ViewScrollOverscrollPolicy, ViewScrollRegionResource, ViewStyleContractError,
    ViewStyleResource, ViewSurfaceResource, ViewTextBlockBounds, ViewTextBlockResource,
    ViewTextResource, ViewTextStyleBinding, ViewThemeEnvironmentDefaults,
    ViewThemeEnvironmentError, ViewThemeResource, ViewValueInputNamespace, ViewValueInputResource,
    ViewValueInputSource, migrated_view_section_compatibility,
};
pub use wire::{DecodedResourceSection, ProductResourceEnvelope};
