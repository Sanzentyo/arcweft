はい。`arcweft` 側の `f32` / `f64` 仕様がまだ未確定なら、**今ここで一番実装・性能・将来拡張に都合がよい仕様へ固定する**のがよいです。

現状確認として、プロジェクト方針上 arcweft 関連では最新の理念・構造把握が前提です。
現在の workspace は `arcweft-core` / `arcweft-lang-syntax` / `arcweft-lang-hir` / `arcweft-lang-sema` / `arcweft-runtime-plan` / `arcweft-runtime-accelerator` / `arcweft-lang-jit-cranelift` などに分かれており、Rust 2024、`unsafe_code = "forbid"` です。
現状の runtime value は `RuntimeValue::Float(String)` として float を raw text 保持しており、`RuntimeSeq::{Values, Dense}`、`Tuple(Vec<RuntimeValue>)`、`Record(Vec<RuntimeFieldValue>)` もあります。
`DenseSeqStorage<T>` はすでに `Vec<T>` backing なので、ここを `Arc<[T]>` や bit wrapper に寄せるより、`Vec<f32>` / `Vec<f64>` を直接使う形が現在の性能設計に合います。
また performance snapshot でも、dense scalar sequence は `DenseSeqStorage<T>` と borrowed view を前提にしており、tuple / record は scalar dense ではなく、必要なら別の columnar / struct-array optimization にする方針が書かれています。

以下、書き直し版です。

---

# Native Float DenseSeq と Columnar Tuple/Record 完了設計

## 0. この文書の結論

この設計では、`arcweft` の `f32` / `f64` 仕様を次のように固定する。

```text
Arcweft の f32/f64 は Rust native f32/f64 をそのまま使う。
```

したがって、最終形では次にする。

```rust
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    I128(i128),
    UInt(u64),
    U128(u128),
    ISize(i64),
    USize(u64),

    F32(f32),
    F64(f64),

    String(String),
    Char(char),
    Duration(LogicalDuration),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    Seq(RuntimeSeq),
    Record(Vec<RuntimeFieldValue>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
}
```

```rust
pub enum DenseSeq {
    Units(usize),

    I8(DenseSeqStorage<i8>),
    I16(DenseSeqStorage<i16>),
    I32(DenseSeqStorage<i32>),
    I64(DenseSeqStorage<i64>),
    I128(DenseSeqStorage<i128>),
    ISize(DenseSeqStorage<i64>),

    U8(DenseSeqStorage<u8>),
    U16(DenseSeqStorage<u16>),
    U32(DenseSeqStorage<u32>),
    U64(DenseSeqStorage<u64>),
    U128(DenseSeqStorage<u128>),
    USize(DenseSeqStorage<u64>),

    Bool(DenseSeqStorage<bool>),
    Bytes(DenseSeqStorage<u8>),
    Chars(DenseSeqStorage<char>),
    Durations(DenseSeqStorage<LogicalDuration>),
    Strings(DenseSeqStorage<String>),

    F32(DenseSeqStorage<f32>),
    F64(DenseSeqStorage<f64>),

    EntityRefs(DenseSeqStorage<String>),
}
```

削除する。

```rust
RuntimeValue::Float(String)
DenseSeq::FloatLiterals(DenseSeqStorage<String>)
DenseSeqKind::FloatLiterals
RuntimeSeq::dense_float_literals
DenseSeq::float_literals
DenseSeq::as_float_literals
runtime_sequence_dense_float_literals
```

採用しない。

```rust
RuntimeF32(f32)
RuntimeF32(u32)
RuntimeF64(f64)
RuntimeF64(u64)
DenseF32Seq
DenseF64Seq
Arc<[T]> backing store
```

理由は単純で、`f32` / `f64` が未確定なら、今から **Rust native float と同じ方向に仕様を寄せる** のが最も都合がよいからである。

---

# 1. Float semantics

## 1.1 Arcweft float は Rust-like native float とする

Arcweft の `f32` / `f64` は次の仕様にする。

```text
storage:
  f32 / f64 を直接保持する。

arithmetic:
  Rust の f32 / f64 operation を使う。

equality:
  Rust の PartialEq と同じ。

ordering:
  Rust の PartialOrd と同じ。

NaN:
  NaN != NaN。

+0.0 / -0.0:
  +0.0 == -0.0。

snapshot:
  表示・保存時は必要に応じて to_bits() を使える。

hash/map key:
  float は通常 map key にしない。
  必要なら明示的に to_bits() して integer key にする。
```

つまり、前回まで考えていた次の仕様は捨てる。

```text
+0.0 と -0.0 を bit identity で別値扱いする。
NaN を Eq にする。
RuntimeValue 全体を Eq に保つ。
```

これは `f32` / `f64` を直接使う最終形と相性が悪い。

`f32` / `f64` を直接使うなら、Arcweft 側の仕様も Rust-like にしてしまうのが一番よい。

---

## 1.2 `RuntimeValue` は `Eq` を持たない

`f32` / `f64` は `Eq` ではない。
したがって、`RuntimeValue` も `Eq` を持たない。

現在の `RuntimeValue` は `Eq, PartialEq` を derive しているが、float 導入後は次にする。

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    // ...
    F32(f32),
    F64(f64),
    // ...
}
```

同様に、`RuntimeValue` を内部に含む型からも `Eq` を外す。

対象。

```rust
RuntimeBinding
RuntimePayload
RuntimeSeq
DenseSeq
DenseSeqStorage<T>
RuntimeFieldValue
RuntimeExpr
RuntimeExprMatchArm
FlowOp
RuntimeFlow
RuntimePlan
```

ただし、識別子や純粋な metadata で float value を含まない型は `Eq` のままでよい。

```rust
FlowRuntimeId
EntryRuntimeId
RuntimePureHelperId
DenseSeqKind
RuntimeBinaryOp
RuntimeUnaryOp
```

この変更は破壊的だが、最終形として正しい。

`RuntimeValue` は一般に `PartialEq` の値領域であり、`NaN` を含むと非反射的になる。

```rust
let x = RuntimeValue::F32(f32::NAN);

