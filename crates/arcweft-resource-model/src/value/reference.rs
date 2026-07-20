use super::{MAX_RESOURCE_VALUE_NESTING, ResourceValueType};
use crate::{
    descriptor::{ResourceValueSchema, ResourceValueSchemaKind},
    identity::{
        ResourceAssetPayloadKindId, ResourceFieldId, ResourceSchemaId, ResourceTypeId,
        ResourceVariantId,
    },
    registry::ResourceTypeRegistry,
    retained::RetainedIdentityKind,
};
use std::collections::BTreeSet;
use thiserror::Error;

/// One structural segment traversed while locating a reference-bearing type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceValueTypePathSegment {
    OptionValue,
    SequenceElement,
    MapKey,
    MapValue,
    RecordField(ResourceFieldId),
    EnumVariant(ResourceVariantId),
}

/// Descriptor-local structural location of a reference-bearing type.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceValueTypePath(Vec<ResourceValueTypePathSegment>);

/// One nominal or exact reference requirement owned by a resource value type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceReferenceRequirementKind<'a> {
    NominalRecord {
        schema_id: &'a ResourceSchemaId,
    },
    NominalEnum {
        schema_id: &'a ResourceSchemaId,
    },
    Asset {
        payload_kind: &'a ResourceAssetPayloadKindId,
    },
    Resource {
        type_id: &'a ResourceTypeId,
    },
    Retained {
        identity: RetainedIdentityKind,
    },
}

/// One reference requirement together with its exact structural path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceReferenceRequirement<'a> {
    path: ResourceValueTypePath,
    kind: ResourceReferenceRequirementKind<'a>,
}

/// Resource value-type traversal exceeded the shared nesting budget.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("resource reference type nesting exceeds the supported depth at {path:?}")]
pub struct ResourceReferenceTraversalError {
    path: ResourceValueTypePath,
}

/// Invalid exact reference category reachable from one resource value type.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceSchemaError {
    #[error("resource reference type nesting exceeds the supported depth at {path:?}")]
    NestingTooDeep { path: ResourceValueTypePath },
    #[error("resource value type references unknown nominal schema `{schema_id}` at {path:?}")]
    UnknownNominalSchema {
        path: ResourceValueTypePath,
        schema_id: ResourceSchemaId,
    },
    #[error(
        "resource value type expects `{schema_id}` at {path:?} to be {expected:?}, found {actual:?}"
    )]
    NominalSchemaKindMismatch {
        path: ResourceValueTypePath,
        schema_id: ResourceSchemaId,
        expected: ResourceValueSchemaKind,
        actual: ResourceValueSchemaKind,
    },
    #[error("resource value type references unknown resource type `{type_id}` at {path:?}")]
    UnknownResourceType {
        path: ResourceValueTypePath,
        type_id: ResourceTypeId,
    },
}

