# Package validation

## Commands

From the extracted package root:

```text
python tools/validate_package.py .
```

From the directory containing the final archive:

```text
python tools/validate_package.py \
  arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract.zip
```

Optional syntax check:

```text
python -m py_compile tools/validate_package.py
```

## Validator checks

The validator fails unless all of these hold:

1. safe directory/ZIP shape: one expected root, no duplicate path, traversal,
   absolute path, symlink, or encrypted entry;
2. required human, machine, CSV, input, manifest, and validator files exist;
3. `OPEN_QUESTIONS.md` is exactly the four bytes `none`;
4. package name, full inspected Git SHA, design-only status, and version marker
   are exact;
5. all Arcweft-owned version markers are exactly `1`;
6. current request/Rust skill/project premise byte sizes and SHA-256 values are
   exact;
7. request Git blob identity is exact;
8. all identity domain bytes are exact and NUL-terminated;
9. policy truth table is Join tag/ordinal 0 and Always tag 1/start 1;
10. producer families cover all nine closed rows;
11. fixed zero-invalid and zero-valid policies are explicit;
12. the sole RuntimeValueDigest owner and Tuple([]) empty rule are exact;
13. current View owners are `ViewProgramId` and
    `AcceptedViewProgramRevision([u8;32])`;
14. ownership context excludes ResourceTypeRegistry;
15. five cuts are exact and Cut 5 is indivisible;
16. no forbidden machine flag is admitted;
17. every decision is `CLOSED` and minimum decision count is met;
18. every mandatory traceability ID exists exactly once and is `CLOSED`;
19. source evidence rows include full Git blobs and numeric line ranges;
20. ownership matrix covers the exact expected current TypeKind variant set;
21. test matrix includes positive, negative, property, tamper, differential,
    exact-limit, one-over, rollback, structural, and Tier-2 rows, plus all
    specifically required semantic scenarios;
22. deletion matrix covers all old carrier/admission/persistence routes;
23. manifest lists every payload except `MANIFEST.json` and
    `MANIFEST.sha256`, with exact byte count and SHA-256;
24. `MANIFEST.sha256` equals the SHA-256 of `MANIFEST.json`; and
25. human documents contain required exact domains/schemas/absence statements.

## Deliberate failure cases

The validator is explicitly tested to fail when a temporary copy is modified
to contain:

- `OPEN_QUESTIONS.md` other than exact `none`;
- a version marker other than `1`;
- a stale request copy;
- an omitted mandatory traceability row;
- an unresolved decision;
- policy/identity conflation in machine policy rows;
- current-View owner mismatch;
- generic/View admission conflation flag;
- missing opaque evidence fields;
- incomplete TaskEvent correlation declaration;
- Cut 5 marked divisible or persistence delayed;
- an added compatibility/dual/String/suffix flag;
- vague source evidence without path/range/blob/observation;
- an altered payload or manifest hash; or
- an unsafe ZIP entry.

## Production validation

The validator does not inspect or mutate production. The required implementation
commands and focused tests are normative in `TEST_MATRIX.md` and
`COMPILE_CLEAN_SEQUENCE.md`.