assert!(x != x);
```

これは Rust-like float semantics として許容する。

---

# 2. なぜ wrapper を使わないか

## 2.1 wrapper は不要

次は採用しない。

```rust
pub struct RuntimeF32(f32);
pub struct RuntimeF64(f64);
```

また、次も採用しない。

```rust
pub struct RuntimeF32(u32);
pub struct RuntimeF64(u64);
```

理由。

```text
1. Arcweft float 仕様を Rust-like に固定すれば wrapper は不要。
2. RuntimeValue の Eq 維持を目的に wrapper を入れる必要がなくなる。
3. DenseSeq hot path で &[f32] / &[f64] を直接返せる。
4. JIT / AOT / batch / std math 関数が native float をそのまま使える。
5. workspace は unsafe_code = "forbid" なので wrapper slice から &[f32] への cast を避けたい。
```

`RuntimeValue::F32(f32)` と `DenseSeq::F32(DenseSeqStorage<f32>)` をそのまま採用する。

---

## 2.2 exact bit comparison は別 API にする

言語の `==` は Rust-like equality にする。
bit exact comparison は別関数にする。

```rust
pub fn f32_bits_eq(lhs: f32, rhs: f32) -> bool {
    lhs.to_bits() == rhs.to_bits()
}

pub fn f64_bits_eq(lhs: f64, rhs: f64) -> bool {
    lhs.to_bits() == rhs.to_bits()
}
```

標準 library 側には次を用意する。

```text
std.f32.to_bits(x) -> u32
std.f32.from_bits(bits: u32) -> f32

std.f64.to_bits(x) -> u64
std.f64.from_bits(bits: u64) -> f64
```

これにより、bit identity が必要なユーザーは明示的に書ける。

```text
std.f32.to_bits(x) == std.f32.to_bits(y)
```

通常の `x == y` は Rust-like equality。

```text
0.0f32 == -0.0f32
```

は true。

```text
std.f32.nan == std.f32.nan
```

は false。

---

# 3. NaN / Infinity の扱い

## 3.1 NaN / Infinity は literal にしない

次は float literal にしない。

```text
NaN
nan
Inf
Infinity
+inf
-inf
```

これらは path / identifier として扱う。

標準定数として提供する。

```text
std.f32.nan
std.f32.infinity
std.f32.neg_infinity
std.f32.epsilon
std.f32.min
std.f32.max
std.f32.pi
std.f32.tau

std.f64.nan
std.f64.infinity
std.f64.neg_infinity
std.f64.epsilon
std.f64.min
std.f64.max
std.f64.pi
std.f64.tau
```

runtime-plan lowering では次のように落とす。

```rust
fn lower_std_float_constant(path: &str) -> Option<RuntimeValue> {
    Some(match path {
        "std.f32.nan" => RuntimeValue::F32(f32::NAN),
        "std.f32.infinity" => RuntimeValue::F32(f32::INFINITY),
        "std.f32.neg_infinity" => RuntimeValue::F32(f32::NEG_INFINITY),
        "std.f32.epsilon" => RuntimeValue::F32(f32::EPSILON),
        "std.f32.min" => RuntimeValue::F32(f32::MIN),
        "std.f32.max" => RuntimeValue::F32(f32::MAX),
        "std.f32.pi" => RuntimeValue::F32(std::f32::consts::PI),
        "std.f32.tau" => RuntimeValue::F32(std::f32::consts::TAU),

        "std.f64.nan" => RuntimeValue::F64(f64::NAN),
        "std.f64.infinity" => RuntimeValue::F64(f64::INFINITY),
        "std.f64.neg_infinity" => RuntimeValue::F64(f64::NEG_INFINITY),
        "std.f64.epsilon" => RuntimeValue::F64(f64::EPSILON),
        "std.f64.min" => RuntimeValue::F64(f64::MIN),
        "std.f64.max" => RuntimeValue::F64(f64::MAX),
        "std.f64.pi" => RuntimeValue::F64(std::f64::consts::PI),
        "std.f64.tau" => RuntimeValue::F64(std::f64::consts::TAU),

        _ => return None,
    })
}
```

---

## 3.2 NaN canonicalization はしない

前回案では NaN payload canonicalization を考えていたが、この最終案ではしない。

理由。

```text
1. f32/f64 を直接使う仕様にしたため。
2. Rust-like semantics では NaN != NaN なので payload equality を通常 equality に使わない。
3. operation ごとの NaN payload 正規化は hot path に余計な処理を入れる。
4. exact bit が必要なら std.f32.to_bits / std.f64.to_bits を使えばよい。
```

つまり、次は不要。

```rust
if value.is_nan() {
    value = f32::from_bits(0x7fc0_0000);
}
```

ただし、snapshot や debug 表示では `to_bits()` を出せるようにする。

---

# 4. FMA 方針

## 4.1 `a * b + c` を暗黙に `mul_add` にしない

これは引き続き禁止する。

理由は、`f32` / `f64` を直接使う仕様にしても変わらない。

通常の式。

```text
a * b + c
```

は次の 2 回丸めである。

```text
tmp = round(a * b)
out = round(tmp + c)
```

一方、FMA は次の 1 回丸めである。

```text
out = round((a * b) + c)
```

したがって、結果 bit が変わり得る。

Arcweft は Rust-like float を採用するので、source に `a * b + c` と書かれているなら、Rust の `a * b + c` と同じ意味にする。
optimizer / JIT / AOT が勝手に FMA に置換してはいけない。

---

## 4.2 明示的な `mul_add` は提供する

FMA 自体は提供する。

```text
std.f32.mul_add(a, b, c)
std.f64.mul_add(a, b, c)
```

これは Rust の `f32::mul_add` / `f64::mul_add` 相当の操作として扱う。

```rust
fn eval_f32_mul_add(a: f32, b: f32, c: f32) -> f32 {
    a.mul_add(b, c)
}

