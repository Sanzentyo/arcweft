# AW-AH-007/008 typed RichText attribute validation delivery status

Date: 2026-07-14
Last re-evaluated: 2026-07-15

## Outcome

The `v2` re-delivery contains no implementation, design, patch, overlay, test,
or validation evidence that can be applied to Arcweft. It is an internally
consistent manifest-only archive, so no production change is justified by this
package. The implementation goal remains open behind the existing standalone
[AW-AH-007/008 design request](../reviews/requests/2026-07-14-aw-ah-007-008-typed-rich-text-attribute-validation.md).

A 2026-07-15 readiness re-evaluation also confirmed that the standalone request
is not itself a final design: it intentionally asks the assignee to decide the
grammar, positional policy, schema owners, value algebra, duplicate/unknown
policy, defaults and limits, recovery, codec scope, and migration behavior.
Those choices materially change the public authoring and checked-lowering
contracts, so implementing them by inference would be speculative. The request
now contains an explicit dispatch contract, required delivery layout, and
decision-completeness gate for a design-only final-contract task.

## 2026-07-15 diagnostic-only re-delivery

The later archive at
`D:/sanze/Downloads/arcweft-aw-ah-007-008-typed-rich-text-attribute-validation(1).zip`
is not a byte-for-byte re-delivery of `v2`, but it is not a successor design or
implementation contract either. It is a failed implementation-orchestration
capture whose own `delivery/README.md` states that the private repository could
not be obtained and that no implementation is present.

| Field | Evidence |
| --- | --- |
| Inspected checkout | Git `02750f530cfb`; Jujutsu parent change `nsrnqtmx` |
| ZIP size | 23,218 bytes |
| ZIP SHA-256 | `a180b4972d0c48d19755f7eb684c499a6d028a20bc9305a4910116cff768c9a4` |
| Archive entries | 14 total; 11 regular files and 3 directories |
| README SHA-256 | `9e8036d99231a20683a87e1e743839b32465c6a23cc3431e28e43b6e7f5124fe` |
| Orchestrator-log SHA-256 | `b78e2209d7f797ef79fd0ce278e576e9344daaee4ee31144a707c93ed360e95a` |
| Embedded request SHA-256 | `807109509d9064f390eacc26f1c182ebdc72fa986ce19d8ab747e5d66653a16d` |

The embedded request is byte-identical to the checked-in standalone request.
Its duplicate full-read copy and both other input/full-read pairs also match
their recorded SHA-256 values. The remaining files are the inputs, clone-failure
logs, and an input-only hash list; there is no archive-wide manifest, final
design, schema matrix, diagnostic matrix, migration/test plan, traceability,
patch, source overlay, Rust change, or validation result.

The archive therefore confirms only that another assignee could not access the
checkout. It does not close any design decision and does not supersede the
standalone request. The required final-contract archive
`arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-final-contract.zip`
was not present in the supplied download location at this re-evaluation.

## Audited package

| Field | Evidence |
| --- | --- |
| Path | `C:/Users/sanze/.codex/codex-remote-attachments/019f5945-ad7b-76f0-ad40-15ace86d23d3/379EA879-DDCE-4C6E-8846-FBE80D62010C/1-arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-v2.zip` |
| ZIP size | 3,200 bytes |
| ZIP SHA-256 | `a7af75a7047ec02bb1b47da1078e4698d436b124d75749c5ecabf9e4d3219d5a` |
| Archive entries | 9 total; 3 non-empty files and 6 empty directories |
| Declared base revision | `52d6fadee2a0eee2fc3a565c4a2354e325eb49a1` |
| Audited checkout parent | `52d6fadee2a0eee2fc3a565c4a2354e325eb49a1` |

`DELIVERY/PACKAGE_MANIFEST.json` lists only these two payload files, and their
declared sizes and hashes match the archive:

| Payload | Size | SHA-256 |
| --- | ---: | --- |
| `DELIVERY/ARCWEFT_SOURCE_MANIFEST.sha256` | 68 | `abcfa6a9d4df344d1781bc2560b5e4cdcae08b39ed303063535e7e1e926a304a` |
| `DELIVERY/PAYLOAD_MANIFEST.sha256` | 108 | `cfbaf2281b914726b86b8486894bad16b4687d2e9f8150eec6beb17d33a1bcef` |

The source manifest contains only
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  -`,
the SHA-256 of an empty byte sequence. The payload manifest lists only that
empty-source manifest. The package manifest itself is 649 bytes with SHA-256
`e6648da25ba9a67975e8761dbb6eb1c59493fee44d185859c3b7e35ad6b7bfe8`.

The archive has empty `arcweft/`, `DELIVERY/inputs/`, `DELIVERY/scripts/`, and
`DELIVERY/validation/` directories. It has no README, request, design,
implementation notes, traceability, patch, overlay manifest, Rust source,
fixture, test, or validation log. Consequently it supplies no explicit
acceptance criteria and no file/blob mapping to compare against the declared
base revision.

## Comparison with the first delivery

The first delivery at
`D:/sanze/Downloads/arcweft-aw-ah-007-008-typed-rich-text-attribute-validation.zip`
was 9,089 bytes with SHA-256
`c15045e19afe1efe4d44d46ce3f6d022de32457c39543d81ae69f84c71ea11c1`.
It contained 19 entries, but only failed delivery reports and validation logs;
it had no implementation payload. The `v2` archive does not repair that
defect: it removes those reports while retaining empty source and validation
directories.

## Completion boundary

No Rust, Cargo, schema, fixture, or stable design file was changed for this
delivery. The existing standalone request already records the accepted
findings, required decisions, ownership constraints, migration order,
diagnostics, codecs, test matrix, and acceptance criteria; duplicating it as a
new sequence request would create two competing design sources.

A usable **design** re-delivery must first provide, at minimum:

- the normative design decisions and explicit acceptance criteria;
- an exhaustive per-surface schema/default/diagnostic matrix;
- exact type ownership, migration order, raw-parser deletion points, and
  behavioral verification plan;
- traceability from every open decision to a selected normative answer; and
- a clear list of explicitly removed surfaces or genuine external blockers.

That design package must not claim implementation completion and does not need
a speculative Rust patch. After the final design contract is accepted, a
separate implementation delivery must provide a patch or complete overlay tied
to exact base blob hashes, non-empty source and test payload, focused parser/HIR,
semantic, formatter/LSP, runtime-plan, codec (where applicable), and
cross-backend evidence, plus a clear list of intentionally excluded work.

Package integrity and checkout revision were checked directly. Compilation,
tests, Clippy, formatting, and structural audit are not applicable because the
archive supplied no implementation and this status record changes no Rust or
public boundary.
