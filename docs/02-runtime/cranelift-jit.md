# Cranelift JIT

Cranelift JIT は `arcweft-lang-jit-cranelift` に置く native-only の最適化 backend として導入する。VM が正規実行系であり、JIT は pure / deterministic な関数に限定する。
The native Cranelift adapter builds functions with Cranelift's `speed`
optimization level; VM conformance remains mandatory for every compiled helper.

`arcweft-core` は Cranelift に依存せず、`jit-cranelift` feature も持たない。product feature 名は `native-jit` とし、native player が `arcweft-lang-jit-cranelift` adapter を選択する。

## 対象

JIT対象:

- easing 関数
- 数値計算
- animation sampling
- layout 式
- filter/map pipeline fusion
- pure helper function
- shader param precompute
- audio envelope / automation curve

JIT対象外:

- flow controlそのもの
- dialogue line execution
- choice / select
- `Need` / `await` / cancellation
- effect発行
- asset/audio/shader load
- wasm call
- UI操作
- plugin call
- save / load
- string-heavy処理
- debug build中の複雑な関数

## Backend

```rust
pub enum ExecBackend {
    Vm(BytecodeVm),
    #[cfg(feature = "native-jit")]
    Cranelift(arcweft_lang_jit_cranelift::CraneliftBackend),
}
```

The VM must remain available as fallback whenever JIT compilation is pending, rejected, or failed.

`arcweft-core` exposes the stable pure-helper execution boundary without taking
a Cranelift dependency:

- `PureFunctionRequest` carries the helper name, runtime expression, and typed
  bindings.
- `PureFunctionBackend` evaluates the request and returns deterministic
  `PureFunctionStats`.
- `VmPureFunctionBackend` is the semantic reference implementation.
- AOT/JIT candidates are checked through the same backend trait and compared
  against the VM result before they are allowed to replace VM execution.

The flow-level AOT executor boundary is intentionally VM-equivalent while
generated dispatch is still future work. Pure helper AOT is already executable:
`AotPureFunctionBackend` compiles the deterministic `i64` subset to a typed
plan, rejects unsupported helpers, and is checked against `VmPureFunctionBackend`
before use.

`arcweft-runtime-accelerator` is the runtime adapter crate that owns pure helper
compile caches and backend selection. `arcweft-core` exposes only
`RuntimePureCallBackend`, so normal flow execution can call pure helpers through
VM, typed AOT, or Cranelift JIT without depending on native code generation.
The CLI default is `--pure-backend auto`: supported deterministic integer pure
helpers are compiled once and then called from ordinary flow execution through
a fixed-size `i64` argument pack. Auto mode tries JIT, then typed AOT, then VM.
Pinned `--pure-backend jit|aot|vm` runs only that selected native/helper tier
before falling back to the VM for unsupported helpers. `--pure-workers auto|N`
and `--pure-batch-min-len N` configure the accelerator-owned thread pool used
for batchable typed AOT helper calls. Scalar flow calls stay on the direct
fixed-argument path so pure helper use does not introduce a Vec allocation.
The VM fallback path uses the same `RuntimeI64Args` pack for deterministic
integer helpers, including batch calls, so fallback correctness checks do not
add argument Vec allocation noise.

`arcweft-lang-jit-cranelift` is the native codegen adapter crate. Its first
executable subset compiles deterministic `i64` pure-helper expressions to
Cranelift, executes the generated code, and compares the result against
`VmPureFunctionBackend`. The current subset covers integer literals, integer
bindings, `+`, `-`, `*`, `/`, unary `-`, integer comparisons, value-producing
`if`, and the registered pure `add(lhs, rhs)` helper. It also supports lexical
`let` bindings lowered as structured pure runtime expressions. Helpers can be
compiled either as no-argument native calls with captured integer bindings or
as native calls with up to four selected local bindings passed as runtime `i64`
inputs.
String-heavy helpers, effectful calls, flow control, and pattern control remain
outside the JIT subset.

```bash
arcw jit check --json --iterations 1000 --warmup 10 --samples 5 --input-seed 0
arcw jit check --case branch-mix --json --julia --iterations 1000 --warmup 10 --samples 5 --input-seed 11
arcw jit check --case let-chain --json --iterations 1000 --warmup 10 --samples 5 --input-seed 17
arcw jit check --case four-input-mix --json --iterations 1000 --warmup 10 --samples 5 --input-seed 13
arcw jit check --case accumulation-mix --json --iterations 1000 --warmup 10 --samples 5 --input-seed 19
arcw jit check game/scripts/math.arcw --helper score --json --input-seed 7
arcw run game/routes/opening.arcw --mode drain --pure-backend auto --json
arcw run game/routes/opening.arcw --mode drain --pure-backend jit --json
```

