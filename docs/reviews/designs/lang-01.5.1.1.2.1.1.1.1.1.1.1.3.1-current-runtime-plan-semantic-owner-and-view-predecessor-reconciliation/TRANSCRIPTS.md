# Semantic transcripts

## 1. Common grammar

All semantic digests use BLAKE3 and a NUL-terminated version-one domain. A
semantic transcript is purpose-built; generic Serde is never hashed.

```text
u8          one byte
u32-le      four little-endian bytes
u64-le      eight little-endian bytes
digest32    exactly 32 bytes
string      byte_len:u32-le || validated UTF-8 bytes
list<T>     count:u32-le || source/owner-order rows
option<T>   present:u8 (0 or 1) || optional payload
```

This grammar is for semantic hashes, not AWBC wire integers. It does not
reopen the accepted canonical AWBC varint codec.

Every conversion and byte count is checked before allocation or `Hasher`
update. Every Arcweft-owned marker remains `1`.

## 2. Retained task-plan transcript

The accepted seven-role transcript is unchanged:

```text
domain = "arcweft.task.plan-semantic.v1\0"
plan_owner_tag:u8
runtime_executable_semantic_digest:digest32
producer_function_semantic_digest:digest32
need_producer_family_tag:u8
task_class_tag:u8
task_request_template_digest:digest32
control_effect_contract_digest:digest32
semantic_binding
```

For structured core plans, `plan_owner_tag = 0`.

Explicit exclusions remain:

- producer contract and producer site;
- payload type and evaluated arguments;
- generation, launch policy and launch ordinal;
- priority and live cancellation scope;
- task-plan map key, completed/self digest, and expected decoded key;
- accepted View revision;
- source span, HIR/arena/session ID, source spelling and debug label; and
- generic serialized or whole-catalog bytes.

## 3. Semantic binding

```text
0 Ordinary:       no payload
1 View:           ViewProgramId canonical semantic bytes
                  ViewMatchSiteId semantic bytes
                  CheckedViewMatchAdmissionDigest bytes
2 AwaitManyBase:  no payload
3 AwaitManyChild: no payload
4 Timeout:        NeedTimeoutContractDigest:digest32
5 Line:           LinePlanSemanticDigest:digest32
```

For View, core constructs the prefix through
`control_effect_contract_digest` and retains its private hasher in the one-use
request. The request finalizer alone appends tag `1` and the three `.1.4` actual
values in the order above. `AcceptedViewProgramRevision` is validated by the
upper authority and is never written.

The exact `ViewProgramId` string byte accessor and the site/admission borrowed
types are frozen only after `.1.4` intake. No raw byte substitute is authorized
by this blocked revision.

The timeout digest is computed from the final typed timeout contract under:

```text
domain = "arcweft.need.timeout-contract.v1\0"
logical_duration
source_outcome_contract
timeout_outcome_contract
race/cancellation behavior
```

The line digest is computed from the final line-task group under:

```text
domain = "arcweft.line.plan-semantic.v1\0"
captures in accepted order
root node ordinal
nodes in dense preorder with closed topology/trigger/join/cancel tags
action FlowOp semantic children with candidate task coordinates
cancel rules in accepted order
cleanup policy and action children
```

Exact duration/outcome encodings and line child tags are frozen during P3 from
the then-current typed owners. Live TaskId/TaskKey/name/priority never enter
either transcript. Both digests have owner-private constructors.

## 4. Corrected executable envelope

The executable digest retains a fixed table envelope because current
`RuntimePlan` already has fourteen ordered owner families and Cut 5 adds the
task table as the fifteenth:

```text
domain = "arcweft.runtime-plan.executable-semantic.v1\0"
owner_tag:u8 = 0
table_count:u8 = 15
for table_tag in 0..14:
  table_tag:u8
  row_count:u32-le
  for each row in the final owner's canonical order:
    row_ordinal:u32-le
    row_kind_tag:u8
    owner_row_semantic_digest_or_inline_task_base
```

Table order is:

```text
0  runtime type declarations
1  local declarations
2  nominal record domains
3  variant domains
4  function sites
5  dialogue content plans
6  entries
7  callable executables
8  flow executables
9  flows
10 pure helpers
11 trait methods
12 line task groups
13 stream plans
14 structured runtime task base rows
```

This revision does **not** accept the returned archive's claimed payload atom
lists as final. A table row succeeds only when every identity-bearing field is
owned by the final core row and has an exhaustive typed visitor. A row with a
remaining String/raw usize identity returns
`UnsupportedExecutableSemanticOwner { table, row, role }`; it is not hashed
by spelling and does not publish a digest.

### Local row correction

The local row is exactly:

```text
row_kind_tag = 0
runtime_plan_type_ordinal:u32-le
```

There are no storage, mutability, initialization, or function-owner atoms.
Those fields do not exist in the current execution owner.

### Record and variant rows

Their final atom order is structurally fixed:

