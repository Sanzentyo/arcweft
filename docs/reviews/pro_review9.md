# Arcweft の `@` EntityRef 化・Rust 風 attribute・literal / primitive 型設計メモ

## 目的

Arcweft の表面構文を、Rust 利用者が誤解しにくく、かつ HIR / Typed IR / VM / Cranelift JIT へ自然に落とせる形へ整理する。

このメモでは次を決定案として扱う。

```text
1. EntityRef の sigil を `#` から `@` へ移す。
2. Attribute は Rust 風の `#[...]` にする。
3. `@bg` / `@show` のような scenario command 専用構文は廃止し、普通の effectful function call にする。
4. 旧 migration / 旧記法 reject 専用処理は削除する。
5. Color は `"#fff"` / `"#ffff"` / `"#rrggbb"` / `"#rrggbbaa"` の typed string literal として扱う。
6. 数値型は `int` / `float` のような曖昧型を置かず、`i32` / `u64` / `f32` のように bit 幅を明示する。
7. unsuffixed numeric literal は expected type がある場合だけ解決し、Rust の `i32` / `f64` fallback は採用しない。
```

現行 docs では `EntityRef := '#' ...`、`ScenarioCommand := '@' Ident ...`、`@` は attribute / scenario command 用として残っている。これを本設計では置き換える。

---

## 1. 新しい字句規則

### 1.1 EntityRef

```text
EntityRef :=
    '@' Ident ('.' Ident)*
  | '@<' EntityBody '>'
```

例:

```awft
@flow.opening
@character.alice
@asset.bg.room
@choice.opening.listen
@<asset:bg/room.ktx2>
@<activity.truck_game>.run(input)
```

`@<...>` は残す。理由は、entity 本体に `/`、`:`、`@sem:...` などを含む場合と、entity ref の直後に field / method / postfix を置く場合を明確に分けるため。

```awft
@activity.truck_game.run(input)      // 曖昧になりやすい
@<activity.truck_game>.run(input)    // 明確
```

### 1.2 RelativeId

RelativeId は現行通り `.suffix` を使う。

```text
RelativeId := '.' Ident ('.' Ident)*
```

これは entity ref ではなく、ID-bearing context 専用。

```awft
alice(id=.opening): おはよう。[p]

choice .first {
    .listen "聞いてみる" -> @flow.alice_intro
}
```

### 1.3 Attribute

`@derive(...)` / `@link<T>(...)` / `@id(...)` のような attribute は廃止し、Rust 風 `#[...]` に寄せる。

```text
Attribute :=
    '#[' AttributePath AttributeArgs? ']'

AttributePath :=
    IdentPath

AttributeArgs :=
    '(' AttributeTokenTree? ')'
```

例:

```awft
#[derive(Clone, StableHash)]
pub struct ChoiceView {
    id: Ref<ChoiceOption>
    label: String
}

#[link(Flow, @flow.alice_intro, level = .Soft)]
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    ...
}
```

最初は attribute args を token tree として lossless に保存し、HIR / semantic pass 側で必要な attribute だけ解釈してよい。Rust 利用者にとって `#[derive(...)]` は自然であり、`@` を entity ref に専有できる。

### 1.4 `#` の扱い

`#` は attribute opener の一部としてだけ使う。

```awft
#[derive(Clone)]  // OK
#flow.opening     // NG: entity ref は @flow.opening
#fff              // NG: color は "#fff"
```

裸の `#fff` を color として許可しない。理由は、`#[...]` attribute と統一的に扱いたいこと、また Color は expected type と組み合わせた typed string literal として扱う方が Rust / JS / CSS 利用者にも説明しやすいこと。

---

## 2. Scenario command の廃止

### 2.1 専用構文から普通の関数呼び出しへ

旧:

```awft
@bg #asset.bg.room fade=300ms
@show alice smile at=center
```

新:

```awft
bg(@asset.bg.room, fade = 300ms)
show(@character.alice, .smile, at = .center)
```

さらに flow の中では通常の expression statement として扱う。

```awft
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    show(@character.alice, .smile, at = .center, fade = 220ms)

    alice(id=.opening): おはよう。[p]
}
```

### 2.2 機能差

