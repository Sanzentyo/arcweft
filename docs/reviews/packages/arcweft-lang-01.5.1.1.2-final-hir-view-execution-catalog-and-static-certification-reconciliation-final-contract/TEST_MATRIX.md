# Normative test matrix index

The authoritative machine-readable matrix is `TEST_MATRIX.csv` with **473** unique rows.

| Prefix | Rows | Scope |
|---|---:|---|
| `API-*` | 20 | compile-fail, visibility, deletion, dependencies |
| `HOT-*` | 10 | hot replacement |
| `IDN-*` | 24 | session versus persistent identity, canonical coordinates, and revision scoping |
| `IMG-*` | 26 | image/resource/animation |
| `INP-*` | 46 | inputs, dynamic fields, layout/modifiers/policies |
| `LIM-*` | 72 | every exact limit and one-over |
| `NEG-*` | 8 | cross-layer failure precedence |
| `PAR-*` | 63 | dynamic/certified cross-backend parity |
| `SAV-*` | 18 | save/replay schema-v2 behavior |
| `STA-*` | 58 | automatic and authored static certification |
| `STR-*` | 40 | View execution structure and identity |
| `T2-*` | 18 | Tier-2 and final repository gates |
| `TAM-*` | 30 | wire/cross-section/certificate tampering |
| `VAL-*` | 40 | ordinary RuntimeValue/AWBC/projection values |

## Global assertions on every applicable row

1. Parse source once and never recover behavior from source strings or spans.
2. Validate accepted HIR/project/resource generation before lookup.
3. Build compiler, bundle, runtime, replacement, and restore candidates in scratch state.
4. A failure commits no partial product, catalog, mount, input, handler, resource, animation, observation, save, or replay state.
5. Reversed deterministic insertion order yields identical canonical bytes/digests.
6. Native, Web, headless, Agent, and MCP observe no static/dynamic path distinction.
7. API/deletion evidence uses typed compilation, constructors, behavior, codecs, Cargo metadata, and structural audit—not source-text grep.

## Required exact-limit method

Each `LIM-*` pair constructs the exact-limit fixture without tripping an earlier limit, then a one-over fixture changing only the named dimension. The implementation note must record the observed charged count and first typed failure.
