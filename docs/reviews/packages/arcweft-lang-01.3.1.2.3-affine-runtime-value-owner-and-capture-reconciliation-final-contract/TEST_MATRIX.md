# Test matrix

New Lang-01.3.1.2.3 cases: **395**. The machine-identical rows are in `TEST_MATRIX.json` and `TEST_MATRIX.csv`. All 803 predecessor rows are retained in `PARENT_TEST_MATRIX_INDEX.json`.

## Coverage counts

| Dimension | Counts |
|---|---|
| Prefix | AWBC=53, BOUND=36, CAP=30, DROP=10, DUP=28, FULL=16, OPS=80, OWN=30, PLAN=20, SNAP=60, STREAM=20, XFER=12 |
| Kind | boundary=4, compile-fail=13, full=25, golden=2, negative=140, parity=11, positive=177, tamper=23 |
| Requirement | R1=16, R10=36, R2=63, R3=30, R4=58, R5=22, R6=53, R7=61, R8=36, R9=20 |

## Execution evidence classes

- `typed API / exact state`: assert typed result and complete before/after slot/table/execution digest.
- `canonical bytes`: assert exact codec bytes plus strict malformed-input rejection.
- `trybuild/compiler diagnostic`: prove removed/inaccessible Rust surfaces cannot compile.
- `Cargo metadata + structure audit`: prove layering/sole-owner/no-sidecar architecture without scanning implementation text as a behavioral gate.
- `raw command/test/metadata evidence`: implementation task must record actual commands/exits; this design artifact does not claim those runs.

## Cases

