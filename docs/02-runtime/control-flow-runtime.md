# Runtime Notes: Control Flow, Patterns, and Loops

## HIR representation

```rust
pub enum Expr {
    If(IfExpr),
    Match(MatchExpr),
    Loop(LoopExpr),
    Block(BlockExpr),
    Scope(ScopeExpr),
    Await(AwaitExpr),
    TryAwait(TryAwaitExpr),
    // ...
}

pub enum Stmt {
    Let { pattern: Pattern, value: Expr },
    LetElse { pattern: Pattern, value: Expr, else_block: Block },
    Scope(ScopeStmt),
    While(WhileStmt),
    WhileLet(WhileLetStmt),
    For(ForStmt),
    Break(Option<Expr>),
    Continue,
    Return(Expr),
    Out(Expr),
    Yield(Expr),
    Expr(Expr),
}
```

`ScopeExpr` / `ScopeStmt` represent `scope name? { ... }`; a bare statement
`{ ... }` lowers as the same scope node with no name. Scope nodes behave like
lexical blocks for evaluation and type checking. When present, the name is
carried in HIR so diagnostics, trace frames, LSP/debug views, and ID generation
can recover the author-visible scope path.

## Type checking

`block` / `scope`:

```text
expression position:
  final expression determines the value type

statement position:
  lexical scope returns Unit unless an explicit transfer leaves the continuation

named scope:
  same typing as block; name does not change the value type
  omitted name creates no generated-ID scope segment
```

`if`:

```text
value position:
  else required, branch types unify

statement position:
  else optional, type Unit
```

`match`:

```text
exhaustive when used in value or statement position
arm types unify when value-producing
```

`loop`:

```text
break expr values determine loop type
break without expr contributes Unit
no reachable break => Never
```

`while` / `for`:

```text
Unit
break expr disallowed
```

## Pattern checking

Pattern checking has three phases:

```text
1. Shape check:
   Does pattern shape match scrutinee type?

2. Binding collection:
   Introduce locals with inferred types.

3. Exhaustiveness / reachability:
   match arms checked for completeness and unreachable arms.
```

## let-else definite assignment

Bindings from `let PAT = EXPR else { ... }` become available after the statement only if the else block diverges.

Divergence examples:

```text
return
goto
break
continue
panic
Never-returning function
```

## Interpreter/VM lowering

The current Rust runtime slice lowers checked HIR into pure data:

```rust
pub enum RuntimeExpr {
    Value(RuntimeValue),
    Local(String),
    EntityRef(String),
    Tuple(Vec<RuntimeExpr>),
    List(Vec<RuntimeExpr>),
    Record(Vec<RuntimeFieldExpr>),
    Variant { path: Option<String>, name: String, payload: Option<Box<RuntimeExpr>> },
    Field { target: Box<RuntimeExpr>, field: String },
    Unary { op: RuntimeUnaryOp, expr: Box<RuntimeExpr> },
    Binary { lhs: Box<RuntimeExpr>, op: RuntimeBinaryOp, rhs: Box<RuntimeExpr> },
    If { condition: Box<RuntimeExpr>, then_expr: Box<RuntimeExpr>, else_expr: Box<RuntimeExpr> },
    Match { scrutinee: Box<RuntimeExpr>, arms: Vec<RuntimeExprMatchArm> },
}

pub enum RuntimePattern {
    Ident(String),
    MutIdent(String),
    Discard,
    Literal(RuntimeValue),
    Entity(String),
    Tuple(Vec<RuntimePattern>),
    Record { path: Option<String>, fields: Vec<RuntimeRecordPatternField>, rest: RuntimePatternRest },
    List { items: Vec<RuntimePattern>, rest: RuntimePatternRest },
    Variant { path: Option<String>, name: String, payload: Option<Box<RuntimePattern>> },
    Whole { name: String, pattern: Box<RuntimePattern> },
    Typed { name: String, ty: String },
}

pub enum RuntimePatternRest {
    Exact,
    Ignore,
    Bind(String),
}
```

For both records and bracket sequences, `Exact` rejects unmentioned values and
`Ignore` accepts them without a binding. Record `Bind` receives the original
complete record; sequence `Bind` receives the unmatched tail.

This evaluator is deliberately small and Sans I/O. It handles deterministic
bool/int/string/entity/list/tuple/record/variant values and structured lexical
bindings. Flow invocation parameters are admitted once through a complete,
coordinate-addressed `RuntimeFlowInvocation` before execution; they are not
step input. Function calls, overloads, numeric unit coercions, and full
type-directed evaluation remain semantic/HIR work before this runtime layer.

`scope name { ... }` lowers to the same control-flow shape as a lexical block,
plus a scope-name push/pop around ID-bearing constructs and trace/debug frames.

```text
PushNamedScope(name)
  body
PopNamedScope(name)
```

The scope path affects only generated or relative line, text-key, choice, and
option IDs created inside the scope. It does not change ordinary entity
references, local variable lookup, or the value returned by the block.

`loop` lowers to a runtime loop frame plus scoped body operations. The frame is
the continuation target for `break` and `continue`; the body scope is popped
before either transfer, so body-local bindings do not leak.

```text
PushLoopFrame(body)
  EnterScope
    body
  ExitScope
  LoopNext(body)
Break(value?) -> pop body scopes, discard queued body ops, pop loop frame
Continue -> pop body scopes, discard queued body ops, enqueue LoopNext(body)
```

