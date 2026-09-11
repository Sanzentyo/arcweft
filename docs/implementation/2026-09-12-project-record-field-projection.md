# Project record field projection — 2026-09-12

Inspected `main == origin/main` at
`723a70b7dc783f130792ede3b33ae04c9a0faacc`. The existing checkout contains
preserved callable, nominal-schema, and Rust ADT migrations. This record
covers only the selected field-name changes, not those migrations.

## Result and scope

`RuntimeProjectRecordFieldProjection` now retains the declaration's typed
`ModuleSegment` name together with its runtime field ID, declaration ordinal,
instantiated type, and semantic type digest. Compiler record emission reads
this complete field relation for both executable fields and layout fields.
It no longer zips it with the persistence `TypeShape` merely to obtain names,
or checks the lengths of those two independently traversed arrays.

The remaining field-coordinate checks stay in place. The projection's private
construction reads the same accepted declaration field that already supplied
its type and ordinal. Names and declaration order therefore have one source
at this boundary. The new regression fixture uses `Named<i64>` with fields
`zeta: T` and `alpha: bool`, covering source order and generic instantiation.
The maintained executable-runtime chapter records the ownership rule.

The selected cut contains three field-name hunks in sema, three consumer hunks
in the compiler, the regression test, that stable paragraph, and this record.
It excludes all graph-admission APIs, Project/Rust nominal ID changes, graph
projection work, and the compiler's existing effect-row changes. No version,
manifest, feature, or dependency edge changes.

This follows the source-order field and single-authority requirements in the
[accepted structural nominal design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/FINAL_DESIGN.md)
and complements the earlier
[checked field-name contract](2026-09-12-record-field-name-contracts.md).
It does not change the selected architecture or claim C1-C6 completion.

## Validation actually run

All commands ran in the preserved dirty checkout. No isolated checkout or
temporary dependency bypass was used.

- Passed: changed-crate formatting; `git diff --check`; selected patch
  applicability and cached-diff review.
- Passed: `just structure-audit-gate`, with 95 workspace packages, 2,324 Rust
  files, 1,282,093 physical Rust LOC, 310 review triggers and zero blocking
  violations.
- Failed during dependency compilation: `cargo check -p arcweft-lang-sema
  --all-targets --all-features` and the exact new sema regression test. Both
  stop at the existing missing nominal Variant `layout` in
  `arcweft-dialogue/src/character_dialogue/schema.rs:83`. The regression test
  did **not** execute; this record claims no sema/compiler behavioral pass.
- Failed: `cargo check --workspace --all-targets --all-features` and
  `cargo clippy --workspace --all-targets --all-features`. Both stop at the
  existing removed `AdapterRustType::opaque_producer` call in
  `arcweft-host-adapter/src/lib.rs:503`.
- Failed during compilation: `just test-workspace` and `just test-doc`, at
  that same host-adapter call. Later recipe commands did not execute.
- Failed during dependency compilation: `just test-tier2` stops in
  `test-slow-mcp`, reporting the host-adapter call and missing Variant layouts
  in Dialogue and `arcweft-runtime-accelerator/src/external.rs:543`. The ignored
  MCP test and later Tier 2 recipes did not execute.

Adjacent core work was also checked in this continuation:
`cargo test -p arcweft-core --all-features` passed **514 tests** (473 unit,
41 integration; zero doctests), and core all-target/all-feature Clippy passed
with warnings. These results validate the exercised core inputs; they do not
substitute for the unexecuted sema/compiler regression.

Cargo commands were sequential, without explicit job counts. Logs and exact
exit codes are retained under the ignored
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` directory, with
`project-field-names-` prefixes. The preceding core runs use
`nominal-schema-core-final-` prefixes. Fetch confirmed the remote base before
staging. The full six-hunk staging patch is retained with those logs.

## Ownership review

Complete working-copy sizes, including adjacent preserved WIP:

| Owner | Physical LOC | Bytes | Classification |
| --- | ---: | ---: | --- |
| sema `final_analysis/nominal_schema.rs` | 2,497 | 101,805 | production projection and sealing |
| its `nominal_schema/tests.rs` | 703 | 23,730 | separate unit-test module |
| compiler `lower.rs` | 7,855 | 332,400 | semantic-to-runtime fact projection |

The sema owner was 2,505 LOC at the inspected commit; this selected change adds
six lines there. Its existing upper trigger remains explicitly reviewed.
The field row owns the coupled name/coordinate/type relation; extracting its
name into a separate catalog would split that authority. Its tests remain in
the existing test-only child module. The other size differences are not
attributed to this cut.

Compiler lowering already owns the dependency inversion from sema facts to
runtime-plan facts. This change deletes its persistence-shape traversal and
consumes the existing semantic field relation. It adds no orchestration,
transport, I/O, cache, or persistent state. The long owner's existing upper
trigger is acknowledged; a separate wrapper for this forwarding operation
would not create a distinct responsibility or improve its state boundary.

## Remaining convergence work

The source schema graph still needs complete compiler/runtime-plan admission,
including all reachable nominal definitions. The new mixed Rust/Project graph
test, generic nominal ID migration, value/program binding, AWBC/restore, and
the rest of the full convergence goal remain outstanding. They are not
accepted by this field-name cut.

The compilation failures above remain required implementation work. Their
producer obligations are tracked by the existing
[nominal Variant/schema closure request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md).
They are repository-resolvable gaps, not external blockers or grounds to
complete the active goal.
