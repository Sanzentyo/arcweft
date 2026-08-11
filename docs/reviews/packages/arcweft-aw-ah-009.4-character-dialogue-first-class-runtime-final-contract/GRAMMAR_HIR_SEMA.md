# Grammar, CST, AST, HIR, sema, resolver, and line identity

## 1. Grammar decision

The parser preserves existing ordinary call and dialogue-content substrate. It
does not create a Character-specific parenthesized call node because syntax
cannot know the target type.

The source grammar is:

```text
CharacterDialogueValueExpr
  := OrdinaryExpr
   | OrdinaryExpr ArgumentList

DialogueContentApplication
  := OrdinaryExpr DialogueContentBlock LinePlanAttachment?
   | ColonDialogueHead ColonDialogueBody LinePlanAttachment?

ColonDialogueHead
  := OrdinaryExpr ':'

LinePlanAttachment
  := 'with' BraceLinePlan
   | 'with' ':' IndentedLinePlan
```

`OrdinaryExpr ArgumentList` remains the existing typed parenthesized call with
exact `ArgumentListSyntax`. Sema selects CharacterFactory or
CharacterReconfigure only when the target type is appropriate.

`DialogueContentBlock` is the existing bracket content block, not a collection
literal.

## 2. CST/AST target node

The old split `SpeakerLine`/string `ContentCall` model is replaced by one
source-structured node:

```rust
pub struct DialogueContentApplicationExpr {
    target: Box<Expr>,
    content: DialogueContent,
    plan: Option<LinePlan>,
    surface: DialogueContentApplicationSurface,
    range: TextRange,
}

pub enum DialogueContentApplicationSurface {
    Bracket {
        target_range: TextRange,
        open_bracket: TextRange,
        content_range: TextRange,
        close_bracket: RecoveredTokenRange,
        plan_range: Option<TextRange>,
    },
    Colon {
        head_range: TextRange,
        colon: TextRange,
        content_range: TextRange,
        indentation: DialogueIndentation,
        plan_range: Option<TextRange>,
    },
}

pub enum RecoveredTokenRange {
    Present(TextRange),
    Missing { insertion: usize },
}
```

`Expr` gains:

```rust
Expr::DialogueContentApplication(Box<DialogueContentApplicationExpr>)
```

There is no `speaker: String`, no `callee: String`, and no `.say`-specific node.

The colon parser creates the same AST node with `surface = Colon`; it does not
create a semantically separate speaker line.

## 3. Exact source ranges

### Parenthesized configuration

The existing exact parenthesized call surface remains authoritative:

- callee expression range;
- opening `(`;
- each argument name/value/full range;
- separators;
- closing `)` or exact missing-token insertion point;
- complete call range.

CharacterDialogue patch facts point back to those existing ranges.

### Bracket application

The node retains:

- full target expression range, including a nested configuration call;
- `[` token range;
- authored dialogue content range excluding delimiters;
- `]` range or exact insertion point;
- complete application range;
- attached `with` keyword/block range.

### Colon application

The node retains:

- full target/head expression range;
- exact colon token;
- inline or indented content range;
- indentation base and body extent;
- attached `with` range;
- complete source-line/application range.

The parser does not fabricate bracket ranges for colon source. Tooling emits a
replacement using these exact ranges.

## 4. Recovery

Recovery is current-grammar recovery:

| Malformation | Retained node | Diagnostic/recovery |
|---|---|---|
| missing `)` | ordinary call with missing close insertion | existing typed call delimiter diagnostic |
| missing `]` | dialogue-content application with `RecoveredTokenRange::Missing` | ordinary unclosed content-block diagnostic |
| empty `[]` | valid empty DialogueContent; later content policy may diagnose | no fabricated text |
| `target:` with no inline/indented body | colon application with empty content range | missing dialogue content diagnostic |
| invalid option expression | ordinary expression recovery inside argument | patch field omitted from accepted sema fact |
| duplicate option | syntax retains both | sema duplicate-coordinate diagnostic |
| malformed `with:` indentation | application retained without accepted plan | ordinary indentation/line-plan diagnostic |
| malformed `with {}` | application retained with recovered line plan | ordinary block/line-plan diagnostic |
| `.say(...)` | ordinary selected call | normal missing/invalid Character method diagnostic |

No spelling-specific `.say` parser, recognizer, code, or test remains.

## 5. Bracket and indexing ambiguity

The existing generic postfix-bracket CST is retained when lossless parsing
cannot decide. AST/HIR classification follows these rules:

1. An expression-start `[...]` remains a collection literal.
2. A target followed by a bracket payload that is a single ordinary expression
   and contains no dialogue-only token remains eligible for indexing.
