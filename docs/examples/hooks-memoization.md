# Owner-local events and derived values example

This example replaces the former universal hook and memo declarations. The
choice owns its condition and selection behavior, the View owns its input
handler, and derived state is an ordinary pure function.

```arcw
fn alice_route_ready(state: GameState) -> bool {
    state.affection[@character.alice] >= 3
}

choice @choice.opening.first {
    option @.listen {
        label = "聞いてみる"
        enabled = alice_route_ready(state)

        select {
            goto @flow.alice_intro
        }

        view {
            Button("聞いてみる")
                .agent_target(@choice.opening.listen)
                .on_pointer_enter {
                    action.invoke(
                        @action.choice.hover,
                        @choice.opening.listen,
                    )
                }
        }
    }
}
```

The View evaluator tracks the reads used to construct the retained View. Input
routing is recorded in the ordinary routed-input trace, and Agent observation
reads the resulting View/choice state. No author-defined global subscription or
cache invalidation namespace is involved.
