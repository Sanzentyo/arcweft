use arcweft_lang_hir::{lower::lower_to_hir, model::HirTopLevelDecl};
use arcweft_lang_syntax::parser::parse_source;

#[test]
fn ordinary_struct_functions_and_entry_are_the_only_role_owners_in_hir() {
    let parsed = parse_source(
        r"
struct GameState {
    score: i32
}

fn initial_game_state() -> GameState {
    initial_game_state()
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
{
    reduce_game(state, event)
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
",
    );
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_to_hir(parsed.typed_tree()).expect("ordinary role owners lower");

    assert_eq!(
        hir.functions()
            .iter()
            .map(|function| function.signature().name())
            .collect::<Vec<_>>(),
        ["initial_game_state", "reduce_game"]
    );
    assert_eq!(
        hir.declarations()
            .iter()
            .filter(|declaration| matches!(declaration, HirTopLevelDecl::Struct(_)))
            .count(),
        1
    );
    assert_eq!(
        hir.declarations()
            .iter()
            .filter(|declaration| matches!(declaration, HirTopLevelDecl::Entry(_)))
            .count(),
        1
    );
}
