# Family grammars

## 1. Shared lexical productions

```text
OuterPrefix      ::= (DocBlock | OuterAttribute)*
Visibility       ::= "pub" | "pub(crate)" | "pub(super)"
ExplicitId<F>    ::= ENTITY_REFERENCE_TOKEN
LocalName        ::= IDENTIFIER
LogicalEnd       ::= ";"? NEWLINE | ";" EOF | EOF
Terminator       ::= ";" | NEWLINE
RetainedHead<F>  ::= OuterPrefix Visibility? F ExplicitId<F>? LocalName
```

Constraints:

- `LocalName` is exactly one non-keyword identifier and cannot contain `.`.
- `ExplicitId<F>` in a declaration header is a plain absolute `@family.segment...` reference. `@.x`, `@family:.x`, package/module-qualified spellings, braces, slashes, and origin-qualified spellings are rejected in retained declaration headers.
- The ID body, without `@`, must parse as `PublicId` and its first segment must equal `F`.
- Omitting `ExplicitId<F>` derives `F.<LocalName>`.
- Only `metric` inserts its closed kind token between the family keyword and optional ID.
- Braced declarations require braces. Indented body forms are not accepted for these seven declarations.
- Action and signal are bodyless and end at `LogicalEnd`. A semicolon is accepted but not required before newline/EOF.

## 2. `asset`: no authored grammar

```text
AssetDeclaration ::= <no production>
```

`asset` remains a `RetainedIdentityFamily` prefix and entity-reference domain. Asset identities are generated from canonical asset virtual paths by project/build catalog admission. Top-level `asset ...` is an ordinary `ErrorItem`; it does not produce `AssetDeclarationItem`, `Item::Asset`, `HirItemKind::Asset`, or a removed-syntax diagnostic.

The canonical catalog ID algorithm is:

1. take a validated, relative, `/`-separated asset virtual path;
2. remove only the final filename extension;
3. split on `/` and reject empty effective results;
4. for each component, lowercase ASCII alphanumeric characters, map `_` and `-` to `_`, and reject every other character;
5. join components with `.` and prefix `asset.`;
6. parse the result through the owned `PublicId`/asset-ID constructor;
7. reject any two distinct virtual paths that normalize to the same asset ID, including case, dash/underscore, and same-stem/different-extension collisions.

The existing algorithm is moved out of CLI-local helpers into an Arcweft-owned `AssetId`/asset-identity API. Filesystem enumeration and byte reads remain adapter-owned; normalized path and identity derivation are pure typed behavior.

## 3. Character

```text
CharacterDecl   ::= RetainedHead<"character"> SurfaceAlias? CharacterBody
SurfaceAlias    ::= "as" IDENTIFIER
CharacterBody   ::= "{" CharacterMember* "}"
CharacterMember ::= "display_name" "=" Expr Terminator?
```

Canonical form:

```arcw
/// Main speaker
#[test.fixture]
pub character @character.alice Alice as alice {
    display_name = "Alice"
}
```

Rules:

- Body is required and may be empty.
- `display_name` is optional and singleton. Its value is one common `Expr` and must semantically be a constant `String`.
- No `voice`, `style`, `view`, dialogue-default, presentation, registry, extension, or override field is accepted in this body.
- `as` is Character-only, optional, singleton, and requires an ordinary alias identifier.
- No generic parameters, fixed parameters, return type, where clause, contract, inheritance, or bodyless form exists.

## 4. View

```text
ViewDecl        ::= RetainedHead<"view"> FixedParameterGroup ViewBody
FixedParameterGroup
                 ::= "(" (ViewParameter ("," ViewParameter)* ","?)? ")"
ViewParameter   ::= BindingName ":" Type ("=" Expr)?
BindingName     ::= IDENTIFIER
ViewBody        ::= "{" ViewExport* ViewFragment "}"
ViewExport      ::= "export" "part" LocalPartPath "as" PublicPartPath Terminator?
ViewFragment    ::= ViewValue*
ViewValue       ::= Expr Terminator?
```

Canonical form:

```arcw
pub view @view.MainDialogue MainDialogue(dialogue: DialogueView) {
    export part panel as dialogue_panel
    Panel {
        Text(dialogue.character.display_name)
        RichText(dialogue.content)
        Style { opacity = 0.5 }
    }.part(panel)
}
```

Rules:

- Exactly one fixed parameter group is required, including `()` for zero parameters.
- A parameter pattern must be one ordinary binding name. Destructuring, rest, receiver, and placeholder patterns remain typed recovery and poison the declaration.
- Parameter defaults are allowed and use common expressions.
- Generics, where clauses, return arrows, requires/ensures, and function bodies are not allowed. A View declaration's callable result is the accepted View product and is not authored with `->`.
- Body is required. Zero or more exports must precede all View values. A later `export` is retained as a typed export node with `syntax.view.misplaced_export` and poisons the declaration.
- The body owns one `ViewFragment` containing ordered common expression descendants. Invalid source becomes typed `ErrorExpression`; no raw View fragment is stored.

