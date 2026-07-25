use crate::entry::{HirEntryDecl, HirEntryItem};
use crate::lower_flow::lower_flow;
use crate::model::{HirFunction, HirLowerError, HirModule, HirSource, HirTopLevelDecl};
use crate::style::{HirStyleDecl, HirStylePatch};
use crate::view_part::HirViewPartOwner;
use arcweft_lang_syntax::ast::{
    items::{
        Attribute, EntityDeclItem, EntryDeclItem, EntryItem, FunctionItem, Item, TypedSyntaxTree,
    },
    module_path::CanonicalModulePath,
};
use arcweft_source::SourceDocument;

/// Lowers a parsed syntax tree into HIR-facing structures.
pub fn lower_to_hir(tree: &TypedSyntaxTree) -> Result<HirModule, Vec<HirLowerError>> {
    let module_path = tree
        .module()
        .map(|module| {
            module
                .module_path()
                .and_then(|path| path.resolve_declaration_for(&CanonicalModulePath::crate_root()))
                .map_err(|error| {
                    HirLowerError::new(
                        format!("module path cannot be lowered: {error}"),
                        Some(*module.range()),
                    )
                })
        })
        .transpose()
        .map_err(|error| vec![error])?;
    let mut state = HirLoweringState {
        attributes: tree.attrs().to_vec(),
        uses: tree.uses().to_vec(),
        module_path,
        source: tree.source().to_owned(),
        source_len: Some(tree.source().len()),
        top_level_ranges: tree.items().iter().filter_map(Item::range).collect(),
        ..HirLoweringState::default()
    };

    for item in tree.items() {
        state.lower_item(item);
    }
    state.finish()
}

/// Lowers one exact source document and retains revision-bound project spans.
pub fn lower_document_to_hir(
    document: &SourceDocument,
    tree: &TypedSyntaxTree,
) -> Result<HirModule, Vec<HirLowerError>> {
    if tree.source() != document.text() {
        return Err(vec![HirLowerError::new(
            "typed syntax tree does not belong to the supplied source document",
            None,
        )]);
    }
    let mut hir = lower_to_hir(tree)?;
    hir.bind_source_document(document)
        .map_err(|error| vec![error])?;
    Ok(hir)
}

#[derive(Default)]
struct HirLoweringState {
    attributes: Vec<Attribute>,
    uses: Vec<arcweft_lang_syntax::ast::common::UseItem>,
    module_path: Option<CanonicalModulePath>,
    source: String,
    source_len: Option<usize>,
    top_level_ranges: Vec<arcweft_lang_syntax::ast::common::TextRange>,
    flows: Vec<crate::model::HirFlow>,
    functions: Vec<HirFunction>,
    declarations: Vec<HirTopLevelDecl>,
    style_patches: Vec<HirStylePatch>,
    view_parts: Vec<HirViewPartOwner>,
    errors: Vec<HirLowerError>,
}

