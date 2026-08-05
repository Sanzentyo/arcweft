# Scope and Relative ID Example

This example shows how named `scope` blocks, relative line IDs, relative choice
IDs, and module-relative paths work together.

```arcw
mod crate.game.routes.opening

use crate.game.prelude.*
use self.characters.{alice}
use super.common.{route_gate}

pub flow opening(state: GameState) -> Result<FlowExit, FlowError> {
    scope rain {
        地の文(id=@.sound):
            扉の向こうから、雨の音がした。[p]

        alice(id=@.comment, voice=auto):
            雨、強くなってきたね。[p]
    }

    scope dream {
        let can_enter = {
            let affection_ok = state.affection[@character.alice] >= 3
            affection_ok
        }

        choice @.first {
            @.listen "聞いてみる" if can_enter -> @flow.alice_intro
            @.silent "黙っている" -> @flow.quiet_intro
        }
    }
}
```

The normalized IDs are:

```text
地の文(id=@.sound)
  -> @say.opening.narrator.rain.sound
  -> @text.opening.narrator.rain.sound

alice(id=@.comment, voice=auto)
  -> @say.opening.alice.rain.comment
  -> @text.opening.alice.rain.comment
  -> @voice.ja-JP.alice.opening.rain.comment

choice @.first
  -> @choice.opening.dream.first

@.listen -> @choice.opening.dream.first.listen
  -> @text.choice.opening.dream.first.listen

@.silent -> @choice.opening.dream.first.silent
  -> @text.choice.opening.dream.first.silent
```

If a named scope is absent, the scope segment is omitted:

```text
alice(id=@.greeting)
  -> @say.opening.alice.greeting
  -> @text.opening.alice.greeting

choice @.first
  -> @choice.opening.first
```

`scope` can also be used as a value-producing expression block. In that case,
the final expression is the value, while the scope name is still used for
diagnostics, traces, LSP display, and ID-bearing constructs inside the block.

```arcw
let can_enter = scope alice_route_check {
    let affection_ok = state.affection[@character.alice] >= 3
    let has_key = state.inventory.contains(@item.alice_key)
    affection_ok && has_key
}
```

Relative `.suffix` IDs are not module paths and are not general entity
references. Module and import paths use `crate.`, `self.`, and `super.`.
General references that need relative lookup must include the entity family.

```arcw
alice(id=@.greeting):        // relative ID context
use self.characters.alice // module path context
goto @flow.opening.next     // ordinary entity reference
goto @flow:.next            // family-relative entity reference
include @flow:.alice_enters // family-relative flow reference
```

`parent.` is reserved as an alias for `super.`, but canonical tooling should
format it as `super.`.

