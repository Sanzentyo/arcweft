# Fixture regeneration excerpt

Seq06.7 makes native exact refresh a reviewed promotion step rather than blind regeneration. Candidate generation must retain the full review packet:

```text
target/arcweft-native-capture-refresh/
  exact-native-golden.environment.json
  <fixture>.png
  <fixture>.observe.json
  <fixture>.imq.json
```

Before replacing a checked-in PNG, compare the candidate against the checked-in reference, inspect the PNG visually, and record the environment fingerprint and before/after metrics in an implementation note. Missing `imq`, missing pinned `MS Mincho`, unsupported backend, or a non-pinned milestone run is an environment blocker, not a PNG-refresh approval.
