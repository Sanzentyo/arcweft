# Non-goals and prohibitions

- No production code, patch, overlay, migration shim, or compatibility reader.
- No caller-name, source-string, feature, workspace-path, or crate-name gate.
- No public fields, `Default`, unchecked constructors, `Deref`, `into_inner`,
  or alternate raw DTO for a final typed boundary.
- No `RuntimePlan::try_admit`, `AwbcProgram::try_admit`, artifact-derived
  generation, optional authority, or declaration-to-fact conversion.
- No recursive generic validation of an opaque payload; opaque checked values
  are atomic after exact owner validation.
- No `AwbcEffectPlanTypedSlot::AudioValue`, no command-only audio site, and no
  generic numeric slot fallback.
- No missing root row for legal `RuntimeExpr`; operational node kinds are
  explicit and closed.
- No second plan/AWBC correlation digest or serialized root-use map.
- No Arcweft-owned version other than `1`.
- No extension trait or parallel enum used to avoid adding behavior to the
  Arcweft-owned original enum/owner.
