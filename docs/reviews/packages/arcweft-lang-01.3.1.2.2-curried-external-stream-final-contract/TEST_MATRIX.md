# Complete acceptance test matrix

Every row is required. “Unchanged” means byte-for-byte or `Eq` equality of all
observable state owned by that layer, excluding diagnostics explicitly returned by
the attempted operation.

## A. Positive and parity cases

| ID | Layer | Scenario | Required result |
| --- | --- | --- | --- |
| P-001 | sema/compiler | Two groups, positional-only parameters | Accepted resolver coordinates project as `(0,0)`, `(1,0)` with no lookup by name. |
| P-002 | sema/compiler | Three groups, positional-or-named parameters supplied positionally | Signature has three groups; authored order and canonical coordinate order are both retained. |
| P-003 | sema/compiler | Three groups, positional-or-named parameters supplied by names in reverse declaration order | Values evaluate in source order; product cells are declaration-coordinate order. |
| P-004 | sema/compiler | Named-only parameters | Names map through accepted resolver evidence and remain signature fingerprint inputs. |
| P-005 | sema/compiler | Defaulted parameter in group 0 | Default plan/fingerprint belongs to group 0 and no later application contains the default expression. |
| P-006 | sema/compiler | Optional omitted parameter | Canonical slot is `OmittedOptional`; coordinate cell still exists. |
| P-007 | sema/compiler | Empty middle group | `completed_groups` advances across the empty group even though no coordinate is added. |
| P-008 | sema/compiler | Positional rest with zero values | One empty `RestPositional` cell is emitted. |
| P-009 | sema/compiler | Positional rest with three values | Values retain authored order inside the one rest cell. |
| P-010 | sema/compiler | Named rest with three names authored out of order | Expressions evaluate in source order; entries are stored in UTF-8 name order. |
| P-011 | sema/compiler | Imported/re-exported alias of the same declaration | Declaration digest and coordinates equal the original declaration; visible alias is not runtime identity. |
| P-012 | sema/compiler | Two different declarations with textually equal signatures | Declaration digests and signature fingerprints differ. |
| P-013 | runtime | Initial external callable value | `next_group=0`, empty product, no instance, no request. |
| P-014 | runtime | Apply first of two groups | Arguments/defaults evaluated once; returns partial with `next_group=1`; zero open requests. |
| P-015 | runtime | Apply first and second of three groups | Each commit advances exactly one; all earlier cells remain unchanged; zero open requests. |
| P-016 | runtime | Explicit expressions with observable counters | Counter sequence equals authored source order exactly once. |
| P-017 | runtime | Two selected defaults | Defaults evaluate exactly once in ascending parameter coordinate after authored expressions. |
| P-018 | runtime | Unrestricted captured values | Partial ownership is unrestricted and checked duplication preserves product equality. |
| P-019 | runtime | One affine captured value in rest aggregate | Partial and aggregate are affine; one owner token exists. |
| P-020 | runtime | Drop non-final partial | Captures are released according to affine rules; no close/open request is emitted. |
| P-021 | runtime | Final group of two-group call | Exactly one instance, Opening state, handle, and full open request commit. |
| P-022 | runtime | Final group of three-group call | Request contains every coordinate from all three groups in strict order. |
| P-023 | runtime | Single-group external Stream call | Initial callable is opened by `OpenStream` directly; full product has `completed_groups=1`. |
| P-024 | runtime | All groups empty | Final application emits one request with `completed_groups=group_count` and empty vectors. |
| P-025 | parity | Direct chained application versus staged local partials | Full products and argument fingerprints equal; request differs only in allocated instance when allocator histories differ. |
| P-026 | parity | Structured RuntimePlan versus AWBC VM | Equal effects, partials, open request, handle types, and state transitions. |
| P-027 | parity | AWBC VM versus compiled region across FiberState handoff | Partial/function value and group cursor survive tier exchange exactly. |
| P-028 | AWBC | Encode/decode/re-encode valid two-group program | Canonical bytes are identical. |
| P-029 | AWBC | Encode/decode/re-encode valid three-group program with default/rest | Canonical bytes are identical. |
| P-030 | AWBC | String-table canonicalization | Parameter names/capability/operation remap correctly; digests and coordinates do not change. |
| P-031 | host | Native/Web/Agent serialize the same request | UTF-8 bytes are identical. |
| P-032 | host | Wide integer payloads | All wide/runtime integers use canonical decimal strings on every host. |
| P-033 | host | Structural coordinate/count integers | JSON numbers decode exactly and remain within `u16`. |
| P-034 | save | Snapshot after group 1 of 3 and restore | Partial product/next group/generation/ownership equal; zero reevaluations and zero opens. |
| P-035 | save | Snapshot with empty applied group | `completed_groups` survives even with no cells. |
| P-036 | save | Snapshot with defaulted/optional/rest captures | Dispositions, default digest, order, types, value digests, and affine owners survive. |
| P-037 | save | Snapshot during safe suspended expression evaluation | Exact cursor and already evaluated temporaries resume without replay. |
| P-038 | hot reload | Identical group/signature layout, code-only compatible change | Swap accepted; existing partial stays pinned; new callable uses new generation. |
| P-039 | hot reload | Identical code and content-only change | Partial remains valid and no fingerprint field changes. |
| P-040 | hot reload | Code-generational signature change with retained old generation | Existing partial completes against old definition; new calls use new definition. |
| P-041 | limits | Exactly 16 groups and 128 total parameters | Accepted; final product has exact complete coordinate coverage. |
| P-042 | rest | Unicode named-rest keys | Canonical order uses UTF-8 bytes, not locale collation. |
| P-043 | hashing | Equivalent direct/staged full products | Argument fingerprints are equal. |
| P-044 | hashing | Same values under different declaration | Argument fingerprints differ. |

