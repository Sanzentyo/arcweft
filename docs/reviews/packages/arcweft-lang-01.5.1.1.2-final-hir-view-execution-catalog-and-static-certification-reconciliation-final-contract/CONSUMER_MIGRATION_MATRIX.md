# Consumer migration matrix

| Consumer | Current boundary | Final boundary | Required evidence |
|---|---|---|---|
| syntax View/attribute/attached body | retain current ownership | add #[static] only at C7 | parser/API/malformed/source-role matrix |
| HIR View item/expr/member/scope/source | retain IDs and generation | carry static attribute role; no execution schema | HIR identity/generation tests |
| sema final analysis | four-way call classification | complete catalog, dependency/static result | catalog completeness and exact type/effect tests |
| compiler View lowerer | literal/dialogue-only and generic fallback | catalog-driven all-shape lowerer | replace view_product matrix |
| compiler source map | ordinal synthetic IDs/ranges | stable node/instruction and HIR role mapping | source and generated artifact tests |
| arcweft-view program | Fx-backed value inventory; no Match | IDs/instructions only; Match on owner enum | API and runtime instruction tests |
| bundle ViewProgram | static-only fields and optional bindings | strict native constant/program bindings; required AWBC refs | roundtrip/tamper/one-over |
| bundle ViewText | string projection/local | typed source or RuntimeProgram | text/RichText/dialogue parity |
| bundle Input/Style | static controls/layout policies | exact member contract + binding | all control/property matrix |
| image/resource product | static image object/ID assumptions | ResourceRef<Image> exact triple/digests | still/animated/mismatch/stale |
| runtime value path | Fx scalar placeholder evaluator | ordinary AWBC RuntimeValue + exact projection | all value families |
| runtime frame | string targets and static defaults in affected paths | typed stable identities and transactional bindings | native/Web/headless parity |
| runtime replacement | program-only assumptions | program+cert+resource candidate validation | tamper/stale/no partial publication |
| session save | schema v2 exact runtime snapshot | unchanged schema; revalidate against bound final artifact | save/replay parity and mismatch |
| Agent/MCP | existing shared observation | same canonical frame, no static-path distinction | redaction/parity |
| generated artifacts | existing artifact binding | include final View semantic/certificate digests | stale binding rejection |
| tests | 7 stale compiler cases | complete final-HIR matrix | this package TEST_MATRIX.csv |
