# Const block and compile-time phase fence

`const { ... }` is Arcweft's compile-time phase boundary. It evaluates one
value block during compilation and replaces the source expression with the
resulting typed constant.

```arcw
let answer = const {
    let base = 40
    base + 2
}
```

The expression has the tail type `T`; Arcweft does not introduce `Const<T>`.
The final RuntimePlan retains only an admitted `RuntimeValue`, and final AWBC
uses the existing constant table and `LoadConst`. The temporary evaluator body
does not enter the product.

## Surface and ownership

```text
ConstBlockExpr := "const" BlockExpr
```

Only the braced expression form is introduced. Arcweft does not add
`const(expr)`, an indentation form, `const fn`, `comptime`, or a const type
wrapper. `const(...)` remains an ordinary call and `foo.const` remains an
ordinary member/path spelling.

Syntax and HIR extend their existing computation-block owner with `Const`:

```text
ComputationBlock(Result | Option | Seq | Stream | Const)
```

Sharing the value-block structure does not make Const a carrier block.
`result {}` and `option {}` own residual propagation; `const {}` owns a phase
fence.

An empty const block evaluates to `Unit`. A Result- or Option-valued tail is
not wrapped or flattened:

```arcw
const {
    parse(TEXT)
}
// Result<T, E>, when parse(TEXT): Result<T, E>
```

## Checked admission

The checked rule has two distinct admissions:

```text
ConstTypeAdmissible(T)
    T has one closed durable constant representation

ConstValueAdmissible(T, value)
    value matches T, its accepted layout, ownership, and resource limits
```

Both are required. A value such as `.None` does not make
`Option<Need<Image>>` admissible; the complete type is rejected because
`Need<Image>` is temporal.

The body must have an empty runtime effect row, but an empty effect row alone
does not prove const evaluability. Dynamic dispatch, runtime captures,
unsupported intrinsics, and other operations outside the ConstEval profile are
rejected independently.

The v1 admissible value families are closed:

- Unit, Bool, exact-width integers, exact F32/F64 bits, String, Char, Duration,
  EntityRef, and bounded Bytes;
- tuples, materialized sequences, ranges, records, nominal records/newtypes,
  variants, Result, Option, and fixed-shape matrix/tensor values when every
  child is admissible;
- exact-identity opaque values only when their producer registers a const codec
  and the value has unrestricted ownership.

Need, Stream, task/thread/line handles, Agent, Function/closure, Iterator,
Reduction, dynamic/open types, borrows, affine values, and values without an
exact durable layout are rejected. The dynamic ownership check reuses the
existing `RuntimeValueOwnership` authority and admits only `Unrestricted`.

## Phase and control fence

Const participates in the same checked lexical-boundary walk as Try, but it is
a hard phase fence. A residual cannot escape from const evaluation to a runtime
callable:

```arcw
fn load() -> Result<Model, LoadError> {
    const {
        try parse_model(TEXT)
    }
}
// error: residual crosses the const phase fence
```

Complete the carrier inside the const phase:

```arcw
let parsed = const {
    result {
        try parse_model(TEXT)
    }
}
```

`Result::Err` and `Option::None` are ordinary successful const-evaluation
outputs. Fuel exhaustion, arithmetic traps, invalid indexing, failed const
assertions, unsupported calls, or verifier failures are compiler diagnostics;
they are not converted into carrier residuals.

Control transfer must remain inside the phase:

- `break` and `continue` may target only loops inside const evaluation;
- a const-evaluable project function may return from its own frame;
- `return` may not target an enclosing runtime callable;
- Await, cancellation, yield/stream emission, goto/Flow transfer, Thread, and
  task start are rejected.

## Captures and callable selection

A const block may reference types, nominal declarations, enum cases, exact
const-callables, registered immutable const values, and previously evaluated
const-block values. It may not capture runtime parameters, mutable locals,
Signal/state/View state, capabilities, devices, resources, or handles.

```arcw
let base = const { 40 }
let answer = const { base + 2 }
```

An ordinary `let base = 40` is not implicitly phase-lifted. Const-value
dependencies form a DAG. Const-callable recursion is allowed, but deterministic
instruction fuel bounds nontermination.

Arcweft does not add `const fn` in v1. Sema infers const eligibility for one
closed callable specialization from the already selected callable identity,
closed substitutions, empty exposed effect row, executable body or registered
const intrinsic, admissible captures, and admissible result type. It never
re-resolves a call by spelling.

Extern/native const evaluation requires an explicit registered intrinsic
contract. An arbitrary Rust callback is never invoked during compilation.
Concrete statically selected trait implementations may be admitted; dynamic
trait dispatch and escaping Function values are not.

## Evaluation and lowering

Const evaluation uses a restricted AWBC execution profile rather than a second
HIR interpreter:

```text
checked Const block
    -> temporary typed RuntimePlan/AWBC slice
    -> ConstEval verifier profile
    -> deterministic AWBC VM with fuel and value budgets
    -> typed RuntimeValue
    -> RuntimeExprKind::Value
    -> ordinary typed AWBC constant + LoadConst
```

The profile admits deterministic value construction, projection, arithmetic,
pattern binding, branch/match/loop CFG, exact direct calls, compiler-generated
synthetic functions, and registered const intrinsics. It rejects host calls,
effects, task/Await/suspension, streams, dialogue/presentation operations,
device/resource work, runtime capabilities, and unknown or dynamic call
targets.

The constant interner is typed. It keys canonical values by the exact AWBC type
and canonical bytes, never by `Debug` formatting or by shape inference from a
runtime value. Record fields use accepted layout order; variants retain their
owner and case; sequences use logical element order; opaque values retain
producer identity and codec revision.

All Arcweft-owned ABI, codec, cache, and evaluator revision markers remain
`1`. The unreleased schema evolves in place; no v2 reader or compatibility
path is introduced.

## Determinism, cache, and diagnostics

ConstEval uses deterministic instruction fuel, call-depth, value-depth,
node-count, byte-count, and sequence-size limits. It does not use wall-clock
timeouts. Float results retain exact bits, including NaN payloads.

The v1 production limits are fixed build inputs:

```text
instruction fuel    10,000,000
call depth          128
value depth         64
value nodes         1,000,000
value bytes         64 MiB
sequence items      1,000,000
```

Tests may inject smaller limits. User- or machine-specific production limits
are not accepted because they would make build success nondeterministic.

Incremental results belong to the existing build artifact system. The key
includes the accepted-HIR semantic digest, exact result layout, closed
substitutions, const captures, transitive callable bodies/interfaces,
registered intrinsic revisions, environment digest, target, ConstEval
revision, and limits digest. Cache hits are revalidated before publication.
The existing query inventory adds one `ConstEval` query family; it does not use
an ad hoc cache directory or source-derived file name.

Diagnostics point to the source Const block and retain a bounded evaluator
stack. They distinguish type/value admission, runtime capture, forbidden
operation, residual phase escape, fuel/size limits, cycles, and evaluator
faults.

## Implementation order

Const implementation follows the Checked Try/carrier boundary and unary Need
cuts. The atomic Const transaction is:

1. syntax/HIR `ComputationBlock(Const)` and source ownership;
2. checked phase/capture/callable/admission facts;
3. typed constant interning and exact-type `LoadConst` lowering;
4. ConstEval verifier profile, VM budgets, and diagnostics;
5. incremental artifact integration and complete negative/parity tests.

## See also

- [Await, unary Need, carrier blocks, and `try`](await-need-result.md)
- [Block scopes](block-scopes.md)
- [Executable Runtime Core](../02-runtime/executable-runtime-core.md)
