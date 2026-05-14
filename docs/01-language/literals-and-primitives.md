# Literals and Primitive Types

Arcweft uses Rust-like primitive spellings and raw-preserving literal syntax so
the parser, HIR, VM, save data, and JIT lowering all see explicit widths and
deterministic unit values.

## Primitive Types

Concrete numeric primitives always spell out their width.

```text
Unit:
  ()

Bool:
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

```awft
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

```awft
1.0
0.5
1e-3
2.0f32
0.5f64
6.283_185f64
```

`-1i32` is unary `-` applied to the positive literal `1i32`; negative numbers
are not separate literal tokens.

Unsuffixed numeric literals require an expected type.

```awft
let a: i32 = 10       // OK
let b: f32 = 2.0      // OK
let c = 10            // error: no expected numeric type
let d = 10i32         // OK
let e = 2.0f32        // OK
```

Function signatures also provide expected types.

```awft
fn add_affection(delta: i32) -> () { ... }

add_affection(3)      // OK: expected i32
add_affection(3u32)   // error: no implicit numeric conversion
```

Arcweft does not perform implicit widening, narrowing, or integer-to-float
conversion.

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

```awft
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

```awft
50%       // unit-number literal
a % b     // remainder
```

Formatters should normalize uppercase audio units such as `LUFS` to lowercase
`lufs`.

## Color

Color is not a bare `#fff` token. It is a normal string literal interpreted as
`Color` only when the expected type is `Color`.

```awft
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

```awft
let c1: Color = "#7aa2ff"
let c2 = Color::srgb("#7aa2ff")?
let c3 = Color::linear_srgb(r = 0.2f32, g = 0.4f32, b = 0.8f32, a = 1.0f32)
```

## String Literals

Normal strings use Rust-like escapes.

```awft
"hello"
"line\nbreak"
"quote: \""
```

Raw strings use Rust-like raw string syntax.

```awft
r"raw string"
r#"raw string with " quotes"#
r##"raw string with "# inside"##
```

MVP does not include JavaScript-style template literals. Use formatting
functions for interpolation:

```awft
format("score={score}", score = score)
```

## Syntax AST Policy

The syntax AST should preserve literal raw text instead of eagerly converting
numeric literals to host Rust integers. This is required for `i128`/`u128`,
overflow diagnostics, suffix-aware type checking, unit normalization, and exact
formatting.

Recommended syntax shapes:

```rust
pub enum Literal {
    String(StringLiteral),
    Int(IntLiteral),
    Float(FloatLiteral),
    Bool(bool),
    Unit,
    UnitNumber(UnitNumberLiteral),
}

pub struct IntLiteral {
    pub raw: String,
    pub radix: IntRadix,
    pub suffix: Option<IntSuffix>,
}

pub struct FloatLiteral {
    pub raw: String,
    pub suffix: Option<FloatSuffix>,
}

pub struct UnitNumberLiteral {
    pub raw_number: String,
    pub suffix: UnitSuffix,
}
```

`Literal::Color` should not exist in the syntax AST. Color is a typed
interpretation of string literals.
