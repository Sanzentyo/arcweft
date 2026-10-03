//! Borrowed views of the selected executable's existing type authority.

use crate::{
    awbc::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId},
    pattern::RuntimeSemanticTypeId,
    plan::{RuntimeFunctionTypeContract, RuntimePlan, RuntimePlanTypeProjection, RuntimeTypeScope},
    runtime_id::RuntimePlanTypeId,
    task::RuntimeProgramOwner,
    value::{RuntimeCallableValue, RuntimeValueView},
};

pub(crate) trait FunctionTypeAuthority {
    type Type: Copy + Eq;
    fn scope(&self, ty: Self::Type) -> Option<&RuntimeTypeScope>;
    fn semantic(&self, ty: Self::Type) -> Option<RuntimeSemanticTypeId>;
    fn by_semantic(&self, ty: RuntimeSemanticTypeId) -> Option<Self::Type>;
    fn function(
        &self,
        ty: Self::Type,
    ) -> Option<(&RuntimeFunctionTypeContract, &[Self::Type], Self::Type)>;
    fn tuple(&self, ty: Self::Type) -> Option<&[Self::Type]>;
    fn record_field(
        &self,
        ty: Self::Type,
        header: &crate::value::RuntimePlaceRecordHeader,
        count: usize,
        ordinal: usize,
    ) -> Option<Self::Type>;
    fn is_unit(&self, ty: Self::Type) -> bool;
    fn compatible(&self, expected: Self::Type, actual: Self::Type) -> bool;
    fn value_matches(
        &self,
        expected: Self::Type,
        value: RuntimeValueView<'_>,
        depth: usize,
    ) -> bool;
    fn callable_type(&self, value: &RuntimeCallableValue) -> Option<Self::Type>;
}

impl FunctionTypeAuthority for AwbcProgram {
    type Type = AwbcTypeId;
    fn scope(&self, ty: Self::Type) -> Option<&RuntimeTypeScope> {
        self.runtime_types.get(ty.index()).map(|row| row.scope())
    }
    fn semantic(&self, ty: Self::Type) -> Option<RuntimeSemanticTypeId> {
        self.runtime_types
            .get(ty.index())
            .map(|row| row.semantic_identity())
    }
    fn by_semantic(&self, ty: RuntimeSemanticTypeId) -> Option<Self::Type> {
        self.semantic_type_id(ty)
    }
    fn function(
        &self,
        ty: Self::Type,
    ) -> Option<(&RuntimeFunctionTypeContract, &[Self::Type], Self::Type)> {
        let AwbcRuntimeTypeShape::Function {
            contract,
            parameters,
            result,
        } = self.runtime_types.get(ty.index())?.shape()
        else {
            return None;
        };
        Some((contract, parameters, *result))
    }
    fn tuple(&self, ty: Self::Type) -> Option<&[Self::Type]> {
        let AwbcRuntimeTypeShape::Tuple(items) = self.runtime_types.get(ty.index())?.shape() else {
            return None;
        };
        Some(items)
    }
    fn is_unit(&self, ty: Self::Type) -> bool {
        self.runtime_types
            .get(ty.index())
            .is_some_and(|row| matches!(row.shape(), AwbcRuntimeTypeShape::Unit))
    }
    fn record_field(
        &self,
        ty: Self::Type,
        header: &crate::value::RuntimePlaceRecordHeader,
        count: usize,
        ordinal: usize,
    ) -> Option<Self::Type> {
        use crate::value::RuntimePlaceRecordHeader as Header;
        let row = self.runtime_types.get(ty.index())?;
        let fields = match (header, row.shape()) {
            (
                Header::Nominal {
                    nominal,
                    semantic_identity,
                    layout,
                },
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id,
                    layout: expected_layout,
                    fields,
                    ..
                },
            ) if self.strings.get(public_id.index())?.as_str() == nominal.as_str()
                && *semantic_identity == row.semantic_identity()
                && *layout == crate::entry::TypeLayoutHash::from_bytes(*expected_layout) =>
            {
                fields
            }
            (Header::Structural { names }, AwbcRuntimeTypeShape::Record { fields, .. })
                if names.len() == count
                    && fields
                        .get(ordinal)?
                        .name
                        .and_then(|name| self.strings.get(name.index()))
                        == names.get(ordinal) =>
            {
                fields
            }
            _ => return None,
        };
        let field = fields.get(ordinal)?;
        (fields.len() == count && field.field.zero_based() as usize == ordinal).then_some(field.ty)
    }
    fn compatible(&self, expected: Self::Type, actual: Self::Type) -> bool {
        self.types_compatible(expected, actual)
    }
    fn value_matches(
        &self,
        expected: Self::Type,
        value: RuntimeValueView<'_>,
        depth: usize,
    ) -> bool {
        crate::awbc::vm::runtime_value_view_matches_type(self, value, expected, depth)
    }
    fn callable_type(&self, value: &RuntimeCallableValue) -> Option<Self::Type> {
        if !matches!(value.owner(), RuntimeProgramOwner::Awbc(owner) if std::ptr::eq(owner.as_ref(), self))
            || value.validate_retained().is_err()
        {
            return None;
        }
        self.by_semantic(value.function_type().ok()?)
    }
}

