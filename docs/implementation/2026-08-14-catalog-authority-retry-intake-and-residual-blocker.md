# Catalog-authority retry intake and residual blocker

Date: 2026-08-14

Continues:
`docs/implementation/2026-08-13-catalog-digest-role-root-return-invalid.md`

Inspected Git baseline:
`36f83f8509417d1110a34f1b32aee6f4a113dcf3` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake.

## Returned retry archive intake

The source download used a Windows collision suffix `(1)`. Repository intake
does not preserve parenthesized numeric suffixes. Because the invalid first
return already occupies the unsuffixed canonical name, the retry is retained
with `_1`:

`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1-catalog-digest-role-root-and-construction-authority-correction-final-contract_1.zip`

SHA-256:
`e0aa31dfefa5bc0d9fab213d19fef6fd74a142cef6dd7d4e6922d05c077bc998`

The 88,438-byte ZIP contains 50 files without a redundant wrapper. It has no
unsafe/rooted/drive/traversal path, symlink/reparse entry, or case-insensitive
collision. All 50 extracted files match their ZIP-member SHA-256 values and
every internal `MANIFEST.sha256` row passes.

Unlike the invalid first return, `SOURCE_REQUEST.md` is byte-identical to the
maintained request, SHA-256
`0c570da664999507d1895813d65a707fb13726d48c489e8fd322c238a3361b78`.
The package reports `READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`, current
main `36f83f8509417d1110a34f1b32aee6f4a113dcf3`, the exact retained-parent hash,
and all Arcweft-owned versions as `1`. Its 15 decision documents, 226-row
inventory, and 671-row test matrix materially answer the maintained request.

## Readiness adjudication

Direct full-package/current-source inspection and an independent Sol max audit
classify the retry as `NOT_READY`, not `INVALID_AS_DELIVERED`. The Character
and View digest transcripts, role declarations/table, semantic root-ID byte
projection, project/producer construction domains, AWBC `MakeRecord` domain,
and checked Choice semantics are concrete and retained. Three residual
conflicts block the root/admission/checked-validator cuts:

1. Decision 13 redeclares `RuntimeValuePath` and
   `RuntimeValuePathSegment` in `pattern.rs`, although current
   `value/ownership/path.rs` already owns public canonical Serde types with
   different segments and integer widths. The retry inventory simultaneously
   says to reuse the current owner. Extending, replacing, or separating those
   types changes public APIs, wire bytes, canonical order, affine evidence, and
   error paths.
2. The 91 RuntimePlan and 33 AWBC mapping rows are conceptual rather than
   independently resolvable. Many current raw owners do not store a checked
   semantic coordinate. Several returned AWBC site variants use an unspecified
   `slot: u32` for tables whose current rows expose only indirect
   signature/function/value references. A raw root-use row and runtime-type
   declaration can therefore self-claim matching authority instead of being
   recomputed from a separately verified typed owner.
3. The compile-clean order requires a validator borrowing
   `AdmittedRuntimeGeneration` in phase 2, while current production lacks that
   type and the returned order does not create generation admission until
   phase 7. A placeholder or temporary constructor is prohibited.
4. The checked outer-shape enum omits current Range/Matrix/Tensor value
   families, and nominal values do not carry the `actual` semantic ID required
   by the proposed pre-lookup error. `RuntimeIndexPath` also derives
   deserialization that bypasses its stated checked constructor.
5. The proposed catalog wrapper crosses the forbidden dialogue-to-runtime-
   driver dependency, accepts a free generation assertion, and names a
   Character-to-View relationship absent from the current CharacterCatalog.

## Safe implementation boundary

The lower `CharacterDialogueRuntimeRole` inherent vocabulary is independent
and may land. The returned Character/View digest owners and transcripts are
also independent once implemented and tested locally. Do not implement the
checked validator, root-use/site declarations, generation admission,
RuntimePlan/AWBC root correlation, nominal-domain issuance, or execution cut
until the child correction closes the three conflicts.

Child correction request:
`docs/reviews/requests/2026-08-14-lang-01.3.1.2.3.2.1.2.1.1.1-checked-value-path-and-resolvable-root-site-correction.md`

## Validation performed

- source ZIP SHA-256 and byte length: verified;
- unsafe path, traversal, symlink/reparse, and case-collision preflight: passed;
- ZIP member versus extracted file SHA-256 parity: 50/50 passed;
- internal `MANIFEST.sha256`: passed;
- request-copy SHA-256 equality: passed;
- all normative Markdown, decision tables, mapping CSVs, metadata, inventory,
  and validation evidence were inspected; and
- no root/admission production code was implemented from the blocked parts.