fn eval_f64_mul_add(a: f64, b: f64, c: f64) -> f64 {
    a.mul_add(b, c)
}
```

禁止するのは FMA そのものではない。
禁止するのは、通常式の暗黙 rewrite である。

---

# 5. std float module

## 5.1 Rust std 相当関数は入れる

次のように入れる。

```text
std.f32.abs(x)
std.f32.floor(x)
std.f32.ceil(x)
std.f32.round(x)
std.f32.trunc(x)
std.f32.fract(x)
std.f32.sqrt(x)
std.f32.sin(x)
std.f32.cos(x)
std.f32.tan(x)
std.f32.exp(x)
std.f32.exp2(x)
std.f32.ln(x)
std.f32.log2(x)
std.f32.log10(x)
std.f32.powf(x, y)
std.f32.atan2(y, x)
std.f32.mul_add(a, b, c)

std.f64.abs(x)
std.f64.floor(x)
std.f64.ceil(x)
std.f64.round(x)
std.f64.trunc(x)
std.f64.fract(x)
std.f64.sqrt(x)
std.f64.sin(x)
std.f64.cos(x)
std.f64.tan(x)
std.f64.exp(x)
std.f64.exp2(x)
std.f64.ln(x)
std.f64.log2(x)
std.f64.log10(x)
std.f64.powf(x, y)
std.f64.atan2(y, x)
std.f64.mul_add(a, b, c)
```

predicate。

```text
std.f32.is_nan(x)
std.f32.is_infinite(x)
std.f32.is_finite(x)
std.f32.is_sign_positive(x)
std.f32.is_sign_negative(x)

std.f64.is_nan(x)
std.f64.is_infinite(x)
std.f64.is_finite(x)
std.f64.is_sign_positive(x)
std.f64.is_sign_negative(x)
```

bit conversion。

```text
std.f32.to_bits(x)
std.f32.from_bits(bits)

std.f64.to_bits(x)
std.f64.from_bits(bits)
```

cast。

```text
std.f32.to_f64(x)
std.f64.to_f32(x)
```

---

## 5.2 JIT / AOT は subset support でよい

VM reference backend は全関数を実装する。

JIT / AOT は対応できるものだけ下げる。

最低限 JIT/AOT に入れるもの。

```text
f32/f64 input
f32/f64 output
+ - * /
unary -
== != < <= > >=
std.f32.to_bits
std.f64.to_bits
```

JIT/AOT が最初から対応しなくてよいもの。

```text
sin
cos
tan
exp
ln
powf
atan2
mul_add
```

これらは unsupported として VM fallback してよい。
ただし VM semantics は必ず持つ。

---

# 6. RuntimeValue / DenseSeq 実装

## 6.1 `RuntimeValue`

変更前。

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeValue {
    // ...
    Float(String),
    // ...
}
```

変更後。

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    I128(i128),
    UInt(u64),
    U128(u128),
    ISize(i64),
    USize(u64),
    F32(f32),
    F64(f64),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    Seq(RuntimeSeq),
    Record(Vec<RuntimeFieldValue>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
}
```

`Eq` は外す。

---

## 6.2 `RuntimeSeq`

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeSeq {
    Values(Vec<RuntimeValue>),
    Dense(DenseSeq),
    TupleColumns(TupleSeq),
    RecordColumns(RecordSeq),
}
```

`TupleColumns` / `RecordColumns` は後述する。

---

## 6.3 `DenseSeqKind`

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenseSeqKind {
    Units,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    Bool,
    Bytes,
    Chars,
    Durations,
    Strings,
    F32,
    F64,
    EntityRefs,
}
```

削除。

```rust
FloatLiterals
```

---

## 6.4 `DenseSeq`

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum DenseSeq {
    Units(usize),

    I8(DenseSeqStorage<i8>),
    I16(DenseSeqStorage<i16>),
    I32(DenseSeqStorage<i32>),
    I64(DenseSeqStorage<i64>),
    I128(DenseSeqStorage<i128>),
    ISize(DenseSeqStorage<i64>),

    U8(DenseSeqStorage<u8>),
    U16(DenseSeqStorage<u16>),
    U32(DenseSeqStorage<u32>),
    U64(DenseSeqStorage<u64>),
    U128(DenseSeqStorage<u128>),
    USize(DenseSeqStorage<u64>),

    Bool(DenseSeqStorage<bool>),
    Bytes(DenseSeqStorage<u8>),
    Chars(DenseSeqStorage<char>),
    Durations(DenseSeqStorage<LogicalDuration>),
    Strings(DenseSeqStorage<String>),

    F32(DenseSeqStorage<f32>),
    F64(DenseSeqStorage<f64>),

    EntityRefs(DenseSeqStorage<String>),
}
```

`DenseSeqStorage<T>` は今の形を維持する。

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct DenseSeqStorage<T> {
    values: Vec<T>,
}

impl<T> DenseSeqStorage<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self { values }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn into_vec(self) -> Vec<T> {
        self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
```

---

## 6.5 constructor

```rust
impl RuntimeSeq {
    pub fn dense_f32(values: Vec<f32>) -> Self {
        Self::Dense(DenseSeq::f32(values))
    }

    pub fn dense_f64(values: Vec<f64>) -> Self {
        Self::Dense(DenseSeq::f64(values))
    }
}
```

```rust
impl DenseSeq {
    pub fn f32(values: Vec<f32>) -> Self {
        Self::F32(DenseSeqStorage::new(values))
    }

