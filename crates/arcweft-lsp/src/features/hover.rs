use crate::documents::DocumentSnapshot;
use crate::features::character_metadata::character_hover_markdown;
use crate::features::dialogue_view_metadata::{DialogueViewTypeMetadata, dialogue_view_types};
use crate::profiles::LspProfile;
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::ItemId,
    item::HirItemKind,
    module::HirModule,
    source_index::{
        HirCallableSourceOwner, HirCallableSourceRole, HirExprSourceRole, HirFlowSourceRole,
        HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
    },
};
use arcweft_lang_sema::{
    callable::{CheckedCallableSourceCategory, CheckedCallableSourceKey},
    effects::EffectSet,
    final_analysis::{CheckedExpressionResolution, FinalSemanticAnalysis},
    types::TypeKind,
};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_source::SourceSpan;
use arcweft_verify_lsp::{LspPositionMapper, profile_hover};
use lsp_types::{Hover, HoverContents, MarkedString, Position};
use std::sync::Arc;

/// Computes hover text for the word under the cursor.
pub fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    position: Position,
) -> Option<Hover> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
    let word = word_at_position_range(document, position);
    if let Some((word, word_range)) = word.as_ref()
        && let Some(hover) = callable_effect_row_hover(profile, document, word, *word_range)
    {
        return Some(hover);
    }
    if let Some(hover) = crate::features::entry_roles::hover(profile, document, offset) {
        return Some(hover);
    }
    if let Some((word, _)) = word.as_ref()
        && let Some(hover) = dialogue_view_model_hover(profile, document, word)
    {
        return Some(hover);
    }
    if let Some(hover) = crate::features::nominal_types::hover(profile, document, offset) {
        return Some(hover);
    }
    if let Some(hover) = closure_effect_row_hover(profile, document, offset) {
        return Some(hover);
    }
    if let Some(hover) = dialogue_application_hover(profile, document, offset) {
        return Some(hover);
    }
    let (word, word_range) = word?;
    let expected_character_type = word
        .starts_with('.')
        .then(|| character_nominal_type_at(profile, document, word_range))
        .flatten();
    if let Some(text) = character_hover_markdown(profile, &word, expected_character_type.as_ref()) {
        return Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(text)),
            range: None,
        });
    }
    if let Some(hover) = dialogue_view_hover(profile, document, &word) {
        return Some(hover);
    }
    profile_hover(&profile.context(), &word)
}

fn dialogue_application_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    let (module, analysis) = accepted_module_and_analysis(profile, document)?;
    analysis
        .expressions()
        .filter_map(|(owner, checked)| {
            if owner.module() != module.module_id() {
                return None;
            }
            let HirExprKind::DialogueContentApplication(_) =
                module.resolve_expr(owner).ok()?.kind()
            else {
                return None;
            };
            let CheckedExpressionResolution::DialogueApplication { target, .. } =
                checked.resolution()
            else {
                return None;
            };
            let target_range = source_range_for_query(
                module.as_ref(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::Target,
                },
            )?;
            if offset < target_range.start() || offset >= target_range.end() {
                return None;
            }
            let character = target
                .character()
                .exact()
                .map_or_else(|| "any character".to_owned(), |id| format!("@{}", id.as_str()));
            Some((target_range, character))
        })
        .min_by_key(|(range, _)| range.end() - range.start())
        .map(|(range, character)| Hover {
            contents: HoverContents::Scalar(MarkedString::String(format!(
                "CharacterDialogue content application\n\ncharacter: `{character}`\n\nresult: `DialogueLine`"
            ))),
            range: Some(
                document
                    .line_index()
                    .range_from_byte_span(range.start(), range.end()),
            ),
        })
}

