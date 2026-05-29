use crate::lower_flow::{lower_flow, lower_flow_item};
use crate::model::{HirFunction, HirLowerError, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::items::{Attribute, FunctionItem, Item, TypedSyntaxTree};

/// Lowers a parsed syntax tree into HIR-facing structures.
pub fn lower_to_hir(tree: &TypedSyntaxTree) -> Result<HirModule, Vec<HirLowerError>> {
    let mut state = HirLoweringState::default();

    for item in tree.items() {
        state.lower_item(item);
    }
    state.finish()
}

#[derive(Default)]
struct HirLoweringState {
    flows: Vec<crate::model::HirFlow>,
    functions: Vec<HirFunction>,
    declarations: Vec<HirTopLevelDecl>,
    top_level_items: Vec<crate::model::HirFlowItem>,
    errors: Vec<HirLowerError>,
    pending_attributes: Vec<Attribute>,
}

impl HirLoweringState {
    fn lower_item(&mut self, item: &Item) {
        match item {
            Item::Attribute(attribute) => {
                self.pending_attributes.push(attribute.clone());
            }
            Item::Flow(flow) => match lower_flow(flow) {
                Ok(flow) => {
                    self.flush_pending_attributes();
                    self.flows.push(flow);
                }
                Err(err) => self.errors.push(err),
            },
            Item::Function(function) => {
                self.functions.push(lower_function(
                    function,
                    std::mem::take(&mut self.pending_attributes),
                ));
            }
            Item::FlowItem(item) => match lower_flow_item(item) {
                Ok(item) => {
                    self.flush_pending_attributes();
                    self.top_level_items.push(item);
                }
                Err(err) => self.errors.push(err),
            },
            Item::Raw(raw) => {
                self.flush_pending_attributes();
                self.errors.push(HirLowerError::new(
                    format!("raw top-level item cannot be lowered: {}", raw.head()),
                    Some(*raw.range()),
                ));
            }
            _ => self.lower_declaration_item(item),
        }
    }

    fn lower_declaration_item(&mut self, item: &Item) {
        self.flush_pending_attributes();
        match item {
            Item::Callable(item) => {
                self.declarations
                    .push(HirTopLevelDecl::Callable(item.clone()));
            }
            Item::Enum(item) => {
                self.declarations.push(HirTopLevelDecl::Enum(item.clone()));
            }
            Item::EntityDecl(item) => {
                self.declarations
                    .push(HirTopLevelDecl::EntityDecl(item.clone()));
            }
            Item::Entry(item) => {
                self.declarations.push(HirTopLevelDecl::Entry(item.clone()));
            }
            Item::ExternCapability(item) => {
                self.declarations
                    .push(HirTopLevelDecl::ExternCapability(item.clone()));
            }
            Item::ExternMod(item) => {
                self.declarations
                    .push(HirTopLevelDecl::ExternMod(item.clone()));
            }
            Item::DialogueDefaults(item) => {
                self.declarations
                    .push(HirTopLevelDecl::DialogueDefaults(item.clone()));
            }
            Item::Hook(item) => {
                self.declarations.push(HirTopLevelDecl::Hook(item.clone()));
            }
            Item::Impl(item) => {
                self.declarations.push(HirTopLevelDecl::Impl(item.clone()));
            }
            Item::MemoFn(item) => {
                self.declarations
                    .push(HirTopLevelDecl::MemoFn(item.clone()));
            }
            Item::Proof(item) => {
                self.declarations.push(HirTopLevelDecl::Proof(item.clone()));
            }
            Item::TrustedAxiom(item) => {
                self.declarations
                    .push(HirTopLevelDecl::TrustedAxiom(item.clone()));
            }
            Item::Test(item) => {
                self.declarations.push(HirTopLevelDecl::Test(item.clone()));
            }
            Item::Bench(item) => {
                self.declarations.push(HirTopLevelDecl::Bench(item.clone()));
            }
            Item::Parser(item) => {
                self.declarations
                    .push(HirTopLevelDecl::Parser(item.clone()));
            }
            Item::Source(item) => {
                self.declarations
                    .push(HirTopLevelDecl::Source(item.clone()));
            }
            Item::State(item) => {
                self.declarations.push(HirTopLevelDecl::State(item.clone()));
            }
            Item::Struct(item) => {
                self.declarations
                    .push(HirTopLevelDecl::Struct(item.clone()));
            }
            Item::Trait(item) => {
                self.declarations.push(HirTopLevelDecl::Trait(item.clone()));
            }
            Item::TypeAlias(item) => {
                self.declarations
                    .push(HirTopLevelDecl::TypeAlias(item.clone()));
            }
            Item::Attribute(_)
            | Item::Flow(_)
            | Item::Function(_)
            | Item::FlowItem(_)
            | Item::Raw(_) => {}
        }
    }

    fn finish(mut self) -> Result<HirModule, Vec<HirLowerError>> {
        self.flush_pending_attributes();
        if self.errors.is_empty() {
            Ok(HirModule {
                flows: self.flows,
                functions: self.functions,
                declarations: self.declarations,
                top_level_items: self.top_level_items,
            })
        } else {
            Err(self.errors)
        }
    }

    fn flush_pending_attributes(&mut self) {
        self.declarations.extend(
            self.pending_attributes
                .drain(..)
                .map(HirTopLevelDecl::Attribute),
        );
    }
}

fn lower_function(function: &FunctionItem, attributes: Vec<Attribute>) -> HirFunction {
    HirFunction {
        attributes,
        kind: function.kind(),
        visibility: function.visibility(),
        signature: function.signature().clone(),
        contracts: function.contracts().to_vec(),
        statements: function.body_statements().to_vec(),
        value: function.body_value().cloned(),
    }
}
