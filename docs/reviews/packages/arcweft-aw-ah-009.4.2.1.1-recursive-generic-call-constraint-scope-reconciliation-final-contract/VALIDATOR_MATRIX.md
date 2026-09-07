# Validator matrix

These are required implementation validators, not passing results of this
design package. Each row must be exercised through typed APIs or executable
behavior; file spelling is not an acceptance gate.

| ID | Owning boundary | Accepted evidence | Rejected evidence and classification |
|---|---|---|---|
| V01 | Schema binder seal | Every candidate declaration slot occurs, owns one kind-correct Bound slot and has checked first use | Duplicate/missing/wrong-kind candidate or forged first use: schema invariant |
| V02 | Lexical projection | Exact accepted owner/generation and complete visible Free inventory | Missing/stale owner, arbitrary argument-derived Free inventory: invariant |
| V03 | Opening gate | Selected schema, operation, position, mapper, lexical scope and exact None/Prepared/Frozen seed | Foreign graph, wrong state/group/schema/lineage or reconstructed seed: invariant before callbacks |
| V04 | Active type/const/effect scope | Inference issuer and slot match the current opening; Free references have lexical proof | Foreign issuer, dangling Bound depth/slot, kind confusion, unauthorized Free: invariant |
| V05 | Relation | Callee inference T -> caller Free(T), same Free identity, supported directional shape/effect relation | Different rigid/concrete types: candidate mismatch; no rigid assignment |
| V06 | Occurs and normalization | A -> B -> i64 normalizes once; T key -> Free(T) is valid | Inference x -> x or x -> F(x): cyclic candidate rejection; preserve existing source-failure precedence |
| V07 | Function binder comparison | Alpha-equal nested schemes, capture-avoiding shift/open | Misbound slot: invariant; incompatible schemes or implicit scheme-to-monotype coercion: mismatch |
| V08 | Source hints/replay | Parametric child projection intersects exact unbound set; closed children get Complete | Outer active inference entering nested solver/final facts: invariant; failed branch restores its checkpoint |
| V09 | Completion | All required current slots bound, live future roots reified, no active issuer | Missing required parameter: rejection; noncanonical/foreign completed evidence: invariant |
| V10 | Frozen inheritance | Exact base/schema/position and all previously bound keys, including early-solved future slots | Extra/missing/stale key or mutable replacement of inherited value: invariant |
| V11 | Future schemes | Caller Free and residual Bound remain distinct; result-only future slots included; solved future slots omitted | First-use-only guessed deferred list, duplicate residual origin or escaped future capture: invariant/rejection at its proper producer |
| V12 | Stable encoders | Kind/depth/slot plus canonical structural/owner evidence; version 1 | Any active issuer or allocation counter: typed encoding invariant; no fallback digest |
| V13 | Closed-instance seal | Callee normalized RHSs closed once under enclosing rows; equality checked for identical key | Free/active slot in invoked ABI or same key with unequal closed input: invariant |
| V14 | Runtime type graph | Correct scope stacks/child transitions; function binder closes all Bound uses | Open value root, wrong stack, forged bound subnode/root or type ID: admission error |
| V15 | Continuation input | Exact original scheme identity and prefix types; Unapplied empty prefix or AfterGroup adjacency | Terminal monotype replacing input scheme, nonempty Unapplied prefix, bad next group: ABI admission error |
| V16 | Captured prefix values | Closed under enclosing instance; may contain an internally closed function scheme | Prefix value depends on its own unbound residual slot: uninferred-parameter rejection before publication |
| V17 | Graph discovery | Existing keys memoized; unique nodes/edges/work/depth charged deterministically | First over-limit charge: typed limit abort; counter overflow/cancellation: typed abort; no partial catalog |
| V18 | Native/AWBC parity | Same exact ABI and closed target; all prefix/current values evaluated once in physical source order | Runtime generic inference, different target by backend, generic nonterminal FunctionSite: invariant |
| V19 | Codec/restore | Scoped graph and scheme identity belong to pinned program; exact prefix values match | Wrong generation, kind/depth/count/type ID or malformed prefix: verifier/restore rejection |
| V20 | Privacy | Only real preparation, completion and admission producers construct opaque evidence | Raw construction/serde of issuer, opening, substitution or frozen handle: compile failure |

## Precedence and transactional boundaries

Keep existing lower inherited-solution precedence: structural ordering and
duplicates, then scope/kind/role, then canonicality/occurs/forbidden forms, then
exact inherited-key agreement. A malformed earlier layer is not masked by a
later missing key. The new Free/Bound/Inference representation changes the
meaning of self-binding but not this precedence.

Work checks use the fixed order in FINAL_CONTRACT.md. A cancelled operation
does not allocate or publish a next row. Per-branch source failure remains
attached to its exact typed source, not flattened to an outer candidate error.
Whole-project runtime publication occurs only after discovery and
materialization finish. No partial replacement of an existing accepted
generation is permitted.

C1/C2 and final callable seal must fail if any active reference remains in any
expression, nominal, closure, default, effect, continuation, diagnostic evidence
or executable fact that is about to publish. A diagnostic may describe an
active variable locally, but no allocation nonce becomes a stable diagnostic
cache or semantic identity.
