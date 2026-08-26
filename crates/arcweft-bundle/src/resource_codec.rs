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
pub mod locale_catalog;
pub mod product_catalog;
pub mod runtime;
pub mod source_map;
pub mod table;
pub mod types;
pub mod view;
pub mod wire;

pub use arcweft_source::SourceSetRevision;
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
pub use locale_catalog::{
    CharacterPresentationCatalogCodecError, CharacterPresentationCatalogSection,
};
pub use product_catalog::{
    AssetCatalogSection as CompactAssetCatalogSection,
    AudioGraphSection as CompactAudioGraphSection,
    ContentCatalogSection as CompactContentCatalogSection,
    DisplayCatalogSection as CompactDisplayCatalogSection,
};
pub use runtime::{
    AdapterRequirementsSection as CompactAdapterRequirementsSection,
    EntrypointsSection as CompactEntrypointsSection,
    RuntimeTypesSection as CompactRuntimeTypesSection,
};
pub use source_map::{
    MAX_SOURCE_BYTES_PER_DOCUMENT, MAX_SOURCE_DISPLAY_NAME_BYTES, MAX_SOURCE_MAP_DOCUMENTS,
    MAX_SOURCE_MAP_TOTAL_UTF8_BYTES, SourceMapBuildError, SourceMapCodecError, SourceMapDocument,
    SourceMapSection,
};
pub use table::{EnumRegistry, EnumSymbol, PublicIdRef, PublicIdTable, StringId, StringTable};
pub use types::{
    CrossSectionRef, DigestRef, ProductSourceRefIndex, SourceRangeRef, StableId,
    ViewProductBuildError,
};
pub use view::{
    SystemColorOverride, ValidatedViewProduct, ValidatedViewProgramResource,
    ValidatedViewStyleResource, ViewActionButtonActionResource, ViewActionButtonResource,
    ViewActionPayloadResource, ViewActionTextControlPayloadField, ViewAwaitBranchSpan,
    ViewCallArgumentBindingRef, ViewDefinitionResource, ViewDisplayFrameResource,
    ViewFocusAutoScrollPolicy, ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource,
    ViewFocusInitialPolicy, ViewFocusNavigationEdge, ViewFocusNavigationResource,
    ViewFocusSkipPolicy, ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewFxArgumentBindingRef,
    ViewInputResource, ViewInstructionSpan, ViewLayoutBoundsKind, ViewLayoutBoundsResource,
    ViewLocalizedTextResource, ViewLogicalRect, ViewParameterResource, ViewProductValidationError,
    ViewProductValidationLimits, ViewProgramId, ViewProgramResource, ViewProgramStyleResources,
    ViewResourceBudget, ViewResourceCompatibility, ViewResourceMergeError,
    ViewRichTextDocumentResource, ViewRuntimeActionButton, ViewRuntimeActionButtonAction,
    ViewRuntimeButtonBounds, ViewRuntimeControlBorderStyle, ViewRuntimeControlCornerFrameStyle,
    ViewRuntimeControlFocusRingStyle, ViewRuntimeControlVisualStyle, ViewRuntimeFocusGroup,
    ViewRuntimeFocusNavigation, ViewRuntimeFocusNavigationEdge, ViewRuntimeGeometryOwner,
    ViewRuntimeGeometryParticipation, ViewRuntimeNodeStyle, ViewRuntimePhysicalNodeStyle,
    ViewRuntimeScrollRegion, ViewRuntimeScrollRegionBounds, ViewRuntimeShadow,
    ViewRuntimeShadowKind, ViewRuntimeStyleProjectionError, ViewRuntimeStyleProperties,
    ViewRuntimeSurface, ViewRuntimeSurfaceBounds, ViewRuntimeTextControl,
    ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandler, ViewRuntimeTextControlHandlers,
    ViewRuntimeTextControlOptions, ViewRuntimeTextSelection, ViewScrollAxis,
    ViewScrollIndicatorsPolicy, ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy,
    ViewScrollRegionResource, ViewStyleContractError, ViewStyleResource, ViewSurfaceResource,
    ViewTextBlockBounds, ViewTextBlockResource, ViewTextResource, ViewTextStyleBinding,
    ViewThemeResource, ViewValueInputNamespace, ViewValueInputResource, ViewValueInputSource,
    migrated_view_section_compatibility,
};
pub use wire::{DecodedResourceSection, ProductResourceEnvelope};
