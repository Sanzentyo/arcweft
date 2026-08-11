# Repository evidence

## 1. Audit method

The audit used the GitHub connector against exact commit
`8ca3677d36dee0ee92eb16c35db108111b222a3c`, read the root `AGENTS.md` and the
attached Rust skill in full, inspected the canonical request and the named
implementation evidence, inspected the relevant current Rust owners, and used
the exact-baseline Arcweft package-intake records for outer-hash and internal
manifest verification.

Evidence levels used below:

- **Direct** — file content or current Rust source read from the exact Git commit.
- **Repository-verified** — binary package hash/member verification recorded by
  an Arcweft intake document at the same exact Git commit.
- **Dispatch-supplied** — identity supplied by the canonical dispatch but not
  exposed by the GitHub object model.

The binary package bytes were not exported out of the connector into this
artifact runtime. Their outer and internal-manifest verification is therefore
cited as repository-verified rather than falsely described as a second local
rehash. The contract conclusions were independently reconciled against the
canonical requests, accepted intake records, implementation evidence, and
current source owners.

## 2. Baseline identities

| Evidence | Value | Level |
| --- | --- | --- |
| Current `main` | `8ca3677d36dee0ee92eb16c35db108111b222a3c` | Direct |
| Jujutsu dispatch change | `swqlskklykxrszxyxtyzptkwurnyvstx` | Dispatch-supplied |
| Root `AGENTS.md` blob | `e91f99213dde67953beda6aa078c370a8dc4541d` | Direct, full file |
| Applicable nested `AGENTS.md` | none found | Direct repository search |
| Rust skill SHA-256 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | Direct, full attached file |
| Arcweft prerequisite SHA-256 | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | Direct, full attached file |
| Canonical request blob | `2fc1f302a7a05e5325fd1dea61871d94eddbab3e` | Direct |
| Dispatch intake blob | `74f81b72e87a2ac0ed436c393c02c188c17bf318` | Direct |

The Git and Jujutsu coordinates are not conflated. The Git commit is the
content authority available through the connector. The Jujutsu ID remains the
required dispatch coordinate and is recorded transparently.

## 3. Package verification evidence

The canonical request names these exact outer hashes:

| Package/input | SHA-256 | Verification evidence |
| --- | --- | --- |
| AW-AH-009.3.3 | `9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78` | package-chain intake: 10/10 |
| AW-AH-009.3.3.1 | `3D81158EB37F503EF7B0F242A79015BA1AB00E3954A8DAE4384F45EAAB55B672` | package-chain intake: 10/10 |
| AW-AH-009.3.3.2 | `C5B6BBF9ADDB45F2D6ECBDFD8F2ABC4D6602F079A847A20DB8F26140D53A248F` | package-chain intake: 13/13 |
| AW-AH-009.3.3 request | `C2A101E93213682B8D05E7F08B2FE58CF8C187E6E6F25129B0513941C2E05B2B` | canonical request |
| AW-AH-009.3.3 return | `BAE928C475214AB141DF108B1B2C2A34D7E1AFCF61110145C7B59074D79AA76E` | dispatch intake: 9 members / 8 payloads verified |
| AW-AH-009.3.3.4 | `DD8096DEDEF9FE2446291B3849DCEABD8BB5192B88533AA12FEE2DFC3CCEC484` | dispatch intake: 10 members / 9 payloads verified |
| AW-AH-009.4 | `A86044FEA7AAFF3EC3829DFA0AD6552C88377CA61FA2911C3B96EA34CA0FFA5E` | dispatch intake: all 19 verified |
| AW-AH-009.4.2 | `05E825DDE033F308F24FC1F6E504B4C26BBA2D61FD33852CE880DC666BA8F2A8` | dispatch intake: all 16 verified |
| AW-AH-009.4.3 | `FD9F97D37B857991120DD5E5E5DB27953257121FC48C79BEEF4FA03DF1F23396` | dispatch intake: all 17 verified |

The exact-baseline intake reports no outer-hash, member-digest, or declared
length mismatch for the dispatch set.

## 4. Applicable repository rules

