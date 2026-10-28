# Sans I/O Core

`arcweft-core` は副作用を実行しない。入力を受け取り、次の状態と要求を返す。

Core が扱う program、bytecode、manifest reference、save snapshot、diagnostic、trace はすべて pure data。Core は path を開かず、filesystem、network、wall-clock、GPU/audio/device handle、Wasm runtime、Cranelift runtime を保持しない。外部 adapter は bytes/string へ serialize された bundle や task result を `RuntimeStepInput` / `TaskEvent` として渡す。

## Runtime ID domains

Runtime lookup IDs are typed canonical paths, not raw public strings. A
`FlowRuntimeId`, `EntryRuntimeId`, `RuntimeLineId`, or `StreamRuntimeId` owns a
runtime path whose Rust type supplies the family. The path itself therefore
does not store source-family prefixes such as `flow`, `entry`, `say`, or
`stream`.

Arcweft keeps three ID domains separate:

- Source references live in parser/HIR/lowering while relative syntax is still
  meaningful. They may contain family-qualified source spelling such as
  `@flow.main` or current/parent-relative addressing.
- Runtime IDs are execution lookup keys. A source-backed Flow retains the
  identity projected from its accepted declaration digest and the separate
  public label `flow.main`. Manually constructed runtime IDs such as `main`
  remain explicit inputs to the canonical runtime-ID API; they do not recreate
  the identity of an accepted source declaration.
- Public/debug labels are deliberate strings used in AWBC reports, manifests,
  logs, diagnostics, and user-facing output. Runtime code must not recover a
  lookup ID by splitting one of these labels.

Flow public labels also form the accepted selector registry for dynamic
`goto`. The immutable Flow table owns source-ordered definitions and invocation
schemas, plus derived identity/schema/selector indexes. An exact admitted
runtime identity takes precedence; otherwise one unique public selector selects
its existing definition, and multiple owners reject as ambiguous. This does
not reconstruct a checked identity from label text. A selector is operational
metadata: changing it can change target resolution and therefore changes the
Flow executable transcript. Purely diagnostic labels remain excluded.

Entity-reference value identity preserves the original runtime-path segment
boundaries, each Flow's retained public selector, and an imported reference's
complete Public/Flow/Line target and generation/type headers. Canonical value
digests, executable literal transcripts, the private literal source codec and
the AWBC binary codec take these fields from the same owning entity traversal.
A joined path label
does not substitute for a runtime identity, and observable selector metadata
does not disappear when a Flow is carried as a value or pattern.

The version-1 AWBC runtime-ID wire stores nonempty segment sequences. Decoding
admits every segment on the same cumulative collection, string and nesting
allowance before retaining the path. Imported references retain their explicit
closed target through the existing owner admission.

### Plan-local declaration origins

Every `RuntimeLocalDeclaration` retains its complete checked declaration source,
type, optional function type context, and immutable `RuntimeLocalPlacement`.
The source carries the origin plus authored binding kind, mutability and retained
storage policy. Placement carries the actual executable owner, physical frame
lifetime, slot mutability, and static first-initialization role. An input carrier
and its destructured pattern bindings have separate initialization roles; a
capture's source declaration does not by itself define its destination slot's
mutability. Its table ordinal only addresses the row and is not semantic identity.

Semantic admission issues checked construction requests. A Core use materializes
one dense slot per request; cloning the request retains that slot, while a fresh
body request creates an independent destination. Unused requests create no final
rows. Before publication, actual FunctionSite, helper, method, Line and Stream
bodies claim their declarations in the sole construction table. Missing or
conflicting owners reject the complete publication. A Flow aliases its owning
FunctionSite body. ProjectCall callee/operand expressions and result declarations
belong to the caller. Line capture packets are explicit imports across frames.

Initialization roles describe static ingress or declaration, independently of
runtime occupancy and checked move/drop/replacement evidence. Whole-local
assignment can statically reinitialize a moved place according to the language's
ownership rules. Semantic transcripts resolve owner coordinates to accepted
definition identities and roles; they never descend back into completed body
digests or hash frame addresses.