    pub fn f64(values: Vec<f64>) -> Self {
        Self::F64(DenseSeqStorage::new(values))
    }
}
```

---

## 6.6 borrowed slice accessor

```rust
impl RuntimeSeq {
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::Dense(values) => values.as_f32_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[f64]> {
        match self {
            Self::Dense(values) => values.as_f64_slice(),
            Self::Values(_) | Self::TupleColumns(_) | Self::RecordColumns(_) => None,
        }
    }
}
```

```rust
impl DenseSeq {
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::F32(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_f64_slice(&self) -> Option<&[f64]> {
        match self {
            Self::F64(values) => Some(values.as_slice()),
            _ => None,
        }
    }
}
```

これにより、JIT / AOT / batch / std math の hot path で `&[f32]` / `&[f64]` を直接渡せる。

---

# 7. Runtime evaluation

## 7.1 unary

```rust
pub(crate) fn evaluate_unary(
    op: RuntimeUnaryOp,
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimeValue::Bool(value)) => {
            Ok(RuntimeValue::Bool(!value))
        }

        (RuntimeUnaryOp::Neg, RuntimeValue::Int(value)) => {
            Ok(RuntimeValue::Int(-value))
        }

        (RuntimeUnaryOp::Neg, RuntimeValue::F32(value)) => {
            Ok(RuntimeValue::F32(-value))
        }

        (RuntimeUnaryOp::Neg, RuntimeValue::F64(value)) => {
            Ok(RuntimeValue::F64(-value))
        }

        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: runtime_unary_op_label(op),
            value: runtime_value_label(&value),
        }),
    }
}
```

---

## 7.2 binary arithmetic

```rust
RuntimeBinaryOp::Add
| RuntimeBinaryOp::Sub
| RuntimeBinaryOp::Mul
| RuntimeBinaryOp::Div => match (lhs, rhs) {
    (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => {
        Ok(RuntimeValue::Int(evaluate_numeric_op(lhs, op, rhs)))
    }

    (RuntimeValue::I128(lhs), RuntimeValue::I128(rhs)) => {
        Ok(RuntimeValue::I128(evaluate_numeric_op(lhs, op, rhs)))
    }

    (RuntimeValue::ISize(lhs), RuntimeValue::ISize(rhs)) => {
        Ok(RuntimeValue::ISize(evaluate_numeric_op(lhs, op, rhs)))
    }

    (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs)) => {
        Ok(RuntimeValue::UInt(evaluate_numeric_op(lhs, op, rhs)))
    }

    (RuntimeValue::U128(lhs), RuntimeValue::U128(rhs)) => {
        Ok(RuntimeValue::U128(evaluate_numeric_op(lhs, op, rhs)))
    }

    (RuntimeValue::USize(lhs), RuntimeValue::USize(rhs)) => {
        Ok(RuntimeValue::USize(evaluate_numeric_op(lhs, op, rhs)))
    }

    (RuntimeValue::F32(lhs), RuntimeValue::F32(rhs)) => {
        Ok(RuntimeValue::F32(evaluate_f32_op(lhs, op, rhs)))
    }

    (RuntimeValue::F64(lhs), RuntimeValue::F64(rhs)) => {
        Ok(RuntimeValue::F64(evaluate_f64_op(lhs, op, rhs)))
    }

    (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
}
```

```rust
fn evaluate_f32_op(lhs: f32, op: RuntimeBinaryOp, rhs: f32) -> f32 {
    match op {
        RuntimeBinaryOp::Add => lhs + rhs,
        RuntimeBinaryOp::Sub => lhs - rhs,
        RuntimeBinaryOp::Mul => lhs * rhs,
        RuntimeBinaryOp::Div => lhs / rhs,
        _ => unreachable!(),
    }
}

fn evaluate_f64_op(lhs: f64, op: RuntimeBinaryOp, rhs: f64) -> f64 {
    match op {
        RuntimeBinaryOp::Add => lhs + rhs,
        RuntimeBinaryOp::Sub => lhs - rhs,
        RuntimeBinaryOp::Mul => lhs * rhs,
        RuntimeBinaryOp::Div => lhs / rhs,
        _ => unreachable!(),
    }
}
```

`f32 + f64` は implicit conversion しない。

```text
1.0f32 + 1.0f64
```

は type error。

---

## 7.3 equality

`RuntimeValue` の derived `PartialEq` をそのまま使う。

```rust
RuntimeBinaryOp::Eq => Ok(RuntimeValue::Bool(lhs == rhs)),
RuntimeBinaryOp::Ne => Ok(RuntimeValue::Bool(lhs != rhs)),
```

結果。

```text
0.0f32 == -0.0f32
=> true

std.f32.nan == std.f32.nan
=> false
```

これは Rust-like float equality として仕様化する。

---

## 7.4 ordering

```rust
RuntimeBinaryOp::Lt
| RuntimeBinaryOp::Le
| RuntimeBinaryOp::Gt
| RuntimeBinaryOp::Ge => match (lhs, rhs) {
    (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs))
    | (RuntimeValue::ISize(lhs), RuntimeValue::ISize(rhs)) => {
        Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
    }

    (RuntimeValue::I128(lhs), RuntimeValue::I128(rhs)) => {
        Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
    }

    (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs))
    | (RuntimeValue::USize(lhs), RuntimeValue::USize(rhs)) => {
        Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
    }

    (RuntimeValue::U128(lhs), RuntimeValue::U128(rhs)) => {
        Ok(RuntimeValue::Bool(compare_ordered(&lhs, op, &rhs)))
    }

    (RuntimeValue::F32(lhs), RuntimeValue::F32(rhs)) => {
        Ok(RuntimeValue::Bool(compare_f32(lhs, op, rhs)))
    }

    (RuntimeValue::F64(lhs), RuntimeValue::F64(rhs)) => {
        Ok(RuntimeValue::Bool(compare_f64(lhs, op, rhs)))
    }

    (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
}
```

```rust
fn compare_f32(lhs: f32, op: RuntimeBinaryOp, rhs: f32) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn compare_f64(lhs: f64, op: RuntimeBinaryOp, rhs: f64) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}
```

NaN が絡む comparison は Rust と同じく false になる。

---

# 8. Float literal lowering

## 8.1 syntax literal

現状の syntax は `Literal::Float { raw: String, suffix: Option<String> }` を持つ。
これを `FloatSuffix` enum にする。

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatSuffix {
    F32,
    F64,
}
```

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    String(String),
    Char {
        raw: String,
        value: char,
    },
    Int {
        raw: String,
        value: i64,
        suffix: Option<IntSuffix>,
    },
    Float {
        raw: String,
        suffix: Option<FloatSuffix>,
    },
    UnitNumber {
        raw: String,
        suffix: UnitNumberSuffix,
    },
    Bool(bool),
    Duration {
        amount: String,
        unit: DurationUnit,
    },
}
```

`pt` / `rad` は `Float` から分離する。

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitNumberSuffix {
    Pt,
    Rad,
}
```

