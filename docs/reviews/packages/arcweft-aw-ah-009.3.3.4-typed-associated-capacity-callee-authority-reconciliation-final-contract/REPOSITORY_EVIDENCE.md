# Repository evidence

## 1. Baseline identity

- Requested and inspected Git commit: `5f33ea20fcde7317332c95324701ed4ea7ab813a`.
- Repository: private `Sanzentyo/arcweft`, inspected through the GitHub connector.
- Requested Jujutsu change: `yxvlsqorouqlolxvwtltxltmtqutsxku`.
- Jujutsu verification boundary: the connector exposes Git objects and does not resolve Jujutsu change metadata; the identifier is recorded from dispatch and no Jujutsu-only content is assumed.

## 2. Policy evidence

The complete root `AGENTS.md` at the Git baseline and the complete supplied Rust skill were read before selecting the design. Applied rules include:

- syntax -> HIR -> sema ownership;
- typed identity instead of strings;
- direct final migration without compatibility aliases, dual readers, shims, or source gates;
- behavior added to the owning Arcweft enum/impl rather than an ad hoc helper trait;
- focused tests, workspace check, strict Clippy, workspace tests, Tier 2, and structural audit;
- no unsafe, unstable feature, or new macro shortcut.

The root `AGENTS.md` was the applicable policy file for the inspected syntax, HIR, sema, LSP, tests, and documentation paths.

## 3. Input and package verification

| Input | Calculated SHA-256 | Verification |
|---|---|---|
| request Markdown | `180978fa7154a3907204db797de00ad58f1b96e4c29e34ab938424877a20f947` | local bytes hashed |
| AW-AH-009.3.1 package | `6ede771a895af981a583fdfd50a080f2eca57bf7a2925216cf725f7dbb418588` | outer hash matches dispatch; 9 non-self manifest entries verified |
| AW-AH-009.3.3 package | `9d1f989f5e0e698aeff1098dd7ecee7e01a66616a00a0571ee333a3b1b7ddc78` | outer hash matches dispatch; 10 non-self manifest entries verified |

Each parent manifest uses an all-zero self entry for `MANIFEST.txt`; that self placeholder was excluded and every other member was verified.

## 4. Stable and implementation evidence inspected

- `docs/01-language/standard-types-and-prelude.md`
  - canonical `Vec<String>.with_capacity(8)` and `Bytes.with_capacity(4096)`.
- `docs/01-language/traits-seq-ranges.md`
  - `WithCapacity` semantic surface and reservable owner families.
- `docs/implementation/2026-07-20-aw-ah-009-3-1-call-surface-production.md`
  - one `CallExpr`, exact range ownership, Pratt/path static-generic migration, source-scan deletion.
- `docs/implementation/2026-07-20-aw-ah-009-3-3-callable-catalog-shared-resolver.md`
  - shared resolver/checker/facts/signature production owner.
- `docs/implementation/2026-07-24-aw-ah-009-3-static-capacity-associated-callee-blocker.md`
  - current stringly success branch and missing type-receiver authority.

## 5. Syntax/parser paths inspected

- `crates/arcweft-lang-syntax/src/expr.rs`
  - `Expr`, `DottedPath`, `SelectExpr`, names, and path labels.
- `crates/arcweft-lang-syntax/src/expr/call_syntax.rs`
  - current `CallExpr { callee, args, syntax }`, `ParenthesizedCallSyntax { callee: TextRange, arguments }`, exact range/recovery APIs.
- `crates/arcweft-lang-syntax/src/expr/lexer.rs`
  - current identifier/static-generic lookahead, `parse_type_ref` validation, accepted continuation by `(`, `::`, or `.`, and rollback behavior.
- `crates/arcweft-lang-syntax/src/expr/pratt.rs`
  - Pratt postfix/select/call owner.
- `crates/arcweft-lang-syntax/src/expr/call_syntax_tests.rs`
  - exact call ranges, static generic/turbofish fixtures, comparison rollback, limits.
- `crates/arcweft-lang-syntax/src/types.rs`
  - `TypeRef`, authored type parsing, generic/path structure and limits.
- `crates/arcweft-lang-syntax/src/types/source.rs`
  - `AuthoredTypeRef`, `TypeRefNodePath`, current node/head/terminal map and `try_map`.

Evidence conclusion: the parser already owns the original token transaction and type grammar, but the final expression token currently collapses static generic structure into a label. Extending the existing type source-map owner and parenthesized callee surface is the narrow typed correction.

## 6. Current valid static-generic source evidence

`tests/fixtures/arcw/spec_should_pass/check/051_collections_vec_array_methods.arcw` contains:

```arcw
let xs = Vec<i32>::with_capacity(4usize)
```