The origin grammar distinguishes an accepted binding coordinate, a whole
function parameter identity, an evaluated result's function definition identity,
and a generated local coordinate. A generated coordinate combines one accepted
expression or lexical scope path with its typed semantic purpose, including any
authored operand position. Closed types, HIR allocation IDs, table ordinals,
runtime values, and debug labels do not enter that coordinate. Distinct generic
instantiations retain the same authored origin while their runtime types differ.

Generated declarations are first described by one borrowed inventory for the
complete executable view. Collection Map uses one item/result pair and iterator
state, independently of the source length. Array Map additionally owns a Vec
accumulator and consumes it into the exact Array result after iteration.
The inventory counts locals in a fixed-width domain and rejects duplicate
semantic roles. Allocation sizes are derived only after admission. The Core
construction issuer checks the remaining local namespace before declarations
are materialized. Expression and lexical Scope requests then enter
one atomic type/local admission; an invalid suffix cannot publish a prefix.
The capacity query does not reserve identities, and final admission rechecks the
same issuer. Local namespace capacity, per-function frame limits, and executable
image seal budgets remain distinct checks.

A selected standard Map has one typed control owner for its constructor family,
prepared receiver, prepared callback, result declaration, and complete ordinary
body. Preparation evaluates both operands once in their accepted source order.
The body calls the original callable through the ordinary same-fiber application
boundary; its result is transferred to the caller only after the selected
iteration or variant has completed. Option and Result transfer their branch
value through the existing value-producing control join before destroying the
branch scope. The result declaration must be visible at the body's exit, and
the body cannot change its caller's lexical scope stack. Semantic transcripts,
resource inventories, and executable seals visit that owned body exactly once.
Native execution and canonical AWBC lowering consume the same admitted body.
When the admitted operation suffix aliases the collection and consumes it with
Sum, that same control owns the source-ordered suffix and its mapping boundary.
The mapping's generated locals remain private; the original collection, aliases,
and consuming result retain their caller scope. The semantic transcript includes
the completion kind, mapping boundary, and complete body. Compiler statistics
derive formed consumers and logical Flow callback sites from this final typed
plan. Flow is an invocation projection of its immutable FunctionSite row, which
is visited once. Physical Array expansion and separately owned implementation
bodies do not multiply the Flow invocation-site count.

Ignoring a fresh Unit result adds no separate executable discard operation.
Its producing effect or control continuation has already completed. Other
discarded expressions still execute for their traps, moves, and resource drops.

Runtime semantic facts retain an accepted origin for every selected expression,
including an effect carrier that owns no runtime value type. A static callee's
namespace spelling is omitted according to the selected call inventory.
Publication rejects missing origins, evidence issued for another HIR allocation,
and facts outside the selected expression topology. Type queries still omit
type-free carriers. The origin and optional type share the same owned fact row.

Manually constructed plans supply explicit identities for their own declared
semantic recipes. Transporting such an identity, or a source-backed origin,
does not itself establish executable body admission or the complete image seal.

### Checked variant values

`RuntimeValue::Variant` owns one `RuntimeVariantValue` with private owner,
ordinal, canonical name, source-shaped payload and optional scoped effect
instantiation. Consumers read its accessors; they cannot assemble or mutate a
live header. Consuming `into_payload` transfers the retained child and destroys
the enclosing header. A copied scoped value retains the same immutable
instantiation lease. The internal logical View also borrows this same carrier;
it has no copied variant header or reconstruction path. Native and AWBC pattern
comparison share the value owner's logical comparison implementation.

Construction uses the complete defining authority. `RuntimeNominalSchemaGraph`
admits a source-ordered case and payload through its original graph.
`RuntimeProgramTypes` selects the original Plan or AWBC case and validates the
complete candidate before publication. Native and pure execution use the
selected Plan case with the active function environment; AWBC uses the selected
type row with its active frame. A finite `RuntimeCheckedVariantCase` projection
does not replace those descriptors for recursive, generic or function-bearing
payloads. Builtin helpers use the sole builtin registry and retain its physical
single-item tuple payload ABI.

Nominal live values reject generic Serde serialization and deserialization.
Their inert snapshot headers are admitted against the selected program before
children become live values. Builtin snapshot headers likewise check the exact
case, name, payload presence and tuple shape before child restore. Snapshot and
codec contract versions remain `1`.

