# Nominal Variant producer closure — 2026-09-11

Inspected `main == origin/main` at
`368d967af2ded47d213b5dba3c34fcc8525431a7`. The prior record-layout cut is
committed and pushed. Callable/effect and the following nominal work remain
uncommitted; they are preserved in the existing checkout. This note does not
claim a completed C1 cut or a buildable full working tree.

## Established in the working tree

The mandatory nominal Variant layout now flows through core identity, plan
domain seeds/domains and selections, project ownership projection, normalized
project owners, AWBC type/identity encoding and reification, typed constants,
and bundle resource fingerprints. Plan admission rejects a project type/domain
layout mismatch before committing its type batch. Canonical value bytes retain
layout; semantic type identity does not consume that derived hash.

`RuntimeNominalSchemaGraph::accepts_value` now uses the same iterative value
visitor as standalone schema validation. Typed graph references resolve only
against the admitted definition set. Nominal record and Variant headers require
the graph layout; variants also compare their explicit semantic ID, case
coordinate, name and payload presence. Tuple and record payload shapes remain
distinct, including empty forms. Record values still lack a separate stored
semantic ID; their graph layout includes the root semantic identity. The C5
value/restore migration is not claimed by that check.

Layout work uses one operation over the graph's explicitly retained admission
limits, with an ephemeral memo of completed root hashes. Repeated nominal
values do not restart that allowance. A separate value allowance, selected by
the caller, accounts for every logical value and canonical output byte,
including opaque interiors. Both operations are bounded; neither changes the
graph's type authority or persists a second catalog. Root value limits apply
before the graph-root lookup. The digest is returned only after the full visit.

All nominal declarations inside a graph must live in its typed definition set.
An inline tree Record/Enum has no semantic definition key and now rejects at
graph admission, just as a stringly Named reference cannot resolve there.
Anonymous graph record payloads use RecordValue; nominal children use
NominalRef. Standalone serde/tree schemas retain their existing separate API.
This closes the otherwise identity-free nested nominal path in the graph;
it is a clarification of the complete typed-definition authority, not a new
name-based resolver or format compatibility path.

The shared validator also now handles the real one-item tuple used by core
Option/Result constructors. It previously compared the inner scalar schema
directly with that wrapper tuple. Actual constructor-produced positives,
flattened/empty/oversized tuple negatives, inclusive node/depth checks and the
precise nested error path now exercise the representation.

## Producer boundary that remains open

Mandatory propagation does not supply missing canonical producer schemas.
Character/base-environment normalized constructors now require layout input,
but compiler production still needs the corresponding canonical projection.
Duration in the real DropPolicy payload exposes schema coverage beyond the
parent's five additions. Dialogue's Apply policy contains external typed
nominal style values, while its fixed owner lacks their active descriptors.
The data decoder has TypeShape evidence but no layer-correct common schema
projection. A zero, label/semantic digest, fabricated schema or weakened
opaque predicate would conceal these gaps rather than implement the contract.

The required, independently usable correction is
[nominal Variant layout producer and schema closure](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md).
It reconciles existing producer-authority work and the parent's compile-clean
order. The decisions can be resolved from repository evidence; they are not
an external blocker and do not justify marking the convergence goal blocked.
The in-flight Rust cut must not be accepted or pushed until every producer
and consumer is migrated to the selected final authority.

## Validation and limits of the evidence

Logs are under the ignored
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` directory.

- Passed: core all-feature library and integration tests, 444 total
  (411 library and 33 integration), log `nominal-graph-value-core-tests.log`.
  New coverage includes actual graph recursion, variant shape/identity/layout
  negatives, exact graph/value/byte allowances, shared opaque work, strict
  version-1 AWBC layout bytes/truncation and plan admission rollback.
- Passed: the intermediate schema-focused run, 35 tests. The final full core
  run additionally includes the graph opaque/initial-budget test.
- Passed: core all-target/all-feature Clippy, with warnings; formatting of
  changed crates. No warning-free or workspace-wide lint result is claimed.
- Passed: canonical structure audit and blocking gate: 95 packages, 2,295 Rust
  files, 1,273,723 physical Rust lines, 309 review triggers, zero blockers.
- Failed: the first core migration check found seven stale test initializers
  and domain-seed call sites; those were migrated before the passing tests.
- Failed: workspace all-target/all-feature check. The first run stopped at two
  adapter test initializers, which were migrated. The retry reaches missing
  production layout initializers in Dialogue schema and the data decoder.
  It stops upstream of compiler; the compiler Character/BuiltinClosed calls
  also need the new layout argument by source inspection. This is an incomplete
  migration, not a passed check or an accepted baseline.
- Not run for this in-flight state: full workspace tests, workspace Clippy,
  doctests and Tier 2. Previous cut results, including the 28 effect-row-related
  compiler failures and the image-animation sample parse failure, are historical
  evidence and do not become passes for these changed inputs.

## Ownership disposition

Current complete working-tree measures: schema integration 1,163 lines /
37,655 bytes; nominal graph owner 405 / 12,377; graph encoder 213 / 7,112;
value predicate context 536 / 21,480; nominal graph tests 926 / 30,328;
pattern owner 2,836 / 103,172; plan construction 2,834 / 112,410; AWBC type
codec 1,200 / 42,542. These include preserved work outside this continuation.

The graph encoder owns only canonical layout work and its operation-local
memo. The predicate context borrows either the explicit standalone tree or
the validated graph and owns the common value-validation setup. Neither owns
runtime storage or I/O. The large pattern owner only gains the required
physical identity field; its existing semantic transcript deliberately omits
the derived layout. Plan construction gains the existing type/domain join's
layout comparison and rollback test. The AWBC codec extends its existing owner
row and tests, without allocating a new carrier/tag family. These changes keep
the existing dependency directions and introduce no manifest, feature,
transport, persistence authority or independently mutable type catalog.