## 5. Action

```text
ActionDecl      ::= RetainedHead<"action"> ActionSignature LogicalEnd
ActionSignature ::= "(" (ActionParameter ("," ActionParameter)* ","?)? ")"
ActionParameter ::= BindingName ":" Type
```

Canonical form:

```arcw
pub action @action.feedback_submit feedback_submit(value: Feedback, count: u32)
```

Rules:

- Exactly one fixed parameter group is required.
- Parameters are ordered ordinary binding names with required types.
- Defaults, generics, where clauses, return arrows, contracts, effect/capability clauses, and bodies are rejected.
- The payload schema is the ordered typed product of the parameters; zero parameters means `Unit`.
- Action is a bodyless typed channel with one project callable/channel facet. It is not a function implementation and has no overload set.

## 6. Activity

```text
ActivityDecl       ::= RetainedHead<"activity"> ActivityBody
ActivityBody       ::= "{" ActivitySection* "}"
ActivitySection    ::= ModeMember | LifecycleMember | InputBlock | OutputBlock | ContractBlock
ModeMember         ::= "mode" "=" ActivityMode Terminator?
ActivityMode       ::= "deterministic" | "checkpointed_realtime" | "external_realtime"
LifecycleMember    ::= "lifecycle" "=" ActivityLifecycle Terminator?
ActivityLifecycle  ::= "stateless" | "snapshot"
InputBlock         ::= "input" "{" ActivityPort* "}"
OutputBlock        ::= "output" "{" ActivityPort* "}"
ActivityPort       ::= IDENTIFIER ":" Type Terminator?
ContractBlock      ::= "contract" "{" RequiresClause* EnsuresClause* "}"
RequiresClause     ::= "requires" Expr Terminator?
EnsuresClause      ::= "ensures" Expr Terminator?
```

Canonical form:

```arcw
pub activity @activity.truck_game TruckGame {
    mode = deterministic
    lifecycle = snapshot
    input {
        route_seed: u64
    }
    output {
        result: TruckResult
    }
    contract {
        requires route_seed > 0
        ensures result.score >= 0
    }
}
```

Rules:

- Body is required and may be empty. Omitted mode defaults to `deterministic`; omitted lifecycle defaults to `stateless`; omitted input/output/contract sections are empty.
- Sections are singleton and must occur in the order shown. Duplicate and out-of-order sections remain typed and poison the declaration.
- Port names are unique across both input and output. Ports have no initializer.
- `requires` clauses must precede `ensures` clauses.
- No source token can encode Rust/WASM/process origin, path, crate, adapter, module, executable, or implementation selection.
- Missing or incompatible implementation binding is diagnosed by compiler/project manifest admission against this typed interface, not by syntax.

## 7. Signal

```text
SignalDecl      ::= RetainedHead<"signal"> ":" ObservableType LogicalEnd
ObservableType  ::= Type
```

Canonical forms:

```arcw
pub signal @signal.current current: Watch<Ref<Flow>>
signal events: Stream<GameEvent, EventError>
signal sample: Sample<f32>
```

Rules:

- The parser owns exactly one common type child and does not inspect display text.
- Semantic admission accepts exactly `Watch<T>`, `Stream<T, E>`, and `Sample<T>` with their shown arities. An unknown head or wrong arity is a semantic type diagnostic on the typed `SignalObservableType`.
- Initializers, bodies, mutability/lifetime/replay/persistence policy tails, host bindings, and adapters are rejected.
- Statement-context `signal` syntax remains independently classified and cannot enter top-level declaration parsing.

## 8. Metric

```text
MetricDecl      ::= OuterPrefix Visibility? "metric" MetricKind ExplicitId<metric>? LocalName ":" Type MetricBody
MetricKind      ::= "counter" | "gauge" | "histogram"
MetricBody      ::= "{" MetricMember* "}"
MetricMember    ::= UnitMember | LabelsBlock | BucketsMember
UnitMember      ::= "unit" "=" STRING Terminator?
LabelsBlock     ::= "labels" "{" MetricLabel* "}"
MetricLabel     ::= IDENTIFIER ":" Type Terminator?
BucketsMember   ::= "buckets" "=" "[" Expr ("," Expr)* ","? "]" Terminator?
```

Canonical form:

```arcw
pub metric gauge @metric.frame_time frame_time: f32 {
    unit = "ms"
    labels {
        scene: String
        quality: RenderQuality
    }
}

metric histogram latency: f64 {
    buckets = [1.0, 2.0, 4.0]
}
```

