//! Static binding declarations of an admitted pattern, in declaration order.

use super::{RuntimePattern, RuntimePatternBindingCoordinate, RuntimePatternKind};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};

/// One declaration site borrowed from its actual admitted pattern.
/// Only `Bind` carries mutability in the admitted grammar; whole, typed,
/// and rest declarations are immutable by construction.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePatternBindingDeclaration<'pattern> {
    coordinate: &'pattern RuntimePatternBindingCoordinate,
    ty: RuntimePlanTypeId,
    mutable: bool,
}

impl<'pattern> RuntimePatternBindingDeclaration<'pattern> {
    #[must_use]
    pub const fn coordinate(self) -> &'pattern RuntimePatternBindingCoordinate {
        self.coordinate
    }

    #[must_use]
    pub const fn local(self) -> RuntimeLocalDeclarationId {
        self.coordinate.local()
    }

    #[must_use]
    pub const fn ty(self) -> RuntimePlanTypeId {
        self.ty
    }

    #[must_use]
    pub const fn is_mutable(self) -> bool {
        self.mutable
    }
}

enum Visit<'pattern> {
    Pattern(&'pattern RuntimePattern),
    Declaration(RuntimePatternBindingDeclaration<'pattern>),
}

/// Iterative traversal of the pattern's complete static declaration inventory.
/// Or alternatives share a checked declaration inventory, so only the first
/// alternative introduces its locals. Each alternative retains its own
/// binding coordinates for matching.
pub struct RuntimePatternBindingDeclarations<'pattern> {
    first: Option<&'pattern RuntimePattern>,
    pending: Vec<Visit<'pattern>>,
}

impl RuntimePattern {
    #[must_use]
    pub fn binding_declarations(&self) -> RuntimePatternBindingDeclarations<'_> {
        RuntimePatternBindingDeclarations {
            first: Some(self),
            pending: Vec::new(),
        }
    }
}

impl<'pattern> Iterator for RuntimePatternBindingDeclarations<'pattern> {
    type Item = RuntimePatternBindingDeclaration<'pattern>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(visit) = self
            .first
            .take()
            .map(Visit::Pattern)
            .or_else(|| self.pending.pop())
        {
            let pattern = match visit {
                Visit::Declaration(declaration) => return Some(declaration),
                Visit::Pattern(pattern) => pattern,
            };
            let declaration = |coordinate, mutable| RuntimePatternBindingDeclaration {
                coordinate,
                ty: pattern.ty(),
                mutable,
            };
            match pattern.kind() {
                RuntimePatternKind::Bind { mutable, binding } => {
                    return Some(declaration(binding, *mutable));
                }
                RuntimePatternKind::Typed { binding } => {
                    return Some(declaration(binding, false));
                }
                RuntimePatternKind::Whole {
                    binding,
                    pattern: child,
                } => {
                    self.pending.push(Visit::Pattern(child));
                    return Some(declaration(binding, false));
                }
                RuntimePatternKind::Discard
                | RuntimePatternKind::Literal(_)
                | RuntimePatternKind::Entity(_) => {}
                RuntimePatternKind::Or(alternatives) => {
                    if let Some(first) = alternatives.first() {
                        self.pending.push(Visit::Pattern(first));
                    }
                }
                RuntimePatternKind::Tuple(items) => {
                    self.pending.extend(items.iter().rev().map(Visit::Pattern));
                }
                RuntimePatternKind::Record { fields, rest } => {
                    if let Some(binding) = rest.binding() {
                        self.pending
                            .push(Visit::Declaration(declaration(binding, false)));
                    }
                    self.pending.extend(
                        fields
                            .iter()
                            .rev()
                            .map(|field| Visit::Pattern(field.pattern())),
                    );
                }
                RuntimePatternKind::Sequence { items, rest } => {
                    if let Some(binding) = rest.binding() {
                        self.pending
                            .push(Visit::Declaration(declaration(binding, false)));
                    }
                    self.pending.extend(items.iter().rev().map(Visit::Pattern));
                }
                RuntimePatternKind::Variant { payload, .. } => {
                    if let Some(payload) = payload {
                        self.pending.push(Visit::Pattern(payload));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
