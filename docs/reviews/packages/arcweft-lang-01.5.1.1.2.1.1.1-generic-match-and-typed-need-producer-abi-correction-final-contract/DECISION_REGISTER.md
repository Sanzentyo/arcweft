# Exact decision register

| ID | Closed decision | Sole owner / representation | Rejected alternatives |
|---|---|---|---|
| D01 | Selector returns one value | synthetic nominal AWBC Variant | Choice, multi-result, nested pair, register export |
| D02 | Arm representation | case ordinal = source arm; payload = binding tuple | optional payload for zero bindings, sentinel case |
| D03 | Construction and decoding APIs | runtime-plan builder, core verifier, driver decoder/transaction | implicit convention, ad hoc tuple reads |
| D04 | View/core independence | lightweight View coordinates; bundle cross-section | `arcweft-view -> arcweft-core`, copied type table |
| D05 | Selection visibility | private driver scratch; public old type deleted | public RuntimeValue-bearing View type |
| D06 | Selector validation | exhaustive typed validation before transactional install | partial install, best-effort decode |
| D07 | Need carrier | `NeedHandle { payload }` + `RuntimeValue::NeedHandle` | String, opaque source token, second endpoint table |
| D08 | Producer/task relation | one flagged synthetic producer + one task plan | implicit plan lookup, start-on-construction |
| D09 | Carrier lifecycle | dedicated construction, verifier, codec, digest, snapshot, replay, replacement | untyped serde, source reconstruction |
| D10 | Old Need path | strict v1 deletion in atomic consumer switch | compatibility reader, fallback String branch |
| D11 | Semantic/runtime type authority | sema `TypeKind`; one `RuntimeNormalizedType`/`AwbcInventory` projection | inferred `TypeId`, View runtime type map |
| D12 | Match arm identity | owner ExprId + ordinal, exact scope/pattern/guard/value/locals | nonexistent arm expression, source range identity |
| D13 | Ownership | sema `CheckedOwnershipDisposition`; bundle admits snapshot clone only | undefined ownership names, runtime guess |
| D14 | Generic Match authority | `CheckedExpressionResolution::Match(Box<CheckedMatch>)` | Structural Match, View-owned duplicate arms |
| D15 | Resource digest input | borrowed `ResourceTypeRegistry`; existing canonical digest | copied digest computation, stale registry |
| D16 | Implementation order | five compile-clean cuts, final atomic switch | empty catalogs, dual authorities, scaffolding |
| D17 | Guard semantics | explicit pattern/bind/guard/Branch chain | `AwbcMatchArm.guard`, View VM, guard omission |

Every row is normative. There are no unresolved alternatives.
