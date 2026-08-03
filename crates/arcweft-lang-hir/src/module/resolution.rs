//! Typed immutable resolution over one accepted module lease.

use arcweft_lang_syntax::attachment::SyntaxNodeId;
use arcweft_source::SourceSpan;

use crate::arena::{ArenaIter, ArenaSnapshot, HirArenaError};
use crate::expr::HirExpr;
use crate::identity::{
    CaptureId, ExprId, HirIdKind, HirTypedId, IdResolveError, ItemId, LocalId, PatternId, ScopeId,
    StmtId, TypeId,
};
use crate::item::HirItem;
use crate::leaf::HirName;
use crate::pattern::HirPattern;
use crate::scope::{HirCapture, HirLocal, HirScope, LocalLookup};
use crate::slot::{HirSlotError, HirSlotMetadata};
use crate::source_index::HirSourceLookupError;
use crate::stmt::HirStmt;
use crate::type_ref::HirType;

use super::{HirLocalResolver, HirModule};

impl HirModule {
    pub(crate) fn resolve_item(&self, id: ItemId) -> Result<&HirItem, IdResolveError> {
        self.resolve_arena(&self.arenas.items, id)
    }

    pub(crate) fn resolve_scope(&self, id: ScopeId) -> Result<&HirScope, IdResolveError> {
        self.resolve_arena(&self.arenas.scopes, id)
    }

    pub(crate) fn resolve_local(&self, id: LocalId) -> Result<&HirLocal, IdResolveError> {
        self.resolve_arena(&self.arenas.locals, id)
    }

    /// Resolves the nearest source-visible lexical local before one exact use
    /// site without reconstructing a binding point from authored name text.
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
            .lookup(scope, name, before.range().start())
            .expect("accepted module scope graph remains acyclic and complete"))
    }

    pub(crate) fn resolve_expr(&self, id: ExprId) -> Result<&HirExpr, IdResolveError> {
        self.resolve_arena(&self.arenas.expressions, id)
    }

    pub(crate) fn resolve_stmt(&self, id: StmtId) -> Result<&HirStmt, IdResolveError> {
        self.resolve_arena(&self.arenas.statements, id)
    }

    pub(crate) fn resolve_type(&self, id: TypeId) -> Result<&HirType, IdResolveError> {
        self.resolve_arena(&self.arenas.types, id)
    }

    pub(crate) fn resolve_pattern(&self, id: PatternId) -> Result<&HirPattern, IdResolveError> {
        self.resolve_arena(&self.arenas.patterns, id)
    }

    pub(crate) fn resolve_capture(&self, id: CaptureId) -> Result<&HirCapture, IdResolveError> {
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

    pub(crate) fn items(&self) -> ArenaIter<'_, HirItem, ItemId> {
        self.iter_arena(&self.arenas.items)
    }

    pub(crate) fn scopes(&self) -> ArenaIter<'_, HirScope, ScopeId> {
        self.iter_arena(&self.arenas.scopes)
    }

    pub(crate) fn locals(&self) -> ArenaIter<'_, HirLocal, LocalId> {
        self.iter_arena(&self.arenas.locals)
    }

    pub(crate) fn expressions(&self) -> ArenaIter<'_, HirExpr, ExprId> {
        self.iter_arena(&self.arenas.expressions)
    }

    pub(crate) fn statements(&self) -> ArenaIter<'_, HirStmt, StmtId> {
        self.iter_arena(&self.arenas.statements)
    }

    pub(crate) fn types(&self) -> ArenaIter<'_, HirType, TypeId> {
        self.iter_arena(&self.arenas.types)
    }

    pub(crate) fn patterns(&self) -> ArenaIter<'_, HirPattern, PatternId> {
        self.iter_arena(&self.arenas.patterns)
    }

    pub(crate) fn captures(&self) -> ArenaIter<'_, HirCapture, CaptureId> {
        self.iter_arena(&self.arenas.captures)
    }

    pub(crate) fn item_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<ItemId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    pub(crate) fn scope_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<ScopeId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    pub(crate) fn local_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<LocalId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    pub(crate) fn expr_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<ExprId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    pub(crate) fn stmt_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<StmtId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    pub(crate) fn type_for_syntax(
        &self,
        syntax: SyntaxNodeId,
    ) -> Result<TypeId, HirSourceLookupError> {
        self.source_owner(syntax)
    }

    pub(crate) fn pattern_for_syntax(
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
