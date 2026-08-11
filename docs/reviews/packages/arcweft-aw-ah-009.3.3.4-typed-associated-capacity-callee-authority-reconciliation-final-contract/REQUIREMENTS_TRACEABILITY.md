# Requirements traceability

## 1. Dispatch requirements

| Request requirement | Closed by |
|---|---|
| exact lossless syntax/HIR representation for String, Bytes, bare Vec, generic Vec, qualified, alias, generic parameter, turbofish | `FINAL_CORRECTION.md` sections 3–5; `TYPE_RECEIVER_MODEL.md` sections 2–4; S01–S20, H01–H06 |
| owner of every delimiter, segment, range, generic argument, and resolved identity | `FINAL_CORRECTION.md` sections 2–3; `TYPE_RECEIVER_MODEL.md` sections 1–4 |
| explicit value-selected/type-associated distinction without sentinel/string | `FINAL_CORRECTION.md` sections 6–8; `TYPE_RECEIVER_MODEL.md` sections 8–9; X01–X06 |
| final `CallCallee` shape and owner layer | `FINAL_CORRECTION.md` section 8; `RESOLVER_INTEGRATION.md` section 3 |
| authored type syntax -> `TypeKind` preserving nominal/alias/module/generic identity | `FINAL_CORRECTION.md` sections 5–7; T01–T13 |
| bare Vec, Vec<T>, unresolved/ambiguous types without `_` or display parsing | `TYPE_RECEIVER_MODEL.md` sections 5–10; T04, T08–T13; C17 |
| single resolver entry for static capacity | `RESOLVER_INTEGRATION.md` sections 1–6 |
| preserve CapacityMethod ID/candidate/family/result/arity/unchecked behavior/work | `FINAL_CORRECTION.md` section 9; `RESOLVER_INTEGRATION.md` sections 6–7, 13; C01–C17, W01–W06 |
| name and collision precedence | `FINAL_CORRECTION.md` sections 6, 10; `RESOLVER_INTEGRATION.md` sections 4–9; X01–X15 |
| lexical values, type names, imports/qualified/aliases, environment, traits, data-last, near miss | X01–X15 and the referenced normative sections |
| malformed generic, missing member, unknown/ambiguous type, invalid member, value call recovery | `FINAL_CORRECTION.md` section 11; R01–R10 |
| each retained authored argument checked exactly once | `RESOLVER_INTEGRATION.md` sections 10, 13–14; C08–C13, R01–R10, W01–W06 |
| registered/non-registered convergence | `FINAL_CORRECTION.md` section 12; `RESOLVER_INTEGRATION.md` sections 2, 12; P01–P07, W06 |
| compiling authority switch and same-switch old-reader deletion | `FINAL_CORRECTION.md` section 13; D01–D12 |
| exact normative precedence over 3.1 and 3.3 | this file sections 2–3; README precedence section |
| public checker/native signature primary equality | P03–P05 |
| exact counters: one registration, one resolver, zero old dispatch, one check per expression | W01–W06 |
| compiling deletion test through typed behavior rather than helper/source scan | D01, D05, D06; D12 explicitly excludes source scans as acceptance tests |
| READY output has `OPEN_QUESTIONS.md` exactly `none` | archive member and `FINAL_STATUS.md` |

## 2. AW-AH-009.3.1 corrections and preserved rows

| Parent 3.1 row/decision | This correction |
|---|---|
| one semantic `Expr::Call(CallExpr)` | preserved exactly |
| `CallExpr { callee, args, syntax }` | preserved exactly |
| exhaustive `CallSurfaceSyntax` | preserved exactly |
| `ParenthesizedCallSyntax { callee: TextRange, arguments }` | `callee` alone is refined to exhaustive `ParenthesizedCalleeSyntax`; `callee_range()` and `range()` results are preserved |
| only parenthesized surface owns `ArgumentListSyntax` | preserved |
| parser-only construction and private fields | preserved |
| no optional argument-list field on `CallExpr` | preserved |
| exact call/callee/argument/recovery ranges | preserved and extended with typed receiver/member lexemes |
| static generic/turbofish parsed by Pratt/path grammar | completed by structural receiver/lexeme retention; current `Vec<T>::with_capacity` fixture preserved |
| no `parse_static_generic_call` source scan | preserved and strengthened: no post-AST type/callee reparse |
| HIR clones syntax Expr without parallel call enum | preserved |
| semantic consumers use `call.callee()`/`call.args()` | preserved; only checker call-target preparation reads the typed parenthesized callee surface |
| no generated/source-less syntax call constructor | preserved |
| limits and recovery | preserved; added lexeme-map exact/one-over rows |

