//! Surface parser for `.awft` source files.
//!
//! This crate owns syntax-level parsing only. It keeps enough structure for
//! formatter, diagnostics, and later HIR lowering, while deliberately avoiding
//! type resolution or runtime semantics.

mod ast;
mod check;
mod expr;
mod lower;
mod parser;
mod symbols;
mod text;

pub use ast::{
    Attribute, ChoiceBlock, ChoiceOption, ContentCall, DialogueToken, EntityRef, Flow, FlowItem,
    FlowKind, HookItem, IfBlock, Item, LinePlan, LinePlanItem, MatchArm, MatchBlock, MemoFn,
    ModuleDecl, ParserItem, Pattern, ScenarioCommand, SpeakerLine, Stmt, SyntaxTree, TextRange,
    UseItem, Visibility, WikiLink,
};
pub use check::{
    EntityKind, TypeCheckEnv, TypeCheckError, TypeCheckReadinessError, TypeKind, typecheck_hir,
    validate_typecheck_ready,
};
pub use expr::{BinaryOp, Expr, Literal, Placeholder, parse_expr};
pub use lower::{
    HirChoice, HirChoiceOption, HirDialogue, HirFlow, HirFlowItem, HirIf, HirLowerError, HirMatch,
    HirMatchArm, HirModule, lower_to_hir,
};
pub use parser::{ParseError, RecoverySuggestion, parse_source, parse_stub};
pub use symbols::{SymbolUse, SymbolUseKind, collect_symbol_uses};
pub use text::parse_dialogue_tokens;

#[cfg(test)]
mod tests {
    use super::{
        DialogueToken, EntityKind, Expr, FlowItem, FlowKind, HirFlowItem, Item, LinePlanItem,
        Pattern, Stmt, SymbolUseKind, TypeCheckEnv, TypeKind, Visibility, collect_symbol_uses,
        lower_to_hir, parse_dialogue_tokens, parse_source, parse_stub, typecheck_hir,
        validate_typecheck_ready,
    };

    #[test]
    fn stub_is_now_real_source_parser() {
        let tree = parse_stub("alice: おはよう。[p]").expect("speaker line parses");
        assert_eq!(tree.items().len(), 1);
        assert!(matches!(
            &tree.items()[0],
            Item::FlowItem(FlowItem::SpeakerLine(_))
        ));
    }

    #[test]
    fn parses_module_use_and_pub_flow() {
        let tree = parse_source(
            r"
mod game::routes::opening

use game::prelude::*
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    @bg #asset.bg.room fade=300ms
    include #frag.alice_enters
}
",
        )
        .expect("module, use, and flow parse");

        assert_eq!(
            tree.module().expect("module").path(),
            "game::routes::opening"
        );
        assert_eq!(tree.uses().len(), 1);
        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow item");
        };
        assert_eq!(flow.visibility(), Some(Visibility::Public));
        assert_eq!(flow.kind(), FlowKind::Flow);
        assert_eq!(flow.id().expect("flow id").body(), "flow.opening");
        assert_eq!(flow.body().len(), 2);
        assert!(
            matches!(&flow.body()[0], FlowItem::ScenarioCommand(command) if command.args().len() == 2)
        );
        assert!(matches!(&flow.body()[1], FlowItem::Include(_)));
    }

    #[test]
    fn parses_scenario_command_args_as_expressions() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    @show alice normal at=right fade=220ms
}
",
        )
        .expect("scenario command args parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::ScenarioCommand(command) = &flow.body()[0] else {
            panic!("expected scenario command");
        };
        assert_eq!(command.name(), "show");
        assert!(matches!(&command.args()[0], Expr::Path(path) if path == "alice"));
        assert!(matches!(
            &command.args()[2],
            Expr::NamedArg { name, value } if name == "at" && matches!(value.as_ref(), Expr::Path(path) if path == "right")
        ));
        assert!(matches!(
            &command.args()[3],
            Expr::NamedArg { name, value } if name == "fade" && matches!(value.as_ref(), Expr::Literal(_))
        ));
    }

    #[test]
    fn parses_fragment_as_flow_like_body() {
        let tree = parse_source(
            r"
pub fragment #frag.alice_enters alice_enters: FlowFragment {
    @show alice normal at=right fade=220ms
    alice: おはよう。[p]
}
",
        )
        .expect("fragment parses");

        let Item::Flow(fragment) = &tree.items()[0] else {
            panic!("expected fragment as flow-like item");
        };
        assert_eq!(fragment.kind(), FlowKind::Fragment);
        assert_eq!(
            fragment.id().expect("fragment id").body(),
            "frag.alice_enters"
        );
        assert!(matches!(&fragment.body()[0], FlowItem::ScenarioCommand(_)));
        assert!(matches!(&fragment.body()[1], FlowItem::SpeakerLine(_)));
    }

    #[test]
    fn parses_choice_block_inside_flow() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    @choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
