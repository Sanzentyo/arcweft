# Lang-01.5.1.1 checked dialogue-profile admission

Date: 2026-07-22

## Outcome

The compiler now admits a launch-selected `DialoguePresentationProfile`
against the exact compiler-owned `ValidatedViewProduct` before line-task and
runtime-plan lowering.

`CheckedDialogueProfile` retains:

- the selected `ProfileId` and typed presentation value;
- one six-field `DialogueProfileRevision`;
- an `Arc` to the same accepted View/Style product retained by
  `CompiledProject`; and
- exact source provenance for the selected View and optional Style sheet.

Admission re-resolves the selected profile from the same
`SourceBackedManifest`, requires the exact resource-registry `Arc`, checks the
registry digest, verifies View/Style product source revisions, resolves the
nominal View and optional sheet, requires the canonical dialogue parameter
role, and rejects missing provenance. It emits source-bound diagnostics for a
missing View, a non-dialogue View, a missing Style, or an incoherent revision.

Profile-based CLI compilation now passes the exact manifest, resolved profile,
topology revision, and resource registry from `LoadedProfileTopology` into the
compiler context. No manifest path, TOML text, detached reader, or defaults ID
is passed across this boundary.

`ViewStyleSheetId` gained its owned `as_str`/`Display` behavior so compiler and
diagnostic consumers do not grow local string projection helpers.

## Completion boundary

This is the catalog-aware admission cut. The checked value is not yet the
runtime-plan presentation owner, so the existing source `dialogue defaults`
path has not been deleted in this cut. The next atomic authority switch must:

- replace raw runtime-plan defaults/options with the checked typed profile and
  revision;
- materialize the profile View/Style/inline-failure values in every line;
- move save/reload and replacement integrity to the same revision; and
- delete `DialogueDefaultsItem` and every syntax/HIR/sema/tooling/runtime
  consumer without a compatibility reader or removed-spelling diagnostic.

No profile admission fallback exists after catalog checking begins. Omitted
profile fields resolve before admission to `std.view.dialogue`, no profile
sheet, and `FailLine`.

## Verification

- `cargo check -p arcweft-compiler --lib --tests`: passed;
- `cargo test -p arcweft-compiler --test dialogue_profile_admission --quiet`:
  4 passed;
- `cargo check -p arcweft-cli --lib`: passed;
- `cargo clippy -p arcweft-compiler --lib --tests --no-deps -- -D warnings`:
  passed.
