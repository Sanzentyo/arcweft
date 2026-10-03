//! Borrowed views of the selected executable's existing type authority.

use crate::{
    awbc::schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId},
    pattern::RuntimeSemanticTypeId,
    plan::{RuntimeFunctionTypeContract, RuntimePlan, RuntimePlanTypeProjection, RuntimeTypeScope},
    runtime_id::RuntimePlanTypeId,
    value::RuntimeValueView,
};

/// Typed relation context for the existing value visitor. It does not own rows.
pub(crate) trait RuntimeValueTypeContext<T: Copy + Eq> {
    fn permits_scope(&self, scope: &RuntimeTypeScope) -> bool;
    fn callable(&mut self, expected: T, actual: T) -> bool;
    fn nominal(&mut self, expected: T, actual: T) -> bool {
        expected == actual
    }
    fn begin_choice(&mut self) {}
    fn begin_alternative(&mut self) {}
    fn finish_alternative(&mut self, _accepted: bool) {}
    fn finish_choice(&mut self) {}
}

impl<T: Copy + Eq> RuntimeValueTypeContext<T> for () {
    fn permits_scope(&self, scope: &RuntimeTypeScope) -> bool {
        scope.is_root()
    }
    fn callable(&mut self, expected: T, actual: T) -> bool {
        expected == actual
    }
}

impl<T: Copy + Eq, C: RuntimeValueTypeContext<T>> RuntimeValueTypeContext<T> for &mut C {
    fn permits_scope(&self, scope: &RuntimeTypeScope) -> bool {
        (**self).permits_scope(scope)
    }
    fn callable(&mut self, expected: T, actual: T) -> bool {
        (**self).callable(expected, actual)
    }
    fn nominal(&mut self, expected: T, actual: T) -> bool {
        (**self).nominal(expected, actual)
    }
    fn begin_choice(&mut self) {
        (**self).begin_choice();
    }
    fn begin_alternative(&mut self) {
        (**self).begin_alternative();
    }
    fn finish_alternative(&mut self, accepted: bool) {
        (**self).finish_alternative(accepted);
    }
    fn finish_choice(&mut self) {
        (**self).finish_choice();
    }
}

pub(crate) trait FunctionTypeAuthority {
    type Type: Copy + Eq;
    fn scope(&self, ty: Self::Type) -> Option<&RuntimeTypeScope>;
    fn semantic(&self, ty: Self::Type) -> Option<RuntimeSemanticTypeId>;
    fn by_semantic(&self, ty: RuntimeSemanticTypeId) -> Option<Self::Type>;
    fn nominal_arguments(
        &self,
        expected: Self::Type,
        actual: Self::Type,
    ) -> Option<Result<(&[Self::Type], &[Self::Type]), ()>>;
    fn function(
        &self,
        ty: Self::Type,
    ) -> Option<(&RuntimeFunctionTypeContract, &[Self::Type], Self::Type)>;
    fn relate_children(
        &self,
        expected: Self::Type,
        actual: Self::Type,
        visit: &mut impl FnMut(Self::Type, Self::Type) -> Result<(), ()>,
    ) -> Option<Result<(), ()>>;
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
    fn value_relation<C: RuntimeValueTypeContext<Self::Type>>(
        &self,
        expected: Self::Type,
        value: RuntimeValueView<'_>,
        context: &mut C,
    ) -> bool;
}

