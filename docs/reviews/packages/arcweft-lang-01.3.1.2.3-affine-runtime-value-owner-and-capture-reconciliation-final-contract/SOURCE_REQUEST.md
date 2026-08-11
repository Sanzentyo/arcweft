# Lang-01.3.1.2.3 — affine runtime value owner and capture reconciliation

## Sequence position and precedence

This is Lang-01.3.1.2.3. It follows the returned and accepted
Lang-01.3.1.2.1, Lang-01.3.1.2.2, and Lang-01.3.1.2.2.1 contracts and must
return before their P4+C1 core publication boundary is implemented.

The returned Stream contracts remain authoritative for grouped callable
coordinates, `RuntimeFunctionValue::ExternalStreamPartial`, the canonical
argument product, `StreamInstanceKey`, the sole instance table, lifecycle,
replay, host, AWBC, bundle, and save boundaries. This request corrects one
repository-evidenced premise only: those contracts refer to an existing ABI-2
affine runtime-value owner, but current production has no such owner.

Accepted and validated syntax, HIR, sema callable resolution, direct
suspension, ordinary-function roles, Proof identity, and Agent runtime work
must not be redesigned without a new concrete production defect.

## Inspected production evidence

The clean pushed baseline is Git commit
`177ba1e61e43fb2da2149869ce35e165d1e93b66`.

Current production demonstrates the missing boundary:

- `crates/arcweft-core/src/value.rs` derives `Clone` for `RuntimeValue`,
  `RuntimePayload`, `RuntimeBinding`, `RuntimeFunctionValue`, `RuntimeExpr`,
  aggregate carriers, and sequence storage.
- `RuntimeFunctionValue` is one closure struct, and
  `RuntimeFunctionValue::partially_apply` clones existing captures and supplied
  arguments.
- structured closure construction uses `RuntimeEnv::bindings_snapshot()` and
  captures the complete visible environment by clone rather than an affine
  transfer selected from typed capture evidence.
- `RuntimeIterator::Values::next` returns `items.get(index).cloned()`, so an
  affine sequence element would be duplicated rather than moved.
- sequence repeat, get, and slice paths, pattern binding, environment lookup,
  call application, AWBC registers/fibers, snapshots, and product-step facade
  synchronization all currently rely on unconditional value cloning.
- removing `Clone` from the direct value/closure/aggregate owners produces 322
  core compile errors before downstream crates are checked. The errors expose
  real ownership consumers; they are not solved by adding a Stream-only clone
  exception.

No production edit from that diagnostic experiment is retained.

## Split reason

The returned Stream contract requires an affine `StreamHandle` and allows an
external partial to become affine when any captured argument is affine. It also
requires the public runtime value model not to expose an unconditional clone
that can duplicate either owner. Current production cannot represent those
rules.

Choosing how closures capture affine values, whether sequence indexing borrows
or moves, what repeat means for affine values, and how snapshot candidate copies
differ from language duplication changes observable language/runtime behavior.
Those decisions are not private implementation algorithms. Implementing one by
local judgment would guess a missing cross-runtime contract and could make the
structured runtime, AWBC VM, compiled regions, save/restore, and Stream table
disagree.

These decisions must be designed together because they share one value owner
and one transfer graph. Splitting them would permit incompatible closure,
aggregate, and VM ownership models.

## Required decisions

1. Define the sole generic runtime ownership classification and token/evidence
   owner used by `RuntimeValue`, aggregates, closures, external Stream partials,
   and `StreamHandle`.
2. Define the public Rust API boundary exactly:
   - which runtime carriers must not implement `Clone`/`Copy`;
   - the exact checked unrestricted-duplication operation and error type;
   - the exact move/transfer/drop operations; and
   - the distinct snapshot-candidate copy operation, if any, including why it
     cannot create two runnable owners.
3. Define structured closure capture from accepted typed evidence:
   - whether only referenced free bindings or another exact set is captured;
   - evaluation and transfer order;
   - unrestricted copy versus affine move;
   - partial application and nested closure behavior;
   - failure atomicity; and
   - the typed compiler/runtime carrier for capture intent. Capturing the whole
     environment by clone must not remain as a fallback.
4. Define affine behavior for every generic value operation that can currently
   clone or fan out a value: local lookup, `let`/pattern binding, tuple/record/
   sequence construction, field/tuple projection, variant payloads, rest
   binding, call arguments, return, closure capture, partial application,
   assignment, cross-fiber transfer, iterator construction/next, sequence
   repeat/get/slice/push, equality, and drop/unwind cleanup.
5. State which operations borrow, move, or require unrestricted values. In
   particular, close the language/runtime result for indexing and slicing an
   affine-containing sequence and for repeating an affine value.
