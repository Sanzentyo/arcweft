# Validation record

## Repository-aware validation executed

Repository access used the private GitHub connector because no local private checkout was available.

Executed repository checks:

1. resolved `main` to a concrete commit;
2. detected that main advanced during reconciliation from `126f7ece0f69062f1cbea3e753cd04af5ead2056` to `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`;
3. compared those commits and re-read the new root `AGENTS.md`, current request, and intake audit;
4. resolved `refs/heads/main` again and recorded equality with the connector-visible origin/main authority;
5. fetched every load-bearing file by exact pinned ref and recorded blob SHA/facts in `REPOSITORY_EVIDENCE.json`;
6. inspected the current private `res` syntax path and confirmed no final public resource HIR/sema owner;
7. inventoried current resource identities/descriptors/values/retained categories/registry issues/digests/limits;
8. inspected strict Taplo and spanned JSON decoder patterns, source ownership, explicit project topology, compiler registry handoff, and AWFB section code owner;
9. inspected current request/predecessor/Lang-01.4/Lang-01.4.1 reconciliation and ZIP intake evidence;
10. queried commit status/workflow metadata at the pinned revision; repository CI is not used as a substitute for this contract validation.

The connector activity summary is in `logs/repository_connector_validation.log`.

## Artifact validation executed

The following are run by `tools/validate_contract.py` (117 required checks) and captured in `logs/contract_validation.log`:

- strict duplicate/floating-token JSON loading of all examples/vectors/status;
- Draft 2020-12 JSON Schema validation when `jsonschema` is available;
- exact root/package/ID/tag/shape checks for bundled examples;
- semantic canonicalization and byte-for-byte regeneration;
- full closed-variant coverage from the full example;
- BLAKE3 self-tests for empty and `abc` official vectors;
- descriptor canonical transcript and derive-key digest recomputation;
- canonical manifest RawDigest recomputation;
- required negative-case inventory and diagnostic codes;
- status invariants (`OPEN_QUESTIONS=0`, implementation ready, final, non-fallback, no production changes);
- required document/file presence and placeholder rejection;
- every `MANIFEST.sha256` entry.

After ZIP creation:

- `unzip -t` verifies the central directory and compressed members;
- the ZIP is extracted to a clean directory;
- the same standalone validator is run against the extracted package;
- the outer archive SHA-256 is recorded next to the final link.

`MANIFEST.sha256` covers the receipt logs themselves. To avoid a self-referential receipt,
`logs/archive_validation.log` records the executed closed-candidate ZIP check; that receipt is
then hashed, the ZIP is rebuilt deterministically, and the identical central-directory and
clean-extraction checks are executed once more against the final bytes. The final outer result
is reported with the download link and does not alter the archive.

## Explicit validation boundary

No local checkout was available. Therefore no Cargo command was executed and this package does not claim that future production code compiles or passes tests. No production code exists in this deliverable, so Cargo build validation is not an applicable artifact success criterion.

Repository contract validation **did** succeed: every decision is reconciled to pinned repository types/owners/tests through connector-accessed blobs, the schema/examples/digests are self-validated, and all required contract questions are closed. This distinction is machine-readable in `STATUS.json` and is not treated as a fallback.
