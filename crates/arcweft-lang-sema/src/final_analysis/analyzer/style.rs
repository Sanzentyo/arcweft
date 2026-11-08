//! Source ownership and contextual expectations for specified Style values.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    expr::{HirExprKind, HirSelectedMember},
    identity::{ExprId, HirModuleId, ItemId, TypeId},
    item::{HirItemKind, HirStyleBodyItem},
    leaf::{HirPathRoot, HirPathSegment},
    module::HirModule,
    source_index::HirStyleBodyPath,
};
use arcweft_presentation::appearance::PresentationEnvironmentField;
use arcweft_view::style::{
    ViewEnvironmentCondition, ViewPropertyKind, ViewStyleTokenId, ViewStyleValueKind,
};

use crate::{
    nominal::{ResolvedTypeRefOutcome, TypeResolutionReport},
    types::{CompileTimeStyleType, TypeKind},
};

use super::super::FinalSemanticAnalysisError;
use super::super::{CheckedStyleEnvironmentTest, CheckedStyleTokenReference};

#[path = "style/grammar.rs"]
pub(super) mod grammar;
#[path = "style/inference.rs"]
mod inference;

struct StyleTokenDeclaration {
    ordinal: u32,
    value: ExprId,
    id: ViewStyleTokenId,
}

#[derive(Default)]
struct StyleTokenScope {
    declarations: BTreeMap<Vec<String>, StyleTokenDeclaration>,
    max_path_segments: usize,
}

#[derive(Clone, Copy)]
struct StyleTokenAnnotationOwner {
    style: ItemId,
    ordinal: u32,
    value: ExprId,
}

pub(super) struct StyleAnalysisContext {
    values: BTreeMap<ExprId, ViewStyleValueKind>,
    annotations: BTreeMap<TypeId, StyleTokenAnnotationOwner>,
    tokens: BTreeMap<ItemId, StyleTokenScope>,
    root_styles: BTreeMap<ExprId, ItemId>,
    constant_dependencies: Vec<ExprId>,
    environments: BTreeMap<ExprId, CheckedStyleEnvironmentTest>,
}

