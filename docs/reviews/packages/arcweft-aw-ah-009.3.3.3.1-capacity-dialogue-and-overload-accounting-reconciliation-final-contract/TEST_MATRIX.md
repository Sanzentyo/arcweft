# Test matrix

All rows use public or crate-owned typed behavior. No row reads implementation
or documentation source text, searches for deleted symbol spellings, introduces
a test-only semantic branch, or uses a context-free result cache.

## A. Phase and family classification

| ID | Setup | Required assertion |
| --- | --- | --- |
| CLASS-001 | Pre-capacity typed ledger over every member of `CallableFamily::ALL` | inventory 23; exact 18 R / 3 U / 2 P; no wildcard classification |
| CLASS-002 | Pre-capacity current observation ledger | exactly 21 executable rows and 42 observation cases |
| CLASS-003 | Pre-capacity final-completion ledger | exactly 20 credited rows and 40 cases; Speaker is `PendingRemoval`; Capacity and Dialogue are `PendingAuthority` |
| CLASS-004 | Post-capacity/pre-Dialogue ledger | inventory 23; exact 18 R / 4 U / 1 P |
| CLASS-005 | Post-capacity current observation ledger | exactly 22 executable rows and 44 observation cases |
| CLASS-006 | Post-capacity final-completion ledger | exactly 21 credited rows and 42 cases; Speaker still `PendingRemoval`; Dialogue pending |
| CLASS-007 | Final typed ledger | inventory 22; exact 19 R / 3 U / 0 P; Speaker absent |
| CLASS-008 | Final current and completion ledgers | both exactly 22 rows / 44 cases |
| CLASS-009 | Every pre-final phase | no phase grants final case credit to both Speaker and final Dialogue |
| CLASS-010 | Exhaustive typed match/const table | adding a family without a classification fails compilation in the owning crate |
| CLASS-011 | Validator/category parity | changing a family validator or migration class without updating its evidence pair fails the matrix test |
| CLASS-012 | Section-19 gate | staged table may report complete, but final acceptance remains open while any `PendingAuthority`/`PendingRemoval` exists |

## B. CapacityMethod authority and unchecked evidence

| ID | Setup | Required assertion |
| --- | --- | --- |
| CAP-001 | Baseline legacy static-capacity success path | zero final family credit; no checker-owned capacity target fact is accepted as final evidence |
| CAP-002 | Baseline drifted homogeneous `_` schema with spread | spread rejection earns zero Capacity credit |
| CAP-003 | Typed `String.with_capacity()` after switch | one associated-callee resolver invocation; Capacity candidate selected; zero args accepted; result `String` |
| CAP-004 | Typed `Bytes.with_capacity(4096)` | exact receiver/member/arity identity; one argument checked with `Unchecked`; result `Bytes` |
| CAP-005 | Typed bare `Vec.with_capacity(8)` | accepted typed bare constructor identity; no `Named("_")`; result preserves accepted bare-Vec semantic identity |
| CAP-006 | Typed `Vec<I32>.with_capacity(8)` | generic item identity preserved through syntax/HIR/nominal resolution and candidate identity |
| CAP-007 | Typed `Vec<T>.with_capacity(8)` inside a generic callable | the same semantic generic parameter `T` reaches `CapacityMethodId`; no text reconstruction |
| CAP-008 | Qualified and aliased receiver forms | canonical typed identity selects the same Capacity family without display-label parsing |
| CAP-009 | Multiple positional arguments | selected Capacity candidate; every argument checked without expected type; no arity rejection |
| CAP-010 | Unknown named argument | selected Capacity candidate; expression checked unchecked; no family-owned unknown-name rejection |
| CAP-011 | Authored spread | selected Capacity candidate; spread expression checked unchecked; no `UnsupportedSpread` from Capacity |
| CAP-012 | Recovered argument entry | ordinary recovery expression is checked unchecked; family remains clean recovery |
| CAP-013 | Registered/non-registered parity | both modes converge on the same typed seed, candidate ID, schema, result, and argument facts |
| CAP-014 | Checker/native signature parity | primary candidate, receiver, member, arity, result, origin, and instantiation are identical |
| CAP-015 | Environment collision | typed environment method wins before Capacity according to the existing resolver order |
| CAP-016 | Associated trait collision | Capacity wins before associated trait resolution for the accepted member |
| CAP-017 | Data-last collision | Capacity wins and data-last fallback is ineligible for the accepted associated call |
| CAP-018 | Value receiver or near-miss member | does not accidentally become a static Capacity candidate |
| CAP-019 | Unknown/ambiguous/malformed type receiver | structured typed failure; arguments recover normally; no string fallback or placeholder candidate |
| CAP-020 | Migration counters | one call registration; one shared resolver invocation; zero old-dispatch calls |
| CAP-021 | Typed deletion compile test | public/crate-owned old static dispatcher API cannot be used after the switch; test is compilation, not source scanning |