## B. Typed negative cases

| ID | Layer | Scenario | Required result |
| --- | --- | --- | --- |
| N-001 | signature | Zero groups | Typed signature error. |
| N-002 | signature | 17 groups | Group-limit error before allocation beyond budget. |
| N-003 | signature | 129 total parameters | Parameter-limit error. |
| N-004 | signature | First group marked `Curried` | Group-kind error. |
| N-005 | signature | Later group marked `Initial` | Group-kind error. |
| N-006 | signature | Noncontiguous group index | Coordinate/index error. |
| N-007 | signature | Noncontiguous parameter index | Coordinate/index error. |
| N-008 | signature | Stored coordinate differs from vector position | Coordinate/index error. |
| N-009 | signature | Duplicate name in one name-visible group | Signature name-collision error. |
| N-010 | signature | Two positional-rest parameters in one group | Signature rest-shape error. |
| N-011 | signature | Two named-rest parameters in one group | Signature rest-shape error. |
| N-012 | signature | Rest parameter marked optional/defaulted | Illegal presence error. |
| N-013 | compiler | Accepted slot lacks coordinate evidence | Compiler invariant error; no source recovery. |
| N-014 | compiler | Checked sema index does not fit runtime `u16` | Checked projection error. |
| N-015 | compiler | RuntimePlan production tries to depend on sema | Cargo/dependency architecture test fails. |
| N-016 | runtime | Apply group 1 to initial partial | `GroupNotNext`; no evaluation or mutation. |
| N-017 | runtime | Repeat already captured group | `GroupNotNext`; original partial unchanged. |
| N-018 | runtime | Retrograde group after group 2 | `GroupNotNext`; original partial unchanged. |
| N-019 | runtime | Group out of range | `GroupOutOfRange`; no evaluation. |
| N-020 | runtime | Wrong Stream definition | `WrongDefinition`; no evaluation/open/state mutation. |
| N-021 | runtime | Foreign declaration with equal visible name/signature | `ForeignDeclaration`; no evaluation/open/state mutation. |
| N-022 | runtime | Signature fingerprint mismatch | `SignatureMismatch`; no evaluation/open/state mutation. |
| N-023 | runtime | Stale/unretained generation | `StaleGeneration`; no evaluation/affine move/open. |
| N-024 | product | Coordinate/value length mismatch | Typed length error; candidate rejected. |
| N-025 | product | Missing required coordinate | `MissingCoordinate`; no commit. |
| N-026 | product | Duplicate coordinate | `DuplicateCoordinate`; no commit. |
| N-027 | product | Unknown coordinate | `UnknownCoordinate`; no commit. |
| N-028 | product | Coordinates reordered | `OutOfOrderCoordinate`; implementation must not sort them. |
| N-029 | product | Cell for a later group in prefix | Completed-group/unknown-coordinate error. |
| N-030 | product | Required parameter uses `OmittedOptional` | `IllegalDisposition`. |
| N-031 | product | Optional parameter uses `Defaulted` | `IllegalDisposition`. |
| N-032 | product | Defaulted parameter carries wrong default digest | `DefaultFingerprintMismatch`. |
| N-033 | product | Non-rest parameter uses rest disposition | `IllegalDisposition`. |
| N-034 | product | Wrong checked value type | `TypeMismatch`. |
| N-035 | product | Positional-rest member wrong type | `MalformedPositionalRest`/`TypeMismatch`. |
| N-036 | product | Named-rest duplicate key | `DuplicateNamedRestEntry`. |
| N-037 | product | Named-rest keys out of canonical order | `OutOfOrderNamedRestEntry`; implementation must not sort them. |
| N-038 | product | Named-rest value wrong type | `TypeMismatch`. |
| N-039 | affine | Same affine token appears in two cells | Affine ownership error; no commit. |
| N-040 | affine | Apply an already moved affine partial | Use-after-move error; no open. |
| N-041 | affine | Attempt unrestricted clone of affine partial | Checked duplication error. |
| N-042 | runtime | Instance ID allocator overflow on final apply | `InstanceIdOverflow`; partial/request/state remain unchanged. |
| N-043 | runtime | Host request batch capacity/budget failure | Prepared transaction rejected before live mutation. |
| N-044 | limits | Rest aggregate exceeds collection-item budget | Limit error before allocation/host request. |
| N-045 | limits | Nested captured value exceeds nesting budget | Limit error before commit. |

