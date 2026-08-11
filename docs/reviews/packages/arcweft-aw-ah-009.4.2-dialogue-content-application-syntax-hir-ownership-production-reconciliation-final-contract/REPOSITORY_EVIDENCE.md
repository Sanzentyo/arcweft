# Repository evidence

## 1. Inspection identity

```text
repository: Sanzentyo/arcweft (private)
final inspected main: e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
inspection date: 2026-07-20
access: private GitHub connector, exact commit refs
local checkout: unavailable in this artifact runtime
Jujutsu change: UNAVAILABLE_FROM_REMOTE_GIT_SNAPSHOT
```

The connector does not expose the repository-local `.jj` operation store or a
printable change ID for this head. The contract records that exact evidence
state and does not invent a value. It is not an open implementation decision.

## 2. Instruction evidence

- `/mnt/data/Rust Skill.txt` was read completely through its final line before
  the contract was finalized. SHA-256:
  `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`.
- root `AGENTS.md` at final main was read completely. Blob:
  `ea4a46132ff8cd004f860c89c854e4cbfe807d86`.
- project premise file SHA-256:
  `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`.
- request SHA-256:
  `d5949b8ff23a9a3231b7b0f1cf42818655dbc66133e5bdafdefc6a454140e405`.

The latest root instructions require dependency direction, small compiling
increments, private fields/checked construction, source authority, exhaustive
migration, structural quality, and no workaround trait/helper when an
Arcweft-owned enum can be extended directly. This package follows those rules.

## 3. Main movement during inspection

The first observed head was
`73ef0e886a47bc8bcae14f63691642a58235bb4e`. Inspection was refreshed through
`a7872b2e16577c4c547db0c4c715adb7a59d0e1e`, the canonical dialogue-doc commit
`3e7ff141d9c83e7c4579bac21f513ec1d8e7bf10`, and finally
`e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139`.

Changes after the initial head added typed Await/callable-schema work, the
canonical CharacterDialogue authoring document, and an unrelated Stream review
record. The relevant call-surface, postfix heuristic, old dialogue AST, IdRef
carrier, and old HIR sidecar blobs remained the same at the final head. The
final package is pinned to the final head.

## 4. Inspected production owners

| Path at final main | Blob/evidence | Finding used by contract |
|---|---|---|
| `AGENTS.md` | `ea4a46132ff8cd004f860c89c854e4cbfe807d86` | latest repository rules read in full |
| `docs/01-language/character-dialogue.md` | `0fdcff71aef0af047dc58a5183c65163f5fae501` | canonical CharacterDialogue direct bracket/colon application; no `.say`/speaker compatibility |
| `crates/arcweft-lang-syntax/src/expr/call_syntax.rs` | `cf3b3663a44b64ca559d921212b7468fbd0fe351` | implemented AW-AH-009.3.1 sole call surface; preserve |
| `docs/implementation/2026-07-20-aw-ah-009-3-1-call-surface-production.md` | repository implementation note | confirms direct call substrate and no final dialogue adapter |
| `crates/arcweft-lang-syntax/src/parser/expression.rs` | `8fcd3cf4ddfeca98e4937973d645bb77f2ebdfe5` | current dialogue selection uses call/non-ASCII/TextToken shape heuristic; ordinary postfix emits Index directly |
| `crates/arcweft-lang-syntax/src/parser/shadow_recovery.rs` | `274019bc94119500eb68f1d424d894c25498d7d2` | shared missing-token and delimiter boundary mechanics to reuse |
| `crates/arcweft-lang-syntax/src/parser/dialogue.rs` | `e4bd5dce...` | string/source reconstruction and plan/content ownership to replace |
| `crates/arcweft-lang-syntax/src/grammar/kinds.rs` | `c7b489742f41ecbddb70fdb1452ae09741115bfd` | current DialogueCallExpression kind; add/delete in original enum |
| `crates/arcweft-lang-syntax/src/grammar/budget.rs` | `d57936352e6614eb0d0363c7f283cfbd3964bba5` | document transaction budgets; no partial publication |
| `crates/arcweft-lang-syntax/src/expr.rs` | `f976b987eafe4683d57c6c88879428126779f661` | existing Call, EntityRefSyntax carrier, old DialogueCall, Index, and numeric expression limits |
| `crates/arcweft-lang-syntax/src/ast/dialogue.rs` | `95af293...` | retained DialogueContent/RichText/controls coexist with old speaker/string surfaces |
| `crates/arcweft-lang-syntax/src/ast/ids.rs` | `3964e11f4b98354f9f2fbfa284d6445c2e9d297e` | existing IdRef has all three forms; EntityRefSyntax omits unqualified Relative |
| `crates/arcweft-lang-syntax/src/ast/common.rs` | `2cc29c3a15395fc5b6b981f16173c2a2e032a7d8` | TextRange is half-open byte range |
| `crates/arcweft-lang-syntax/src/ast/line_plan.rs` | `af5135a88c3410758b1f0e2a72706581f4ff2ed0` | canonical LinePlan/LinePlanItem to retain and type-project |
| `crates/arcweft-lang-hir/src/model.rs` | `fb4aad1ba6932fc914b3fc755d65a47f4e992a9a` | current Flow-only HirDialogue sidecar and syntax clones to delete |
| `crates/arcweft-lang-hir/src/lower_dialogue.rs` | `d52927d49b74b8489f4e67ed253419e04512e2de` | old sidecar/string lowering owner |
| proof-concurrency v6.1.1 indexed contract | accepted SHA in §5 | final HirDatabase/arenas/SourceKey/SyntheticKey/ScopeId/HirModuleStatus/transaction substrate |
| AW-AH-009.3.2 indexed contract | accepted SHA in §5 | single Arc<HirProject> tooling/request lifecycle |
| `docs/implementation/2026-07-19-aw-ah-009-4-cut1-character-dialogue-domain.md` | repository implementation note | Cut 1 domain/runtime substrate implemented; preserve |

