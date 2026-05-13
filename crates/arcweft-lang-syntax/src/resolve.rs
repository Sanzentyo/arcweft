use crate::check::EntityKind;
use crate::lower::{HirFlowItem, HirModule};
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use core::fmt;
use std::collections::HashMap;

/// Entity registry used by parser/HIR integration tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NameRegistry {
    entities: HashMap<String, EntityKind>,
}

/// Name-resolution failure for entity references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameResolutionError {
    message: String,
}

/// Builds a registry from declarations visible in one HIR module.
#[must_use]
pub fn registry_from_hir(module: &HirModule) -> NameRegistry {
    let mut registry = NameRegistry::new();
    for flow in module.flows() {
        if let Some(id) = flow.id() {
            registry = registry.with_entity(
                id.body(),
                match flow.kind() {
                    crate::ast::FlowKind::Flow => EntityKind::Flow,
                    crate::ast::FlowKind::Fragment => EntityKind::Fragment,
                },
            );
        }
        for item in flow.body() {
            register_flow_item(item, &mut registry);
        }
    }
    registry
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
        HirFlowItem::Borrow(block) => {
            for item in block.body() {
                register_flow_item(item, registry);
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
        HirFlowItem::Await(await_with) => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    register_flow_item(item, registry);
                }
            }
        }
        HirFlowItem::Stmt(_)
        | HirFlowItem::LetScope { .. }
        | HirFlowItem::Include(_)
        | HirFlowItem::Scenario { .. } => {}
    }
}

fn register_choice(choice: &crate::lower::HirChoice, registry: &mut NameRegistry) {
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
        self.entities.contains_key(id)
    }
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

impl fmt::Display for NameResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for NameResolutionError {}