機能的な差は不要。`bg(...)` / `show(...)` / `audio.ensure_bgm(...)` などを effectful function として型付けすればよい。

```awft
fn bg(asset: Ref<Asset<Image>>, fade: Duration = 0ms) -> ()
effects { render_command }

fn show(
    character: Ref<Character>,
    pose: CharacterPose,
    at: StagePosition,
    fade: Duration = 0ms,
) -> ()
effects { render_command }
```

ここでの重要点は、command を syntax 特別扱いではなく、semantic effect として扱うこと。

```text
syntax:
  Expr::Call / Stmt::Expr

HIR/typecheck:
  function call resolution
  effect check
  command emission lowering

runtime:
  RuntimeStepOutput.Command / EffectRequest へ lower
```

JIT 対象にはしない。JIT は pure / deterministic な numeric/dataflow 関数だけに限定し、effectful call は VM / runtime lowering 側で扱う。

---

## 3. 旧記法 reject 処理の削除

`@` を EntityRef にするため、旧 `@memo` / `@choice` / 旧 hook header などを検出して特別な migration error を出す処理は削除する。

削除対象の例:

```text
parser.rs:
  reject_old_memo_attribute
  reject_old_hook_header_syntax
  @choice reject 専用処理
  @memo reject 専用処理

tests.rs:
  rejects_old_at_choice_syntax
  rejects_at_bracket_timed_cue_as_raw_line_plan_item のような
  「旧記法をわざわざ parse して reject する」目的のテスト
```

今後は、旧記法は単に「現在の文法に存在しない token / expression」として自然に parse error になれば十分。

```awft
@choice @choice.opening.first { ... }
// 新文法では `@choice` は EntityRef。
// その後ろに choice body が続くので、自然な構文エラーになる。
```

移行補助が必要な場合は parser に埋め込まず、formatter / CLI migration tool として別に実装する。

```bash
arcw migrate syntax --from old-at-command --to function-command
arcw migrate refs --from hash --to at
```

---

## 4. Grammar diff

`docs/01-language/grammar.md` の中心差分は次。

```diff
- EntityRef    := '#' Ident ('.' Ident)* | '#<' EntityBody '>'
+ EntityRef    := '@' Ident ('.' Ident)* | '@<' EntityBody '>'

- `#` is reserved for entity references and is never a comment introducer.
- `@` remains available for attributes and scenario commands such as `@bg`,
- but `choice` is a flow item and is written without `@`.
+ `@` is reserved for entity references.
+ `#` starts Rust-like attributes only in the `#[...]` form.
+ Scenario commands are ordinary effectful function calls.

- FlowItem := ... | ScenarioCommand | ...
+ FlowItem := ... | ExprStmt | ...

- ScenarioCommand := '@' Ident ScenarioArgs?
+ // removed

+ Attribute := '#[' AttributePath AttributeArgs? ']'
```

Choice 例も全て `@` ref へ更新する。

```diff
- choice .first -> #choice.opening.dream.first
- .listen       -> #choice.opening.dream.first.listen
+ choice .first -> @choice.opening.dream.first
+ .listen       -> @choice.opening.dream.first.listen
```

ただし正規化後の PublicId 本体は `choice.opening.dream.first` のままでよい。`@` は表面構文上の sigil であって ID 本体には含めない。

```text
surface:
  @choice.opening.listen

EntityRef.body:
  "choice.opening.listen"

PublicId:
  "choice.opening.listen"
```

---

## 5. Color literal

### 5.1 基本方針

Color は `color"#fff"` ではなく、`"#fff"` のような string literal を Color expected context で解釈する。

```awft
let white: Color = "#fff"
let white_alpha: Color = "#ffff"
let panel: Color = "#1e1e2ecc"

Text("Settings")
    .color("#fff")
    .background("#101018cc")
```

`"#fff"` は syntax 上は普通の `Literal::String` として保持する。typecheck 時に expected type が `Color` の場合だけ Color として検証・正規化する。

```awft
let s: String = "#fff"  // String
let c: Color = "#fff"   // Color

