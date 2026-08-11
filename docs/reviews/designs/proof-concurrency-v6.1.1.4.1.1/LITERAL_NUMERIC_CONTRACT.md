# Corrected literal, Duration, overflow, and numeric contract

## Direct typed lowering

Lowering consumes canonical attached typed literal components only. It never calls `raw_text`, slices a source document, scans Rowan tokens, splits display labels, or runs a second lexer.

## Strings and characters

Escaped and raw strings both lower to decoded UTF-8. Delimiter/escape spelling is source-only. `DecodedStringBytes` is charged after decoding and before HIR allocation: 8,388,608 bytes commits; 8,388,609 returns `HirLowerError::Limit` and rolls back the owner. Invalid escape and unterminated forms under the limit retain String-family poison; no empty default is fabricated.

A Character must decode to exactly one Unicode scalar. Empty, multiple-scalar, invalid-escape, and unterminated forms retain typed Character poison.

## Integers

Magnitude is a non-negative `HirBigUint` in canonical little-endian base-2^32 limbs. Unary minus is a separate `HirUnaryOp::Negate`. `u128::MAX + 1` and larger values are retained exactly up to 65,536 source digits. One-over is a hard lowerer limit failure, not an overflow flag in a truncated `u128`.

The checker selects explicit suffix, exact contextual integer type, then i32. It checks the arbitrary magnitude against the selected width. Unary minus admits the one additional positive magnitude required by the selected signed minimum. Rejection publishes no runtime constant.

## Canonical decimal

1. Remove leading coefficient zeros.
2. Canonical zero is digits `[0]`, scale 0, exponent 0.
3. Initial scale is the authored fractional digit count.
4. Remove trailing coefficient zeros while scale is positive, decrementing scale.
5. Remove remaining trailing coefficient zeros and add their count to exponent10.
6. Add the authored exponent with checked arithmetic.
7. Preflight coefficient digits <= 65,536, scale <= 65,536, and absolute exponent <= 1,000,000.

The value is `coefficient × 10^(exponent10 - scale)`. Hard limit failure rolls back; malformed digits under limits retain typed decimal poison.

## Float phase boundary

HIR stores canonical decimal plus optional explicit width. Width selection is explicit suffix, exact f32/f64 context, then f64. `arcweft-lang-sema::literal` converts with IEEE-754 round-to-nearest, ties-to-even.

- finite normal/subnormal and signed zero => `FloatLiteralCheckResult::Accepted(CheckedFloatLiteral)`;
- a value rounding to infinity => `Rejected(FloatLiteralCheckError::WidthOverflow)`;
- NaN/infinity are not literal spellings.

`HirFloatIssue::WidthOverflow` is deleted. The checker does not publish default zero, infinity, truncated bits, or runtime-plan data on rejection.

## Unit number

A canonical decimal is paired with one typed unit. Percent is Ratio; px/pt/em/rem/vw/vh are Length; deg/rad/turn are Angle; db/lufs are Audio; bpm/bars are Music. No downstream suffix parser exists.

## Duration representation and equality

Duration is a distinct HIR and language type. The source amount is a non-negative canonical decimal with ns/us/ms/s/min/h. Exact multiplication factors are 1, 1,000, 1,000,000, 1,000,000,000, 60,000,000,000, and 3,600,000,000,000. Fractional nanoseconds retain Duration-family poison; no rounding/truncation occurs.

```rust
pub struct HirDurationSemanticValue { nanoseconds: HirBigUint }
pub struct HirDurationValue {
    semantic: HirDurationSemanticValue,
    authored_unit: HirDurationUnit,
}
```

Both records derive structural Eq/Hash/Ord. Structural equality includes unit. Semantic equality/order/hash use only `HirDurationSemanticValue`, obtained through `semantic_value()`.

| Property | `1s` versus `1000ms` |
|---|---|
| `HirDurationValue` Eq/Hash/Ord | different |
| `HirDurationSemanticValue` Eq/Hash/Ord | equal |
| source component | different exact unit spans/spellings |
| diagnostics | retain each normalized authored unit |
| HIR structural/incremental fingerprint | different |
| checked-value/runtime artifact fingerprint | equal |

Structural fingerprint preimage: `arcweft-hir-duration-struct-v1\0`, canonical BigUint bytes, one-byte normalized unit. Semantic fingerprint preimage: `arcweft-duration-value-v1\0`, canonical BigUint bytes only. Neither uses `std::hash::Hash` output.

The checker admits nanoseconds <= `u64::MAX` and returns `CheckedDurationLiteral { nanoseconds:u64 }`. One-over returns `DurationLiteralCheckError::RuntimeRangeOverflow { observed, maximum:u64::MAX }` and publishes no LogicalDuration. `HirDurationIssue::RuntimeRangeOverflow` is deleted.

## Compact numeric sequence

One expression owns ID-less ordered elements, each with arbitrary magnitude/radix. Common suffix is stored once. `MissingFinalElement`, `InvalidElement`, and `ConflictingSuffix` retain the typed sequence variant and poison it. No element receives ExprId.

65,536 elements and 262,144 total digits are inclusive. Exact commits. One-over rolls back the expression, source rows, diagnostics, and staged big integers without prefix truncation.

## Stable bytes

Big integers encode ULEB128 limb count followed by little-endian u32 limbs. Decimals encode canonical coefficient length/bytes, scale u32 LE, exponent i32 LE. Checked floats encode width and exact bits. Checked Duration encodes u64 nanoseconds LE. Authored source text and Rust Hash output are excluded.
