# Control flow, patterns, and semantic input

## Control-flow contracts

Expression and statement positions share the checked HIR and pattern authority.
A block or `scope` expression returns its final expression. A statement scope
returns Unit unless a control transfer leaves it. A named scope changes relative
ID and diagnostic paths, not local lookup or the value type. The language rules
are defined by [block scopes](../01-language/block-scopes.md) and the
[converged language surface](../01-language/converged-language-surface.md).

An `if` used as a value has an `else` and compatible branch results. A statement
`if` may omit `else`. Expression and statement `match` require exhaustiveness;
value-producing arms have a checked common result. A `loop` obtains its result
from reachable `break expr` transfers; no reachable break gives Never. `while`,
`while let`, and `for` return Unit. Their bindings and lexical body scopes do
not escape an iteration.

Pattern shape, binding collection, coverage, and unreachable alternatives use
one checked pattern/coverage model. Exact record or sequence patterns reject
unmatched elements; an ignored rest accepts them without a binding. A bound
record rest retains the original complete record, and a bound sequence rest
receives the unmatched tail. Guards use the selected semantic Copy obligations
for their temporary bindings; the original scrutinee remains owned until an arm
is selected. A rejected guard discards its temporary scope before continuing.

`let PAT = EXPR else { ... }` makes its bindings available afterward only when
the else continuation diverges. Return, goto, an enclosing break/continue, and
a Never-returning computation are control transfers rather than fabricated
ordinary values.

## Generic Match semantic transcript and publication

`FinalSemanticAnalysis::checked_match` validates the exact accepted HIR/symbol
generation and returns one complete `CheckedMatch`. Its bounded transaction
covers the requested expression or statement Match, its accepted expression,
pattern, statement and body subtree, and every nested Match. Source-ordered
arms, guards, binding/result types, callable and body evidence, and the closed
coverage result feed that same transaction. Core's `MatchSourceDefinition` and
`MatchSourceWriter` provide the typed semantic source grammar used by the
transcript; consumers do not maintain a second Match authority.

Semantic atoms use accepted identities and structural coordinates. Raw arena
IDs, source spans, formatting, debug/Serde spelling, and authored scope labels
do not supply semantic identity. Complete checked type/substitution evidence
owns the coverage universe. Missing or unresolved evidence, cancellation, work
or byte-limit exhaustion, non-exhaustiveness, and invalid nested evidence fail
the transaction before a complete product is published. Work and byte lengths
are checked before writes; limits do not produce sentinel success digests.

A zero-arm Match is exhaustive only for an actually uninhabited domain, such as
Never; an inhabited domain is rejected. Unreachable alternatives remain typed
coverage evidence. Never and `let-else` preserve their real continuation and
single-consumption behavior in Native and decoded AWBC execution and restoration.
The compiler consumes exhaustive checked products for expression and statement
owners and publishes no partially checked subtree.

This semantic product does not create another runtime Match decoder or make
semantic acceptance depend on retained View admission. View, executable task
admission and save formats consume their own owning contracts.

## Execution and replay

The runtime executes the accepted plan without reconstructing authored source.
Original function/body coordinates identify code; live values, iterators,
scopes and continuations belong to their execution state. The owning scope and
loop control entries handle break/continue cleanup and result custody.
Flow parameters enter once through a complete coordinate-addressed
`RuntimeFlowInvocation`; runtime steps carry event/result ingress.
See [the runtime step boundary](runtime-step-and-executors.md) and
[task scheduling](async-scheduler.md) for the executable contracts.

Evaluation and observable event order are deterministic under their admitted
inputs. Debug loop counters do not replace the semantic continuation state.

## Function semantic input evidence

The function transcript uses the accepted function definition and its actual
owned expression or executable body. Code references are accepted definition
leaves; ordinary recursive calls do not recursively expand called bodies. The
complete executable image commits every referenced definition's body row.

Callable state and origin references form a finite typed graph. The semantic
visitor computes each reachable definition/origin digest once per graph
transcript, preserving transition and partial-application source order. Private
state ordinals are memo keys only; no allocation coordinate is written into
these child digests. A Visiting edge rejects a forbidden cycle. Accepted code
and nominal references remain semantic leaves. An explicit borrowed-edge
stack avoids Rust recursion and avoids expanding shared subgraphs as trees.
Every graph visit, child transcript and emitted cached digest uses the same
work/byte meter; a child error poisons the parent before publication.

A retained input carries its origin, source role and transfer evidence from
`RuntimeFunctionInputBinding`. These are independent of frame-ingress
`Owned`/`Unrestricted` requirements. Origin tags are Binding (0), whole
Parameter (1), and EvaluatedResult (2), each followed by its accepted 32-byte
identity. Source tags are Capture (0), CapturedParameter (1), and Parameter
(2), followed by the source position; the two formal roles also include the
existing Value/Shared/Affine passing tag. Transfer is Transferred (0) plus
Copy/SnapshotClone/Move (0/1/2), ExternalBinding (1), or Formal (2).

In particular, extracting a body with external free bindings creates no
Copy/Move operation, and retaining a whole formal preserves its passing
evidence. Neither role may be inferred from a value's current occupancy,
type, local ordinal, or an ingress requirement. This corrects the narrower
three-mode capture field in the retained task-plan correction package: the
current producer transcript writes origin, semantic input type, source and
transfer evidence for each canonical retained input. The package remains a
historical design mirror; its omission does not authorize inventing a transfer.

The body-root transcript includes the full admitted signature/prologue and
owned body, including ordered task build-coordinate references. Producer
endpoints are ordered positions in that owned operation tree. Their path
contains balanced body roles, operation source ordinals, and the endpoint
ordinal within an operation, so empty branches and multiple AwaitMany
endpoints remain distinct. Completed task digests, expected keys, generations
and source/debug labels do not supply endpoint or task-reference authority.


