use std::collections::BTreeSet;

use thiserror::Error;

use super::HirExecutableProjectView;
use crate::expr::HirTypeRootDisposition;
use crate::identity::{ExprId, TypeId};
use crate::module::HirModule;

/// Typed failure while closing expression-to-type root ownership for one
/// executable HIR project.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirExpressionTypeRootProjectionError {
    #[error("expression {expression:?} has a foreign type root {type_id:?}")]
    ForeignRoot { expression: ExprId, type_id: TypeId },
    #[error("expression {expression:?} has an unresolved type root {type_id:?}")]
    MissingRoot { expression: ExprId, type_id: TypeId },
    #[error("type {parent:?} has a foreign child type {child:?}")]
    ForeignChild { parent: TypeId, child: TypeId },
    #[error("type {parent:?} has an unresolved child type {child:?}")]
    MissingChild { parent: TypeId, child: TypeId },
    #[error("type {type_id:?} has a scope from a foreign HIR module")]
    TypeScopeMismatch { type_id: TypeId },
    #[error("type-root projection found a cycle through type {type_id:?}")]
    CyclicType { type_id: TypeId },
}

/// Complete transitive disposition projection for expression-owned type roots.
///
/// Disposition reachability is retained independently. A type with a runtime
/// use is runtime-bearing even when also used for semantic admission or callee
/// resolution. The projection is built once
/// from final HIR and can then be shared by runtime and semantic consumers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirExpressionTypeRootProjection {
    runtime_bearing: BTreeSet<TypeId>,
    semantic: BTreeSet<TypeId>,
    resolution_inputs: BTreeSet<TypeId>,
}

impl HirExpressionTypeRootProjection {
    /// Closes every expression type root and all same-module type children in
    /// the supplied executable project.
    pub fn from_project(
        project: HirExecutableProjectView<'_>,
    ) -> Result<Self, HirExpressionTypeRootProjectionError> {
        let mut state = TypeRootProjectionState::default();
        for (_, module) in project.modules() {
            for (expression, owner) in module.expressions() {
                for root in owner.kind().direct_type_roots() {
                    visit_type(
                        module,
                        root.type_id(),
                        root.disposition(),
                        TypeRootSource::Expression(expression),
                        &mut state,
                    )?;
                }
            }
        }
        Ok(state.projection)
    }

    /// Returns every type reached through a runtime-bearing root.
    pub fn runtime_bearing_types(&self) -> impl ExactSizeIterator<Item = TypeId> + '_ {
        self.runtime_bearing.iter().copied()
    }

    /// Returns every type reached through a semantic root, including dual-use
    /// types that are also runtime-bearing.
    pub fn semantic_types(&self) -> impl ExactSizeIterator<Item = TypeId> + '_ {
        self.semantic.iter().copied()
    }

    pub fn contains_runtime_bearing(&self, type_id: TypeId) -> bool {
        self.runtime_bearing.contains(&type_id)
    }

    pub fn contains_semantic(&self, type_id: TypeId) -> bool {
        self.semantic.contains(&type_id)
    }

    pub fn is_semantic_only(&self, type_id: TypeId) -> bool {
        self.contains_semantic(type_id) && !self.contains_runtime_bearing(type_id)
    }

    /// Whether all expression-rooted uses are consumed before runtime typing.
    pub fn is_non_runtime(&self, type_id: TypeId) -> bool {
        (self.contains_semantic(type_id) || self.resolution_inputs.contains(&type_id))
            && !self.contains_runtime_bearing(type_id)
    }

    pub fn disposition(&self, type_id: TypeId) -> Option<HirTypeRootDisposition> {
        if self.contains_runtime_bearing(type_id) {
            Some(HirTypeRootDisposition::RuntimeBearing)
        } else if self.contains_semantic(type_id) {
            Some(HirTypeRootDisposition::SemanticOnly)
        } else if self.resolution_inputs.contains(&type_id) {
            Some(HirTypeRootDisposition::ResolutionInput)
        } else {
            None
        }
    }
}

impl HirExecutableProjectView<'_> {
    /// Builds the one shared transitive expression-to-type disposition
    /// projection for this exact executable HIR generation.
    pub fn type_root_projection(
        self,
    ) -> Result<HirExpressionTypeRootProjection, HirExpressionTypeRootProjectionError> {
        HirExpressionTypeRootProjection::from_project(self)
    }
}

#[derive(Clone, Copy)]
enum TypeRootSource {
    Expression(ExprId),
    Type(TypeId),
}

#[derive(Default)]
struct TypeRootProjectionState {
    projection: HirExpressionTypeRootProjection,
    seen: BTreeSet<(HirTypeRootDisposition, TypeId)>,
    visiting: BTreeSet<TypeId>,
}

fn visit_type(
    module: &HirModule,
    type_id: TypeId,
    disposition: HirTypeRootDisposition,
    source: TypeRootSource,
    state: &mut TypeRootProjectionState,
) -> Result<(), HirExpressionTypeRootProjectionError> {
    if state.seen.contains(&(disposition, type_id)) {
        return Ok(());
    }
    if !state.visiting.insert(type_id) {
        return Err(HirExpressionTypeRootProjectionError::CyclicType { type_id });
    }
    if type_id.module() != module.module_id() {
        state.visiting.remove(&type_id);
        return Err(match source {
            TypeRootSource::Expression(expression) => {
                HirExpressionTypeRootProjectionError::ForeignRoot {
                    expression,
                    type_id,
                }
            }
            TypeRootSource::Type(parent) => HirExpressionTypeRootProjectionError::ForeignChild {
                parent,
                child: type_id,
            },
        });
    }
    let ty = module.resolve_type(type_id).map_err(|_| {
        let error = match source {
            TypeRootSource::Expression(expression) => {
                HirExpressionTypeRootProjectionError::MissingRoot {
                    expression,
                    type_id,
                }
            }
            TypeRootSource::Type(parent) => HirExpressionTypeRootProjectionError::MissingChild {
                parent,
                child: type_id,
            },
        };
        state.visiting.remove(&type_id);
        error
    })?;
    if ty.scope().module() != module.module_id() {
        state.visiting.remove(&type_id);
        return Err(HirExpressionTypeRootProjectionError::TypeScopeMismatch { type_id });
    }
    match disposition {
        HirTypeRootDisposition::RuntimeBearing => {
            state.projection.runtime_bearing.insert(type_id);
        }
        HirTypeRootDisposition::SemanticOnly => {
            state.projection.semantic.insert(type_id);
        }
        HirTypeRootDisposition::ResolutionInput => {
            state.projection.resolution_inputs.insert(type_id);
        }
    }
    for child in ty.kind().direct_type_children() {
        if child.module() != module.module_id() {
            state.visiting.remove(&type_id);
            return Err(HirExpressionTypeRootProjectionError::ForeignChild {
                parent: type_id,
                child,
            });
        }
        visit_type(
            module,
            child,
            disposition,
            TypeRootSource::Type(type_id),
            state,
        )?;
    }
    state.visiting.remove(&type_id);
    state.seen.insert((disposition, type_id));
    Ok(())
}

#[cfg(test)]
#[path = "type_roots/tests.rs"]
mod tests;
