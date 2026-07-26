//! Lexical identity and registered project-symbol resolution.

use arcweft_lang_hir::symbol::{CallableDeclarationId, CallableSymbol};
use arcweft_lang_syntax::ast::{
    module_path::CanonicalModulePath,
    symbol_path::{ProjectSymbolPath, SymbolPath},
};
use arcweft_source::{SourceRange, SourceSpan};

use super::{TypeChecker, is_character_entity_literal};
use crate::{
    callable::{LexicalBindingIndex, SemanticScopeId},
    types::{EntityKind, TypeKind},
};

impl TypeChecker<'_> {
    pub(super) fn reset_semantic_root_scope(&mut self, module: Option<&CanonicalModulePath>) {
        self.local_callable_ids.clear();
        self.semantic_scope_stack.clear();
        self.current_module = module.cloned();
        let root = self.allocate_semantic_scope();
        self.semantic_scope_stack.push(root);
    }

    pub(super) fn push_semantic_scope(&mut self) -> SemanticScopeId {
        let scope = self.allocate_semantic_scope();
        self.semantic_scope_stack.push(scope);
        scope
    }

    pub(super) fn pop_semantic_scope(&mut self) {
        self.semantic_scope_stack
            .pop()
            .expect("semantic scope stack must stay balanced");
    }

    pub(super) fn current_semantic_scope(&self) -> SemanticScopeId {
        *self
            .semantic_scope_stack
            .last()
            .expect("semantic root scope must exist while checking executable code")
    }

    pub(super) fn allocate_semantic_binding(&mut self) -> LexicalBindingIndex {
        let id = LexicalBindingIndex::from_u32(self.next_semantic_binding);
        self.next_semantic_binding = self
            .next_semantic_binding
            .checked_add(1)
            .expect("semantic binding inventory exceeds u32::MAX entries");
        id
    }

    fn allocate_semantic_scope(&mut self) -> SemanticScopeId {
        let id = SemanticScopeId::from_u32(self.next_semantic_scope);
        self.next_semantic_scope = self
            .next_semantic_scope
            .checked_add(1)
            .expect("semantic scope inventory exceeds u32::MAX entries");
        id
    }

    pub(super) fn speaker_reference_type(&self, reference: &str) -> Option<TypeKind> {
        if is_character_entity_literal(reference) {
            return Some(TypeKind::entity_ref(EntityKind::Character));
        }
        if let Some(ty) = self.locals.get(reference) {
            return Some(ty.clone());
        }
        if let Some(ty) = self.global_symbols.get(reference) {
            return Some(ty.clone());
        }
        if let Some(module) = self.current_module.as_ref()
            && let Some(source) = self
                .source_document_for_current_module()
                .and_then(|document| {
                    document
                        .span(SourceRange::new(0, document.text().len()))
                        .ok()
                })
            && self
                .resolve_project_character_in(module, reference, &source)
                .is_some()
        {
            return Some(TypeKind::entity_ref(EntityKind::Character));
        }
        if let Some(module) = self.current_module.as_ref()
            && let Some(declaration) = self.resolve_project_callable_in(module, reference)
            && let Some(ty) = self.project_functions.get(declaration)
        {
            return Some(ty.clone());
        }
        self.env.symbol_type(reference).cloned()
    }

    pub(super) fn resolve_project_callable(
        &self,
        reference: &str,
    ) -> Option<&CallableDeclarationId> {
        let module = self.current_module.as_ref()?;
        self.resolve_project_callable_in(module, reference)
    }

    fn resolve_project_callable_in(
        &self,
        module: &CanonicalModulePath,
        reference: &str,
    ) -> Option<&CallableDeclarationId> {
        let symbols = self.project_symbols?;
        let source = symbols.callable_symbols().next()?.source();
        let path = reference.parse::<ProjectSymbolPath>().ok()?;
        let path = SymbolPath::try_from(&path).ok()?;
        symbols
            .resolve_callable(module, &path, source)
            .ok()
            .map(CallableSymbol::declaration)
    }

    pub(super) fn resolve_project_character_in(
        &self,
        module: &CanonicalModulePath,
        reference: &str,
        source: &SourceSpan,
    ) -> Option<arcweft_character::id::CharacterId> {
        let symbols = self.project_symbols?;
        let registered = self.registered_environment?;
        let path = reference.parse::<ProjectSymbolPath>().ok()?;
        let path = SymbolPath::try_from(&path).ok()?;
        registered
            .resolve_character_owner(symbols, module, &path, source)
            .ok()
    }
}
