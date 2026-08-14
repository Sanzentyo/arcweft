# Validation evidence

## Performed

- read the current request in full and copied it byte-for-byte as `SOURCE_REQUEST.md`; SHA-256 `034eb287c315d699d1cf110babaffbd80650d2b8c1eb340bb6e8d6b6efc6c32e`;
- rehashed the retained retry archive as `e0aa31dfefa5bc0d9fab213d19fef6fd74a142cef6dd7d4e6922d05c077bc998`;
- attempted the exact current-main clone and preserved the DNS failure in `git-clone-current-main.log`;
- verified current `main` commit `36f83f8509417d1110a34f1b32aee6f4a113dcf3` and rehashed all 43 commit-pinned local source/policy captures plus 1 exact web-inspection row;
- parsed every JSON and CSV member and decoded every textual member as UTF-8;
- checked exactly 12 one-to-one required-decision rows/documents, 772 unique inventory rows, and 1878 unique test rows;
- checked the sole canonical `RuntimeValuePath` segment table (tags 0–10), complete 23-variant physical `RuntimeValue` shape table, 20 checked-type tags, and 12 integer-width tags;
- checked 69 RuntimePlan nested slot tags, 22 coordinate-step tags, 15 top-level plan sites, 17 top-level AWBC sites, 43 remaining AWBC nested slot tags, 63 instruction slots, 13 terminator slots, and 35 audio slots for uniqueness and selected exclusions;
- checked `MakeFunction`/`ApplyFunction`, `GotoDynamic`, and `RegisterCleanupEffectArgument` emit no invented typed-slot tag; audio and receiver-state rows remain mechanically direct, while effect audio values resolve through the exact audio-command reference;
- checked `RuntimeIndexPath` and mandatory root/site/domain newtypes cannot bypass the selected checked constructor through derived Serde;
- checked the direct plan/AWBC equality grammar emits exactly one authority tag and uses the exact checked-type grammar without another digest/root map;
- checked exact admitted plan/AWBC/product wrapper APIs, same-parent pair admission, domain-bound checked context, and the product-step raw-program deletion cut;
- checked no production `.rs`, patch, diff, overlay, compatibility artifact, unsafe path, case collision, symlink, generic uppercase `CLOSED`, positive alias, or positive generic `slot: u32` authority remains.

The finalizer then wrote a sorted SHA-256 manifest, constructed a deterministic 76-member ZIP with fixed timestamp/mode/order/compression, ran `ZipFile.testzip`, extracted it to a fresh directory, verified every manifest row and safe path, and rebuilt the ZIP byte-for-byte. The archive is emitted only if every assertion passes.

## Not performed

No production implementation or local Git checkout exists in this environment. Cargo check/test, rustfmt, Clippy, repository structure/dependency audit, executable codec/golden/tamper suites, and Tier 2 were therefore not run and are not claimed. Exact implementation-time commands are listed in `ACCEPTANCE_COMMANDS.md`.