Normative precedence: where the parent shows `ParenthesizedCallSyntax::callee: TextRange`, this correction's enum field is authoritative. All parent range accessors and ordinary-call behavior remain authoritative.

## 3. AW-AH-009.3.3 corrections and preserved rows

| Parent 3.3 row/decision | This correction |
|---|---|
| one `resolve_call_target` | preserved |
| `CallCallee::Selected` has a value receiver expression | preserved for values; type receivers use new `AssociatedType` variant |
| 23-family inventory | preserved; no new family |
| `CapacityMethodId { receiver, method, arity }` | preserved; inherent associated selector added |
| `CallableCandidateId::CapacityMethod` / capacity family | preserved |
| capacity result equals receiver | preserved exactly |
| capacity arguments are intentionally unchecked | preserved through normative `variadic_unchecked` schema |
| current baseline homogeneous `_` capacity schema | identified as implementation drift, not parent authority; replaced in the existing owner with accepted unchecked schema |
| selected value precedence | preserved unchanged for `CallCallee::Selected` |
| environment before capacity, capacity before trait/data-last | preserved for associated types as typed environment > capacity > trait; data-last is structurally ineligible without a value |
| typed trait ambiguity terminal | preserved |
| data-last requires receiver injection | preserved; `TypeReceiver` cannot inject |
| normalized untyped environment fallback after data-last | preserved for values; ineligible for type-associated requests |
| transactional candidate probing and selected replay | preserved |
| checker-owned target facts and native signature projection | preserved |
| work/cancellation/limits/cache policies | preserved |
| no label/source/Rust display string parsed into identity | preserved and made true for static capacity |

Normative precedence: any parent wording that assumes every receiver is represented by `receiver_expression: TypeExpressionId` is corrected by `CallCallee::AssociatedType`. The accepted capacity family, candidate, ID, result, and unchecked argument decisions remain unchanged.

## 4. Stable-language reconciliation

| Stable surface | Design result |
|---|---|
| `Vec<String>.with_capacity(8)` | canonical dot-member, typed generic receiver |
| `Bytes.with_capacity(4096)` | canonical dot-member, typed builtin receiver |
| `WithCapacity::with_capacity(capacity: usize) -> Self` semantic intent | result remains exact receiver; this focused correction does not tighten accepted 3.3 unchecked argument validation |
| current valid `Vec<i32>::with_capacity(4usize)` fixture | preserved by explicit-generic terminal path separator |

The stable single-parameter trait description and the accepted 3.3 unchecked checker schema are not reconciled by changing argument validation here; the request explicitly forbids changing unchecked semantics to manufacture rejection rows.

## 5. Non-goals mapped

| Non-goal | Closure |
|---|---|
| no builtin ID / 24th family | existing Capacity ID/family only; D03 |
| no display-string parsing | typed syntax/type product/request only; D01 |
| no compatibility/dual reader/shim/source gate | direct switch; D12 |
| no `_` compatibility placeholder | bare Vec typed failure; C07, C17, D06 |
| no superseded Dialogue carriers | no Dialogue changes |
| no argument semantic tightening | `variadic_unchecked`; C08–C13 |
| no broad redesign of ordinary call/cache/source identity | `CallExpr` and call surface preserved except focused callee refinement |

## 6. Completion mapping

All request decisions are closed and all test rows are assigned. No requirement maps to `OPEN_QUESTIONS.md`; that file is exactly `none`.
