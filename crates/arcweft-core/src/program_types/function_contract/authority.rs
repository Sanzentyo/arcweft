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