let x = "#fff"          // String。Color にはしない。
Text("A").color("#fff") // `.color` が Color を要求するので Color
```

### 5.2 許可する形式

```text
"#rgb"        // 12-bit RGB, alpha = ff
"#rgba"       // 16-bit RGBA
"#rrggbb"     // 24-bit RGB, alpha = ff
"#rrggbbaa"   // 32-bit RGBA
```

例:

```awft
"#fff"       // rgba8 = [255, 255, 255, 255]
"#ffff"      // rgba8 = [255, 255, 255, 255]
"#0000"      // rgba8 = [0, 0, 0, 0]
"#7aa2ff"    // rgba8 = [122, 162, 255, 255]
"#1e1e2ecc"  // rgba8 = [30, 30, 46, 204]
```

### 5.3 Color space

MVP では `Color` は sRGB RGBA とする。

将来、色空間を明示したい場合は constructor を使う。

```awft
let c1: Color = "#7aa2ff"
let c2 = Color::srgb("#7aa2ff")?
let c3 = Color::linear_srgb(r = 0.2f32, g = 0.4f32, b = 0.8f32, a = 1.0f32)
let c4 = Color::display_p3("#7aa2ff")?
```

surface literal として `p3"#..."` のような prefix literal は急がない。Rust 利用者には constructor の方が意味が明確。

---

## 6. Primitive 型

### 6.1 Concrete primitive set

`int` / `uint` / `float` のような曖昧な concrete type は置かない。

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

Text / engine primitives:
  String
  Duration
  Color
  Ratio
  Length
  Angle
```

現行 docs の `Unit` / `Bool` は、表面構文では Rust 風に `()` / `bool` へ寄せる。診断や manifest 表示では `Unit` と呼んでもよい。

```awft
fn f() -> () { ... }
let ok: bool = true
let x: i32 = 10
let y: usize = choices.len()
```

### 6.2 禁止する型名

```awft
let x: int = 10       // NG
let y: uint = 10      // NG
let z: float = 1.0    // NG
let n: Number = 1.0   // NG, standard primitive としては置かない
```

理由:

```text
- VM / JIT / save data / wasm / native で bit 幅を明確にする。
- numeric fallback による hidden portability bug を避ける。
- Rust 利用者には `i32` / `f32` が自然。
```

### 6.3 `usize` / `isize`

`usize` / `isize` は欲しい。ただし用途を制限する。

許可:

```awft
fn nth<T>(xs: List<T>, index: usize) -> Option<T> { ... }

let i: usize = 0usize
let item = choices[i]
for i in 0usize..choices.len() {
    ...
}
```

制限:

```text
usize / isize は target-sized なので、
save data / public manifest / stable serialized schema / network protocol には使わない。
```

例:

```awft
state GameState {
    current_choice: u32 = 0      // OK
    // current_choice: usize = 0 // NG in persisted state
}
```

Typed IR / VM / JIT での扱い:

```text
VM:
  canonical debug dump は u64 / i64 表示でもよい。

Native JIT:
  target pointer width に lower。

Wasm:
  wasm32 なら usize = u32。

Stable serialization:
  usize/isize は禁止。明示的に u32/u64/i32/i64 を選ばせる。
```

---

## 7. Numeric literal

### 7.1 Integer literal

Rust 風を採用する。

```text
DecimalInt := [0-9] [0-9_]*
HexInt     := '0x' [0-9A-Fa-f_]+
OctInt     := '0o' [0-7_]+
BinInt     := '0b' [01_]+

IntSuffix :=
    i8 | i16 | i32 | i64 | i128
  | u8 | u16 | u32 | u64 | u128
  | isize | usize
```

例:

```awft
0
1_000
0xffu8
0xff_u8       // optional: Rust compatibility を優先するなら許可
0b1010_0101u8
0o755u32
42usize
```

`-1i32` は negative literal ではなく、unary `-` + `1i32` として扱う。これは Rust と同じ。

```awft
let a: i32 = -1
```

syntax AST では raw string を保持し、parse 時点で `i64` に潰さない。`i128` / `u128` / overflow diagnostics のため。

```rust
pub struct IntLiteral {
    pub raw: String,
    pub radix: IntRadix,
    pub suffix: Option<IntSuffix>,
}
```

### 7.2 Integer literal の型解決

unsuffixed integer literal は expected type がある場合だけ解決する。

```awft
let a: i32 = 10       // OK
let b: u64 = 10       // OK
let c = 10            // NG: expected type がない
let d = 10u32         // OK
```