6. Define the corresponding AWBC ABI-2 register/frame rules and verifier facts:
   move versus copy instructions, aggregate operand consumption, branch joins,
   cleanup, safe points, child-fiber exchange, compiled-region parity, and trap
   atomicity. Do not introduce a second Stream-specific register model.
7. Define snapshot/save/restore ownership:
   candidate construction, unique token/lease occurrence, original-versus-
   candidate exclusivity, tamper rejection order, failed-restore cleanup, and
   exact generation pin traversal for affine external partials and handles.
8. Define host/replay/persistent eligibility. Stream handles and affine partials
   must not leak through general `RuntimePayload` or canonical data codecs unless
   the final contract explicitly supplies their owning typed boundary.
9. Reconcile current `RuntimeExpr::Value(RuntimeValue)`, plan cloning, AOT/JIT
   plan caches, and test fixture construction. Select either a checked
   unrestricted plan-value carrier or another single-owner design; do not add a
   parallel runtime value model or a panic-on-Clone implementation.
10. Provide the compile-clean interleave with Lang-01.3 P4+C1 through P8+C6,
    including the exact point at which unconditional clone APIs disappear and
    the point at which Stream handles become constructible.

## Required consumer inventory

The returned package must inspect and cover at least:

- `arcweft-core::value`, `value::range`, sequence constructors/implementations,
  `pattern`, structured engine environment/evaluation/suspension, AOT plans;
- AWBC schema/codec/verifier/VM/fiber/product-step/snapshot and compiled-region
  exchange;
- `arcweft-runtime-plan` lowering/constants, runtime accelerator/JIT, runtime
  driver save/restore/swap, runtime host, native/Web/Agent adapters;
- bundle and canonical value codecs; and
- Lang-01.3 grouped partial/product/handle/table owners from the three returned
  parent packages.

## Required implementation order

1. Freeze the generic ownership classification, transfer graph, checked
   duplication, capture evidence, and snapshot candidate boundary.
2. Replace structured runtime closure/environment/aggregate clone paths and add
   direct ownership tests while no Stream handle is constructible.
3. Replace AWBC/fiber/compiled-region clone paths and land verifier ownership
   facts in the protected ABI-2 cut.
4. Apply Lang-01.3 P4+C1 using the returned grouped function/argument/handle
   types on the now-final generic owner.
5. Complete RuntimePlan, codec, host, bundle, save/restore, and hot-reload cuts
   without a Stream-only ownership sidecar.
6. Delete obsolete unconditional clone/snapshot/facade paths and run the parent
   full matrix.

## Tests to specify

- unrestricted scalar, aggregate, closure, and external partial duplication;
- direct and recursively nested affine duplication rejection;
- affine capture, nested capture, partial application, call, return, move,
  cross-fiber transfer, drop, and use-after-move;
- exact evaluation/transfer order and failure non-mutation;
- iterator-next moves each affine element once; repeat/get/slice behavior follows
  the selected typed rule with exact one-over/boundary cases;
- branch/match ownership joins and unwind cleanup;
- structured runtime, AWBC VM, and compiled-region parity;
- snapshot candidate exclusivity, duplicate token/lease tampering, failed restore
  atomicity, generation pins, and no open/evaluation replay;
- general payload/replay/host rejection for affine-only values;
- compile-fail tests proving removed `Clone`/`Copy` surfaces are unreachable;
- full Lang-01.3 partial/open/handle/save matrix after integration; and
- workspace check, strict Clippy, Tier 2, Cargo metadata, and structure audit.

## Constraints and non-goals

- Do not implement `Clone` by panicking, silently sharing, rotating a lease
  without the owning table, or relying on later verifier cleanup.
- Do not add an affine side table keyed by debug strings, a Stream-only value
  enum, a second environment, a copied capture registry, or source-text free-
  variable reconstruction.
- Do not add compatibility aliases, dual readers, migration shims, endpoint
  DTOs, source gates, removed-syntax diagnostics, CSS, or Takumi.
- Keep core and data crates Sans I/O and preserve layer direction.
- Do not redesign callable selection/accounting, group coordinates, Stream
  lifecycle/replay/policy, Proof identity, or ordinary-function syntax.

## Expected output

Return one independently usable final-contract archive named
`arcweft-lang-01.3.1.2.3-affine-runtime-value-owner-and-capture-reconciliation-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owners/APIs, structured and
AWBC transfer semantics, snapshot/save rules, a supersession delta against the
three returned Lang-01.3 packages, a complete consumer/deletion inventory, an
ordered compile-clean implementation plan, and a positive/negative/tamper/full-
matrix test plan.
