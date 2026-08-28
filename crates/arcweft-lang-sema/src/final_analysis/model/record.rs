//! Sealed record-field sources and checked record-pattern rows.

use arcweft_core::{entry::TypeLayoutHash, value::RuntimeRecordFieldId};
use arcweft_lang_hir::{
    identity::{ExprId, LocalId, PatternId},
    leaf::HirName,
};
use std::collections::BTreeSet;

use crate::{
    env::nominal::{AcceptedEnvironmentRecordIdentity, AcceptedNominalId},
    record_field::CheckedRecordFieldSemanticId,
    semantic_coordinate::{
        CheckedBindingCoordinateEvidence, CheckedExpressionCoordinateEvidence, CheckedSemanticPath,
        StableCheckedBindingCoordinate, StablePatternCoordinate,
    },
    types::{SemanticTypeDigest, TypeKind},
};

use super::CheckedProjectNominal;

/// Exact checked result of one project or accepted-environment field select.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFieldSelection {
    owner_type: SemanticTypeDigest,
    field: CheckedRecordFieldSemanticId,
    declaration_ordinal: u32,
    runtime_field: Option<RuntimeRecordFieldId>,
    field_type: SemanticTypeDigest,
    diagnostic_name: HirName,
}

impl CheckedFieldSelection {
    pub(crate) fn try_new(
        owner_type: SemanticTypeDigest,
        field: CheckedRecordFieldSemanticId,
        declaration_ordinal: u32,
        runtime_field: Option<RuntimeRecordFieldId>,
        field_type: SemanticTypeDigest,
        diagnostic_name: HirName,
    ) -> Option<Self> {
        match (field, runtime_field) {
            (CheckedRecordFieldSemanticId::Project(_), Some(runtime_field))
                if runtime_field.zero_based() == declaration_ordinal => {}
            (CheckedRecordFieldSemanticId::Environment(semantic_id), None)
                if semantic_id
                    == crate::env::nominal::AcceptedEnvironmentFieldSemanticId::issue(
                        owner_type,
                        declaration_ordinal,
                        field_type,
                    ) => {}
            _ => return None,
        }
        Some(Self {
            owner_type,
            field,
            declaration_ordinal,
            runtime_field,
            field_type,
            diagnostic_name,
        })
    }

    pub const fn owner_type(&self) -> SemanticTypeDigest {
        self.owner_type
    }

    pub(crate) const fn field(&self) -> CheckedRecordFieldSemanticId {
        self.field
    }

    pub const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub const fn runtime_field(&self) -> Option<RuntimeRecordFieldId> {
        self.runtime_field
    }

    pub const fn field_type(&self) -> SemanticTypeDigest {
        self.field_type
    }

    pub const fn diagnostic_name(&self) -> &HirName {
        &self.diagnostic_name
    }
}

/// Exact generation-local expression source paired with its C1 coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRecordExpressionSource {
    raw: ExprId,
    coordinate: CheckedSemanticPath,
}

impl CheckedRecordExpressionSource {
    pub(crate) fn from_evidence(evidence: CheckedExpressionCoordinateEvidence) -> Self {
        Self {
            raw: evidence.owner(),
            coordinate: evidence.into_coordinate(),
        }
    }

    pub const fn raw(&self) -> ExprId {
        self.raw
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }
}

/// Exact generation-local binding source paired with its C1 coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRecordBindingSource {
    raw: LocalId,
    coordinate: StableCheckedBindingCoordinate,
}

impl CheckedRecordBindingSource {
    pub(crate) fn from_evidence(evidence: CheckedBindingCoordinateEvidence) -> Self {
        Self {
            raw: evidence.owner(),
            coordinate: evidence.into_coordinate(),
        }
    }

    pub const fn raw(&self) -> LocalId {
        self.raw
    }

    pub const fn coordinate(&self) -> &StableCheckedBindingCoordinate {
        &self.coordinate
    }
}