An arbitrary public payload mapper cannot preserve this admission invariant.
Core owns the narrow finite-float normalization and sparse typed patch merge
operations. They retain container identities and scoped origins; merge rejects
affine inputs before any copying and both operations enforce the existing
nesting limit. Normalization uses canonical ordered sequence rows so equivalent
Native and AWBC values do not publish different storage representations in a
codec. Callers still admit the resulting value against its expected
role or executable type at the consuming boundary.

## Execution ownership

`Engine` owns the admitted plan and the root/child fibers. A `FlowFiber` carries
execution identity and one `NativeFlowExecutionFrame`; that frame owns its
original program authority, environment, pending work, control continuations,
observations, stream state, and terminal invocation result. Live execution and
inert private rollback use the same owning grammar with different storage types.
The authoritative definitions are in [engine.rs](../../crates/arcweft-core/src/engine.rs).

Flow roots and structured function sites share `RuntimeExecutableBody`.
Consumers read `RuntimeFlow::body()` and its `effects()` / `ops()` accessors;
construction admits the closed effect row and operations together. Semantic
inference supplies omitted Flow rows, while authored bounds retain their
declared scopes and unused permissions. AWBC signatures preserve that row and
the verifier checks returning calls using `EffectId` coverage. Terminal Flow
transfers begin a new effect scope as described in
[control transfer](../01-language/control-transfer-return-out-yield.md).

`Engine::step(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult` は
`RuntimeStepMode` と `RuntimeStepBudget::max_ops` に従って lowered flow op を
内部 drain する。`OneOp` は最大 1 op、`Drain` / `Server` は blocked/done/failed
または budget まで進め、`Game` は presentation-visible output で host に制御を返す。
`Dialogue` は line task group を実行し、`Choice` は入力待ち、`Await` は元Sourceで
認証されたNeed／ThreadHandleを評価して停止・復帰する。`Let` / `LetElse` /
`If` / `IfLet` / `Match` / `Loop` / `While` / `WhileLet` / `For` は
`RuntimeValue` / `RuntimeExpr` / `RuntimePattern` と `RuntimeEnv` で評価され、
選択されたbodyは元program内のsource rangeとしてframeのwork queueへ入り、
runtime bindingとiteratorの進行は型付きcontinuationが所有する。
queueは `FlowOp` の複製列ではなく `NativePendingWork` を保持する。
[work owner](../../crates/arcweft-core/src/engine/work.rs) と
[実行・停止復帰の契約](runtime-step-and-executors.md) を参照する。
frameのcontrol stackはlexical scopeとloop continuationを保持し、
`break` / `continue` が body 内の残り op と scope-local binding を破棄して
最も近い loop/while/while-let continuation へ移るために使われる。
`let name = scope { ... }` は final expression を scope 内で評価してから外側の
pattern に束縛し、`let name = loop { break expr }` は `break expr` の値を
loop result pattern に束縛する。
Flow parameter は `RuntimeStepInput` に含まれない。低レベルの明示 Flow 起動は、
plan-owned parameter schema に対する complete な
`FlowParameterCoordinate` 順の値列を `RuntimePlan::seal_flow_invocation` で検証し、
affine な `RuntimeFlowInvocation` として executor 構築時に一度だけ消費する。
Entry と route は checked Entry plan が同じ coordinate ABI を発行する。最初の op
実行後に step input から parameter local を再束縛する経路はない。
branch / match / while-let pattern binding は選択された lexical scope にだけ束縛される。
Match はコンテナ全体を覆う共通 Block を作らず、通常の arm はそれぞれ独立した
`MatchArm` scope を所有し、Thread の braced arm だけが単一の `Block` scope を所有する。
binding は guard 評価後や arm 終了後には外側や sibling arm へ漏れない。
`Goto` / `GotoExpr` / `Return` / `ReturnExpr` は所有frameのwork・結果・statusを遷移させる。
`RuntimeStepInput` の task/host ingress は replay-stable order に正規化され、外部
capability の `StreamPlan` と stream state に反映される。stream payload は
`RuntimePayload` として `RuntimeValue` shape を保持し、CLI 表示だけが human-readable
label に変換する。frameの `observations`
は emitted log / signal / metric / event を累積し、CLI/LSP/test/replay tooling が
JSON で観測できる。
実 thread、wall-clock、renderer、audio、device、filesystem は adapter 側の責務である。
`ScopeExit::Completed | Cancelled | Failed` は outcome-guarded `defer` stack の
選択に使う。

