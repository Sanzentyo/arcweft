# Traceability

Baseline: `Sanzentyo/arcweft main@4fd6331dc342d30a7f4ac7774852b60801866ef7`.

## Required decisions

| Request decision | Final contract resolution | Primary implementation sections | Test evidence |
|---|---|---|---|
| 1. select one ownership/projection model | Explicit owner + exact path + arguments; Rust export owner is always `RustPackage`, adapter-native is explicit `Environment`; no contextual guessing | `FINAL-CONTRACT.md` §§3–5; `API-SHAPES.md` §§1–6 | ABI-004, ADP-001–010, NOM-001–005, CALL-024–027 |
| 2. exact Rust/API types | Newtyped package/path/parameter identities; recursive Rust and adapter type carriers; exact method receiver and nested node types | `API-SHAPES.md` §§1–7 | ABI-001–018, ADP-001–024, CALL-001–009 |
| 3. owner mapping/collisions | Rust package owner; adapter selected provider only; package-local path + typed adapter mount; global exact full-path collision | `FINAL-CONTRACT.md` §§3–4 | ADP-004–009, NOM-013–016, CALL-011–012, REG-018–019 |
| 4. projection context/order | Source-backed input → accepted inventory → one `AcceptedNominalWorld` → project schemas → metadata → environment callables → atomic commit | `CONSTRUCTION-ORDER.md` §§2–8 | REG-001–020, CALL-017–021, META-011 |
| 5. structured failures | Typed item/site/source diagnostics; exact unknown/inaccessible/owner/arity/limit/stamp distinctions; deterministic bounded report | `ERROR-ROLLBACK.md` §§1–9 | NOM-003–027, ERR-001–010, REG-011–013 |
| 6. context-free conversion | `AdapterTypeKind::to_sema_type_kind()` deleted; no public `TypeKind` conversion | `FINAL-CONTRACT.md` §7; `API-SHAPES.md` §9 | ADP-023, CUT-001–003, ERR-010 |
| 7. schema/tooling/persistence | Exact semantic digests; existing candidate/receiver IDs receive accepted types; typed signature/hover; existing environment digest key populated | `SCHEMA-TOOLING-PERSISTENCE.md` §§1–16 | DIG-001–015, PERSIST-001–010, TOOL-001–015 |
| 8. non-callable metadata | Same atomic cut; immutable accepted Rust metadata catalog keyed by accepted ID; typed generic substitution; no Rust `Named` keys | `FINAL-CONTRACT.md` §8; `API-SHAPES.md` §7 | META-001–018, CUT-004–005 |

## Required implementation order

| Request order | Prescribed cut |
|---|---|
| correct manifest carriers/errors | `IMPLEMENTATION-MAP.md` Cuts 1–3 |
| add accepted-world projection context | Cuts 4–6 |
| project receivers/groups/results/nested args | Cut 6 and `CONSTRUCTION-ORDER.md` Phase 7 |
| migrate non-callable metadata | Cut 5 and Phase 6 |
| delete context-free conversion/string behavior | Cuts 2, 6, 10 |
| update registration, queries, tooling, persistence, tests | Cuts 7–10 |

## Required tests

| Request test | Contract test IDs |
|---|---|
| Rust nominal free-function parameter and result | CALL-001, CALL-010 |
| same nominal as method receiver and later curried group | CALL-002, CALL-003, CALL-029 |
| nested Option, Result, tuple, sequence/vector, generic arguments | CALL-004–009 |
| exact equality with authored extern | NOM-010, CALL-010 |
| same terminal name from two Rust packages and project declaration | NOM-014, CALL-011, CALL-012, DIG-002 |
| unknown path | NOM-004, CALL-013, META-010 |
| inaccessible export | NOM-005, CALL-014, ADP-018, TOOL-010 |
| malformed path | ABI-003, NOM-021 |
| wrong arity | NOM-006, CALL-015, META-014 |
| over-limit path/type/arguments/work | ABI-003, ABI-008–010, NOM-015, NOM-022–027 |
| package/adapter owner mismatch | ADP-008, ADP-010, NOM-003, META-009, CALL-027 |
| duplicate accepted export | NOM-013, REG-006, REG-012, ERR-004 |
| deterministic publication order | CALL-020, REG-005, REG-017 |
| deterministic digest | ADP-020, META-012, DIG-001–015 |
| deterministic structured error | REG-011–013, ERR-001–009 |
| atomic rollback on failed nested type | CALL-013, CALL-017, META-010–011, REG-006–010 |
| signature help preserves accepted ID | TOOL-001–004 |
| hover preserves accepted ID | TOOL-005–007, TOOL-015 |
| method lookup preserves accepted ID | TOOL-008–009 |
| overload resolution preserves accepted ID | CALL-011–012, TOOL-004 |
| detached/incomplete fail closed | NOM-019–020, ERR-010 |
| manifest carrier round trips | ABI-001, ABI-004–005, ABI-011–012, ADP-001–002, ADP-011–013, CUT-005 |
| persistent carrier round trips | PERSIST-005–006 |
| non-callable metadata | META-001–018 |

## Constraints

| Request constraint | Contract enforcement |
|---|---|
| preserve syntax → HIR → sema → runtime-plan/verify → tooling | `CONSTRUCTION-ORDER.md` §1 and `NON-GOALS.md` §1 |
| reuse accepted catalog/world/owner/package/path/checked schema | `FINAL-CONTRACT.md` §§3–6 and `API-SHAPES.md` §§4–6 |
| no `From<&TypeRef> for TypeKind` | `FINAL-CONTRACT.md` §7; `NON-GOALS.md` §3 |
| no `Named` semantic identity | `FINAL-CONTRACT.md` §§7–8; `NON-GOALS.md` §3 |
| no suffix/terminal equality | `FINAL-CONTRACT.md` §4; `NON-GOALS.md` §2 |
| no dual readers/aliases/shims/version bump | `FINAL-CONTRACT.md` §12; `NON-GOALS.md` §4 |
| no resolver/query/poison redesign | `FINAL-CONTRACT.md` §6; `IMPLEMENTATION-MAP.md` Cut 4 |
| minimum registration transaction change | `CONSTRUCTION-ORDER.md` §§3–5; `ERROR-ROLLBACK.md` §7 |
| Rust ABI/data carriers Sans I/O | `API-SHAPES.md` §§1, 10 |
| typed API tests, no source scans/gates | `NON-GOALS.md` §8; all rows of `TEST-MATRIX.csv` |

## Expected output components

| Expected component | Package file |
|---|---|
| selected owner model | `FINAL-CONTRACT.md` |
| exact Rust/API shapes | `API-SHAPES.md` |
| construction/dependency order | `CONSTRUCTION-ORDER.md` |
| structured error and rollback table | `ERROR-ROLLBACK.md` |
| schema/digest consequences | `SCHEMA-TOOLING-PERSISTENCE.md` |
| exhaustive test matrix | `TEST-MATRIX.csv` |
| traceability | this file |
| non-goals | `NON-GOALS.md` |
| commands actually run | `COMMANDS-RUN.md` |
| repository-aware validation | `REPOSITORY-VALIDATION.md` |
| machine-readable decisions | `contract/DECISIONS.json` |
| artifact integrity | `MANIFEST.sha256`, sibling ZIP SHA-256 |
