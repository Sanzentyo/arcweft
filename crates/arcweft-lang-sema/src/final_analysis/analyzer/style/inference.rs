//! Finite native-kind constraints for original sheet-local token declarations.
//!
//! Principal Source forms, explicit annotations and typed uses constrain one
//! domain per initializer. Unannotated aliases impose equality; contextual
//! Length-to-radii conversion remains directed. No expression facts or guessed
//! runtime values are published while domains are unresolved.

use super::{
    FinalSemanticAnalysisError, StyleAnalysisContext,
    grammar::{StyleArgumentType, StyleFactorySignature, StyleSourceGrammar},
};
use crate::types::{CompileTimeStyleType, StyleCallableId};
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, HirModuleId, ItemId},
    leaf::{HirLiteral, HirStringLiteral},
    module::HirModule,
};
use arcweft_view::style::ViewStyleValueKind;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

// The native registry owns tags. Fail at compilation if its closed domain no
// longer fits this compact compiler-private set; never truncate a new family.
const _: () = {
    let mut index = 0;
    while index < ViewStyleValueKind::ALL.len() {
        assert!((ViewStyleValueKind::ALL[index].semantic_tag() as u32) < u32::BITS);
        index += 1;
    }
};

#[derive(Clone, Copy, Eq, PartialEq)]
struct StyleTypeDomain(u32);

impl StyleTypeDomain {
    fn from_kinds(kinds: impl IntoIterator<Item = ViewStyleValueKind>) -> Self {
        Self(
            kinds
                .into_iter()
                .fold(0, |bits, kind| bits | (1 << kind.semantic_tag())),
        )
    }
    fn one(kind: ViewStyleValueKind) -> Self {
        Self::from_kinds([kind])
    }
    fn all() -> Self {
        Self::from_kinds(ViewStyleValueKind::ALL.iter().copied())
    }
    fn unique(self) -> Option<ViewStyleValueKind> {
        self.0
            .is_power_of_two()
            .then(|| {
                ViewStyleValueKind::ALL
                    .iter()
                    .copied()
                    .find(|kind| self.0 == Self::one(*kind).0)
            })
            .flatten()
    }
    fn intersect(&mut self, other: Self) -> bool {
        let before = *self;
        self.0 &= other.0;
        *self != before
    }
    fn accepts_context(kind: ViewStyleValueKind) -> Self {
        if kind == ViewStyleValueKind::BorderRadii {
            Self::from_kinds([kind, ViewStyleValueKind::Length])
        } else {
            Self::one(kind)
        }
    }
}

impl StyleSourceGrammar {
    fn principal_domain(module: &HirModule, owner: ExprId) -> StyleTypeDomain {
        let Ok(expression) = module.resolve_expr(owner) else {
            return StyleTypeDomain(0);
        };
        match expression.kind() {
            HirExprKind::Call(call) => match Self::callable(module, call) {
                Some((_, StyleCallableId::Token)) => StyleTypeDomain::all(),
                Some((_, callable)) => match StyleFactorySignature::for_callable(callable).result {
                    Some(CompileTimeStyleType::Value(kind)) => StyleTypeDomain::one(kind),
                    _ => StyleTypeDomain(0),
                },
                None => StyleTypeDomain(0),
            },
            HirExprKind::BracketSequence(sequence) => StyleTypeDomain::from_kinds(
                ViewStyleValueKind::ALL.iter().copied().filter(|kind| {
                    let Some(element_type) = Self::sequence_element(*kind) else {
                        return false;
                    };
                    sequence.elements().iter().all(|child| {
                        let Ok(child) = module.resolve_expr(*child) else {
                            return false;
                        };
                        match child.kind() {
                            HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(
                                _,
                            ))) => element_type == CompileTimeStyleType::FontFamily,
                            HirExprKind::Call(call) => {
                                Self::callable(module, call).is_some_and(|(_, callable)| {
                                    StyleFactorySignature::for_callable(callable).result
                                        == Some(element_type)
                                })
                            }
                            _ => false,
                        }
                    })
                }),
            ),
            _ => StyleTypeDomain::from_kinds(Self::literal_kinds(module, owner)),
        }
    }
}