3. A payload containing dialogue controls, RichText tokens, line breaks,
   dialogue interpolation, or a line-plan attachment is a dialogue-content
   candidate.
4. When syntax remains ambiguous, HIR retains `HirPostfixBracket` with exact
   target/payload ranges.
5. Sema resolves:
   - `Ref<Character>` or `CharacterDialogue` target plus dialogue payload to
     content application;
   - an `Index`-capable target plus expression payload to indexing;
   - two viable interpretations to a structured ambiguity diagnostic;
   - no viable interpretation to the ordinary target/type mismatch.
6. Character and CharacterDialogue are not indexable, so their direct bracket
   forms are unambiguous after typed resolution.
7. Tooling never uses identifier spelling to choose.

`items[0]` remains indexing. `configured[#[name][p]]` remains dialogue content.
A record literal passed as the first positional configuration argument is an
ordinary look argument and is type-checked normally.

## 6. Colon ambiguity

Colon content syntax is recognized only in executable flow/statement line
position with a complete expression head. Record fields, type annotations,
match arms, labels, and block headers retain their existing grammar precedence.

If the colon head resolves to neither `Ref<Character>` nor
`CharacterDialogue`, sema emits `AW-CD-003`; the parser does not reinterpret it
as another line family.

## 7. HIR

```rust
pub struct HirDialogueContentApplication {
    source_module: CanonicalModulePath,
    target: AuthoredExpr,
    content: DialogueContent,
    plan: Option<LinePlan>,
    line_id_syntax: Option<IdRef>,
    text_key_syntax: Option<IdRef>,
    surface: DialogueContentApplicationSurface,
    range: TextRange,
}
```

`line_id_syntax` and `text_key_syntax` are extracted only from the accepted
outer immediate-application configuration call. Both must be authored
`IdRef`/entity-reference syntax that resolves at compile time; an arbitrary
runtime expression in either coordinate is a source-ranged type/lowering
error. They are not copied into a
CharacterDialogue patch.

The old HIR fields are deleted:

```text
speaker_surface
callee: String
id/look/voice/etc copied as string-associated line options
DialogueSpeakerSlug
```

The target remains a full expression. Aliases, calls, branches, closures, and
function values are preserved structurally.

HIR source identity uses the existing `SourceDocument`/project source map.
There is no reparse of `target` text.

## 8. Accepted sema facts

```rust
pub enum CheckedCharacterDialogueTarget {
    Character {
        character: CharacterDialogueCharacterType,
        expression: TypeExpressionId,
    },
    Dialogue {
        ty: CharacterDialogueType,
        expression: TypeExpressionId,
    },
}

pub struct CheckedCharacterDialogueFactory {
    target: CheckedCharacterDialogueTarget,
    patch: CheckedCharacterDialoguePatch,
    result: CharacterDialogueType,
    call: CheckedCallableTargetFact,
}

pub struct CheckedCharacterDialogueReconfigure {
    target: CheckedCharacterDialogueTarget,
    patch: CheckedCharacterDialoguePatch,
    result: CharacterDialogueType,
    call: CheckedCallableTargetFact,
}

pub struct CheckedCharacterDialogueApplication {
    target: CheckedCharacterDialogueTarget,
    application_patch: Option<CheckedCharacterDialoguePatch>,
    line: RuntimeLineId,
    text_key: TextKey,
    content_range: SourceSpan,
    plan_result: TypeKind,
    surface: DialogueContentApplicationSurface,
}
```

These facts are generation-owned by the accepted type-check report and use the
existing exact expression/call identities.

## 9. Call typing

### Character reference call

```text
Ref<Character<Exact(C)>> + (...) -> CharacterDialogue<Exact(C)>
Ref<Character<Any>>      + (...) -> CharacterDialogue<Any>
```

The callee is evaluated once. `()` is a valid empty patch.

### CharacterDialogue reconfiguration

```text
CharacterDialogue<C> + (...) -> CharacterDialogue<C>
```

No patch may change `C`.

### Content application

```text
Ref<Character<C>>[content]
  == Ref<Character<C>>()[content]

CharacterDialogue<C>[content]
  -> DialogueLine<R>
```

Colon is the same checked application with a different source surface.

### Wrong targets

- parenthesized call on a non-character/non-dialogue uses ordinary callable
  resolution; if the user intended Character factory, `AW-CD-001`;
- reconfiguration-specific diagnostics are emitted only when a
  CharacterDialogue expected context proves that intent;
- bracket/colon content application to the wrong type emits `AW-CD-003`.

## 10. Expected types and generics

- A function parameter may be `CharacterDialogue`.
- A return type may be `CharacterDialogue`.
- Generic `T` may infer/bind a CharacterDialogue through ordinary rules.
- `Option<CharacterDialogue>`, `Result<CharacterDialogue,E>`,
  `Vec<CharacterDialogue>`, and records are permitted if their ordinary
  container/runtime rules permit the contained value.
