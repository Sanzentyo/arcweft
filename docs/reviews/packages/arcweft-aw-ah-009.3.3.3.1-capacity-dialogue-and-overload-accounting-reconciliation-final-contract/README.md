# AW-AH-009.3.3.3.1 Capacity, Dialogue, and overload accounting reconciliation

## Status

`READY_FOR_IMPLEMENTATION`

This package is a design-only correction. It changes no production Rust, Cargo
manifest, test, schema, fixture, stable design document, or repository overlay.
It is authoritative only for the CapacityMethod, Dialogue, Speaker-transition,
and overload-accounting corrections named by the parent request.

## Inspected baseline

| Coordinate | Value | Verification |
| --- | --- | --- |
| Repository | `Sanzentyo/arcweft` | GitHub connector |
| Branch | `main` | GitHub connector |
| Git commit | `8ca3677d36dee0ee92eb16c35db108111b222a3c` | independently resolved as current `main` |
| Jujutsu change | `swqlskklykxrszxyxtyzptkwurnyvstx` | dispatch coordinate supplied by the canonical request; GitHub does not expose Jujutsu metadata |
| Root `AGENTS.md` Git blob | `e91f99213dde67953beda6aa078c370a8dc4541d` | read in full; no nested `AGENTS.md` exists at the baseline |
| Rust skill SHA-256 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | attached file read in full |
| Arcweft prerequisite SHA-256 | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | attached file read in full |
| Canonical request Git blob | `2fc1f302a7a05e5325fd1dea61871d94eddbab3e` | read in full from the baseline |
| Canonical request SHA-256 | `BE1D54C1763EEB5F2C76B91ED45970E5C6EA958EC2C3A308190E20F983251EF2` | repository dispatch intake |

The Git baseline is independently verified. The Jujutsu identifier is retained
as the dispatch coordinate rather than falsely claimed as independently queried
through GitHub.

## Consumed contract chain

| Input | SHA-256 | Role in this correction |
| --- | --- | --- |
| AW-AH-009.3.3 shared catalog/resolver | `9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78` | sole resolver, candidate ordering, transactions, checker-owned facts, signature projection |
| AW-AH-009.3.3.1 curried groups | `3D81158EB37F503EF7B0F242A79015BA1AB00E3954A8DAE4384F45EAAB55B672` | resolved-callable success boundary; unchanged |
| AW-AH-009.3.3.2 typed external publication | `C5B6BBF9ADDB45F2D6ECBDFD8F2ABC4D6602F079A847A20DB8F26140D53A248F` | typed segmented project paths; unchanged |
| AW-AH-009.3.3 request | `C2A101E93213682B8D05E7F08B2FE58CF8C187E6E6F25129B0513941C2E05B2B` | original migration-evidence question |
| AW-AH-009.3.3 returned package | `BAE928C475214AB141DF108B1B2C2A34D7E1AFCF61110145C7B59074D79AA76E` | adjudicated, not accepted wholesale |
| AW-AH-009.3.3.4 Capacity authority | `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484` | authoritative typed associated callee and `variadic_unchecked` behavior |
| AW-AH-009.4 CharacterDialogue | `A86044FEA7AAFF3EC3829DFA0AD6552C88377CA61FA2911C3B96EA34CA0FFA5E` | authoritative final first-class value and deletion direction |
| AW-AH-009.4.2 Dialogue syntax/HIR | `05E825DDE033F308F24FC1F6E504B4C26BBA2D61FD33852CE880DC666BA8F2A8` | authoritative bracket/colon content-application owner |
| AW-AH-009.4.3 line identity | `FD9F97D37B857991120DD5E5E5DB27953257121FC48C79BEEF4FA03DF1F23396` | authoritative project line identity and diagnostics |

The exact-baseline dispatch intake records that all named inputs matched their
outer hashes. It also records complete internal-manifest verification for the
five newly retained packages: AW-AH-009.3.3.3 (9 members), .3.3.4 (10), .4
(19), .4.2 (16), and .4.3 (17).

## Decision summary

1. `CapacityMethod` is `PendingAuthority` before the accepted AW-AH-009.3.3.4
   public switch. Neither the legacy string dispatcher nor the current
   homogeneous `_` schema receives migration credit. After the switch it is
   `IntentionallyUnchecked`, using the accepted typed associated callee and
   genuine `variadic_unchecked` behavior for zero, one, multiple, named, spread,
   and recovered arguments.
