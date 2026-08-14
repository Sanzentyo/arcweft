# Current-main source evidence

Inspected commit: `80348beed0efa72db07f712122217b4e679e0a97` on `main`; parent:
`eb450570acff118ccc3e2a75751144f037af170f`. The complete request is 11995 bytes,
0 lines, SHA-256 `2498106d805515f2fba326ef55685a8699aec2ab1abb986e22bc2f0a1f984cc6`, and is
byte-identical to `SOURCE_REQUEST.md`.

The current commit was obtained through its GitHub commit patch and exact
commit-pinned raw source captures because the execution container's direct
`git clone` DNS lookup failed. The patch reconstructed the maintained request,
intake note, extracted parent package, and retained parent ZIP. Production files
named in `SOURCE_EVIDENCE.csv` were fetched at the full commit SHA, not from an
unpinned branch URL.

The commit patch is documentation-only relative to parent
`eb450570acff118ccc3e2a75751144f037af170f`; the relevant production sources therefore remain the
current implementations inspected here. `SOURCE_EVIDENCE.csv` records exact
hashes, byte/line counts, and retrieval URLs. `WEB_SOURCE_EVIDENCE.csv`
records the exact full-SHA line ranges used for the existing AWFB section enum,
container verifier, product writer, and SectionId formula where the execution
download tool declined the Rust MIME type; those URLs were still read directly
at the same commit.

This design uses source inspection as review evidence only. Future acceptance
must use typed/codec/compile/runtime tests in `TEST_MATRIX.csv`, not source
spelling or file placement gates.