## C. Dialogue pending and final authority

| ID | Setup | Required assertion |
| --- | --- | --- |
| DIA-001 | Pre-switch final ledger | Dialogue is `PendingAuthority`; zero accepted and zero negative final cases |
| DIA-002 | Pre-switch `SpeakerLine` resolver observation | may remain a current Speaker regression observation; earns zero Dialogue final credit |
| DIA-003 | Pre-switch frozen string/`HirDialogue` content path | cannot satisfy the final Dialogue row |
| DIA-004 | Pre-switch ordinary call lacking final CharacterFactory/Reconfigure candidate authority | cannot be fabricated into final Dialogue evidence |
| DIA-005 | Atomic switch precondition missing Proof attached HIR/project | Dialogue remains pending |
| DIA-006 | Atomic switch precondition missing accepted line identity/project collision transaction | Dialogue remains pending |
| DIA-007 | Atomic switch precondition leaves any frozen Speaker/string/`HirDialogue` reader | Dialogue remains pending and Speaker remains uncredited |
| DIA-008 | Final CharacterReconfigure accepted pair | existing `Expr::Call`; exact/dependent `look`; OpenChecked custom field; candidate selected; same CharacterDialogue identity returned |
| DIA-009 | Final CharacterReconfigure spread negative pair | candidate observed; spread rejected with `UnsupportedSpread`; rejected target and recovery facts published |
| DIA-010 | Final CharacterFactory ordinary call | existing `Expr::Call`, not a second dialogue call AST/HIR owner |
| DIA-011 | Final bracket ContentApplication | typed Proof HIR operation; accepted line identity; `ContentApplication` ID; `DialogueLine` result; no `Expr::Call` encoding |
| DIA-012 | Final colon ContentApplication | same semantic/HIR owner as bracket without fabricated bracket tokens |
| DIA-013 | Final family transition | Speaker family/ID absent in the same typed inventory transition that activates all three final Dialogue IDs |
| DIA-014 | Typed deletion compile failures | `SpeakerCallableId`, Speaker/SpeakerPreset public types, and obsolete enum variants are not nameable after the switch |
| DIA-015 | Runtime-plan publication | final typed CharacterDialogue configuration and content application reach runtime plan with no callee-label reconstruction |
| DIA-016 | Checker/native signature parity | CharacterFactory/Reconfigure primary candidates match; ContentApplication projects from its typed HIR owner |
| DIA-017 | No dual credit | final 22/44 passes only when Speaker is absent and Dialogue is credited |

## D. Physical versus retained overload accounting

In these rows `P` is `physical_candidate_argument_evaluations` and `R` is
`retained_argument_inference_facts`.

