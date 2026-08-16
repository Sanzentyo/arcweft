# Literals and Primitive Types

Arcweft uses Rust-like primitive spellings and raw-preserving literal syntax so
the parser, HIR, VM, save data, and JIT lowering all see explicit widths and
deterministic unit values.

## Primitive Types

Concrete numeric primitives always spell out their width.

```text
Unit:
  Unit

Boolean:
  bool

Signed integers:
  i8 i16 i32 i64 i128

Unsigned integers:
  u8 u16 u32 u64 u128

Target-sized integers:
  isize usize

Floats:
  f32 f64

Engine primitives:
  String
  char
  TextCluster
  Duration
  Color
  Ratio
  Length
  Angle
```

`int`, `uint`, `float`, and `Number` are not concrete standard primitive type
names. Use an explicit type such as `i32`, `u64`, or `f32`.

`usize` and `isize` are allowed for indexing and target-sized APIs, but stable
serialized state, public manifests, save schemas, and network protocols should
use fixed-width integer types instead.

## Numeric Literals

Integer literals preserve their raw spelling, radix, and optional suffix.

```arcw
0
1_000
0xffu8
0xff_u8
0b1010_0101u8
0o755u32
42usize
10i32
```

Float literals preserve their raw spelling and optional suffix.

```arcw
1.0
0.5
1e-3
2.0f32
0.5f64
6.283_185f64
```

`-1i32` is unary `-` applied to the positive literal `1i32`; negative numbers
are not separate literal tokens.

Arcweft `f32` and `f64` values use Rust-like native float semantics. Ordinary
equality is `PartialEq`: `0.0f32 == -0.0f32` is true, and NaN is not equal to
itself. Exact bit identity is an explicit operation through the standard float
module.

NaN and infinity are not float literal spellings. Use standard constants:

```arcw
std.f32.nan
std.f32.infinity
std.f32.neg_infinity
std.f64.nan
std.f64.infinity
std.f64.neg_infinity
```

The standard float module provides Rust-like math, predicate, bit conversion,
and explicit cast helpers:

```arcw
let x: f32 = std.f32.sqrt(4.0f32)
let exact: u32 = std.f32.to_bits(-0.0f32)
let restored: f32 = std.f32.from_bits(exact)
let finite: bool = std.f64.is_finite(1.0f64)
let widened: f64 = std.f32.to_f64(x)
let narrowed: f32 = std.f64.to_f32(widened)
```

`a * b + c` is not implicitly rewritten to fused multiply-add. Use
`std.f32.mul_add(a, b, c)` or `std.f64.mul_add(a, b, c)` when one-rounding FMA
semantics are intended.

Unsuffixed numeric literals use expected types when one is available. Without
an expected type, integer literals default to `i32` and float literals default
to `f64`.

```arcw
let a: i32 = 10       // OK
let b: f32 = 2.0      // OK
let c = 10            // OK: defaults to i32
let d = 2.0           // OK: defaults to f64
let e = 10i32         // OK
let f = 2.0f32        // OK
```

Function signatures also provide expected types.

```arcw
fn add_affection(delta: i32) -> Unit { ... }

add_affection(3)      // OK: expected i32
add_affection(3u32)   // error: no implicit numeric conversion
```

Arcweft does not perform implicit widening, narrowing, or integer-to-float
conversion.

## char and TextCluster

`char` is the low-level Unicode scalar value type. It is intentionally close to
Rust's `char` and is useful for parser, tokenizer, normalization, and low-level
text processing.

`char` is not a visual character.

`TextCluster` is the Arcweft text unit used for display, reveal, ruby, and text
effects. It can be based on Unicode grapheme clusters, but it is not specified
as exactly UAX #29 grapheme clustering. The engine may group or split text
units for variation selectors, combining marks, emoji sequences, vertical text,
ruby bases, localization, and presentation effects.

```text
char:
  Unicode scalar value

TextCluster:
  display/reveal/ruby/effect text unit
```

Single-character literals use a Rust-like string body with a `c` suffix. The
body must decode to exactly one Unicode scalar value.

```arcw
let a: char = "a"c
let newline: char = "\n"c
let light: char = "💡"c
let hiragana: char = "\u{3042}"c
```

Rejected forms:

```arcw
let empty: char = ""c
let two_scalars: char = "ab"c
let combining_sequence: char = "e\u{301}"c
let flag_sequence: char = "🇯🇵"c
```

Use `String` or `TextCluster` APIs for display text units. A one-scalar `char`
literal does not imply that the value occupies one player-visible glyph cell.