```text
record:
  owner_type_semantic_digest:digest32
  field_count:u32-le
  each field in declaration order:
    field_ordinal:u32-le
    accepted_field_semantic_identity:digest32
    field_type_semantic_digest:digest32

variant:
  owner_type_semantic_digest:digest32
  accepted_nominal_semantic_identity:digest32
  case_count:u32-le
  each case in declaration order:
    case_ordinal:u32-le
    accepted_case_semantic_identity:digest32
    payload_type:option<digest32>
```

The identity byte constructors/domains remain gated on `.1.2`; therefore
golden bytes are not final yet. Runtime lookup labels are not separately
written.

### Task base row

Table 14 is inline and never contains a completed task digest:

```text
row_kind_tag:u8 = 0
coordinate_ordinal:u32-le  // must equal row_ordinal
producer_function_semantic_digest:digest32
family_tag:u8
task_class_tag:u8
task_request_template_digest:digest32
control_effect_contract_digest:digest32
binding_shape
```

`binding_shape` uses the tags in section 3, but View writes tag `1` without its
upper payload. Candidate Flow/line/View task references write:

```text
task_reference_tag:u8
coordinate_ordinal:u32-le
```

The coordinate issuer is checked in memory and never written. After sealing,
the corresponding public executable edge stores a `RuntimeTaskPlanIndex` with
the same ordinal; the semantic encoder does not hash a completed digest or map
key.

## 5. Producer-function digest gate

The domain remains reserved:

```text
"arcweft.runtime-plan.producer-function-semantic.v1\0"
```

The required structural order is:

```text
accepted_function_semantic_identity
function_role
parameters in declaration order
captures in accepted capture order
result type
typed body
producer endpoints in checked source order
```

Exact identity/path types and their inner transcripts are `.1.2` outputs.
Until those are ingested, `ProducerFunctionSemanticDigest` has no production
constructor and this domain has no accepted golden vector. Hashing current
`RuntimeFunctionSiteId`, function-table ordinal, helper name, raw trait index,
or display label is forbidden.

## 6. Request-template digest gate

The domain is retained:

```text
"arcweft.task.request-template.v1\0"
```

Every final request begins with a closed request-family tag. The Host family
then writes the route-independent semantic digest issued by the resolved
`HostOperationCatalogRow` and source-ordered typed argument roles. It does not
write the current duplicate capability/operation strings or the custom
operation's whole-catalog-bound runtime lookup identity.

```text
request_family_tag:u8
host when tag=0:
  host_operation_request_semantic_digest:digest32
  argument_count:u32-le
  each argument in checked runtime order:
    ordinal:u32-le
    passing_tag:u8  // positional, named, spread
    accepted_name_identity:option<digest32>
    expression_child_role
    runtime_expression_semantic_digest:digest32
```

The catalog-row child is:

```text
domain = "arcweft.host-operation-request-semantic.v1\0"
operation_kind_and_accepted_operation_coordinate
capability:string
HostTaskRequestContract typed transcript
```

It excludes `HostRouteId`, `HostRestartPolicy`, and
`HostCancellationContract`. The existing catalog still owns and validates
those fields for live execution. A private-field `HostOperationPlanAdmission`
couples this child digest to the exact `HostOperationIdentity`; callers cannot
submit the pair independently.

Await, AwaitMany base/child, timeout, line, View, and MakeNeedHandle payloads
must be enumerated from current/landed typed owners during `.1.3.1`
finalization. `.1.4` owns the retained View payload. No unspecified variant may
hash an empty/default row.

Literal/evaluated payload bytes are excluded and remain in runtime value and
producer-instance identity.

## 7. Control/effect digest gate

The domain is retained:

```text
"arcweft.task.control-effect-contract.v1\0"
```

The final digest is the exhaustive inherent traversal of the inline
`RuntimeControlEffectContract`. Its variant tag is followed only by fields
physically present on that contract. The returned archive's generic effect-row
list is rejected because no current `RuntimeControlEffectContract(Id)` owner
exists.

Final tags and payload bytes remain blocked until the current Host/Await/
AwaitMany/timeout/line facts and `.1.4` View subscription facts are mapped into
the final closed enum. There is no accepted empty fallback.

## 8. Source-order and stability rule

An ordinal is semantic only as a role within its final owner:

- table rows use the final table's canonical admitted order;
- fields, cases, parameters, captures, arguments, arms, and child operations
  use their accepted owner order;
- a reference writes the referenced owner's ordinal after issuer/range
  validation; and
- no BTreeMap/hash-map iteration order is used unless the owning type already
  defines that key order as its canonical semantics.

`.1.2` differential tests must prove that HIR arena allocation, spans, and
formatting do not perturb accepted paths/identities. `.1.4` must prove the same
for View sites and retained slots. Task-plan integration consumes those proofs;
it does not recreate them.

## 9. Cycle exclusions

The executable encoder must have no read access to:

- `TaskPlanSemanticDigest`;
- the task digest lookup map;
- private expected keys;
- `NeedProducerSpec.plan`;
- View accepted revision; or
- a final task-plan index as a key rather than a candidate role ordinal.

A typed visitor dependency graph test must establish that every edge descends
to a finite final row, accepted semantic identity leaf, or owner-checked
coordinate leaf. Encountering a forbidden structural cycle rejects before any
task digest is returned.
