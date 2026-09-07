# Acceptance matrix

Status of every unmarked row: REQUIRED, NOT RUN for the selected new model.
Existing passing controls and observed failures are listed separately in
REPOSITORY_EVIDENCE.md. Passing a lower unit test does not close a runtime row.

| ID | Named test / fixture | Required observable result |
|---|---|---|
| A01 | recursive_generic_call_keeps_its_enclosing_type_rigid | Existing sema reproducer succeeds; inferred callee T is exactly caller Free(T) |
| A02 | generic_result_constructor_retains_enclosing_parameter_identity | Existing Ok(value) control remains accepted |
| A03 | generic_constructor_cannot_bind_an_enclosing_parameter_to_another_type | Existing incompatible Result<String,String> control rejects |
| A04 | generic_closure_call_reuses_its_rigid_parameter | Existing closure uses the same Free(T), with no new candidate binder |
| A05 | generic_mutual_recursion_closes_two_declarations | Two typed declarations calling each other preserve each lexical owner; native/AWBC bounded recursion returns identical result |
| A06 | recursive_call_in_closure_and_attached_default | Exact closure/default partition and captured Free scope; default omission executes once and supplied content skips it |
| A07 | explicit_recursive_type_arguments_do_not_rebind_caller | Explicit same and distinct callee types obey the callee schema; no assignment to caller Free(T) |
| A08 | nested_generic_shapes_share_one_scope_rule | Nominal, receiver, Result/Option/Need, tuple, array and nested function occurrences close through the same fold |
| A09 | scoped_const_recursion_and_future_const | Caller N and callee inference N differ; rigid length mismatch rejects; future length survives prefix and closes later |
| A10 | scoped_effect_recursion_and_future_effect | Bound future effect row survives prefix; closed terminal row and suspension/control evidence agree |
| A11 | inference_cycles_are_distinct_from_recursive_identity | x -> x and x -> F(x) reject; declaration T -> Free(T) seals; transitive A -> B -> i64 normalizes |
| A12 | mixed_caller_and_future_parameter_in_one_term | A recursive prefix containing caller B and residual callee B retains Free(B) and Bound(0,slot) in one compound type |
| A13 | one_curried_prefix_supports_distinct_later_generic_arguments | choose<A,B>(A)(B)->B creates one prefix; String and i64 calls create two closed instances and return correct values |
| A14 | early_solved_future_parameter_is_inherited | Producer context/explicit argument closes future B; subsequent group retains that value and conflicting use rejects |
| A15 | result_only_future_parameter_is_reified | Result-only unknown remains bound before terminal completion; missing terminal inference rejects |
| A16 | three_group_prefix_preserves_residual_bindings | Prepared and Frozen second-group paths agree; aliases/closures reuse prefix without mutation |
| A17 | generic_unapplied_function_value_retains_group_zero | Bare generic function aliases retain checked Unapplied provenance and empty prefix; first real call consumes group zero |
| A18 | function_scheme_comparison_is_predicative | Alpha-equivalent schemes compare equal; published scheme to monomorphic arrow is a typed mismatch; producer context can instead close it before freezing |
| A19 | parametric_expected_shape_does_not_leak_to_child_solver | Nested candidate receives no outer active inference variable; concrete projected child expectation still constrains nested result |
| A20 | choice_probe_rollback_and_materialization_replay | Different candidate branches allocate different issuers yet produce equal reified evidence; a failing branch publishes no facts |
| A21 | allocation_history_does_not_change_solution_or_instance_keys | Equivalent accepted source built with perturbed HIR allocation/interner history yields equal normalized solutions, digests and sorted instance inventory |
| A22 | finite_polymorphic_recursive_graph_reaches_fixed_point | Finite switch from caller type to a concrete type produces finite canonical nodes; repeated visits do not append substitution layers |
| A23 | growing_specialization_fails_at_inclusive_limit | Growing F<T> -> F<Option<T>> stops at configured structural/depth/work bound with exact typed fields, no stack overflow or partial catalog |
| A24 | instance_limit_edges_and_zero_boundaries | Empty graph with zero bound succeeds; first disallowed row fails; exactly-at-limit and limit+1 exercised for all five counters |
| A25 | prior_generation_survives_failed_specialization | Accepted compile remains usable after a later cancelled/over-limit compile fails |
| A26 | continuation_scheme_codec_and_snapshot_roundtrip | Native value and AWBC snapshot restore exact scheme/lineage/prefix under pinned generation; forged bound depth/type/generation rejects |
| A27 | concrete_current_group_prefix_cannot_capture_future_hole | Uninferred runtime prefix value rejects before lowering; no Unit/wildcard/opaque substitution |
| A28 | parent_project_call_regressions | Named/rest ABI order, source-once evaluation, attached defaults, Agent, Content/Fx, dialogue, unwind and suspending calls retain behavior |
| A29 | closed_instance_projection_uses_exact_nested_partitions | Function, closure and default locals/types/effects come from exact closed owners; no global fact fallback |
| A30 | production_authority_privacy | External/raw scope/solution/frozen constructors and serialization fail to compile; real gate fixtures succeed |

## Observed compiler reproducer

The following complete source was actually compiled during investigation:

~~~arcw
fn choose<A, B>(first: A)(second: B) -> B { second }

flow main() -> i64 {
    let prefix = choose(1i64)
    let text = prefix("text")
    return prefix(2i64)
}
~~~

Current result: semantic analysis succeeds, runtime-plan projection rejects
the open declaration-owned B because it has no closed runtime representation.
The selected result is one quantified prefix and two ordinary closed terminal
instances. It is not acceptable to duplicate the prefix computation, adopt
the first terminal type, reject the second terminal use, or issue a generic
FunctionSite.

## Semantic mixed-scope witness

Use a types-owned scoped fixture for
Tuple(Free(caller B), Bound(0, future callee B)), nested under the remaining
function binder. Also exercise a source recursive curried function which swaps
A/B arguments on a bounded recursive branch and swaps its pair result back.
The two declaration IDs can coincide with current schema IDs while the
reference cases remain distinct. Assert the semantic references and returned
runtime values, not a debug string.

## Validation tiers after implementation

Follow docs/implementation/test-execution-policy.md for focused tests,
workspace compilation and broader tiers. Required final gates include
workspace all-target/all-feature check and Clippy, format check, canonical
structured audit, relevant compiler/native/AWBC/codec/snapshot tests and the
full affected parent matrices. Run the repository-wide tier before claiming
the parent migration complete. Record failed, blocked and not-run commands
separately. No explicit Cargo job count is used for ordinary commands.
