# AW-AH-009.4.2 final production-reconciliation contract

## Status

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
PRODUCTION_CHANGES_INCLUDED=0
```

This archive is the independently throwable, implementation-ready contract for
Arcweft dialogue-content application syntax and typed-HIR ownership. It fixes
one exact production end state for bracket application, colon application,
postfix index/dialogue ambiguity, ordinary-call composition, typed `IdRef`
coordinates, parser recovery, source mapping, limits, direct migration, and the
required tests.

The inspected repository revision is:

```text
Sanzentyo/arcweft
main e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
Jujutsu change UNAVAILABLE_FROM_REMOTE_GIT_SNAPSHOT
```

The latest commit is a documentation-only consolidation of an unrelated
Stream source-elimination review. Production parser, syntax AST, HIR, and the
canonical `CharacterDialogue` authoring document relevant to this contract are
unchanged from the immediately preceding revision. The contract nevertheless
records and targets the latest commit above.

## Normative result

The final source model has exactly these properties:

1. an expression-start `[` remains a collection literal;
2. every `[` following a completed expression first becomes one lossless
   `PostfixBracketExpression` CST node;
3. typed syntax attempts exactly one ordinary-expression interpretation and
   exactly one existing `DialogueContent` interpretation, without name, type,
   Character, `.say`, callable, or source-string heuristics;
4. one viable interpretation becomes `Expr::Index` or
   `Expr::DialogueContentApplication`; two viable interpretations become one
   bounded `Expr::PostfixBracket`; no viable interpretation becomes the same
   generic node with two typed failure summaries;
5. colon syntax directly creates `Expr::DialogueContentApplication` with the
   exact byte-based `DialogueIndentation` carrier defined here;
6. bracket and colon applications lower to one
   `HirExprKind::DialogueContentApplication` meaning;
7. unresolved bracket ambiguity lowers to `HirExprKind::PostfixBracket`, whose
   candidate roots and children are ordinary `ExprId` values in the accepted
   proof-HIR expression arena;
8. the existing AW-AH-009.3.1 `CallExpr`, `CallSurfaceSyntax`, and
   `ArgumentListSyntax` remain the sole ordinary-call authority;
9. expression entity references directly carry the existing `IdRef`, so
   absolute, relative, and family-relative values survive without source
   reconstruction;
10. old speaker/string/dialogue-call surfaces are deleted in one direct public
    replacement series.

## Archive map

- `FINAL_CONTRACT.md` is the precedence and decision ledger.
- `PRODUCTION_RECONCILIATION.md` states what current production keeps and what
  it replaces.
- `COLON_INDENTATION_MODEL.md` fixes the complete indentation carrier.
- `POSTFIX_BRACKET_CST_AST.md` fixes CST ownership, candidate classification,
  AST shapes, and plan attachment.
- `TYPED_HIR_OWNERSHIP.md` fixes arena ownership, source component mapping,
  poison, scopes, and executable gating.
- `CALL_AND_IDREF_INTEGRATION.md` fixes ordinary-call and coordinate behavior.
- `RECOVERY_AND_LIMITS.md` fixes every requested recovery case, budget, and
  transaction boundary.
- `MIGRATION_AND_DELETION.md` inventories direct consumers and deletion gates.
- `IMPLEMENTATION_HANDOFF.md` fixes compiling cuts and validation order.
- `TEST_MATRIX.md` contains 100 exact required test names and outcomes.
- `REQUIREMENTS_TRACEABILITY.md` maps every request obligation to decisions and
  tests.
- `REPOSITORY_EVIDENCE.md` records the latest-main inspection and evidence
  boundary.
- `OPEN_QUESTIONS.md` is exactly `none`.
- `FINAL_STATUS.md` records readiness and verification scope.
- `MANIFEST.txt` is the sorted member manifest. Its self-entry uses 64 zeroes;
  all other entries are exact SHA-256 values.

## Dispatch rule

Send this archive unchanged to one implementation assignee together with the
four predecessor packages named in the request. The assignee must implement the
public syntax/HIR replacement through all downstream consumers as one unmerged,
workspace-compiling series. No production patch, checkout, generated output,
or implementation is included here.

## Verification boundary

Current production was inspected read-only through the private GitHub
connector at the exact commit above. Artifact construction verifies member
names, UTF-8/LF encoding, sorted manifest entries, non-self hashes, ZIP order,
CRC/decompression, clean extraction equality, deterministic rebuild equality,
and the exact `OPEN_QUESTIONS.md` content. No Rust production command is
claimed because production changes were prohibited.