関数引数では signature から expected type が来る。

```awft
fn add_affection(delta: i32) { ... }

add_affection(3)      // OK: 3 は i32
add_affection(3u32)   // NG: u32 を i32 に暗黙変換しない
```

暗黙の widening / narrowing はしない。

```awft
let a: i32 = 10u8     // NG unless explicit conversion
let b: u64 = 10u32    // NG unless explicit conversion

let c: i32 = i32::from(10u8)
let d: u64 = u64::from(10u32)
```

### 7.3 Float literal

```text
FloatLiteral :=
    digits '.' digits? exponent? FloatSuffix?
  | digits exponent FloatSuffix?

FloatSuffix :=
    f32 | f64

Exponent :=
    ('e' | 'E') ('+' | '-')? digits
```

例:

```awft
1.0
0.5
1e-3
1.0f32
0.5f64
6.283_185f64
```

型解決:

```awft
let a: f32 = 0.5      // OK
let b: f64 = 0.5      // OK
let c = 0.5           // NG: fallback なし
let d = 0.5f32        // OK
```

Rust の `f64` fallback は採用しない。理由は整数と同じで、JIT / shader / layout / audio で bit 幅の曖昧さを避けるため。

### 7.4 `NaN` / `Infinity`

`NaN` / `Infinity` を literal としては置かない。

```awft
let a = f32::NAN
let b = f64::INFINITY
```

これにより parser は literal と path / associated constant を分けやすくなる。

---

## 8. Unit-number literal

### 8.1 Duration

現行の `ms` / `s` を一般化する。

```text
DurationLiteral :=
    Number ('ns' | 'us' | 'ms' | 's' | 'min' | 'h')
```

例:

```awft
120ms
1.5s
2min
1h
```

内部正規化は `nanos: i128` がよい。

```text
1.5s -> 1_500_000_000ns
```

Float にせず decimal / rational として parse して nanos に落とすと、決定性が高い。

### 8.2 Percent / Ratio

`%` は binary remainder operator としても使うため、字句規則を明確にする。

```text
RatioLiteral :=
    Number '%'
```

ただし `Number` と `%` の間に空白がない場合だけ literal。

```awft
50%       // Ratio literal
100%      // Ratio literal

a % b     // remainder operator
a% b      // formatter should reject or normalize to `a % b`
```

型:

```awft
let opacity: Ratio = 50%
```

`Ratio` は `f32` ではない。Typed IR で必要に応じて `f32` / `f64` へ正規化する。

```text
50% -> Ratio { numerator: 1, denominator: 2 }
```

### 8.3 Length

UI / layout / text 用。

```text
LengthLiteral :=
    Number ('px' | 'pt' | 'em' | 'rem' | 'vw' | 'vh')
```

例:

```awft
Text("Settings")
    .font_size(18pt)
    .padding(24px)
    .width(50%)
```

`50%` は expected type により解釈を変える。

```awft
let opacity: Ratio = 50%   // Ratio
let width: Length = 50%    // Length::Percent
```

つまり syntax 上は `UnitNumberLiteral { suffix: Percent }`、typecheck で `Ratio` か `Length` へ解決する。

### 8.4 Angle

```text
AngleLiteral :=
    Number ('deg' | 'rad' | 'turn')
```

例:

```awft
90deg
3.14159rad
0.25turn
```

内部正規化は radians。

```text
Typed IR:
  Angle { radians: f64 }

JIT:
  expected type に応じて f32/f64 radians
```

### 8.5 Audio / music unit

Audio docs に合わせて次を入れる。

```text
AudioLiteral :=
    Number ('db' | 'lufs' | 'bpm' | 'bars')
```

例:

```awft
# 新文法では ref は @ だが、値リテラルはそのまま
pub mixer snapshot @mix.dialogue {
    @bus.bgm.volume = -8db over 300ms
    @bus.voice.volume = 0db
}

pub music pattern @music.pattern.soft_piano {
    tempo = 92bpm
}

pub ducking @duck.voice_over_bgm {
    amount = -6db
    attack = 120ms
    release = 500ms
}
```

入力として `LUFS` を受けてもよいが、formatter は `lufs` へ正規化する。

