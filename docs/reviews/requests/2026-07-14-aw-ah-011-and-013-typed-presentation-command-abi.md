# Request: AW-AH-011 and AW-AH-013 typed presentation command ABI

Date: 2026-07-14

## Request status and independence

This standalone request covers the shared typed-boundary failure behind
AW-AH-011 and AW-AH-013. The accepted evidence is from revision
`4204d25965129ced50abe82cf5de67d528b483d0`; implementation targets the current
checkout. It defines identity, validation, defaults, AWBC, and driver
consumption after the separate AW-AH-012 canonical-alias cleanup has landed.

## Findings and evidence

### AW-AH-011: typed HIR becomes strings and is reparsed (P1/high `confirmed_adhoc`)

- `crates/arcweft-core/src/effect.rs:327-330` represents `RuntimeCall` as a
  callee `String` plus `Vec<String>` arguments.
- `crates/arcweft-runtime-plan/src/flow/presentation.rs:15-109` converts typed
  HIR arguments back to source-like labels and builds `"name = value"` text.
- `crates/arcweft-runtime-driver/src/display.rs:358-770` reparses names,
  separators, quotes, PublicIds, numbers, units, and enums.
- `crates/arcweft-runtime-driver/src/presentation_handles.rs:1-600` does the
  same for handle lifecycle commands.

The path is typed HIR -> source-like strings -> AWBC constants -> a driver-local
grammar. A string containing ` = `, a quote, an alias, or a changed source label
can alter runtime meaning.

### AW-AH-013: malformed values become valid state (P1/high `silent_fallback`)

- `crates/arcweft-runtime-driver/src/display.rs:395-426` turns invalid viewport
  dimensions into 1280x720 defaults.
- `crates/arcweft-runtime-driver/src/display.rs:519-650` synthesizes missing
  image IDs and defaults malformed fit, depth, opacity, visibility, playback,
  alignment, and transform values.
- `crates/arcweft-runtime-driver/src/display.rs:650-770` clamps or defaults
  malformed and overflowing numeric input.

Examples include `design_width="oops"` becoming 1280, `opacity="oops"`
becoming 1.0, out-of-range alignment becoming a clamped valid value, and bad
playback rate becoming 1.0. Omission and invalid authoring are indistinguishable.

## Established substrate that must be preserved

- `arcweft-core` is Sans I/O and already owns typed runtime effects such as
  wait and audio commands. A presentation command belongs in a similarly typed
  runtime boundary; filesystem/window/GPU behavior remains in adapters.
- Ordinary external host calls may still need a generic host-call model. This
  request removes presentation semantics from `RuntimeCall`; it does not make
  every plugin/host call a closed core enum.
- AW-AH-012 is a preceding implementation-ready cut: retain one canonical
  command/argument spelling, delete compiler/runtime alias readers, and reject
  removed spellings through ordinary structured compile diagnostics. This is
  fixed substrate for this request, not a design subject.
- HIR/sema already resolve call structure and types. Runtime-plan must lower
  those checked identities directly instead of recreating source text.
- AWBC already has deterministic effect/constant inventories and a VM/host
  boundary. Extend the typed schema deliberately; do not bypass AWBC with a
  side channel.
- Existing presentation handle/state, bundle image object, viewport fit,
  placement, transform, playback, lifetime, and PublicId types are candidate
  owners. Reuse or move behavior onto their original types when dependency
  direction permits rather than projecting fields into duplicate structs.
- Native, Web, headless, and Agent presentation paths converge on shared
  runtime/renderer data. The typed command must not fork backend semantics.
- Native-only Style and implemented AW-AH fixes are settled substrate; reuse
  their shared typed IDs but do not redesign them here.

## Design objective

Define a closed, versioned, typed presentation command contract from checked
language calls through runtime-plan and AWBC to the runtime driver. Source
spellings resolve once in compiler/sema. Required, optional, and defaulted
fields are explicit; malformed values become source-aware errors; the driver
performs exhaustive command dispatch without parsing authoring strings.

## Required design decisions

1. Inventory every presentation operation currently reaching the driver and
   group it by domain: viewport set/clear; image object/background operations;
   View/image/menu/overlay mount; handle create/show/hide/update/dispose; and
   any owner/lifetime cleanup command. Identify ordinary host calls that remain
   outside this enum.
