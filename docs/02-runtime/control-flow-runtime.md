# Runtime Notes: Control Flow, Patterns, and Loops

## HIR representation

```rust
pub enum Expr {
    If(IfExpr),
    Match(MatchExpr),
    Loop(LoopExpr),
    Block(BlockExpr),
    Await(AwaitExpr),
    TryAwait(TryAwaitExpr),
    // ...
}

pub enum Stmt {
    Let { pattern: Pattern, value: Expr },
    LetElse { pattern: Pattern, value: Expr, else_block: Block },
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

## Type checking

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