```awft
-18LUFS   // parse OK
-18lufs   // formatted canonical
```

---

## 9. String literal

### 9.1 Basic string

通常 string は Rust 風 escape を基本にする。

```text
StringLiteral := '"' ... '"'
```

例:

```awft
"hello"
"line\nbreak"
"quote: \""
```

### 9.2 Raw string

Rust 風 raw string を採用する。

```awft
r"raw string"
r#"raw string with " quotes"#
r##"raw string with "# inside"##
```

Dialogue / rich text / Typst / shader など、escape を減らしたい場面で有効。

### 9.3 Multi-line string

Rust 風 raw stringで十分なら triple quote は急がない。ただし docs / Typst / shader block の authoring では multi-line source が多いので、`"""..."""` を入れるかどうかは別途決定する。

候補:

```awft
let text = """
multi
line
"""
```

または raw string のみ:

```awft
let text = r#"
multi
line
"#
```

Rust 利用者中心なら、MVP は raw string のみでよい。

### 9.4 Interpolation

JS/TS 風 template literal は MVP では入れない。Dialogue syntax と衝突しやすく、LSP recovery も難しくなるため。

```awft
// MVP:
format("score={score}", score = score)

// 将来候補:
f"score={score}"
```

### 9.5 Color as typed string

Color は string literal の expected type 解釈。

```awft
let c: Color = "#1e1e2ecc"
```

syntax parser では `Literal::String("#1e1e2ecc")` のまま。typecheck で `Color` expected の場合だけ検証し、Typed IR で `TypedLiteral::Color` にする。

---

## 10. Bool / Unit / Option / null

```awft
true
false
()
```

`null` はない。`Option<T>` を使う。

```awft
let next: Option<Ref<Flow>> = None
let next = Some(@flow.alice_intro)
```

---

## 11. Char literal は MVP では入れない

Rust 風なら `'a'` char literal が自然だが、Arcweft では `'choose:` のような label syntax があるため、MVP では `char` literal を入れない方が安全。

```awft
'choose: loop {
    break 'choose route
}
```

文字単位処理が必要な場合は `String` / `Text` API で扱う。

```awft
"あ".chars().first()
```

将来 `char` を入れる場合は、label と明確に分ける grammar が必要。

---

## 12. List / tuple / record literal

既存の式 grammar に合わせて維持する。

```awft
[1i32, 2i32, 3i32]
("alice", 3i32)
{ x = 1i32, y = 2i32 }
Point { x = 1.0f32, y = 2.0f32 }
```

`{ ... }` は block / record literal / scope と文脈で分かれるため、syntax parser は引き続き context-aware に扱う。

---

## 13. Type inference policy

### 13.1 期待型がある場合

```awft
let count: u32 = 10
let alpha: f32 = 0.5
let duration: Duration = 300ms
let color: Color = "#fff"
```

### 13.2 期待型がない場合

```awft
let count = 10     // NG
let alpha = 0.5    // NG

let count = 10u32  // OK
let alpha = 0.5f32 // OK
let color = "#fff" // OK, String
```

`"#fff"` は String としては unambiguous なので `let color = "#fff"` は String になる。Color にしたいなら annotation か expected type が必要。

```awft
let color: Color = "#fff"
Button().background("#fff")
```

### 13.3 Numeric conversion

暗黙 numeric conversion はしない。

```awft
fn set_alpha(alpha: f32) -> ()

set_alpha(1)       // NG: integer literal から f32 への暗黙変換なし
set_alpha(1.0)     // OK: expected f32
set_alpha(1.0f32)  // OK
```

整数から float にしたい場合は明示する。

```awft
set_alpha(f32::from(1u8))
```

---

## 14. Syntax AST 設計

`crates/arcweft-lang-syntax/src/expr.rs` の `Literal` は、現在 `Int(i64)` になっているが、bit 幅明示と overflow diagnostics のため raw-preserving にする。