impl FunctionTypeAuthority for RuntimePlan {
    type Type = RuntimePlanTypeId;
    fn scope(&self, ty: Self::Type) -> Option<&RuntimeTypeScope> {
        self.type_table().get(ty).map(|row| row.scope())
    }
    fn semantic(&self, ty: Self::Type) -> Option<RuntimeSemanticTypeId> {
        self.type_table().get(ty).map(|row| row.semantic_identity())
    }
    fn by_semantic(&self, ty: RuntimeSemanticTypeId) -> Option<Self::Type> {
        self.type_table().id_for_semantic(ty)
    }
    fn function(
        &self,
        ty: Self::Type,
    ) -> Option<(&RuntimeFunctionTypeContract, &[Self::Type], Self::Type)> {
        let RuntimePlanTypeProjection::Function {
            contract,
            parameters,
            result,
        } = self.type_table().get(ty)?.projection()
        else {
            return None;
        };
        Some((contract, parameters, *result))
    }
    fn tuple(&self, ty: Self::Type) -> Option<&[Self::Type]> {
        let RuntimePlanTypeProjection::Tuple(items) = self.type_table().get(ty)?.projection()
        else {
            return None;
        };
        Some(items)
    }
    fn is_unit(&self, ty: Self::Type) -> bool {
        self.type_table()
            .get(ty)
            .is_some_and(|row| matches!(row.projection(), RuntimePlanTypeProjection::Unit))
    }
    fn record_field(
        &self,
        ty: Self::Type,
        header: &crate::value::RuntimePlaceRecordHeader,
        count: usize,
        ordinal: usize,
    ) -> Option<Self::Type> {
        use crate::value::RuntimePlaceRecordHeader as Header;
        let row = self.type_table().get(ty)?;
        match (header, row.projection()) {
            (
                Header::Nominal {
                    nominal,
                    semantic_identity,
                    layout,
                },
                RuntimePlanTypeProjection::Nominal {
                    nominal: expected_nominal,
                    layout: expected_layout,
                    ..
                },
            ) if nominal == expected_nominal
                && *semantic_identity == row.semantic_identity()
                && layout == expected_layout =>
            {
                let fields = self.nominal_record_domains().get(ty)?.fields();
                let field = fields.get(ordinal)?;
                (fields.len() == count && field.field().zero_based() as usize == ordinal)
                    .then_some(field.ty())
            }
            (Header::Structural { names }, RuntimePlanTypeProjection::Record(fields))
                if names.len() == count
                    && fields.len() == count
                    && names.get(ordinal)?.as_str() == fields.get(ordinal)?.diagnostic_name() =>
            {
                Some(*fields[ordinal].ty())
            }
            _ => None,
        }
    }
    fn compatible(&self, expected: Self::Type, actual: Self::Type) -> bool {
        expected == actual
    }
    fn value_matches(&self, expected: Self::Type, value: RuntimeValueView<'_>, _: usize) -> bool {
        self.validate_live_value_view(
            expected,
            value,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
        .is_ok()
    }
    fn callable_type(&self, value: &RuntimeCallableValue) -> Option<Self::Type> {
        if !matches!(value.owner(), RuntimeProgramOwner::Plan(owner) if std::ptr::eq(owner.as_ref(), self))
            || value.validate_retained().is_err()
        {
            return None;
        }
        self.by_semantic(value.function_type().ok()?)
    }
}
