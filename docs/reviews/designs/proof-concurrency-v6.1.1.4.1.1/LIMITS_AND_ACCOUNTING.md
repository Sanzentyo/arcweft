# Exact limits and accounting phases

## Single HIR owner

The final implementation extends `arcweft_lang_hir::identity::HirLimit` and its inherent `maximum()` method. It does not create `HirLeafLimits`, a free-standing limit helper, or per-consumer constants.

| Limit owner | Inclusive maximum | Observed/limit type | Charge phase | Exact boundary | One-over outcome |
|---|---:|---|---|---|---|
| `HirLimit::SourceDocumentBytes` | 8,388,608 | `usize` after checked `u64` conversion | HIR document preflight before transaction | lowerer may begin | `HirLowerError::Limit`; no transaction/public ID |
| `HirLimit::DecodedStringBytes` | 8,388,608 | `usize` | after typed decode, before `Box<str>` allocation | decoded value staged | full owner rollback |
| `HirLimit::NameBytes` | 1,024 | `usize` | typed name/segment construction | name staged | owner rollback; no partial path |
| `HirLimit::PathSegments` | 256 | `usize` | path preflight | complete path staged | no prefix path/source rows |
| `HirLimit::PathSemanticBytes` | 65,536 | `usize` checked sum | path preflight | complete path staged | checked-add/one-over limit error |
| `HirLimit::RegistrySegments` | 256 | `usize` | registry-path preflight | complete path staged | no prefix registry key |
| `HirLimit::RegistrySemanticBytes` | 65,536 | `usize` checked sum | registry-path preflight | complete path staged | checked-add/one-over limit error |
| `HirLimit::NumericDigitsPerLiteral` | 65,536 | `usize` | before BigUint allocation | exact BigUint staged | no HIR literal/limbs |
| `HirLimit::DecimalCoefficientDigits` | 65,536 | `usize` | before decimal allocation | exact decimal staged | no partial decimal |
| `HirLimit::DecimalScale` | 65,536 | `usize` | canonicalization preflight | exact decimal staged | full owner rollback |
| `HirLimit::DecimalExponentAbs` | 1,000,000 | `usize` after checked absolute conversion | canonicalization preflight | exact decimal staged | full owner rollback |
| `HirLimit::NumericSequenceElements` | 65,536 | `usize` | sequence preflight | complete element slice staged | no prefix sequence |
| `HirLimit::NumericSequenceTotalDigits` | 262,144 | `usize` checked sum | sequence preflight | complete sequence staged | no prefix/partial limbs |
| `HirLimit::ThreadFlowItems` | 65,536 | `usize` | Thread body preflight | full ordered body staged | no partial body/scope |

`SourceDocumentBytes` deliberately matches the repository-owned `MAX_REGISTRATION_SOURCE_BYTES` (`u64` 8,388,608). `SourceDocument::try_new` remains a general revision-identity constructor; the final HIR lowerer applies the production HIR bound uniformly to accepted and local documents before allocation.

`PathSemanticBytes` sums semantic segment UTF-8 bytes only. Identifier segments are not Unicode-normalized or case-folded, so their semantic bytes are their parser-validated UTF-8 bytes; external-capable segments likewise retain their exact code points. Root/separator spelling remains in source components and is excluded. `RegistrySemanticBytes` sums a named scope, when present, plus key segment UTF-8 bytes; builtin scope names add zero semantic payload bytes.

`NumericDigitsPerLiteral` counts radix-valid digit characters after removing the radix prefix, `_` separators, and the typed suffix; hexadecimal `a`-`f`/`A`-`F` each count as one digit. `DecimalCoefficientDigits` counts the canonical decimal coefficient after leading/trailing-zero normalization, while `DecimalScale` is preflighted against the authored fractional digit count (excluding `_`) before canonical reduction so an enormous zero tail cannot bypass the limit. `DecimalExponentAbs` is the absolute mathematical authored exponent after sign parsing; an exponent that cannot be converted with checked arithmetic reports the same limit with `observed = usize::MAX`. `NumericSequenceTotalDigits` is the checked sum of each present element's `NumericDigitsPerLiteral` count.

`SourceDocumentBytes` counts exact UTF-8 source bytes. `DecodedStringBytes` counts UTF-8 bytes after typed escape decoding. The two limits are charged independently even though decoding normally does not increase this grammar's source length.

## Failure ownership

```rust
pub struct HirLimitError {
    limit: HirLimit,
    observed: usize,
    maximum: usize,
}
```

All count conversion and addition is checked. Conversion/addition overflow is reported as the same limit with `observed = usize::MAX`; the transaction aborts. A hard limit never becomes `HirPoisonState`, `HirStringIssue`, `HirIntegerIssue`, `HirDecimalIssue`, or `HirDurationIssue`.

## Interaction with other budgets

- Existing slot/scopes/diagnostic `HirLimit` variants are also charged; all must pass.
- Ordinary call arguments remain 128, nested calls 32, recovery nodes 256, diagnostics 128, candidates/results at most 2 under the accepted callable owner.
- RichText retains 4,096 tags, 32,768 arguments/content, 16,384 body bytes, 32 arguments/tag or inline call, 64 key bytes, and 4,096 encoded/decoded value bytes under its accepted syntax/checker owners.
- Limits are conjunctive, never merged. The first deterministic failed preflight in normative phase order is reported; no earlier staged data is published.
- Decoder limits revalidate persisted checked values where an actual codec exists; authoring defaults are never applied to malformed/over-limit data.

## Exact/one-over proof

Each limit has two direct tests. The exact case commits all expected IDs/source rows. The one-over case asserts the returned `HirLimitError`, `NotPublished` rollback receipt, unchanged arena lengths, unchanged source-index length, no diagnostic/candidate/checked-value entry, and no retained allocation reachable from a public ID.