`while` and `while let` use the same frame stack. `while let` keeps successful
pattern bindings in the body scope only. Guard evaluation receives temporary
copies of only the pattern-bound values selected by semantic Copy obligations;
the original scrutinee remains owned until the guard succeeds. A failed guard
discards those temporary copies before trying the next arm or restoring the
outer environment.

```text
PushWhileLetFrame(pattern, expr, guard, body)
  EnterScope
    Bind(pattern bindings)
    body
  ExitScope
  WhileLetNext(pattern, expr, guard, body)
```

Type checking still keeps `while` and `while let` as `Unit`; only `loop` can
produce a value through `break expr`.

## Replay

Control-flow constructs are deterministic as long as expression evaluation is deterministic. Loop iteration count is recorded only for debug traces, not as semantic state.

## Function semantic input evidence

The function transcript uses the accepted function definition and its actual
owned expression or executable body. Code references are accepted definition
leaves; ordinary recursive calls do not recursively expand called bodies. The
complete executable image commits every referenced definition's body row.

Callable state and origin references form a finite typed graph. The semantic
visitor computes each reachable definition/origin digest once per graph
transcript, preserving transition and partial-application source order. Private
state ordinals are memo keys only; no allocation coordinate is written into
these child digests. A Visiting edge rejects a forbidden cycle. Accepted code
and nominal references remain semantic leaves. An explicit borrowed-edge
stack avoids Rust recursion and avoids expanding shared subgraphs as trees.
Every graph visit, child transcript and emitted cached digest uses the same
work/byte meter; a child error poisons the parent before publication.

A retained input carries its origin, source role and transfer evidence from
`RuntimeFunctionInputBinding`. These are independent of frame-ingress
`Owned`/`Unrestricted` requirements. Origin tags are Binding (0), whole
Parameter (1), and EvaluatedResult (2), each followed by its accepted 32-byte
identity. Source tags are Capture (0), CapturedParameter (1), and Parameter
(2), followed by the source position; the two formal roles also include the
existing Value/Shared/Affine passing tag. Transfer is Transferred (0) plus
Copy/SnapshotClone/Move (0/1/2), ExternalBinding (1), or Formal (2).

In particular, extracting a body with external free bindings creates no
Copy/Move operation, and retaining a whole formal preserves its passing
evidence. Neither role may be inferred from a value's current occupancy,
type, local ordinal, or an ingress requirement. This corrects the narrower
three-mode capture field in the retained task-plan correction package: the
current producer transcript writes origin, semantic input type, source and
transfer evidence for each canonical retained input. The package remains a
historical design mirror; its omission does not authorize inventing a transfer.

The body-root transcript includes the full admitted signature/prologue and
owned body, including ordered task build-coordinate references. Producer
endpoints are ordered positions in that owned operation tree. Their path
contains balanced body roles, operation source ordinals, and the endpoint
ordinal within an operation, so empty branches and multiple AwaitMany
endpoints remain distinct. Completed task digests, expected keys, generations
and source/debug labels do not supply endpoint or task-reference authority.


## Static request role evidence

A checked execution argument slot owns its static request role identity. The
identity is minted after the callable/schema destination is validated, then
transported unchanged in the compiler's source-ordered runtime operand row.
A receiver or callee is not an argument role; an Argument row must carry this
evidence before the runtime call is published.

Accepted record and variant payload field roles use their existing accepted
field identities. An open named argument uses the existing schema-owned open
argument identity, whose validated binding bytes are semantic data. A fixed
project formal uses its source-independent declaration identity and checked
group/parameter coordinate. Other fixed formals use their checked callable
identity and formal coordinate. Source names, expression values, ABI allocation
positions, and call-site revisions do not reconstruct a fixed formal role.
Types, source order, passing and request paths remain distinct transcript roles.

The version-one checked request-role domain is
`arcweft.lang.checked-request-role.v1\0`. Its closed family tags are accepted
variant payload field (0), accepted record field (1), fixed formal (2), and
open argument (3). Field/open families contain the existing 32-byte identity.
A fixed formal contains project declaration (0) or checked callable (1)
identity, followed by group and parameter ordinals. Consumers receive the
opaque issued identity, without a byte constructor or source-based fallback.


Core Host argument templates retain that identity alongside the executable
value in each positional/named/spread variant. Builder admission copies the
identity while admitting the expression; evaluation consumes the value and
binding mode. Display names remain runtime routing data, not a reconstruction
source for the static request transcript.

A producer endpoint borrows its actual plan, function and operation. The Host
request encoder accepts the corresponding HostCall through this capability;
it rejects a foreign plan or a different endpoint operation before issuing Q.
Argument roles and paths are source ordered: each starts with Operand(ordinal),
named arguments add NamedArgument(accepted role identity), and spread arguments
add SpreadArgument(ordinal). The Host request argument vector owns no additional
request field rows. Literal payloads and display names are not read by this
projection; the actual expression type and admitted source role are retained.

Source classification follows the actual expression without evaluation.
Need-typed sources are NeedHandle. Local reads use the function's accepted
capture prologue to distinguish Capture from Local; field/tuple/record reads
are Projection. Aggregate constructions use AggregateItem. Let/Scope/Assign
wrappers follow their result expression iteratively. Other eager computation
results, including operators and conditionals, use CallResult. Classification,
role/path projection and encoding share the same checked work/byte meter.
