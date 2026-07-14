//! Canonicalization evidence emitted while the normal checker owns the scope.

use std::collections::BTreeMap;

use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_hir::symbol::{CallableDeclarationId, CallableSymbol};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use super::{TypeChecker, is_character_entity_literal};
use crate::canonicalization::{
    CheckedCanonicalizationInventory, CheckedSpeakerLine, SemanticBindingId, SemanticScopeId,
    SemanticSymbolIdentity, SpeakerLineOutcome, SpeakerLineSyntaxId,
};
use crate::types::{EntityKind, SpeakerLineType, TypeKind};

impl TypeChecker<'_> {
    pub(super) fn reset_semantic_root_scope(&mut self, module: Option<&CanonicalModulePath>) {
        self.local_symbol_identities.clear();
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

    pub(super) fn allocate_semantic_binding(&mut self) -> SemanticBindingId {
        let id = SemanticBindingId(self.next_semantic_binding);
        self.next_semantic_binding = self
            .next_semantic_binding
            .checked_add(1)
            .expect("semantic binding inventory exceeds u32::MAX entries");
        id
    }

    fn allocate_semantic_scope(&mut self) -> SemanticScopeId {
        let id = SemanticScopeId(self.next_semantic_scope);
        self.next_semantic_scope = self
            .next_semantic_scope
            .checked_add(1)
            .expect("semantic scope inventory exceeds u32::MAX entries");
        id
    }

    pub(super) fn record_checked_speaker_line(&mut self, dialogue: &HirDialogue) {
        let Some(surface) = dialogue.speaker_surface().copied() else {
            return;
        };
        let Some(module) = dialogue
            .source_module()
            .or(self.current_module.as_ref())
            .cloned()
        else {
            return;
        };
        if self
            .canonicalization_sources
            .and_then(|sources| sources.source(&module))
            .is_none()
        {
            return;
        }

        let reference = dialogue.callee().to_owned();
        let (symbol, resolved_type) = self.resolve_speaker_reference(&reference, &module);
        let outcome = resolved_type
            .as_ref()
            .map_or(SpeakerLineOutcome::Unresolved, |ty| {
                match ty.speaker_line_classification() {
                    Some(SpeakerLineType::Preset(entity_kind)) => {
                        SpeakerLineOutcome::Preset { entity_kind }
                    }
                    Some(SpeakerLineType::Speaker(entity_kind)) => {
                        SpeakerLineOutcome::Speaker { entity_kind }
                    }
                    None => SpeakerLineOutcome::NonSpeaker,
                }
            });
        self.checked_speaker_lines.push(CheckedSpeakerLine::new(
            SpeakerLineSyntaxId::new(module, surface.head_range()),
            surface,
            self.current_semantic_scope(),
            reference,
            symbol,
            resolved_type,
            outcome,
        ));
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
            && let Some(declaration) = self.resolve_project_callable_in(module, reference)
            && let Some(ty) = self.project_functions.get(declaration)
        {
            return Some(ty.clone());
        }
        self.env.symbol_type(reference).cloned()
    }

    fn resolve_speaker_reference(
        &self,
        reference: &str,
        module: &CanonicalModulePath,
    ) -> (Option<SemanticSymbolIdentity>, Option<TypeKind>) {
        if is_character_entity_literal(reference) {
            return (
                Some(SemanticSymbolIdentity::EntityLiteral {
                    kind: EntityKind::Character,
                    canonical_name: reference.to_owned(),
                }),
                Some(TypeKind::entity_ref(EntityKind::Character)),
            );
        }
        if let Some(ty) = self.locals.get(reference).cloned() {
            return (
                self.local_symbol_identities.get(reference).cloned(),
                Some(ty),
            );
        }
        if let Some(ty) = self.global_symbols.get(reference).cloned() {
            return (
                Some(SemanticSymbolIdentity::ModuleValue {
                    module: module.clone(),
                    name: reference.to_owned(),
                }),
                Some(ty),
            );
        }
        if let Some(declaration) = self.resolve_project_callable_in(module, reference) {
            return (
                Some(SemanticSymbolIdentity::Callable {
                    declaration: declaration.clone(),
                }),
                self.project_functions.get(declaration).cloned(),
            );
        }
        if let Some(ty) = self.env.symbol_type(reference).cloned() {
            return (
                Some(SemanticSymbolIdentity::EnvironmentValue {
                    name: reference.to_owned(),
                }),
                Some(ty),
            );
        }
        (None, None)
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
        self.callable_symbols?
            .resolve(module, reference)
            .ok()
            .map(CallableSymbol::declaration)
    }

    pub(super) fn finish_canonicalization_inventories(
        &mut self,
    ) -> Vec<CheckedCanonicalizationInventory> {
        let Some(sources) = self.canonicalization_sources else {
            self.checked_speaker_lines.clear();
            return Vec::new();
        };
        let mut by_module = BTreeMap::<CanonicalModulePath, Vec<CheckedSpeakerLine>>::new();
        for line in std::mem::take(&mut self.checked_speaker_lines) {
            by_module
                .entry(line.id().module().clone())
                .or_default()
                .push(line);
        }
        sources
            .sources()
            .cloned()
            .map(|source| {
                let lines = by_module.remove(source.module()).unwrap_or_default();
                CheckedCanonicalizationInventory::new(source, lines)
            })
            .collect()
    }
}