impl ResourceValueType {
    /// Collects every direct nominal and exact reference requirement through
    /// structural collection wrappers.
    ///
    /// The registry and manifest conversion layers consume this inventory
    /// instead of maintaining field-name or family-specific descriptor walks.
    /// Use [`Self::validate_reference_invariants`] when accepted nominal schema
    /// children must also be followed.
    pub fn reference_requirements(
        &self,
    ) -> Result<Vec<ResourceReferenceRequirement<'_>>, ResourceReferenceTraversalError> {
        let mut requirements = Vec::new();
        let mut path = ResourceValueTypePath::default();
        self.collect_reference_requirements(0, &mut path, &mut requirements)?;
        Ok(requirements)
    }

    /// Validates every exact reference category reachable through this type.
    ///
    /// Nominal records and enums are traversed through the accepted immutable
    /// registry. Recursive nominal schemas terminate at the first active
    /// schema while every non-recursive occurrence retains its exact path.
    pub fn validate_reference_invariants(
        &self,
        registry: &ResourceTypeRegistry,
        path: &ResourceValueTypePath,
    ) -> Result<(), ResourceSchemaError> {
        let mut path = path.clone();
        let mut active_schemas = BTreeSet::new();
        self.validate_reference_invariants_at(registry, 0, &mut path, &mut active_schemas)
    }

    fn collect_reference_requirements<'a>(
        &'a self,
        depth: usize,
        path: &mut ResourceValueTypePath,
        requirements: &mut Vec<ResourceReferenceRequirement<'a>>,
    ) -> Result<(), ResourceReferenceTraversalError> {
        if depth > MAX_RESOURCE_VALUE_NESTING {
            return Err(ResourceReferenceTraversalError { path: path.clone() });
        }
        match self {
            Self::Option(value) => value.collect_with_segment(
                depth,
                path,
                ResourceValueTypePathSegment::OptionValue,
                requirements,
            ),
            Self::Vec(value) | Self::NonEmptyVec(value) => value.collect_with_segment(
                depth,
                path,
                ResourceValueTypePathSegment::SequenceElement,
                requirements,
            ),
            Self::Map { key, value } => {
                key.collect_with_segment(
                    depth,
                    path,
                    ResourceValueTypePathSegment::MapKey,
                    requirements,
                )?;
                value.collect_with_segment(
                    depth,
                    path,
                    ResourceValueTypePathSegment::MapValue,
                    requirements,
                )
            }
            Self::NominalRecord(schema_id) => {
                requirements.push(ResourceReferenceRequirement::new(
                    path.clone(),
                    ResourceReferenceRequirementKind::NominalRecord { schema_id },
                ));
                Ok(())
            }
            Self::NominalEnum(schema_id) => {
                requirements.push(ResourceReferenceRequirement::new(
                    path.clone(),
                    ResourceReferenceRequirementKind::NominalEnum { schema_id },
                ));
                Ok(())
            }
            Self::AssetRef { payload_kind } => {
                requirements.push(ResourceReferenceRequirement::new(
                    path.clone(),
                    ResourceReferenceRequirementKind::Asset { payload_kind },
                ));
                Ok(())
            }
            Self::ResourceRef { type_id } => {
                requirements.push(ResourceReferenceRequirement::new(
                    path.clone(),
                    ResourceReferenceRequirementKind::Resource { type_id },
                ));
                Ok(())
            }
            Self::RetainedIdentityRef { identity } => {
                requirements.push(ResourceReferenceRequirement::new(
                    path.clone(),
                    ResourceReferenceRequirementKind::Retained {
                        identity: *identity,
                    },
                ));
                Ok(())
            }
            Self::Scalar(_) | Self::ConstrainedScalar(_) => Ok(()),
        }
    }

    fn collect_with_segment<'a>(
        &'a self,
        depth: usize,
        path: &mut ResourceValueTypePath,
        segment: ResourceValueTypePathSegment,
        requirements: &mut Vec<ResourceReferenceRequirement<'a>>,
    ) -> Result<(), ResourceReferenceTraversalError> {
        path.0.push(segment);
        let result = self.collect_reference_requirements(depth + 1, path, requirements);
        path.0.pop();
        result
    }

    fn validate_reference_invariants_at(
        &self,
        registry: &ResourceTypeRegistry,
        depth: usize,
        path: &mut ResourceValueTypePath,
        active_schemas: &mut BTreeSet<ResourceSchemaId>,
    ) -> Result<(), ResourceSchemaError> {
        if depth > MAX_RESOURCE_VALUE_NESTING {
            return Err(ResourceSchemaError::NestingTooDeep { path: path.clone() });
        }
        match self {
            Self::Option(value) => value.validate_with_segment(
                registry,
                depth,
                path,
                ResourceValueTypePathSegment::OptionValue,
                active_schemas,
            ),
            Self::Vec(value) | Self::NonEmptyVec(value) => value.validate_with_segment(
                registry,
                depth,
                path,
                ResourceValueTypePathSegment::SequenceElement,
                active_schemas,
            ),
            Self::Map { key, value } => {
                key.validate_with_segment(
                    registry,
                    depth,
                    path,
                    ResourceValueTypePathSegment::MapKey,
                    active_schemas,
                )?;
                value.validate_with_segment(
                    registry,
                    depth,
                    path,
                    ResourceValueTypePathSegment::MapValue,
                    active_schemas,
                )
            }
            Self::NominalRecord(schema_id) => {
                let schema = required_nominal_schema(
                    registry,
                    schema_id,
                    ResourceValueSchemaKind::Record,
                    path,
                )?;
                if !active_schemas.insert(schema_id.clone()) {
                    return Ok(());
                }
                let ResourceValueSchema::Record(schema) = schema else {
                    unreachable!("required_nominal_schema checked the schema kind");
                };
                let result = schema.fields().iter().try_for_each(|field| {
                    field.value_type().validate_with_segment(
                        registry,
                        depth,
                        path,
                        ResourceValueTypePathSegment::RecordField(field.id()),
                        active_schemas,
                    )
                });
                active_schemas.remove(schema_id);
                result
            }
            Self::NominalEnum(schema_id) => {
                let schema = required_nominal_schema(
                    registry,
                    schema_id,
                    ResourceValueSchemaKind::Enum,
                    path,
                )?;
                if !active_schemas.insert(schema_id.clone()) {
                    return Ok(());
                }
                let ResourceValueSchema::Enum(schema) = schema else {
                    unreachable!("required_nominal_schema checked the schema kind");
                };
                let result = schema.variants().iter().try_for_each(|variant| {
                    variant.payload().map_or(Ok(()), |payload| {
                        payload.validate_with_segment(
                            registry,
                            depth,
                            path,
                            ResourceValueTypePathSegment::EnumVariant(variant.id()),
                            active_schemas,
                        )
                    })
                });
                active_schemas.remove(schema_id);
                result
            }
            Self::ResourceRef { type_id } => {
                if registry.get(type_id).is_none() {
                    return Err(ResourceSchemaError::UnknownResourceType {
                        path: path.clone(),
                        type_id: type_id.clone(),
                    });
                }
                Ok(())
            }
            Self::Scalar(_)
            | Self::ConstrainedScalar(_)
            | Self::AssetRef { .. }
            | Self::RetainedIdentityRef { .. } => Ok(()),
        }
    }

    fn validate_with_segment(
        &self,
        registry: &ResourceTypeRegistry,
        depth: usize,
        path: &mut ResourceValueTypePath,
        segment: ResourceValueTypePathSegment,
        active_schemas: &mut BTreeSet<ResourceSchemaId>,
    ) -> Result<(), ResourceSchemaError> {
        path.0.push(segment);
        let result =
            self.validate_reference_invariants_at(registry, depth + 1, path, active_schemas);
        path.0.pop();
        result
    }
}

