# Literal and numeric contract

## 1. Direct typed lowering

The only input is the canonical attached typed literal owner produced by syntax. Lowering consumes decoded characters/strings, typed radix/suffix/unit enums, canonical digit tokens, and typed recovery issues. It must not call `raw_text`, slice the source document, scan Rowan tokens, split a display label, or run another lexer.

## 2. String and character

Escaped and raw string syntax both lower to `HirStringLiteral::Value(decoded_utf8)`. Delimiter spelling, quote count, and escape spelling are source components only. Invalid escape and unterminated forms lower to the String family in poisoned state; they do not fabricate an empty value. Character syntax must decode to exactly one Unicode scalar. Empty, multiple-scalar, invalid-escape, or unterminated forms remain poisoned Character literals.

## 3. Integer

The literal magnitude is non-negative. A leading minus is always `HirUnaryOp::Negate` over the literal. Syntax separators and prefixes are validated before HIR; the HIR magnitude is `HirBigUint` in little-endian base-2^32 limbs. This representation retains `u128::MAX + 1` and arbitrary larger values exactly up to the resource digit limit.

The checker selects a type in this order: explicit suffix; exact contextual integer type; otherwise i32. It compares the arbitrary magnitude to the selected range. Unary minus is checked as a combined operation and admits exactly one extra positive magnitude for the selected signed minimum. An out-of-range value is a typed checker error and no runtime constant is published.

Structural HIR equality includes magnitude, radix, and explicit suffix. `integer_value_eq` compares magnitude plus selected type and ignores radix.

## 4. Canonical decimal

Parse coefficient digits, decimal point, and exponent from typed syntax components. Reject missing/invalid components as typed family recovery. Then:

1. remove coefficient leading zeros;
2. if all digits are zero, canonicalize to digits `[0]`, scale 0, exponent 0;
3. set scale to the count of digits originally following the decimal point;
4. remove trailing coefficient zeros while scale is positive, decrementing scale;
5. remove any remaining trailing coefficient zeros and add their count to exponent10;
6. add the authored exponent to exponent10 using checked arithmetic;
7. enforce coefficient <= 65,536 digits, scale <= 65,536, and `abs(exponent10) <= 1,000,000`.

The value is `coefficient × 10^(exponent10 - scale)`. These rules make `100`, `1e2`, and `1.00e2` canonical to the same decimal value record.

## 5. Float

Width selection is deterministic: explicit `f32`/`f64` suffix; otherwise an exact contextual f32/f64 expectation; otherwise f64. The checker converts the canonical decimal to IEEE-754 round-to-nearest, ties-to-even and records exact `to_bits()` output in `CheckedFloatLiteral`. Finite subnormal values and signed zero are accepted. A literal that rounds to infinity is rejected as `WidthOverflow`. NaN and infinity are not literal spellings; standard constants are resolved as paths. Unary minus is applied to the checked bits, preserving negative zero.

## 6. Unit number

The canonical decimal is paired with exactly one unit enum. Percent is Ratio; px/pt/em/rem/vw/vh are Length; deg/rad/turn are Angle; db/lufs are Audio; bpm/bars are Music. Unit spelling aliases are normalized by typed syntax. No unit-number payload is later re-parsed or guessed from a suffix string.

## 7. Duration

Duration is not a UnitNumber. Its HIR type identity is always `Duration`. The source amount is a non-negative canonical decimal paired with ns/us/ms/s/min/h. Lowering multiplies the exact decimal by the unit's nanosecond factor (1, 1,000, 1,000,000, 1,000,000,000, 60,000,000,000, or 3,600,000,000,000). A valid HIR Duration contains an arbitrary-precision whole-nanosecond magnitude. A fractional nanosecond is poisoned `FractionalNanosecond`; no truncation or rounding occurs. The checker requires the magnitude <= `u64::MAX`, matching `arcweft_core::time::LogicalDuration`; one-over is `RuntimeRangeOverflow`. Equality and ordering compare whole nanoseconds; authored unit is retained only for typed diagnostics/fingerprint and does not change value equality.

## 8. Compact numeric sequence

One `HirNumericSequence` owns one expression identity and ordered ID-less elements. Every element has an arbitrary-precision magnitude and radix. All explicit suffixes must agree; a common suffix is stored once. An absent final element after a separator is `MissingFinalElement { ordinal }` on the same typed variant and poisons the expression. A malformed element is `InvalidElement { ordinal, issue }`; a suffix disagreement is `ConflictingSuffix { ordinal, first, conflicting }`. Valid prefix elements remain ordered, but no missing/invalid element is fabricated and no element receives an ExprId.

Limits: 65,536 elements and 262,144 total digits. Exact commits. Either one-over rolls back the complete expression, its component source rows, diagnostic, and any BigUint allocation. No prefix truncation is permitted.

## 9. Deterministic bytes

Big integers encode limb count as ULEB128 then little-endian u32 limbs. Decimals encode coefficient digit count and bytes, scale u32 LE, exponent i32 LE. Float checked values encode width then exact bits. Duration checked values encode canonical u64 nanoseconds LE. The codec never embeds authored text or `Hash` output.
