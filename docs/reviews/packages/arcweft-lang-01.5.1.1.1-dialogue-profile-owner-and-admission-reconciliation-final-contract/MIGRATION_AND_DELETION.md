# Migration, deletion, and review order

The current main has completed this migration. The order below is the required
reviewable sequence for reconstruction or future refactoring because it avoids
both a dependency cycle and a half-accepted candidate.

## Increment 1 — verify the retained owner substrate

Without redesigning it, verify:

- launch-owned sole decoder/spec/source map;
- dialogue-owned presentation profile and inline policy;
- `ViewId::standard_dialogue()`;
- exact `inline-failure` wire;
- field-wise pure resolution; and
- neutral `arcweft-manifest-model` dependencies.

No production behavior changes are needed in this increment.

## Increment 2 — place the reusable revision below compiler/runtime-plan

Add or retain `DialogueProfileRevision` in `arcweft-dialogue` with all six typed
fields, strict serde, derived exact equality, and codec tests. Do not place it
in compiler and then add a runtime-plan dependency back to compiler. Do not add
a conversion shim or duplicate wire type.

This increment compiles without changing runtime authority.

## Increment 3 — complete compiler-owned View/Style product facts

Ensure one `CompiledViewProduct` exposes the exact immutable product Arc,
complete source revision, View/Style source provenance, program ID/revision,
dialogue capability, and resource-registry digest required by admission.

Add missing behavior to the legitimate owning View/product types rather than
scattered helper matches or extension traits.

## Increment 4 — add the checked profile and one admission function

Implement `CheckedDialogueProfile::try_admit` in compiler with all invariants
and structured source-bound errors. Keep constructors private/compiler-internal.
The compiler may still produce the prior runtime plan until this increment is
green, but it must not publish the new checked profile partially.

## Increment 5 — make `CompiledProject` and runtime-plan require the checked value

Replace raw dialogue defaults/options in `RuntimePlanLowerOptions` and display
plans with the checked profile/presentation/revision. Update codecs in the same
review cut so no product silently drops revision facts.

## Increment 6 — migrate all consumers

Migrate, in dependency order:

1. runtime-plan and bundle codecs;
2. runtime-driver catalog/generation;
3. save/replay and hot replacement;
4. CLI and LSP;
5. native, Web, headless, Agent, and MCP observations;
6. fixtures, examples, snapshots, and Tier 2 expectations.

Every consumer receives the same checked candidate. None reparse.

## Increment 7 — deletion-driven language cleanup

Once the checked path is green, delete directly:

- `DialogueDefaultsItem` syntax node and parser success branch;
- corresponding syntax visitor/AST APIs;
- HIR/sema/tooling/runtime representations;
- `RuntimePlanLowerOptions::dialogue_defaults` and separate inline-policy
  option;
- orphan `@dialogue.*` selectors and diagnostics that presume them;
- fixtures relying on source `dialogue defaults`.

Then fix every exposed consumer. Do not keep a deprecated variant, dead helper,
compatibility parser, alias, or source-gated path.

## Increment 8 — atomic publication closure

Require one complete revision tuple at the publication boundary. Add rejection
and rollback tests before enabling the new generation path. The final switch
must be atomic: there is never a state in which a new manifest profile is paired
with an old View/Style product or resource registry.

## Increment 9 — validation

Run focused tests, workspace checks, Clippy with warnings denied, full workspace
suite, Tier 2, parity suites, codec tests, and structured Cargo metadata checks.
Only after every required tier is green should the migration be considered
closed.

## Prohibited migration shortcuts

- project-loader importing runtime-driver;
- a temporary second catalog that later “converges”;
- a dialogue-only source map;
- raw string IDs or prefix-family tests in compiler;
- local extension traits for Arcweft-owned enums;
- endpoint-named conversion helpers;
- dual readers/dual spellings;
- source grep as an acceptance gate;
- publishing a profile before its product/revision is checked.