impl ResourceValueTypePath {
    pub fn new(segments: impl IntoIterator<Item = ResourceValueTypePathSegment>) -> Self {
        Self(segments.into_iter().collect())
    }

    pub fn segments(&self) -> &[ResourceValueTypePathSegment] {
        &self.0
    }
}

impl<'a> ResourceReferenceRequirement<'a> {
    const fn new(path: ResourceValueTypePath, kind: ResourceReferenceRequirementKind<'a>) -> Self {
        Self { path, kind }
    }

    pub const fn path(&self) -> &ResourceValueTypePath {
        &self.path
    }

    pub const fn kind(&self) -> ResourceReferenceRequirementKind<'a> {
        self.kind
    }
}

impl ResourceReferenceTraversalError {
    pub const fn path(&self) -> &ResourceValueTypePath {
        &self.path
    }
}

fn required_nominal_schema<'a>(
    registry: &'a ResourceTypeRegistry,
    schema_id: &ResourceSchemaId,
    expected: ResourceValueSchemaKind,
    path: &ResourceValueTypePath,
) -> Result<&'a ResourceValueSchema, ResourceSchemaError> {
    let schema =
        registry
            .schema(schema_id)
            .ok_or_else(|| ResourceSchemaError::UnknownNominalSchema {
                path: path.clone(),
                schema_id: schema_id.clone(),
            })?;
    if schema.kind() != expected {
        return Err(ResourceSchemaError::NominalSchemaKindMismatch {
            path: path.clone(),
            schema_id: schema_id.clone(),
            expected,
            actual: schema.kind(),
        });
    }
    Ok(schema)
}
