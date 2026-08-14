//! Typed immutable resolution over one accepted module lease.

use arcweft_lang_syntax::attachment::SyntaxNodeId;
use arcweft_source::SourceSpan;

use crate::arena::{ArenaIter, ArenaSnapshot, HirArenaError};
use crate::expr::{HirCallExpr, HirExpr, HirExprKind};
use crate::identity::{
    CaptureId, ExprId, HirIdKind, HirTypedId, IdResolveError, ItemId, LocalId, PatternId, ScopeId,
    StmtId, TypeId,
};
use crate::item::HirItem;
use crate::leaf::{HirName, HirPath};
use crate::pattern::HirPattern;
use crate::scope::{HirCapture, HirLocal, HirScope, LocalLookup};
use crate::slot::{HirSlotError, HirSlotMetadata};
use crate::source_index::HirSourceLookupError;
use crate::stmt::HirStmt;
use crate::type_ref::HirType;

use super::{HirLocalResolver, HirModule};

impl HirModule {
    /// Resolves one item ID against this exact immutable module revision.
    pub fn resolve_item(&self, id: ItemId) -> Result<&HirItem, IdResolveError> {
        self.resolve_arena(&self.arenas.items, id)
    }

    /// Resolves one scope ID against this exact immutable module revision.
    pub fn resolve_scope(&self, id: ScopeId) -> Result<&HirScope, IdResolveError> {
        self.resolve_arena(&self.arenas.scopes, id)
    }

    /// Resolves one local ID against this exact immutable module revision.
    pub fn resolve_local(&self, id: LocalId) -> Result<&HirLocal, IdResolveError> {
        self.resolve_arena(&self.arenas.locals, id)
    }

