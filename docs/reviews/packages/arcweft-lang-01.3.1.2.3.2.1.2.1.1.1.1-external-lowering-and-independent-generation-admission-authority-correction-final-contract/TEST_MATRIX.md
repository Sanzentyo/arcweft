# Test matrix summary

Total rows: **2021**. Retained-and-corrected parent rows:
**1878**. New focused correction rows: **143**.

Kinds: `boundary`=15, `build`=15, `compile`=14, `compile-fail`=32, `compile_fail`=13, `compile_pass`=4, `cycle/alias`=19, `deletion`=14, `exclusion`=75, `golden`=50, `integration`=23, `mapping`=245, `matrix`=7, `negative`=933, `ordering`=11, `positive`=439, `precedence`=11, `structural`=1, `tamper`=71, `unit`=29.

Mandatory rejection scans performed by the package generator:

- no positive/exclusion AudioCommand row remains command-only;
- no positive `EffectPlan::AudioValue` row remains;
- no opaque checked row requires payload recursion/path/depth/work;
- no Option `None` row fabricates a child/index overflow;
- every plan expression positive row states root `[0]`;
- focused rows exist for the real external lowerer, bypass surfaces,
  independent fact tamper, cross-parent inputs, audio aliases, operational
  roots, generation-first restore, and every compile phase.

The CSV—not this summary—is normative.