The full root `AGENTS.md` establishes the following relevant constraints:

- typed layer direction and one accepted owner per boundary;
- direct migration to the final model rather than compatibility aliases, dual
  readers, or deprecated carriers;
- no source gates or permanent removed-syntax diagnostics;
- no source/display string reconstruction when a typed identity exists;
- no ad hoc helper/extension-trait workaround for Arcweft-owned types;
- focused validation followed by workspace check, strict Clippy, relevant Tier
  2, and structural audit;
- package intake by outer hash and internal manifest;
- no partial implementation claim against complete package acceptance criteria.

The full Rust skill additionally requires deliberate visibility, clean owned
APIs/newtypes, cautious macro/allow use, no unsafe/`Box::leak`/`forget` without
approval, checkpoint Clippy, and final formatting. This design introduces no
unsafe, unstable feature, macro, dependency, or public compatibility API.

## 5. Current family and schema owners

### Family inventory

Direct inspection of
`crates/arcweft-lang-sema/src/callable/identity.rs` at blob
`484dcfcd0fe9255faed05803d07186e43bfc65ac` shows a 23-family inventory that
contains both `Dialogue`, `CapacityMethod`, and `Speaker`, plus candidate IDs for
all three. This is the required pre-final inventory shape.

### Current Capacity schema drift

Direct inspection of
`crates/arcweft-lang-sema/src/callable/schema/families.rs` at blob
`5098b97d73f4325db59e2c39cbf5c36bb6379c10` shows:

- `CapacityMethodId::signature_schema` currently delegates to a closed
  homogeneous schema using `Named("_")` and the authored arity;
- Drop, Promotion, and Speaker use the repository's `variadic_unchecked`
  helper;
- `variadic_unchecked` owns an optional unchecked rest-positional parameter,
  OpenUnchecked unknown named arguments, and unchecked spread.

This confirms that the current Capacity schema is drift, not truthful final
negative evidence, and that the accepted correction must reuse the existing
unchecked schema behavior rather than invent a rejection.

### Current static Capacity bypass

Direct inspection of
`crates/arcweft-lang-sema/src/checker/helpers.rs` at blob
`73e50d08ce8161f3987cd2215173f0cf5004fe2b` and
`crates/arcweft-lang-sema/src/checker/expr.rs` at blob
`195c59801c73c1262be6e45e77e827ed6d11db2d` shows:

- `well_known_static_capacity_method_type(&str)` slices `Vec<...>` text;
- bare `Vec.with_capacity` becomes `Vec<Named("_")>`;
- String/Bytes spellings are matched directly;
- `check_call_expr` invokes that helper before the registered shared-resolver
  path, checks arguments untyped, and returns early.

This confirms `PendingAuthority` at the baseline and the exact same-cut deletion
obligation selected by AW-AH-009.3.3.4.

### Accepted Capacity switch evidence

The accepted blocker/intake note at blob
`e58202f253eff74673725af536f5d64866b31ecc` records the verified route:

```text
ParenthesizedCalleeSyntax::PathMember
  -> Expr::Call(CallExpr)
  -> SourceBackedTypeRef / nominal resolution
  -> CallCallee::AssociatedType
  -> single resolve_call_target
  -> CapacityMethodId
  -> checker-owned facts
  -> native semantic signature projection
```

It also fixes value-first dot resolution, explicit type-associated generic
resolution, environment-before-Capacity and Capacity-before-trait/data-last
precedence, and same-cut deletion of the old readers.

## 6. Current Dialogue owners

Direct inspection of
`crates/arcweft-lang-sema/src/callable/dialogue.rs` at blob
`f4c59781d9637d9e0c7b9e9f4ea2398a2f78d5de` shows the frozen identities:

```text
DialogueCallableId::{SpeakerLine, ContentCall}
DialogueCalleeIdentity::{Speaker, SpeakerPreset, Content}
```

Direct inspection of the current Dialogue schema in
`schema/families.rs` shows a Unit-returning, named-field/rest-named schema tied
to Speaker/Content identity. Direct resolver inspection at blob
`1248ac91e076635ec587d9338c27d78ec7c4c856` shows `CallCallee::Dialogue` and a
resolver branch that constructs this frozen candidate.

