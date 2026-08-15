# Accepted semantic-fact/provenance retry return invalid

Date: 2026-08-15

Continues:
`docs/implementation/2026-08-14-accepted-semantic-fact-provenance-return-invalid.md`

Inspected Git baseline:
`35d42efdd89fef8fde73f62be2a3e38fd5e81e52` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake.

## Returned archive intake

The downloaded archive ended in `(1).zip`. The canonical repository archive
and extracted directory already contain the previous invalid return, so this
retry uses `_1` rather than retaining parentheses or overwriting evidence:

`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-and-compile-clean-admission-order-correction-final-contract_1.zip`

SHA-256:
`6db356fb978ef5f4afd4903fe109417a6a6f77a2925b5483ee1a73d4c6dbf19a`

The 86,753-byte archive has one redundant top-level wrapper, stripped only in
the extracted mirror. It contains 37 files and has no unsafe/rooted/drive/
traversal path, symlink/reparse entry, or case-insensitive collision. All 37
extracted files match their ZIP members and all internal
`MANIFEST.sha256` rows pass.

The package includes the exact maintained source request and previous-invalid
note. Their SHA-256 values match the repository files at intake:

- request: `1b54121c38f7f957f9c168a02d25fef26ba21e7f50da9fc89e4b390ac9281c65`;
- previous invalid note:
  `e17112bc1e6a6ce5611e1131448a8cec4efb647cfdabacfc042232d48dc15dc9`.

The package reports `READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`, current
Git SHA `35d42efdd89fef8fde73f62be2a3e38fd5e81e52`, and every Arcweft-owned
version fixed at `1`. It contains no production patch.

## Readiness adjudication

Full-package/current-source inspection and an independent Sol max audit
classify this retry as `INVALID_AS_DELIVERED`. The problems are resolvable
within the requested repository design and are not external missing
authorities, so `NOT_READY` is not appropriate.

1. `SYN-023` through `SYN-061` require exact normalized types for Agent
   record, tuple, and field scaffolds, but the named accepted sources are not
   representable. Current compiler projection rejects `ActionTarget`,
   `CaptureTarget`, `Probe`, `Predicate`, `AgentValue`, and related Agent
   families. `RuntimeTypeShape` has no anonymous-record or Agent shape and the
   checked/operational plan kind has no corresponding family. Current accepted
   facts intentionally classify the Agent call carrier as non-value. The
   package adds neither the missing type algebra nor another legitimate
   semantic owner, so its synthetic rows cannot be constructed.
2. The proposed `RuntimeTypedExpr` only retains expression-node type rows.
   Current expressions such as if-let and match contain patterns whose types
   and binding coordinates must remain attached recursively. The returned
   API neither replaces those fields with `RuntimeTypedPattern` nor defines a
   single recursive typed carrier, leaving raw and typed authorities split.
3. The implementation order is not compile-clean. P01 privatizes
   `RuntimePlan` and removes `Default`/derived decode, while external
   runtime-plan/compiler/CLI/player/host/driver consumers migrate only in P04
   or are absent from the inventory. P08 similarly privatizes
   `AwbcProgram` before its lowerer, bundle, verifier, VM, driver, and fixtures
   migrate. A core-only focused test cannot establish a workspace-clean cut.
4. The claimed RuntimePlan custom version-1 decoder has no exact codec or
   Serde grammar, limits, decoder API, error precedence, or canonical bytes.
   The package entirely omits current `BytecodeProgram` RuntimePlan
   conversion/round-trip APIs, which would either stop compiling or discard
   the new local/type/site tables.
5. P14 makes JIT, runtime-codegen, and accelerator depend on
   runtime-driver. Maintained architecture places runtime-driver and
   runtime-codegen as sibling consumers of core AWBC. The return also requires
   driver publication to validate JIT/codegen support without providing a
   lower-layer backend capability input. Referencing the backends from the
   driver would introduce a cycle; not referencing them leaves the promised
   publication check impossible.
6. The 533-row test matrix mostly repeats positive/missing/duplicate/mismatch
   names for synthetic rows, but those rows do not define the missing type
   authority. The 247-row inventory omits `BytecodeProgram` entirely and does
   not close the nested typed-carrier or same-phase constructor migrations.
   Row count is not exhaustive evidence.

## Next action

Do not create another child correction or re-submit the same request. The user
selected direct Sol max resolution after repeated oversized invalid returns.
The final resolution is recorded at:

`docs/reviews/designs/lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-final-resolution/README.md`

That resolution preserves the landed substrate and directly closes the Agent
type, recursive typed-carrier, RuntimePlan codec/BytecodeProgram, backend
layering, and phase-order decisions. Implementation proceeds from that design
rather than another returned archive.

No production code was changed from this returned package. Independently safe
existing substrate remains valid; the invalid synthetic/publication design is
not used as implementation authority.

## Validation performed

- source ZIP and retained ZIP SHA-256/byte equality: passed;
- unsafe path, traversal, symlink/reparse, case-collision preflight: passed;
- ZIP member versus extracted-file SHA-256 parity: 37/37 passed;
- internal `MANIFEST.sha256`: passed with zero errors;
- current request and previous-invalid copies/hashes: passed;
- all normative Markdown and CSV surfaces were inspected;
- current core/runtime-plan/compiler/bundle/driver/backend Cargo and source
  owners were compared with the returned APIs; and
- no Cargo/test command was run because the return is design-only and contains
  no production patch.
