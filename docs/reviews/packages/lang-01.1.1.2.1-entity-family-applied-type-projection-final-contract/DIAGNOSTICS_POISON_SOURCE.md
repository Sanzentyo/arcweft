# Diagnostics, poison, source evidence, and work

## Diagnostic and poison table

| Case | Outer node | Argument node | Diagnostic | Primary | Poison/result |
|---|---|---|---|---|---|
| valid `Ref<Character>` | `Builtin(Ref)` | `EntityFamily(Character)` | none | — | complete, no poison |
| valid `Ref<Flow>` | `Builtin(Ref)` | `EntityFamily(Flow)` | none | — | complete, no poison |
| bare `Ref` | `Failed(WrongArity)` | none | `sema.nominal.wrong_arity`; target `Builtin(Ref)`, expected `Exact(1)`, actual `0` | constructor head `Ref` | one authoritative `NominalTypeDiagnostic` poison |
| `Ref<Character, String>` | `Failed(WrongArity)` | child facts retained | same, actual `2` | constructor head `Ref` | one outer poison unless a child already failed; canonical union |
| `Ref<String>` | `Builtin(Ref)` | replaced with `Failed(WrongArgumentKind)`; recovered `String` retained | `sema.nominal.wrong_kind`; target `Builtin(Ref)`, argument `0`, expected `EntityFamily`, actual `Type(String)` | argument head `String` | one authoritative poison, propagated to outer error |
| `Ref<3>` | `Builtin(Ref)` | replaced with `Failed(WrongArgumentKind)`; const fact retained | same; actual `ConstInt(3)` | argument node/head | one authoritative poison; no unexplained upstream-only poison |
| `Ref<Option<String>>` | `Builtin(Ref)` | generic child root replaced with wrong argument kind; nested `String` fact retained | same; actual `Type(Option<String>)` | argument constructor head `Option` | one authoritative poison |
| project/external/accepted/open ordinary type | `Builtin(Ref)` | successful child replaced with wrong argument kind | same; actual `Type(...)` | argument head | one authoritative poison; no project rename edge |
| unknown child | `Builtin(Ref)` | `Failed(Unknown)` | existing `sema.nominal.unknown_type` only | child head | child authoritative poison reused; no second wrong-kind |
| ambiguous child | `Builtin(Ref)` | `Failed(Ambiguous)` | existing `sema.nominal.ambiguous_type` only | child head, deterministic related candidates | child poison reused |
| inaccessible child | `Builtin(Ref)` | `Failed(Inaccessible)` | existing `sema.nominal.inaccessible_type` only | child head, deterministic related candidates | child poison reused |
| syntax recovery child | `Builtin(Ref)` | `Poisoned(syntax_poison)` | existing syntax diagnostic outside this resolver | child evidence | syntax poison propagated, no duplicate nominal diagnostic |
| detached unknown/project-dependent child | `Builtin(Ref)` | `DetachedUnavailable` | no authoritative accepted-world source diagnostic | child local head | one non-authoritative `DetachedUnavailable` poison; report `Detached` |
| work/limit halt | existing failed/poisoned facts | visited facts retained | existing limit/work code | node head | existing global-halt behavior unchanged |

`NominalTypeDiagnosticKind::WrongArgumentKind` continues to map to
`NominalTypeDiagnosticCode::WrongKind`. No `Ref`-specific diagnostic code is
introduced.

## Exact source ranges

Using zero-based half-open byte ranges:

### `Ref<Character>`

- root whole: `0..14`
- root head and terminal segment: `0..3`
- argument whole/head/terminal: `4..13`

### `Ref<Flow>`

- root whole: `0..9`
- root head and terminal segment: `0..3`
- argument whole/head/terminal: `4..8`

### `Ref<String>`

- root whole: `0..11`
- root head/terminal: `0..3`
- argument whole/head/terminal and wrong-kind primary: `4..10`

Accepted reports must carry the exact `SourceSpan` revision corresponding to
each local range. Detached reports carry local `TextRange` only and never
fabricate a project source identity.

## Poison invariants

1. A locally diagnosed wrong arity/kind allocates exactly one poison with
   `TypePoisonOrigin::NominalTypeDiagnostic` and
   `authoritative_for_annotation == true`.
2. An already-error child’s poison is reused; the outer projection emits no
   duplicate diagnostic or poison.
3. Detached unavailability allocates exactly one
   `TypePoisonOrigin::DetachedUnavailable` record with
   `authoritative_for_annotation == false`.
4. Syntax and upstream poisons retain their existing origin.
5. Cause lists are sorted and deduplicated. Diagnostic deduplication remains
   `(kind, primary)` after deterministic sorting.
6. A successful result cannot contain a poison and a failed result cannot be
   promoted to success by fallback.

## Work invariants

The exact simple-case counts in an empty accepted world are:

| Input | Work |
|---|---:|
| `Ref<Character>` | 2 |
| `Ref<Flow>` | 2 |
| `Ref<String>` | 2 |
| `Ref<3>` | 2 |
| `Ref` | 1 |
| `Ref<Character, String>` | 3 |
| `Ref<ProjectType>` with one selected project record | 3 |

`Ref<E>`, `Speaker<E>`, and `SpeakerPreset<E>` must have identical work for the
same `E`, source maps, world, limits, and cache state. Existing open-rule and
candidate scan charges remain additive and unchanged.
