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
pub mod table;
pub mod types;
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
pub use table::{EnumRegistry, EnumSymbol, PublicIdRef, PublicIdTable, StringId, StringTable};
pub use types::{CrossSectionRef, DigestRef, SourceRangeRef, StableId};
pub use wire::{DecodedResourceSection, ProductResourceEnvelope};