    /// Resolves the nearest source-visible lexical local before one exact use
    /// site without reconstructing a binding point from authored name text.
    ///
    /// # Panics
    ///
    /// Panics only if this already accepted module contains an invalid local
    /// inventory or an incomplete or cyclic scope graph.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the lookup boundary consumes the exact use-site span while retaining no caller-owned source carrier"
    )]
    pub fn lookup_local(
        &self,
        scope: ScopeId,
        name: &HirName,
        before: SourceSpan,
    ) -> Result<LocalLookup, IdResolveError> {
        self.resolve_scope(scope)?;
        let resolver = HirLocalResolver::published(
            &self.slots,
            &self.arenas.scopes,
            &self.arenas.locals,
            &self.arenas.statements,
        )
        .expect("accepted module local-resolution inventory remains valid");
        Ok(resolver
            .lookup(scope, name.as_str(), before.range().start())
            .expect("accepted module scope graph remains acyclic and complete"))
    }

    /// Resolves a one-segment implicit path through the lexical-local graph.
    ///
    /// This is the path-owned counterpart of [`Self::lookup_local`]. It
    /// preserves parser segment classification, including keyword receivers,
    /// while rejecting explicit module roots and qualified paths before local
    /// lookup.
    ///
    /// # Panics
    ///
    /// Panics only if this already accepted module contains an invalid local
    /// inventory or an incomplete or cyclic scope graph.
    pub fn lookup_path_local(
        &self,
        scope: ScopeId,
        path: &HirPath,
        before: &SourceSpan,
    ) -> Result<LocalLookup, IdResolveError> {
        let Some(name) = path.lexical_name() else {
            return Ok(LocalLookup::NotFound);
        };
        self.resolve_scope(scope)?;
        let resolver = HirLocalResolver::published(
            &self.slots,
            &self.arenas.scopes,
            &self.arenas.locals,
            &self.arenas.statements,
        )
        .expect("accepted module local-resolution inventory remains valid");
        Ok(resolver
            .lookup(scope, name, before.range().start())
            .expect("accepted module scope graph remains acyclic and complete"))
    }

    /// Resolves one expression ID against this exact immutable module revision.
    pub fn resolve_expr(&self, id: ExprId) -> Result<&HirExpr, IdResolveError> {
        self.resolve_arena(&self.arenas.expressions, id)
    }

    /// Resolves the runtime value receiver carried by one final-HIR call.
    ///
    /// An unresolved-dot callee already stores the receiver expression
    /// directly. A value callee may retain a synthetic `Select` carrier; in
    /// that case the selected target is the value receiver consumed by call
    /// lowering, not the carrier itself.
    pub fn resolve_call_value_receiver(
        &self,
        call: &HirCallExpr,
    ) -> Result<Option<ExprId>, IdResolveError> {
        let Some(callee) = call.callee().value_expression() else {
            return Ok(None);
        };
        let receiver = match self.resolve_expr(callee)?.kind() {
            HirExprKind::Select(select) => select.target(),
            _ => callee,
        };
        Ok(Some(receiver))
    }

    /// Resolves one statement ID against this exact immutable module revision.
    pub fn resolve_stmt(&self, id: StmtId) -> Result<&HirStmt, IdResolveError> {
        self.resolve_arena(&self.arenas.statements, id)
    }

    /// Resolves one type ID against this exact immutable module revision.
    pub fn resolve_type(&self, id: TypeId) -> Result<&HirType, IdResolveError> {
        self.resolve_arena(&self.arenas.types, id)
    }

    /// Resolves one pattern ID against this exact immutable module revision.
    pub fn resolve_pattern(&self, id: PatternId) -> Result<&HirPattern, IdResolveError> {
        self.resolve_arena(&self.arenas.patterns, id)
    }

    /// Resolves one capture ID against this exact immutable module revision.
    pub fn resolve_capture(&self, id: CaptureId) -> Result<&HirCapture, IdResolveError> {
        self.resolve_arena(&self.arenas.captures, id)
    }

    pub(crate) fn metadata<I: HirTypedId>(
        &self,
        id: I,
    ) -> Result<&HirSlotMetadata, IdResolveError> {
        match self.slots.resolve(id) {
            Ok(metadata) => Ok(metadata),
            Err(HirSlotError::Resolve(error)) => Err(error),
            Err(error) => {
                unreachable!("validated module slot failed immutable resolution: {error}")
            }
        }
    }

    /// Iterates live item slots in arena order, not authored source order.
    pub fn items(&self) -> impl ExactSizeIterator<Item = (ItemId, &HirItem)> + '_ {
        self.iter_arena(&self.arenas.items)
    }

    /// Iterates live scope slots in arena order.
    pub fn scopes(&self) -> impl ExactSizeIterator<Item = (ScopeId, &HirScope)> + '_ {
        self.iter_arena(&self.arenas.scopes)
    }

    /// Iterates live local slots in arena order.
    pub fn locals(&self) -> impl ExactSizeIterator<Item = (LocalId, &HirLocal)> + '_ {
        self.iter_arena(&self.arenas.locals)
    }

    /// Iterates live expression slots in arena order.
    pub fn expressions(&self) -> impl ExactSizeIterator<Item = (ExprId, &HirExpr)> + '_ {
        self.iter_arena(&self.arenas.expressions)
    }

    /// Iterates live statement slots in arena order.
    pub fn statements(&self) -> impl ExactSizeIterator<Item = (StmtId, &HirStmt)> + '_ {
        self.iter_arena(&self.arenas.statements)
    }

    /// Iterates live type slots in arena order.
    pub fn types(&self) -> impl ExactSizeIterator<Item = (TypeId, &HirType)> + '_ {
        self.iter_arena(&self.arenas.types)
    }

    /// Iterates live pattern slots in arena order.
    pub fn patterns(&self) -> impl ExactSizeIterator<Item = (PatternId, &HirPattern)> + '_ {
        self.iter_arena(&self.arenas.patterns)
    }

    /// Iterates live capture slots in arena order.
    pub fn captures(&self) -> impl ExactSizeIterator<Item = (CaptureId, &HirCapture)> + '_ {
        self.iter_arena(&self.arenas.captures)
    }

    /// Projects an attached syntax node to its final item identity.
    pub fn item_for_syntax(&self, syntax: SyntaxNodeId) -> Result<ItemId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    /// Projects an attached syntax node to its final scope identity.
    pub fn scope_for_syntax(&self, syntax: SyntaxNodeId) -> Result<ScopeId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    /// Projects an attached syntax node to its final local identity.
    pub fn local_for_syntax(&self, syntax: SyntaxNodeId) -> Result<LocalId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    /// Projects an attached syntax node to its final expression identity.
    pub fn expr_for_syntax(&self, syntax: SyntaxNodeId) -> Result<ExprId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    /// Projects an attached syntax node to its final statement identity.
    pub fn stmt_for_syntax(&self, syntax: SyntaxNodeId) -> Result<StmtId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    /// Projects an attached syntax node to its final type identity.
    pub fn type_for_syntax(&self, syntax: SyntaxNodeId) -> Result<TypeId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    /// Projects an attached syntax node to its final pattern identity.
    pub fn pattern_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<PatternId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    fn resolve_arena<'arena, T, I: HirTypedId>(
        &self,
        arena: &'arena ArenaSnapshot<T, I>,
        id: I,
    ) -> Result<&'arena T, IdResolveError> {
        match arena.resolve(&self.slots, id) {
            Ok(value) => Ok(value),
            Err(HirArenaError::Slot(HirSlotError::Resolve(error))) => Err(error),
            Err(error) => {
                unreachable!("validated module arena failed immutable resolution: {error}")
            }
        }
    }

    fn iter_arena<'module, T, I: HirTypedId>(
        &'module self,
        arena: &'module ArenaSnapshot<T, I>,
    ) -> ArenaIter<'module, T, I> {
        arena
            .try_iter(&self.slots)
            .expect("validated module arena remains bound to its published slot snapshot")
    }

    fn source_owner<I: HirTypedId>(&self, syntax: SyntaxNodeId) -> Result<I, HirSourceLookupError> {
        let expected = self.provenance().syntax_snapshot().lineage();
        let actual = syntax.lineage();
        if expected.database() != actual.database() {
            return Err(HirSourceLookupError::WrongSyntaxDatabase {
                expected: expected.database(),
                actual: actual.database(),
            });
        }
        if expected != actual {
            return Err(HirSourceLookupError::WrongSyntaxLineage { expected, actual });
        }

        if let Some(owner) = self.slots.prepared_source_owner::<I>(syntax) {
            self.metadata(owner)
                .expect("validated source allocation resolves in its published module");
            return Ok(owner);
        }

        match self.source_kind_for_syntax(syntax) {
            Some(actual) => Err(HirSourceLookupError::KindMismatch {
                syntax,
                expected: I::KIND,
                actual,
            }),
            None => Err(HirSourceLookupError::NotLowered {
                syntax,
                expected: I::KIND,
            }),
        }
    }

    fn source_kind_for_syntax(&self, syntax: SyntaxNodeId) -> Option<HirIdKind> {
        [
            self.slots
                .prepared_source_owner::<ItemId>(syntax)
                .map(|_| HirIdKind::Item),
            self.slots
                .prepared_source_owner::<ScopeId>(syntax)
                .map(|_| HirIdKind::Scope),
            self.slots
                .prepared_source_owner::<LocalId>(syntax)
                .map(|_| HirIdKind::Local),
            self.slots
                .prepared_source_owner::<ExprId>(syntax)
                .map(|_| HirIdKind::Expr),
            self.slots
                .prepared_source_owner::<StmtId>(syntax)
                .map(|_| HirIdKind::Stmt),
            self.slots
                .prepared_source_owner::<TypeId>(syntax)
                .map(|_| HirIdKind::Type),
            self.slots
                .prepared_source_owner::<PatternId>(syntax)
                .map(|_| HirIdKind::Pattern),
        ]
        .into_iter()
        .flatten()
        .next()
    }
}