fn character_nominal_type_at(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word_range: TextRange,
) -> Option<TypeKind> {
    let (module, analysis) = accepted_module_and_analysis(profile, document)?;
    analysis
        .expressions()
        .filter(|(id, _)| {
            source_range_for_query(
                module.as_ref(),
                HirSourceQuery::Expr {
                    owner: *id,
                    role: HirExprSourceRole::Whole,
                },
            )
            .is_some_and(|range| {
                range.start() <= word_range.start() && word_range.end() <= range.end()
            })
        })
        .filter_map(|(id, checked)| {
            let range = source_range_for_query(
                module.as_ref(),
                HirSourceQuery::Expr {
                    owner: id,
                    role: HirExprSourceRole::Whole,
                },
            )?;
            checked
                .ty()
                .character_nominal()
                .is_some()
                .then_some(checked.ty())
                .map(|ty| (range.end() - range.start(), ty.clone()))
        })
        .min_by_key(|(span, _)| *span)
        .map(|(_, ty)| ty)
}

fn dialogue_view_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word: &str,
) -> Option<Hover> {
    if let Some(hover) = dialogue_view_model_hover(profile, document, word) {
        return Some(hover);
    }
    for model in dialogue_view_types(profile, Some(document)) {
        if let Some((field, ty)) = DialogueViewTypeMetadata::character_fields()
            .into_iter()
            .find(|(field, _)| *field == word)
        {
            let ty = match ty {
                arcweft_lang_sema::types::TypeKind::Named(name) => name,
                other => format!("{other:?}"),
            };
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "DialogueCharacter.{field}: {ty}\n\nRuntime-supplied nested Character field."
                ))),
                range: None,
            });
        }
        if let Some((field, ty)) = DialogueViewTypeMetadata::fields()
            .into_iter()
            .find(|(field, _)| *field == word)
        {
            let ty = match ty {
                arcweft_lang_sema::types::TypeKind::Named(name) => name,
                other => format!("{other:?}"),
            };
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "{}.{}: {ty}\n\nRuntime-supplied dialogue View field.",
                    model.name, field
                ))),
                range: None,
            });
        }
    }
    None
}

fn dialogue_view_model_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word: &str,
) -> Option<Hover> {
    let model = dialogue_view_types(profile, Some(document))
        .into_iter()
        .find(|model| model.name == word)?;
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!(
            "Dialogue View input model\n\n{}",
            model.declaration()
        ))),
        range: None,
    })
}

fn closure_effect_row_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    let (module, analysis) = accepted_module_and_analysis(profile, document)?;
    let target = closure_effect_hover_target(module.as_ref(), analysis.as_ref(), offset)?;
    let effects = analysis
        .checked_callables()
        .closure_at_source(&target.source)
        .ok()?
        .concrete();
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(checked_effect_hover_text(
            "closure expression",
            effects,
        ))),
        range: Some(
            document
                .line_index()
                .range_from_byte_span(target.header_range.start(), target.header_range.end()),
        ),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClosureEffectHoverTarget {
    source: SourceSpan,
    header_range: TextRange,
}

fn closure_effect_hover_target(
    module: &HirModule,
    analysis: &FinalSemanticAnalysis,
    offset: usize,
) -> Option<ClosureEffectHoverTarget> {
    analysis
        .expressions()
        .filter_map(|(id, _)| {
            if id.module() != module.module_id()
                || !matches!(
                    module.resolve_expr(id).ok()?.kind(),
                    HirExprKind::Closure(_)
                )
            {
                return None;
            }
            let lookup = module
                .source_site(
                    module.provenance().source_identity(),
                    HirSourceQuery::Expr {
                        owner: id,
                        role: HirExprSourceRole::Whole,
                    },
                )
                .ok()?;
            let HirSourcePresence::Present(HirSourceSite::Span(source)) = lookup.presence() else {
                return None;
            };
            let whole = TextRange::new(source.range().start(), source.range().end());
            let body = source_range_for_query(
                module,
                HirSourceQuery::Expr {
                    owner: id,
                    role: HirExprSourceRole::Body,
                },
            )?;
            let header_range = TextRange::new(whole.start(), body.start());
            if offset < header_range.start() || offset >= header_range.end() {
                return None;
            }
            Some(ClosureEffectHoverTarget {
                source: source.clone(),
                header_range,
            })
        })
        .min_by_key(|target| target.header_range.end() - target.header_range.start())
}

