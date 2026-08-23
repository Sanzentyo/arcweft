# State machine and failure atomicity


| State | Owner | Observable outside checker? | Allowed operation | Exit |
|---|---|---:|---|---|
| `Allocated` | checked match certifier | No | create builder with owner/universe/expected count | `Collecting` |
| `Collecting` | checked match certifier | No | record each source arm exactly once; poison on recovery | `Collected` or `Poisoned` |
| `Collected` | coverage engine | No | normalize/order atoms, compute closure and redundancy | `Closed` or `Open` |
| `Open` | diagnostic owner | No | emit structured gaps/redundancy; stop lowering | terminal error |
| `Poisoned` | diagnostic owner | No | preserve prior diagnostics; stop lowering | terminal error |
| `Closed` | transcript sealer | No | encode canonical body, compute digest, self-verify | `Complete` |
| `Complete` | checked catalog | Yes, immutable | intern by digest and issue `CompleteMatchTranscriptId` | `Admitted` |
| `Admitted` | HIR/runtime plan | Yes | lower/build/persist using capability-typed reference | runtime/persistence |
| `RestoreCandidate` | restore coordinator | No | decode, validate references/digests/closure | `Admitted` or reject |


No public transition exists from `Open`, `Poisoned`, or `RestoreCandidate` to HIR/runtime observability. Only the checked sealer and restore verifier may create `Complete`.
