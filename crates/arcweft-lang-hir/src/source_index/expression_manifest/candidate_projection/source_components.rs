//! Canonical candidate source rows produced by the existing source freeze.
//!
//! These rows move into the module's sole source index only after every arena
//! and candidate payload has passed validation. No candidate lookup path or
//! second source catalog survives publication.

use std::collections::BTreeMap;
use std::sync::Arc;

use arcweft_lang_syntax::attachment::source_file::AttachedPathRoot;
use arcweft_lang_syntax::attachment::{
    AttachedCandidateExpressionChild, AttachedCandidateNode, AttachedCandidatePathExpression,
    AttachedCandidatePatternProjection,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionProjection, SyntaxCallArgumentPart,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::types::TypeRefComponentRole;
use arcweft_source::SourceSpan;

use crate::dialogue_application::HirAttachedContentApplicationFamily;
use crate::expr::HirExprKind;
use crate::identity::{ExprId, PatternId, TypeId};
use crate::pattern::HirPatternKind;
use crate::source_index::{
    HirCallArgumentSourcePart, HirExprSourceRole, HirPatternSourceRole, HirSourceIndex,
    HirSourceQuery, HirSourceRequirement, HirSourceSite, HirTypeSourceRole,
    expression_component_role,
};

#[derive(Default)]
pub(crate) struct CandidateSourceComponents {
    rows: BTreeMap<HirSourceQuery, (HirSourceRequirement, Option<HirSourceSite>)>,
}

impl CandidateSourceComponents {
    #[allow(
        clippy::too_many_lines,
        reason = "one atomic source projection binds expression components and the typed invocation and path components they own"
    )]
    pub(super) fn expression(
        &mut self,
        parsed: &ParsedSource,
        owner: ExprId,
        payload: &HirExprKind,
        node: AttachedCandidateNode<'_>,
    ) -> Option<()> {
        let projection = node.expression_projection()?;
        let target = if matches!(
            projection,
            ExpressionProjection::AttachedContentApplication(_)
        ) {
            node.semantic_expression_children()
                .find_map(|child| match child {
                    AttachedCandidateExpressionChild::Authored {
                        component_role: ExpressionComponentRole::Target,
                        node,
                        ..
                    }
                    | AttachedCandidateExpressionChild::Recovered {
                        component_role: ExpressionComponentRole::Target,
                        node,
                        ..
                    } => Some(node),
                    _ => None,
                })
        } else {
            None
        };
        let target_is_call = target.is_some_and(|node| {
            matches!(
                node.expression_projection(),
                Some(ExpressionProjection::Call(_))
            )
        });
        let requirements = super::super::requirements::expression_requirements(
            payload,
            projection,
            target_is_call,
        )?;
        let mut components = Vec::new();
        for component in node.expression_components()? {
            if let Some(role) = expression_component_role(projection, component.role()) {
                components.push((role, component.source_span().clone()));
            }
        }
        if let HirExprKind::AttachedContentApplication(application) = payload {
            match application.family() {
                HirAttachedContentApplicationFamily::ContentCall { .. } if target_is_call => {
                    let target = target?;
                    for component in target.expression_components()? {
                        if let Some(role) = expression_component_role(
                            target.expression_projection()?,
                            component.role(),
                        ) {
                            components.push((role, component.source_span().clone()));
                        }
                    }
                }
                HirAttachedContentApplicationFamily::DialogueLine { coordinates, .. } => {
                    for coordinate in coordinates {
                        for (syntax_part, part) in [
                            (
                                SyntaxCallArgumentPart::Whole,
                                HirCallArgumentSourcePart::Whole,
                            ),
                            (
                                SyntaxCallArgumentPart::Name,
                                HirCallArgumentSourcePart::Name,
                            ),
                            (
                                SyntaxCallArgumentPart::Value,
                                HirCallArgumentSourcePart::Value,
                            ),
                        ] {
                            let source = target?
                                .expression_components()?
                                .find(|component| {
                                    component.role()
                                        == ExpressionComponentRole::CallArgument {
                                            argument: coordinate.argument().get(),
                                            part: syntax_part,
                                        }
                                })?
                                .source_span()
                                .clone();
                            components.push((
                                HirExprSourceRole::ConfigurationArgument {
                                    argument: coordinate.argument(),
                                    part,
                                },
                                source,
                            ));
                        }
                    }
                }
                HirAttachedContentApplicationFamily::ContentCall { .. } => {}
            }
        }
        if matches!(projection, ExpressionProjection::Path) {
            match node.path_expression_view()? {
                AttachedCandidatePathExpression::Value(path) => {
                    let root = match path.root() {
                        AttachedPathRoot::ImplicitCrate => None,
                        AttachedPathRoot::Crate { source }
                        | AttachedPathRoot::SelfModule { source } => Some(source),
                        AttachedPathRoot::Super { levels } => {
                            let first = levels.first()?;
                            let last = levels.last()?;
                            Some(
                                parsed
                                    .document()
                                    .span(arcweft_source::SourceRange::new(
                                        first.range().start(),
                                        last.range().end(),
                                    ))
                                    .ok()?,
                            )
                        }
                    };
                    if let Some(root) = root {
                        components.push((HirExprSourceRole::PathRoot, root));
                    }
                    let count = path.segments().len();
                    for (ordinal, segment) in path.segments().enumerate() {
                        components.push((
                            HirExprSourceRole::PathSegment {
                                ordinal: u32::try_from(ordinal).ok()?,
                            },
                            segment.source_span(),
                        ));
                    }
                    if let Some(missing) = path.missing_name() {
                        components.push((
                            HirExprSourceRole::PathSegment {
                                ordinal: u32::try_from(count).ok()?,
                            },
                            missing.source_span(),
                        ));
                    }
                }
                AttachedCandidatePathExpression::NominalType(root) => {
                    for component in root.node().type_components()? {
                        let role = match component.role() {
                            TypeRefComponentRole::PathRoot => HirExprSourceRole::PathRoot,
                            TypeRefComponentRole::PathSegment { ordinal } => {
                                HirExprSourceRole::PathSegment { ordinal }
                            }
                            _ => continue,
                        };
                        components.push((role, component.source_span().clone()));
                    }
                }
            }
        }
        self.record(parsed, requirements, components, |role| {
            HirSourceQuery::Expr { owner, role }
        })
    }

    pub(super) fn type_ref(
        &mut self,
        parsed: &ParsedSource,
        owner: TypeId,
        node: AttachedCandidateNode<'_>,
    ) -> Option<()> {
        let value = node.type_projection()?.value();
        let requirements = crate::source_index::type_projection::type_requirements(value);
        let components = node
            .type_components()?
            .into_iter()
            .filter(|component| {
                crate::source_index::type_projection::final_type_component(value, component.role())
            })
            .map(|component| {
                (
                    HirTypeSourceRole::from(component.role()),
                    component.source_span().clone(),
                )
            });
        self.record(parsed, requirements, components, |role| {
            HirSourceQuery::Type { owner, role }
        })
    }

    pub(super) fn pattern(
        &mut self,
        parsed: &ParsedSource,
        owner: PatternId,
        payload: &HirPatternKind,
        source: AttachedCandidatePatternProjection<'_>,
    ) -> Option<()> {
        let requirements = crate::source_index::pattern_projection::pattern_requirements(payload);
        let components = source.components().into_iter().map(|component| {
            (
                HirPatternSourceRole::from(component.role()),
                component.source_span().clone(),
            )
        });
        self.record(parsed, requirements, components, |role| {
            HirSourceQuery::Pattern { owner, role }
        })
    }

    fn record<R: Ord>(
        &mut self,
        parsed: &ParsedSource,
        requirements: BTreeMap<R, HirSourceRequirement>,
        components: impl IntoIterator<Item = (R, SourceSpan)>,
        query: impl Fn(R) -> HirSourceQuery,
    ) -> Option<()> {
        let mut sites = BTreeMap::new();
        for (role, source) in components {
            let query = query(role);
            if query.is_slot_whole() {
                continue;
            }
            let site = HirSourceSite::from_attached_span(parsed.document(), &source).ok()?;
            if sites.insert(query, site).is_some() {
                return None;
            }
        }
        for (role, requirement) in requirements {
            let query = query(role);
            let source = sites.remove(&query);
            if requirement == HirSourceRequirement::Required && source.is_none() {
                return None;
            }
            if self.rows.insert(query, (requirement, source)).is_some() {
                return None;
            }
        }
        sites.is_empty().then_some(())
    }

    pub(crate) fn install(self, index: &mut HirSourceIndex) -> Option<()> {
        if self.rows.keys().any(|query| {
            index.requirements.contains_key(query) || index.components.contains_key(query)
        }) {
            return None;
        }
        for (query, (requirement, site)) in self.rows {
            Arc::make_mut(&mut index.requirements).insert(query.clone(), requirement);
            if let Some(site) = site {
                Arc::make_mut(&mut index.components).insert(query, site);
            }
        }
        Some(())
    }
}
