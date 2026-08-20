# Reactive View unary-Need match blocker — 2026-08-21

## Inspected state

- Git baseline: `680b7c42005febeb2a9f9c8b387669b729b7463c`
  (`Execute Await progress observers`).
- Branch and remote: clean `main`, equal to `origin/main` before this
  documentation cut.
- Stable authority inspected:
  `docs/01-language/converged-language-surface.md`,
  `docs/01-language/state-flow-reducer.md`, and
  `docs/03-presentation/view-reactive.md`.
- Accepted package inspected: Lang-01.5.1.1.2 final-HIR View execution contract
  and its mandatory Lang-01.3.1.2.3.1 affine/View ABI correction.

## Outcome

Reactive unary-Need observation cannot be implemented as a local continuation
of the Await observer cut. Current stable language authority requires ordinary
checked `match` in View context and forbids an `AwaitView` surface. Current
production instead retains an unreleased View-product `Await` instruction with
an `I32` selector and four fixed branches: pending, ready, error, and denied.
That model duplicates domain error/denial outside the unary Need payload and
has no typed pattern or subscription owner.

The accepted Lang-01.5.1.1.2 package closes the broader final-HIR View execution
gap and selects `ViewInstruction::Match`, ordinary RuntimeValue/AWBC dynamic
evaluation, and one checked View catalog. However, its direct-await rows predate
the maintained decision that View observes Need through ordinary match. The
mandatory affine correction also describes direct-await retained slots using
ready/error/denied terminology. Applying either old expression literally would
restore the superseded branch model.

Production also lacks the boundaries needed to invent the replacement safely:

- final semantic View facts do not publish a checked Need subscription/match
  owner;
- compiler View lowering still accepts only a narrow static subset and emits no
  Match instruction;
- View value programs are presentation-only `FxRuntimeValue` programs and
  cannot carry a Need identity, `Progress`, or an arbitrary Ready payload;
- runtime-driver View evaluation receives retained bindings but no canonical
  Need publication inventory/subscription cursor; and
- bundle/save/hot-replacement schemas have no typed pattern, binding, or Need
  subscription identity to validate.

## Blocking request

The independently throwable correction is
[Lang-01.5.1.1.2.1 reactive unary-Need match reconciliation](../reviews/requests/2026-08-21-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation.md).

It is a correction of only the parent package's direct-await rows. Accepted
final-HIR View catalog, generic Match, dynamic-value, ownership, static-proof,
resource, save, and hot-replacement decisions remain authoritative unless the
request finds a concrete current-repository contradiction.

## Performed and not performed

Performed:

- inspected stable language/View docs, current View program/bundle/runtime
  owners, parent package intake/status, and the affine correction's Need rows;
- corrected the stale maintained View chapter that still documented the old
  four-way integer Await discriminant; and
- created the throwable correction request above.

Not performed:

- no production Rust, codec, parser, HIR, semantic fact, compiler product,
  runtime evaluator, save schema, or compatibility layer was added;
- no old Await product path was deleted before a typed replacement exists; and
- no Cargo or runtime validation is claimed for this documentation-only cut.

## Next independent work

Dialogue/RichText completion and the line-plan fixture can proceed without the
blocked View subscription decision. Reactive View implementation resumes only
after the correction returns implementation-ready with zero open questions.