## Unit-Number Literals

Unit-number literals are syntax-level values that type checking resolves into
semantic primitives.

```text
Duration:
  ns us ms s min h

Ratio / percentage:
  %

Length:
  px pt em rem vw vh

Angle:
  deg rad turn

Audio / music:
  db lufs bpm bars
```

Examples:

```arcw
let fade: Duration = 300ms
let frame: Duration = 16_666us
let hold: Duration = 1.5s
let font_size: Length = 100pt
let padding: Length = 24px
let width: Length = 50%
let opacity: Ratio = 85%
let quarter: Angle = 0.25turn
let right_angle: Angle = 90deg
let theta: Angle = 1.57079632679rad
let bgm_gain = -6db
let target_loudness = -18lufs
let tempo = 92bpm
```

`%` is a unit suffix only when it is adjacent to the number. With whitespace it
is the remainder operator.

```arcw
50%       // unit-number literal
a % b     // remainder
```

Formatters should normalize uppercase audio units such as `LUFS` to lowercase
`lufs`.

## Color

Color is not a bare `#fff` token. It is a normal string literal interpreted as
`Color` only when the expected type is `Color`.

```arcw
let s: String = "#fff"
let c: Color = "#fff"

Text("Settings")
    .color("#fff")
    .background("#101018cc")
```

Accepted color strings in `Color` context:

```text
"#rgb"
"#rgba"
"#rrggbb"
"#rrggbbaa"
```

MVP `Color` is sRGB RGBA. Future color-space-specific values should use
constructors rather than prefix literals.

```arcw
let c1: Color = "#7aa2ff"
let c2 = try Color.srgb("#7aa2ff")
let c3 = Color.linear_srgb(r = 0.2f32, g = 0.4f32, b = 0.8f32, a = 1.0f32)
```

## String Literals

Normal strings use Rust-like escapes.

```arcw
"hello"
"line\nbreak"
"quote: \""
```

Raw strings use Rust-like raw string syntax.

```arcw
r"raw string"
r#"raw string with " quotes"#
r##"raw string with "# inside"##
```

MVP does not include JavaScript-style template literals. Use formatting
functions for interpolation:

```arcw
format("score={score}", score = score)
```

## Syntax AST Policy

The syntax AST preserves literal raw text instead of eagerly converting
numeric literals to host Rust integers. This is required for `i128`/`u128`,
overflow diagnostics, suffix-aware type checking, unit normalization, and exact
formatting.

The canonical integer syntax node owns its raw spelling, radix, and typed
`IntSuffix`. Its non-negative magnitude is parsed on demand as `u128`; a value
larger than `u128` remains a valid syntax node so semantic analysis can emit a
structured overflow diagnostic. Compact integer bracket sequences retain these
same nodes instead of narrowing their elements to a host integer width.

Canonical syntax shapes:

```rust
pub enum Literal {
    String(StringLiteral),
    Char(CharLiteral),
    Int(IntLiteral),
    Float(FloatLiteral),
    Bool(bool),
    Unit,
    UnitNumber(UnitNumberLiteral),
}

pub struct IntLiteral {
    raw: String,
    radix: IntRadix,
    suffix: Option<IntSuffix>,
}

pub struct FloatLiteral {
    pub raw: String,
    pub suffix: Option<FloatSuffix>,
}

pub struct UnitNumberLiteral {
    pub raw_number: String,
    pub suffix: UnitSuffix,
}

pub struct CharLiteral {
    pub raw: String,
    pub value: char,
}
```

Non-canonical primitive spellings are syntax errors in every type-owning source
position, including binding ascriptions, declaration fields, callable
signatures, and trait/impl members. Parser recovery may omit the invalid typed
node, but it must retain a structured diagnostic; it must not silently turn an
ascribed binding into an inferred one.

`Literal::Color` should not exist in the syntax AST. Color is a typed
interpretation of string literals.

Semantic analysis records the resolved numeric primitive for every integer or
float literal and compact integer sequence after applying suffix, expected-type,
or fallback rules. Checked runtime-plan lowering consumes that evidence; it must
not independently reapply `i32` / `f64` fallback when an expected type exists.
For example, an unsuffixed literal expected as `u128` lowers directly to a
`u128` runtime value. Unary negation admits the one extra positive magnitude
needed to represent each signed minimum, including `-128i8` and the `i128`
minimum, but the same positive literal is out of range outside that negation.
Contract SMT lowering uses arbitrary-precision decimal integer terms, so an
exact `u128` literal is not narrowed through a host `i64` on its way to QF_LIA.

