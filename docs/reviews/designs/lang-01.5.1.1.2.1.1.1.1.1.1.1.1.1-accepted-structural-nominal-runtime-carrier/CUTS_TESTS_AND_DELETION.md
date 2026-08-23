# Compile-clean cuts, tests, and deletion

Each cut is one reviewable implementation commit, validated and pushed before
the next cut. No cut may publish structural ownership success before C6.

## C1 - core schema and identity substrate

Implement in arcweft-core:

- RuntimeNominalRecordShape and the validated RuntimeNominalSchemaGraph;
- RuntimeTypeSchema Tuple, Result, RecordValue, ExactOpaque, and NominalRef;
- one shared shortest-u32-varint helper used by schema/value/AWBC internals;
- layout transcript/golden tests, including recursive reachable graphs;
- RuntimeCheckedType Record and layout-bearing nominal Variant/identity; and
- shape/field-ID additions to nominal record layout, while retaining the old
  value constructor temporarily as crate-private migration-only call sites.

Delete in C1: duplicate schema encoder helpers and fixed-u32 length/ordinal
helpers at touched canonical schema/value sites. Do not add AcceptedRuntimeCarrier.

Gate: core checks/tests, formatting, Clippy on touched targets, structure audit,
and source negatives. Public structural accepted success remains absent.

## C2 - accepted catalog join and final-analysis graph

Implement in adapter-sema and lang-sema:

- RustAdt semantic role and split opaque/Rust publication constructors;
- removal of Rust metadata's fabricated opaque producer publication;
- ordered enum record fields and duplicate/empty-name rejection;
- retained metadata publication item and bijective atomic registration join;
- AcceptedRustProjectionStamp and generation-bound final-analysis projection;
- Unseen/Visiting/Complete graph traversal keyed by instantiated semantic ID;
- exact opaque leaf projection and deterministic work/error precedence; and
- same graph reuse by the ownership classifier without publishing success.

Delete in C2: BTreeMap record payload shape, metadata-kind fallback, opaque
producer on adapter Rust declaration inputs, and structural marker WIP.

Gate: sema/adapter tests plus compile fixtures for unit/tuple/record/newtype,
duplicates, generics, self/mutual recursion, nested opaque, stale world, and
metadata-only input.

## C3 - compiler and existing RuntimePlan authorities

Implement:

- in-place ProjectNominal-to-Nominal spelling migration in the generic plan
  projection and all exhaustive matches;
- general RuntimeResolvedNominal source provenance and nominal variant owner;
- record domain shape/field IDs/optional names and variant domain layout;
- atomic RuntimePlanBuilder admission of the validated schema graph;
- graph-aware RuntimePlan::accepts_value;
- compiler stamp revalidation and one-batch publication of every reachable
  nominal definition; and
- private checked RuntimeVariantValue plus migration of every live constructor.

Delete in C3: old Project-only enum cases, raw variant struct construction,
copied compiler schema projection, domain name-to-ordinal helpers, and any
side table retained after plan seal.

Gate: runtime-plan/core/compiler tests, recursive domain tests, native/AWBC
plan parity, and compile-fail visibility tests.

## C4 - AWBC type/constant rows and canonical codecs

Implement existing-tag in-place rows from WIRE_AND_RESTORE.md, update lowering,
verification, VM, type projection, constants, patterns, and all fixtures.
Record/variant constant names derive only from accepted type rows.

Delete in C4: constant field_names, constant case_name, old NominalRecord field
grammar, variant identity without layout, and any provisional/new tag.

Gate: exact fragments and whole-program v1 goldens; canonical round trips;
300/ac02 varint positive; overlong, sixth-byte, unknown-tag, wrong field ID,
wrong shape/name/layout, and trailing-byte negatives.

## C5 - authority-bound snapshot restore

Implement program-context snapshot/restore for every fiber, product-step, task
publication, closure capture, iterator, reduction, and agent recursive site.
Add semantic identity and explicit field IDs to nominal record DTOs and exact
type/layout checks. Build private candidates and retain whole-session swap.

Delete in C5: context-free snapshot conversions, public unchecked
RuntimeNominalRecordValue::new, raw nominal_into_live, and every restore route
that constructs a value before resolving the current program.

Gate: exact snapshot JSON, nested recursive round trips, wrong
program/type/identity/layout/field/case/opaque negatives, work-limit
transactions, and unchanged state/digest assertions on failure.

## C6 - ownership success and final deletion gate

Replace the current structural MissingRuntimeSnapshotOwner branch with exact
Record or Variant success only after C1-C5. The classifier returns the current
RuntimeOwnershipProjection/certificate and validates the live value through
the accepted graph before canonical digest exposure. Opaque is unchanged.

Delete in C6: temporary fail-closed branch, obsolete all-Rust-ADT rejection
fixtures, old constructors/helpers, compatibility comments, and dead arms.

Gate: full differential/structural gates, workspace check/tests, format,
touched-crate Clippy, structure audit, diff check, then commit/push.

## Required positive matrix

| Family | Required proof |
|---|---|
| struct | unit, tuple0/1/N, record0/1/N, newtype |
| enum | unit, tuple0/1/N, record0/1/N in source case order |
| nested | Tuple, Seq/Vec, Option, Result, structural Record, exact opaque |
| generic | one/two parameters, repeated instance, distinct arguments |
| recursive | self record, self variant through Option, mutual record/variant, generic recursion |
| identity | same layout at sema, compiler, plan, AWBC, live, snapshot, restore |
| ordering | field/case reorder changes layout; catalog reorder does not |
| persistence | canonical value digest and snapshot round trip |

## Required negative matrix

Each test asserts no bytes, digest, plan row, live value, snapshot result, or
state mutation becomes visible.

- duplicate/empty field and case names;
- missing metadata, metadata-only row, opaque-plus-metadata, wrong item, owner,
  origin, arity, package provenance, world, revision, or environment digest;
- unbound generic, nonnominal cycle, dangling ref, conflicting definition, and
  every work limit exactly one over;
- wrong record shape, field count/ID/name/order/type, identity, or layout;
- wrong variant owner/layout/ordinal/name/payload presence/type;
- wrong Option/Result convention and one-field tuple flattening;
- wrong opaque producer, admission, identity, class, persistence, or argument;
- unknown/colliding tag, noncanonical varint, bad range, duplicate row, trailing
  bytes, and old row grammar;
- snapshot restored against a different program/type table; and
- source gates for AcceptedRuntimeCarrier, invented runtime crate/path, V2/V3,
  legacy/migration/old reader, source reconstruction, copied catalog, public
  unchecked constructor, and context-free snapshot conversion.

## Validation commands per final cut

Run without an explicit Cargo job count:

    cargo fmt --all -- --check
    cargo check -p arcweft-core -p arcweft-lang-sema -p arcweft-adapter-sema \
      -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-runtime-driver \
      --all-targets --all-features
    cargo test -p arcweft-core -p arcweft-lang-sema -p arcweft-adapter-sema \
      -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-runtime-driver \
      --all-targets --all-features
    cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking
    git diff --check

Run the design validator/negative corpus before C1 intake and again against
the implementation commit with refreshed source evidence.