## Host requests and effects

`RuntimeStepOutput` separates pure effect data in `RuntimeEffectBatch` from
host requests in `HostRequestBatch`. Task submissions, audio, host calls,
line commands, content requests, and next-step root events keep their typed
owners; they are requests for adapters to execute. File writes, compression,
encryption, cloud sync, and device I/O stay outside Core.
The complete input/output grammar is owned by
[step.rs](../../crates/arcweft-core/src/step.rs); materialized line effects are
owned by [effect.rs](../../crates/arcweft-core/src/effect.rs).

## Dialogue line task groups

A dialogue `with` block lowers to a sealed, plan-owned `LineTaskGroup`.
Its dense node table owns Sequence, Start, Parallel, Child, and Action nodes;
Action leaves contain typed `FlowOp` bodies. The group retains its definition,
captures, activation exports, result type, handle sites, cancellation rules,
and cleanup. The owning grammar is in
[line_task.rs](../../crates/arcweft-core/src/line_task.rs).

The shared activation reducer schedules those bodies and retains their exact
capture packets. Native child execution uses the ordinary frame and original
program authority. This is deterministic VM/runtime work; OS threads and I/O
remain adapter responsibilities. Lifecycle and suspension rules belong to
[RuntimeStep and Executors](runtime-step-and-executors.md).

`RuntimeEffectExpr::Assert` evaluates its condition as a typed `Bool` inside
the Sans-I/O runtime. `true` produces no host request. Only `false`
materializes `LineEffectRequest::Assert`, so hosts treat that request as an
already-established failure and never parse `condition` text to decide whether
it failed. A non-`Bool` condition is a typed runtime materialization error.

The persisted failure identity is the checked 16-byte
`RuntimeAssertionGuardId`; condition and message strings are presentation data.
Hosts return `RuntimeAssertionFailure` as core data. A fresh compiler session
may join that guard to the exact statement, zero-based condition index, mode,
and revision-bound source span through a non-serialized runtime-plan inventory
bound to the exact runtime-plan artifact fingerprint. Without that exact
association, CLI, LSP, Agent, and debug presentation use persisted evidence and
must not fabricate HIR or syntax identity. Both paths emit the stable code
`runtime.assertion_failed`.

Ordinary line-plan `wait(...)` admits only `Duration`. A mark trigger retains
its typed `RuntimeDialogueMarkId`; it cannot become a string wait target.
The checked suspension retains its exact operand. RuntimePlan lowers that operand
through the ordinary Flow value continuation, including calls and branches, into
one evaluated Wait effect. Materialization accepts the typed Duration value and
never parses a formatted label.

The version-one AWBC Wait payload declares one Duration source: `Input` consumes
the sole parameter of an exact Duration signature; `Constant` names a Duration
constant and has no parameters. Static effect packets, including Choice effects,
use the constant source without a register context. These are alternatives in
one payload grammar; neither source falls back to the other. Both have no result.
The decoder rejects unknown source tags and the former array-shaped Serde payload.

`yield` is not a line effect. It lowers only through Stream generation plans;
a dialogue line plan must use `out` for line-scope values.

`on` and `at` do not lower by inserting synthetic `wait` effects. They become
task triggers so scheduling and replay can reason about when a child task starts.
`together` preserves the parallel boundary and the lowering pass rejects
obvious deterministic conflicts such as two children writing the same signal or
line `out` value unless the effect category is append-only, such as structured
logs or event emission.

Checked flows retain their `RuntimeFlow` / `RuntimeExecutableBody` owners.
`FlowOp::Dialogue` carries a typed content identity, target, and result binding;
it does not recover its task graph from an untyped task-group index. Await,
calls, control bodies, defer registrations, and effects use the owning
[FlowOp grammar](../../crates/arcweft-core/src/plan.rs).