/// Sealed semantic and executable source of one record-expression value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedRecordValueSource {
    Expression(CheckedRecordExpressionSource),
    Binding(CheckedRecordBindingSource),
}

impl CheckedRecordValueSource {
    pub const fn expression(&self) -> Option<&CheckedRecordExpressionSource> {
        match self {
            Self::Expression(source) => Some(source),
            Self::Binding(_) => None,
        }
    }

    pub const fn binding(&self) -> Option<&CheckedRecordBindingSource> {
        match self {
            Self::Expression(_) => None,
            Self::Binding(source) => Some(source),
        }
    }
}

/// One field in the complete authored record-expression plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionRecordField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    runtime_field: RuntimeRecordFieldId,
    semantic_id: CheckedRecordFieldSemanticId,
    field_type: SemanticTypeDigest,
    source: CheckedRecordValueSource,
}

impl CheckedExpressionRecordField {
    pub(crate) const fn new(
        source_ordinal: u32,
        declaration_ordinal: u32,
        runtime_field: RuntimeRecordFieldId,
        semantic_id: CheckedRecordFieldSemanticId,
        field_type: SemanticTypeDigest,
        source: CheckedRecordValueSource,
    ) -> Self {
        Self {
            source_ordinal,
            declaration_ordinal,
            runtime_field,
            semantic_id,
            field_type,
            source,
        }
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub const fn runtime_field(&self) -> RuntimeRecordFieldId {
        self.runtime_field
    }

    pub(crate) const fn semantic_id(&self) -> CheckedRecordFieldSemanticId {
        self.semantic_id
    }

    pub const fn field_type(&self) -> SemanticTypeDigest {
        self.field_type
    }

    pub const fn source(&self) -> &CheckedRecordValueSource {
        &self.source
    }
}

/// Exact checked owner of one record pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedRecordPatternOwner {
    Project {
        nominal: CheckedProjectNominal,
        semantic_type: SemanticTypeDigest,
        layout: TypeLayoutHash,
        field_count: u32,
    },
    Environment {
        record: AcceptedEnvironmentRecordIdentity,
    },
    VariantPayload {
        payload: crate::types::VariantPayloadType,
        semantic_type: SemanticTypeDigest,
        field_count: u32,
    },
}

impl CheckedRecordPatternOwner {
    pub const fn semantic_type(&self) -> SemanticTypeDigest {
        match self {
            Self::Project { semantic_type, .. } => *semantic_type,
            Self::Environment { record } => record.semantic_type(),
            Self::VariantPayload { semantic_type, .. } => *semantic_type,
        }
    }

    pub const fn project_nominal(&self) -> Option<&CheckedProjectNominal> {
        match self {
            Self::Project { nominal, .. } => Some(nominal),
            Self::Environment { .. } | Self::VariantPayload { .. } => None,
        }
    }

    pub(crate) fn project(
        nominal: CheckedProjectNominal,
        layout: TypeLayoutHash,
        field_count: u32,
    ) -> Self {
        let semantic_type = nominal.identity();
        Self::Project {
            nominal,
            semantic_type,
            layout,
            field_count,
        }
    }

    pub(crate) const fn environment(record: AcceptedEnvironmentRecordIdentity) -> Self {
        Self::Environment { record }
    }

    pub(crate) fn variant_payload(payload: crate::types::VariantPayloadType) -> Option<Self> {
        let field_count = u32::try_from(payload.shape().record_fields()?.len()).ok()?;
        let semantic_type =
            TypeKind::VariantPayload(Box::new(payload.clone())).semantic_identity_digest();
        Some(Self::VariantPayload {
            payload,
            semantic_type,
            field_count,
        })
    }

    pub const fn environment_nominal(&self) -> Option<&AcceptedNominalId> {
        match self {
            Self::Project { .. } | Self::VariantPayload { .. } => None,
            Self::Environment { record } => Some(record.nominal()),
        }
    }