2. Choose the core embedding, for example a dedicated
   `LineEffectRequest::Presentation` carrying a closed `PresentationCommand`.
   Explain capability/effect classification and why presentation is or is not
   a host-call capability.
3. Define exact command variants and payload records. Avoid one universal map
   of optional fields. Operations with different invariants should have
   different constructors or variants.
4. Define typed identity for handles, resources, owners, targets, layers,
   assets, images, Views, menus, and overlays. Decide when a shared `PublicId`
   is sufficient and when a family-specific newtype prevents cross-family use.
5. Define value/unit types and checked ranges for dimensions, position, depth,
   opacity, alignment, duration, playback rate, transforms, visibility, fit,
   scale policy, and lifetime. Specify fixed-point versus integer encoding and
   prohibit non-finite intermediate representations.
6. Produce a required/optional/default matrix for every field. Defaults are
   applied exactly once by a typed constructor or checked lowering context and
   only when a field is absent. A malformed present value never selects the
   default.
7. Decide which arguments may be dynamic runtime expressions. Specify how
   typed expressions are evaluated into checked command values and where a
   dynamic type/range error is reported without reverting to source parsing.
8. Define handle identity generation and lifecycle. State when IDs must be
   authored, when compiler-generated IDs are permitted, how they are stable,
   and why asset-name-based runtime synthesis is or is not allowed.
9. Consume the canonical command and argument identities fixed by the
   AW-AH-012 cut. Define their mapping to typed command variants without
   reopening source spellings or putting names in the runtime ABI.
10. Treat the provisional string payload as unreleased. The corrected typed
    format is the initial supported contract; do not preserve a dual reader or
    bump a version solely to memorialize the discarded representation.
11. Define AWBC encoding: effect discriminant, command discriminants, payload
    tables/constants/registers, canonical ordering, size limits, and handling
    of unknown command/version values. Do not encode a typed payload as JSON or
    `"name = value"` constants.
12. Define VM/host delivery and runtime-driver errors. The driver consumes
    validated commands exhaustively; it may reject unavailable resources or
    illegal lifecycle transitions but must not repair malformed authoring.
13. Define save/replay/debug/observation implications. State which command or
    resulting state is persisted, how deterministic replay works, and whether
    diagnostic provenance survives into traces without retaining raw argument
    strings as runtime truth.
14. Define command/per-field source provenance and its bounded encoding when it
    crosses AWBC, plus resource limits for strings, matrices, identifiers,
    payloads, lifecycle inventories, and decode allocation.

## Ownership and layer constraints

- Language builtin spelling and type checking belong to syntax/HIR/sema.
- `arcweft-core` owns the host-neutral command/value contract and remains
  Sans I/O. AWBC owns its deterministic executable encoding.
- Runtime-plan owns checked HIR-to-command construction and source diagnostics;
  it must not call `expr_label` to encode presentation meaning.
- Runtime driver owns state transitions and resource availability checks, not
  authoring grammar, quote removal, unit parsing, or defaults.
- Bundle/presentation types should own inherent validation/conversions when
  dependency direction allows. Avoid public `foo_to_bar` projections and
  extension traits.
- Native/Web/headless/Agent adapters consume one resulting state and may report
  platform capability errors without changing command semantics.

## Non-goals

- Do not redesign ordinary external host calls, the renderer, View evaluation,
  native Style, image decoding, or window adapters.
- Do not fix the issue by adding stricter parsing to `named_arg`,
  `public_id_arg`, or the existing presentation `parse_*` helpers.
- Do not reopen AW-AH-012 source-name selection or reintroduce an alias reader.
- Do not add a compatibility `RuntimeCall` wrapper or accept both typed and
  string presentation commands.
- Do not add an automatic migration command unless a separately authorized
  task demonstrates released source/artifact compatibility requirements.
- Do not synthesize valid state after a decode/type/range error.

## Migration order

1. Land the independent AW-AH-012 cleanup, leaving one canonical source
   spelling per command/argument and no compiler/runtime alias reader.
2. Complete the AW-AH-009 nominal identity decision before command payload
   identity or its ABI is frozen.
