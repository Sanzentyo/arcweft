# Arcweft: unsafe audit proofs and formal verification proofs

## 0. Goal

This document extends the current `pro_review11` lifetime/thread/drop design with two proof-discharge mechanisms for operations that the compiler cannot prove automatically:

1. **Audited unsafe proof**
   - The author writes a human safety explanation.
   - The compiler/linter checks that an explanation exists and records an obligation.
   - This is similar in spirit to Rust `unsafe` plus a `SAFETY:` comment.

2. **Formal verification proof**
   - The author writes or references a `proof` item.
   - A verifier discharges generated proof obligations.
   - The compiler can accept the operation as verified when the proof succeeds.

This is needed for game scripts because Arcweft should be stricter by default, but still permit controlled promotion and upper-lifetime access when appropriate:

```awft
'flow.flags.seen_alice_intro <- true
'global.settings.skip_seen <- true

'flow.cache.last_line <-
    line_summary
    |> promote('flow, proof = @proof.line_summary_to_flow)
```

---

## 1. Current repository state

### 1.1 Contract clauses already exist

The current AST already has contract clauses:

```rust
ContractClause::Requires
ContractClause::Ensures
ContractClause::Invariant
ContractClause::Assume
ContractClause::Reads
ContractClause::Effects
ContractClause::NoEffect
ContractClause::Modifies
ContractClause::Decreases
```

This is useful because formal proof syntax should build on the existing contract vocabulary rather than inventing an unrelated system.

### 1.2 Lifetime/thread/drop syntax already exists in syntax layer

Current syntax work already includes:

```text
- `Expr::LifetimePath`
- `Stmt::LifetimeSet`
- `Stmt::Thread`
- `Stmt::DeferBlock`
- `Stmt::Wait`
- `DialogueToken::Mark`
- `LineOptions::{look, stage, portrait, focus, cleanup}`
- `LinePlanItem::{Init, Thread, On, Finally, Stmt}`
- `BlockStyle::Flat`
```

So the missing part is not basic syntax. The missing part is proof-aware semantics:

```text
- proof obligations;
- formal proof items;
- unsafe/audit blocks;
- effect/capability checks for upper lifetime mutation;
- safe/unsafe lifetime promotion;
- verifier integration;
- LSP/CLI workflow for obligations.
```

### 1.3 Current checker is only a shallow lifetime checker

The current checker can:

```text
- record a simple guarantee for `line.focus`;
- reject missing marker handlers;
- reject duplicate marks;
- reject removed `[hook ...]`;
- reject some double-drops of lifetime registry keys;
- require optional access for unguaranteed lifetime keys.
```

But it does not yet have:

```text
- a real lifetime hierarchy;
- an upper-lifetime write permission model;
- safe promotion proofs;
- unsafe promotion audit tracking;
- proof certificates;
- thread capture proof obligations;
- MustDrop typestate proof obligations.
```

---

## 2. Terms

### 2.1 Safe operation

An operation the compiler can prove automatically.

```awft
'line.focus |> drop
```

if `focus = .soft` made `'line.focus` statically guaranteed.

### 2.2 Verified operation

An operation that is not automatically obvious, but has a formal proof.

```awft
'flow.cache.last_line <-
    line_summary
    |> promote('flow, proof = @proof.line_summary_to_flow)
```

### 2.3 Audited unsafe operation

An operation accepted because the author asserts and documents why it is safe.

