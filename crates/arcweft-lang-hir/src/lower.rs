use crate::lower_flow::{lower_flow, lower_flow_item};
use crate::model::{HirFunction, HirLowerError, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::items::{Attribute, FunctionItem, Item, TypedSyntaxTree};

/// Lowers a parsed syntax tree into HIR-facing structures.
pub fn lower_to_hir(tree: &TypedSyntaxTree) -> Result<HirModule, Vec<HirLowerError>> {
    let mut state = HirLoweringState {
        attributes: tree.attrs().to_vec(),
        ..HirLoweringState::default()
    };

    for item in tree.items() {
        state.lower_item(item);
    }
    state.finish()
}

#[derive(Default)]
struct HirLoweringState {
    attributes: Vec<Attribute>,
    flows: Vec<crate::model::HirFlow>,
    functions: Vec<HirFunction>,
    declarations: Vec<HirTopLevelDecl>,
    top_level_items: Vec<crate::model::HirFlowItem>,
    errors: Vec<HirLowerError>,
}

impl HirLoweringState {
    fn lower_item(&mut self, item: &Item) {
        match item {
            Item::Flow(flow) => match lower_flow(flow) {
                Ok(flow) => {
                    self.flows.push(flow);
                }
                Err(err) => self.errors.push(err),
            },
            Item::Function(function) => {
                self.functions.push(lower_function(function));
            }
            Item::FlowItem(item) => match lower_flow_item(item) {
                Ok(item) => {
                    self.top_level_items.push(item);
                }
                Err(err) => self.errors.push(err),
            },
            Item::Raw(raw) => {
                self.errors.push(HirLowerError::new(
                    format!("raw top-level item cannot be lowered: {}", raw.head()),
                    Some(*raw.range()),
                ));
            }
            _ => self.lower_declaration_item(item),
        }
    }

    fn lower_declaration_item(&mut self, item: &Item) {
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
            Item::Flow(_) | Item::Function(_) | Item::FlowItem(_) | Item::Raw(_) => {}
        }
    }

    fn finish(self) -> Result<HirModule, Vec<HirLowerError>> {
        if self.errors.is_empty() {
            Ok(HirModule {
                attributes: self.attributes,
                flows: self.flows,
                functions: self.functions,
                declarations: self.declarations,
                top_level_items: self.top_level_items,
            })
        } else {
            Err(self.errors)
        }
    }
}

fn lower_function(function: &FunctionItem) -> HirFunction {
    HirFunction {
        attributes: function.attrs().to_vec(),
        kind: function.kind(),
        visibility: function.visibility(),
        signature: function.signature().clone(),
        contracts: function.contracts().to_vec(),
        statements: function.body_statements().to_vec(),
        value: function.body_value().cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::lower_to_hir;
    use arcweft_lang_syntax::parser::parse_source;

    #[test]
    fn lowering_preserves_flow_attributes() {
        let tree = parse_source(
            r#"
#[allow(id::flow_module_mismatch)]
flow @flow.opening opening {
    return "done"
}
"#,
        )
        .into_typed_tree();

        let hir = lower_to_hir(&tree).expect("source lowers to HIR");
        let flow = hir.flows().first().expect("flow lowers");
        assert_eq!(flow.attributes().len(), 1);
        assert_eq!(flow.attributes()[0].name(), "allow");
        assert_eq!(
            flow.attributes()[0].args(),
            Some("id::flow_module_mismatch")
        );
        assert!(flow.has_attribute("allow"));
    }

    #[test]
    fn lowering_preserves_source_inner_attributes() {
        let tree = parse_source(
            r#"
#![generated(tool)]

flow @flow.opening opening {
    return "done"
}
"#,
        )
        .into_typed_tree();

        let hir = lower_to_hir(&tree).expect("source lowers to HIR");
        assert_eq!(hir.attributes().len(), 1);
        assert_eq!(hir.attributes()[0].name(), "generated");
        assert_eq!(hir.attributes()[0].args(), Some("tool"));
        assert!(hir.has_attribute("generated"));
    }

    #[test]
    fn lowering_preserves_dialogue_defaults_attributes() {
        let tree = parse_source(
            r#"
#[profile(note="mobile defaults")]
pub dialogue defaults @dialogue.defaults.mobile {
    rich_text {
        ruby {
            size = 10px
        }
    }
}
"#,
        )
        .into_typed_tree();

        let hir = lower_to_hir(&tree).expect("source lowers to HIR");
        let defaults = hir
            .declarations()
            .iter()
            .find_map(|decl| match decl {
                crate::model::HirTopLevelDecl::DialogueDefaults(defaults) => Some(defaults),
                _ => None,
            })
            .expect("dialogue defaults lowers");

        assert_eq!(defaults.attrs().len(), 1);
        assert_eq!(defaults.attrs()[0].name(), "profile");
        assert_eq!(defaults.attrs()[0].args(), Some("note=\"mobile defaults\""));
    }
}