## C. AWBC tamper and verifier cases

| ID | Layer | Tamper | Required result |
| --- | --- | --- | --- |
| T-001 | codec | Codec version 7 with ABI-2 payload | Unsupported codec before table decode. |
| T-002 | codec | ABI 1 with codec 8 | ABI mismatch. |
| T-003 | codec | Removed Source table bytes | Payload/table mismatch or unknown removed layout; never accepted. |
| T-004 | codec | Removed Source opcode `0x22` | Unknown instruction opcode. |
| T-005 | codec | Removed Source opcode `0x23` | Unknown instruction opcode. |
| T-006 | codec | Unknown runtime type tag 23 | Unknown runtime-type tag at exact offset. |
| T-007 | codec | Unknown constant tag 19 | Unknown constant tag at exact offset. |
| T-008 | codec | Unknown argument operand tag 5 | Unknown operand tag at exact offset. |
| T-009 | codec | Noncanonical vector length integer | Noncanonical integer error. |
| T-010 | codec | Coordinate vector count exceeds budget | Budget error before allocation. |
| T-011 | metadata | Group range out of bounds | Structural verifier error. |
| T-012 | metadata | Group owner mismatch | Structural verifier error. |
| T-013 | metadata | Parameter range overlaps another group | Structural verifier error. |
| T-014 | metadata | Group index reordered | Structural verifier error. |
| T-015 | metadata | Parameter coordinate reordered | Structural verifier error. |
| T-016 | metadata | Signature fingerprint byte changed | Fingerprint verifier error. |
| T-017 | instruction | Coordinate/value vector lengths differ | Instruction verifier error. |
| T-018 | instruction | Duplicate coordinate | Instruction verifier error. |
| T-019 | instruction | Missing optional coordinate | Instruction verifier error; optional must have an omission cell. |
| T-020 | instruction | Apply opcode used for final group | Instruction verifier error. |
| T-021 | instruction | Open opcode used for non-final group | Instruction verifier error. |
| T-022 | instruction | Callee static type has another definition | Instruction verifier error. |
| T-023 | instruction | Callee static type has another next group | Instruction verifier error. |
| T-024 | instruction | Destination partial next group wrong | Instruction verifier error. |
| T-025 | instruction | Open destination item/error type wrong | Instruction verifier error. |
| T-026 | instruction | Operand register type wrong | Instruction verifier error. |
| T-027 | instruction | Default digest changed | Instruction verifier error. |
| T-028 | instruction | Rest operand uses scalar register | Instruction verifier error. |
| T-029 | codec | Valid bytes plus trailing data | Trailing/payload-length error. |
| T-030 | parity | Decode then re-encode a tampered-but-repairable ordering | Decode rejects; it never normalizes to valid bytes. |

## D. Host JSON negative/tamper cases

| ID | Layer | Input | Required result |
| --- | --- | --- | --- |
| J-001 | all hosts | Duplicate top-level `generation` | Duplicate-field error. |
| J-002 | all hosts | Unknown top-level field | Unknown-field error. |
| J-003 | all hosts | Unknown `arguments.flat_arguments` | Unknown-field error. |
| J-004 | all hosts | Duplicate coordinate field | Duplicate-field error. |
| J-005 | all hosts | Unknown value `kind` | Unknown-tag error. |
| J-006 | all hosts | `omitted_optional` with `value: null` | Unknown-field error. |
| J-007 | all hosts | Uppercase/short/nonhex digest | Digest format error. |
| J-008 | all hosts | Numeric generation instead of string | Scalar-kind error. |
| J-009 | all hosts | Generation `"007"` | Noncanonical decimal error. |
| J-010 | all hosts | Generation `"+7"` or whitespace | Noncanonical decimal error. |
| J-011 | all hosts | Signed payload `"-0"` | Noncanonical decimal error. |
| J-012 | all hosts | Coordinate `65536` or negative | Bounds/type error. |
| J-013 | all hosts | Coordinate/value length mismatch | Product validation error before provider call. |
| J-014 | all hosts | Reordered coordinates | Product validation error before provider call. |
| J-015 | all hosts | Named-rest duplicate/out-of-order keys | Rest validation error before provider call. |
| J-016 | parity | Native accepts input rejected by Web/Agent | Test fails; acceptance sets must be identical. |