```rust
pub enum Literal {
    String(StringLiteral),
    Int(IntLiteral),
    Float(FloatLiteral),
    Bool(bool),
    Unit,
    UnitNumber(UnitNumberLiteral),
}

pub struct StringLiteral {
    pub raw: String,
    pub cooked: String,
    pub kind: StringLiteralKind,
}

pub enum StringLiteralKind {
    Normal,
    Raw { hashes: u8 },
}

pub struct IntLiteral {
    pub raw: String,
    pub radix: IntRadix,
    pub suffix: Option<IntSuffix>,
}

pub enum IntRadix {
    Binary,
    Octal,
    Decimal,
    Hex,
}

pub enum IntSuffix {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    Isize,
    Usize,
}

pub struct FloatLiteral {
    pub raw: String,
    pub suffix: Option<FloatSuffix>,
}

pub enum FloatSuffix {
    F32,
    F64,
}

pub struct UnitNumberLiteral {
    pub raw_number: String,
    pub suffix: UnitSuffix,
}

pub enum UnitSuffix {
    Ns,
    Us,
    Ms,
    S,
    Min,
    H,
    Percent,
    Px,
    Pt,
    Em,
    Rem,
    Vw,
    Vh,
    Deg,
    Rad,
    Turn,
    Db,
    Lufs,
    Bpm,
    Bars,
}
```

`Literal::Color` は syntax AST には入れない。Color は string literal の typed interpretation として扱う。

---

## 15. Typed HIR / Typed IR literal

typecheck 後は正規化済み literal にする。

```rust
pub enum TypedLiteral {
    Bool(bool),
    Unit,

    Int {
        value: i128,
        ty: SignedIntTy,
    },
    UInt {
        value: u128,
        ty: UnsignedIntTy,
    },
    Float {
        bits: FloatBits,
        ty: FloatTy,
    },

    String(String),

    Duration {
        nanos: i128,
    },
    Color {
        rgba8: [u8; 4],
        color_space: ColorSpace,
    },
    Ratio {
        numerator: i128,
        denominator: i128,
    },
    Length(LengthValue),
    Angle {
        radians: f64,
    },
    Audio(AudioLiteralValue),
}

pub enum SignedIntTy {
    I8, I16, I32, I64, I128, Isize,
}

pub enum UnsignedIntTy {
    U8, U16, U32, U64, U128, Usize,
}

pub enum FloatTy {
    F32,
    F64,
}

pub enum LengthValue {
    Px(f64),
    Pt(f64),
    Em(f64),
    Rem(f64),
    Vw(f64),
    Vh(f64),
    Percent { numerator: i128, denominator: i128 },
}
```

Typed HIR では units を意味付きに保つ。Typed IR / JIT lowering で必要な backend representation に変換する。

---

## 16. JIT への lowering

JIT は pure numeric/dataflow subset に限定する。unit を含む literal は、JIT に入る前に expected type / layout context で正規化する。

```text
Duration:
  i64 nanos
  or f64 seconds if API explicitly requires seconds

Ratio:
  f32 / f64

Length:
  layout pass が px に resolve
  JIT へは f32 / f64 px

Angle:
  radians f32 / f64

Color:
  packed u32
  or vec4<f32> linear/srgb converted
```

例:

```awft
pure jit fn ease_out(t: Ratio) -> f32 {
    let x: f32 = t.to_f32()
    1.0f32 - (1.0f32 - x) * (1.0f32 - x)
}
```

`bg(...)` / `show(...)` / `audio.ensure_bgm(...)` のような effectful function は JIT 対象外。

---

## 17. Parser 実装方針

### 17.1 `expr.rs`

変更:

```diff
- '#' => self.lex_entity(),
+ '@' => self.lex_entity(),

+ '#' if self.starts_with("#[") => Token::AttrStart or parser-level attribute handling
+ '#' => error/unexpected token

- fn lex_number_or_duration
+ fn lex_number_or_unit

- Literal::Int(i64)
+ Literal::Int(IntLiteral)

- Literal::Float(String)
+ Literal::Float(FloatLiteral)

- Literal::Duration { amount, unit }
+ Literal::UnitNumber(UnitNumberLiteral)
```

`parse_entity_expr`:

```diff
- strip_prefix("#<")
- strip_prefix('#')
+ strip_prefix("@<")
+ strip_prefix('@')
```

### 17.2 `parser.rs`

変更:

```diff
- parse_attribute(trimmed, range) // @attribute
+ parse_outer_attribute_lines()   // #[...]

- reject_old_memo_attribute
- reject_old_hook_header_syntax
- ScenarioCommand parsing
```