The AW-AH-009.4 request and accepted implementation evidence require deletion of
Speaker/SpeakerPreset and replacement with first-class CharacterDialogue. The
AW-AH-009.4.2 private cut explicitly states that its private syntax/HIR carriers
are not public authority. The AW-AH-009.4.3 intake states that current production
still lacks the accepted project source-site/line inventory. The Proof public
switch readiness note states that Dialogue's public source owner must join the
same attached syntax/HIR/project authority switch and forbids a dual reader.

These direct facts require Dialogue to remain `PendingAuthority` at the baseline.

## 7. Current resolver and transaction behavior

Direct inspection of the shared resolver shows:

- one `resolve_call_target` entry;
- selected-call ordering with environment methods before Capacity, Capacity
  before traits, and data-last later;
- final candidates carried through typed `ResolvedCallable` values;
- no basis for a second signature-only resolver.

Direct inspection of
`checker/expr/registered_call/selection.rs` at blob
`044abaaa60f9e6648ba0f06159d87abf4a1b02a3` shows:

- every candidate, including a singleton, is evaluated inside a checkpoint and
  rolled back;
- candidate-specific argument checks and specificity are retained only in probe
  objects;
- a unique winner is replayed and committed;
- ambiguity publishes a deterministic primary tied projection without selected
  replay;
- singleton rejection performs a rejected recovery replay;
- multiple rejection uses a stable probe projection.

Direct inspection of
`checker/registered_candidate_transaction.rs` at blob
`6f7e8a08cd73502cc000b0da376e5ca3ee22e873` shows that rollback restores
`TypeCheckStats` and semantic state including judgments, lowering evidence,
captures, numeric fallback, effects, scopes, references, Speaker observations,
curried state, and call-target facts, while preserving only the typed terminal
query error through the accepted channel.

Direct inspection of
`checker/expr/registered_call/arguments.rs` at blob
`1b9d0fe0b74b94e719ccff0304ed69970a509287` shows the exact physical evaluation
sites selected by this contract:

- `check_registered_argument_slot_with_inferred` computes substituted expected
  type immediately before the physical slot checker;
- fixed-literal spreads loop through logical slots;
- typed-rest spread physically checks the container expression;
- unmapped arguments physically call the ordinary expression checker;
- `CandidateArgumentProbe`, callable work, and signature work are separate
  admission/charging steps.

This code proves that a retained/checkpointed stat cannot truthfully serve as a
physical candidate evaluation count.

## 8. Implementation evidence inspected

| File | Git blob | Relevant conclusion |
| --- | --- | --- |
| `2026-07-21-aw-ah-009-3-semantic-selection-and-resource-accounting.md` | `2a2c861e...` | contextual transactional probing/replay and separate work accounting are implemented; final CharacterDialogue gap remains |
| `2026-07-20-aw-ah-009-4-2-private-cut-2.md` | `4c6a6f...` | private carriers are not public/final HIR authority |
| `2026-07-21-aw-ah-009-4-3-source-site-line-identity-intake.md` | `6a3f924...` | accepted line/project design is ready, current production remains provisional |
| `2026-07-21-proof-public-switch-readiness.md` | `4bfb775...` | Dialogue must join one attached syntax/HIR/project switch; no dual reader |
| `2026-07-24-aw-ah-009-3-3-3-1-dispatch-intake.md` | `74f81b72...` | request is the only current design dispatch; package set verified; no production change |

Abbreviated blob prefixes are included only where the full blob was not needed
as an external identity. The exact paths and Git commit make the evidence
reproducible.

## 9. Verification boundary and readiness

No missing design boundary was found. The Jujutsu ID could not be independently
queried through GitHub, and package bytes could not be exported from the
connector into the local artifact builder; both limitations are explicitly
recorded. They do not alter the exact Git content authority, the repository's
same-commit package verification, or any selected type/ownership decision.

The design is therefore `READY_FOR_IMPLEMENTATION`, while current production
remains at the staged pre-switch state documented above.
