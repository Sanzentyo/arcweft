# Static requirement wire and fragment dispatch

## Requirement owner

Automatic proof still runs for every definition/subtree. `#[static]` adds a mandatory checked requirement. Because runtime does not rerun source sema, the requirement is serialized.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewStaticRequirementResource {
    pub subject: ViewStaticSubjectResource,
    pub requirement: ViewStaticRequirementDigest,
    pub attribute_source: Option<SourceRangeRef>,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewStaticRequirementDigest(BundleDigest);
```

Digest domain:

```text
arcweft.view.static-requirement.v1\0
```

Digest fields are length-delimited canonical values:

```text
ViewProgramIdentity
subject kind
subject program-local coordinate/span identity
literal requirement kind = required_static
```

`attribute_source` is diagnostic evidence and excluded from the requirement digest. Sorted requirement digests are included in `ViewProgramSemanticDigest`; the complete transcript and certificates remain part of `AcceptedViewProgramRevision`.

## Transcript order

```text
schema
accepted_revision
program
value_programs
definitions
resolved_resources
static_requirements
static_fragments
static_certificates
source_map_digest
```

No new AWFB section, outer field, codec tag, or save schema is allocated. The unreleased transcript schema value remains `arcweft.view_program.v1` and is directly replaced.

## Exact joins

For each requirement subject:

- exactly one requirement row;
- exactly one fragment/certificate pair;
- certificate subject equals requirement subject;
- `proof_origin = AuthoredRequired`;
- requirement digest is included in the certificate digest;
- requirement, fragment, and certificate bind the same program semantic digest and accepted revision.

For an unannotated subject:

- no requirement row;
- zero certificates means dynamic execution;
- one valid `Automatic` certificate is allowed;
- `AuthoredRequired` is forbidden.

Any omission, duplicate, origin mismatch, subject mismatch, stale requirement digest, missing fragment, or tampered certificate rejects the candidate before publication.

## Certificate digest correction

The certificate digest adds the optional requirement digest in a tagged field:

```text
proof_origin = automatic
requirement = none
```

or

```text
proof_origin = authored_required
requirement = exact ViewStaticRequirementDigest
```

The two forms cannot canonicalize to the same bytes.

## Fragment span and dispatch

Every product subject has one exact half-open instruction span and parent subject relation under the accepted revision.

Validation rules:

1. identical subject/span duplicates are invalid;
2. sibling spans are disjoint;
3. strict ancestor containment is valid;
4. crossing/partial overlap is invalid;
5. a child span must lie fully inside its declared parent;
6. fragment bytes may reference only resources/lifecycle rows in their certified closure.

Runtime traversal:

1. at subject entry, if no ancestor fragment is active and this subject has a valid certificate, select this fragment;
2. execute the fragment as the subject's render body;
3. suppress all descendant fragment selection until the fragment exits;
4. if the subject is dynamic, traverse normally and allow a certified child at its boundary;
5. sibling fragments are selected independently in authored order.

Thus the outermost available fragment wins. Certificates for descendants may remain as audit/hot-replacement evidence but are not double-executed under a selected ancestor.