fn callable_effect_row_hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    word: &str,
    word_range: TextRange,
) -> Option<Hover> {
    let (module, analysis) = accepted_module_and_analysis(profile, document)?;
    let callable = callable_at_word(module.as_ref(), word, word_range)?;
    let effects = match &callable.owner {
        CallableEffectOwner::Flow(owner) => analysis.item(*owner)?.effects(),
        CallableEffectOwner::CheckedCallable(source) => {
            let catalog = analysis.checked_callables();
            catalog
                .callable_at_source(source)
                .ok()?
                .exposed_row()
                .concrete()
        }
    };
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(checked_effect_hover_text(
            callable.label.as_str(),
            effects,
        ))),
        range: Some(
            document
                .line_index()
                .range_from_byte_span(word_range.start(), word_range.end()),
        ),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallableHoverTarget {
    owner: CallableEffectOwner,
    label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CallableEffectOwner {
    Flow(ItemId),
    CheckedCallable(CheckedCallableSourceKey),
}

fn callable_at_word(
    module: &HirModule,
    word: &str,
    word_range: TextRange,
) -> Option<CallableHoverTarget> {
    module.source_ordered_items().iter().find_map(|owner| {
        let item = module.resolve_item(*owner).ok()?;
        let (name, role) = match item.kind() {
            HirItemKind::Flow(flow) => (
                flow.identity().name()?.as_str(),
                HirItemSourceRole::Flow(HirFlowSourceRole::Name),
            ),
            HirItemKind::Function(function) => (
                function.name().resolved()?.as_str(),
                HirItemSourceRole::Callable(HirCallableSourceRole::Name {
                    owner: HirCallableSourceOwner::Item,
                }),
            ),
            _ => return None,
        };
        if name != word {
            return None;
        }
        let lookup = module
            .source_site(
                module.provenance().source_identity(),
                HirSourceQuery::Item {
                    owner: *owner,
                    role,
                },
            )
            .ok()?;
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
            return None;
        };
        let range = TextRange::new(span.range().start(), span.range().end());
        (range.start() <= word_range.start() && word_range.end() <= range.end()).then(|| {
            let owner = match item.kind() {
                HirItemKind::Flow(_) => CallableEffectOwner::Flow(*owner),
                HirItemKind::Function(_) => CallableEffectOwner::CheckedCallable(
                    CheckedCallableSourceKey::from_span(CheckedCallableSourceCategory::Name, span),
                ),
                _ => unreachable!("callable kind was filtered above"),
            };
            CallableHoverTarget {
                owner,
                label: word.to_owned(),
            }
        })
    })
}

fn accepted_module_and_analysis(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> Option<(Arc<HirModule>, Arc<FinalSemanticAnalysis>)> {
    let accepted = profile.accepted_environment()?;
    let executable = accepted.executable()?;
    let project = accepted.project();
    let module =
        Arc::clone(project.hir_for_open_document(document.uri(), document.source_document())?);
    Some((module, Arc::clone(executable.final_analysis())))
}

fn source_range_for_query(module: &HirModule, query: HirSourceQuery) -> Option<TextRange> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .ok()?;
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        return None;
    };
    Some(TextRange::new(span.range().start(), span.range().end()))
}

fn checked_effect_hover_text(label: &str, effects: &EffectSet) -> String {
    let labels = effects.to_labels();
    let effects = if labels.is_empty() {
        "{ }".to_owned()
    } else {
        format!("{{ {} }}", labels.join(", "))
    };
    format!("checked effects for `{label}`\n\neffects: {effects}")
}