### OWN (30)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `OWN-001` | R1 | positive | unrestricted scalar | G1/P4+C1 | Unrestricted |
| `OWN-002` | R1 | positive | unrestricted aggregate | G1/P4+C1 | Unrestricted |
| `OWN-003` | R1 | positive | direct affine leaf | G1/P4+C1 | Affine |
| `OWN-004` | R1 | positive | nested affine tuple | G1/P4+C1 | Affine with exact tuple path |
| `OWN-005` | R1 | positive | nested affine record | G1/P4+C1 | Affine with exact record/sequence path |
| `OWN-006` | R1 | positive | affine closure | G1/P4+C1 | Affine |
| `OWN-007` | R1 | positive | affine partial | G1/P4+C1 | Affine |
| `OWN-008` | R1 | positive | omitted optional | G1/P4+C1 | Unrestricted |
| `OWN-009` | R1 | positive | rest join | G1/P4+C1 | Affine |
| `OWN-010` | R1 | negative | ownership cache mismatch | G1/P4+C1 | OwnershipInvariant at exact first path |
| `OWN-011` | R1 | tamper | duplicate owner graph | G1/P4+C1 | reject first duplicate owner occurrence |
| `OWN-012` | R1 | positive | deterministic path order | G1/P4+C1 | first error follows canonical path order on repeated runs |
| `OWN-013` | R1 | positive | owner ID evidence copy | G1/P4+C1 | no token/table/lease authority is created |
| `OWN-014` | R1 | boundary | owner ordinal exact limit | G1/P4+C1 | last ordinal succeeds exactly once; no authority is published before commit |
| `OWN-015` | R1 | negative | owner ordinal overflow | G1/P4+C1 | typed allocation overflow before state mutation |
| `OWN-016` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no public constructor or Clone |
| `OWN-017` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no struct literal or Clone |
| `OWN-018` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-019` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-020` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-021` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-022` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails because the existing RuntimeSeq owner has no Clone; no parallel sequence type exists |
| `OWN-023` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-024` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-025` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-026` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-027` | R2 | compile-fail | public trait boundary | G3/P4+C1 | Compilation fails at the removed/inaccessible API; no Clone |
| `OWN-028` | R2 | positive | closed payload clone | G1 | Clone succeeds because no runtime owner is reachable |
| `OWN-029` | R7 | positive | snapshot image clone | G3 | Both images are non-runnable and active owner occurrence remains one |
| `OWN-030` | R1 | full | no parallel owner authority | Final | One RuntimeValue model, one Stream table, no debug-string/Stream-only ownership sidecar |
### DUP (28)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `DUP-001` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Unit value; source unchanged |
| `DUP-002` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Bool value; source unchanged |
| `DUP-003` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Integer value; source unchanged |
| `DUP-004` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Decimal value; source unchanged |
| `DUP-005` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Text value; source unchanged |
| `DUP-006` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Bytes value; source unchanged |
| `DUP-007` | R2 | positive | unrestricted duplication | G1 | Independent equivalent EntityId value; source unchanged |
| `DUP-008` | R2 | positive | unrestricted duplication | G1 | Independent equivalent Record value; source unchanged |
| `DUP-009` | R2 | positive | recursive unrestricted duplication | G1/P4+C1 | Complete independent graph; source unchanged; no shared mutable/runtime authority |
| `DUP-010` | R2 | positive | recursive unrestricted duplication | G1/P4+C1 | Complete independent graph; source unchanged; no shared mutable/runtime authority |
| `DUP-011` | R2 | positive | recursive unrestricted duplication | G1/P4+C1 | Complete independent graph; source unchanged; no shared mutable/runtime authority |
| `DUP-012` | R2 | positive | recursive unrestricted duplication | G1/P4+C1 | Complete independent graph; source unchanged; no shared mutable/runtime authority |
| `DUP-013` | R2 | positive | recursive unrestricted duplication | G1/P4+C1 | Complete independent graph; source unchanged; no shared mutable/runtime authority |
| `DUP-014` | R2 | positive | recursive unrestricted duplication | G1/P4+C1 | Complete independent graph; source unchanged; no shared mutable/runtime authority |
| `DUP-015` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at root; source value, owner token, lease and table unchanged |
| `DUP-016` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at Tuple(1); source value, owner token, lease and table unchanged |
| `DUP-017` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at Record(resource); source value, owner token, lease and table unchanged |
| `DUP-018` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at Sequence(2); source value, owner token, lease and table unchanged |
| `DUP-019` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at VariantPayload; source value, owner token, lease and table unchanged |
| `DUP-020` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at ClosureCapture(1); source value, owner token, lease and table unchanged |
| `DUP-021` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at ExternalStreamArgument(1,0); source value, owner token, lease and table unchanged |
| `DUP-022` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at RestPositional(3); source value, owner token, lease and table unchanged |
| `DUP-023` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at RestNamed(token); source value, owner token, lease and table unchanged |
| `DUP-024` | R2 | negative | affine duplication rejection | G1/P4+C1 | AffineLeaf at IteratorRemaining(0); source value, owner token, lease and table unchanged |
| `DUP-025` | R2 | boundary | first-error determinism | G1 | Always reject canonical first RuntimeValuePath |
| `DUP-026` | R2 | negative | no lease rotation | P4+C1 | Reject without allocating/rotating consumer lease or touching table cursors |
| `DUP-027` | R2 | negative | no provider activity | P7+C5 | Reject with zero host requests/provider calls/scheduler observations |
| `DUP-028` | R2 | tamper | cache mismatch before copy | G1 | Reject invariant before constructing output |
### XFER (12)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `XFER-001` | R2 | positive | copy slot | G1/G3 | source Live; destination equivalent Live |
| `XFER-002` | R2 | positive | move slot | G1/G3 | source Moved; destination owns same owner ID/lease |
| `XFER-003` | R2 | negative | copy affine | G1/G3 | CopyOfAffine; slots/table unchanged |
| `XFER-004` | R2 | negative | source moved | G1/G3 | SourceNotLive; destination unchanged |
| `XFER-005` | R2 | negative | source dropped | G1/G3 | SourceNotLive; destination unchanged |
| `XFER-006` | R2 | negative | destination live | G1/G3 | DestinationNotEmpty; both live unchanged |
| `XFER-007` | R2 | negative | same slot | G1/G3 | reject before mutation |
| `XFER-008` | R2 | tamper | duplicate owner batch | G1/G3 | DuplicateOwner; no source is taken and every source revision remains unchanged |
| `XFER-009` | R2 | positive | mixed batch order | G1/G3 | destinations in plan order; only A1 source moved |
| `XFER-010` | R2 | negative | late staged copy failure | G1/G3 | staged earlier copy discarded; all source slots unchanged |
| `XFER-011` | R2 | negative | stale source revision | G1/G3 | StaleRevision; all other sources/destinations/table remain unchanged |
| `XFER-012` | R2 | positive | transfer cleanup obligation | G1/G3 | cleanup obligation follows destination exactly once |
### DROP (10)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `DROP-001` | R2 | positive | unrestricted scalar | G2/G3/P4+C1 | terminal slot and no domain mutation |
| `DROP-002` | R2 | positive | direct handle | G2/G3/P4+C1 | slot Dropped; matching table consumer transition once |
| `DROP-003` | R2 | positive | nested aggregate | G2/G3/P4+C1 | releases descending owner ordinal; each once |
| `DROP-004` | R2 | negative | stale lease | G2/G3/P4+C1 | prepare drop rejects; slot/table unchanged |
| `DROP-005` | R2 | tamper | duplicate owner nested | G2/G3/P4+C1 | prepare drop rejects duplicate before release |
| `DROP-006` | R2 | negative | active execution transfer guard | G2/G3/P4+C1 | TransferInProgress before token/table/slot mutation |
| `DROP-007` | R2 | negative | double drop | G2/G3/P4+C1 | typed use-after-drop; no table change |
| `DROP-008` | R2 | positive | unwind cleanup | G2/G3/P4+C1 | reverse registration/release order exactly once |
| `DROP-009` | R2 | positive | cancellation cleanup | G2/G3/P4+C1 | all remaining items dropped once before terminalization |
| `DROP-010` | R2 | full | no Rust language drop | G2/G3/P4+C1 | all successful paths use explicit prepared drop; no lease action in Rust Drop |
### CAP (30)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `CAP-001` | R3 | positive | exact free set | G1/G2/P4+C1 | captures exactly a,c |
| `CAP-002` | R3 | positive | parameter exclusion | G1/G2/P4+C1 | parameter is not capture |
| `CAP-003` | R3 | positive | first-use order | G1/G2/P4+C1 | capture slots c,a,b |
| `CAP-004` | R3 | positive | repeated use reuse | G1/G2/P4+C1 | one CaptureId/one slot |
| `CAP-005` | R3 | positive | nearest shadow | G1/G2/P4+C1 | uses nearest visible local only |
| `CAP-006` | R3 | positive | initializer pre-binding | G1/G2/P4+C1 | initializer use captures outer according to accepted pre-binding rule |
| `CAP-007` | R3 | positive | unrestricted mode | G1/G2/P4+C1 | Copy source; outer remains Live |
| `CAP-008` | R3 | positive | affine mode | G1/G2/P4+C1 | Move source; outer slot becomes Moved; closure affine |
| `CAP-009` | R3 | positive | mixed modes | G1/G2/P4+C1 | Copy/Move/Copy in capture ordinal order |
| `CAP-010` | R3 | positive | mutable capture slot | G1/G2/P4+C1 | closure-owned slot changes; outer binding unchanged |
| `CAP-011` | R3 | positive | nested unrestricted capture | G1/G2/P4+C1 | copy; outer capture remains live |
| `CAP-012` | R3 | positive | nested affine capture | G1/G2/P4+C1 | move; outer capture slot becomes moved |
| `CAP-013` | R3 | negative | outer use after nested move | G1/G2/P4+C1 | typed use-after-move |
| `CAP-014` | R3 | negative | plan identity mismatch | G1/G2/P4+C1 | PlanIdentity; env unchanged |
| `CAP-015` | R3 | negative | missing source | G1/G2/P4+C1 | SourceNotLive; env unchanged |
| `CAP-016` | R3 | negative | moved source | G1/G2/P4+C1 | SourceNotLive; env unchanged |
| `CAP-017` | R3 | negative | type mismatch | G1/G2/P4+C1 | TypeMismatch before copies/moves |
| `CAP-018` | R3 | negative | duplicate source | G1/G2/P4+C1 | DuplicateSource |
| `CAP-019` | R3 | negative | destination gap | G1/G2/P4+C1 | noncanonical destination rejection |
| `CAP-020` | R3 | negative | copy affine contradiction | G1/G2/P4+C1 | CopyOfAffine; env/table unchanged |
| `CAP-021` | R3 | negative | duplicate owner across sources | G1/G2/P4+C1 | ownership rejection before mutation |
| `CAP-022` | R3 | boundary | exact capture limit | G1/G2/P4+C1 | succeeds in exact order |
| `CAP-023` | R3 | negative | one-over capture limit | G1/G2/P4+C1 | LimitExceeded; no closure/env mutation |
| `CAP-024` | R3 | negative | staged copy failure atomicity | G1/G2/P4+C1 | discard staged copies; every source and env revision unchanged |
| `CAP-025` | R3 | negative | source revision recheck failure atomicity | G1/G2/P4+C1 | StaleRevision; staged copies are discarded and every source remains Live |
| `CAP-026` | R3 | positive | suspension after capture | G1/G2/P4+C1 | capture slots/owner IDs preserved; no environment recapture |
| `CAP-027` | R3 | negative | no whole environment fallback | G1/G2/P4+C1 | unrelated local stays outer/live and absent from closure |
| `CAP-028` | R3 | full | no source reconstruction | G1/G2/P4+C1 | same exact captures; no scan/name parsing |
| `CAP-029` | R3 | positive | closure unrestricted duplicate | G1/G2/P4+C1 | independent captures; source live |
| `CAP-030` | R3 | negative | closure affine duplicate | G1/G2/P4+C1 | reject at exact capture path |
### OPS (80)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `OPS-001` | R4 | positive | local Copy | G2 | both uses succeed; original retained until moved/dropped |
| `OPS-002` | R4 | positive | local Move | G2 | source moved; owner unchanged |
| `OPS-003` | R4 | negative | local use-after-move | G2 | typed error before dependent expression |
| `OPS-004` | R4 | positive | borrow local | G2 | owner stays live; borrow ends before suspension |
| `OPS-005` | R4 | negative | escaping borrow | G2 | rejected before executable plan |
| `OPS-006` | R4 | positive | let evaluation once | G2 | counter once; one owner in binding |
| `OPS-007` | R4 | positive | let x=x prebinding | G2 | reads outer then binds new slot |
| `OPS-008` | R4 | positive | tuple pattern move | G2 | each selected field moved once; source terminal |
| `OPS-009` | R4 | positive | record pattern order | G2 | binding/transfer order follows authored fields |
| `OPS-010` | R4 | positive | sequence pattern | G2 | elements move left-to-right; remainder explicitly handled |
| `OPS-011` | R4 | positive | variant match | G2 | payload moved only after arm selection |
| `OPS-012` | R4 | negative | borrowed duplicate pattern | G2 | copy rejection; no bindings |
| `OPS-013` | R4 | positive | owned rest | G2 | rest owns moved remaining members |
| `OPS-014` | R4 | positive | borrowed rest unrestricted | G2 | checked copies; source retained |
| `OPS-015` | R4 | negative | borrowed rest affine | G2 | reject before any binding |
| `OPS-016` | R4 | positive | ignored rest | G2 | no copy; source/owned plan behavior unchanged |
| `OPS-017` | R4 | positive | tuple construction | G2 | operands consumed in language order; one complete aggregate |
| `OPS-018` | R4 | positive | record construction | G2 | operands consumed in language order; one complete aggregate |
| `OPS-019` | R4 | positive | sequence construction | G2 | operands consumed in language order; one complete aggregate |
| `OPS-020` | R4 | positive | variant construction | G2 | operands consumed in language order; one complete aggregate |
| `OPS-021` | R4 | positive | positional rest construction | G2 | operands consumed in language order; one complete aggregate |
| `OPS-022` | R4 | positive | named rest construction | G2 | operands consumed in language order; one complete aggregate |
| `OPS-023` | R4 | negative | constructor late failure | G2 | ordinary prior effects remain; every temporary cleaned once; no aggregate |
| `OPS-024` | R4 | positive | non-consuming projection unrestricted | G2 | checked copy; record live |
| `OPS-025` | R4 | negative | non-consuming projection affine | G2 | reject; record/table unchanged |
| `OPS-026` | R4 | positive | consuming projection affine | G2 | handle moved out; remainder dropped; record terminal |
| `OPS-027` | R4 | tamper | consuming projection drop failure | G2 | reject; whole aggregate remains live |
| `OPS-028` | R4 | negative | variant payload non-consuming | G2 | copy rejection |
| `OPS-029` | R4 | positive | ordinary call consuming | G2 | callee/argument moved into frame; no clone |
| `OPS-030` | R4 | positive | ordinary call reuse | G2 | two calls; explicit copy visible in plan/AWBC |
| `OPS-031` | R4 | negative | call frame late validation failure | G2 | callee/args remain live; no frame |
| `OPS-032` | R4 | positive | return affine | G2 | owner moved into caller destination; callee slot moved |
| `OPS-033` | R4 | positive | return cleanup | G2 | result transferred then remaining locals dropped reverse registration |
| `OPS-034` | R4 | positive | assignment success | G2 | replacement installed and old handle dropped atomically |
| `OPS-035` | R4 | negative | assignment type failure | G2 | old slot unchanged; replacement returned/cleaned |
| `OPS-036` | R4 | tamper | assignment old-drop failure | G2 | old remains live; replacement remains owned; no revision |
| `OPS-037` | R4 | positive | cross-fiber Copy | G2 | parent retained; child receives duplicate |
| `OPS-038` | R4 | positive | cross-fiber Move | G2 | parent moved; child owns same token/lease |
| `OPS-039` | R4 | negative | cross-fiber mixed failure | G2 | no child/ID/scope/observation; parent unchanged |
| `OPS-040` | R4 | full | no ambient child env | G2 | child contains exactly 2 values |
| `OPS-041` | R4 | positive | iterator construction | G2 | sequence moved; iterator affine |
| `OPS-042` | R4 | positive | iterator next once | G2 | A,B,None,None; each owner returned once |
| `OPS-043` | R4 | positive | iterator drop remainder | G2 | B,C dropped exactly once |
| `OPS-044` | R4 | positive | iterator save order | G2 | returns B then C; no replayed A |
| `OPS-045` | R5 | positive | sequence repeat | G2 | empty result; source explicitly dropped |
| `OPS-046` | R5 | positive | sequence repeat | G2 | singleton owns original |
| `OPS-047` | R5 | positive | sequence repeat | G2 | empty; source dropped |
| `OPS-048` | R5 | positive | sequence repeat | G2 | singleton owns original |
| `OPS-049` | R5 | positive | sequence repeat | G2 | one copy then original |
| `OPS-050` | R5 | positive | sequence repeat | G2 | MAX values and bounded work |
| `OPS-051` | R5 | negative | sequence repeat | G2 | limit error; source returned unchanged |
| `OPS-052` | R5 | negative | sequence repeat | G2 | permission/count mismatch; source unchanged |
| `OPS-053` | R5 | negative | sequence repeat | G2 | rejected by sema/lowering before runtime |
| `OPS-054` | R5 | negative | sequence repeat | G2 | no partial sequence; source unchanged |
| `OPS-055` | R5 | positive | index unrestricted in bounds | G2 | copy b; source unchanged |
| `OPS-056` | R5 | negative | index affine in bounds | G2 | ownership error before bounds copy |
| `OPS-057` | R5 | negative | index affine out of bounds | G2 | ownership error precedes bounds |
| `OPS-058` | R5 | negative | index unrestricted out of bounds | G2 | bounds error; source unchanged |
| `OPS-059` | R5 | positive | slice unrestricted empty | G2 | empty unrestricted sequence |
| `OPS-060` | R5 | negative | slice affine empty | G2 | ownership error; empty is not exception |
| `OPS-061` | R5 | positive | slice unrestricted boundary | G2 | copies selected values ascending |
| `OPS-062` | R5 | negative | slice invalid range | G2 | range error before copies |
| `OPS-063` | R5 | positive | consuming take affine | G2 | selected owner moved; remainder dropped |
| `OPS-064` | R5 | negative | consuming take stale remainder | G2 | whole sequence retained |
| `OPS-065` | R5 | positive | push affine | G2 | element moved into sequence; source error wrapper empty |
| `OPS-066` | R5 | negative | push capacity | G2 | capacity error returns element; sequence unchanged |
| `OPS-067` | R4 | positive | equality unrestricted | G2 | true; no ownership changes |
| `OPS-068` | R4 | positive | equality affine Eq-capable future leaf | G2 | result with both owners live; no copy/move |
| `OPS-069` | R4 | negative | equality handle | G2 | NotComparable before owner details leak |
| `OPS-070` | R4 | negative | equality function | G2 | NotComparable |
| `OPS-071` | R4 | tamper | equality forged evidence | G2 | EvidenceMismatch; owners unchanged |
| `OPS-072` | R4 | positive | branch identical live | G2 | accepted |
| `OPS-073` | R4 | positive | branch identical moved | G2 | accepted terminal fact |
| `OPS-074` | R4 | negative | branch live vs moved used later | G2 | JoinStateMismatch |
| `OPS-075` | R4 | positive | branch live vs moved dead | G2 | accepted equal terminal facts |
| `OPS-076` | R4 | negative | loop affine missing reinit | G2 | reject loop header mismatch |
| `OPS-077` | R4 | positive | loop affine reinitialized | G2 | accepted exact Live fact |
| `OPS-078` | R4 | positive | cleanup normal exit | G2 | reverse registration, once |
| `OPS-079` | R4 | positive | cleanup return | G2 | return transfer then reverse cleanup |
| `OPS-080` | R4 | positive | cleanup trap | G2 | same owner releases/ordering as structured/AWBC |
### AWBC (53)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `AWBC-001` | R6 | golden | CopyValue small IDs | P6+C4 | bytes `2a 01 02` |
| `AWBC-002` | R6 | golden | CopyValue multibyte IDs | P6+C4 | canonical varu32 bytes |
| `AWBC-003` | R6 | negative | unknown next opcode | P6+C4 | unknown opcode |
| `AWBC-004` | R6 | negative | removed opcode | P6+C4 | unknown opcode |
| `AWBC-005` | R6 | negative | noncanonical dst varint | P6+C4 | hard codec error |
| `AWBC-006` | R6 | negative | truncated src | P6+C4 | truncation error |
| `AWBC-007` | R6 | negative | trailing bytes | P6+C4 | trailing-byte error |
| `AWBC-008` | R6 | positive | CopyValue verifier | P6+C4 | accepted facts |
| `AWBC-009` | R6 | negative | CopyValue affine static | P6+C4 | CopyRequiresUnrestricted |
| `AWBC-010` | R6 | tamper | CopyValue runtime mismatch | P6+C4 | trap before registers/table change |
| `AWBC-011` | R6 | negative | CopyValue live destination | P6+C4 | DestinationLive |
| `AWBC-012` | R6 | negative | CopyValue same register | P6+C4 | reject |
| `AWBC-013` | R6 | positive | Move unrestricted | P6+C4 | source Moved; dst Live Unrestricted |
| `AWBC-014` | R6 | positive | Move affine | P6+C4 | same owner ID/lease in dst; source Moved |
| `AWBC-015` | R6 | negative | Move moved source | P6+C4 | UseAfterMove |
| `AWBC-016` | R6 | negative | Move live destination | P6+C4 | trap/verifier error before take |
| `AWBC-017` | R6 | positive | Drop unrestricted | P6+C4 | Dropped fact; cleanup discharged |
| `AWBC-018` | R6 | positive | Drop affine | P6+C4 | table-aware release; Dropped fact |
| `AWBC-019` | R6 | negative | Drop stale lease runtime | P6+C4 | trap before slot/table mutation |
| `AWBC-020` | R6 | negative | Drop moved slot | P6+C4 | cleanup/use-after-move error |
| `AWBC-021` | R6 | positive | tuple constructor | G3/P6+C4 | both consumed; dst ownership Affine |
| `AWBC-022` | R6 | positive | record constructor | G3/P6+C4 | all consumed in field order |
| `AWBC-023` | R6 | positive | sequence constructor | G3/P6+C4 | all consumed; no internal clone |
| `AWBC-024` | R6 | positive | variant constructor | G3/P6+C4 | payload consumed |
| `AWBC-025` | R6 | positive | MakeFunction mixed capture | G3/P6+C4 | U retained, A moved, closure affine |
| `AWBC-026` | R6 | positive | ordinary ApplyFunction | G3/P6+C4 | all consumed into frame |
| `AWBC-027` | R6 | positive | return | G3/P6+C4 | result moved to caller then cleanup |
| `AWBC-028` | R6 | positive | equality instruction | G3/P6+C4 | borrow; both remain live |
| `AWBC-029` | R6 | positive | branch condition | G3/P6+C4 | borrow/typed use; no owner transfer |
| `AWBC-030` | R6 | positive | ApplyExternalStreamGroup operands | P6+C4 | all consumed; dst partial |
| `AWBC-031` | R6 | positive | Apply omitted optional | P6+C4 | no register/owner use |
| `AWBC-032` | R6 | negative | Apply late failure | P6+C4 | all registers remain live; dst empty |
| `AWBC-033` | R6 | positive | OpenStream operands | P6+C4 | consumed; dst unique handle/table/request |
| `AWBC-034` | R6 | negative | Open payload rejection | P6+C4 | trap before consume/token/table/request |
| `AWBC-035` | R6 | negative | Open destination live | P6+C4 | reject before any source take |
| `AWBC-036` | R6 | positive | FinishStream fail outcome | P6+C4 | typed consuming terminal transition |
| `AWBC-037` | R6 | positive | join identical facts | G3/P6+C4 | join accepted |
| `AWBC-038` | R6 | negative | join live/moved | G3/P6+C4 | JoinStateMismatch |
| `AWBC-039` | R6 | positive | join explicit drop normalization | G3/P6+C4 | terminal facts equal |
| `AWBC-040` | R6 | negative | loop back-edge mismatch | G3/P6+C4 | verifier rejection |
| `AWBC-041` | R6 | positive | cleanup move transfer | G3/P6+C4 | obligation follows destination |
| `AWBC-042` | R6 | negative | duplicate cleanup | G3/P6+C4 | DuplicateCleanup |
| `AWBC-043` | R6 | negative | missing affine cleanup | G3/P6+C4 | LiveCleanupAtFrameExit |
| `AWBC-044` | R6 | negative | cancel cleanup leak | G3/P6+C4 | verifier rejection |
| `AWBC-045` | R6 | positive | safe point owned slots | G3/P6+C4 | safe point accepted |
| `AWBC-046` | R6 | negative | safe point active transfer | G3/P6+C4 | UnsafePointDuringOwnershipTransaction |
| `AWBC-047` | R6 | negative | safe point borrow | G3/P6+C4 | safe point rejected |
| `AWBC-048` | R6 | positive | spawn mixed capture | G3/P6+C4 | atomic child and parent transitions |
| `AWBC-049` | R6 | negative | spawn late limit failure | G3/P6+C4 | parent unchanged; no child/scope/observation |
| `AWBC-050` | R6 | parity | interpreter vs compiled copy/move | G3/P6+C4 | identical result/register/owner/table/cleanup digest |
| `AWBC-051` | R6 | parity | interpreter vs compiled trap | G3/P6+C4 | identical unchanged pre-state and error |
| `AWBC-052` | R6 | tamper | stale compiled exchange | G3/P6+C4 | reject whole exchange; core state unchanged |
| `AWBC-053` | R6 | full | one register model | G3/P6+C4 | metadata/structure audit finds no Stream register sidecar |
### SNAP (60)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `SNAP-001` | R7 | positive | begin global snapshot | G3/P8+C6 | state SnapshotFrozen while guard lives |
| `SNAP-002` | R7 | negative | snapshot at local-only point | G3/P8+C6 | typed blocker; execution remains runnable |
| `SNAP-003` | R7 | positive | snapshot image | G3/P8+C6 | image contains data/evidence, no token |
| `SNAP-004` | R7 | positive | resume original | G3/P8+C6 | original runs with same owner/table state |
| `SNAP-005` | R7 | positive | copy image | G3/P8+C6 | no new active owner/table row |
| `SNAP-006` | R7 | negative | candidate execution attempt | G3/P8+C6 | no public step/value/token API |
| `SNAP-007` | R7 | positive | restore into empty | G3/P8+C6 | one complete active execution |
| `SNAP-008` | R7 | positive | restore replace | G3/P8+C6 | old owners retired then candidate active in one revision |
| `SNAP-009` | R7 | negative | alongside install compile fail | G3/P8+C6 | no API/compile failure |
| `SNAP-010` | R7 | tamper | duplicate owner ID | G3/P8+C6 | reject at duplicate-owner step |
| `SNAP-011` | R7 | tamper | duplicate key/lease handle | G3/P8+C6 | reject after owner check in fixed order |
| `SNAP-012` | R7 | tamper | orphan handle | G3/P8+C6 | reciprocity rejection |
| `SNAP-013` | R7 | tamper | orphan table consumer | G3/P8+C6 | reciprocity rejection |
| `SNAP-014` | R7 | tamper | stale lease | G3/P8+C6 | reciprocity rejection; no rotation |
| `SNAP-015` | R7 | tamper | layout mismatch | G3/P8+C6 | hard rejection |
| `SNAP-016` | R7 | tamper | ownership cache mismatch | G3/P8+C6 | ownership recomputation rejection before reciprocity |
| `SNAP-017` | R7 | tamper | affine in RuntimePayload field | G3/P8+C6 | payload exclusion rejection |
| `SNAP-018` | R7 | tamper | missing generation pin | G3/P8+C6 | exact pin mismatch |
| `SNAP-019` | R7 | tamper | extra generation pin | G3/P8+C6 | exact pin mismatch |
| `SNAP-020` | R7 | negative | missing artifact | G3/P8+C6 | Missing generation/artifact before activation |
| `SNAP-021` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-022` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-023` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-024` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-025` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-026` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-027` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-028` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-029` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-030` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-031` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-032` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-033` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-034` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-035` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-036` | R7 | positive | generation pin traversal | P8+C6 | Exact generation appears once in sorted required set |
| `SNAP-037` | R7 | negative | envelope checksum failure | P8+C6 | old execution unchanged/runnable |
| `SNAP-038` | R7 | negative | version/identity failure | P8+C6 | old execution unchanged |
| `SNAP-039` | R7 | negative | limit failure | P8+C6 | candidate discarded; old unchanged |
| `SNAP-040` | R7 | negative | unknown tag | P8+C6 | candidate discarded |
| `SNAP-041` | R7 | negative | register fact failure | P8+C6 | reject before table activation |
| `SNAP-042` | R7 | negative | child/scope failure | P8+C6 | reject before table activation |
| `SNAP-043` | R7 | negative | table accounting failure | P8+C6 | reject before reciprocity/activation |
| `SNAP-044` | R7 | negative | allocation-plan failure | P8+C6 | old execution unchanged |
| `SNAP-045` | R7 | negative | epoch recheck failure | P8+C6 | candidate discarded; active state preserved |
| `SNAP-046` | R7 | negative | artifact recheck failure | P8+C6 | candidate discarded; active state preserved |
| `SNAP-047` | R7 | negative | no authored evaluation | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-048` | R7 | negative | no default evaluation | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-049` | R7 | negative | no Stream Open | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-050` | R7 | negative | no provider work | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-051` | R7 | negative | no host dispatch | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-052` | R7 | negative | no replay injection | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-053` | R7 | negative | no scheduler step | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-054` | R7 | negative | no lease rotation | P8+C6 | Counter remains zero/state unchanged except final restore revision |
| `SNAP-055` | R7 | tamper | tamper rejection ordering | P8+C6 | checksum error always first |
| `SNAP-056` | R7 | parity | canonical deterministic error | P8+C6 | same typed first error/path |
| `SNAP-057` | R7 | positive | hot reload exact generation | P8+C6 | old generation retained; owner/instance unchanged |
| `SNAP-058` | R7 | negative | hot reload missing generation | P8+C6 | reject before active mutation |
| `SNAP-059` | R7 | negative | no schema1 migration | P8+C6 | strict version rejection; no dual reader |
| `SNAP-060` | R7 | parity | save round trip parity | P8+C6 | identical value/owner/table/request/trace outcome |
### BOUND (36)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `BOUND-001` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-002` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-003` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-004` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-005` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-006` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-007` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-008` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-009` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-010` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-011` | R8 | positive | general payload eligible | G1.3/P7+C5 | Success; canonical payload round trip |
| `BOUND-012` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (Function); consuming error returns original value |
| `BOUND-013` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (Function); consuming error returns original value |
| `BOUND-014` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (Function/Affine); consuming error returns original value |
| `BOUND-015` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (StreamHandle); consuming error returns original value |
| `BOUND-016` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (Iterator); consuming error returns original value |
| `BOUND-017` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (Reference); consuming error returns original value |
| `BOUND-018` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (Continuation); consuming error returns original value |
| `BOUND-019` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (RuntimeTable); consuming error returns original value |
| `BOUND-020` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (OpaqueRuntimeValue); consuming error returns original value |
| `BOUND-021` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (nested StreamHandle); consuming error returns original value |
| `BOUND-022` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (nested Function); consuming error returns original value |
| `BOUND-023` | R8 | negative | general payload rejection | G1.3/P7+C5 | Typed ineligible error at exact path (nested Iterator); consuming error returns original value |
| `BOUND-024` | R8 | positive | owned payload failure | G1.3 | Original owner/value returned; table unchanged |
| `BOUND-025` | R8 | negative | final Open payload preflight | C3/P7+C5 | reject before instance/token/lease/request/destination |
| `BOUND-026` | R8 | positive | non-final affine partial local | C3 | partial exists locally and is Affine; not payload/replay |
| `BOUND-027` | R8 | parity | host adapter parity | P7+C5 | Native bytes equal canonical core bytes; no endpoint owner DTO |
| `BOUND-028` | R8 | parity | host adapter parity | P7+C5 | Web bytes equal canonical core bytes; no endpoint owner DTO |
| `BOUND-029` | R8 | parity | host adapter parity | P7+C5 | Agent bytes equal canonical core bytes; no endpoint owner DTO |
| `BOUND-030` | R8 | parity | host adapter parity | P7+C5 | Headless bytes equal canonical core bytes; no endpoint owner DTO |
| `BOUND-031` | R8 | negative | host event payload | P7+C5 | closed payload codec rejects |
| `BOUND-032` | R8 | full | replay no live owner | P7+C5 | only IDs/digests/payload/lifecycle facts; no token/handle/partial |
| `BOUND-033` | R8 | tamper | replay cannot mint lease | P7+C5 | cannot decode/install handle; typed protocol rejection |
| `BOUND-034` | R8 | full | bundle no live owner | P6/P8 | no RuntimeAffineOwnerToken/StreamHandle/live partial field |
| `BOUND-035` | R8 | positive | save is owning boundary | P8+C6 | strict dormant evidence accepted unlike general payload |
| `BOUND-036` | R8 | negative | canonical value old opaque tag | Final | unknown tag/version; no shim |
### PLAN (20)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `PLAN-001` | R9 | positive | scalar constant | G1.3/G2/P4+C1 | ID stored; fresh equivalent value |
| `PLAN-002` | R9 | positive | aggregate constant | G1.3/G2/P4+C1 | recursive fresh value |
| `PLAN-003` | R9 | negative | affine constant | G1.3/G2/P4+C1 | reject ownership; original returned |
| `PLAN-004` | R9 | negative | closure constant | G1.3/G2/P4+C1 | PlanConstantIneligible; original returned |
| `PLAN-005` | R9 | negative | partial constant | G1.3/G2/P4+C1 | PlanConstantIneligible |
| `PLAN-006` | R9 | negative | iterator constant | G1.3/G2/P4+C1 | PlanConstantIneligible |
| `PLAN-007` | R9 | negative | reference constant | G1.3/G2/P4+C1 | PlanConstantIneligible |
| `PLAN-008` | R9 | boundary | constant count exact limit | G1.3/G2/P4+C1 | success canonical IDs |
| `PLAN-009` | R9 | negative | constant one-over | G1.3/G2/P4+C1 | limit error; table builder unchanged |
| `PLAN-010` | R9 | positive | Arc plan clone | G1.3/G2/P4+C1 | only Arc<RuntimePlan>, IDs/digests, and immutable artifacts are shared; FlowOp has no Bind/*Next/ForNext; no pending FlowOp clone exists; the sole live iterator stays in FlowControlStackEntryKind::For and bindings commit directly to RuntimeEnv; await/host/child-join states retain owner cursors and never cloned plan payloads |
| `PLAN-011` | R9 | positive | instantiate twice | G1.3/G2/P4+C1 | two independent executable values are constructed by cloning closed constant data and consuming each copy; no executable graph Clone is invoked |
| `PLAN-012` | R9 | tamper | table ownership cache mismatch | G1.3/G2/P4+C1 | reject before plan publication |
| `PLAN-013` | R9 | positive | AOT scalar immediate | G1.3/G2/P4+C1 | same as instantiate |
| `PLAN-014` | R9 | parity | JIT/AOT/interpreter constants | G1.3/G2/P4+C1 | identical values/digests and no owner state |
| `PLAN-015` | R9 | negative | stale constant ID | G1.3/G2/P4+C1 | typed ID error |
| `PLAN-016` | R9 | positive | constant digest deterministic | G1.3/G2/P4+C1 | same canonical digest |
| `PLAN-017` | R9 | negative | constant digest tamper | G1.3/G2/P4+C1 | hard rejection |
| `PLAN-018` | R9 | compile-fail | removed live expression and pattern literals | G1.3/G2/P4+C1 | both live-value variants, direct RuntimePlan Clone, all five runtime-only FlowOp variants, and pending cloned-op storage are unreachable |
| `PLAN-019` | R9 | full | borrowed pattern literal constant | G1.3/G2/P4+C1 | all paths borrow the checked constant and agree; no RuntimeValue literal is materialized or cloned |
| `PLAN-020` | R9 | positive | fixture authority | G1.3/G2/P4+C1 | one real handle/table relation; no raw token |
### STREAM (20)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `STREAM-001` | R10 | positive | P4+C1 mint | P4+C1..P8+C6 | one token/lease/key/handle/table/request commit |
| `STREAM-002` | R10 | negative | pre-G3 constructibility | P4+C1..P8+C6 | no production constructor/path |
| `STREAM-003` | R10 | positive | handle move | P4+C1..P8+C6 | same owner ID/key/lease; one occurrence |
| `STREAM-004` | R10 | negative | handle use after move | P4+C1..P8+C6 | typed error; table untouched |
| `STREAM-005` | R10 | positive | handle drop | P4+C1..P8+C6 | parent lifecycle/drop-retention behavior once |
| `STREAM-006` | R10 | positive | partial unrestricted duplicate | P4+C1..P8+C6 | checked duplicate succeeds |
| `STREAM-007` | R10 | negative | partial affine duplicate | P4+C1..P8+C6 | reject exact canonical coordinate path |
| `STREAM-008` | R10 | positive | non-final group consume | P4+C1..P8+C6 | one new partial; old owners consumed |
| `STREAM-009` | R10 | negative | non-final group failure ownership | P4+C1..P8+C6 | owned failure returns callee/evaluated group |
| `STREAM-010` | R10 | positive | final Open consume | P4+C1..P8+C6 | one handle/table/request and consumed inputs |
| `STREAM-011` | R10 | negative | final Open payload owner | P4+C1..P8+C6 | reject before allocation/publication |
| `STREAM-012` | R10 | tamper | token-table reciprocity | P4+C1..P8+C6 | runtime/snapshot validation rejects |
| `STREAM-013` | R10 | positive | save partial exact generation | P4+C1..P8+C6 | save pin includes exact generation |
| `STREAM-014` | R10 | positive | save handle exact generation | P4+C1..P8+C6 | save pin/table evidence exact |
| `STREAM-015` | R10 | negative | restore duplicate handle | P4+C1..P8+C6 | atomic tamper rejection |
| `STREAM-016` | R10 | parity | structured/AWBC/compiled partial | P4+C1..P8+C6 | same partial product/ownership/result |
| `STREAM-017` | R10 | parity | structured/AWBC/compiled Open | P4+C1..P8+C6 | same request bytes/instance sequencing/owner facts |
| `STREAM-018` | R10 | positive | CopyValue reuse before group | P4+C1..P8+C6 | source copy remains, operand register consumed |
| `STREAM-019` | R10 | negative | no implicit group clone | P4+C1..P8+C6 | verifier/use-after-move rejection |
| `STREAM-020` | R10 | full | all parent rows retained | P4+C1..P8+C6 | 803 predecessor cases preserved with source identity |
### FULL (16)

| ID | Req | Kind | Area | Stage | Expected |
|---|---|---|---|---|---|
| `FULL-001` | R10 | full | cargo fmt | Final | zero exit |
| `FULL-002` | R10 | full | workspace check | Final | zero exit |
| `FULL-003` | R10 | full | strict clippy | Final | zero exit; no allow workaround |
| `FULL-004` | R10 | full | workspace tests | Final | all green |
| `FULL-005` | R10 | full | Tier 2 | Final | all affected Tier 2 green |
| `FULL-006` | R10 | full | Cargo metadata | Final | layer direction/no cycles/feature reachability exact |
| `FULL-007` | R10 | full | structure audit | Final | all review triggers dispositioned; no typed blocking violation |
| `FULL-008` | R10 | full | public API compile-fail | Final | all removed Clone/constructors/old variant unreachable |
| `FULL-009` | R10 | full | codec golden | Final | 0x2a exact; unknown/removed strict |
| `FULL-010` | R10 | full | state parity | Final | identical outcomes/owners/cleanup/traces |
| `FULL-011` | R10 | full | snapshot tamper | Final | all fixed-order and atomicity cases green |
| `FULL-012` | R10 | full | parent P1 matrix | Final | all green |
| `FULL-013` | R10 | full | parent P2 matrix | Final | all green |
| `FULL-014` | R10 | full | parent P3 matrix | Final | all green |
| `FULL-015` | R10 | full | no forbidden architecture | Final | no side table/second env/register/value/DTO/shim/source gate/CSS/Takumi |
| `FULL-016` | R10 | full | implementation evidence | Final | no unexecuted pass claim |
