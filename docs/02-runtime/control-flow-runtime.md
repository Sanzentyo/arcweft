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
    Record { path: Option<String>, fields: Vec<RuntimeRecordPatternField>, rest: bool },
    List { items: Vec<RuntimePattern>, rest: Option<String> },
    Variant { path: Option<String>, name: String, payload: Option<Box<RuntimePattern>> },
    Whole { name: String, pattern: Box<RuntimePattern> },
    Typed { name: String, ty: String },
}
```

This evaluator is deliberately small and Sans I/O. It handles deterministic
bool/int/string/entity/list/tuple/record/variant values, structured bindings,
and explicit external bindings from `FrameInput`. Function calls, overloads,
numeric unit coercions, and full type-directed evaluation remain semantic/HIR
work before this runtime layer.

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

`loop` lowers to a block with a loop-exit slot.

```text
LoopStart
  body
  Jump LoopStart
LoopBreak(value) -> write exit slot, jump LoopEnd
LoopEnd -> read exit slot
```

`while let` lowers to repeated match:

```awft
loop {
    match expr {
        PAT when guard => body
        _ => break
    }
}
```

but type checking keeps `while let` as Unit.

## Replay

Control-flow constructs are deterministic as long as expression evaluation is deterministic. Loop iteration count is recorded only for debug traces, not as semantic state.