"#,
        )
        .expect("choice block parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::Choice(choice) = &flow.body()[0] else {
            panic!("expected choice");
        };
        assert_eq!(
            choice.id().expect("choice id").body(),
            "choice.opening.first"
        );
        assert_eq!(choice.options().len(), 2);
        assert_eq!(choice.options()[0].label(), "聞いてみる");
    }

    #[test]
    fn parses_delimited_entity_refs_with_semantic_hashes() {
        let tree = parse_source(
            r"
flow #<flow.alice_intro@sem:b3_9f2a1c> opening {
    include #<frag.alice_enters@sem:f0_00aa>
}
",
        )
        .expect("delimited entity refs parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let id = flow.id().expect("flow id");
        assert!(id.is_delimited());
        assert_eq!(id.body(), "flow.alice_intro@sem:b3_9f2a1c");
        let FlowItem::Include(fragment) = &flow.body()[0] else {
            panic!("expected include");
        };
        assert!(fragment.is_delimited());
        assert_eq!(fragment.body(), "frag.alice_enters@sem:f0_00aa");
    }

    #[test]
    fn parses_colon_speaker_with_indented_line_plan() {
        let tree = parse_source(
            r"
alice(voice=auto, face=smile):
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
with:
    at(0.42s): alice.stage.face(worried)
    cancel on input .SkipLine => continue
    return (actor, voice)
",
        )
        .expect("speaker line with line plan parses");

        let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
            panic!("expected top-level speaker flow item");
        };
        assert_eq!(line.speaker(), "alice");
        assert_eq!(line.content().tokens().len(), 4);
        assert!(line.plan().is_some());
        let plan = line.plan().expect("line plan");
        assert!(matches!(&plan.items()[0], LinePlanItem::TimedCue { .. }));
        assert!(matches!(&plan.items()[1], LinePlanItem::CancelRule(_)));
        assert!(matches!(&plan.items()[2], LinePlanItem::Return(_)));
    }

    #[test]
    fn parses_bracket_speaker_call_with_with_colon_plan() {
        let tree = parse_source(
            r"
alice[
    おはよう。[p]
]
with:
    at(0.42s): alice.stage.face(smile)
",
        )
        .expect("bracket speaker call with with-colon plan parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        assert_eq!(call.callee(), "alice");
        assert_eq!(call.content().raw(), "おはよう。[p]");
        let plan = call.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::TimedCue {
                anchor: Expr::Literal(_),
                body: Expr::MethodCall { .. }
            }
        ));
    }

    #[test]
    fn parses_colon_form_with_inline_bracket_content() {
        let tree = parse_source("alice(voice=auto):[今日は少しだけ。[p]]")
            .expect("colon inline bracket content parses as dialogue text");

        let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
            panic!("expected speaker line");
        };
        assert_eq!(line.speaker(), "alice");
        assert_eq!(line.args(), Some("voice=auto"));
        assert_eq!(line.content().raw(), "[今日は少しだけ。[p]]");
    }

    #[test]
    fn parses_compat_at_bracket_timed_cue() {
        let tree = parse_source(
            r"
alice[おはよう。[p]]
with:
    at(0.42s)[alice.stage.face(worried)]
",
        )
        .expect("compat at bracket cue parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        let plan = call.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::TimedCue {
                anchor: Expr::Literal(_),
                body: Expr::MethodCall { .. }
            }
        ));
    }

    #[test]
    fn parses_choice_option_with_condition() {
        let tree = parse_source(
            r#"
@choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
}
"#,
        )
        .expect("choice option condition parses");

        let Item::FlowItem(FlowItem::Choice(choice)) = &tree.items()[0] else {
            panic!("expected choice");
        };
        let option = &choice.options()[0];
        assert_eq!(
            option.id().expect("choice option id").body(),
            "choice.opening.listen"
        );
        assert!(matches!(option.condition(), Some(Expr::Binary { .. })));
        assert_eq!(option.target().body(), "flow.alice_intro");
    }

    #[test]
    fn parses_character_content_call_with_brace_plan() {
        let tree = parse_source(
            r##"
alice.say(id=#say.opening.dream_hint, voice=auto)[
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
]
with {
    at(0.42s) { alice.stage.face(worried, crossfade=120ms) }
}
"##,
        )
        .expect("content call parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        assert_eq!(call.callee(), "alice.say");
        assert!(call.plan().is_some());
    }

    #[test]
    fn parses_documented_hook_memo_and_parser_items() {
        let tree = parse_source(
            r#"
hook #hook.choice_visible
on #choice.opening.listen
phase AfterLayout
check every frame
{
    signal #signal.choice_visible <- true
}

memo fn route_title(route: Ref<Flow>) -> String
cache session
{
    registry.flow(route).title
}

pub parser parse_player_command: Parser<PlayerCommand, ParseError> {
    alt { "advance" => PlayerCommand::Advance }
}
"#,
        )
        .expect("hook, memo, and parser items parse");

        assert!(matches!(&tree.items()[0], Item::Hook(_)));
        assert!(matches!(&tree.items()[1], Item::MemoFn(_)));
        assert!(matches!(&tree.items()[2], Item::Parser(_)));
    }

    #[test]
    fn parses_attributes_and_wiki_links() {
        let tree = parse_source(
            r"
/// links to [[flow.alice_intro]]
@derive(Debug)
flow #flow.opening opening {}
",
        )
        .expect("attributes and wiki links parse");

        assert_eq!(tree.wiki_links()[0].body(), "flow.alice_intro");
        assert!(matches!(&tree.items()[0], Item::Attribute(attr) if attr.name() == "derive"));
        assert!(matches!(&tree.items()[1], Item::Flow(_)));
    }

    #[test]
    fn dialogue_tokenizer_covers_tags_ruby_expr_and_escapes() {
        let tokens = parse_dialogue_tokens(
            r#"今日は\｜少し、｜変な夢《へんなゆめ》#[fmt("夢", color=blue)][p][hook mark]"#,
        );

        assert!(tokens.iter().any(|token| matches!(token, DialogueToken::Ruby { base, ruby } if base == "変な夢" && ruby == "へんなゆめ")));
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, DialogueToken::Expr(Expr::Call { .. })))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "p"))
        );
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, DialogueToken::Tag(tag) if tag.name() == "hook"))
        );
    }

    #[test]
    fn dialogue_tokenizer_preserves_escaped_brackets_hash_and_braces() {
        let tokens = parse_dialogue_tokens(r"\[literal\] \#not_expr \{cue\}");

        assert!(matches!(tokens[0], DialogueToken::Escape('[')));
        assert!(matches!(tokens[2], DialogueToken::Escape(']')));
        assert!(matches!(tokens[4], DialogueToken::Escape('#')));
        assert!(matches!(tokens[6], DialogueToken::Escape('{')));
        assert!(matches!(tokens[8], DialogueToken::Escape('}')));
        assert!(
            tokens
                .iter()
                .all(|token| !matches!(token, DialogueToken::Tag(_) | DialogueToken::Expr(_)))
        );
    }

    #[test]
    fn reports_unclosed_flow_block() {
        let errors = parse_source("flow #flow.bad bad {").expect_err("unclosed block fails");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("unclosed block"));
        assert!(!errors[0].recovery().is_empty());
    }

    #[test]
    fn reports_invalid_entity_reference() {
        let errors = parse_source("flow # bad { }").expect_err("invalid entity ref fails");
        assert!(errors[0].message().contains("entity reference"));
    }

    #[test]
    fn reports_unclosed_dialogue_content_block() {
        let errors = parse_source("alice[おはよう。[p]").expect_err("unclosed content fails");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("dialogue content"));
        assert_eq!(errors[0].expected(), &["]"]);
        assert!(!errors[0].recovery().is_empty());
    }

    #[test]
    fn reports_unclosed_line_plan_block_after_cue() {
        let errors = parse_source(
            r"
alice[おはよう。[p]]
with {
    at(0.42s) { alice.stage.face(worried)
",
        )
        .expect_err("unclosed line plan fails");

        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("line plan"));
        assert!(!errors[0].recovery().is_empty());
    }

    #[test]
    fn parses_expression_shapes_needed_by_hir_lowering() {
        let pipe = super::parse_expr("state |> has_affection_at_least(#character.alice, 3)")
            .expect("pipe expr parses");
        assert!(matches!(pipe, Expr::Pipe { .. }));

        let method = super::parse_expr("choices.filter(_.enabled).map(_.label)")
            .expect("method chain parses");
        assert!(matches!(method, Expr::MethodCall { .. }));

        let indexed =
            super::parse_expr("state.affection[#character.alice]").expect("index expr parses");
        assert!(matches!(indexed, Expr::Index { .. }));

        let dialogue_call =
            super::parse_expr("alice.say()[聞いて。[p]]").expect("dialogue call expr parses");
        assert!(matches!(dialogue_call, Expr::DialogueCall { .. }));

        let timeline_offset = super::parse_expr("end-250ms").expect("timeline offset parses");
        assert!(matches!(timeline_offset, Expr::Binary { .. }));

        let placeholder = super::parse_expr("clamp(0, ^, 100)").expect("placeholder call parses");
        assert!(matches!(placeholder, Expr::Call { .. }));

        let delimited = super::parse_expr("#<say.opening.dream_hint@sem:b3_9f2a1c>")
            .expect("delimited ref expr parses");
        assert!(matches!(delimited, Expr::EntityRef(entity) if entity.is_delimited()));
    }

    #[test]
    fn line_plan_items_keep_typed_expressions() {
        let tree = parse_source(
            r"
alice:
    聞いて。[p]
with:
    reveal = voice
    let voice = line.voice_handle()
    return (actor, voice)
",
        )
        .expect("line plan exprs parse");

        let Item::FlowItem(FlowItem::SpeakerLine(line)) = &tree.items()[0] else {
            panic!("expected speaker line");
        };
        let plan = line.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::Option { value: Expr::Path(path), .. } if path == "voice"
        ));
        assert!(matches!(
            &plan.items()[1],
            LinePlanItem::Let {
                expr: Expr::MethodCall { .. },
                ..
            }
        ));
        assert!(matches!(
            &plan.items()[2],
            LinePlanItem::Return(Expr::Tuple(_))
        ));
    }

    #[test]
    fn parses_multiline_timed_cue_body_as_expression() {
        let tree = parse_source(
            r"
alice[
    おはよう。[p]
]
with:
    at(0.42s):
        alice.stage.face(smile)
",
        )
        .expect("multiline timed cue parses");

        let Item::FlowItem(FlowItem::ContentCall(call)) = &tree.items()[0] else {
            panic!("expected content call");
        };
        let plan = call.plan().expect("line plan");
        assert!(matches!(
            &plan.items()[0],
            LinePlanItem::TimedCue {
                anchor: Expr::Literal(_),
                body: Expr::MethodCall { .. }
            }
        ));
    }

    #[test]
    fn await_with_keeps_awaited_expression() {
        let tree = parse_source(
            r"
flow #flow.loading loading {
    await load_opening_assets()? with { pending p => scene #scene.loading { progress p.ratio } }
}
",
        )
        .expect("await with parses");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        let FlowItem::AwaitWith(await_with) = &flow.body()[0] else {
            panic!("expected await with");
        };
        assert!(await_with.propagates_error());
        assert!(matches!(await_with.expr(), Expr::Call { .. }));
        assert!(await_with.pending().is_some());
    }

    #[test]
    fn flow_typed_statements_keep_patterns_and_exprs() {
        let tree = parse_source(
            r"
flow #flow.opening opening {
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    return Ok(FlowExit::Done)
    goto #flow.title
}
",
        )
        .expect("typed flow statements parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(matches!(
            &flow.body()[0],
            FlowItem::Stmt(Stmt::Let {
                pattern: Pattern::Tuple(_),
                expr: Expr::DialogueCall { .. },
            })
        ));
        assert!(matches!(
            &flow.body()[1],
            FlowItem::Stmt(Stmt::Return(Expr::Call { .. }))
        ));
        assert!(matches!(
            &flow.body()[2],
            FlowItem::Stmt(Stmt::Goto(Expr::EntityRef(entity))) if entity.body() == "flow.title"
        ));
    }

    #[test]
    fn parses_if_and_match_flow_blocks_for_hir() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if state.ready {
        goto #flow.ready
    }
    match next {
        None => goto #flow.title
        _ => goto #flow.fallback
    }
}
",
        )
        .expect("if and match parse");

        let Item::Flow(flow) = &tree.items()[0] else {
            panic!("expected flow");
        };
        assert!(
            matches!(&flow.body()[0], FlowItem::If(block) if matches!(block.condition(), Expr::Path(path) if path == "state.ready"))
        );
        assert!(matches!(&flow.body()[1], FlowItem::Match(block) if block.arms().len() == 2));

        let hir = lower_to_hir(&tree).expect("if and match lower");
        assert!(matches!(&hir.flows()[0].body()[0], HirFlowItem::If(_)));
        assert!(matches!(&hir.flows()[0].body()[1], HirFlowItem::Match(_)));
    }

    #[test]
    fn typechecks_if_and_match_flow_blocks() {
        let tree = parse_source(
            r"
flow #flow.branching branching {
    if state.ready {
        goto #flow.ready
    }
    match next {
        None => goto #flow.title
        _ => goto #flow.fallback
    }
}
",
        )
        .expect("if and match fixture parses");
        let hir = lower_to_hir(&tree).expect("if and match fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("state.ready", TypeKind::Bool)
            .with_symbol("next", TypeKind::Named("Option<Ref<Flow>>".to_owned()));

        typecheck_hir(&hir, &env).expect("if and match fixture typechecks");
    }

    #[test]
    fn lowers_edge_case_flow_to_hir_without_raw_reparse() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    @bg #asset.bg.room fade=300ms
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    await load_opening_assets()? with { pending p => scene #scene.loading { progress p.ratio } }
    alice[
        今日は｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with:
        at(end-250ms): alice.stage.face(worried)
    @choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
    }
    goto #flow.title
}
"#,
        )
        .expect("edge flow parses");

        let hir = lower_to_hir(&tree).expect("edge flow lowers");
        let flow = &hir.flows()[0];
        assert!(
            flow.body()
                .iter()
                .any(|item| matches!(item, HirFlowItem::Stmt(Stmt::Let { .. })))
        );
        assert!(flow.body().iter().any(|item| matches!(
            item,
            HirFlowItem::Await {
                propagates_error: true,
                ..
            }
        )));
        assert!(flow.body().iter().any(
            |item| matches!(item, HirFlowItem::Dialogue(dialogue) if dialogue.callee() == "alice")
        ));
        assert!(flow.body().iter().any(
            |item| matches!(item, HirFlowItem::Dialogue(dialogue) if dialogue.plan().is_some())
        ));
        assert!(flow
            .body()
            .iter()
            .any(|item| matches!(item, HirFlowItem::Choice(choice) if choice.options()[0].condition().is_some())));
    }

    #[test]
    fn collects_hir_symbol_uses_for_type_checking_without_reparsing() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    let (actor, (_, voice)) = alice.say()[聞いて。[p]]
    alice[
        #[fmt("夢", color=blue)]を見た。[p]
    ]
    with:
        at(0.42s): alice.stage.face(worried)
    @choice #choice.opening.first {
        #choice.opening.listen "聞く" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
    }
}
"#,
        )
        .expect("symbol fixture parses");
        let hir = lower_to_hir(&tree).expect("symbol fixture lowers");
        let uses = collect_symbol_uses(&hir);

        assert!(uses.iter().any(
            |symbol| symbol.kind() == SymbolUseKind::DialogueCallee && symbol.name() == "alice"
        ));
        assert!(
            uses.iter()
                .any(|symbol| symbol.kind() == SymbolUseKind::Method && symbol.name() == "face")
        );
        assert!(
            uses.iter()
                .any(|symbol| symbol.kind() == SymbolUseKind::EntityRef
                    && symbol.name() == "character.alice")
        );
        assert!(
            uses.iter()
                .all(|symbol| symbol.kind() != SymbolUseKind::RawExpr)
        );
        validate_typecheck_ready(&hir).expect("edge fixture is typecheck-ready");
    }

    #[test]
    fn typecheck_readiness_rejects_raw_dialogue_expressions() {
        let tree = parse_source(
            r#"
alice[
    #[fmt("夢", color=)]を見た。[p]
]
"#,
        )
        .expect("raw dialogue expression fixture parses lossily");
        let hir = lower_to_hir(&tree).expect("raw dialogue expression still lowers");
        let errors = validate_typecheck_ready(&hir).expect_err("raw expr blocks type checking");

        assert!(errors[0].message().contains("raw expression"));
    }

    #[test]
    fn typechecks_edge_case_hir_with_explicit_environment() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    @show alice normal at=right fade=220ms
    let (actor, (_, voice)) = alice.say(voice=auto)[聞いて。[p]]
    await load_opening_assets()? with { pending p => scene #scene.loading { progress p.ratio } }
    alice[
        #[fmt("夢", color=blue)]を見た。[p]
    ]
    with:
        at(0.42s): alice.stage.face(worried)
    @choice #choice.opening.first {
        #choice.opening.listen "聞く" if state.affection[#character.alice] >= 3 -> #flow.alice_intro
    }
    goto #flow.title
}
"#,
        )
        .expect("typecheck fixture parses");
        let hir = lower_to_hir(&tree).expect("typecheck fixture lowers");
        let env = TypeCheckEnv::new()
            .with_symbol("alice", TypeKind::Ref(EntityKind::Character))
            .with_symbol("alice.stage", TypeKind::Named("StageActor".to_owned()))
            .with_symbol("auto", TypeKind::Named("VoicePolicy".to_owned()))
            .with_symbol("blue", TypeKind::Named("Color".to_owned()))
            .with_symbol("normal", TypeKind::Named("Pose".to_owned()))
            .with_symbol("right", TypeKind::Named("StagePosition".to_owned()))
            .with_symbol("worried", TypeKind::Named("Face".to_owned()))
            .with_symbol("end", TypeKind::Duration)
            .with_symbol(
                "state.affection",
                TypeKind::Named("Map<Ref<Character>, Int>".to_owned()),
            )
            .with_function("fmt", TypeKind::DisplayText)
            .with_function(
                "load_opening_assets",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                },
            )
            .with_method(
                TypeKind::Ref(EntityKind::Character),
                "say",
                TypeKind::Named("SayBuilder".to_owned()),
            )
            .with_method(
                TypeKind::Named("StageActor".to_owned()),
                "face",
                TypeKind::Named("StageCue".to_owned()),
            )
            .with_index(
                TypeKind::Named("Map<Ref<Character>, Int>".to_owned()),
                TypeKind::Int,
            );

        typecheck_hir(&hir, &env).expect("edge fixture typechecks");
    }

    #[test]
    fn typechecks_fragment_hir_and_include_target() {
        let tree = parse_source(
            r"
pub fragment #frag.alice_enters alice_enters: FlowFragment {
    alice: おはよう。[p]
}

flow #flow.opening opening {
    include #frag.alice_enters
}
",
        )
        .expect("fragment include fixture parses");
        let hir = lower_to_hir(&tree).expect("fragment include fixture lowers");
        let env = TypeCheckEnv::new().with_symbol("alice", TypeKind::Ref(EntityKind::Character));

        assert_eq!(hir.flows()[0].kind(), FlowKind::Fragment);
        typecheck_hir(&hir, &env).expect("fragment include fixture typechecks");
    }

    #[test]
    fn typecheck_reports_wrong_choice_target_kind() {
        let tree = parse_source(
            r#"
flow #flow.opening opening {
    @choice #choice.opening.first {
        #choice.opening.listen "聞く" -> #asset.bg.room
    }
}
"#,
        )
        .expect("bad choice target fixture parses");
        let hir = lower_to_hir(&tree).expect("bad choice target lowers");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
            .expect_err("choice target must be a flow ref");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("choice target"))
        );
    }

    #[test]
    fn lowering_rejects_unstructured_raw_items() {
        let tree = parse_source("unknown top level syntax").expect("raw item is syntax-preserved");
        let errors = lower_to_hir(&tree).expect_err("raw item cannot lower");
        assert!(errors[0].message().contains("raw"));
    }
}
