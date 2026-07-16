use crate::lower_flow::{lower_flow, lower_flow_item};
use crate::model::{HirAgent, HirFunction, HirLowerError, HirModule, HirSource, HirTopLevelDecl};
use crate::style::{HirStyleDecl, HirStylePatch};
use crate::view_part::HirViewPartOwner;
use arcweft_lang_syntax::ast::{
    items::{AgentItem, Attribute, EntityDeclItem, FunctionItem, Item, TypedSyntaxTree},
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
    source_len: Option<usize>,
    top_level_ranges: Vec<arcweft_lang_syntax::ast::common::TextRange>,
    flows: Vec<crate::model::HirFlow>,
    functions: Vec<HirFunction>,
    agents: Vec<HirAgent>,
    declarations: Vec<HirTopLevelDecl>,
    style_patches: Vec<HirStylePatch>,
    view_parts: Vec<HirViewPartOwner>,
    top_level_items: Vec<crate::model::HirFlowItem>,
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
            Item::Agent(agent) => {
                self.agents
                    .push(lower_agent(agent, self.module_path.clone()));
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
                self.lower_entity_declaration(item);
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
                    .push(HirTopLevelDecl::Style(HirStyleDecl::from(item)));
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
            Item::Flow(_)
            | Item::Function(_)
            | Item::Agent(_)
            | Item::FlowItem(_)
            | Item::Raw(_) => {}
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
            Ok(HirModule {
                module_path: self
                    .module_path
                    .unwrap_or_else(CanonicalModulePath::crate_root),
                attributes: self.attributes,
                uses: self.uses,
                source_len: self.source_len,
                top_level_ranges: self.top_level_ranges,
                flows: self.flows,
                functions: self.functions,
                agents: self.agents,
                declarations: self.declarations,
                style_patches: self.style_patches,
                view_parts: self.view_parts,
                top_level_items: self.top_level_items,
                source_map: None,
            })
        } else {
            Err(self.errors)
        }
    }
}

fn lower_agent(agent: &AgentItem, module_path: Option<CanonicalModulePath>) -> HirAgent {
    HirAgent {
        attributes: agent.attrs().to_vec(),
        module_path,
        item: agent.clone(),
    }
}

fn lower_function(
    function: &FunctionItem,
    module_path: Option<CanonicalModulePath>,
) -> HirFunction {
    HirFunction {
        attributes: function.attrs().to_vec(),
        module_path,
        kind: function.kind(),
        visibility: function.visibility(),
        signature: function.signature().clone(),
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
        ast::ids::EntityRef,
        parser::{ParseOptions, SourceDialect, parse_document, parse_source},
    };

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
    fn lowering_preserves_agent_items() {
        let parsed = parse_document(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe }
{
    observe()
}
",
            ParseOptions {
                source_dialect: SourceDialect::Agent,
            },
        );
        assert_eq!(parsed.errors(), &[]);

        let module = lower_to_hir(parsed.typed_tree()).expect("agent lowers");

        assert_eq!(module.agents().len(), 1);
        let agent = &module.agents()[0];
        assert!(agent.has_attribute("agent"));
        assert_eq!(agent.item().name(), "opening_smoke");
        assert_eq!(
            agent.item().id().map(EntityRef::body),
            Some("agent.opening_smoke")
        );
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
pub dialogue defaults @dialogue.mobile {
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
