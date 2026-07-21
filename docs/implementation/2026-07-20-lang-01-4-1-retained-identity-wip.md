# Lang-01.4.1 retained-identity production reconciliation

## Package evidence

The implementation source is
`arcweft-lang-01.4.1-resource-reference-and-retained-identity-schema-contract-correction-final-contract.zip`.
Its external SHA-256 is
`2ff03e6c97d29783244ee8b6fbc7f8fb60261f1922634d81e8f3fba17a4e563a`.
All 19 payload entries match the archive's `MANIFEST.txt`; the package reports
`READY_FOR_IMPLEMENTATION` and zero open questions.

## Accepted correction

The three reference categories remain disjoint:

- `ResourceRef<T>` identifies one accepted `res` of exact `ResourceTypeId`;
- `AssetRef<P>` identifies one compatible packaged asset payload;
- `RetainedIdentityRef<K>` identifies one accepted retained owner of one of
  seven closed kinds.

The retained kinds are Character, View, Action, Layer, Signal, presentation
target, and scroll region. Presentation targets retain global or View scope.
Scroll regions retain both owner View `EntityId` and region `PublicId`.
Neither category is represented by a resource-directory entry or
`SavedResourceRef`.

## Current cut

The generic `arcweft-resource-model` substrate now contains:

- the closed `RetainedIdentityKind` inventory and canonical tokens;
- canonical resolved global, presentation-target, and scroll-region values;
- exact `ResourceValueType::RetainedIdentityRef` and
  `ResourceConstValue::RetainedIdentityRef` variants;
- exact-kind constant validation, distinct from category mismatch;
- an inherent `ResourceValueType::validate_reference_invariants` traversal
  that follows accepted nominal record/enum children and preserves exact
  collection/member paths;
- source-aware retained dependency records with typed value paths;
- recursive registry/default traversal through option, list, map, record, and
  enum nodes without a field-name switch;
- standalone v1 canonical transcripts and raw BLAKE3 digests matching the
  package vectors;
- canonical-byte map-key ordering and length-delimited embedding of the
  retained standalone transcript in the resource semantic encoder; and
- registry semantic-digest participation for the new type and constant.

## Validation at the focused cut

Validated on parent revision `c56c82240`:

- `cargo test -p arcweft-resource-model` — 36 unit/integration tests and
  doc-tests passed;
- `cargo check -p arcweft-resource-model --all-targets --all-features` —
  passed;
- `cargo clippy -p arcweft-resource-model --all-targets --all-features -- -D warnings`
  — passed;
- `cargo fmt --all --check` — passed;
- `git diff --check` over the resource/common-owner slice — passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 0 errors,
  129 warnings across 1,732 Rust files and 801,908 physical Rust LOC.

The largest file in the new crate is the 1,014-LOC integration test
`tests/registry_contract.rs`. The largest production file is `src/value.rs`
at 714 LOC. No production, facade, or integration-test warning threshold is
crossed.

## Latest-main intake audit

The implementation-ready package was re-audited on `main` at
`163281b6089c269d7132941a3ee8fd52710b9b2f`. Its external SHA-256 remains
`2ff03e6c97d29783244ee8b6fbc7f8fb60261f1922634d81e8f3fba17a4e563a`,
and the current focused `arcweft-resource-model` suite still passes all 36
unit/integration tests plus doc-tests.

No additional isolated production cut exists after the generic substrate:

- the exact retained kind/value/path/transcript contracts are already owned by
  `arcweft-resource-model`; adding another catalog, DTO, or projection there
  would create a second authority rather than complete a consumer;
- the nine corrected built-in field paths cannot be published as partial
  descriptors because the complete built-in schemas, stable field/schema IDs,
  defaults, codecs, and atomic registry transaction must land together;
- accepted owner resolution, visibility, aliases, source occurrences, and
  diagnostics require the one accepted project world after the public
  syntax/HIR identity switch and the in-flight callable-sema work release their
  owners;
- bundle retained-dependency records require the common resource definition,
  directory, value-path wire, and source-table product rather than a standalone
  provisional section; and
- the public manifest codec and bundle publication remain gated by the
  unreturned Lang-01.4.2 wire correction. A retained-only reader would be a
  forbidden second reader.

Accordingly this audit adds no Rust scaffold, compatibility shape, detached
wire record, or unused lowerer. The next production cut must begin at one of
the dependency releases below and implement its complete atomic boundary.

## Dependency-ordered remainder

The following work remains part of Lang-01.4/01.4.1 rather than being counted
as complete:

1. attach retained owner resolution to the one accepted project symbol world
   after the AW-AH-009.3 callable cut releases `arcweft-lang-sema`;
2. publish the nine corrected Image/Voice/Rig field paths when their complete
   built-in descriptors are installed;
3. consume the returned AW-AH-009.4.1.2 TTS provider/speaker contract
   atomically for Voice and VoiceProfile, without a provisional string field;
4. consume
   [`Lang-01.4.2`](../reviews/requests/2026-07-20-lang-01.4.2-resource-extension-manifest-wire-contract-correction.md)
   before implementing the public extension-manifest reader/encoder; that
   request is still required because Lang-01.4.1 freezes only the retained
   branch, not the complete manifest wire;
5. add resource directory, retained dependency product, owner lowerers,
   Agent projection, save rejection, and candidate-generation validation when
   the base Lang-01.4 resource product is installed; and
6. run the complete RR-001–RR-080 matrix, affected Tier 2 suites, workspace
   validation, and structural audit at closure.

The retained declaration surface itself remains owned by the independent
retained-global-identity grammar reconciliation. This cut consumes accepted
owner facts and does not add a resource-specific parser branch.

## Prohibitions retained

No compatibility alias, dual reader, raw family string, source gate,
removed-spelling diagnostic, CSS path, or Takumi path is introduced.
