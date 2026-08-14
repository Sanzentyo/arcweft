# Site-resolution row classes

`RUNTIME_PLAN_SITE_RESOLUTION.csv` and `AWBC_SITE_RESOLUTION.csv` carry a mandatory `row_class` column.

- `direct_runtime_plan_typed_site` / `direct_awbc_typed_site`: emits one exact final coordinate after actual-owner resolution.
- `independent_type_declaration_prerequisite`: validates the referenced type declaration but is not itself a runtime publication site.
- `coordinate_descent_grammar`: defines a closed descent edge used inside a direct site.
- `indirect_function_frame_signature_invariant`: validates function/signature/frame ownership before a direct `Signature` or `FunctionFrame` site can resolve.
- `indirect_audio_command_reference`: validates `AwbcEffectPlan.audio`; actual typed values are separate `AudioCommand` sites.
- `indirect_owner_invariant`: required reference/type relationship that cannot independently publish a RuntimeValue root.
- `deliberate_exclusion`: current owner carries no closed RuntimeCheckedType publication; the row states why.

Only direct rows participate in the pair transcript. Prerequisite and indirect rows must pass first. Exclusions never synthesize a site.