```awft
unsafe lifetime @unsafe.cache_last_line
reason = "cache a serializable summary for the rest of the flow"
{
    /// SAFETY:
    /// - `line_summary` is owned.
    /// - It contains no `'line` references.
    /// - It contains no scoped handles.
    /// - It is used only for debug/replay annotation.
    'flow.cache.last_line <-
        line_summary
        |> promote_unchecked('flow)
}
```

### 2.4 Assume

`assume` is a logical assertion trusted by the verifier. It must be restricted.

Recommended rule:

```text
- `assume` is allowed in proof/debug contexts.
- `assume` in production code must be inside `unsafe audit` or a proof item with an explicit assumption list.
- every assume generates an audit obligation unless discharged by a configured trusted axiom.
```

---

## 3. Proof obligation model

The compiler should generate proof obligations for operations it cannot prove.

```rust
pub enum ProofObligationKind {
    LifetimeAccess,
    LifetimeWrite,
    LifetimePromotion,
    UnsafePromotion,
    DetachedThreadCapture,
    ThreadCapture,
    MustDropDischarge,
    UseAfterDrop,
    ConcurrentWrite,
    ConcurrentExclusiveAxisWrite,
    CleanupSuppression,
    GlobalStateMutation,
}
```

Each obligation records:

```rust
pub struct ProofObligation {
    id: ObligationId,
    kind: ProofObligationKind,
    source: SourceAnchor,
    required_scope: Option<LifetimeScopeKind>,
    target_scope: Option<LifetimeScopeKind>,
    terms: Vec<ProofTerm>,
    discharge: Option<ProofDischarge>,
}
```

Discharge modes:

```rust
pub enum ProofDischarge {
    Automatic,
    FormalProof(EntityRef),
    UnsafeAudit(EntityRef),
    TrustedAxiom(EntityRef),
}
```

---

## 4. Lifetime hierarchy

Use a real hierarchy instead of raw strings.

```text
'cue <= 'line <= 'scene <= 'flow <= 'session <= 'global
```

`'persistent` is storage-backed and should be handled separately.

```rust
pub enum LifetimeScopeKind {
    Cue,
    Line,
    Scene,
    Flow,
    Session,
    Global,
    Persistent,
    Named(String),
}
```

Lifetime key:

```rust
pub struct LifetimeKey {
    scope: LifetimeScopeKind,
    path: Vec<String>,
}
```

Examples:

```awft
'line.focus
'flow.flags.seen_alice_intro
'session.unlocks.alice_route
'global.settings.skip_seen
```

---

## 5. Safe upper-lifetime writes

### 5.1 Read upper lifetime

Reading upper lifetime state from a line is allowed if deterministic and available.

```awft
let seen = 'flow.flags.seen_alice_intro?
let skip_seen = 'global.settings.skip_seen?
```

Non-optional access requires static guarantee:

```awft
let seen = 'flow.flags.seen_alice_intro
```

If not guaranteed:

```text
error: lifetime key `'flow.flags.seen_alice_intro` is not statically guaranteed
help: use `'flow.flags.seen_alice_intro?`
```

### 5.2 Write upper lifetime

Writing upper lifetime state from a lower scope is allowed only with effect/capability.

```awft
flow @flow.opening opening(state: GameState)
effects { state.write('flow) }
{
    alice:
        見たことにする。[mark .seen]
    with {
        on .seen {
            'flow.flags.seen_alice_intro <- true
        }
    }
}
```

Writing global state requires stronger capability:

```awft
flow @flow.settings settings(state: GameState)
effects { state.write('global) }
{
    'global.settings.skip_seen <- true
}
```

If the effect is missing:

```text
error: writing `'global.settings.skip_seen` requires `effects { state.write('global) }`
```

### 5.3 Concurrent upper-lifetime writes

Raw concurrent writes to the same key should be rejected or require a merge/update operation.

Risky:

```awft
thread a {
    'flow.counter <- 1
}

thread b {
    'flow.counter <- 2
}
```

Better:

```awft
thread a {
    'flow.counter <- update(|x| x + 1)
}

thread b {
    'flow.counter <- update(|x| x + 1)
}
```

or:

```awft
'flow.flags <- merge_patch({ seen_alice_intro = true })
```

---

## 6. Safe promotion

### 6.1 `promote`

```awft
'flow.cache.last_line <-
    line_summary
    |> promote('flow)
```

Safe `promote('flow)` is accepted only if the checker can prove:

```text
- value is owned;
- value contains no references shorter than 'flow;
- value contains no scoped handles shorter than 'flow;
- value is serializable/replay-safe if needed;
- drop of the original owner cannot invalidate the promoted value;
- type implements Promote<'flow>.
```

If automatic proof fails:

```text
error: cannot prove `line_summary` is valid for 'flow
help: add a formal proof with `promote('flow, proof = @proof...)`
help: or use `promote_unchecked` inside `unsafe lifetime`
```

### 6.2 Formal proof for promote

```awft
'flow.cache.last_line <-
    line_summary
    |> promote('flow, proof = @proof.line_summary_to_flow)
```

Proof item:

```awft
proof @proof.line_summary_to_flow
proves safe_promote(LineSummary, 'flow)
requires owned(LineSummary)
requires serializable(LineSummary)
requires no_scoped_handles(LineSummary)
requires no_lifetime_below(LineSummary, 'flow)
{
    assert fields(LineSummary).all(|f| owned(f))
    assert fields(LineSummary).all(|f| serializable(f))
    assert fields(LineSummary).all(|f| no_scoped_handles(f))
}
```

The proof item is checked by the verifier. The compiler should accept the operation only if the proof verifies or if project policy allows unverified proof stubs.

---

## 7. Audited unsafe promotion

### 7.1 Syntax

```awft
unsafe lifetime @unsafe.cache_last_line
reason = "cache a serializable line summary for flow-local debug UI"
{
    /// SAFETY:
    /// - `line_summary` is created from owned scalar values.
    /// - It contains no handles.
    /// - It contains no references into 'line.
    /// - It is not persisted outside the flow.
    'flow.cache.last_line <-
        line_summary
        |> promote_unchecked('flow)
}
```

Rules:

```text
- `unsafe lifetime` requires an id.
- `reason = "..."`
  is required.
- a `SAFETY:` comment is required.
- every unchecked operation inside the block is recorded as an obligation.
- unsafe block does not bypass determinism or Sans I/O.
```

### 7.2 Comment requirement

The linter should require a `SAFETY:` comment immediately inside or before the block.

Allowed:

```awft
/// SAFETY:
/// `summary` is owned and contains no handles.
unsafe lifetime @unsafe.promote_summary
reason = "flow-local debug cache"
{
    'flow.summary <- summary |> promote_unchecked('flow)
}
```

Allowed:

```awft
unsafe lifetime @unsafe.promote_summary
reason = "flow-local debug cache"
{
    /// SAFETY:
    /// `summary` is owned and contains no handles.
    'flow.summary <- summary |> promote_unchecked('flow)
}
```

Not allowed:

```awft
unsafe lifetime @unsafe.promote_summary
reason = "trust me"
{
    'flow.summary <- summary |> promote_unchecked('flow)
}
```

Diagnostic:

```text
error: unsafe lifetime block requires a SAFETY comment
```

### 7.3 Machine-readable safety note

A comment is good for authoring, but tools also need structured data. Add optional `safety` fields:

```awft
unsafe lifetime @unsafe.promote_summary
reason = "flow-local debug cache"
safety = {
    owned = true,
    no_handles = true,
    no_borrow_below = 'flow,
}
{
    /// SAFETY:
    /// See fields above.
    'flow.summary <- summary |> promote_unchecked('flow)
}
```

The structured `safety` fields are not a formal proof. They are audit metadata.

---

## 8. Formal proof items

### 8.1 Top-level proof item

```awft
proof @proof.line_summary_to_flow
proves safe_promote(LineSummary, 'flow)
requires owned(LineSummary)
requires serializable(LineSummary)
requires no_scoped_handles(LineSummary)
requires no_lifetime_below(LineSummary, 'flow)
{
    assert fields(LineSummary).all(|f| owned(f))
    assert fields(LineSummary).all(|f| serializable(f))
    assert fields(LineSummary).all(|f| no_scoped_handles(f))
}
```

### 8.2 Proof item AST

Add:

```rust
pub struct ProofItem {
    visibility: Option<Visibility>,
    id: EntityRef,
    proves: Expr,
    requires: Vec<Expr>,
    body: Vec<ProofStep>,
    range: TextRange,
}

pub enum ProofStep {
    Assert(Expr),
    Assume { expr: Expr, reason: Option<String> },
    Check(Expr),
    Calc(Vec<Expr>),
    BySolver { solver: String, options: Vec<(String, Expr)> },
    Raw(String),
}
```

### 8.3 Inline proof reference

```awft
promote('flow, proof = @proof.line_summary_to_flow)
```

or for upper writes:

```awft
'flow.cache.last_line <- line_summary
    proof = @proof.flow_cache_write
```

Prefer the function-argument style first because it uses existing expression syntax:

```awft
line_summary |> promote('flow, proof = @proof.line_summary_to_flow)
```

### 8.4 Proof modes

Project config:

```toml
[verify]
mode = "check"       # off | lint | check | strict
solver = "smt"       # smt | builtin | external
cache = true

[unsafe]
audit_required = true
safety_comment_required = true
allow_in_release = false
```

Meaning:

```text
off
  Do not verify proofs.

lint
  Collect obligations and report warnings.

check
  Verify required proofs; audited unsafe allowed only by policy.

strict
  No undisclosed audited unsafe in release; all non-trivial obligations need formal proof or trusted axiom.
```

---

## 9. Formal proof vs audited unsafe

| Case | Syntax | Compiler trust |
|---|---|---|
| Automatic | `promote('flow)` | compiler proves directly |
| Formal proof | `promote('flow, proof = @proof.x)` | verifier discharges |
| Audited unsafe | `unsafe lifetime @unsafe.x { promote_unchecked('flow) }` | human-audited, policy-controlled |
| Assume | `assume ...` | trusted, should generate obligation unless in trusted axiom |

Recommended policy:

```text
debug/dev:
  audited unsafe allowed with SAFETY comment.

release:
  audited unsafe either forbidden or requires explicit allowlist.

security-sensitive build:
  formal proof required for lifetime promotion/global mutation.
```

---

## 10. Unsafe blocks

### 10.1 Kinds

```awft
unsafe lifetime @unsafe.id { ... }
unsafe thread @unsafe.id { ... }
unsafe drop @unsafe.id { ... }
unsafe effect @unsafe.id { ... }
```

Meaning:

```text
unsafe lifetime
  lifetime promotion, upper-lifetime write, lifetime escape.

unsafe thread
  detached capture, shared access proof bypass, concurrent write override.

unsafe drop
  manual override of MustDrop/typestate proof.

unsafe effect
  effect capability proof bypass; must still obey Sans I/O boundaries.
```

### 10.2 Required fields

All unsafe blocks require:

```text
- id
- reason
- SAFETY comment
```

Example:

```awft
unsafe thread @unsafe.analytics_detach
reason = "analytics payload is owned and detached"
{
    /// SAFETY:
    /// Payload is cloned into session-owned data before detaching.
    thread detached analytics {
        telemetry.record(payload |> clone_owned |> promote('session))
    }
}
```

---

## 11. `assume`

### 11.1 Existing `assume`

`ContractClause::Assume` already exists. Keep it.

### 11.2 Restrict use

`assume` should be allowed in these contexts:

```text
- proof item body;
- unsafe block;
- test/spec mode;
- trusted axiom declarations.
```

It should not silently appear in ordinary production flow logic without being recorded.

Example:

```awft
proof @proof.foo
proves no_lifetime_below(LineSummary, 'flow)
{
    assume source_generated(LineSummary)
        reason = "generated manifest validated by build step"

    check no_lifetime_below(LineSummary, 'flow)
}
```

Every `assume` needs:

```text
- reason;
- source location;
- proof/audit obligation;
- optional trusted axiom id.
```

---

## 12. Trusted axioms

Some facts may come from build tools, schema validation, or external certified tools.

```awft
trusted axiom @axiom.resource_manifest_hashes
proves all_resources_have_semantic_hash
source = @tool.arcw_resource_check
```

Use sparingly.

Formal proofs may depend on trusted axioms:

```awft
proof @proof.voice_manifest_safe
proves voice_manifest_safe
requires @axiom.resource_manifest_hashes
{
    check all_voice_entries_have_locale
    check all_voice_entries_have_stable_id
}
```

---

## 13. Examples

### 13.1 Safe line-to-flow write with capability

```awft
flow @flow.opening opening(state: GameState)
effects { state.write('flow) }
{
    alice:
        見たことにする。[mark .seen]
    with {
        on .seen {
            'flow.flags.seen_alice_intro <- true
        }
    }
}
```

### 13.2 Formal proof for promotion

```awft
flow @flow.opening opening(state: GameState)
effects { state.write('flow) }
{
    alice:
        記録する。[mark .record]
    with {
        init {
            let summary = make_line_summary(state)
        }

        on .record {
            'flow.cache.last_line <-
                summary
                |> promote('flow, proof = @proof.line_summary_to_flow)
        }
    }
}

proof @proof.line_summary_to_flow
proves safe_promote(LineSummary, 'flow)
requires owned(LineSummary)
requires serializable(LineSummary)
requires no_scoped_handles(LineSummary)
requires no_lifetime_below(LineSummary, 'flow)
{
    assert LineSummary.fields.all(serializable)
    assert LineSummary.fields.all(no_scoped_handles)
}
```

### 13.3 Audited unsafe promotion

```awft
unsafe lifetime @unsafe.cache_last_line
reason = "temporary debug cache until formal proof is added"
{
    /// SAFETY:
    /// `summary` is generated from owned scalar values only.
    /// It contains no handles and no borrowed text buffers.
    'flow.cache.last_line <-
        summary
        |> promote_unchecked('flow)
}
```

### 13.4 Unsafe detached thread capture

```awft
unsafe thread @unsafe.detach_analytics
reason = "analytics payload is cloned and session-owned"
{
    /// SAFETY:
    /// `payload` is converted to owned session data before the detached task starts.
    thread detached analytics {
        let payload =
            payload
            |> clone_owned
            |> promote('session, proof = @proof.analytics_payload_session)

        telemetry.record(payload)
    }
}
```

### 13.5 Global state write

```awft
flow @flow.settings settings(state: GameState)
effects { state.write('global) }
{
    'global.settings.skip_seen <- true
}
```

If the value is from a line:

```awft
unsafe lifetime @unsafe.global_skip_setting
reason = "user setting is scalar and globally valid"
{
    /// SAFETY:
    /// Boolean value is scalar, owned, deterministic, and replay-safe.
    'global.settings.skip_seen <-
        local_skip_value
        |> promote_unchecked('global)
}
```

---

## 14. Diagnostics

### 14.1 Missing proof

```text
error: cannot prove promotion from 'line to 'flow
help: use `promote('flow, proof = @proof.id)`
help: or use `promote_unchecked('flow)` inside `unsafe lifetime`
```

### 14.2 Unsafe block without safety comment

```text
error: unsafe lifetime block requires a SAFETY comment
```

### 14.3 Unsafe block without reason

```text
error: unsafe block requires `reason = "..."`
```

### 14.4 Assume without reason

```text
error: `assume` requires a reason or trusted axiom
```

### 14.5 Global write missing capability

```text
error: writing `'global.settings.skip_seen` requires `effects { state.write('global) }`
```

### 14.6 Proof item failed

```text
error: proof @proof.line_summary_to_flow failed obligation no_lifetime_below(LineSummary, 'flow)
```

---

## 15. Implementation plan

### 15.1 AST additions

Add top-level items:

```rust
Item::Proof(ProofItem)
Item::TrustedAxiom(TrustedAxiomItem)
```

Add statements:

```rust
Stmt::UnsafeBlock(UnsafeBlock)
Stmt::Assume { expr: Expr, reason: Option<String> }
```

Add unsafe block:

```rust
pub struct UnsafeBlock {
    kind: UnsafeKind,
    id: EntityRef,
    reason: String,
    safety_comment: Option<String>,
    body: Vec<Stmt>,
}
```

Add proof items as above.

### 15.2 Parser

Parse:

```awft
unsafe lifetime @unsafe.id
reason = "..."
{
    /// SAFETY: ...
    ...
}
```

Parse:

```awft
proof @proof.id
proves expr
requires expr
{
    assert expr
    assume expr reason = "..."
    check expr
}
```

Parse `assume` as contract clause and proof step carefully.

### 15.3 CST / comment retention

Because audited unsafe requires `SAFETY:` comments, the lossless CST must expose comments near unsafe blocks. Do not rely only on typed AST, because comments may be dropped.

Checker/linter should inspect CST/source anchors to find:

```text
/// SAFETY:
```

near the unsafe block.

### 15.4 Checker

Add:

```rust
ProofObligationCollector
ProofDischargeChecker
LifetimeRegionChecker
UnsafeAuditChecker
PromoteChecker
UpperLifetimeWriteChecker
ThreadCaptureChecker
MustDropTypestateChecker
```

### 15.5 CLI

Commands:

```bash
arcw verify
arcw verify --strict
arcw verify --emit-obligations obligations.json
arcw unsafe list
arcw unsafe audit-check
arcw unsafe require-formal
```

### 15.6 LSP

Features:

```text
- show proof obligations at cursor
- generate proof stub
- generate unsafe audit block
- validate SAFETY comment
- navigate from unsafe operation to proof/audit id
- list assumes/trusted axioms
- show lifetime promotion graph
```

### 15.7 Build policy

Config:

```toml
[verify]
mode = "check"

[unsafe]
allow_audited = true
allow_audited_in_release = false
safety_comment_required = true
reason_required = true
formal_required_for = ["global", "persistent", "detached_thread"]
```

---

## 16. Required doc updates

### 16.1 `docs/01-language/types-and-effects.md`

Add:

```text
- lifetime hierarchy
- state.write('scope) effects
- safe promotion
- unsafe lifetime
- proof obligations
```

### 16.2 `docs/01-language/contracts.md` or new `verification.md`

Add:

```text
- proof item syntax
- assert/check/assume in proofs
- trusted axioms
- proof discharge modes
```

### 16.3 `docs/02-runtime/core.md`

Clarify:

```text
- unsafe never bypasses Sans I/O
- global writes lower to deterministic events
- proof/audit metadata appears in trace/diagnostics
```

### 16.4 `docs/04-tooling/cli.md`

Add `arcw verify` and `arcw unsafe`.

### 16.5 `docs/04-tooling/lsp.md`

Add proof obligation UX.

---

## 17. Recommended final policy

Use three layers:

```text
1. Safe static proof
   The compiler proves it. No unsafe syntax.

2. Formal proof
   Author provides `proof @proof...`; verifier checks it.

3. Audited unsafe
   Author uses `unsafe ...` with id, reason, and SAFETY comment.
   Allowed only by policy and never silently.
```

For release builds, recommended default:

```toml
[verify]
mode = "check"

[unsafe]
allow_audited_in_release = false
formal_required_for = ["global", "persistent", "detached_thread"]
```

This gives game authors practical escape hatches while still allowing serious projects to require formal verification for lifetime promotion, global mutation, and detached concurrency.
