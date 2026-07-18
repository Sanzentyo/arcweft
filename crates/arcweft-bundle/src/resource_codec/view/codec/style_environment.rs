//! Typed source-graph validation for native Style environment guards.

use super::super::model::ViewStyleResource;
use crate::resource_codec::SourceRangeRef;
use arcweft_view::style::{
    ViewEnvironmentCondition, ViewEnvironmentWrapperIndex, ViewStyleSourceId,
};
use thiserror::Error;

/// The retained environment range whose source contract failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewStyleEnvironmentSourceRole {
    Predicate {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    Body {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    Scope {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    Clause {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    GuardedRule,
}

/// Invalid product provenance for one native Style environment guard.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleEnvironmentSourceError {
    #[error("Style environment {role:?} references an unknown product source")]
    UnknownSource {
        role: ViewStyleEnvironmentSourceRole,
    },
    #[error("Style environment {role:?} references an unknown Style source range")]
    UnknownRange {
        role: ViewStyleEnvironmentSourceRole,
    },
    #[error("Style environment {role:?} source range is reversed")]
    ReversedRange {
        role: ViewStyleEnvironmentSourceRole,
    },
    #[error("Style environment {role:?} source range is empty")]
    EmptyRange {
        role: ViewStyleEnvironmentSourceRole,
    },
    #[error("Style environment {role:?} source range exceeds normalized source bounds")]
    SourceOutOfBounds {
        role: ViewStyleEnvironmentSourceRole,
    },
    #[error("the outer environment wrapper and guarded rule have different source owners")]
    WrongRuleOwner,
    #[error("Style environment {role:?} crosses the effective path's source owner")]
    CrossSourceRelation {
        role: ViewStyleEnvironmentSourceRole,
    },
    #[error("wrapper {wrapper:?} predicate is not contained by its scope")]
    PredicateNotContainedByScope {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    #[error("wrapper {wrapper:?} body is not contained by its scope")]
    BodyNotContainedByScope {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    #[error("wrapper {wrapper:?} predicate overlaps its body")]
    PredicateBodyOrder {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    #[error("a clause is not contained by its owning wrapper {wrapper:?} predicate")]
    ClauseNotContainedByPredicate {
        wrapper: ViewEnvironmentWrapperIndex,
    },
    #[error("wrapper {child:?} scope is not contained by wrapper {parent:?} body")]
    NestedScopeNotContained {
        parent: ViewEnvironmentWrapperIndex,
        child: ViewEnvironmentWrapperIndex,
    },
    #[error("the guarded rule is not contained by wrapper {wrapper:?} body")]
    GuardedRuleNotContained {
        wrapper: ViewEnvironmentWrapperIndex,
    },
}

#[derive(Clone, Copy)]
struct ResolvedWrapper {
    index: ViewEnvironmentWrapperIndex,
    predicate: SourceRangeRef,
    body: SourceRangeRef,
    scope: SourceRangeRef,
}

pub(in crate::resource_codec::view) fn validate_structure(
    style: &ViewStyleResource,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    for sheet in style.program.sheets() {
        for rule in sheet.rules() {
            let Some(condition) = rule.environment() else {
                continue;
            };
            let rule_range = owned_range(
                style,
                rule.source(),
                ViewStyleEnvironmentSourceRole::GuardedRule,
            )?;
            let wrappers = resolve_wrappers(style, condition)?;
            let outer = wrappers
                .first()
                .expect("checked environment conditions retain one wrapper");
            if outer.scope.source() != rule_range.source() {
                return Err(ViewStyleEnvironmentSourceError::WrongRuleOwner);
            }

            validate_wrapper_graph(&wrappers, outer.scope)?;
            validate_clauses(style, condition, &wrappers, outer.scope)?;

            let innermost = wrappers
                .last()
                .expect("checked environment conditions retain one wrapper");
            if !contains(innermost.body, rule_range) {
                return Err(ViewStyleEnvironmentSourceError::GuardedRuleNotContained {
                    wrapper: innermost.index,
                });
            }
        }
    }
    Ok(())
}

fn resolve_wrappers(
    style: &ViewStyleResource,
    condition: &ViewEnvironmentCondition,
) -> Result<Vec<ResolvedWrapper>, ViewStyleEnvironmentSourceError> {
    condition
        .wrappers()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, wrapper)| {
            let index = ViewEnvironmentWrapperIndex::new(
                u8::try_from(index).expect("environment wrapper count is bounded by four"),
            );
            Ok(ResolvedWrapper {
                index,
                predicate: owned_range(
                    style,
                    wrapper.predicate_source(),
                    ViewStyleEnvironmentSourceRole::Predicate { wrapper: index },
                )?,
                body: owned_range(
                    style,
                    wrapper.body_source(),
                    ViewStyleEnvironmentSourceRole::Body { wrapper: index },
                )?,
                scope: owned_range(
                    style,
                    wrapper.scope_source(),
                    ViewStyleEnvironmentSourceRole::Scope { wrapper: index },
                )?,
            })
        })
        .collect()
}

fn validate_wrapper_graph(
    wrappers: &[ResolvedWrapper],
    owner: SourceRangeRef,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    for wrapper in wrappers {
        ensure_path_source(
            owner,
            wrapper.predicate,
            ViewStyleEnvironmentSourceRole::Predicate {
                wrapper: wrapper.index,
            },
        )?;
        ensure_path_source(
            owner,
            wrapper.body,
            ViewStyleEnvironmentSourceRole::Body {
                wrapper: wrapper.index,
            },
        )?;
        ensure_path_source(
            owner,
            wrapper.scope,
            ViewStyleEnvironmentSourceRole::Scope {
                wrapper: wrapper.index,
            },
        )?;
        if !contains(wrapper.scope, wrapper.predicate) {
            return Err(
                ViewStyleEnvironmentSourceError::PredicateNotContainedByScope {
                    wrapper: wrapper.index,
                },
            );
        }
        if !contains(wrapper.scope, wrapper.body) {
            return Err(ViewStyleEnvironmentSourceError::BodyNotContainedByScope {
                wrapper: wrapper.index,
            });
        }
        if wrapper.predicate.end_byte() > wrapper.body.start_byte() {
            return Err(ViewStyleEnvironmentSourceError::PredicateBodyOrder {
                wrapper: wrapper.index,
            });
        }
    }
    for pair in wrappers.windows(2) {
        if !contains(pair[0].body, pair[1].scope) {
            return Err(ViewStyleEnvironmentSourceError::NestedScopeNotContained {
                parent: pair[0].index,
                child: pair[1].index,
            });
        }
    }
    Ok(())
}

fn validate_clauses(
    style: &ViewStyleResource,
    condition: &ViewEnvironmentCondition,
    wrappers: &[ResolvedWrapper],
    owner: SourceRangeRef,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    for clause in condition.clauses() {
        let role = ViewStyleEnvironmentSourceRole::Clause {
            wrapper: clause.wrapper(),
        };
        let clause_range = owned_range(style, clause.source(), role)?;
        ensure_path_source(owner, clause_range, role)?;
        let predicate = wrappers[clause.wrapper().index()].predicate;
        if !contains(predicate, clause_range) {
            return Err(
                ViewStyleEnvironmentSourceError::ClauseNotContainedByPredicate {
                    wrapper: clause.wrapper(),
                },
            );
        }
    }
    Ok(())
}

fn owned_range(
    style: &ViewStyleResource,
    id: ViewStyleSourceId,
    role: ViewStyleEnvironmentSourceRole,
) -> Result<SourceRangeRef, ViewStyleEnvironmentSourceError> {
    let range = *style
        .source_map_refs
        .get(id.value() as usize)
        .ok_or(ViewStyleEnvironmentSourceError::UnknownRange { role })?;
    if range.start_byte() > range.end_byte() {
        return Err(ViewStyleEnvironmentSourceError::ReversedRange { role });
    }
    if range.start_byte() == range.end_byte() {
        return Err(ViewStyleEnvironmentSourceError::EmptyRange { role });
    }
    let source = style
        .source_refs
        .get(range.source().index())
        .ok_or(ViewStyleEnvironmentSourceError::UnknownSource { role })?;
    if u64::from(range.end_byte()) > source.source_len() {
        return Err(ViewStyleEnvironmentSourceError::SourceOutOfBounds { role });
    }
    Ok(range)
}

fn ensure_path_source(
    owner: SourceRangeRef,
    related: SourceRangeRef,
    role: ViewStyleEnvironmentSourceRole,
) -> Result<(), ViewStyleEnvironmentSourceError> {
    if owner.source() == related.source() {
        Ok(())
    } else {
        Err(ViewStyleEnvironmentSourceError::CrossSourceRelation { role })
    }
}

const fn contains(parent: SourceRangeRef, child: SourceRangeRef) -> bool {
    parent.start_byte() <= child.start_byte() && child.end_byte() <= parent.end_byte()
}

#[cfg(test)]
mod tests {
    use super::contains;
    use crate::resource_codec::{ProductSourceRefIndex, SourceRangeRef};

    fn range(start: u32, end: u32) -> SourceRangeRef {
        SourceRangeRef::new(
            ProductSourceRefIndex::try_from_index(0).expect("fixture source index"),
            start,
            end,
        )
    }

    #[test]
    fn containment_accepts_strict_and_equal_boundaries_and_rejects_one_byte_beyond() {
        let parent = range(10, 20);

        assert!(contains(parent, range(11, 19)));
        assert!(contains(parent, parent));
        assert!(contains(parent, range(10, 19)));
        assert!(contains(parent, range(11, 20)));
        assert!(!contains(parent, range(9, 19)));
        assert!(!contains(parent, range(11, 21)));
    }
}