Ellipsized blob IDs above are recorded only where the connector result available
to this package was already path/commit authoritative; exact commit pin and path
are the fetch authority. No result depends on guessing a blob.

## 5. Accepted package evidence

```text
AW-AH-009.4 a86044fea7aaff3ec3829dfa0ad6552c88377ca61fa2911c3b96ea34ca0ffa5e
AW-AH-009.3.1 6ede771a895af981a583fdfd50a080f2eca57bf7a2925216cf725f7dbb418588
proof-concurrency-v6.1.1 1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef
AW-AH-009.3.2 8701ff3ae6024cd62c33c4b36abdfa358bfa30aa93209655870c475eea1dd40d
```

Status/summary/member evidence for proof-concurrency and AW-AH-009.3.2 was
available through the attached-file index, including source-backed allocation,
synthetic roles, slot metadata, recovered module status, scopes, project view,
and lifecycle. This contract does not claim the unavailable predecessor ZIP
bytes were revalidated locally.

## 6. Consumer evidence

Repository code search at the final head identified old-model consumers in:

- syntax parser/AST/source-range/flow/view/line-plan helpers and tests;
- HIR model/lowering/project/ID/dialogue identity and tests;
- sema symbols, semantic traversal/facts, checker expression/line-plan,
  project-index, style token graph, and dialogue resolution tests;
- runtime-plan expression/flow/function-values/line-task/render/fx;
- verifier;
- compiler project/HIR traversal;
- tooling dialogue content/canonicalization/export;
- LSP actions/inlay/cascade/navigation/source lookup;
- CLI bundle/view-mount traversal and test support.

`MIGRATION_AND_DELETION.md` turns this evidence into the direct migration
inventory. Rust exhaustive matches and compile-fail tests, not source-spelling
tests, are the final completeness gate.

## 7. Verification boundary

Verified from current source:

- exact latest head and relevant file contents/blobs;
- landed ordinary-call substrate;
- current heuristic/generic-postfix contradiction;
- current old speaker/string AST and Flow-only HIR sidecar;
- existing three-form IdRef and missing expression carrier;
- existing LinePlan/content/runtime substrate and dependency ownership;
- concrete downstream consumer families.

Normatively fixed by this package:

- all public/private shapes, algorithms, range rules, source roles, HIR
  ownership, scope behavior, limits, recovery, migration order, and 100 tests.

Not claimed until implementation:

- compilation of the proposed final shapes;
- execution of Rust tests/Clippy/format/`just verify`;
- final implementation commit/Jujutsu identity;
- runtime behavior of code not yet implemented.

This is a design-only evidence boundary, not an unresolved contract choice.
