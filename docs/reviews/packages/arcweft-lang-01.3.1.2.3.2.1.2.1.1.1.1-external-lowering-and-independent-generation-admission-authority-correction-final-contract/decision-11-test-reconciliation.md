# Decision 11 — contradictory tests are replaced

`TEST_MATRIX.csv` is the sole normative matrix. It starts from the retained
parent matrix, replaces every contradictory row, and adds focused rows for the
new authority boundary.

The replacements are exact:

- all 35 actual AudioCommand value fields are mandatory effect-owned sites;
- `EffectPlan::AudioValue` is a compile-fail/nonexistent-variant assertion, not
  a positive site;
- Option `None` has no payload child, no path push, and no index-overflow test;
- other absent/nonnumeric edges test absence or wrong-shape behavior, never a
  fabricated numeric overflow;
- opaque generic validation tests atomic owner-only behavior and zero payload
  work/depth/path effects;
- the real external lowerer is a compile-pass case; bypass surfaces are
  compile-fail or runtime decode failures;
- every checked and operational expression family has root `[0]` evidence;
- raw plan/AWBC self-tamper cannot alter the independent generation;
- every implementation phase has a compile row and a same-phase deletion row.

`TEST_MATRIX.md` records the generated counts and rejection scans.
