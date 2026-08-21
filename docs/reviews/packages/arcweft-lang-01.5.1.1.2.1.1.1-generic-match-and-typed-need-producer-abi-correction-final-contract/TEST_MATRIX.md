# Test matrix

The executable CSV contains 148 exact rows. It covers positive, negative, tamper, exact-limit, one-over, rollback, differential, structural-absence, persistence/replay/replacement, projection, bundle-codec, and Tier-2 classes.

| Range | Focus |
|---|---|
| T001–T020 | selector values, heterogeneous bindings, guards, malformed results, rollback |
| T021–T040 | generic CheckedMatch completeness/identity and ownership |
| T041–T058 | verifier/AOT differential, structural guard absence, tamper, install rollback |
| T059–T080 | typed Need construction, task relationship, extraction, generation/contract errors |
| T081–T100 | dependency/API/schema absence, String rejection, registry digest, type projection |
| T101–T120 | cross-layer atomicity, compile-clean cuts, final structural deletion, manifest tamper |
| T121–T132 | save/restore/replay/replacement and digest tamper |
| T133–T140 | exact-limit/one-over and Linux/Windows/macOS Tier-2 |
| T141–T148 | compiler projection, typed Need round-trip, dependency and bundle-source-map closure |

## Required test-class counts

- `differential`: 3
- `exact-limit`: 3
- `negative`: 64
- `one-over`: 3
- `positive`: 30
- `rollback`: 13
- `structural`: 24
- `tamper`: 6
- `tier-2`: 2

Every row names the decision evidence it closes. Structural rows must inspect source, public API/AST, bundle schemas/codecs, generated schemas, and maintained fixtures—not merely execute a happy path.
