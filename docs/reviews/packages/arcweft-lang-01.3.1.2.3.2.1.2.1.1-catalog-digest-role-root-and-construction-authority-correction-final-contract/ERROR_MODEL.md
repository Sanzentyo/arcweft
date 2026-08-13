# Typed error model

Errors are closed enums owned by the admission/construction layer, with deterministic fields. Display text is not a protocol and consumers must not match strings.

| Stable kind | Required context | Failure point |
|---|---|---|
| `MissingRequiredRole` | role | role-root completeness |
| `DuplicateRole` | role, first/second paths | role-root uniqueness |
| `UnknownRole` | ordinal, grammar version | role decode/admission |
| `UnsupportedDigestGrammarVersion` | expected, actual | raw assertion admission |
| `CatalogDigestAssertionMismatch` | role, expected, derived | per-role comparison |
| `RoleRootAssertionMismatch` | expected, derived | root comparison |
| `PlanAwbcRootMismatch` | plan root, AWBC root | pair admission |
| `GenerationAssertionMismatch` | source, expected, derived | generation comparison |
| `WrongGeneration` | operation, expected, actual | handle/value use |
| `StaleConstructionAuthority` | issued generation, active generation | pre-mutation gate |
| `WrongConstructionRole` | expected role, actual role | capability use |
| `WrongProducer` | expected producer, actual producer | external construction |
| `UnknownNominalLayout` | nominal/layout key, role | layout resolution |
| `NominalLayoutMismatch` | expected/actual layout, field path | construction/validation |
| `NestedNominalGenerationMismatch` | value path, expected/actual generation | recursive validation |
| `ConstructedValueValidationFailed` | value path, typed cause | postcondition |
| `CatalogLimitExceeded` | limit kind, bound, observed/work used | bounded canonicalization |

Error precedence is deterministic: malformed/version/limit errors; duplicate/unknown references; typed closure; role digest comparison; root comparison; plan/AWBC pair comparison; generation comparison; capability scope; value validation. Tests freeze precedence where multiple faults are present.

Errors never expose an admitted handle or partially constructed value. Transactional consumers return their prior state unchanged.