fn word_at_position_range(
    document: &DocumentSnapshot,
    position: Position,
) -> Option<(String, TextRange)> {
    let offset = document
        .line_index()
        .try_byte_offset_from_position(position)
        .ok()?;
    let text = document.text();
    let start = text[..offset]
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_symbol_char(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    let end = text[offset..]
        .char_indices()
        .find_map(|(index, ch)| (!is_symbol_char(ch)).then_some(offset + index))
        .unwrap_or(text.len());
    (start < end).then(|| (text[start..end].to_owned(), TextRange::new(start, end)))
}

fn is_symbol_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '@' | ':' | '-')
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::documents::{AcceptedOpenDocument, DocumentStore};
    use crate::positions::PositionEncoding;
    use crate::profiles::{LspProfileResolver, LspProfileTestHarness};
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Uri};

    #[test]
    fn hover_describes_distinct_closed_flow_and_function_effect_rows() {
        let source = r"
extern capability fixture_agent {
    fn observe() -> Unit effects { agent.observe }
}

fn load_story() -> Unit
effects { agent.observe }
{
    fixture_agent.observe()
    ()
}

flow @flow.opening opening
effects { network.request }
{}
";
        let fixture = accepted_effect_hover_fixture("effect-row-hover", source);
        let source = fixture.source.as_str();
        let document = &fixture.document;
        let offset = source.find("opening\n").expect("flow name offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let flow_hover = hover(&fixture.profile, document, position).expect("effect row hover");

        match flow_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("checked effects for `opening`"));
                assert!(text.contains("effects: { network.request }"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let offset = source.find("load_story").expect("function name offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let function_hover =
            hover(&fixture.profile, document, position).expect("function effect row hover");

        match function_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(
                    text.contains("checked effects for `load_story`"),
                    "unexpected Function hover: {text}"
                );
                assert!(
                    text.contains("effects: { agent.observe }"),
                    "unexpected Function effects: {text}"
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn hover_describes_inferred_function_effect_row() {
        let source = r"
extern capability fixture_agent {
    fn observe() -> Unit effects { agent.observe }
}

fn load_story() -> Unit
{
    fixture_agent.observe()
    ()
}
";
        let fixture = accepted_effect_hover_fixture("inferred-effect-row-hover", source);
        let source = fixture.source.as_str();
        let document = &fixture.document;
        let offset = source.find("load_story").expect("function name offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let function_hover = hover(&fixture.profile, document, position)
            .expect("inferred Function effect row hover");

        match function_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(
                    text.contains("checked effects for `load_story`"),
                    "unexpected Function hover: {text}"
                );
                assert!(
                    text.contains("effects: { agent.observe }"),
                    "unexpected inferred Function effects: {text}"
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn callable_effect_row_hover_ignores_body_name_references() {
        let source = r"
extern capability fixture_agent {
    fn observe() -> Unit effects { agent.observe }
}

fn load_story() -> Unit
effects { agent.observe }
{
    fixture_agent.observe()
    ()
}

flow @flow.opening opening
effects { agent.observe }
{
    let body = load_story()
}
";
        let fixture = accepted_effect_hover_fixture("effect-row-body-hover", source);
        let source = fixture.source.as_str();
        let document = &fixture.document;
        let offset = source.rfind("load_story").expect("body call offset");
        let position = document.line_index().position_from_byte_offset(offset);
        let (word, word_range) =
            word_at_position_range(document, position).expect("body call word");

        assert!(
            callable_effect_row_hover(&fixture.profile, document, &word, word_range).is_none(),
            "body call references must not be treated as callable declarations"
        );
    }

    #[test]
    fn hover_describes_closure_expression_inferred_open_effect_row() {
        let source = r"
extern capability fixture_agent {
    fn observe() -> Unit effects { agent.observe }
}

fn retain_callback() -> Unit
effects { }
{
    let later = |_unit: Unit| -> Unit {
        fixture_agent.observe()
        ()
    }
    ()
}
";
        let fixture = accepted_effect_hover_fixture("closure-effect-row-hover", source);
        let source = fixture.source.as_str();
        let document = &fixture.document;
        let offset = source.find("|_unit").expect("closure header offset") + 1;
        let position = document.line_index().position_from_byte_offset(offset);
        let closure_hover =
            hover(&fixture.profile, document, position).expect("closure effect row hover");

        match closure_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("checked effects for `closure expression`"));
                assert!(text.contains("effects: { agent.observe }"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let body_offset = source.rfind("observe").expect("body call offset");
        let body_position = document.line_index().position_from_byte_offset(body_offset);
        let body_hover = hover(&fixture.profile, document, body_position);
        if let Some(body_hover) = body_hover {
            match body_hover.contents {
                HoverContents::Scalar(MarkedString::String(text)) => assert!(
                    !text.contains("checked effects for `closure expression`"),
                    "closure expression hover should stay limited to the closure header: {text}"
                ),
                other => panic!("unexpected hover contents: {other:?}"),
            }
        }
    }

    #[test]
    fn hover_describes_closure_expression_expected_effect_row_bound() {
        let source = r"
extern capability fixture_agent {
    fn observe() -> Unit effects { agent.observe }
}

fn retain_callback() -> Unit
effects { }
{
    let later: (Unit) -> Unit effects { agent.observe } = |_unit: Unit| -> Unit {
            fixture_agent.observe()
            ()
        }
    ()
}
";
        let fixture = accepted_effect_hover_fixture("closure-effect-row-bound-hover", source);
        let source = fixture.source.as_str();
        let document = &fixture.document;
        let offset = source.find("|_unit").expect("closure header offset") + 1;
        let position = document.line_index().position_from_byte_offset(offset);
        let closure_hover =
            hover(&fixture.profile, document, position).expect("closure effect row hover");

        match closure_hover.contents {
            HoverContents::Scalar(MarkedString::String(text)) => {
                assert!(text.contains("checked effects for `closure expression`"));
                assert!(text.contains("effects: { agent.observe }"));
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn checked_effect_hover_text_renders_exact_final_effects() {
        let effects = EffectSet::from_labels(["fs.read"]).expect("valid checked effects");
        let text = checked_effect_hover_text("callback", &effects);
        assert!(text.contains("checked effects for `callback`"));
        assert!(text.contains("effects: { fs.read }"));
    }

    struct EffectHoverFixture {
        _project: EffectHoverProject,
        profile: LspProfile,
        document: DocumentSnapshot,
        source: String,
    }

    fn accepted_effect_hover_fixture(label: &str, authored: &str) -> EffectHoverFixture {
        let project = EffectHoverProject::new(label);
        let source = format!(
            "{authored}\n\nfn lsp_test_controller() -> Result<Unit, AgentError>\neffects {{}}\n{{\n    Ok(())\n}}\n\nentry agent @entry.agent.main {{\n    controller = lsp_test_controller\n}}\n"
        );
        project.write("arcw.toml", &EffectHoverProject::manifest());
        project.write("src/main.arcw", &source);
        let source_path = project.path("src/main.arcw");
        let profile = LspProfileTestHarness::new(LspProfileResolver::new(
            RuntimeHostRunnerKind::Native,
            Some("agent".to_owned()),
        ))
        .resolve_for_document_path(&source_path)
        .expect("effect-hover profile construction")
        .publish_for_test();
        assert!(
            profile.diagnostics().is_empty(),
            "effect-hover fixture diagnostics: {:?}",
            profile.diagnostics()
        );
        let uri = file_uri(&source_path);
        let accepted = profile.accepted_environment().expect("accepted profile");
        let accepted_source = accepted
            .project()
            .sources()
            .by_uri(&uri)
            .expect("accepted effect-hover source");
        let authority = AcceptedOpenDocument::new(Arc::clone(accepted_source.document()), None);
        let mut store = DocumentStore::default();
        let document = store
            .open_with_authority(
                DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri,
                        language_id: "arcweft".to_owned(),
                        version: 1,
                        text: source.clone(),
                    },
                },
                PositionEncoding::Utf16,
                Some(&authority),
            )
            .expect("accepted effect-hover document");
        EffectHoverFixture {
            _project: project,
            profile,
            document,
            source,
        }
    }

    struct EffectHoverProject {
        root: PathBuf,
    }

    impl EffectHoverProject {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("arcweft-lsp-{label}-{unique}"));
            fs::create_dir_all(&root).expect("effect-hover project root");
            Self { root }
        }

        fn path(&self, path: impl AsRef<Path>) -> PathBuf {
            self.root.join(path)
        }

        fn manifest() -> String {
            r#"schema = 1

[package]
id = "org.arcweft.tests.effect-hover"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#
            .to_owned()
        }

        fn write(&self, path: &str, contents: &str) {
            let path = self.path(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("effect-hover fixture parent");
            }
            fs::write(path, contents).expect("effect-hover fixture write");
        }
    }

    impl Drop for EffectHoverProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn file_uri(path: &Path) -> Uri {
        format!(
            "file:///{}",
            path.to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        )
        .parse()
        .expect("file URI")
    }
}