Runtime lowering errors on unsupported flow
items instead of converting them to `Noop`.

Raw line-plan statements fail lowering. They are not reparsed, silently accepted,
or dropped from the runtime plan.

`defer` is not thread-specific syntax. It registers cleanup on the current
runtime scope. In the line-plan model, the line/root scope, child
thread scopes, and event-handler scopes each have a cleanup stack. A bare
`defer` must run when its owning scope exits, including normal completion,
early control transfer, line cancellation, and child-task cancellation.
Outcome-guarded forms `defer on completed`, `defer on cancelled`, and
`defer on failed` retain their place in the scope's registration order. The
shared activation assigns each reached registration a monotonic identity distinct
from the source defer site; the identity and capture packet survive save/restore
so repeated visits to one site remain separate work items. The
unwinder visits registrations in reverse order, runs bodies whose filter
matches the current exit, and releases captured resources for every skipped
registration.
The activation fixes the exit reason before draining the stack. It moves one
matching capture packet to a dialogue-owned child at a time and waits for that
exact registration ID to finish before selecting the next; a body failure does
not restart the stack with a different filter.

Lifetime registry paths are typed static keys, not stringly dynamic maps.
The core model keeps the data Sans I/O; host backends receive deterministic
effect requests instead of direct mutation.

```text
'frame
'tick
'cue <= 'line <= 'scene <= 'flow <= 'session <= 'global
'persistent
```

`'persistent` is storage-backed and is checked separately from ordinary runtime
memory. Reads from an upper scope are allowed when the key is guaranteed, or
when the access is optional such as `'flow.flags?`. Writes to upper scopes lower
to replayable state-update events and require explicit capabilities.

```arcw
flow opening(state: GameState)
effects { state.write('flow), state.write('global) }
{
    alice()[見たことにする。[mark @.seen][p]]
    with {
        on mark(@.seen) {
            'flow.flags.seen_alice_intro <- true
            'global.settings.skip_seen <- true
        }
    }
}
```

Line lifetime registry paths such as `'line.focus` are owned by the line scope.
Guaranteed keys can be read directly; optional reads use `'line.focus?`. Drop
operations remove registered values and run their adapter cleanup policy through
emitted requests. Lower-scope keys such as `'line.*` are not available outside
their active scope or across a thread boundary unless an explicit move/share or
detach operation makes the capture safe.

The checker treats lifetime operations as intrinsics with typestate effects:

```text
drop
drop_optional
on_drop
expose
share
detach
promote
promote_unchecked
clone_owned
```

Safe `promote('target)` requires owned data, no shorter lifetime references, and
replay-safe serialization when the target is `session`, `global`, or
`persistent`. `promote_unchecked` is an unsafe-like lifetime proof escape hatch:
it requires an explicit `unsafe lifetime` region, a reason string, and a project
capability. It cannot bypass determinism or the Sans I/O boundary.

Line-level cleanup is represented by line-scope `defer`; there is no separate
line-plan cleanup keyword. Bare `defer` runs after line-scoped child tasks are
cancelled/joined and their defer stacks have run, but before any remaining
automatic line-registry drops. Flow-level threads, line-plan threads, handler
tasks, and ordinary lexical runtime scopes share the same cleanup model. The VM
must treat cancellation as scope exit for cleanup: child task cancellation first
unwinds that task's defer stack, then the line task group runs the matching
line-scope defer stacks.

## Determinism

- wall-clock を state に入れない。
- random は seeded RNG capability。
- task 完了は frame boundary で正規化。
- replay は `RuntimeStepInput` 列と task/audio/view result を記録。

## Dispatch and caches in core

`arcweft-core` は OS callback を直接副作用として実行しない。Typed owner-local
handler output is committed at its phase boundary as semantic action, `Command`,
or scoped update.

Subsystem caching は `arcweft-core` の意味論を変えない。cache hit/miss は state
hash に含めず、pure computation の結果だけを再利用する。詳細は
[Runtime Dispatch and Caches](hooks-memoization.md) を参照。

