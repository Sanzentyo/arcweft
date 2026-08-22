//! Typed direct-child authority for final HIR patterns.

use crate::identity::{LocalId, PatternId, TypeId};

use super::{
    HirPatternBinding, HirPatternChildRole, HirPatternField, HirPatternKind,
    HirPatternSequenceRest, HirVariantPatternPayload,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPatternChild {
    Pattern(PatternId),
    Type(TypeId),
    Local(LocalId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPatternChildEdge {
    child: HirPatternChild,
    role: HirPatternChildRole,
}

impl HirPatternChildEdge {
    const fn new(child: HirPatternChild, role: HirPatternChildRole) -> Self {
        Self { child, role }
    }

    pub const fn child(&self) -> HirPatternChild {
        self.child
    }

    pub const fn role(&self) -> HirPatternChildRole {
        self.role
    }
}

impl HirPatternKind {
    /// Returns every directly owned pattern/type/local child in semantic order.
    pub fn child_edges(&self) -> Vec<HirPatternChildEdge> {
        let mut edges = Vec::new();
        match self {
            Self::Binding(binding) => {
                push_binding(&mut edges, binding, HirPatternChildRole::BindingLocal);
            }
            Self::MutableBinding(binding) => {
                push_binding(
                    &mut edges,
                    binding,
                    HirPatternChildRole::MutableBindingLocal,
                );
            }
            Self::Variant(pattern) => match pattern.payload() {
                HirVariantPatternPayload::Pattern(pattern)
                | HirVariantPatternPayload::Recovered {
                    pattern: Some(pattern),
                    ..
                } => edges.push(HirPatternChildEdge::new(
                    HirPatternChild::Pattern(*pattern),
                    HirPatternChildRole::VariantPayload,
                )),
                HirVariantPatternPayload::Absent
                | HirVariantPatternPayload::Recovered { pattern: None, .. } => {}
            },
            Self::Tuple { elements } | Self::BracketSequence { elements, .. } => {
                for (ordinal, pattern) in elements.iter().enumerate() {
                    edges.push(HirPatternChildEdge::new(
                        HirPatternChild::Pattern(*pattern),
                        HirPatternChildRole::Element {
                            ordinal: checked_ordinal(ordinal),
                        },
                    ));
                }
                if let Self::BracketSequence {
                    rest: HirPatternSequenceRest::Bound(local),
                    ..
                } = self
                {
                    edges.push(HirPatternChildEdge::new(
                        HirPatternChild::Local(*local),
                        HirPatternChildRole::SequenceRestLocal,
                    ));
                }
            }
            Self::Record { fields, .. } => {
                for (field, payload) in fields.iter().enumerate() {
                    let field = checked_ordinal(field);
                    match payload {
                        HirPatternField::Explicit { pattern, .. } => {
                            edges.push(HirPatternChildEdge::new(
                                HirPatternChild::Pattern(*pattern),
                                HirPatternChildRole::RecordField { field },
                            ));
                        }
                        HirPatternField::Shorthand { local, .. } => {
                            edges.push(HirPatternChildEdge::new(
                                HirPatternChild::Local(*local),
                                HirPatternChildRole::RecordShorthandLocal { field },
                            ));
                        }
                        HirPatternField::Rest {
                            binding: Some(local),
                        } => edges.push(HirPatternChildEdge::new(
                            HirPatternChild::Local(*local),
                            HirPatternChildRole::RecordRestLocal { field },
                        )),
                        HirPatternField::Rest { binding: None }
                        | HirPatternField::Invalid { .. } => {}
                    }
                }
            }
            Self::WholeBinding { binding, pattern } => {
                push_binding(&mut edges, binding, HirPatternChildRole::WholeBindingLocal);
                edges.push(HirPatternChildEdge::new(
                    HirPatternChild::Pattern(*pattern),
                    HirPatternChildRole::NestedPattern,
                ));
            }
            Self::Or { alternatives } => {
                for (ordinal, pattern) in alternatives.iter().enumerate() {
                    edges.push(HirPatternChildEdge::new(
                        HirPatternChild::Pattern(*pattern),
                        HirPatternChildRole::OrAlternative {
                            ordinal: checked_ordinal(ordinal),
                        },
                    ));
                }
            }
            Self::TypedBinding { binding, ty } => {
                push_binding(&mut edges, binding, HirPatternChildRole::TypedBindingLocal);
                edges.push(HirPatternChildEdge::new(
                    HirPatternChild::Type(*ty),
                    HirPatternChildRole::TypedBindingType,
                ));
            }
            Self::Literal(_) | Self::EntityReference(_) | Self::Discard | Self::Error(_) => {}
        }
        edges
    }
}

fn push_binding(
    edges: &mut Vec<HirPatternChildEdge>,
    binding: &HirPatternBinding,
    role: HirPatternChildRole,
) {
    if let HirPatternBinding::Bound { local, .. } = binding {
        edges.push(HirPatternChildEdge::new(
            HirPatternChild::Local(*local),
            role,
        ));
    }
}

fn checked_ordinal(value: usize) -> u32 {
    u32::try_from(value).expect("accepted HIR pattern child sequences fit checked u32 limits")
}
