use crate::lower_flow::{lower_flow, lower_flow_item};
use crate::model::{HirFunction, HirLowerError, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::items::{FunctionItem, Item, TypedSyntaxTree};

/// Lowers a parsed syntax tree into HIR-facing structures.
pub fn lower_to_hir(tree: &TypedSyntaxTree) -> Result<HirModule, Vec<HirLowerError>> {
    let mut flows = Vec::new();
    let mut functions = Vec::new();
    let mut declarations = Vec::new();
    let mut top_level_items = Vec::new();
    let mut errors = Vec::new();

    for item in tree.items() {
        match item {
            Item::Flow(flow) => match lower_flow(flow) {
                Ok(flow) => flows.push(flow),
                Err(err) => errors.push(err),
            },
            Item::Function(function) => functions.push(lower_function(function)),
            Item::FlowItem(item) => match lower_flow_item(item) {
                Ok(item) => top_level_items.push(item),
                Err(err) => errors.push(err),
            },
            Item::Attribute(item) => {
                declarations.push(HirTopLevelDecl::Attribute(item.clone()));
            }
            Item::Callable(item) => {
                declarations.push(HirTopLevelDecl::Callable(item.clone()));
            }
            Item::Enum(item) => {
                declarations.push(HirTopLevelDecl::Enum(item.clone()));
            }
            Item::EntityDecl(item) => {
                declarations.push(HirTopLevelDecl::EntityDecl(item.clone()));
            }
            Item::ExternMod(item) => {
                declarations.push(HirTopLevelDecl::ExternMod(item.clone()));
            }
            Item::DialogueDefaults(item) => {
                declarations.push(HirTopLevelDecl::DialogueDefaults(item.clone()));
            }
            Item::Hook(item) => {
                declarations.push(HirTopLevelDecl::Hook(item.clone()));
            }
            Item::Impl(item) => {
                declarations.push(HirTopLevelDecl::Impl(item.clone()));
            }
            Item::MemoFn(item) => {
                declarations.push(HirTopLevelDecl::MemoFn(item.clone()));
            }
            Item::Proof(item) => {
                declarations.push(HirTopLevelDecl::Proof(item.clone()));
            }
            Item::TrustedAxiom(item) => {
                declarations.push(HirTopLevelDecl::TrustedAxiom(item.clone()));
            }
            Item::Test(item) => {
                declarations.push(HirTopLevelDecl::Test(item.clone()));
            }
            Item::Bench(item) => {
                declarations.push(HirTopLevelDecl::Bench(item.clone()));
            }
            Item::Parser(item) => {
                declarations.push(HirTopLevelDecl::Parser(item.clone()));
            }
            Item::Source(item) => {
                declarations.push(HirTopLevelDecl::Source(item.clone()));
            }
            Item::State(item) => {
                declarations.push(HirTopLevelDecl::State(item.clone()));
            }
            Item::Struct(item) => {
                declarations.push(HirTopLevelDecl::Struct(item.clone()));
            }
            Item::Trait(item) => {
                declarations.push(HirTopLevelDecl::Trait(item.clone()));
            }
            Item::TypeAlias(item) => {
                declarations.push(HirTopLevelDecl::TypeAlias(item.clone()));
            }
            Item::Raw(raw) => errors.push(HirLowerError::new(
                format!("raw top-level item cannot be lowered: {}", raw.head()),
                Some(*raw.range()),
            )),
        }
    }

    if errors.is_empty() {
        Ok(HirModule {
            flows,
            functions,
            declarations,
            top_level_items,
        })
    } else {
        Err(errors)
    }
}

fn lower_function(function: &FunctionItem) -> HirFunction {
    HirFunction {
        kind: function.kind(),
        signature: function.signature().clone(),
        contracts: function.contracts().to_vec(),
        statements: function.body_statements().to_vec(),
        value: function.body_value().cloned(),
    }
}
