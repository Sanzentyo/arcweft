# Path, lifetime, call, and Thread contract

## Path resolution

A `HirPath` retains its root and typed segments exactly. Resolution takes `HirPathResolutionContext { snapshot, owner_scope }`.

- `ImplicitCrate`: consult the immutable import-alias table for the first segment. A unique alias substitutes its published typed target while retaining external project identity. If no alias exists, start at the current project's crate root. Ambiguous aliases poison the path; they never fall back.
- `Crate`: start at the current project's crate root and do not consult aliases for the root.
- `SelfModule`: start at the owner module.
- `Super { depth }`: walk exactly `depth` canonical module parents; escaping the crate poisons the path. Depth zero is canonical SelfModule.

Resolution consumes typed `HirPathSegment` values. Project/external symbols preserve hyphen-capable `HirProjectSymbolSegment`; language identifiers use `HirName`. Resolution returns a typed published symbol identity or a typed issue. A source label is never split.

Source query roles are `PathRoot` and `PathSegment { ordinal }`. The semantic path contains no span. Structural equality compares root and segments. Resolved-target equality compares publication identities under the same project generation.

## Type regions versus registry lifetime

`HirTypeRegion` appears only in HIR type nodes. Named regions carry `HirRegionName`; elided regions carry a `SyntheticKey` with the owning TypeId, role ElidedRegion, ordinal zero. Region equality is nominal for named regions and key identity for elided regions.

`HirLifetimeRegistryPath` appears only in runtime registry operations. Scope variants are Frame, Tick, Cue, Line, Scene, Flow, Session, Global, Persistent, and Named. Ordered key segments are validated identifiers. `LifetimePath` expression means Read; its `optional` bit is the authored `?`. Write, MoveOut, Drop, and Expose are statement-only modes. Optional non-read access is invalid. Registry equality compares scope, segments, and optionality; it never compares a type region.

## Ordinary and associated calls

`HirCallExpr` owns one callee and ordered arguments.

- Value callee: one same-module ExprId.
- Associated type callee: one same-module TypeId root, a member HirName, and exact separator category `DotFallback` or `ExplicitDoubleColon`.

For `target.member(...)`, the checker first checks target as a value expression. Any value-space result, including a value-space error, owns the call. Nominal fallback occurs only when typed value lookup returns definitive absence. Explicit `Type::member` is nominal-only. Turbofish and generic delimiters are part of the authored TypeId tree, not a third call-separator category: `Vec::<T>::with_capacity` has `ExplicitDoubleColon`, while its receiver TypeId source components retain `::<T>`. The TypeId tree retains generic parameters, aliases, qualified/project identity, and its own source components. `Vec<T>.with_capacity` projects that tree to the existing nominal product and then directly to `CallCallee::AssociatedType`. Bare `Vec.with_capacity` fails generic arity before candidate admission.

The shared resolver preserves its existing precedence and accounting. Environment methods precede capacity methods; capacity precedes associated trait methods. Untyped/data-last fallback is ineligible. Candidate attempts and retained results are each at most two. No second resolver, Capacity-only enum, argument replay pass, or signature-help candidate inventory may be added.

Call child source roles are Callee, AssociatedReceiver, AssociatedSeparator, AssociatedMember, and `CallArgument { argument, part }`. Receiver generic/turbofish delimiters remain TypeId source components. Ordinary argument limit is 128. RichText call contexts call the same constructor with limit 32. Missing callee/operand remains typed poison when the call family is known; an unclassifiable fragment becomes Error.

## Thread body and runtime projection

Thread lowering creates one child ScopeId and an ordered `HirThreadBody`. Every source `FlowItem` projects directly, in source order, to the exhaustive `HirThreadFlowItem` variant and a typed StmtId or dialogue-application ExprId. `Stmt`, `Choice`, `If`, `IfLet`, `Match`, `Loop`, `While`, `WhileLet`, `For`, `Select`, `SourceLocale`, `Scope`, `Include`, and `AwaitWith` retain their statement/flow identity; `SpeakerLine` and `ContentCall` become the existing typed dialogue-application expression owner; parser recovery becomes `Error(StmtId)`. There is no block expression ID and no tail. An empty authored body is valid and evaluates to Unit; only an absent required body is `MissingBody`. The Thread expression yields `ThreadHandle<Unit>`.

Bindings created by one body item become visible only to later items in the child scope according to the corresponding statement/flow rule. They never leak to the parent or sibling Thread. Nested scopes own their own locals.

Attached admission adds the task to the parent cancellation set. Parent cancellation or scope exit cancels and joins it. Detached admission transfers the task to the scheduler owner; parent scope exit does not join it. Detached capture validation requires owned/static captures and rejects frame/tick/cue/line registry borrows. Both modes return a handle. Explicit handle cancellation and runtime shutdown cancel detached work. Poisoned Thread HIR has no runtime-plan node and cannot execute.

`arcweft-runtime-plan` owns the projection:

```rust
pub struct RuntimeThreadPlan {
    handle: RuntimeValueId,
    mode: RuntimeThreadMode,
    body: Box<[RuntimeFlowStep]>,
    cancellation: RuntimeCancellationOwner,
}
```

The projection consumes typed IDs and checker facts only. It does not reopen syntax or infer mode/name from source.