impl FunctionTypeAuthority for AwbcProgram {
    type Type = AwbcTypeId;
    fn nominal_arguments(
        &self,
        expected: Self::Type,
        actual: Self::Type,
    ) -> Option<Result<(&[Self::Type], &[Self::Type]), ()>> {
        let expected = self.runtime_types.get(expected.index())?;
        let actual = self.runtime_types.get(actual.index())?;
        let (Some(a), Some(b)) = (expected.nominal_declaration(), actual.nominal_declaration())
        else {
            return None;
        };
        use AwbcRuntimeTypeShape as T;
        Some(match (expected.shape(), actual.shape()) {
            (
                T::NominalRecord {
                    arguments: x,
                    shape: p,
                    ..
                },
                T::NominalRecord {
                    arguments: y,
                    shape: q,
                    ..
                },
            ) if a == b && p == q => Ok((x, y)),
            (T::Variant { arguments: x, .. }, T::Variant { arguments: y, .. }) if a == b => {
                Ok((x, y))
            }
            _ => Err(()),
        })
    }
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
    fn relate_children(
        &self,
        expected: Self::Type,
        actual: Self::Type,
        visit: &mut impl FnMut(Self::Type, Self::Type) -> Result<(), ()>,
    ) -> Option<Result<(), ()>> {
        use AwbcRuntimeTypeShape as T;
        let expected = self.runtime_types.get(expected.index())?.shape();
        let actual = self.runtime_types.get(actual.index())?.shape();
        Some(match (expected, actual) {
            (T::Tuple(a), T::Tuple(b)) => relate_slices(a, b, visit),
            (
                T::Variant {
                    owner: a,
                    arguments: x,
                    cases: p,
                },
                T::Variant {
                    owner: b,
                    arguments: y,
                    cases: q,
                },
            ) if a == b && p.len() == q.len() => relate_slices(x, y, visit).and_then(|()| {
                p.iter().zip(q).try_for_each(|(p, q)| {
                    if p.name != q.name {
                        return Err(());
                    }
                    match (p.payload, q.payload) {
                        (Some(p), Some(q)) => visit(p, q),
                        (None, None) => Ok(()),
                        _ => Err(()),
                    }
                })
            }),
            (T::Sequence { kind: a, item: x }, T::Sequence { kind: b, item: y }) if a == b => {
                visit(*x, *y)
            }
            (T::Array { length: a, item: x }, T::Array { length: b, item: y }) if a == b => {
                visit(*x, *y)
            }
            (
                T::Map {
                    kind: a,
                    key: x,
                    value: u,
                },
                T::Map {
                    kind: b,
                    key: y,
                    value: v,
                },
            ) if a == b => visit(*x, *y).and_then(|()| visit(*u, *v)),
            (T::Stream { item: x, error: u }, T::Stream { item: y, error: v }) => {
                visit(*x, *y).and_then(|()| visit(*u, *v))
            }
            (T::Range(x), T::Range(y))
            | (T::Iterator(x), T::Iterator(y))
            | (T::Need(x), T::Need(y))
            | (T::Task(x), T::Task(y))
            | (T::Shared(x), T::Shared(y))
            | (T::Reference(x), T::Reference(y)) => visit(*x, *y),
            (
                T::Record {
                    public_id: a,
                    fields: x,
                },
                T::Record {
                    public_id: b,
                    fields: y,
                },
            ) if a == b && x.len() == y.len() => x.iter().zip(y).try_for_each(|(x, y)| {
                if x.field != y.field || x.name != y.name {
                    return Err(());
                }
                visit(x.ty, y.ty)
            }),
            _ => return None,
        })
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
    fn value_relation<C: RuntimeValueTypeContext<Self::Type>>(
        &self,
        expected: Self::Type,
        value: RuntimeValueView<'_>,
        context: &mut C,
    ) -> bool {
        self.validate_value_relation(expected, value, context)
            .is_ok()
    }
}

impl FunctionTypeAuthority for RuntimePlan {
    type Type = RuntimePlanTypeId;
    fn nominal_arguments(
        &self,
        expected: Self::Type,
        actual: Self::Type,
    ) -> Option<Result<(&[Self::Type], &[Self::Type]), ()>> {
        let x = self.type_table().get(expected)?;
        let y = self.type_table().get(actual)?;
        let (Some(a), Some(b)) = (x.nominal_declaration(), y.nominal_declaration()) else {
            return None;
        };
        let (
            RuntimePlanTypeProjection::Nominal { arguments: x, .. },
            RuntimePlanTypeProjection::Nominal { arguments: y, .. },
        ) = (x.projection(), y.projection())
        else {
            return Some(Err(()));
        };
        let same_kind = match (
            self.nominal_record_domains().get(expected),
            self.nominal_record_domains().get(actual),
        ) {
            (Some(p), Some(q)) => p.shape() == q.shape(),
            (None, None) => {
                self.variant_domains().get(expected).is_some()
                    && self.variant_domains().get(actual).is_some()
            }
            _ => false,
        };
        Some(if a == b && same_kind {
            Ok((x, y))
        } else {
            Err(())
        })
    }
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
    fn relate_children(
        &self,
        expected: Self::Type,
        actual: Self::Type,
        visit: &mut impl FnMut(Self::Type, Self::Type) -> Result<(), ()>,
    ) -> Option<Result<(), ()>> {
        use RuntimePlanTypeProjection as T;
        let expected = self.type_table().get(expected)?.projection();
        let actual = self.type_table().get(actual)?.projection();
        Some(match (expected, actual) {
            (T::Tuple(a), T::Tuple(b)) => relate_slices(a, b, visit),
            (
                T::BuiltinVariant { owner: a, cases: x },
                T::BuiltinVariant { owner: b, cases: y },
            ) if a == b && x.len() == y.len() => {
                x.iter().zip(y).try_for_each(|(x, y)| match (x, y) {
                    (Some(x), Some(y)) => visit(*x, *y),
                    (None, None) => Ok(()),
                    _ => Err(()),
                })
            }
            (T::Sequence { kind: a, item: x }, T::Sequence { kind: b, item: y }) if a == b => {
                visit(*x, *y)
            }
            (T::Array { length: a, item: x }, T::Array { length: b, item: y }) if a == b => {
                visit(*x, *y)
            }
            (
                T::Map {
                    kind: a,
                    key: x,
                    value: u,
                },
                T::Map {
                    kind: b,
                    key: y,
                    value: v,
                },
            ) if a == b => visit(*x, *y).and_then(|()| visit(*u, *v)),
            (T::Stream { item: x, error: u }, T::Stream { item: y, error: v }) => {
                visit(*x, *y).and_then(|()| visit(*u, *v))
            }
            (T::Range(x), T::Range(y))
            | (T::Iterator(x), T::Iterator(y))
            | (T::Need(x), T::Need(y))
            | (T::ThreadHandle(x), T::ThreadHandle(y))
            | (T::Shared(x), T::Shared(y))
            | (T::Reference(x), T::Reference(y)) => visit(*x, *y),
            (
                T::Option {
                    item: x,
                    some_payload: u,
                },
                T::Option {
                    item: y,
                    some_payload: v,
                },
            ) => visit(*x, *y).and_then(|()| visit(*u, *v)),
            (
                T::Result {
                    value: x,
                    error: u,
                    value_payload: p,
                    error_payload: q,
                },
                T::Result {
                    value: y,
                    error: v,
                    value_payload: r,
                    error_payload: s,
                },
            ) => visit(*x, *y)
                .and_then(|()| visit(*u, *v))
                .and_then(|()| visit(*p, *r))
                .and_then(|()| visit(*q, *s)),
            (T::Record(a), T::Record(b)) if a.len() == b.len() => {
                a.iter().zip(b).try_for_each(|(a, b)| {
                    if a.diagnostic_name() != b.diagnostic_name() {
                        return Err(());
                    }
                    visit(*a.ty(), *b.ty())
                })
            }
            _ => return None,
        })
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
    fn value_relation<C: RuntimeValueTypeContext<Self::Type>>(
        &self,
        expected: Self::Type,
        value: RuntimeValueView<'_>,
        context: &mut C,
    ) -> bool {
        self.validate_value_relation(expected, value, context)
            .is_ok()
    }
}

fn relate_slices<T: Copy>(
    a: &[T],
    b: &[T],
    visit: &mut impl FnMut(T, T) -> Result<(), ()>,
) -> Result<(), ()> {
    if a.len() != b.len() {
        return Err(());
    }
    a.iter().zip(b).try_for_each(|(a, b)| visit(*a, *b))
}
