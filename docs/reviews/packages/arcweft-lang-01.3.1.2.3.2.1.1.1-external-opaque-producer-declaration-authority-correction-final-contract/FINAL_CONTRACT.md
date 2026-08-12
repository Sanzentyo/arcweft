# Final contract

## 1. Authority and scope

This document closes all result-changing decisions in the attached request at
repository commit `78f50f5b5ac082745bab91b7373a6602918a436d`. The package is normative only for external
opaque-producer declaration authority. The parent package remains authoritative
for runtime opaque values, owners, admission, checked-type projection,
composite and variant behavior, AWBC, persistence, and all unrelated ownership
surfaces.

## 2. Sole lower-layer owners

`arcweft-adapter-context::manifest::nominal` owns
`AdapterOpaqueTypeProducerId`. `arcweft-rust-abi::producer` owns
`ArcweftRustOpaqueTypeProducerId`. These are separate newtypes because neither
lower crate may depend on core or sema. Their spelling contract is identical to
core runtime identity: exact nonempty UTF-8 with no Unicode control character.
They perform no trimming, case folding, Unicode normalization, path parsing, or
name derivation. They impose no ID-specific byte limit; existing bounded input,
string, manifest, and work-accounting limits remain the allocation boundary.

The public constructors additionally reject the exact case-sensitive prefix
`std.`. `std` without a dot and differently cased prefixes are ordinary IDs.
Only fixed core and CharacterDialogue constructors may create reserved
producers, and they do so directly in the core-owned type rather than through
an external descriptor bypass.

## 3. Mandatory descriptor evidence

`AdapterNominalDeclaration` gains a private mandatory
`opaque_producer: AdapterOpaqueTypeProducerId` field. Its validating constructor
accepts that field between `arity` and `visibility`; the exact schema-2 key is
`nominal_types[].opaque_producer` in JSON and TOML.

`ArcweftRustTypeDecl` gains public mandatory
`opaque_producer: ArcweftRustOpaqueTypeProducerId`; the exact schema-2 JSON key
is `types[].opaque_producer`. Programmatic declarations must construct the
validated newtype explicitly. A manifest with zero type declarations needs no
producer datum; functions do not acquire one.

`AdapterRustType` acquires no producer field and no setter. Its accessor returns
`self.decl.opaque_producer()`. Cloning the existing whole declaration during
package mounting preserves authored evidence but creates no separately mutable
or overriding authority.

## 4. Hard schema cuts

Adapter manifest schema 1 and Rust ABI schema 1 are deleted. Schema 2 is the
only successful version. Public manifest decoding is available only through
version-preflighting entry points. Direct `Deserialize` on the public manifest
root and declaration rows is removed where it would bypass preflight; private
schema-2 wire DTOs perform body decoding after a supported header has been
proved.

Raw JSON/TOML syntax and header shape are decoded first, then schema support,
then producer presence/type, spelling, reserved namespace, remaining body
validation, package mount/Rust ABI validation, duplicate/capacity/work checks,
and atomic publication. A schema-1 document with no producer always reports
`UnsupportedSchema`, never `MissingOpaqueProducer`.

## 5. Derive authority

`#[derive(ArcweftType)]` accepts exactly one mandatory helper option:

```rust
#[derive(ArcweftType)]
#[arcweft(opaque_producer = "example.gameplay")]
struct PlayerScore { value: i64 }
```

The macro validates the decoded `LitStr` value with the Rust-ABI-owned spelling
contract. It never derives the producer from the Rust item name, accepted path,
crate, package, or generated metadata. Missing, duplicate, malformed, unknown,
non-string, empty, control-containing, and reserved values have fixed diagnostics
and spans in `SCHEMA_2_CODEC_AND_DERIVE.md`.

## 6. Exact external admission

External descriptors do not gain an admission field. Adapter-native and
Rust-export rows always become `RuntimeOpaqueTypeAdmission::ExactIdentity` at
checked-type projection. `ProducerWide` remains confined to the parent's fixed
CharacterDialogue semantic top. Unknown descriptor keys such as `admission`
are rejected as ordinary schema-2 body errors.

## 7. Sema publication

`arcweft-adapter-sema` converts both lower-layer ID types to
`RuntimeOpaqueTypeProducerId` through one private owner enum with inherent
methods. It repeats core spelling validation defensively and rejects `std.`
with typed variants carrying source kind, exact producer string, and exact
producer payload span. It never converts an error to an unstructured string.

`AcceptedNominalInventoryInput` gains a mandatory private
`runtime_producer: RuntimeOpaqueTypeProducerId`. Catalog construction calls
`AcceptedNominalRecord::try_new_opaque` with that field. Producer evidence is
retained by accepted type instantiation and cloned unchanged by generic
substitution. Missing producer is unrepresentable after validated publication.

## 8. Shared producer domains

Producer strings are not unique keys. Multiple declarations, packages, and
generic instantiations may intentionally use the same non-reserved producer.
Catalog collision checks remain based on accepted nominal identity. Equal
producer plus unequal semantic identity creates two exact owners, not one row
and not a collision. No producer registry, side table, uniqueness index,
callback, trait object, or schema publication is introduced.

## 9. Canonical generated evidence and digests

Generated registration source uses the header `adapter-manifest-v2` and writes
`opaque-producer=<utf8-byte-length>:<exact-bytes>` in every nominal and Rust type
row. The source map owns the payload range only. Existing semantic sort order
is unchanged.

The adapter environment-manifest digest moves from domain
`arcweft.environment-manifest.v1\0` to
`arcweft.environment-manifest.v2\0` and inserts producer bytes in nominal and
Rust type rows. The accepted nominal catalog moves from
`arcweft.accepted-nominal-catalog.v1\0` to v2 and hashes the producer-bearing
semantic row. The external type-input v1 digest remains unchanged. Rust ABI's
BLAKE3 over deterministic pretty JSON remains the same algorithm; schema 2 and
the explicit field change its bytes. Generated source has no independent
identity owner.

The semantic type identity digest remains nominal declaration identity plus
normalized arguments. It must be implemented explicitly rather than hashing the
new producer-bearing `AcceptedNominalType` as a whole. Producer-only changes
therefore alter registration/catalog/artifact identity while leaving semantic
nominal identity unchanged.

## 10. Migration and completion

Implementation is deletion-driven and starts in the two lower descriptor
owners. Rust ABI schema 2 and adapter manifest schema 2 may be separate
compile-clean commits. The adapter-sema and lang-sema mandatory publication
switch is one protected atomic merge group: no commit in that group is
releasable with producerless successful publication. All schema-1 readers,
writers, goldens, producerless constructors, derive success without the helper
attribute, temporary fallback, compatibility carrier, and post-build overlay
are deleted at the owning gate.

After all focused and workspace gates pass, parent A1.2 may resume. This
package has `OPEN_QUESTIONS=0`.
