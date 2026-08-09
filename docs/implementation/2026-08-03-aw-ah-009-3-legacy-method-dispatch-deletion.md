# AW-AH-009.3 legacy method dispatcher deletion

Date: 2026-08-03

Status: `VALIDATED_WITH_KNOWN_FSERROR_FIXTURE_FRONTIER`

Inspected baseline: `15f1c78171207450ad8a16a99a3aec877227f50c`, which was
both local `main` and `origin/main` before this cut.

The main checkout contained protected Proof, Unit-return, grammar, and
implementation-note work. This cut was validated in a detached worktree that
contained only the five Rust paths listed by the commit plus this note. The
validated Rust files matched the selected main-checkout content exactly; the
unstaged Unit-return hunk in `tests/typecheck.rs` was not admitted.

## Deleted production path

This cut physically removes the checker-owned method success path rather than
repairing or wrapping it:

- `well_known_capacity_method_type` is deleted;
- `is_reservable_type` is deleted;
- the early `check_inherent_method_call` branch that returned its result is
  deleted; and
- no renamed string helper, compatibility reader, fallback nominal, or second
  resolver replaces them.

String and collection methods now reach the registered callable inventory and
shared resolver. The acceptance evidence checks exact selected
`CapacityMethodId` identities for `trim`, `to_string`, `pop`, `pop_front`,
`collect`, `push`, `reserve`, `shrink`, `shrink_to`, and
`with_capacity`. It also checks exact resolver and authored-argument accounting.
The line-plan fixture uses current `CharacterDialogue` content application and
registered semantic authority instead of extending the removed `.say` test
surface.

Deleting the helper exposed eight detached compiler tests that had depended on
its implicit success. Those consumers now compile through the production
`compile_source` project path. Missing same-group arguments are rejected at
registered type checking; supported calls retain their runtime plan; and
unsupported source-function values or partials retain their runtime-plan
diagnostics. No test-only wrapper around `typed_tree`, old HIR lowering, or
`linked_module` was introduced.

## `old_dispatch_calls` boundary

`old_dispatch_calls == 0` is not used as proof that the deleted helper is gone:
that helper did not increment the counter. Physical deletion plus exact shared
resolver candidate facts are the evidence for this cut.

The counter remains temporarily because its two live increments still measure
the frozen Speaker/SpeakerPreset fallback and the registered function-value
fallback. The counter, its aggregation, and its assertions are deleted with
those two remaining routes in their public authority-switch cut. Removing only
the counter here would hide live obsolete readers rather than remove them.

### Subsequent final deletion

The Proof public-switch working copy has now performed that later deletion.
`FinalSemanticAnalysisWork` has no legacy-dispatch field or increment, the
Speaker/SpeakerPreset and function-value fallback readers are absent, and only
the shared-resolver accounting sealed into `CallTargetFacts` remains. Thus
`old_dispatch_calls == 0` is no longer a runtime assertion or acceptance
counter; physical absence plus exact resolver/candidate facts are the current
evidence. This paragraph supersedes the temporary-state sentence above without
rewriting the historical validation result of the original isolated cut.

## Validation evidence

Passed against the exact isolated cut:

- `cargo fmt --all -- --check`;
- focused registered capacity, instance-method, and line-plan tests;
- `cargo test -p arcweft-lang-sema --all-features`;
- `cargo test -p arcweft-compiler --lib` (`92` passed);
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features`;
- every `just test-workspace` command before the Arcweft fixture suite; and
- the final persistent-cache build golden suite (`2` passed).

`just test-workspace` is not claimed as fully passing. Its Arcweft fixture
command produced `3` passes and the same two pre-existing failures:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both still require publication of the capability-owned `FsError` nominal by
the final attached-HIR public switch. This cut does not add a global
`FsError`, source gate, fixture bypass, compatibility alias, or partial
capability reader.

Tier 2 was not run. The production change is confined to semantic method
selection and does not change a runtime, renderer, Agent, MCP, or capture
implementation path; the workspace compiler and runtime-plan tests provide the
relevant integration evidence.

## Structural audit

The canonical command

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

scanned `4,112` files, `2,244` Rust files, `1,101,706` physical Rust lines,
and `95` manifests. It reported `0` errors and `176` warnings in dry-run mode.
No audit suppression or generated report rewrite was admitted to this cut.

## Remaining boundary

This cut does not claim the Proof public authority switch or AW-AH-009.3 family
closure. The next deletion boundary remains the atomic final-HIR/project
consumer switch, including typed capability-member publication, one accepted
`Arc<HirProject>`, removal of detached readers, and the two `FsError` fixtures.
The protected Unit-return tests and unrelated documentation changes remain
outside this commit.