    const fn field_count(&self) -> u32 {
        match self {
            Self::Project { field_count, .. } => *field_count,
            Self::Environment { record } => record.field_count(),
            Self::VariantPayload { field_count, .. } => *field_count,
        }
    }
}

/// Sealed source of one record-pattern field.
#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckedRecordPatternSourceKind {
    Pattern {
        raw: PatternId,
        coordinate: StablePatternCoordinate,
    },
    Binding(CheckedRecordBindingSource),
}

/// Sealed source of one record-pattern field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRecordPatternSource {
    kind: CheckedRecordPatternSourceKind,
}

/// Borrowed exhaustive view of one checked record-pattern source.
#[derive(Clone, Copy, Debug)]
pub enum CheckedRecordPatternSourceRef<'a> {
    Pattern(PatternId),
    Binding(&'a CheckedRecordBindingSource),
}

impl CheckedRecordPatternSource {
    pub(crate) const fn pattern(raw: PatternId, coordinate: StablePatternCoordinate) -> Self {
        Self {
            kind: CheckedRecordPatternSourceKind::Pattern { raw, coordinate },
        }
    }

    pub(crate) const fn from_binding(binding: CheckedRecordBindingSource) -> Self {
        Self {
            kind: CheckedRecordPatternSourceKind::Binding(binding),
        }
    }

    pub const fn value(&self) -> CheckedRecordPatternSourceRef<'_> {
        match &self.kind {
            CheckedRecordPatternSourceKind::Pattern { raw, .. } => {
                CheckedRecordPatternSourceRef::Pattern(*raw)
            }
            CheckedRecordPatternSourceKind::Binding(binding) => {
                CheckedRecordPatternSourceRef::Binding(binding)
            }
        }
    }

    pub const fn raw_pattern(&self) -> Option<PatternId> {
        match &self.kind {
            CheckedRecordPatternSourceKind::Pattern { raw, .. } => Some(*raw),
            CheckedRecordPatternSourceKind::Binding(_) => None,
        }
    }

    pub const fn binding(&self) -> Option<&CheckedRecordBindingSource> {
        match &self.kind {
            CheckedRecordPatternSourceKind::Pattern { .. } => None,
            CheckedRecordPatternSourceKind::Binding(binding) => Some(binding),
        }
    }

    pub(crate) const fn pattern_coordinate(&self) -> Option<&StablePatternCoordinate> {
        match &self.kind {
            CheckedRecordPatternSourceKind::Pattern { coordinate, .. } => Some(coordinate),
            CheckedRecordPatternSourceKind::Binding(_) => None,
        }
    }
}

/// One source-ordered field in a checked record pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRecordPatternField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    runtime_field: Option<RuntimeRecordFieldId>,
    semantic_id: CheckedRecordFieldSemanticId,
    field_type: TypeKind,
    field_type_digest: SemanticTypeDigest,
    source: CheckedRecordPatternSource,
}

impl CheckedRecordPatternField {
    pub(crate) fn new(
        source_ordinal: u32,
        declaration_ordinal: u32,
        runtime_field: Option<RuntimeRecordFieldId>,
        semantic_id: CheckedRecordFieldSemanticId,
        field_type: TypeKind,
        source: CheckedRecordPatternSource,
    ) -> Self {
        let field_type_digest = field_type.semantic_identity_digest();
        Self {
            source_ordinal,
            declaration_ordinal,
            runtime_field,
            semantic_id,
            field_type,
            field_type_digest,
            source,
        }
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub const fn runtime_field(&self) -> Option<RuntimeRecordFieldId> {
        self.runtime_field
    }

    pub(crate) const fn semantic_id(&self) -> CheckedRecordFieldSemanticId {
        self.semantic_id
    }

    pub const fn field_type(&self) -> &TypeKind {
        &self.field_type
    }

    pub const fn field_type_digest(&self) -> SemanticTypeDigest {
        self.field_type_digest
    }

    pub const fn source(&self) -> &CheckedRecordPatternSource {
        &self.source
    }
}

/// Exact rest disposition of one checked record pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedRecordPatternRest {
    Absent,
    Ignore,
    Binding(CheckedRecordBindingSource),
}