2. `Dialogue` is `PendingAuthority` until one compiling public switch installs
   the Proof-backed typed content application, accepted project line identity,
   final semantic/runtime-plan publication, and simultaneously deletes every
   frozen Speaker/string/`HirDialogue` reader and the Speaker family/ID. It is
   `RejectingSchema` only after that switch.
3. Speaker remains a typed current-phase `IntentionallyUnchecked` observation
   before the Dialogue switch, but its final-completion disposition is always
   `PendingRemoval`. It earns no final-model row or case credit and is absent
   from the final inventory.
4. Physical candidate-specific expression evaluation and retained inference are
   different evidence. Physical evaluation is recorded at the actual
   candidate/pass/argument-slot checker entry. Retained inference is the final
   multiset projection of existing `CheckedCallArgumentSlotFact` values in the
   committed or deterministic recovery `CheckedCallTarget`.
5. AW-AH-009.3 section 19 receives a complete staged classification table, but
   its end-to-end family-matrix acceptance gate remains open while any
   `PendingAuthority` or `PendingRemoval` disposition exists.

## Fixed phase cardinalities

| Phase | Inventory | Current executable rows | Current observation cases | Final-model credited rows | Final completion cases |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pre-capacity | 23 | 21 | 42 | 20 | 40 |
| Post-capacity / pre-Dialogue | 23 | 22 | 44 | 21 | 42 |
| Final post-Dialogue / Speaker deleted | 22 | 22 | 44 | 22 | 44 |

Each credited or executable row contributes exactly two cases. A rejecting row
contributes one accepted and one schema-rejected/poisoned case. An intentionally
unchecked row contributes one accepted and one clean-recovery case. A pending
row contributes no final cases. Speaker's two current observations never enter
the final-completion count.

## Normative precedence

This correction replaces only conflicting portions of AW-AH-009.3.3 and its
AW-AH-009.3.3.3 return:

- the CapacityMethod row and its fabricated spread rejection;
- the Dialogue row based on `SpeakerLine` or the frozen content carrier;
- Speaker's treatment as final completion evidence;
- the 20/3, 23/46, and broad physical-"exactly once" claims;
- AW-AH-009.3 section 19 and section 36 item 4 where they conflict with this
  package.

AW-AH-009.3.3 section 23 remains authoritative for transactional, contextual
candidate checking. AW-AH-009.3.3.4 remains authoritative for Capacity.
AW-AH-009.4/.4.2/.4.3 remain authoritative for Dialogue. The shared resolver,
candidate order, curried-group validation, typed external publication,
checker-owned call-target facts, native cache, and signature projection are not
redesigned.

## Required implementation sequence

1. Adopt this staged evidence vocabulary without changing production behavior.
2. Land the accepted AW-AH-009.3.3.4 typed Capacity switch and delete the old
   dispatcher, `_` placeholder schema, text slicing, and label readers in the
   same compiling cut.
3. Close all remaining implementable AW-AH-009.3 resolver, accepted-HIR,
   lifecycle, cache, limit, parity, and matrix rows, leaving only Dialogue's
   explicitly pending row.
4. Establish the required Proof attached syntax/HIR/project authority.
5. Land AW-AH-009.4.2, .4.3, sema, and runtime-plan publication in one compiling
   Dialogue switch and delete Speaker and all frozen readers in that same cut.
6. Activate the final Dialogue pair, remove the Speaker row, and only then close
   the final family-matrix acceptance gate.

## Non-goals

This package does not introduce a second resolver, context-free argument cache,
constraint IR, alternate HIR arena, compatibility alias, dual reader,
deprecated carrier, removed-syntax diagnostic, source gate, CSS path, or Takumi
path. It does not alter accepted Capacity, CharacterDialogue, line-identity,
curried-group, or typed-external designs.

## Package members

- `FINAL_CORRECTION.md` — exact replacement rules and switch gates.
- `FAMILY_CLASSIFICATION.md` — all family states and both evidence axes.
- `OVERLOAD_ACCOUNTING.md` — physical versus retained evidence semantics.
- `TEST_MATRIX.md` — executable classification, switch, overload, and drift rows.
- `REQUIREMENTS_TRACEABILITY.md` — parent-row precedence and requirement mapping.
- `REPOSITORY_EVIDENCE.md` — inspected source owners, hashes, and verification boundaries.
- `OPEN_QUESTIONS.md` — exact ready-state marker.
- `FINAL_STATUS.md` — readiness and implementation gating.
- `MANIFEST.txt` — sorted member hashes and byte lengths.
