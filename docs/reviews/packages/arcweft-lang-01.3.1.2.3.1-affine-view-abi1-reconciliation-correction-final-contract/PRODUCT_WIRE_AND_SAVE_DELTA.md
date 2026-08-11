# Product wire and save delta

## Allocation table

| Carrier | Final allocation |
|---|---|
| AWBC ABI | **1** |
| AWBC codec | 8 |
| Stream opcodes | `0x27..=0x29` |
| generic `CopyValue` | `0x2a` |
| ViewProgram AWFB section / codec | kind/tag 9 |
| ViewProgram common product schema | 1 |
| ViewText codec | 11 |
| bundle session save | `arcweft.bundle_session`, version 2 |

No ABI 2, new AWFB section, numeric codec tag, outer field, save version, or dual reader is allocated.

## ViewProgram transcript direct replacement

The exact root order becomes:

```text
schema
accepted_revision
program
value_programs
definitions
resolved_resources
static_requirements
static_fragments
static_certificates
source_map_digest
```

`ViewValueInputBinding` gains `ownership` and `transfer`. `ViewStaticCertificateResource` gains optional `requirement`. Strict unknown/duplicate-field rejection and canonical re-encode remain mandatory.

New digest domain:

```text
arcweft.view.static-requirement.v1\0
```

`ViewProgramSemanticDigest` includes sorted static requirement digests and corrected value-program input transfer facts. `AcceptedViewProgramRevision` continues to bind the whole canonical program, fragments, certificates, and source-map digest without a circular seed.

## Save schema 2 direct replacement

The whole-execution save adds the exact affine allocator cursor inside the existing schema-2 snapshot owner. View runtime rows stop serializing live `RuntimeBinding`; they use typed coordinates plus the whole-execution `RuntimeValueSnapshotV2` graph. No schema-1 or provisional schema-2 reader remains.

Restore validates in this order:

1. envelope/schema/content identity;
2. ABI **1** and codec 8;
3. limits/canonical bytes;
4. type/layout/ownership facts;
5. allocator cursor;
6. duplicate owner/Stream reciprocity;
7. View retained slots are Unrestricted;
8. static requirement/certificate/fragment joins;
9. exact generation pins;
10. activation-domain lease and current holder;
11. non-fallible atomic publication.
