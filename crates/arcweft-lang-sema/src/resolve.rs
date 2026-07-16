use crate::env::TypeCheckEnv;
use crate::project_index::ProjectSemanticIndex;
use crate::registration::RegisteredSemanticWorld;
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use crate::types::{EntityKind, TypeKind};
use arcweft_lang_hir::model::{HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::items::EntityDeclKind;
use arcweft_source::{Diagnostic, DiagnosticSeverity};
use std::collections::HashMap;
use thiserror::Error;

/// Entity registry used by parser/HIR integration tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameRegistry {
    entities: HashMap<String, EntityKind>,
}

/// Name-resolution failure for entity references.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct NameResolutionError {
    message: String,
}

impl NameResolutionError {
    /// Builds the shared diagnostic representation for compiler, CLI, LSP, and Agent surfaces.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.message.clone()).with_code("sema.resolve")
    }
}

/// Builds a registry from declarations visible in one HIR module.
#[must_use]
pub fn registry_from_hir(module: &HirModule) -> NameRegistry {
    let mut registry = NameRegistry::new();
    for flow in module.flows() {
        if let Some(id) = flow.id() {
            registry = registry.with_entity(id.body(), EntityKind::Flow);
        }
        for item in flow.body() {
            register_flow_item(item, &mut registry);
        }
    }
    for declaration in module.declarations() {
        match declaration {
            HirTopLevelDecl::Source(source) => {
                if let Some(id) = source.item().id() {
                    registry.insert(id.body(), EntityKind::Source);
                }
            }
            HirTopLevelDecl::EntityDecl(item) => {
                registry.insert(item.id().body(), entity_decl_registry_kind(item.kind()));
                if let Some(view) = item.view_body().and_then(|body| body.view()) {
                    for input in view.text_control_inputs() {
                        registry.insert(input.canonical_body(), EntityKind::Input);
                    }
                }
            }
            HirTopLevelDecl::Entry(item) => {
                registry.insert(item.id().body(), EntityKind::Entry);
            }
            HirTopLevelDecl::Style(item) => {
                registry.insert(item.id().body(), EntityKind::Style);
            }
            HirTopLevelDecl::Test(item) => {
                if let Some(id) = item.id().as_absolute() {
                    registry.insert(id.body(), EntityKind::Test);
                }
            }
            HirTopLevelDecl::Bench(item) => {
                if let Some(id) = item.id().as_absolute() {
                    registry.insert(id.body(), EntityKind::Bench);
                }
            }
            _ => {}
        }
    }
    registry
}

/// Builds a registry from one HIR module plus externally supplied semantic symbols.
#[must_use]
pub fn registry_from_hir_and_env(module: &HirModule, env: &TypeCheckEnv) -> NameRegistry {
    let mut registry = registry_from_hir(module);
    for (name, ty) in &env.symbols {
        if let TypeKind::Ref(entity) = ty {
            registry.insert(name.as_str(), entity.kind().clone());
        }
    }
    registry
}

/// Builds a registry through the committed project semantic world.
#[must_use]
pub fn registry_from_hir_and_registered(
    module: &HirModule,
    registered: &RegisteredSemanticWorld,
) -> NameRegistry {
    registry_from_hir_and_env(module, registered.environment().typecheck_env())
}

/// Builds a registry from one HIR module plus an Agent project semantic index.
#[must_use]
pub fn registry_from_hir_and_project(
    module: &HirModule,
    project: &ProjectSemanticIndex,
) -> NameRegistry {
    let mut registry = registry_from_hir(module);
    for entity in project.entities().values() {
        registry.insert(entity.id().as_str(), entity.ty().kind().clone());
    }
    registry
}

