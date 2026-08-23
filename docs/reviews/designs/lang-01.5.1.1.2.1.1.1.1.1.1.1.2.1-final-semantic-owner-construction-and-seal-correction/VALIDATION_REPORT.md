# Validation report

## Scope

Documentation-only design resolution against Git commit
`300e824eea6740eab0ae708508cce00a1bd49435`. No production Rust, Cargo
manifest, test, fixture, branch, worktree, commit, push, or returned ZIP was
created or changed.

## Performed and passed

- verified initial clean `main == origin/main` at the full SHA above;
- confirmed the design directory did not exist before authoring;
- inspected the maintained request, accepted parent C1/C2/C3 design, and the
  current source owners listed in `SOURCE_EVIDENCE.md`;
- checked the required member set, `READY_FOR_IMPLEMENTATION`, and exact
  `none` open-question content;
- checked Markdown whitespace with `git diff --check`/new-file checks;
- checked every relative Markdown link in the design and updated request; and
- audited each proposed new domain byte literal for exact repository equality.
- reconciled the nominal projection budget split with existing
  `NominalResolutionLimits` and `NominalAggregationLimits`;
- audited every current prepared/published fact-family owner required by the
  exhaustive projection request visitor; and
- confirmed `env/nominal.rs`, not a TypeCheckEnv map, owns accepted exact
  records, catalog digest, lookup, and instantiation.
- confirmed the amended schemas define cancellation, per-root/project limits,
  arithmetic overflow, identity mismatch, missing-cache failure, retained
  `TypeShape`, exhaustive request inventories, borrowed post-seal lookup, and
  private environment field construction.

The proposed domains are:

```text
arcweft.lang.accepted-project-item.v1\0
arcweft.lang.accepted-variant-case.v1\0
arcweft.lang.accepted-record-field.v1\0
arcweft.lang.accepted-environment-field.v1\0
arcweft.lang.accepted-character-look.v1\0
arcweft.lang.effect-semantic.v1\0
arcweft.view.specified-value-semantic.v1\0
```

The first five already occur only in the accepted parent design as proposed
domains and do not occur as a production digest domain. The final two are new
and have no exact existing repository match. The removed View-modifier domain
is not implemented or registered.

## Failed

None.

## Blocked

None. All result-changing choices in the request are closed.

## Not run

- Cargo format/check/test/Clippy, workspace tests, Tier 2, and structural gates
  were not run because this cut changes documentation only.
- The request's returned-archive validator and negative corpus were not run;
  no external returned archive is being accepted by this repository-local
  design task.
- Production implementation tests listed in
  `CUTS_TESTS_AND_DELETION.md` remain implementation acceptance criteria, not
design validation evidence.