Rules:

- `MetricKind` is closed and appears immediately after `metric`.
- Body is required and may be empty. Members are singleton and ordered `unit`, `labels`, `buckets`.
- Unit is exactly one string token.
- Label names are unique. Label types must semantically be stable scalar, string, boolean, or finite enum values accepted by the metric-label capability.
- Metric value type must be numeric. Counter and gauge reject buckets. Histogram requires a non-empty, finite, strictly increasing constant sequence representable by the value type. A histogram may omit authored buckets only when the selected runtime/profile supplies a typed default bucket policy; otherwise semantic admission reports a missing-bucket error.
- Exporter, storage, retention, and aggregation backend selection are not source members.

## 9. Layer

```text
LayerDecl       ::= RetainedHead<"layer"> ":" LayerKind LayerBody
LayerKind       ::= "background" | "world_2d" | "character" | "effects"
                  | "dialogue" | "game_view" | "html_view" | "activity"
                  | "modal" | "overlay" | "debug" | "agent"
                  | "offscreen" | "custom"
LayerBody       ::= "{" LayerMember* "}"
LayerMember     ::= ParentMember | PhaseMember | ZMember | VisibleMember | TransformMember
                  | InputMember | HitTestMember | CaptureMember | AccessibilityMember
                  | ViewMember | ActivityMember
ParentMember    ::= "parent" "=" LayerRef Terminator?
PhaseMember     ::= "phase" "=" RenderPhase Terminator?
ZMember         ::= "z" "=" Expr Terminator?
VisibleMember   ::= "visible" "=" Expr Terminator?
TransformMember ::= "transform" "=" Expr Terminator?
InputMember     ::= "input" "=" InputPolicy Terminator?
HitTestMember   ::= "hit_test" "=" HitTestPolicy Terminator?
CaptureMember   ::= "capture" "=" CapturePolicy Terminator?
AccessibilityMember
                 ::= "accessibility" "=" AccessibilityPolicy Terminator?
ViewMember      ::= "view" "=" ViewRef Terminator?
ActivityMember  ::= "activity" "=" ActivityRef Terminator?

LayerRef        ::= RetainedReference<layer>
ViewRef         ::= RetainedReference<view>
ActivityRef     ::= RetainedReference<activity>
RenderPhase     ::= "background" | "world" | "characters" | "effects" | "dialogue"
                  | "game_view" | "html_view" | "modal" | "debug" | "agent_overlay"
InputPolicy     ::= "ignore" | "pass_through" | "hit_test" | "modal" | "capture"
HitTestPolicy   ::= "none" | "bounds" | "view_tree" | "object_id_mask"
CapturePolicy   ::= "none" | "color" | "object_id" | "mask" | "all"
AccessibilityPolicy
                 ::= "hidden" | "exposed" | "container"
```

Canonical form:

```arcw
pub layer @layer.dialogue dialogue_ui: dialogue {
    parent = @layer.root
    phase = dialogue
    z = 100
    visible = true
    input = hit_test
    hit_test = view_tree
    capture = none
    accessibility = container
    view = @view.MainDialogue
}
```

Rules:

- `root` is engine-owned and is not an authored `LayerKind`.
- Body is required and may be empty. Every named member is singleton.
- Default values are owned by the typed `LayerKind`/policy enums, not free match helpers:

| Kind | Default phase |
|---|---|
| background | background |
| world_2d | world |
| character | characters |
| effects | effects |
| dialogue | dialogue |
| game_view | game_view |
| html_view | html_view |
| activity | game_view |
| modal | modal |
| overlay | modal |
| debug | debug |
| agent | agent_overlay |
| offscreen | background |
| custom | world |

Other defaults: parent is the engine root; `z = 0`; visible is true; transform is identity; input is `hit_test`; hit-test is `bounds`; capture is `none`; accessibility is `exposed`; View and Activity references are absent.

- `z` must be a constant `i32`, visible a constant `Bool`, and transform a value admitted by the existing typed presentation transform owner.
- `view` and `activity` are mutually exclusive content owners. An `activity` kind requires an Activity reference before runtime materialization. A View reference is permitted only for `dialogue`, `game_view`, `html_view`, `modal`, `overlay`, `debug`, `agent`, and `custom`. Other kind/content mismatches are semantic errors.
- Parent existence and cycles are project-level checks over resolved Layer symbols.
- CSS, Takumi, raw style bags, and configured-resource semantics are not Layer grammar.

## 10. Rejected header components

For all seven authored families, any family-inappropriate generic parameter group, where clause, return arrow, contract, alias, origin clause, initializer, body, or trailing word is retained in an error/recovery child and diagnosed by that family or by `syntax.declaration.unexpected_header`. It never becomes a raw tail consumed by sema.