fn entity_decl_registry_kind(kind: EntityDeclKind) -> EntityKind {
    match kind {
        EntityDeclKind::Asset => EntityKind::Asset,
        EntityDeclKind::Image => EntityKind::Image,
        EntityDeclKind::Character => EntityKind::Character,
        EntityDeclKind::View => EntityKind::View,
        EntityDeclKind::Action => EntityKind::Action,
        EntityDeclKind::Activity => EntityKind::Activity,
        EntityDeclKind::Content => EntityKind::Content,
        EntityDeclKind::Signal => EntityKind::Signal,
        EntityDeclKind::Metric => EntityKind::Metric,
        EntityDeclKind::Layer => EntityKind::Layer,
        EntityDeclKind::Voice => EntityKind::Voice,
        EntityDeclKind::Se => EntityKind::Se,
        EntityDeclKind::Bgm => EntityKind::Bgm,
        EntityDeclKind::AudioBus => EntityKind::AudioBus,
        EntityDeclKind::MixerSnapshot => EntityKind::MixerSnapshot,
        EntityDeclKind::Ducking => EntityKind::Ducking,
        EntityDeclKind::Motion => EntityKind::Motion,
        EntityDeclKind::Rig => EntityKind::Rig,
    }
}

/// Validates entity references collected from HIR against a registry.
pub fn validate_hir_references(
    module: &HirModule,
    registry: &NameRegistry,
) -> Result<(), Vec<NameResolutionError>> {
    let errors = collect_symbol_uses(module)
        .into_iter()
        .filter(|symbol| symbol.kind() == SymbolUseKind::EntityRef)
        .filter(|symbol| !registry.contains(symbol.name()))
        .map(|symbol| {
            NameResolutionError::new(format!("unresolved entity reference `{}`", symbol.name()))
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn register_flow_item(item: &HirFlowItem, registry: &mut NameRegistry) {
    match item {
        HirFlowItem::Dialogue(dialogue) => {
            if let Some(id) = dialogue.id() {
                registry.insert(id.body(), EntityKind::DialogueLine);
            }
            if let Some(text_key) = dialogue.text_key() {
                registry.insert(text_key.body(), EntityKind::Text);
            }
        }
        HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
            register_choice(choice, registry);
        }
        HirFlowItem::If(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
            for item in block.else_body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::IfLet(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
            for item in block.else_body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::Match(block) => {
            for arm in block.arms() {
                for item in arm.body() {
                    register_flow_item(item, registry);
                }
            }
        }
        HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::While(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::WhileLet(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::For(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::Select(block) => {
            for branch in block.branches() {
                for item in branch.body() {
                    register_flow_item(item, registry);
                }
            }
        }
        HirFlowItem::SourceLocale(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::Scope(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    register_flow_item(item, registry);
                }
            }
        }
        HirFlowItem::Thread(thread) => {
            for item in thread.body() {
                register_flow_item(item, registry);
            }
        }
        HirFlowItem::Stmt(_) | HirFlowItem::LetScope { .. } | HirFlowItem::Include(_) => {}
    }
}

fn register_choice(choice: &arcweft_lang_hir::model::HirChoice, registry: &mut NameRegistry) {
    if let Some(id) = choice.id() {
        registry.insert(id.body(), EntityKind::Choice);
    }
    for option in choice.options() {
        if let Some(id) = option.id() {
            registry.insert(id.body(), EntityKind::ChoiceOption);
        }
    }
}

impl NameRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an entity by public id.
    #[must_use]
    pub fn with_entity(mut self, id: impl Into<String>, kind: EntityKind) -> Self {
        self.insert(id, kind);
        self
    }

    fn insert(&mut self, id: impl Into<String>, kind: EntityKind) {
        self.entities.insert(id.into(), kind);
    }

    fn contains(&self, id: &str) -> bool {
        self.entities.contains_key(id) || is_manifest_backed_family(id)
    }
}

fn is_manifest_backed_family(id: &str) -> bool {
    let Some(family) = id.split('.').next() else {
        return false;
    };
    matches!(
        family,
        "asset" | "voice" | "se" | "bgm" | "bus" | "mix" | "duck" | "motion" | "rig" | "capture"
    )
}

impl NameResolutionError {
    fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable name-resolution failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}