理由。

```text
1. f32/f64 と unit-number は runtime value として別概念。
2. pt/rad を DenseSeq::F32/F64 に混ぜてはいけない。
3. semantic layer で unit conversion しやすくなる。
```

---

## 8.2 float literal grammar

認める。

```text
1.0
1.0f32
1.0f64
1e3
1e3f32
1.25e-3
1_000.25
```

認めない。

```text
NaN
Inf
Infinity
```

`.5` はこの TODO では入れない。
現行 parser は `.` を range / field / relative path に使っているため、`0.5` を必須にする方が安全。

---

## 8.3 width rule

```text
1. suffix f32 がある => f32
2. suffix f64 がある => f64
3. expected type が f32 => f32
4. expected type が f64 => f64
5. sequence 内で explicit f32 があり、他が unsuffixed => f32
6. sequence 内で explicit f64 があり、他が unsuffixed => f64
7. explicit f32 と explicit f64 が同じ inference group に混在 => type error
8. expected type も suffix もない => f64
```

現状では unsuffixed float は expected type がないと error だが、これをやめる。

```text
1.0 => f64
```

にする。

---

## 8.4 parse

native parser を使う。

```rust
pub enum FloatLiteralParseError {
    InvalidSeparator,
    InvalidSuffix,
    InvalidFloat,
    Overflow,
}
```

```rust
pub fn parse_f32_literal(raw: &str) -> Result<f32, FloatLiteralParseError> {
    let body = strip_float_suffix(raw, FloatSuffix::F32)?;
    let normalized = normalize_float_literal_body(body)?;

    let value = normalized
        .parse::<f32>()
        .map_err(|_| FloatLiteralParseError::InvalidFloat)?;

    if value.is_infinite() && !literal_spells_infinity(raw) {
        return Err(FloatLiteralParseError::Overflow);
    }

    Ok(value)
}
```

```rust
pub fn parse_f64_literal(raw: &str) -> Result<f64, FloatLiteralParseError> {
    let body = strip_float_suffix(raw, FloatSuffix::F64)?;
    let normalized = normalize_float_literal_body(body)?;

    let value = normalized
        .parse::<f64>()
        .map_err(|_| FloatLiteralParseError::InvalidFloat)?;

    if value.is_infinite() && !literal_spells_infinity(raw) {
        return Err(FloatLiteralParseError::Overflow);
    }

    Ok(value)
}
```

ただし、literal spelling で infinity は認めないので、通常は overflow error になる。

```text
1e10000f32
```

は `f32::INFINITY` ではなく compile error。

Infinity が欲しいなら、明示的にこれを使う。

```text
std.f32.infinity
```

---

# 9. runtime-plan lowering の穴を閉じる

## 9.1 現状の問題

現状の runtime-plan は次のように raw literal から runtime value を作っている。

```rust
Literal::Float { raw, .. } => RuntimeValue::Float(raw.clone())
```

この形では、次を正しく lower できない。

```text
let x: f32 = 1.0
```

syntax 上の literal は unsuffixed なので `raw = "1.0"` しかない。
しかし sema の expected type により、これは `f32` として lower されるべきである。

したがって、runtime-plan は raw literal だけを見てはいけない。
sema が決めた expression type を参照する必要がある。

---

## 9.2 checked expression type table を導入する

`arcweft-lang-sema` は checked result として expression type table を返す。

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct CheckedModule {
    pub hir: HirModule,
    pub expr_types: ExprTypeTable,
    pub diagnostics: Vec<TypeCheckError>,
}
```

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirExprId(pub usize);
```

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ExprTypeTable {
    entries: Vec<ExprTypeEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExprTypeEntry {
    pub expr: HirExprId,
    pub ty: TypeKind,
}
```

runtime-plan は `Literal::Float` を lower するとき、必ず resolved type を受け取る。

```rust
fn lower_float_literal(
    raw: &str,
    resolved_type: &TypeKind,
) -> Result<RuntimeValue, RuntimePlanError> {
    match resolved_type {
        TypeKind::F32 => parse_f32_literal(raw)
            .map(RuntimeValue::F32)
            .map_err(RuntimePlanError::FloatLiteral),

        TypeKind::F64 => parse_f64_literal(raw)
            .map(RuntimeValue::F64)
            .map_err(RuntimePlanError::FloatLiteral),

        other => Err(RuntimePlanError::FloatLiteralTypeMismatch {
            found: other.clone(),
        }),
    }
}
```

---

# 10. sequence folding

## 10.1 f32/f64 dense collection

```rust
pub fn runtime_sequence_from_literal_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    match values.first() {
        Some(RuntimeValue::Unit) if values.iter().all(|value| matches!(value, RuntimeValue::Unit)) => {
            runtime_sequence_dense_units(values.len())
        }

        Some(RuntimeValue::Bool(_)) => collect_dense_or_values(
            values,
            take_bool_value,
            RuntimeValue::Bool,
            runtime_sequence_dense_bool,
        ),

        Some(RuntimeValue::Int(_)) => collect_dense_or_values(
            values,
            take_int_value,
            RuntimeValue::Int,
            runtime_sequence_dense_i64,
        ),

        Some(RuntimeValue::F32(_)) => collect_dense_or_values(
            values,
            take_f32_value,
            RuntimeValue::F32,
            runtime_sequence_dense_f32,
        ),

        Some(RuntimeValue::F64(_)) => collect_dense_or_values(
            values,
            take_f64_value,
            RuntimeValue::F64,
            runtime_sequence_dense_f64,
        ),

        Some(RuntimeValue::Tuple(_)) => {
            collect_tuple_columns_or_values(values)
        }

        Some(RuntimeValue::Record(_)) => {
            collect_record_columns_or_values(values)
        }

        _ => runtime_sequence_values(values),
    }
}
```

```rust
fn take_f32_value(value: RuntimeValue) -> Result<f32, RuntimeValue> {
    match value {
        RuntimeValue::F32(value) => Ok(value),
        value => Err(value),
    }
}