impl HirLoweringState {
    fn lower_item(&mut self, item: &Item) {
        match item {
            Item::Flow(flow) => match lower_flow(flow) {
                Ok(mut flow) => {
                    flow.module_path.clone_from(&self.module_path);
                    self.flows.push(flow);
                }
                Err(err) => self.errors.push(err),
            },
            Item::Function(function) => {
                self.functions
                    .push(lower_function(function, self.module_path.clone()));
            }
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
            Item::Enum(item) => {
                self.declarations.push(HirTopLevelDecl::Enum(item.clone()));
            }
            Item::EntityDecl(item) => {
                self.lower_entity_declaration(item);
            }
            Item::Entry(item) => {
                self.declarations.push(HirTopLevelDecl::Entry(lower_entry(
                    item,
                    self.module_path.clone(),
                )));
            }
            Item::ExternCapability(item) => {
                self.declarations
                    .push(HirTopLevelDecl::ExternCapability(item.clone()));
            }
            Item::Impl(item) => {
                self.declarations.push(HirTopLevelDecl::Impl(item.clone()));
            }
            Item::Proof(item) => {
                self.declarations.push(HirTopLevelDecl::Proof(item.clone()));
            }
            Item::Test(item) => {
                self.declarations.push(HirTopLevelDecl::Test(item.clone()));
            }
            Item::Bench(item) => {
                self.declarations.push(HirTopLevelDecl::Bench(item.clone()));
            }
            Item::Source(item) => {
                self.declarations
                    .push(HirTopLevelDecl::Source(HirSource::new(
                        item.clone(),
                        self.module_path.clone(),
                    )));
            }
            Item::Style(item) => {
                self.declarations
                    .push(HirTopLevelDecl::Style(HirStyleDecl::from_syntax(
                        item,
                        &self.source,
                    )));
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
            Item::Flow(_) | Item::Function(_) | Item::Raw(_) => {}
        }
    }

    fn lower_entity_declaration(&mut self, item: &EntityDeclItem) {
        if let Some(view) = item.view_body().and_then(|body| body.view()) {
            self.view_parts.extend(HirViewPartOwner::from_syntax(
                self.module_path.clone(),
                item,
                view,
            ));
            for patch in view.style_patches() {
                let Ok(ordinal) = u32::try_from(self.style_patches.len()) else {
                    self.errors.push(HirLowerError::new(
                        "too many inline style patches",
                        Some(patch.range()),
                    ));
                    break;
                };
                self.style_patches
                    .push(HirStylePatch::from_syntax(ordinal, patch));
            }
        }
        self.declarations
            .push(HirTopLevelDecl::EntityDecl(item.clone()));
    }

    fn finish(self) -> Result<HirModule, Vec<HirLowerError>> {
        if self.errors.is_empty() {
            let module_path = self
                .module_path
                .unwrap_or_else(CanonicalModulePath::crate_root);
            let declaration_modules = vec![module_path.clone(); self.declarations.len()];
            Ok(HirModule {
                module_path,
                attributes: self.attributes,
                uses: self.uses,
                source_len: self.source_len,
                top_level_ranges: self.top_level_ranges,
                flows: self.flows,
                functions: self.functions,
                declarations: self.declarations,
                declaration_modules,
                style_patches: self.style_patches,
                view_parts: self.view_parts,
                source_map: None,
            })
        } else {
            Err(self.errors)
        }
    }
}

fn lower_entry(entry: &EntryDeclItem, module_path: Option<CanonicalModulePath>) -> HirEntryDecl {
    let items = entry
        .items()
        .iter()
        .map(|item| match item {
            EntryItem::StateType {
                ty,
                value_range,
                range,
            } => HirEntryItem::StateType {
                ty: ty.clone(),
                value_range: *value_range,
                range: *range,
            },
            EntryItem::Initializer {
                path,
                value_range,
                range,
            } => HirEntryItem::Initializer {
                path: path.clone(),
                value_range: *value_range,
                range: *range,
            },
            EntryItem::EventType {
                ty,
                value_range,
                range,
            } => HirEntryItem::EventType {
                ty: ty.clone(),
                value_range: *value_range,
                range: *range,
            },
            EntryItem::Reducer {
                path,
                value_range,
                range,
            } => HirEntryItem::Reducer {
                path: path.clone(),
                value_range: *value_range,
                range: *range,
            },
            EntryItem::Controller {
                path,
                value_range,
                range,
            } => HirEntryItem::Controller {
                path: path.clone(),
                value_range: *value_range,
                range: *range,
            },
            EntryItem::Goto(target) => HirEntryItem::Goto(target.clone()),
            EntryItem::Route {
                method,
                path,
                target,
                bindings,
            } => HirEntryItem::Route {
                method: method.clone(),
                path: path.clone(),
                target: target.clone(),
                bindings: bindings.clone(),
            },
            EntryItem::Option { name, value } => HirEntryItem::Option {
                name: name.clone(),
                value: value.clone(),
            },
            EntryItem::Raw(raw) => HirEntryItem::Raw(raw.clone()),
        })
        .collect();
    HirEntryDecl::new(
        module_path,
        entry.kind().clone(),
        entry.visibility(),
        entry.id().clone(),
        items,
        *entry.range(),
    )
}

fn lower_function(
    function: &FunctionItem,
    module_path: Option<CanonicalModulePath>,
) -> HirFunction {
    HirFunction {
        attributes: function.attrs().to_vec(),
        documentation: function.doc().cloned(),
        module_path,
        visibility: function.visibility(),
        signature: function.signature().clone(),
        signature_source: function.signature_source().clone(),
        contracts: function.contracts().to_vec(),
        statements: function.body_statements().to_vec(),
        value: function.body_value().cloned(),
        range: *function.range(),
    }
}

#[cfg(test)]
mod tests {
    use super::lower_to_hir;
    use arcweft_lang_syntax::{
        ast::flow::Stmt,
        expr::{CallArg, CallExpr, Expr, ParenthesizedCalleeSyntax},
        parser::parse_source,
    };

