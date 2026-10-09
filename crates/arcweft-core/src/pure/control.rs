//! Borrowed physical lexical projection for an already admitted FunctionSite.
//! Numeric registers stay backend-owned; Unit is a typed zero-size value/place,
//! never a numeric sentinel. This projection does not issue execution authority.
use super::RuntimePureFunctionRef;
use crate::pattern::{RuntimePattern, RuntimePatternKind};
use crate::plan::RuntimePlanTypeProjection;
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::scope::{RuntimeScopeExitTarget, RuntimeScopeFrameKind};
use crate::value::{
    RuntimeEvalError, RuntimeExpr, RuntimeExprKind, RuntimeLocalReadMode, RuntimeValue,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimePureUnitSource {
    ty: RuntimePlanTypeId,
    kind: UnitSource,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum UnitSource {
    Literal,
    Local {
        local: RuntimeLocalDeclarationId,
        mode: RuntimeLocalReadMode,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimePureUnitValue {
    ty: RuntimePlanTypeId,
    value: (),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimePureUnitDestination {
    ty: RuntimePlanTypeId,
    local: Option<RuntimeLocalDeclarationId>,
}

#[derive(Clone, Debug)]
pub struct RuntimePureControlBindings<'a, T> {
    function: RuntimePureFunctionRef<'a>,
    numeric: T,
    units: BTreeMap<RuntimeLocalDeclarationId, UnitPlace>,
    scopes: Vec<PhysicalScope<T>>,
    next_scope: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnitPlace {
    Initialized(RuntimePlanTypeId),
    Vacant(RuntimePlanTypeId),
}
#[derive(Clone, Debug)]
struct PhysicalScope<T> {
    id: usize,
    kind: RuntimeScopeFrameKind,
    numeric: T,
    units: BTreeMap<RuntimeLocalDeclarationId, UnitPlace>,
}
impl<'a, T: Clone> RuntimePureControlBindings<'a, T> {
    pub fn new(function: RuntimePureFunctionRef<'a>, numeric: T) -> Self {
        Self {
            function,
            numeric,
            units: BTreeMap::new(),
            scopes: Vec::new(),
            next_scope: 0,
        }
    }
    pub fn numeric(&self) -> &T {
        &self.numeric
    }
    pub fn numeric_mut(&mut self) -> &mut T {
        &mut self.numeric
    }
    pub fn enter_scope(&mut self, kind: RuntimeScopeFrameKind) -> usize {
        let id = self.next_scope;
        self.next_scope = self
            .next_scope
            .checked_add(1)
            .expect("the admitted finite physical body bounds its lexical scopes");
        self.scopes.push(PhysicalScope {
            id,
            kind,
            numeric: self.numeric.clone(),
            units: self.units.clone(),
        });
        id
    }
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
    pub fn contains_scope(&self, id: usize) -> bool {
        self.scopes.iter().any(|scope| scope.id == id)
    }
    pub fn exit_scope(
        &mut self,
        target: RuntimeScopeExitTarget<usize>,
    ) -> Result<(), RuntimeEvalError> {
        let exit = target
            .resolve(self.scopes.iter().rev().map(|scope| (scope.id, scope.kind)))
            .map_err(|error| self.unsupported(error.to_string()))?;
        for _ in exit.targets() {
            let scope = self
                .scopes
                .pop()
                .expect("a resolved exit retains its frames");
            self.numeric = scope.numeric;
            self.units = scope.units;
        }
        Ok(())
    }
    pub fn compatible_after_branch(&self, other: &Self) -> bool {
        self.units == other.units
            && self.scopes.len() == other.scopes.len()
            && self.scopes.iter().zip(&other.scopes).all(|(left, right)| {
                left.id == right.id && left.kind == right.kind && left.units == right.units
            })
    }
    pub fn finish_return(&mut self) {
        while let Some(scope) = self.scopes.pop() {
            self.numeric = scope.numeric;
            self.units = scope.units
        }
    }
    fn unsupported(&self, reason: String) -> RuntimeEvalError {
        RuntimeEvalError::UnsupportedPure {
            name: self.function.name.to_owned(),
            reason,
        }
    }
    fn is_unit_type(&self, ty: RuntimePlanTypeId) -> bool {
        self.function
            .plan()
            .type_table()
            .get(ty)
            .is_some_and(|row| matches!(row.projection(), RuntimePlanTypeProjection::Unit))
    }
    pub fn unit_source(
        &self,
        value: &RuntimeExpr,
    ) -> Result<Option<RuntimePureUnitSource>, RuntimeEvalError> {
        if !self.is_unit_type(value.ty()) {
            return Ok(None);
        }
        let kind = match value.kind() {
            RuntimeExprKind::Value(RuntimeValue::Unit) => UnitSource::Literal,
            RuntimeExprKind::Local(read) if read.fields().is_empty() => UnitSource::Local {
                local: read.local(),
                mode: read.mode(),
            },
            _ => {
                return Err(self.unsupported(
                    "Unit control operand is outside the admitted physical value projection".into(),
                ));
            }
        };
        Ok(Some(RuntimePureUnitSource {
            ty: value.ty(),
            kind,
        }))
    }
    pub fn read_unit(
        &mut self,
        source: RuntimePureUnitSource,
    ) -> Result<RuntimePureUnitValue, RuntimeEvalError> {
        if !self.is_unit_type(source.ty) {
            return Err(RuntimeEvalError::InvalidExpressionType(source.ty));
        }
        if let UnitSource::Local { local, mode } = source.kind {
            if self.units.get(&local) != Some(&UnitPlace::Initialized(source.ty)) {
                return Err(RuntimeEvalError::UninitializedLocal(local));
            }
            if mode == RuntimeLocalReadMode::Move {
                self.units.insert(local, UnitPlace::Vacant(source.ty));
                for scope in &mut self.scopes {
                    if scope.units.contains_key(&local) {
                        scope.units.insert(local, UnitPlace::Vacant(source.ty));
                    }
                }
            }
        }
        Ok(RuntimePureUnitValue {
            ty: source.ty,
            value: (),
        })
    }
    pub fn evaluate_unit(
        &mut self,
        value: &RuntimeExpr,
    ) -> Result<Option<RuntimePureUnitValue>, RuntimeEvalError> {
        let Some(source) = self.unit_source(value)? else {
            return Ok(None);
        };
        self.read_unit(source).map(Some)
    }
    pub fn unit_destination(
        &self,
        pattern: &RuntimePattern,
    ) -> Result<RuntimePureUnitDestination, RuntimeEvalError> {
        if !self.is_unit_type(pattern.ty()) {
            return Err(RuntimeEvalError::InvalidExpressionType(pattern.ty()));
        }
        let local = match pattern.kind() {
            RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
                Some(binding.local())
            }
            RuntimePatternKind::Discard => None,
            _ => {
                return Err(
                    self.unsupported("Unit result has an unsupported binding pattern".into())
                );
            }
        };
        Ok(RuntimePureUnitDestination {
            ty: pattern.ty(),
            local,
        })
    }
    pub fn publish_unit(
        &mut self,
        destination: RuntimePureUnitDestination,
        value: RuntimePureUnitValue,
    ) -> Result<(), RuntimeEvalError> {
        let RuntimePureUnitValue {
            ty: value_ty,
            value: (),
        } = value;
        if destination.ty != value_ty || !self.is_unit_type(destination.ty) {
            return Err(RuntimeEvalError::InvalidExpressionType(destination.ty));
        }
        if let Some(local) = destination.local {
            self.units
                .insert(local, UnitPlace::Initialized(destination.ty));
            for scope in &mut self.scopes {
                if scope.units.contains_key(&local) {
                    scope
                        .units
                        .insert(local, UnitPlace::Initialized(destination.ty));
                }
            }
        }
        Ok(())
    }
    pub fn bind_unit(
        &mut self,
        pattern: &RuntimePattern,
        value: RuntimePureUnitValue,
    ) -> Result<RuntimePureUnitDestination, RuntimeEvalError> {
        let destination = self.unit_destination(pattern)?;
        self.publish_unit(destination, value)?;
        Ok(destination)
    }
}

impl<'a, V: Clone> RuntimePureControlBindings<'a, BTreeMap<RuntimeLocalDeclarationId, V>> {
    /// Checks physical numeric place reads in source evaluation order. Hidden
    /// or dynamically conditional Move operands retain native control; no
    /// evaluator, scalar value, source body or execution capability is created.
    pub fn consume_numeric_expression(
        &mut self,
        value: &RuntimeExpr,
    ) -> Result<(), RuntimeEvalError> {
        match value.kind() {
            RuntimeExprKind::Value(_) => Ok(()),
            RuntimeExprKind::Local(read) => {
                if !read.fields().is_empty() {
                    return Err(self.unsupported(
                        "scalar field read is outside physical place projection".into(),
                    ));
                }
                if !self.numeric.contains_key(&read.local()) {
                    return Err(RuntimeEvalError::UninitializedLocal(read.local()));
                }
                if read.mode() == RuntimeLocalReadMode::Move {
                    self.numeric.remove(&read.local());
                    for scope in &mut self.scopes {
                        scope.numeric.remove(&read.local());
                    }
                }
                Ok(())
            }
            RuntimeExprKind::Unary { expr, .. } => self.consume_numeric_expression(expr),
            RuntimeExprKind::Binary { lhs, rhs, .. } => {
                self.consume_numeric_expression(lhs)?;
                self.consume_numeric_expression(rhs)
            }
            RuntimeExprKind::Call { args, .. } => {
                for argument in args {
                    self.consume_numeric_expression(argument.value())?;
                }
                Ok(())
            }
            // Expression-owned lexical/conditional evaluation remains in its
            // current Copy-only physical subset. Structured control above
            // carries complete Move availability through its actual frames.
            _ => {
                let reads = value
                    .evaluation_free_local_reads(self.function.plan())
                    .map_err(|error| self.unsupported(error.to_string()))?;
                if reads
                    .iter()
                    .any(|(_, mode)| *mode == RuntimeLocalReadMode::Move)
                {
                    return Err(self.unsupported(
                        "conditional/expression-local Move requires native control".into(),
                    ));
                }
                for (local, _) in reads {
                    if !self.numeric.contains_key(&local) {
                        return Err(RuntimeEvalError::UninitializedLocal(local));
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests;
