# seq06.9c Compositing Golden Promotion Review

This fixture lane is manual-only.

Do not enable default CI enforcement for exact PNG comparison. Promotion must be
performed only on a pinned native GPU environment after the JSON evidence packet
has already been reviewed.

Suggested manual flow:

1. Run the focused capture/evidence JSON tests.
2. Run the ignored exact PNG lane on the pinned host:

   ```bash
   cargo test -p arcweft-takumi-adapter --test compositing_capture_exact_png -- --ignored --nocapture
   ```

3. Compare the generated drift packet with `expected-evidence.json`.
4. Promote PNG baselines only when the JSON evidence and reviewer notes agree
   that drift is caused by an intentional renderer/compositor change.
5. Keep generated PNG files out of this package unless the environment is pinned
   and the promotion note records GPU, driver, OS, and Arcweft commit.

This review package intentionally does not use CPU-rasterized Takumi output as
expected evidence.
