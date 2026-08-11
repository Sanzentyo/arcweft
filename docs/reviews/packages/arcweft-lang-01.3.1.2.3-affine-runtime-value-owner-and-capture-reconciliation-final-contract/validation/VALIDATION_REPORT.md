# Validation report

## Result classes

| Class | Result | Exact scope |
|---|---|---|
| Supplied input identity | PASS | SHA-256 and byte length recorded for request, Rust Skill, premise, and all three predecessor ZIPs |
| Current policy identity | PASS | exact bytes/line counts/SHA-256 for root plus four applicable scoped AGENTS files; all read through final line |
| Current raw production corroboration | PASS | exact bytes/SHA-256 for value, environment, range, sequence, pattern, and plan owners, plus parsed raw-main engine/flow evidence explicitly lacking a retained byte hash; confirms live clone/plan-continuation seams without claiming a branch-head commit pin |
| Predecessor archive transport | PASS | `unzip -t`; internal `MANIFEST.sha256` verification: 17/17, 40/40, 23/23 |
| Predecessor test preservation | PASS | 530 + 168 + 105 = 803 rows retained with source ZIP and source-matrix digests in `PARENT_TEST_MATRIX_INDEX.json` |
| New contract matrix | PASS | 395 unique rows; JSON/CSV identity; R1–R10 and all 12 test prefixes present |
| Executable ownership-law model | PASS | 20/20 `unittest` cases; raw output in `reference-model-test-output.txt` |
| Package structure and text encoding | PASS | required members, safe relative paths, no symlinks/caches/VCS/build output, UTF-8, LF-only, final LF |
| Open questions/status | PASS | `OPEN_QUESTIONS.md` exactly `none\n`; READY status and baseline fields consistent |
| Machine-readable contract | PASS | schema/value assertions in `verify_contract.py`; ABI2/codec8, `0x2a`, snapshot exclusivity, Arc-only plan sharing, flow block/control-frame cut, plan constants, counts, and parent hashes |
| Member integrity | PASS | sorted `MANIFEST.txt`; SHA-256 for every member; 64-zero self-entry |
| ZIP transport | PASS | deterministic sorted fixed-timestamp build; archive CRC extraction; byte-identical second rebuild |
| Production Rust validation | NOT RUN | no local production checkout/full current-main Git SHA and no production Rust commands were run in this design-only assignment |

## Executed local commands

```text
python3 -B -m unittest discover -s reference_model -p 'test_*.py' -v
python3 -B validation/verify_contract.py --root . --write-manifest
python3 -B validation/build_zip.py --root . --output <final-name>.zip
unzip -t <final-name>.zip
python3 -B validation/build_zip.py --root . --output <second-name>.zip
cmp <final-name>.zip <second-name>.zip
```

All PASS rows above are input/package/reference-model or read-only raw-file facts actually checked before delivery. They do not imply that the target Rust API compiles or that Arcweft production tests passed.

## Mandatory implementation evidence not run here

The ordered G/P/C cuts must record actual results for the current repository's command authority, including at least:

```text
cargo fmt --all -- --check
focused owner-specific unit/integration/compile-fail/codec/parity tests
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo metadata --no-deps --format-version 1
just structure-audit
just structure-audit-gate
```

The implementation note must distinguish pass, fail, blocked, and not-run, and must record the full Git SHA and dirty state. No production patch, worktree, branch, PR, commit, or push is included or claimed by this archive.