impl StyleAnalysisContext {
    pub(in crate::final_analysis::analyzer) fn infer_tokens(
        &mut self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut domains = BTreeMap::new();
        for scope in self.tokens.values() {
            for token in scope.declarations.values() {
                let module = modules
                    .get(&token.value.module())
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                let domain = self.value_kind(token.value).map_or_else(
                    || StyleSourceGrammar::principal_domain(module, token.value),
                    StyleTypeDomain::one,
                );
                if domain.0 == 0 {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: token.value,
                    });
                }
                domains.insert(token.value, domain);
            }
        }
        let mut aliases = BTreeMap::<ExprId, Vec<ExprId>>::new();
        for (root, style) in &self.root_styles {
            let module = modules
                .get(&root.module())
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            let expected = self
                .value_kind(*root)
                .or_else(|| domains.get(root).and_then(|domain| domain.unique()))
                .map(CompileTimeStyleType::Value);
            // An unannotated alias synthesizes its declaration's principal kind.
            // It must not infer a conversion merely because a use permits one.
            let root_is_alias = self.value_kind(*root).is_none()
                && matches!(module.resolve_expr(*root).map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?.kind(),
                    HirExprKind::Call(call) if StyleSourceGrammar::callable(module, call).is_some_and(|(_, callable)| callable == StyleCallableId::Token));
            if root_is_alias {
                let HirExprKind::Call(call) = module
                    .resolve_expr(*root)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .kind()
                else {
                    return Err(FinalSemanticAnalysisError::InvalidOwner);
                };
                let binding = StyleFactorySignature::for_callable(StyleCallableId::Token)
                    .bind(call)
                    .map_err(|_| FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: *root,
                    })?;
                let [(_, path)] = binding.as_slice() else {
                    return Err(FinalSemanticAnalysisError::InvalidOwner);
                };
                let target = self
                    .lookup_token(module, *style, *path)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: *path })?
                    .value;
                aliases.entry(*root).or_default().push(target);
                aliases.entry(target).or_default().push(*root);
            } else {
                self.constrain_style_uses(module, *style, *root, expected, &mut domains)?;
            }
        }
        // Monotone intersections visit each edge only after a domain shrinks.
        // A domain loses at most one bit per native family, so forward references
        // and long alias chains do not require quadratic rescans or recursion.
        let mut queue = domains.keys().copied().collect::<VecDeque<_>>();
        let mut queued = domains.keys().copied().collect::<BTreeSet<_>>();
        while let Some(owner) = queue.pop_front() {
            queued.remove(&owner);
            let source = domains[&owner];
            for target in aliases.get(&owner).into_iter().flatten() {
                let domain = domains
                    .get_mut(target)
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                if domain.intersect(source) && queued.insert(*target) {
                    queue.push_back(*target);
                }
                if domain.0 == 0 {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: *target,
                    });
                }
            }
        }
        let inferred = domains
            .into_iter()
            .map(|(owner, domain)| {
                domain
                    .unique()
                    .map(|kind| (owner, kind))
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // All domains must be unique before any inferred expectation is visible.
        for (owner, kind) in inferred {
            self.values.entry(owner).or_insert(kind);
        }
        Ok(())
    }

    fn constrain_style_uses(
        &self,
        module: &HirModule,
        style: ItemId,
        root: ExprId,
        expected: Option<CompileTimeStyleType>,
        domains: &mut BTreeMap<ExprId, StyleTypeDomain>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut pending = vec![(root, expected)];
        while let Some((owner, expected)) = pending.pop() {
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            match expression.kind() {
                HirExprKind::Call(call) => {
                    let (_, callable) = StyleSourceGrammar::callable(module, call)
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
                    let signature = StyleFactorySignature::for_callable(callable);
                    let binding = signature.bind(call).map_err(|_| {
                        FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner }
                    })?;
                    if callable == StyleCallableId::Token {
                        let Some(CompileTimeStyleType::Value(kind)) = expected else {
                            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            });
                        };
                        let [(_, path)] = binding.as_slice() else {
                            return Err(FinalSemanticAnalysisError::InvalidOwner);
                        };
                        let target = self
                            .lookup_token(module, style, *path)
                            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner: *path,
                            })?
                            .value;
                        let domain = domains
                            .get_mut(&target)
                            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                        domain.intersect(StyleTypeDomain::accepts_context(kind));
                        if domain.0 == 0 {
                            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                owner,
                            });
                        }
                    } else {
                        for (index, child) in binding.into_iter().rev() {
                            if let StyleArgumentType::Style(ty) = signature.parameters[index].ty {
                                pending.push((child, Some(ty)));
                            }
                        }
                    }
                }
                HirExprKind::BracketSequence(sequence) => {
                    let element = match expected {
                        Some(CompileTimeStyleType::Value(kind)) => {
                            StyleSourceGrammar::sequence_element(kind)
                        }
                        _ => None,
                    };
                    for child in sequence.elements().iter().rev() {
                        pending.push((*child, element));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