    fn first_function_statement_call(hir: &crate::model::HirModule) -> &CallExpr {
        let statement = hir
            .functions()
            .first()
            .expect("fixture lowers one function")
            .statements()
            .first()
            .expect("fixture lowers one statement");
        match statement {
            Stmt::Expr {
                expr: Expr::Call(call),
                ..
            } => call,
            other => panic!("expected associated call statement, found {other:?}"),
        }
    }

    #[test]
    fn associated_callee_survives_module_clone() {
        let fixtures = [
            "String.with_capacity(64)",
            "Bytes.with_capacity(4096)",
            "Vec.with_capacity(8)",
            "Vec<I32>.with_capacity(8)",
            "Vec<T>.with_capacity(8)",
            "pkg::types::Buffer<I32>.with_capacity(8)",
            "Alias<I32>.with_capacity(8)",
            "Vec<I32>::with_capacity(8)",
            "Vec<T>::with_capacity(8)",
            "Vec::<I32>.with_capacity(8)",
            "Vec::<I32>::with_capacity(8)",
            "Vec<Option<Result<T,E>>>.with_capacity(8)",
        ];

        for fixture in fixtures {
            let source = format!("fn main() -> Unit {{\n    {fixture}\n    ()\n}}\n");
            let parsed = parse_source(source);
            assert_eq!(parsed.errors(), &[], "{fixture}");
            let hir = lower_to_hir(parsed.typed_tree()).expect("associated source lowers");
            let cloned = hir.clone();
            let original_call = first_function_statement_call(&hir);
            let cloned_call = first_function_statement_call(&cloned);
            assert_eq!(original_call, cloned_call, "{fixture}");

            let original_surface = original_call
                .parenthesized_syntax()
                .expect("associated call remains parenthesized");
            let cloned_surface = cloned_call
                .parenthesized_syntax()
                .expect("cloned associated call remains parenthesized");
            assert!(matches!(
                original_surface.callee(),
                ParenthesizedCalleeSyntax::PathMember(_)
            ));
            assert_eq!(original_surface.callee(), cloned_surface.callee());

            let original = original_call
                .path_member_callee_syntax()
                .expect("original typed callee");
            let cloned = cloned_call
                .path_member_callee_syntax()
                .expect("cloned typed callee");
            assert_eq!(original.receiver().value(), cloned.receiver().value());
            assert_eq!(
                original.receiver().source().nodes(),
                cloned.receiver().source().nodes()
            );
            assert_eq!(
                original.receiver().source().lexemes(),
                cloned.receiver().source().lexemes()
            );
            assert_eq!(original.separator(), cloned.separator());
            assert_eq!(original.member(), cloned.member());
            assert_eq!(original.member_range(), cloned.member_range());
            assert_eq!(original.whole(), cloned.whole());
            assert_eq!(original_call.callee_range(), cloned_call.callee_range());
            assert_eq!(original_call.range(), cloned_call.range());
        }
    }

