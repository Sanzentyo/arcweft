# Cranelift JIT

Cranelift JIT は `arcweft-lang-jit-cranelift` に置く native-only の最適化 backend として導入する。VM が正規実行系であり、JIT は pure / deterministic な関数に限定する。

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

```bash
arcw jit check --compare-vm
arcw jit dump-clif fn.logic.affection_score
arcw jit dump-asm fn.logic.affection_score
```

```awft
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

```awft
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

```awft
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
