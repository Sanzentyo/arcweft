# Verification boundary and completed package checks

## Actually inspected

- The request, premise, and Rust skill files were read in full; byte counts, full SHA-256, line counts, and terminal-block hashes are in `PROCESS_INPUT_READ_PROOF.json`.
- The request is copied byte-for-byte under `request/`.
- Both available predecessor ZIPs were opened, compressed-data tested, hashed, and inventoried in `PARENT_CONTRACT_LEDGER.json` where present.
- A usable private Git checkout was not acquired in this packaging environment. Repository-dependent compile/test and exact-source verification are therefore specified as acceptance obligations rather than reported as completed.
- Latest `origin/main` AGENTS.md could not be fetched in this environment; the package does not claim otherwise.
- Repository evidence rows, when available, contain only revision/path/line/term/disposition—not copied production source.

## Package checks completed

The generated archive was subjected to:

1. sorted fixed-timestamp/fixed-permission deterministic ZIP construction;
2. `ZipFile.testzip()` compressed-data/CRC validation;
3. fresh extraction;
4. SHA-256 verification of every `MANIFEST.sha256` entry;
5. byte-identical comparison of the extracted request copy with the uploaded request;
6. UTF-8 decode of every Markdown/text artifact;
7. JSON parse of every JSON artifact;
8. CSV parse and uniform-column validation of every CSV artifact;
9. a prohibited-member audit for production/patch/overlay/binary artifacts;
10. a second independent deterministic rebuild and byte-for-byte archive comparison.

## Not claimed

This design-only archive does not claim that Cargo check/test, Clippy, rustfmt, repository Tier 2, structural source audit, or an implementation patch was executed. Those require production implementation and are listed in `ACCEPTANCE_COMMANDS.md`. No production implementation is included.

## Design evidence confidence

- Request/package integrity: directly verified.
- Parent archive integrity: directly verified for locally available ZIP bytes; no claim beyond those bytes.
- Repository path/symbol inventory: directly verified only when `repo_checkout_available=true` in `CONTRACT_METADATA.json`; curated design-obligation rows are clearly labeled otherwise.
- Production behavior: inferred from repository evidence and predecessor contracts, not executed by this design-only artifact.
