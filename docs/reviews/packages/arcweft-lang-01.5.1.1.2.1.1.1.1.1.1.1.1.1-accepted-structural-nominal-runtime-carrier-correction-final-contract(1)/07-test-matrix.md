# 07. Test matrix

Every row names the layer, fixture, expected assertion, and regression caught. Test names are proposed and should be placed next to the actual owner modules discovered in `01-evidence-basis.md`.

| ID | Layer | Fixture / action | Required assertion |
|---|---|---|---|
| T1 | unit/carrier | construct structural carrier with matching shape/payload | sealed carrier, correct class/shape/payload |
| T2 | unit/carrier | structural constructor with mismatched payload shape | exact `PayloadShapeMismatch` |
| T3 | unit/carrier | construct nominal with declared representation | sealed nominal, exact instance and representation |
| T4 | unit/carrier | nominal with equal-looking but undeclared representation | exact `NominalRepresentationMismatch` |
| T5 | unit/carrier | dangling payload handle | exact `DanglingPayload` |
| T6 | match | structural S vs structural S/Direct | accepted identity projection |
| T7 | match | structural S vs nominal N | rejected, no nominal synthesis |
| T8 | match | nominal N vs same N | accepted |
| T9 | match | nominal N1 vs N2 with same shape | rejected |
| T10 | match | nominal N<A> vs N<B> | rejected by canonical generic args |
| T11 | match | nominal N repr S vs structural S with valid witness | accepted, expected projection steps |
| T12 | match | same without witness | rejected |
| T13 | match | witness names another nominal source | typed stale/invariant error at validation, never arm fallback |
| T14 | match | opaque nominal vs structural pattern | rejected unless contract explicitly emits witness |
| T15 | alias | transparent alias and canonical target | same structural/nominal key after checked normalization |
| T16 | newtype | two newtypes with identical representation | distinct nominal instance keys |
| T17 | coverage | static constraint table and runtime plan | identical semantic digest |
| T18 | coverage | mutate one witness/constraint in serialized fixture | load rejects digest mismatch |
| T19 | transcript | every admission matrix row | stable class/constraint/witness/outcome fields, no pointer data |
| T20 | codec | encode structural carrier golden vector | exact canonical bytes |
| T21 | codec | encode generic nominal carrier golden vector | exact canonical bytes and declaration-order args |
| T22 | codec | nonminimal varint / unknown flags / trailing bytes | typed rejection before allocation/publication |
| T23 | codec | duplicate identity-bearing field in extensible framing | `DuplicateField` |
| T24 | restore | unknown catalog/shape/nominal/payload keys | distinct typed errors |
| T25 | restore | representation mismatch after key resolution | reject entire staged batch |
| T26 | restore | valid live→snapshot→restore | semantic equality and identical re-encoding |
| T27 | restore | restored carrier under same match plan | same selected arm/transcript outcome |
| T28 | coordinator | one invalid carrier in multi-task batch | no task/handle/wakeup published |
| T29 | determinism | random insertion and worker scheduling permutations | identical snapshot carrier bytes and plan digest |
| T30 | property/fuzz | arbitrary valid carrier records | decode(encode(x)) semantic round trip; no panic |
| T31 | property/fuzz | arbitrary byte strings under size cap | decoder never panics/over-allocates; canonical errors only |
| T32 | compile/lint | implementation inspection | no extension trait/side-table workaround; owner enum has inherent behavior |

## Required command gates after implementation

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Use the repository's narrower mandated commands from the applicable `AGENTS.md` in addition to, not instead of, these gates. Golden byte tests must pin the format version. Property tests must cap recursion, generic argument count, key length, and aggregate allocation.