fn take_f64_value(value: RuntimeValue) -> Result<f64, RuntimeValue> {
    match value {
        RuntimeValue::F64(value) => Ok(value),
        value => Err(value),
    }
}
```

```rust
pub fn runtime_sequence_dense_f32(values: Vec<f32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f32(values))
}

pub fn runtime_sequence_dense_f64(values: Vec<f64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_f64(values))
}
```

---

## 10.2 `FloatLiterals` は完全削除

削除対象。

```rust
fn take_float_value(value: RuntimeValue) -> Result<String, RuntimeValue>
pub fn runtime_sequence_dense_float_literals(values: Vec<String>) -> RuntimeValue
RuntimeSeq::dense_float_literals
DenseSeq::float_literals
DenseSeq::as_float_literals
```

float literal text は runtime storage に残さない。

---

# 11. Snapshot / debug / exact comparison

## 11.1 snapshot は bit form を使える

runtime equality は Rust-like `PartialEq` だが、snapshot では exact bit を残してよい。

```rust
fn snapshot_f32(value: f32) -> String {
    format!("f32:0x{:08x}", value.to_bits())
}

fn snapshot_f64(value: f64) -> String {
    format!("f64:0x{:016x}", value.to_bits())
}
```

例。

```text
RuntimeValue::F32(1.0)
=> f32:0x3f800000

RuntimeValue::F32(-0.0)
=> f32:0x80000000

RuntimeValue::F32(f32::NAN)
=> f32:0x7fc00000 など
```

NaN payload は canonicalize しないため、snapshot は実際の bit を出す。

---

## 11.2 exact tests は `to_bits` を使う

通常 equality。

```rust
assert_eq!(RuntimeValue::F32(0.0), RuntimeValue::F32(-0.0));
assert_ne!(RuntimeValue::F32(f32::NAN), RuntimeValue::F32(f32::NAN));
```

exact bit test。

```rust
assert_ne!(0.0f32.to_bits(), (-0.0f32).to_bits());
```

Arcweft standard library でも同じ。

```text
std.f32.to_bits(0.0f32) != std.f32.to_bits(-0.0f32)
```

---

# 12. Columnar tuple / record sequence

## 12.1 single tuple / record は RuntimeValue に残す

これは scalar value として残す。

```rust
RuntimeValue::Tuple(Vec<RuntimeValue>)
RuntimeValue::Record(Vec<RuntimeFieldValue>)
```

ただし、sequence of tuple / sequence of record は `RuntimeSeq::Values(Vec<RuntimeValue>)` ではなく columnar にする。

```rust
RuntimeSeq::TupleColumns(TupleSeq)
RuntimeSeq::RecordColumns(RecordSeq)
```

---

## 12.2 TupleSeq

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TupleSeq {
    len: usize,
    columns: Vec<RuntimeSeq>,
}

impl TupleSeq {
    pub fn new(len: usize, columns: Vec<RuntimeSeq>) -> Result<Self, RuntimeSeqError> {
        validate_column_lengths(len, columns.iter())?;
        Ok(Self { len, columns })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn columns(&self) -> &[RuntimeSeq] {
        &self.columns
    }

    pub fn column(&self, index: usize) -> Option<&RuntimeSeq> {
        self.columns.get(index)
    }
}
```

`len` は明示的に持つ。
`columns.first().len()` から推測しない。

理由。

```text
[(), (), ()]
```

これは arity 0 / len 3 の tuple sequence だからである。

---

## 12.3 RecordSeq

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RecordSeq {
    len: usize,
    fields: Vec<RecordSeqField>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordSeqField {
    name: String,
    values: RuntimeSeq,
}
```

```rust
impl RecordSeq {
    pub fn new(
        len: usize,
        mut fields: Vec<RecordSeqField>,
        order: RecordFieldOrder,
    ) -> Result<Self, RuntimeSeqError> {
        canonicalize_record_field_order(&mut fields, order)?;
        validate_record_columns(len, &fields)?;
        Ok(Self { len, fields })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn fields(&self) -> &[RecordSeqField] {
        &self.fields
    }

    pub fn field_by_ordinal(&self, ordinal: usize) -> Option<&RuntimeSeq> {
        self.fields.get(ordinal).map(|field| &field.values)
    }

    pub fn field_by_name(&self, name: &str) -> Option<&RuntimeSeq> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.values)
    }
}
```

record sequence storage に `HashMap` は使わない。
canonical `Vec` を使う。

field order rule。

```text
nominal record:
  type definition order

anonymous structural record:
  field name lexicographic order
```

runtime hot path では `field_by_name` を使わない。
runtime-plan lowering 時点で ordinal に解決する。

---

## 12.4 record projection

現在の runtime expr。

```rust
RuntimeExpr::Field {
    target: Box<RuntimeExpr>,
    field: String,
}
```

最終形。

```rust
RuntimeExpr::ProjectRecord {
    target: Box<RuntimeExpr>,
    ordinal: usize,
}
```

tuple。

```rust
RuntimeExpr::ProjectTuple {
    target: Box<RuntimeExpr>,
    index: usize,
}
```

source。

```text
player.score
```

lowering 後。

```rust
RuntimeExpr::ProjectRecord {
    target: Box::new(...),
    ordinal: 2,
}
```

runtime で string compare しない。

---

# 13. Arc / shared storage 方針

## 13.1 DenseSeqStorage は Vec のまま

変更しない。

```rust
pub struct DenseSeqStorage<T> {
    values: Vec<T>,
}
```

理由。

```text
1. 現行実装が Vec<T> backing。
2. Dense hot path は borrowed slice view 前提。
3. f32/f64 では &[f32] / &[f64] を直接返したい。
4. Arc<[T]> は ordinary sequence に atomic refcount cost を持ち込む。
5. language Arc と storage sharing を混同しない。
```

---

## 13.2 language Arc は別 value

言語上の `Arc<T>` / shared value が必要なら、runtime value に明示する。

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct SharedRuntimeValue(std::sync::Arc<RuntimeValue>);
```

