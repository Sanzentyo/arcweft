# AW-AH-007/008 rich-text schema boundary — cut 1

## Scope and repository basis

This cut implements the independently useful M3a substrate from the
AW-AH-007/008 typed RichText attribute-validation final contract on Git
`fbd74038aa186240d3e73bacc3842a9d7f6fb4e9`.

The package is implementation-ready for the full sequence. This cut is not the
full sequence and does not redefine completion around the subset implemented
here. It adds only the owner-neutral schema descriptor boundary required before
dialogue and presentation owners publish their concrete schemas.

The M1 lossless argument grammar is already present in commit
`240f5bc8fb71532863556efdb668ba78335cf91c`. M2 ordered/ranged RichText HIR is
not implemented on this base and overlaps the active Proof syntax/HIR ownership
switch. This cut therefore does not edit syntax, HIR, parser recovery, or Proof
identity code.

## Implemented contract

The new Sans I/O `arcweft-rich-text-schema` crate owns exactly the generic M3a
descriptor vocabulary:

- tag schemas and owner-typed property schemas;
- source-form and selector contracts;
- checked value-kind descriptions;
- integer/fixed numeric, unit, enum, encoded-byte, and decoded-byte limits;
- required, optional, defaulted, and conditional property presence;
- explicit single/repeated multiplicity;
- reject-only unknown-property policy; and
- checked output-family classification.

`RichTextEnumSchemaId` is an owner-selected static newtype. It identifies one
closed enum domain without placing enum membership or a registry in this
crate. Concrete tag, selector, property, default-provenance, checked-value,
diagnostic, and wire identities remain with their specified later owners.

The contract names `RichTextEnumSchemaId` but does not spell out its storage.
This cut completes that mechanical detail as `&'static str` behind an opaque
newtype because schemas are immutable static owner metadata and the wire
contract later uses an owner-validated string identity. This does not create a
registry, serialization contract, or compatibility surface; a concrete flaw
found during owner integration may replace this unreleased internal shape
directly.

There is no parser, semantic checker, runtime conversion, codec, global map,
root facade re-export, compatibility alias, or raw value in the new crate.

## Dependency and structure result

The workspace adds one leaf member with no dependencies. Current dependency
fan-in is zero because concrete dialogue/presentation owner integration belongs
to the next M3 cut; fan-out is zero. The intended next edges are one-way:

```text
arcweft-rich-text-schema
  <- arcweft-dialogue
  <- arcweft-presentation
```

The crate must remain below syntax, HIR, sema, runtime-plan, renderer, codec,
I/O, and platform layers. It must never acquire an owner inventory or become a
second selector/property registry.

The structural audit measured the new production file at 14,354 bytes and 439
physical LOC, including a 114-line embedded unit-test module. Its responsibilities
are limited to the owner-neutral descriptor vocabulary and focused construction
tests. It is below the 1,200-LOC production warning threshold. The audit found
no dependency edge into or out of the new crate and no new structural warning;
the workspace result was 0 errors and 131 pre-existing warnings. Generated
reports are in
`docs/implementation/structure-audits/aw-ah-007-008-rich-text-schema-cut-1-2026-07-21/`.

## Remaining package work

The following stay open:

1. M2: ordered/ranged `HirRichTextTag` ownership after the Proof syntax/HIR
   ownership switch provides the final source identity boundary.
2. M3b: dialogue and presentation owner enums/inherent schemas, including the
   bounded `CC-001` removal, checked sema IR, typed proxy catalog, and structured
   diagnostics.
3. M4: schema-driven checking for every matrix family.
4. M5: private total runtime converters and strict DisplayCatalog/ViewText
   transcript codecs.
5. M6: atomic production cutover and deletion of all raw reparsers, fallback
   branches, unknown executable variants, and provisional codec readers.
6. M7-M9: shared formatter/LSP consumption, API-driven corpus, cross-backend
   and Tier 2 validation, documentation, and final cleanup.

No CSS/Takumi path, removed-syntax recognizer, source gate, dual reader,
deprecated field, or compatibility shim is introduced.

## Validation

The following validation passed on the stated base:

```bash
cargo test -p arcweft-rich-text-schema
cargo check -p arcweft-rich-text-schema --all-targets
cargo clippy -p arcweft-rich-text-schema --all-targets -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

- schema unit tests: 3 passed;
- schema doc tests: 0 tests, passed;
- all-target schema check: passed;
- strict all-target schema Clippy (`-D warnings`): passed;
- workspace formatting check: passed; and
- structural audit: 3,445 files, 1,793 Rust files, 826,398 Rust physical LOC,
  94 package manifests, 0 errors, 131 pre-existing warnings.