Flow item:

```diff
- FlowItem::ScenarioCommand(ScenarioCommand)
+ FlowItem::Stmt(Stmt::Expr(Expr::Call { ... }))
```

`bg(...)` / `show(...)` は普通に expression parser へ渡る。

### 17.3 `ast.rs`

変更:

```diff
- pub struct Attribute { name: String, args: Option<String>, ... }
+ pub struct Attribute {
+     path: String,
+     args: Option<String>, // initially token-tree/raw
+     range: TextRange,
+ }

- pub struct ScenarioCommand
- FlowItem::ScenarioCommand
```

`EntityRef` は body / delimited / relative / range を維持してよい。sigil は body には含めない。

### 17.4 `lower.rs`

変更:

```diff
- HirFlowItem::Scenario { name, args }
+ // removed

- FlowItem::ScenarioCommand(command) => HirFlowItem::Scenario { ... }
+ // ordinary Stmt::Expr call path
```

### 17.5 `check.rs`

`TypeKind::Int` / `TypeKind::Float` を分解する。

```rust
pub enum TypeKind {
    Unit,
    Bool,
    SignedInt(SignedIntTy),
    UnsignedInt(UnsignedIntTy),
    Float(FloatTy),
    String,
    Duration,
    Color,
    Ratio,
    Length,
    Angle,
    Ref(EntityKind),
    ...
}
```

literal checking は expected type を受け取る形にする。

```rust
fn check_expr_expected(&mut self, expr: &Expr, expected: Option<&TypeKind>) -> Option<TypeKind>
```

---

## 18. Formatter / diagnostics

### 18.1 Formatter

```text
- `#flow.opening` は `@flow.opening` へ移行候補を出す。
- `@bg ...` は `bg(...)` へ migration tool が変換する。
- `@derive(...)` は `#[derive(...)]` へ移行する。
- `-18LUFS` は `-18lufs` へ正規化する。
- `a% b` は `a % b` へ正規化するか、`a % b` を要求する。
- `50 %` は Ratio literal ではなく remainder として扱う。
```

### 18.2 Diagnostics

例:

```text
error: `#flow.opening` is not an entity reference in the current grammar
help: write `@flow.opening`

error: bare hex colors are not Arcweft expressions
help: write `"#fff"` and use it where `Color` is expected

error: numeric literal has no inferred type
help: add a suffix such as `10i32`, or add a type annotation: `let x: i32 = 10`

error: `int` is not a concrete Arcweft type
help: use an explicit width such as `i32`, `i64`, or `usize`
```

---

## 19. Example after the change

```awft
mod crate::game::routes::opening

use crate::game::prelude::*
use super::logic::affection::has_affection_at_least

#[derive(Clone, StableHash)]
pub struct ChoiceView {
    id: Ref<ChoiceOption>
    label: String
    color: Color
}

pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    show(@character.alice, .smile, at = .center, fade = 220ms)

    scope greeting {
        alice(id=.opening): おはよう。[p]
    }

    let threshold: i32 = 3
    let can_enter_alice: bool =
        state |> has_affection_at_least(@character.alice, threshold)

    Text("聞いてみる")
        .font_size(18pt)
        .padding(x = 24px, y = 12px)
        .color("#fff")
        .background("#101018cc")
        .opacity(85%)

    scope dream {
        choice .first {
            .listen "聞いてみる" if can_enter_alice -> @flow.alice_intro
            .listen_locked "聞いてみる" -> @flow.alice_locked
            .silent "黙っている" -> @flow.quiet_intro
        }
    }
}
```

---

## 20. Test plan

### 20.1 EntityRef

```rust
#[test]
fn parses_at_entity_refs() {
    let expr = parse_expr("@flow.opening").unwrap();
    assert!(matches!(expr, Expr::EntityRef(entity) if entity.body() == "flow.opening"));
}

