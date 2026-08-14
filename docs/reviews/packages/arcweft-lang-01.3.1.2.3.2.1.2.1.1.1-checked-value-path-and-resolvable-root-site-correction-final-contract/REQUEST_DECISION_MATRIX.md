# Required exact decision matrix

1. **one final checked-value path model** — `decision-01-final-path-authority.md`; exact outputs: RuntimeValuePath; RuntimeValuePathSegment; RuntimeCheckedTypePath; exact modules/derives/API/limits/order.
2. **path migration and Serde grammar** — `decision-02-path-migration-and-wire.md`; exact outputs: single consumer migration; tags 0-10; human/non-human Serde data model; no alias.
3. **checked-type versus value-path push rules** — `decision-03-checked-path-push-rules.md`; exact outputs: all requested composites; index widths; overflow; first-error order.
4. **mechanically resolvable RuntimePlan sites** — `decision-04-runtime-plan-site-authority.md`; exact outputs: typed declarations/wrappers; exact site/slot enums; owner-field table; exclusions; AdmittedRuntimePlan API in ADMISSION_AND_PAIR_API.md.
5. **mechanically resolvable AWBC sites** — `decision-05-awbc-site-authority.md`; exact outputs: typed constants/patterns; named slot enums; reference resolution; bounds/duplicates/alias/cycles; AdmittedAwbcProduct API in ADMISSION_AND_PAIR_API.md.
6. **tamper-proof plan/AWBC correlation** — `decision-06-independent-root-correlation.md`; exact outputs: independent generation facts; delete root-use rows; coordinate-only origins; direct equality transcript; same-parent pair wrapper and product-step cut in ADMISSION_AND_PAIR_API.md.
7. **compile-clean validator/generation order** — `decision-07-compile-clean-context-order.md`; exact outputs: AdmittedRuntimeGeneration then sealed context then validator; exact APIs/delete order; exact admission/context/pair construction order in ADMISSION_AND_PAIR_API.md.
8. **inventory and tests** — `decision-08-inventory-and-tests.md`; exact outputs: updated inventory/test matrices; removed conceptual rows; tamper cases.
9. **complete outer shapes and nominal evidence** — `decision-09-outer-shapes-and-nominal-evidence.md`; exact outputs: all current RuntimeValue variants; bytes-as-sequence; admitted descriptor semantic source.
10. **bounded Serde for RuntimeIndexPath/newtypes** — `decision-10-newtype-serde-and-wire.md`; exact outputs: nonempty/bounded custom Deserialize; private wire DTOs; no derived bypass.
11. **layer-correct catalog bridge** — `decision-11-catalog-admission-bridge.md`; exact outputs: dialogue-owned bridge; core provenance token; no free generation scalar; custom-field View source.
12. **current-world role issuance** — `decision-12-role-issuance.md`; exact outputs: TypeCheckEnv + AcceptedNominalWorld + registrar atomic registry; no AcceptedNominalEnvironment.
