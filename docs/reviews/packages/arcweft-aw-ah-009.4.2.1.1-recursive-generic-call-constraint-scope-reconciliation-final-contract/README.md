# AW-AH-009.4.2.1.1 generic application scope reconciliation

Date: 2026-09-07
Design status: READY_FOR_IMPLEMENTATION
Production status: NOT_IMPLEMENTED

This is a locally authored response to REQUEST.md, based on the existing dirty
main checkout at 69b30b530f5f1da39f5c8f0d7ee7f0f3de70bd29. It is a design
artifact, not evidence that the implementation passes. The request's recursion
regression still fails. An investigation probe also demonstrates that a
reusable prefix with a later generic parameter fails runtime projection.

Read FINAL_CONTRACT.md, RUST_SHAPES.md, PRODUCTION_RECONCILIATION.md and both
matrices together. They select one application binder, one scoped TypeKind
algebra, normalized frozen substitutions, quantified callable value types,
and one bounded closed-instance discovery transaction. No implementation may
select only the recursive map collision and leave future parameters ambiguous.

The ordinary ProjectCall input/output and same-fiber call/return protocol remain.
The scope correction also requires binder-aware runtime type admission and
Unapplied value-position validation described in sections 7 and 8. These
are explicitly justified amendments to the parent's closed-function-type
assumption; they are not an alternate project-call interpreter.

REPOSITORY_EVIDENCE.md distinguishes source inspection, actual command results,
and planned validation. SOURCE_INPUTS.tsv hashes the exact inspected source
inputs. VALIDATION_EVIDENCE.txt retains the actual failing compiler probe log.
No production patch or production test is included.

MANIFEST.txt is the sorted member list excluding itself. SHA256SUMS.txt hashes
all substantive members, excluding itself and MANIFEST.txt, avoiding circular
self hashes. Archive verification additionally checks the manifest and hash-list
membership and byte equality against the searchable extracted mirror.

Implementation remains a single uncommitted replacement through all affected
consumers. Design readiness does not authorize a partial production commit.