    #[test]
    fn associated_call_has_no_parallel_hir_call_enum() {
        let source = "fn main() -> Unit {\n    Vec<I32>.with_capacity(8)\n    ()\n}\n";
        let parsed = parse_source(source);
        assert_eq!(parsed.errors(), &[]);
        let hir = lower_to_hir(parsed.typed_tree()).expect("associated source lowers");
        let function = hir.functions().first().expect("function lowers");
        let calls = function
            .statements()
            .iter()
            .filter_map(|statement| match statement {
                Stmt::Expr {
                    expr: Expr::Call(call),
                    ..
                } => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.parenthesized_syntax().is_some())
                .count(),
            1
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.path_member_callee_syntax().is_some())
                .count(),
            1
        );
        assert!(matches!(
            calls[0]
                .parenthesized_syntax()
                .expect("one parenthesized carrier")
                .callee(),
            ParenthesizedCalleeSyntax::PathMember(_)
        ));
        assert!(!matches!(calls[0].callee(), Expr::Call(_)));
    }

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
    fn lowering_preserves_compact_numeric_spread_literal_ranges() {
        let source = r"
fn main() -> Unit {
    spread_choice([1i32, 22i32]...)
    ()
}
";
        let tree = parse_source(source).into_typed_tree();
        let hir = lower_to_hir(&tree).expect("numeric spread source lowers to HIR");
        let statement = hir.functions()[0]
            .statements()
            .first()
            .expect("spread call statement");
        let Stmt::Expr {
            expr: Expr::Call(call),
            ..
        } = statement
        else {
            panic!("first statement must remain a typed call")
        };
        let [CallArg::Spread { value }] = call.args() else {
            panic!("call must retain one spread argument")
        };
        let Expr::NumericBracketSeq(sequence) = value.as_ref() else {
            panic!("spread value must retain the compact numeric sequence")
        };

        let first = sequence.literal_range(0).expect("first literal range");
        let second = sequence.literal_range(1).expect("second literal range");
        assert_eq!(&source[first.as_range()], "1i32");
        assert_eq!(&source[second.as_range()], "22i32");
        assert!(first.end() < second.start());
    }

    #[test]
    fn lowering_rejects_recovery_only_project_root_items() {
        let parsed = parse_source(
            r"
alice: Hello[p]

flow @flow.opening opening {
    return
}
",
        );

        assert_eq!(parsed.errors().len(), 1);
        assert_eq!(parsed.errors()[0].code(), "syntax.parse");
        assert_eq!(parsed.errors()[0].message(), "unexpected top-level item");
        assert!(matches!(
            parsed.typed_tree().items(),
            [
                arcweft_lang_syntax::ast::items::Item::Raw(_),
                arcweft_lang_syntax::ast::items::Item::Flow(_)
            ]
        ));

        let errors =
            lower_to_hir(parsed.typed_tree()).expect_err("recovery-only root item must not lower");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message(),
            "raw top-level item cannot be lowered: alice: Hello[p]"
        );
    }

    #[test]
    fn lowering_preserves_speaker_surface_only_for_authored_colon_sugar() {
        let source = r"flow opening {
    alice(voice=auto): Hello[p]
    alice.say()[Again[p]]
}
";
        let parsed = parse_source(source);
        assert_eq!(parsed.errors(), &[]);

        let hir = lower_to_hir(parsed.typed_tree()).expect("dialogue source lowers");
        let dialogues = hir.flows()[0]
            .body()
            .iter()
            .filter_map(|item| match item {
                crate::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(dialogues.len(), 2);

        let surface = dialogues[0]
            .speaker_surface()
            .expect("colon-style speaker line keeps parser provenance");
        assert_eq!(
            &source[surface.source_line_range().as_range()],
            "    alice(voice=auto): Hello[p]"
        );
        assert_eq!(
            &source[surface.head_range().as_range()],
            "alice(voice=auto)"
        );
        assert_eq!(
            &source[surface.arguments_range().unwrap().as_range()],
            "voice=auto"
        );
        assert_eq!(
            &source[surface.inline_content_range().unwrap().as_range()],
            "Hello[p]"
        );
        assert_eq!(dialogues[1].speaker_surface(), None);
    }

    #[test]
    fn lowering_rejects_wrong_dialogue_id_families() {
        for (line, expected) in [
            (
                "alice(id=@text.not_a_line): Bad[p]",
                "dialogue line ID must use the `say` family",
            ),
            (
                "alice(text_key=@say.not_a_text_key): Bad[p]",
                "dialogue text key must use the `text` family",
            ),
        ] {
            let source = format!("flow @flow.opening opening {{\n    {line}\n}}\n");
            let parsed = parse_source(&source);
            assert_eq!(parsed.errors(), &[], "source for {line:?}");
            let errors = lower_to_hir(parsed.typed_tree()).expect_err("wrong family must fail");
            assert!(
                errors.iter().any(|error| error.message() == expected),
                "expected {expected:?} for {line:?}, got {errors:?}"
            );
        }
    }
}