| ID | Setup | Required assertion |
| --- | --- | --- |
| OA-001 | Two candidates, one ordinary argument, unique winner | two `Probe` events + one `SelectedReplay`; `P=3`, `R=1` |
| OA-002 | Two candidates, contextual `.Variant` expectations | each probe records its exact expected enum type; only winner/recovery projection remains in R |
| OA-003 | Two candidates, unsuffixed numeric literal | candidate-specific numeric fallback/lowering evidence differs during probes; only final replay evidence retained |
| OA-004 | Two function-signature candidates and closure argument | unselected captures/effects/diagnostics roll back; selected replay facts retained once |
| OA-005 | Two candidates and partial placeholder/function value | candidate-specific partial/curried facts are isolated; selected facts retained; curried-group contract unchanged |
| OA-006 | Two generic candidates | substitutions are candidate-local during probes; winner substitution retained; no cross-candidate leakage |
| OA-007 | Nested call checked against candidate-specific result | nested target facts and expected result are candidate-local; selected nested facts alone survive |
| OA-008 | Fixed literal spread of `k` slots, two candidates, one winner | `P=3k`, `R=k`; one authored argument index and `k` logical slot indices |
| OA-009 | Typed-rest spread | one container expression physical event per candidate/pass; item compatibility is not double-counted as traversal |
| OA-010 | Ambiguous two-candidate tie | probe events only; no selected replay; R is deterministic primary tied projection; target is ambiguous |
| OA-011 | Two rejected candidates | probe events for both; no replay; R is stable primary rejected probe projection |
| OA-012 | Singleton rejected candidate | one probe + one `RejectedRecoveryReplay`; precise diagnostics/judgments retained; `P=2` per ordinary slot, `R=1` |
| OA-013 | IntentionallyUnchecked clean recovery | probe + selected replay physical events; one retained unchecked fact; no family-owned shape rejection |
| OA-014 | Cancellation before first slot admission | `P=0`, `R=0`; typed terminal error retained |
| OA-015 | Cancellation after a completed slot | P equals completed prefix; `R=0`; no partial checked target published |
| OA-016 | Deadline/work failure during candidate probing | completed physical prefix survives; all semantic probe state rolls back; terminal failure retained |
| OA-017 | Selected replay work accounting | replay physical events exist even when speculative candidate work is deliberately not charged twice |
| OA-018 | Candidate materialization/comparison only | resolver/materialization/comparison counts never increment P |
| OA-019 | Probe rollback | TypeCheckStats, judgments, lowering evidence, captures, effects, scopes, borrows, diagnostics, and call facts return to checkpoint; physical events remain |
| OA-020 | Nested overloads | context stack assigns each physical event the correct call expression, candidate, pass, argument, and slot |
| OA-021 | Missing/non-callable target with ordinary recovery checks | no candidate event and no retained candidate inference fact; ordinary recovery checks remain outside this metric |
| OA-022 | Same source expression under different expected types | test proves distinct typed events; no context-free result cache or constraint shortcut exists |
| OA-023 | Public semantic evidence | existing checked call-target facts expose final retained slots; physical trace remains crate-owned operational evidence |
| OA-024 | Determinism | repeated identical accepted builds produce the same ordered typed event/fact projections under stable candidate order |

## E. Drift and structural guards

| ID | Setup | Required assertion |
| --- | --- | --- |
| DRIFT-001 | Capacity schema inspection through typed API | `variadic_unchecked`; unknown names OpenUnchecked; spread Unchecked; no homogeneous `_` parameter |
| DRIFT-002 | Capacity associated callee through checker | no old dispatcher, source scan, display-label parser, or sentinel receiver identity |
| DRIFT-003 | Dialogue ordinary operations | only CharacterFactory/Reconfigure use `Expr::Call`; ContentApplication does not |
| DRIFT-004 | Final family inventory | Speaker absent; Dialogue present; total 22 |
| DRIFT-005 | Resolver count/parity | exactly one shared resolver path; no signature-only resolver |
| DRIFT-006 | Source identity | accepted source/HIR identity is typed; no source gate or ad hoc source reparse |
| DRIFT-007 | Compatibility audit | no alias, deprecated carrier, dual reader, removed-syntax diagnostic, CSS, or Takumi path |
| DRIFT-008 | Production owner audit | no second argument fact store, second Dialogue HIR arena, or context-free expression cache |
| DRIFT-009 | Compile-time classification exhaustiveness | new family or category change cannot compile/pass without explicit table and pair updates |
| DRIFT-010 | Manifest/package validation | archive members, SHA-256, lengths, status, and exact four-byte `OPEN_QUESTIONS.md` are valid |

## F. Validation order

For each implementation cut, run the repository-required sequence:

1. focused syntax/HIR/sema tests for the changed owner;
2. focused checker/signature parity and accounting tests;
3. `cargo check --workspace --all-targets --all-features`;
4. strict workspace Clippy with warnings denied;
5. relevant workspace/Tier 2 suites for the changed public/runtime path;
6. structural audit and final formatting/diff checks.

The design package itself performs no repository command or code change.