impl StyleAnalysisContext {
    pub(super) fn from_modules(
        modules: &BTreeMap<HirModuleId, &HirModule>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut context = Self {
            values: BTreeMap::new(),
            annotations: BTreeMap::new(),
            tokens: BTreeMap::new(),
            root_styles: BTreeMap::new(),
            constant_dependencies: Vec::new(),
            environments: BTreeMap::new(),
        };
        for module in modules.values() {
            for (owner, item) in module.items() {
                let HirItemKind::Style(style) = item.kind() else {
                    continue;
                };
                for (ordinal, token) in style.tokens().iter().enumerate() {
                    if context.root_styles.insert(token.value(), owner).is_some() {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                    let id = arcweft_id::PublicId::try_from(token)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    let name: Vec<String> = id.as_str().split('.').map(str::to_owned).collect();
                    let scope = context.tokens.entry(owner).or_default();
                    scope.max_path_segments = scope.max_path_segments.max(name.len());
                    if scope
                        .declarations
                        .insert(
                            name,
                            StyleTokenDeclaration {
                                ordinal,
                                value: token.value(),
                                id: ViewStyleTokenId::from_public_id(id),
                            },
                        )
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                    let Some(ty) = token.value_type() else {
                        continue;
                    };
                    if context
                        .annotations
                        .insert(
                            ty,
                            StyleTokenAnnotationOwner {
                                style: owner,
                                ordinal,
                                value: token.value(),
                            },
                        )
                        .is_some()
                    {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                }
                context.collect_body(
                    module,
                    owner,
                    style.body(),
                    &HirStyleBodyPath::root(),
                    &BTreeSet::new(),
                )?;
            }
        }
        Ok(context)
    }

    fn collect_body(
        &mut self,
        module: &HirModule,
        style: ItemId,
        body: &[HirStyleBodyItem],
        path: &HirStyleBodyPath,
        fields: &BTreeSet<PresentationEnvironmentField>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for (ordinal, item) in body.iter().enumerate() {
            match item {
                HirStyleBodyItem::Rule(rule) => {
                    for declaration in rule.declarations() {
                        let Some(kind) = declaration
                            .property()
                            .as_str()
                            .and_then(ViewPropertyKind::from_source_name)
                        else {
                            continue;
                        };
                        self.insert_value(declaration.value(), kind.value_kind())?;
                        if self
                            .root_styles
                            .insert(declaration.value(), style)
                            .is_some()
                        {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                        }
                    }
                }
                HirStyleBodyItem::Environment(environment) => {
                    if environment.clauses().is_empty()
                        || path.ordinals().len() >= ViewEnvironmentCondition::MAX_WRAPPERS
                        || fields
                            .len()
                            .checked_add(environment.clauses().len())
                            .is_none_or(|count| count > ViewEnvironmentCondition::MAX_CLAUSES)
                    {
                        return Err(FinalSemanticAnalysisError::InvalidOwner);
                    }
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                    let mut nested_fields = fields.clone();
                    for (clause, original) in environment.clauses().iter().enumerate() {
                        let checked = CheckedStyleEnvironmentTest::from_source(
                            module,
                            style,
                            path.clone(),
                            ordinal,
                            u32::try_from(clause)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                        )
                        .ok_or(
                            FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner: original.value(),
                            },
                        )?;
                        if !nested_fields.insert(checked.test().field())
                            || self
                                .environments
                                .insert(original.value(), checked)
                                .is_some()
                        {
                            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner: original.value(),
                            });
                        }
                    }
                    let mut nested_path = path.ordinals().to_vec();
                    nested_path.push(ordinal);
                    self.collect_body(
                        module,
                        style,
                        environment.body(),
                        &HirStyleBodyPath::from_ordinals(nested_path.into_boxed_slice()),
                        &nested_fields,
                    )?
                }
                HirStyleBodyItem::Recovered(_) => {}
            }
        }
        Ok(())
    }

    fn insert_value(
        &mut self,
        owner: ExprId,
        kind: ViewStyleValueKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.values.insert(owner, kind).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(())
    }

    pub(super) fn environment(&self, owner: ExprId) -> Option<&CheckedStyleEnvironmentTest> {
        self.environments.get(&owner)
    }

    pub(super) fn annotation_owner(&self, owner: TypeId) -> Option<(ItemId, u32)> {
        self.annotations
            .get(&owner)
            .map(|annotation| (annotation.style, annotation.ordinal))
    }

    pub(super) fn complete_annotations(
        &mut self,
        reports: &BTreeMap<TypeId, TypeResolutionReport>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for (owner, annotation) in &self.annotations {
            let Some(report) = reports.get(owner) else {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner: *owner });
            };
            let ResolvedTypeRefOutcome::Complete(product) = report.outcome() else {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner: *owner });
            };
            let TypeKind::CompileTimeStyle(CompileTimeStyleType::Value(kind)) = product.recovered()
            else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            if self.values.insert(annotation.value, *kind).is_some() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        Ok(())
    }

    pub(super) fn value_kind(&self, owner: ExprId) -> Option<ViewStyleValueKind> {
        self.values.get(&owner).copied()
    }

    pub(super) fn token_initializer(&self, style: ItemId, id: &ViewStyleTokenId) -> Option<ExprId> {
        let name = id
            .public_id()
            .as_str()
            .split('.')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        self.tokens
            .get(&style)?
            .declarations
            .get(&name)
            .map(|token| token.value)
    }

    pub(super) fn constant_depth(&self) -> usize {
        self.constant_dependencies.len()
    }

    pub(super) fn enter_constant(&mut self, initializer: ExprId) -> bool {
        if self.constant_dependencies.len()
            >= arcweft_view::style::ViewStyleSheet::MAX_TOKEN_REFERENCE_DEPTH
            || self.constant_dependencies.contains(&initializer)
        {
            return false;
        }
        self.constant_dependencies.push(initializer);
        true
    }

    pub(super) fn leave_constants(&mut self, depth: usize) {
        self.constant_dependencies.truncate(depth);
    }

    pub(super) fn token(
        &self,
        module: &HirModule,
        style: ItemId,
        expression: ExprId,
    ) -> Option<CheckedStyleTokenReference> {
        let token = self.lookup_token(module, style, expression)?;
        Some(CheckedStyleTokenReference::new(
            style,
            token.ordinal,
            token.value,
            token.id.clone(),
            self.value_kind(token.value)?,
        ))
    }

    fn lookup_token(
        &self,
        module: &HirModule,
        style: ItemId,
        mut expression: ExprId,
    ) -> Option<&StyleTokenDeclaration> {
        let scope = self.tokens.get(&style)?;
        let mut suffix = Vec::new();
        // No declared name can match after this exact scope-derived bound.
        // Qualifiers remain HIR syntax and are never evaluated as values.
        let mut name = loop {
            match module.resolve_expr(expression).ok()?.kind() {
                HirExprKind::Select(select) => {
                    let HirSelectedMember::Name(member) = select.member() else {
                        return None;
                    };
                    if suffix.len() >= scope.max_path_segments {
                        return None;
                    }
                    suffix.push(member.as_str().to_owned());
                    expression = select.target();
                }
                HirExprKind::Path(path) => {
                    let path = path.as_resolved()?;
                    if path.root() != HirPathRoot::ImplicitCrate
                        || path.segments().len().checked_add(suffix.len())?
                            > scope.max_path_segments
                    {
                        return None;
                    }
                    break path
                        .segments()
                        .iter()
                        .map(|segment| match segment {
                            HirPathSegment::Identifier(name) => name.as_str().to_owned(),
                            HirPathSegment::ProjectSymbol(name) => name.as_str().to_owned(),
                        })
                        .collect::<Vec<_>>();
                }
                _ => return None,
            }
        };
        name.extend(suffix.into_iter().rev());
        scope.declarations.get(&name)
    }
}