```rust
pub enum RuntimeValue {
    // ...
    Shared(SharedRuntimeValue),
}
```

ordinary sequence storage を `Arc<[T]>` にする必要はない。

---

# 14. crate 別変更

## 14.1 `arcweft-core`

変更。

```text
RuntimeValue::Float(String) 削除
RuntimeValue::F32(f32) 追加
RuntimeValue::F64(f64) 追加

RuntimeSeq::TupleColumns 追加
RuntimeSeq::RecordColumns 追加

DenseSeq::FloatLiterals 削除
DenseSeq::F32(DenseSeqStorage<f32>) 追加
DenseSeq::F64(DenseSeqStorage<f64>) 追加

DenseSeqKind::FloatLiterals 削除
DenseSeqKind::F32 追加
DenseSeqKind::F64 追加
```

derive 変更。

```text
Eq を外す。
PartialEq は残す。
```

対象。

```text
RuntimeValue
RuntimeSeq
DenseSeq
DenseSeqStorage
RuntimePayload
RuntimeBinding
RuntimeFieldValue
RuntimeExpr
RuntimePlan 周辺の value/expression を含む型
```

---

## 14.2 `arcweft-lang-syntax`

変更。

```text
Literal::Float suffix を Option<String> から Option<FloatSuffix> にする。
pt/rad を Literal::UnitNumber に分離する。
1e3 / 1e-3 を float として lex する。
NaN / Inf / Infinity は literal にしない。
```

---

## 14.3 `arcweft-lang-sema`

変更。

```text
TypeKind::F32 / TypeKind::F64 は維持。
unsuffixed float default を f64 にする。
expected f32/f64 の位置では unsuffixed float を expected type に寄せる。
sequence 内 explicit f32/f64 混在を error にする。
std.f32.* / std.f64.* constants を typecheck する。
std.f32.* / std.f64.* functions を typecheck する。
checked expression type table を出す。
```

---

## 14.4 `arcweft-runtime-plan`

変更。

```text
Literal::Float { raw, .. } から RuntimeValue::Float(raw.clone()) を作らない。
checked expression type table を見て RuntimeValue::F32 / RuntimeValue::F64 を作る。
std.f32.* / std.f64.* constants を RuntimeValue に lower する。
std.f32.* / std.f64.* calls を RuntimeExpr の float builtin に lower する。
Field { field: String } を ProjectRecord { ordinal } に寄せる。
```

---

## 14.5 `arcweft-runtime-accelerator`

追加。

```text
DenseSeqView::F32(&[f32])
DenseSeqView::F64(&[f64])
RuntimePureInputType::F32
RuntimePureInputType::F64
RuntimePureOutputType::F32
RuntimePureOutputType::F64
```

JIT input boundary は `&[f32]` / `&[f64]` をそのまま使える。

---

# 15. 削除リスト

完全削除。

```text
RuntimeValue::Float(String)
DenseSeq::FloatLiterals
DenseSeqKind::FloatLiterals
RuntimeSeq::dense_float_literals
DenseSeq::float_literals
DenseSeq::as_float_literals
runtime_sequence_dense_float_literals
take_float_value
float literal raw text based runtime equality
float literal raw text based dense storage
```

採用しない。

```text
RuntimeF32 wrapper
RuntimeF64 wrapper
DenseF32Seq
DenseF64Seq
Arc<[T]> DenseSeqStorage
NaN canonicalization pass
bit identity language equality
```

---

# 16. Test plan

## 16.1 Rust-like equality

```rust
#[test]
fn f32_zero_sign_uses_rust_equality() {
    assert_eq!(RuntimeValue::F32(0.0), RuntimeValue::F32(-0.0));
}
```

```rust
#[test]
fn f32_nan_is_not_equal_to_itself() {
    let value = RuntimeValue::F32(f32::NAN);

    assert_ne!(value, value);
}
```

---

## 16.2 exact bits

```rust
#[test]
fn f32_bits_can_distinguish_zero_sign() {
    assert_ne!(0.0f32.to_bits(), (-0.0f32).to_bits());
}
```

---

## 16.3 dense f32

```rust
#[test]
fn f32_sequence_becomes_dense_f32() {
    let value = runtime_sequence_from_literal_values(vec![
        RuntimeValue::F32(1.0),
        RuntimeValue::F32(2.0),
        RuntimeValue::F32(3.0),
    ]);

    assert!(matches!(
        value,
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F32(_)))
    ));
}
```

---

## 16.4 f32 slice view

```rust
#[test]
fn dense_f32_exposes_native_slice() {
    let seq = RuntimeSeq::dense_f32(vec![1.0, 2.0, 3.0]);

    assert_eq!(seq.as_f32_slice(), Some([1.0, 2.0, 3.0].as_slice()));
}
```

---

## 16.5 no float literal string storage

grep test。

```text
RuntimeValue::Float
DenseSeq::FloatLiterals
dense_float_literals
as_float_literals
take_float_value
```

production code に残さない。

---

## 16.6 FMA

```rust
#[test]
fn ordinary_mul_add_expression_is_not_rewritten_to_fma() {
    let expr = parse_expr("a * b + c").unwrap();
    let lowered = lower_checked_expr(expr).unwrap();

    assert!(!contains_float_mul_add_builtin(&lowered));
}
```

