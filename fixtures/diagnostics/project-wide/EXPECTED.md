# Expected diagnostic fixture behavior

This fixture is intentionally small and is meant to be copied into a temp project test.

- `src/main.arcw` is the root module and should compile far enough for a project smoke path.
- `src/routes/opening.arcw` contains `flow @flow.opening start`, which is intended to trigger syntax lint `AWF0102` (`identity::decl_binding_mismatch`) when that module is loaded by project discovery/import logic.
- A future regression test can add a parse-error variant by editing `src/routes/opening.arcw` to an incomplete declaration and should expect `syntax.parse` with a source snapshot.

The package does not claim this fixture was executed in the packaging container. See `verification/VALIDATION.md`.