The command reports workload metadata, VM/AOT/JIT conformance, AOT and JIT
compile time, repeated AOT/native-call time, JIT-compiled batch-loop time, VM
evaluation time, per-iteration medians, speedup ratios, deterministic
accumulators, input binding names, and pure-helper lowering counters. Its timing
loop feeds deterministic varying integer inputs through the typed AOT plan,
compiled JIT function, JIT batch loop, and VM reference. Builtin cases exercise a
score helper, branch-heavy arithmetic, lexical `let` chains, a four-input
arithmetic/branch mix, and a denser accumulation-style arithmetic mix so the
native-call boundary and the compiled loop can be compared independently.
The JIT batch loop carries deterministic input values through loop parameters
and advances them with bounded wraparound instead of recomputing modulo for each
input on every iteration.
When `--julia` is present, the report compares the same accumulator with Julia
and includes both scalar JIT/Julia and JIT-batch/Julia speed ratios.
`--input-seed` makes the input series reproducible while allowing local A/B
comparisons. It does not read source paths or persist host absolute paths.

Flow execution and bench JSON expose the same accelerator boundary. Per-step
pure stats include scalar calls, batch calls, batch item counts, backend call
counts, stack-packed argument calls, copied argument/result byte counts,
thread-pool jobs, Vec argument allocations, and VM fallback counts. Executor
stats include the selected pure backend, worker policy, resolved worker count,
batch threshold, helper acceleration summary, compile attempts, cache
hits/misses, and compile elapsed time. These counters are meant to show whether
natural flow code is staying on the zero-allocation scalar path or crossing into
batch/thread-pool execution.
`arcw bench --json` pure-helper sections also include a `runtime_batch` summary
that exercises `RuntimePureAccelerator::call_i64_batch` under the selected
worker and batch-threshold settings, so AOT/JIT helper timing can be compared
with the runtime batch/thread-pool path in one path-free report.

When a `.arcw` path is provided, `arcw jit check` runs the normal parse, HIR
lowering, reference validation, typecheck-readiness, and typecheck path first.
It then selects a `#[pure] fn` helper, lowers the expression body or a simple
local-`let` statement body with either a final value expression or tail
`return` to the runtime pure-helper request, and uses the VM as the conformance
reference before timing the Cranelift function. The current source-backed check
supports the same 0-to-4-input integer subset as the native adapter.

## IR lowering

```text
Typed IR function
  → Purity/effect check
  → Type layout check
  → Cranelift Signature
  → CLIF generation
  → compile
  → function pointer cache
```

## 同値性検査

JIT対象関数は、dev/test profile で VM と比較する。

The comparison must use deterministic input values and must not write host
absolute paths into snapshots, profile JSON, benchmark JSON, or CLIF/assembly
dump metadata.

```bash
arcw jit check --json
arcw jit dump-clif fn.logic.affection_score
arcw jit dump-asm fn.logic.affection_score
```

```arcw
property @test.jit_vm_equivalence_affection_score {
    for_all input in gen<AffectionInput>() {
        let vm = eval_vm(@fn.affection_score, input)
        let jit = eval_jit(@fn.affection_score, input)
        assert_eq vm, jit
    }
}
```

## JIT と lazy

関数は初回使用時に JIT できる。

```arcw
lazy jit fn @fn.layout_choices
```

flow 内で必要なら `Need` として扱う。ただし通常は VM fallback を使い、JIT 完了後に差し替える。

```text
JIT pending:
  VMで実行
JIT ready:
  frame boundaryでJITへ切替
JIT failed:
  VM継続 + diagnostic
```

## 契約

JIT は契約済み pure subset のみ。

```arcw
fn score(choice: ChoiceDef)(state: GameState) -> i32
requires choice.is_valid()
ensures result >= 0
pure
jit
{
    ...
}
```

## safety

- JIT code は engine internal only。
- user/mod script では VM が正。
- native product では feature flag で有効化。
- web では runtime JIT 無効。AOT compiled Wasm player + bytecode VM を使用し、必要なら将来の build-time AOT Wasm helper として扱う。
- Wasmtime は plugin/activity sandbox 用であり、JIT backend ではない。