```rust
#[test]
fn explicit_mul_add_lowers_to_builtin() {
    let expr = parse_expr("std.f32.mul_add(a, b, c)").unwrap();
    let lowered = lower_checked_expr(expr).unwrap();

    assert!(contains_float_mul_add_builtin(&lowered));
}
```

---

## 16.7 std math

```rust
#[test]
fn std_f32_sin_returns_f32() {
    let value = eval_expr("std.f32.sin(0.0f32)").unwrap();

    assert_eq!(value, RuntimeValue::F32(0.0));
}
```

```rust
#[test]
fn std_f64_sqrt_returns_f64() {
    let value = eval_expr("std.f64.sqrt(4.0)").unwrap();

    assert_eq!(value, RuntimeValue::F64(2.0));
}
```

---

## 16.8 tuple columnar

```rust
#[test]
fn tuple_sequence_uses_columns() {
    let value = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Tuple(vec![RuntimeValue::Int(1), RuntimeValue::F32(1.0)]),
        RuntimeValue::Tuple(vec![RuntimeValue::Int(2), RuntimeValue::F32(2.0)]),
    ]);

    assert!(matches!(
        value,
        RuntimeValue::Seq(RuntimeSeq::TupleColumns(_))
    ));
}
```

---

## 16.9 record columnar

```rust
#[test]
fn record_sequence_uses_columns() {
    let value = runtime_sequence_from_literal_values(vec![
        RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "name".to_owned(),
                value: RuntimeValue::String("a".to_owned()),
            },
            RuntimeFieldValue {
                name: "score".to_owned(),
                value: RuntimeValue::F64(1.0),
            },
        ]),
        RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "name".to_owned(),
                value: RuntimeValue::String("b".to_owned()),
            },
            RuntimeFieldValue {
                name: "score".to_owned(),
                value: RuntimeValue::F64(2.0),
            },
        ]),
    ]);

    assert!(matches!(
        value,
        RuntimeValue::Seq(RuntimeSeq::RecordColumns(_))
    ));
}
```

---

# 17. 実装順序

## Step 1: RuntimeValue / DenseSeq の破壊的置換

```text
RuntimeValue::Float(String) 削除
RuntimeValue::F32(f32) 追加
RuntimeValue::F64(f64) 追加
DenseSeq::FloatLiterals 削除
DenseSeq::F32(DenseSeqStorage<f32>) 追加
DenseSeq::F64(DenseSeqStorage<f64>) 追加
Eq derive 削除
```

## Step 2: float evaluation

```text
unary - for f32/f64
binary + - * / for f32/f64
comparison for f32/f64
Rust-like PartialEq
```

## Step 3: syntax suffix cleanup

```text
FloatSuffix enum
UnitNumberSuffix enum
Literal::UnitNumber
exponent float lexing
NaN/Inf literal rejection
```

## Step 4: checked type table

```text
HirExprId
ExprTypeTable
CheckedModule
runtime-plan lower float using resolved TypeKind
```

## Step 5: std float module

```text
std.f32 constants
std.f64 constants
std.f32 functions
std.f64 functions
to_bits / from_bits
mul_add explicit only
```

## Step 6: columnar tuple / record sequence

```text
RuntimeSeq::TupleColumns
RuntimeSeq::RecordColumns
TupleSeq
RecordSeq
RecordSeqField
ProjectTuple
ProjectRecord
```

## Step 7: accelerator/JIT

```text
DenseSeqView::F32(&[f32])
DenseSeqView::F64(&[f64])
RuntimePureInputType::F32/F64
RuntimePureOutputType::F32/F64
basic arithmetic support
math functions VM fallback
```

---

# 18. 完了条件

```text
[ ] RuntimeValue::Float(String) が存在しない
[ ] RuntimeValue::F32(f32) が存在する
[ ] RuntimeValue::F64(f64) が存在する
[ ] DenseSeq::FloatLiterals が存在しない
[ ] DenseSeq::F32(DenseSeqStorage<f32>) が存在する
[ ] DenseSeq::F64(DenseSeqStorage<f64>) が存在する
[ ] DenseSeqStorage<T> は Vec<T> のまま
[ ] RuntimeValue は Eq を derive していない
[ ] RuntimeValue は PartialEq を derive している
[ ] 0.0f32 == -0.0f32 は true
[ ] std.f32.nan == std.f32.nan は false
[ ] std.f32.to_bits で bit exact comparison ができる
[ ] NaN/Inf は literal ではなく std path
[ ] std.f32.sin などの math functions が VM で動く
[ ] a * b + c は暗黙に mul_add へ rewrite されない
[ ] tuple sequence は TupleColumns になる
[ ] record sequence は RecordColumns になる
[ ] record field projection は ordinal lowering になる
```

---

# 19. 最終形

最終的な float 方針。

```text
Arcweft f32/f64 は Rust native f32/f64 とする。
```

最終的な runtime value。

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    F32(f32),
    F64(f64),
    // ...
}
```

最終的な dense sequence。

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum DenseSeq {
    F32(DenseSeqStorage<f32>),
    F64(DenseSeqStorage<f64>),
    // ...
}
```

最終的な dense storage。

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct DenseSeqStorage<T> {
    values: Vec<T>,
}
```

最終的な equality。

```text
RuntimeValue equality は PartialEq。
float equality は Rust-like。
NaN != NaN。
+0.0 == -0.0。
```

最終的な exact bit API。

```text
std.f32.to_bits
std.f32.from_bits
std.f64.to_bits
std.f64.from_bits
```

この設計が、現時点で最も都合がよい。

理由は、`f32` / `f64` を storage、runtime eval、JIT/AOT boundary、std math 関数で直接扱えるためである。
`Eq` 維持や bit identity equality のために wrapper を導入するより、Arcweft 側の仕様を Rust-like native float に寄せる方が、実装も性能も最終形として自然である。

impl<T: Eq> Eq for DenseSeqStorage<T> {} とかで実装をするとかして、Eq が必要な関数を上手いことを使えるようにしてもよい