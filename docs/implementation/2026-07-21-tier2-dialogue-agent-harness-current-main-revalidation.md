# Tier 2 dialogue/Agent harness current-main revalidation — 2026-07-21

## Scope

This cut revalidates the ignored Tier 2 MCP, Agent-observe, native auxiliary
capture, and exact visual-golden harness against the accepted production
contract:

```text
dialogue runtime state
  -> authored persistent View mount
  -> shared ResolvedTextDocument / TextLayout / PreparedTextBatch
  -> ViewPrimitive::Text
  -> ViewCompositor / SharedRenderer
  -> typed Agent observation and MCP resource publication
```

The initial test run was made from `a83613775abebb9574e9c8cee549e7ede798574e`.
Before this evidence note was sealed, the working change was rebased through
the documentation-only `c957a61e` commit and onto `118a9870`, which introduces
the shared RichText owner schemas. Because that latter change is in this
harness's risk area, the complete Tier 2 recipe was rerun after the rebase.
The note was finally based on documentation-only successor `888a0c09`; the
production revision covered by the final Tier 2 run remains `118a9870`.
There is no production or Tier 2 harness change in this cut.

## Result

The previously recorded stale expectations for old resource URIs, ordinal
semantic identities, and pre-View dialogue geometry do not reproduce in the
current checkout. The harness already discovers typed resource links and
capture references from the public protocol output and follows authored View
placement and shared prepared-text geometry.

No expectation was deleted or weakened. No compatibility URI, old semantic
identity, geometry fallback, duplicate renderer, source-text gate, or
production workaround was added.

The ignored `web/assets/noto-sans-jp-vf.ttf` test fixture was copied into the
isolated workspace for validation only. Its size was 9,590,844 bytes and its
SHA-256 was
`5113756F8A3B5D01B2211025E267C50121E3B36F465B7BBAF3CDAF4C3430BFD0`.
It is not tracked by this cut.

## Validation

The feature set was kept fixed at the checked-in Tier 2
`arcweft-cli/native-capture` route with `CARGO_INCREMENTAL=0` and two Cargo
build jobs.

```bash
just test-slow-mcp
# 22 passed; 0 failed

just test-tier2
# slow MCP: 22 passed; 0 failed
# slow Agent observe: 1 passed; 0 failed
# native auxiliary capture: 16 passed; 0 failed
# visual smoke and exact golden: 7 passed; 0 failed

cargo test -p arcweft-agent-protocol -p arcweft-agent-mcp --all-targets --quiet
# 54 passed; 0 failed

just test-cli-native
# visual smoke: 2 passed; 0 failed
# exact shared-renderer dialogue layer capture: 1 passed; 0 failed

cargo clippy -p arcweft-cli --features native-capture --test check -- -D warnings
# passed

cargo fmt --all -- --check
# passed

cargo +nightly -Zscript tools/structure-audit.rs --root .
# 3,462 files; 1,804 Rust files; 829,611 physical Rust LOC
# 94 manifests; 0 errors; 131 warnings
```

The first isolated cold build took 22 minutes 34 seconds before the MCP tests
ran. The complete Tier 2 recipe took 268.7 seconds on the first base and 904.9
seconds after rebuilding the affected dependency graph on `118a9870`. These
timings are workspace-local build costs, not changes to the runtime contract.

## Remaining work

There is no dialogue/Agent Tier 2 harness repair remaining at this revision.
Later broad changes that touch runtime, authored View projection, Agent/MCP,
capture, or public resource identity must rerun `just test-tier2`; this record
is commit-bound evidence, not a permanent waiver.