/// Complete checked semantic plan for one record pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRecordPattern {
    owner: CheckedRecordPatternOwner,
    fields: Box<[CheckedRecordPatternField]>,
    rest: CheckedRecordPatternRest,
}

impl CheckedRecordPattern {
    pub(crate) fn try_new(
        owner: CheckedRecordPatternOwner,
        fields: Box<[CheckedRecordPatternField]>,
        rest: CheckedRecordPatternRest,
    ) -> Option<Self> {
        let mut previous_source = None;
        let mut declaration_ordinals = BTreeSet::new();
        let mut runtime_fields = BTreeSet::new();
        let mut semantic_ids = BTreeSet::new();
        for field in &fields {
            let valid_owner_field = match (&owner, field.semantic_id(), field.runtime_field()) {
                (
                    CheckedRecordPatternOwner::Project { .. },
                    CheckedRecordFieldSemanticId::Project(_),
                    Some(runtime),
                ) => {
                    runtime.zero_based() == field.declaration_ordinal()
                        && runtime_fields.insert(runtime)
                }
                (
                    CheckedRecordPatternOwner::Environment { .. },
                    CheckedRecordFieldSemanticId::Environment(_),
                    None,
                ) => true,
                (
                    CheckedRecordPatternOwner::VariantPayload { payload, .. },
                    CheckedRecordFieldSemanticId::VariantPayload(semantic_id),
                    None,
                ) => usize::try_from(field.declaration_ordinal())
                    .ok()
                    .and_then(|ordinal| payload.shape().record_fields()?.get(ordinal))
                    .is_some_and(|expected| {
                        expected.ordinal() == field.declaration_ordinal()
                            && expected.semantic_id() == semantic_id
                            && expected.ty() == field.field_type()
                    }),
                _ => false,
            };
            if previous_source.is_some_and(|previous| previous >= field.source_ordinal())
                || !valid_owner_field
                || !declaration_ordinals.insert(field.declaration_ordinal())
                || !semantic_ids.insert(field.semantic_id())
                || field.field_type_digest() != field.field_type().semantic_identity_digest()
            {
                return None;
            }
            if let Some(coordinate) = field.source().pattern_coordinate()
                && !matches!(
                    coordinate.steps(),
                    [crate::semantic_coordinate::StablePatternCoordinateStep::RecordField {
                        field: coordinate_field,
                        source_ordinal,
                    }] if *coordinate_field == field.semantic_id()
                        && *source_ordinal == field.source_ordinal()
                )
            {
                return None;
            }
            previous_source = Some(field.source_ordinal());
        }
        let field_count = usize::try_from(owner.field_count()).ok()?;
        if fields.len() > field_count
            || (matches!(rest, CheckedRecordPatternRest::Absent) && fields.len() != field_count)
        {
            return None;
        }
        Some(Self {
            owner,
            fields,
            rest,
        })
    }

    pub const fn owner(&self) -> &CheckedRecordPatternOwner {
        &self.owner
    }

    pub const fn fields(&self) -> &[CheckedRecordPatternField] {
        &self.fields
    }

    pub const fn has_rest(&self) -> bool {
        !matches!(self.rest, CheckedRecordPatternRest::Absent)
    }

    pub const fn rest(&self) -> &CheckedRecordPatternRest {
        &self.rest
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self.owner() {
            CheckedRecordPatternOwner::Project { nominal, .. } => nominal.visit_types(visitor)?,
            CheckedRecordPatternOwner::Environment { .. } => {}
            CheckedRecordPatternOwner::VariantPayload { payload, .. } => {
                visitor(&TypeKind::VariantPayload(Box::new(payload.clone())))?;
            }
        }
        for field in self.fields() {
            visitor(field.field_type())?;
        }
        Ok(())
    }
}