- Exact character payload is preserved through monomorphic aliases, parameters,
  returns, and captures.
- Generic or branch abstraction widens only according to the exact join table
  in `FINAL_CONTRACT.md`.
- A downcast from `Any` to `Exact(C)` requires an ordinary typed character
  match; no source-name test is allowed.
- A CharacterDialogue is not callable as `Fn`, cannot satisfy a function trait
  by implicit coercion, and does not enter generic function partial-call state.
- `CharacterDialoguePatch` is not a source-visible generic type.

## 11. Custom fields

The shared resolver receives an immutable
`CharacterDialogueCustomFieldRegistry` from the accepted semantic world.

Resolution order for a named argument:

1. exact reserved coordinate;
2. canonical custom binding in current project/module scope;
3. ambiguity diagnostic;
4. unknown custom field diagnostic.

A custom field cannot shadow or alias a reserved name. Binding identity, not
source spelling, enters checked facts and runtime values.

## 12. Signature schemas

### CharacterFactory / CharacterReconfigure

The schema has one optional positional-or-named `look` coordinate followed by
named-only standard coordinates. In reusable context, `id` and `text_key` are
not in the schema. In immediate-application context they are application
coordinates, not config coordinates.

Unknown named arguments are `OpenChecked`: every name must resolve in the typed
custom-field registry. Spread arguments are rejected.

### ContentApplication

The bracket/colon schema is:

```text
target: Ref<Character> | CharacterDialogue
content: DialogueContent
line_plan: optional LinePlan
result: DialogueLine<R>
effects: existing dialogue/line effects
```

It is not represented as a parenthesized parameter group.

## 13. Signature help

The authored delimiter selects the help surface:

- cursor in `(` after Character or CharacterDialogue:
  configuration schema, current character-dependent look type, custom fields,
  and reusable/immediate context;
- cursor in `[`:
  content application summary and dialogue-content tags/controls, not config
  parameters;
- cursor in a colon head:
  concise content-application summary;
- cursor in `with`:
  line-plan schema and current line result type.

Signature help never invents or displays `.say`.

## 14. Source-site line-ID derivation

### Flow owner

For a flow ID `flow.game.intro`, named scopes `scene`, `greeting`, and the
second generated application in that exact prefix:

```text
say.flow.game.intro.scene.greeting.002
```

### Callable owner

For package `game`, module `game.dialogue`, function `phone_line`, named scope
`retry`, first generated application:

```text
say.fn.game.game.dialogue.function.phone_line.retry.001
```

For nested callable owner paths, every typed owner-path segment appears between
the owner family and callable name.

### Explicit relative IDs

Within the same flow/callable source owner:

```arcw
id = @.greeting
id = @say:.greeting
```

both produce:

```text
<prefix>.greeting
```

`@super.greeting` removes one named scope only. It cannot walk above the
source-owner prefix.

### Explicit absolute IDs

```arcw
id = @say.opening.greeting
```

is used exactly after `PublicId` and family validation.

### Collision pass

After HIR lowering, one project-wide line identity builder inserts every
explicit and generated ID transactionally. Any duplicate produces two source
labels and no accepted project. Generated counters never skip an occupied
candidate.

### Text keys

Absent text key:

```text
say.flow.game.intro.001
  -> text.flow.game.intro.001
```

No component is interpreted as a character identity.

## 15. Formatter/canonicalizer source model

The formatter consumes syntax only. It may preserve colon source and the chosen
`with:`/brace style while formatting whitespace and delimiters. It cannot
perform semantic expansion.

The project-aware canonicalizer consumes the checked application fact:

```arcw
alice:
    text
```

becomes:

```arcw
alice[
    text
]
```

and:

```arcw
alice(look = worried):
    text
```

becomes:

```arcw
alice(look = worried)[
    text
]
```

The replacement uses exact surface ranges. It never emits `.say`, never parses
a callee name, and does not run without matching accepted source revision.

## 16. Required HIR/sema deletions

Delete in the atomic HIR/sema cut:

```text
SpeakerLine
SpeakerLineSurface
speaker: String
ContentCall.callee: String
HirDialogue.callee
DialogueSpeakerSlug
TypeKind::Speaker
TypeKind::SpeakerPreset
SpeakerLineType
CheckedSpeakerLine
SpeakerLineOutcome
speaker_line_classification
DialogueCalleeIdentity::Speaker
DialogueCalleeIdentity::SpeakerPreset
DialogueCallableId::SpeakerLine
all `.say` suffix stripping
all name-based preset/character classification
```
