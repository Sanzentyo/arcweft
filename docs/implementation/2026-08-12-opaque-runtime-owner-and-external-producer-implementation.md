# Opaque runtime owner and external producer implementation evidence

Date: 2026-08-12

Inspected Git baseline:
`f7572b0a54af4876c75e8bddf333dc0caa17eb61` on `main`, equal to
`origin/main`. Implementation and validation were performed in the preserved,
unstaged working tree rooted at that commit.

Relevant returned packages:

- `Lang-01.3.1.2.3.2.1.1` opaque composite checked-type owner
  reconciliation; and
- `Lang-01.3.1.2.3.2.1.1.1` external opaque producer declaration authority
  correction.

## Implemented state

The working tree completes the A1.1 and A1.2 production gates and the external
producer declaration correction:

- `arcweft-core` owns producer IDs, exact/producer-wide opaque owners, exact
  opaque values, inherent checked-type acceptance, canonical runtime-value tag
  16, nesting, and nominal layout evidence;
- `arcweft-lang-sema` publishes producer-bearing accepted nominal facts,
  preserves producer evidence through instantiation and substitution, and
  projects complete recursive runtime checked types without the deleted Named
  or selected-case fallbacks;
- `arcweft-dialogue` owns CharacterDialogue exact/Any opaque owners and its
  validated encode/decode boundary;
- `arcweft-runtime-plan` and the compiler use complete variant owners and typed
  projection paths, including both Result/Option branches and exact nominal
  layouts;
- adapter manifest, adapter metadata, project loading, Rust ABI, derive input,
  adapter sema, generated source, and catalog publication carry one mandatory
  external producer declaration; and
- obsolete free matchers, producerless accepted opaque rows, duplicate
  CharacterDialogue type ownership, and name/digest/selected-case runtime
  fallbacks are deleted.

The dependency-direction test was reconciled with accepted commit
`ab9c7942f`: `arcweft-lang-sema -> arcweft-core` is the intentional lower
Sans-I/O dependency. The reverse dependency and dependencies from sema into
project/loader/host-I/O crates remain forbidden.

## Design precedence and deviations

The repository-wide version policy in `AGENTS.md` is authoritative over the
returned package's proposed increments. Every Arcweft-owned schema, codec,
wire, protocol, ABI, save/snapshot, cache, digest-domain, and generated-source
marker remains fixed at `1`. The producer-bearing and opaque-bearing shapes
replace the unreleased earlier shapes directly. No V2/V3 model, legacy reader,
compatibility branch, or old-version writer was retained.

The accepted nominal catalog digest retains its version-1 domain and canonical
key order, adds producer-bearing semantics, and excludes source spans according
to the correction's explicit shared digest rule.

## Structural review

The structure audit found no blocking trigger. The reviewed large owners remain
cohesive:

- `arcweft-runtime-plan::semantic_facts` owns normalized runtime semantic facts,
  recursive projection paths, and checked variant selection; it owns no I/O or
  runtime state;
- `arcweft-compiler::lower` remains the exhaustive HIR-to-runtime-plan lowering
  boundary; and
- `arcweft-lang-sema::env::nominal` remains the accepted nominal catalog,
  construction, collision, and digest authority.

No reverse layer dependency, copied producer side table, parallel matcher, or
source-string reconstruction was introduced.

## Validation performed and passed

- `cargo fmt --all` and `git diff --check`.
- `CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
  --jobs 4`.
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features
  --jobs 4 -- -D warnings`.
- `cargo test -p arcweft-core --all-targets --all-features`: 277 unit tests,
  1 public-API compile test, 8 direct-suspension tests, 2 assertion tests, and
  11 runtime-ID tests passed.
- `cargo test -p arcweft-lang-sema --lib --all-features`: 188 passed.
- `cargo test -p arcweft-compiler --lib --all-features`: 51 passed.
- `arcweft-runtime-plan`: 26 unit, 1 public-API compile, 10 assertion, 59 AWBC
  parity, and 3 iterator tests passed.
- adapter-context: 20 unit and 1 public-API test passed; adapter-metadata: 3 unit
  and 8 codec tests passed; adapter-sema: 11 passed; adapter-desktop: 6 unit and
  1 integration test passed.
- dialogue: 31 unit and 4 rich-text tests passed; manifest-model: 16 passed;
  Rust ABI: 11, build: 2, and derive macro compile-fail/export: 1/3 passed.
- project-loader: 146 unit tests passed; dependency direction: 4 passed.
- `just structure-audit` and `just structure-audit-gate`: 2,159 files, 2,031
  Rust files, 1,005,195 Rust LOC, 95 packages, 183 review triggers, and zero
  blocking findings.

## Validation failed

- The full LSP test run passed 205 of 213 tests. Eight existing presentation
  fixtures fail because compiler lowering reports that presentation callable
  `Show` requires the pending typed Presentation command ABI. The same blocker
  exists at the inspected baseline and is not an opaque-producer or version
  regression.
- `just verify` did not complete: Windows returned OS error 1455 (paging file
  too small), followed by memory-map failures and cascading `rustc` failures.
  Workspace check and Clippy passed separately after that run.
- `just test-tier2` stopped in its first slow MCP target: 5 tests passed and 17
  failed. Native observation fixtures fail to initialize recovered HIR modules;
  direct reproduction reports `hir.project.execution`, consistent with the
  existing pending Presentation command ABI blocker. One rich-text proxy
  fixture additionally reports an existing `syntax.choice.missing_body` error;
  three protocol assertions receive no result after initialization failure.
  Later Tier-2 recipes were therefore not run.

## Remaining work and non-goals

A1.3 and A1.4 are not credited by this note. In particular, the AWBC type and
constant tables do not yet carry opaque rows, and persistence has not yet been
migrated. Those gates will also keep codec, ABI, and save schema markers at
`1`, replacing unreleased shapes directly. The pending typed Presentation
command ABI and unrelated Tier-2 fixture repair are explicit non-goals of this
opaque-owner cut.