Therefore `Vec<T>::with_capacity` is an existing accepted source family and cannot be rejected or replaced by dot-only syntax. The final model preserves that form and also preserves accepted turbofish token grammar. It does not generalize nongeneric `String::with_capacity`.

## 7. HIR/source paths inspected

- `crates/arcweft-lang-hir/src/model.rs`
  - current HIR retains syntax `Expr` values and source documents.
- `crates/arcweft-lang-hir/src/symbol/nominal.rs`
  - `SourceBackedTypeRef::try_bind`, exact document identity, node map mapping.

Evidence conclusion: a parallel HIR call enum or text-keyed side table would duplicate the accepted owner. The typed call surface can be cloned and source-bound through existing HIR infrastructure.

## 8. Nominal/type paths inspected

- `crates/arcweft-lang-sema/src/types.rs`
  - builtin collection/text types, generic parameter, project/accepted/open nominal and poison identities.
- `crates/arcweft-lang-sema/src/types/nominal.rs`
  - `GenericTypeParameterId` and nominal identity carriers.
- `crates/arcweft-lang-sema/src/nominal/input.rs`
  - accepted/detached `TypeResolutionInput` authority.
- `crates/arcweft-lang-sema/src/nominal/model.rs`
  - type name/alias/declaration facts and resolved product.
- `crates/arcweft-lang-sema/src/nominal/resolver/engine/resolution.rs`
  - scoped generic/Self, builtin, project/import/alias, environment/open order and typed failures.
- `crates/arcweft-lang-sema/src/checker/nominal_resolution.rs`
  - checker binding, cache, diagnostics, and accepted/detached resolution.

Evidence conclusion: all required semantic identities already exist. A new type parser or string-to-`TypeKind` helper is unnecessary and prohibited.

## 9. Callable/checker/signature paths inspected

- `crates/arcweft-lang-sema/src/callable/identity.rs`
  - `CapacityMethodId { receiver, method, arity }` and existing constructor/accessors.
- `crates/arcweft-lang-sema/src/callable/schema/families.rs`
  - family-owned schema constructors. At the baseline, `CapacityMethodId::signature_schema` calls `homogeneous` with `Named("_")`; this conflicts with the request/accepted 3.3 unchecked behavior and is treated as implementation drift to be corrected in the existing owner.
- `crates/arcweft-lang-sema/src/callable/resolver.rs`
  - current value-only `CallCallee::Selected`, registered-world request fields, selected precedence, candidate/instantiation products, work/cancellation controls.
- `crates/arcweft-lang-sema/src/checker/expr/registered_call.rs`
  - shared resolver/checker connection.
- `crates/arcweft-lang-sema/src/checker/expr/registered_call/selection.rs`
  - candidate transaction and argument replay.
- `crates/arcweft-lang-sema/src/checker/expr.rs`
  - early static-capacity success before shared resolver.
- `crates/arcweft-lang-sema/src/checker/helpers.rs`
  - `well_known_static_capacity_method_type(&str)`, `Vec<...>` text slicing, bare-Vec `_` placeholder, and current path-label input.
- `crates/arcweft-lang-sema/src/checker/call_target_facts.rs`
  - checker-owned target facts.
- sema signature modules and LSP bridge
  - native semantic projection and transport boundary.

Evidence conclusion: the current helper loses generic identity and bypasses resolver facts. The correct fix is an associated `CallCallee` variant plus behavior on `CapacityMethodId`, not another helper.

## 10. Parent-package evidence

AW-AH-009.3.1 requires:

- one `Expr::Call(CallExpr)`;
- parser-owned exact ranges;
- Pratt/path static generic/turbofish parsing;
- deletion of source scanning;
- HIR clone rather than a parallel call enum.

AW-AH-009.3.3 requires:

- one shared resolver;
- capacity owned by `CapacityMethodId` and the existing capacity family;
- exact receiver/method/arity identity and result equal to receiver;
- intentionally unchecked capacity arguments;
- environment before capacity, capacity before trait/data-last for selected methods;
- transactional facts/work/signature projection;
- no string parsed into identity.

This correction supplies only the missing typed bridge and refines the two shapes that assumed a text-only callee and value-only receiver.

## 11. Verification boundary of this archive

Verified directly:

- request bytes and digest;
- parent package outer digests and non-self manifest members;
- exact Git tree files and policies listed above;
- current valid static-generic fixture;
- archive required member set, UTF-8/LF formatting, hashes, lengths, `OPEN_QUESTIONS.md`, status, ZIP integrity, and sidecar digest after generation.

Not performed because production changes were explicitly prohibited:

- compilation of the proposed Rust APIs;
- modification or execution of proposed tests against a changed checkout;
- workspace/Tier 2 validation against an implementation;
- native `jj` resolution of the supplied change ID.

These are implementation validation steps. They do not leave a design decision open.