## E. Persistence and hot-reload failure cases

| ID | Layer | Scenario | Required result |
| --- | --- | --- | --- |
| S-001 | save | Save at non-safe-point structured group application | `ExternalStreamGroupApplicationActive` blocker. |
| S-002 | save | Partial capture is unsnapshotable | Typed unsnapshotable-capture blocker. |
| S-003 | save | Partial's generation artifact missing | Typed missing-generation blocker. |
| S-004 | save | Partial only, no external instance | No external-live Stream blocker. |
| S-005 | save | Final application already created Opening external instance | Parent external-live blocker applies. |
| S-006 | restore | Schema version 1 | Unsupported schema; no migration. |
| S-007 | restore | Artifact identity mismatch | Atomic restore failure. |
| S-008 | restore | AWBC ABI/codec mismatch | Atomic restore failure. |
| S-009 | restore | Wrong definition/declaration/signature | Atomic restore failure before value installation. |
| S-010 | restore | `next_group != completed_groups` | Atomic restore failure. |
| S-011 | restore | Corrupt coordinate/value product | Atomic restore failure. |
| S-012 | restore | Value digest mismatch | Atomic restore failure. |
| S-013 | restore | Duplicate affine token across registers/partial | Atomic restore failure. |
| S-014 | restore | Restore succeeds | No expression evaluation, instance allocation, provider call, or open request. |
| H-001 | hot reload | Group count changes | At least `CodeGenerational`; never code-compatible. |
| H-002 | hot reload | Group order/kind changes | At least `CodeGenerational`. |
| H-003 | hot reload | Parameter coordinate/name/order changes | At least `CodeGenerational`. |
| H-004 | hot reload | Passing/rest kind changes | At least `CodeGenerational`. |
| H-005 | hot reload | Presence/default digest changes | At least `CodeGenerational`. |
| H-006 | hot reload | Parameter/item/error type changes | At least `CodeGenerational`. |
| H-007 | hot reload | Provider ABI/adapter requirements change | `RestartRequired` under existing policy. |
| H-008 | hot reload | Active old partial receives new-generation plan | Typed signature/generation error before evaluation. |
| H-009 | hot reload | Old generation prematurely retired | Subsequent apply returns `StaleGeneration` atomically; retention test must prevent this in valid flow. |
| H-010 | hot reload | Attempt to translate old captures to new layout | No API exists; compile/type-level test proves absence of migration path. |

## F. Architecture and deletion evidence

| ID | Layer | Check | Required result |
| --- | --- | --- | --- |
| A-001 | Cargo metadata | `arcweft-core` dependencies | No syntax/HIR/sema/verifier/tooling/host/I/O dependency. |
| A-002 | Cargo metadata | `arcweft-runtime-plan` production dependencies | No sema dependency. |
| A-003 | Cargo metadata | compiler dependencies | Compiler can consume sema and emit core/RuntimePlan types. |
| A-004 | public API | Function-value owner | Closed `Closure`/`ExternalStreamPartial` enum; no public sidecar/extension trait. |
| A-005 | public API | Open request | Requires group-aware product; a final-group-only construction does not type-check or fails constructor validation. |
| A-006 | codec | Codec-8 decoder | Has no accepted Source/codec-7/flat request variant. |
| A-007 | behavior | Resolver invocation accounting | Accepted lowering does not perform a second callable lookup. |
| A-008 | structure audit | Changed owner files | Responsibilities, exact LOC/bytes, and dependency fan-in/out satisfy current AGENTS thresholds or have explicit rationale. |
| A-009 | broad validation | Workspace/Agent/runtime path | `fmt`, workspace check/clippy/tests, Tier 2, metadata, and structure audit all pass at final implementation commit. |

No acceptance row is satisfied by scanning source for a spelling. API shape,
behavior, canonical bytes, typed rejection, Cargo metadata, and structural metrics
are the evidence.