3. Publish the command/field/default/range inventory using those fixed
   canonical semantic identities.
4. Add final core command/value/error types and their canonical AWBC schema,
   including decode limits and tamper tests.
5. Add VM/host transport and typed driver dispatch. Keep it unreachable until
   producers migrate; do not expose a second public runtime reader.
6. Switch sema/runtime-plan producers to construct typed commands directly,
   including typed dynamic values and provenance.
7. Migrate handle cleanup, viewport, image/background, and all mount/update
   paths in dependency order, with focused end-to-end tests at each compiling
   increment.
8. Switch capability inventories, verifier/static effect metadata, replay,
   debug, and observation consumers to the new effect kind.
9. Delete presentation use of `RuntimeCall`, `presentation_create_args`,
   `presentation_handle_call`, `named_arg`, `public_id_arg`, presentation
   `parse_*`, synthesized-ID fallbacks, string fixtures, and old AWBC
   call encoding. No released cut may accept both representations.

## Diagnostics, errors, and codecs

Define structured compiler diagnostics for unknown/duplicate/missing argument,
type mismatch, invalid unit, malformed or out-of-range number, invalid PublicId
family, and illegal dynamic expression. Each needs a stable code and tight
field/source range. Source-name errors remain owned by the AW-AH-012 cut.

Define typed runtime errors for unavailable resource, duplicate handle,
unknown handle, illegal lifecycle transition, ownership/lifetime violation,
and unsupported platform capability. These must be distinct from malformed
AWBC/decode errors and source diagnostics.

The AWBC codec must reject unknown command/version discriminants, truncated or
oversized payloads, noncanonical tables, wrong-family IDs, invalid units/ranges,
duplicate fields/records, dangling constants/registers, and lifecycle payloads
that violate constructor invariants. Required fields remain required; decode
must never apply authoring defaults to a present malformed field.

## Required tests

- Prerequisite evidence proves AW-AH-012 leaves one canonical source identity
  and AW-AH-009 supplies the approved nominal identities. ABI tests begin from
  those resolved semantic identities rather than re-specifying spellings.
- Required, absent optional, present valid, and present malformed fields have
  distinct results for every command family.
- HIR -> runtime plan -> AWBC encode/decode -> VM/host -> driver preserves all
  typed IDs, units, lifetimes, defaults, and source provenance exactly.
- Strings containing ` = `, quotes, escapes, and alias-like text remain data
  and cannot change argument structure.
- Viewport invalid dimensions, image invalid opacity/fit/alignment/playback,
  transform overflow, invalid booleans, wrong-family PublicIds, and missing IDs
  never become defaults, clamps, or synthesized IDs.
- Handle create/show/hide/update/dispose and scope cleanup cover valid and every
  illegal transition deterministically.
- Ordinary host calls still use their explicit generic contract and cannot be
  mistaken for a presentation command.
- AWBC round-trip is deterministic. Tampered unknown version/variant, wrong
  payload kind, truncated/oversized record, invalid range/unit/ID, dangling
  reference, and noncanonical encoding are rejected.
- Capability/verifier inventories classify the new effect correctly.
- Save/replay/debug/observation behavior follows the chosen persistence model.
- Native, Web, headless, and Agent-facing runs reach equivalent presentation
  state or the same typed platform capability error.

Test owned APIs; never add a source gate for helper names or paths.

## Expected output

- Exhaustive typed command inventory and mapping from resolved semantic identity.
- Exact core value/command/error types, constructors, ranges, ownership, and
  dependency direction.
- Required/optional/default and dynamic-expression matrices.
- Normative AWBC/VM/driver ABI, version, limits, and tamper behavior.
- Compiler/runtime diagnostics, provenance, compatibility decision, and a
  migration plan that deletes the string reader.
- End-to-end, codec, tamper, lifecycle, verifier, replay, and cross-backend test
  matrices.

## Acceptance criteria

The design is ready only when every operation maps to one closed variant;
field/default/error behavior is explicit; ordinary host calls remain separate;
AW-AH-012 and AW-AH-009 are treated as fixed prerequisites; and no argument
strings, driver grammar, silent fallback, or dual codec remains.