#[test]
fn parses_delimited_at_entity_refs() {
    let expr = parse_expr("@<asset:bg/room.ktx2>").unwrap();
    assert!(matches!(expr, Expr::EntityRef(entity) if entity.is_delimited()));
}
```

### 20.2 Attribute

```rust
#[test]
fn parses_rust_like_attributes() {
    let tree = parse_source(r#"
#[derive(Clone, StableHash)]
pub struct ChoiceView {
    id: Ref<ChoiceOption>
}
"#).unwrap();

    assert!(matches!(tree.items()[0], Item::Struct(_)));
    // struct.attrs()[0].path == "derive"
}
```

### 20.3 Commands as calls

```rust
#[test]
fn parses_bg_as_function_call() {
    let tree = parse_source(r#"
flow @flow.opening opening {
    bg(@asset.bg.room, fade = 300ms)
}
"#).unwrap();

    // flow.body()[0] is Stmt::Expr(Expr::Call { callee: Path("bg"), ... })
}
```

### 20.4 No old command syntax

```rust
#[test]
fn at_bg_is_not_a_command() {
    let errors = parse_source(r#"
flow @flow.opening opening {
    @bg @asset.bg.room
}
"#).unwrap_err();

    // @bg parses as EntityRef; the following tokens make it invalid as a statement.
}
```

### 20.5 Color typed string

```rust
#[test]
fn string_hex_becomes_color_only_with_expected_color() {
    let lit = Literal::String("#fff".into());

    assert_eq!(check_literal(&lit, expected(Color)), TypedLiteral::Color { ... });
    assert_eq!(check_literal(&lit, expected(String)), TypedLiteral::String("#fff".into()));
}
```

### 20.6 Numeric fallback disabled

```rust
#[test]
fn unsuffixed_numeric_literal_requires_expected_type() {
    assert_error("let x = 10");
    assert_ok("let x: i32 = 10");
    assert_ok("let x = 10i32");
}
```

---

## 21. PR 分割案

### PR 1: docs update

```text
docs/01-language/grammar.md
docs/01-language/syntax.md
docs/00-overview/decisions.md
docs/00-overview/implementation-guide.md
```

内容:

```text
- `@` EntityRef
- `#[...]` attribute
- scenario command removal
- commands as functions
- literal / primitive type policy
```

### PR 2: parser sigil change

```text
crates/arcweft-lang-syntax/src/expr.rs
crates/arcweft-lang-syntax/src/parser.rs
crates/arcweft-lang-syntax/src/ast.rs
crates/arcweft-lang-syntax/src/tests.rs
```

内容:

```text
- `@` entity refs
- `#[...]` attributes
- `#` entity refs removal
- tests update
```

### PR 3: remove old syntax reject and ScenarioCommand

```text
- reject_old_memo_attribute removal
- reject_old_hook_header_syntax removal
- ScenarioCommand AST removal
- HirFlowItem::Scenario removal
- @bg/@show tests replacement
```

### PR 4: literal AST modernization

```text
- IntLiteral raw-preserving
- FloatLiteral suffix
- UnitNumberLiteral
- StringLiteral raw string support
- no `i64` eager parse
```

### PR 5: typecheck literal policy

```text
- primitive TypeKind split
- no `int` / `float`
- no numeric fallback
- expected-type literal resolution
- Color from typed string
- usize/isize restrictions for persisted schemas
```

### PR 6: Typed IR / VM / JIT lowering

```text
- TypedLiteral
- unit normalization
- JIT accepted type layout checks
- VM fallback tests
- compare-vm tests for pure numeric functions
```

---

## 22. 最終決定案

```text
EntityRef:
  @flow.opening
  @<asset:bg/room.ktx2>

Attribute:
  #[derive(Clone)]
  #[link(Flow, @flow.opening)]

Command:
  bg(@asset.bg.room, fade = 300ms)
  show(@character.alice, .smile, at = .center)

Color:
  "#fff"
  "#ffff"
  "#rrggbb"
  "#rrggbbaa"
  expected type が Color の場合だけ Color として解釈

Numeric types:
  i8 i16 i32 i64 i128
  u8 u16 u32 u64 u128
  isize usize
  f32 f64

Forbidden concrete type names:
  int
  uint
  float
  Number

Numeric literal:
  no fallback
  suffix or expected type required

JIT:
  pure numeric/dataflow only
  units are normalized before JIT lowering
```

この組み合わせが、Rust 利用者中心の期待に最も近く、Arcweft の HIR / VM / JIT / deterministic runtime とも相性がよい。
