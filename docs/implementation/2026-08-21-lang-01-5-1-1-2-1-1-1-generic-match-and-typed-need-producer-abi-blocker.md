# Lang-01.5.1.1.2.1.1.1 generic Match and typed Need producer ABI blocker

Date: 2026-08-21
Inspected Git commit: `dec4f6c2de3be87d28a2f976b1ae51e3b40dd3fd`

## Status

- Corrected package classification: `ACCEPTED_DESIGN_CONTRACT`
- Language/lifecycle decisions: accepted
- Product/runtime implementation readiness: `BLOCKED_PENDING_CORRECTION`
- Independently safe work: generic checked Match semantic authority only
- Production changes made by this audit: none

The post-intake implementation audit found two result-changing ABI gaps that
the returned design-validation package describes inconsistently with current
production. Implementing either by convention would create a new authority
rather than project the accepted one.

## Blocking evidence

### Generic Match output

The returned schema places `AwbcRegisterId` in `ViewMatchArmBinding` while also
requiring `arcweft-view` to remain independent of `arcweft-core`.
`AwbcFunctionSignature.result` is one optional type
(`crates/arcweft-core/src/awbc/schema.rs:776`), `AwbcTerminator::Return` carries
one optional register (`schema.rs:1386`), and the VM clones that value before
`FiberState::finish_return` pops the callee frame
(`vm.rs:1264`, `fiber.rs:1295`). Function-local registers therefore cannot be
the post-return binding interface.

The correction must select one ordinary single-value result ABI, exact
type/digest/wire ownership, and a dependency-safe mapping from result payload
ordinals to View locals. A synthetic closed variant keyed by source arm with a
tuple of source-ordered binding values is the leading candidate, but this is a
persistent ABI choice and is not selected by implementation code.

### Typed Need producer

The returned contract requires typed `NeedHandle(T)`, direct `NeedId`
extraction, and no RuntimeValue string surrogate. Current AWBC owns only
unparameterized `AwbcRuntimeType::NeedHandle`
(`crates/arcweft-core/src/awbc/schema.rs:667`), accepts
`RuntimeValue::String` for it (`fiber.rs:2274`), and converts that string to
`NeedId` in `await_target` (`vm.rs:1318`).

The correction must select the typed handle carrier, result and task binding
ABI, verifier/VM extraction API, ownership/persistence behavior, and deletion
of the old string admission. The package's evidence claim that this typed
non-string route already exists is incorrect at the inspected commit.

The generic Match guard path also needs an explicit selection. The verifier
checks guard function signatures, but the current VM Match terminator selects
the first pattern match without invoking the guard
(`crates/arcweft-core/src/awbc/vm.rs:1103`). The runtime-plan lowerer already
has structural pattern/guard/Branch lowering, so reusing that route is the
smallest current-safe candidate; the correction must make the choice explicit.

### Constructibility corrections

The package also uses declaration-oriented `TypeId` for inferred expression
and local types, a nonexistent match-arm expression ID, and undefined runtime
type/ownership names. The correction must map these to current normalized
`TypeKind` and exact HIR arm coordinates, or define one total projection with
constructible owners.

The `ResourceTypeRegistryDigest` route is not independently blocked: the
acyclic owner is `arcweft-resource-model`, which can be borrowed through
`FinalSemanticCatalogs` and supplied by the compiler's existing resource-type
registry. The correction request asks that exact route be made normative.

## Safe next cut

Before the ABI correction returns, a sema-only generic checked Match cut can
replace `CheckedExpressionResolution::Structural` for Match with one complete
`CheckedMatch` fact retaining scrutinee, source-ordered HIR arms, normalized
types, effects, and coverage. It must be generic to all Match expressions and
must not create a Need-only side table, empty checked View catalog, product
codec, selector result, or runtime placeholder.

The complete parent `CheckedViewCatalog` can follow only when it references
that one Match authority without copying arms/bindings. Product/runtime Match,
typed unary-Need producer binding, journal publication, and old Await deletion
remain blocked pending acceptance of
[`Lang-01.5.1.1.2.1.1.1`](../reviews/requests/2026-08-21-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction.md).

No Rust, manifest, fixture, generated artifact, build, test, Clippy, or
platform validation was changed or run for this documentation-only audit.

## 2026-08-21 returned-package reconciliation

The returned Lang-01.5.1.1.2.1.1.1 archive is retained and internally valid,
but repository reconciliation found further result-changing blockers. Product,
runtime, and generic checked-Match implementation remain blocked by
[`Lang-01.5.1.1.2.1.1.1.1`](../reviews/requests/2026-08-21-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction.md).
See the
[return intake](2026-08-21-lang-01-5-1-1-2-1-1-1-generic-match-typed-need-return-intake.md)
for the verified archive identity and the opcode, non-View task identity,
coverage, ownership, digest, and request-copy findings.