## Audio command construction and source

Audio commands use one closed operation grammar parameterized by the expression
phase. Construction seeds, borrowed source frames and admitted commands retain
the same metadata and argument roles. The owning traversal visits each expression
once in command ABI order and stops at the first rejection; construction,
free-local analysis and source transport use that traversal.

Loop regions must have their start before their end, and microphone constraints
must satisfy their structural channel/rate rules before child expressions enter
construction. These checks use the existing typed audio metadata owners. Bounds
against decoded resource frames and actual device support remain with the runtime
resource/adapter owner when those facts are available. Source framing and counted
child admission do not grant a completed executable or task-plan proof; the real
operation factory and common executable seal remain required.

## Line graph semantic evidence

A Line row commits its accepted definition, result type, ordered captures and
activation exports, activation body, dense handle-site roles, root coordinate,
source-ordered nodes and child references, cancellation rules and every cleanup
outcome body/policy. Each action uses the same typed Flow visitor as function
bodies, including task construction coordinates. Node/trigger/join/cancel and
cleanup tags are exhaustive on their owning enums. Node and mark ordinals are
coordinates within the admitted Line graph; mark spellings and completed task
plan keys are excluded. Child-count arithmetic and the Line row quota precede
semantic traversal, and the transcript shares the complete pass's work/byte
meter. The Line owner issues its digest only after successful completion.

## Static request role evidence

Static Signal and Metric observations carry unrestricted literal values without
a selected root plan-type ID. Construction and private source materialization
use the same original plan value/domain authority. The complete borrowed graph
must pass resource and ownership admission before live mapping; every nominal
descendant must match its declared identity, layout, field/case and payload
types, even beneath structural containers or context-free opaque payloads.
Structural value tags do not reconstruct a root type or grant an executable
owner. Source rows share the caller's resource/work bank, and any source or
factory rejection prevents common executable publication.

Private static-effect sources retain every non-Audio request kind, including
complete text metadata, ordered fields/arguments, optional roles, assertion
identity/profile and original literal values. Complete framing and aggregate
counts precede request retention. Inspection and materialization read the same
grammar into the actual request type; no parallel request catalog is retained.
The source returns a construction seed, with the real operation factory and
common executable seal still required. Literal descendants share the original
resource/work bank. Expression-carrying Audio commands use their dedicated
Audio seed source; the static source boundaries reject raw Audio carriers.

A checked execution argument slot owns its static request role identity. The
identity is minted after the callable/schema destination is validated, then
transported unchanged in the compiler's source-ordered runtime operand row.
A receiver or callee is not an argument role; an Argument row must carry this
evidence before the runtime call is published.

Accepted record and variant payload field roles use their existing accepted
field identities. An open named argument uses the existing schema-owned open
argument identity, whose validated binding bytes are semantic data. A fixed
project formal uses its source-independent declaration identity and checked
group/parameter coordinate. Other fixed formals use their checked callable
identity and formal coordinate. Source names, expression values, ABI allocation
positions, and call-site revisions do not reconstruct a fixed formal role.
Types, source order, passing and request paths remain distinct transcript roles.

A ProjectCall's physical operand array is its source evaluation order. Its
fixed/rest logical rows and attached-presence row alone own ABI placement and
reference those physical operands. Construction and private source transport
retain this same mapping; they do not require or reconstruct a second ABI
position on each physical operand. The callable-state owner still validates
the complete parameter, attached-default and capture contract before admission.

The version-one checked request-role domain is
`arcweft.lang.checked-request-role.v1\0`. Its closed family tags are accepted
variant payload field (0), accepted record field (1), fixed formal (2), and
open argument (3). Field/open families contain the existing 32-byte identity.
A fixed formal contains project declaration (0) or checked callable (1)
identity, followed by group and parameter ordinals. Consumers receive the
opaque issued identity, without a byte constructor or source-based fallback.


Core HostCall and MakeNeed startup argument templates retain that identity
alongside the executable value in each positional/named/spread variant. Builder
admission copies the identity while admitting the expression; evaluation
consumes the value and binding mode. Display names remain runtime routing data, not a reconstruction
source for the static request transcript.

A producer endpoint borrows its actual plan, function and operation. The Host
request encoder accepts the corresponding HostCall or MakeNeed startup through
this capability. It rejects a foreign plan or a different endpoint operation
before issuing Q.
Argument roles and paths are source ordered: each starts with Operand(ordinal),
named arguments add NamedArgument(accepted role identity), and spread arguments
add SpreadArgument(ordinal). The Host request argument vector owns no additional
request field rows. Literal payloads and display names are not read by this
projection; the actual expression type and admitted source role are retained.

Source classification follows the actual expression without evaluation.
Need-typed sources are NeedHandle. Local reads use the function's accepted
capture prologue to distinguish Capture from Local; field/tuple/record reads
are Projection. Aggregate constructions use AggregateItem. Let/Scope/Assign
wrappers follow their result expression iteratively. Other eager computation
results, including operators and conditionals, use CallResult. Classification,
role/path projection and encoding share the same checked work/byte meter.

MakeNeed startup admits only scalar positional/named bindings compatible with
the selected producer request. It rejects spread and mismatched names. Builder
admission checks the complete argument count before zipping with the type row,
so extra arguments cannot be discarded. Native and AWBC consumers evaluate
the retained expression once in source order; role metadata is static input
evidence. Asset request routing retains the selected formal name even when the
source used positional binding. The request transcript retains that source
binding role and the accepted identity without reconstructing it from names.
