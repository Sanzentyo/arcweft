# Proof concurrency v6.1.1 — Stage 1 remaining-row audit

Date: 2026-07-20

## Outcome

Proof-concurrency v6.1.1 Stage 0 is complete and most independently settled
Stage 1 private grammar rows are implemented. Stage 1 as a gate is not
complete, and Stage 2 must not start against a knowingly provisional item or
expression inventory.

No additional positive Stage 1 grammar row is independently
implementation-ready at this revision. The configured-resource row with a
settled final private grammar, typed `res`, was completed by Lang 01.4 Cut 1a.
That result must not be confused with the retained global-identity
declarations. `asset`, `character`, `view`, `action`, abstract `activity`,
`signal`, `metric`, and `layer` still exist in the public
`EntityDeclItem`/`EntityDeclKind` path, but the private grammar has no typed
declaration row for them. Lang 01.4 explicitly excludes assets and unrelated
identity-bearing declarations from `res`.

The remaining obsolete declarations cannot be made more structured without
preserving a source form whose final owner is elsewhere or creating a
forbidden dual reader. The retained global-identity declarations have the
opposite problem: they require a final typed private grammar without being
collapsed into `res`. That missing contract is now isolated as
[Proof 01.1.1.2 retained global-identity declaration grammar reconciliation](../reviews/requests/2026-07-20-seq-proof-01.1.1.2-retained-global-identity-declaration-grammar-reconciliation.md).

This is an evidence audit only.  It changes no parser, CST, public AST, HIR,
or runtime contract.

## Row matrix

| Stage 1 row | Current production state | Final-direction evidence | Decision |
| --- | --- | --- | --- |
| `extern mod` | `item.rs` recognizes `ExternModuleItem`; `document.rs` deliberately leaves it a generic logical-line wrapper. | Lang 01.5 moves concrete adapter/module binding to generated metadata and the build profile.  Lang 01.5.1 records that old launch/profile readers are still live; adding the final manifest decoder first would create a dual reader, while deleting them first would lose runtime inputs. | Do not add a private `ExternModuleItem` shadow node.  Implement only as part of the atomic single-manifest decoder and consumer migration. |
| `dialogue defaults`, source `content`, and concrete Activity origin | `item.rs` recognizes `DialogueDefaultsItem`; source `content` and Activity declarations remain in the old public entity path and have no typed private descendant. | Lang 01.5 assigns dialogue selection, content units, and concrete Activity implementation binding to the typed manifest/profile while retaining ordinary root entities and the abstract Activity interface. The current Lang 01.5.1 note records that the one decoder/consumer migration remains open. | Do not add private nodes for the forms that are being removed. Delete them only with the atomic metadata/profile migration. This does not authorize deletion of the abstract Activity identity declaration. |
| live `source` | `item.rs` recognizes `SourceItem`; the accepted private document grammar does not model it as a typed descendant. | Lang 01.3.1 fixes the destination as ordinary `fn -> Stream<T, E>` and removes both the keyword and `Source<T, E>`.  Lang 01.3.1.2.1 records unresolved mutually inconsistent callable, instance, replay, policy, save/AWBC, and adapter-wire shapes. | Do not add a private `SourceItem` shadow node.  It would commit an obsolete surface just before its removal.  Wait for the corrected typed Stream runtime/wire contract. |
| configured resource families | Lang 01.4 Cut 1a already provides the sole private `res` grammar: typed nominal head, explicit ref, fields, diagnostics, and recovery. | Lang 01.4 replaces only `image`, `voice`, `voice profile`, `se`, `bgm`, `audio bus`, `mixer snapshot`, `ducking`, `motion`, and `rig`. Its public AST/HIR/registry/runtime migration remains open. | The private `res` Stage 1 row is complete. Do not add parallel shadow nodes for those configured-resource family keywords. |
| retained global identity declarations | The public AST still accepts `EntityDeclKind::{Asset, Character, View, Action, Activity, Signal, Metric, Layer}`. The private `item.rs` has no declaration classifier for those heads, so they fall through the generic top-level path rather than owning typed declaration descendants. | The top-level reduction keeps dedicated declarations when they own stable global identity, an execution/host boundary, or a dedicated body. Lang 01.4 says `res` is not an owner for assets, actions, or other unrelated identity-bearing declarations. | Stage 1 is not complete for these rows. Do not force them into `res` and do not freeze the current stringly `EntityDeclItem` as the final grammar. A final typed grammar/ownership reconciliation is still required. |
| regular-project top-level statements | The private inventory still exposes `TopLevelFlowItem`, and the public AST still exposes `Item::FlowItem`. | The top-level reduction requires ordinary project source to contain declarations only; REPL/script execution is a separate dialect/owner. | This is deletion work, not a new grammar row. Remove it end to end with fixture migration and ordinary current-grammar recovery; do not retain a removed-spelling diagnostic or compatibility dialect in the project parser. |

## Why no Stage 2 implementation can start independently

Stage 2 attaches public identity to the accepted grammar.  It cannot safely
start while obsolete forms still enter the document as legacy generic wrappers
and retained global-identity declarations lack typed private descendants.
Assigning identities to the obsolete forms would make provisional surfaces
part of the public identity contract. Assigning only generic wrapper identity
to declarations that remain would preserve the current stringly boundary.
Both outcomes conflict with the final-contract policy.

The independently actionable work is therefore contract completion, not a
Rust implementation cut:

1. [Lang 01.5.1 single-manifest decoder production reconciliation](../reviews/requests/2026-07-17-lang-01.5.1-single-manifest-decoder-production-reconciliation.md)
   must choose and implement the single reader/consumer migration for extern
   bindings and dialogue/profile metadata.
2. [Lang 01.3.1.2.1 typed Stream runtime/wire correction](../reviews/requests/2026-07-19-lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction.md)
   must settle the final Stream instance, replay, policy, save/AWBC, and
   adapter shapes before the `source` surface can be removed.
3. The existing [Lang 01.4 typed resource declaration contract](../reviews/requests/2026-07-16-lang-01.4-typed-resource-declaration-surface-final-contract.md)
   remains the owner of the later public resource-family migration; its private
   grammar prerequisite is already complete.
4. [Proof 01.1.1.2 retained global-identity declaration grammar reconciliation](../reviews/requests/2026-07-20-seq-proof-01.1.1.2-retained-global-identity-declaration-grammar-reconciliation.md)
   must keep `asset` separate from `res`, preserve the dedicated
   Character/View/Action/Activity/Signal/Metric/Layer identities, and define
   typed body ownership before Stage 2.

## Stage-gate status

| Proof v6.1.1 stage | Status at this audit | Direct repository evidence |
| --- | --- | --- |
| Stage 0 — baseline and final enum/event substrate | **Implemented** | `grammar/{kinds,event,build,budget}.rs`, syntax/HIR limit enums, `SyntheticRole`, `AssertionMode::is_runtime_capable`, and callable-owner policy methods; completion evidence is in `2026-07-16-proof-concurrency-v6-1-1-stage-0-event-builder.md`. Later View and extern-capability work legitimately extends the callable-owner enum and must not be deleted to recreate the package-era inventory. |
| Stage 1 — private one-pass lossless grammar | **Partial; gate not met** | Structured private descendants exist for module/use, flow, ordinary function, predicate/proof, nominal types, trait/impl, `res`, entry, extern capability, test/bench, and Style, plus shared statement/expression/type/pattern families. The obsolete wrappers and retained-identity rows above remain unresolved. |
| Stage 2 — private grammar reconciliation and typed attachment | **Not started** | `GrammarBuild` still exposes an `UnattachedGrammarIndex`. The production incremental database still assigns `SyntaxNodeId(NonZeroU64)` to the old CST and has no `SyntaxDatabaseId`, `SyntaxLineageId`, snapshot-owned `AstNode<K>`, attachment table, or typed round trip. |
| Stage 3 — atomic public syntax switch | **Not started** | Public `TypedSyntaxTree` owns a source `String` and `Vec<Item>`; public `ParsedSource` still separately owns the old Rowan tree and detached typed tree. Compiler/HIR callers consume `typed_tree()`. |
| Stage 4 — final predicate/proof surface and exact `ProofBlock` | **Private shadow only** | The private parser covers ordinary-name predicate/proof headers, bodies, recovery, and limits. Public `ProofItem` still owns `IdRef`, raw `body: String`, and `Vec<ProofClause>` and stable proof documentation still shows the provisional entity-ID form. Trusted-proof policy is implemented, but it remains attached to the provisional public proof representation until this switch. |
| Stage 5 — private `HirDatabase`, arenas, scopes, locals, captures | **Identity vocabulary only** | Module-qualified typed ID, limit, and synthetic-role enums exist. There is no `HirDatabase`, immutable paged arena/module snapshot implementation, liveness ledger, final scope/local/capture arenas, or direct attached-node lowering. |
| Stage 6 — atomic HIR/project/symbol switch | **Not started beyond reusable symbol substrate** | `HirModule` remains a `Vec`/clone model that stores syntax values. `lower_to_hir(&TypedSyntaxTree)`, `HirProject::linked_module`, and `HirModule::append_module_body` remain. `ProjectSymbolTable` exists and owns later View/extern-capability callables, but current registration iterates ordinary functions only; predicate/proof registration and `ProofArtifactId` are absent. |
| Stage 7 — runtime assertion identity/persistence boundary | **Policy substrate only** | Check/Debug/Prove disposition and release omission exist. Core `RuntimeAssertion` still has only public `condition`, `message`, and `profile` fields; guard/fingerprint newtypes, condition index, session inventory/fault identity, execution diagnostic context, and the atomic codec migration are absent. |
| Stage 8 — migration, deletion, docs, tests, structural closure | **Partial cleanup only** | Borrow-block, hook, memo, parser, separate trusted-axiom, state, reducer, and Agent declaration removals are complete or supersede package-era rows. Provisional proof, detached AST, clone/linked HIR, obsolete metadata/source/function/resource paths, public callers, final direct/compile-fail suites, and final structural evidence remain. |

On a stage-gate basis, the original package is approximately **25–30%**
complete. After removing rows that later user decisions deliberately
superseded, the still-valid requirement set is approximately **30–35%**
complete. These are planning estimates, not test-pass percentages: Stage 2
through Stage 7 contain the majority of the public migration and runtime
boundary work.

## Superseding decisions that must be preserved

- The ownership/borrow block is permanently deleted. Current `&`, `&mut`,
  prefix borrow/dereference, lexical bindings, and explicit `drop` are the
  authority; no historical recognizer or diagnostic returns.
- `trusted axiom` remains deleted. Trust is
  `#[verify.trusted(reason = "...")]` metadata on an ordinary proof, with
  policy-controlled acceptance and transitive verifier reporting. The final
  proof switch replaces the provisional proof carrier without creating a
  second trust model.
- Package-era `AgentItem`, `StateItem`, `HookItem`, `MemoFunctionItem`,
  `ParserItem`, `MemoBlockExpression`, and source-visible task/dialogue/stream
  callable families are not implementation targets. Later Lang contracts
  either removed them or selected ordinary declarations.
- Later View and accepted-callable contracts added valid
  `CallableDeclarationOwner::View` and `ExternCapability` responsibilities.
  Proof integration extends that one owner/table; it must not shrink the enum
  back to the package-era three-row example.
- The package's statement that Tier 2 is optional is superseded by current
  repository policy. Any broad public syntax/HIR cut that spans multiple
  crates and affects runtime, Agent, MCP, or capture paths must run
  `just test-tier2` and reconcile stale expectations rather than preserving an
  obsolete production shape.

## Next safe order

1. Finish the implementation-ready Lang 01.5.1 single-manifest decoder and
   consumer migration, then delete `extern mod`, `dialogue defaults`, source
   `content`, and concrete Activity-origin syntax atomically.
2. Complete the independently implementation-ready Lang 01.1.1 ordinary
   function-role migration and the remaining Lang 01.4 public `res` migration
   at their own coherent cut points; neither should be folded into a proof
   compatibility layer.
3. Obtain the corrected Lang 01.3.1.2.1 Stream contract and the AW-AH-009.4.2
   then AW-AH-009.4.3 CharacterDialogue source/HIR contracts. They block the
   final source/expression/HIR inventory and cannot be guessed during Proof
   Stage 2.
4. Complete
   [Proof 01.1.1.2 retained global-identity declaration grammar reconciliation](../reviews/requests/2026-07-20-seq-proof-01.1.1.2-retained-global-identity-declaration-grammar-reconciliation.md),
   implement its private rows, and remove regular-project top-level
   statements.
5. Re-run the complete reduced Stage 1 inventory gate. Only then begin private
   Stage 2 grammar-node reconciliation and snapshot-owned typed attachment.

## Completion boundary

This audit intentionally does not introduce a removed-syntax recognizer,
diagnostic, alias, compatibility path, or source-text gate.  The legacy forms
will disappear through ordinary grammar removal at their respective atomic
migration boundaries, then existing parser/compiler rejection tests can cover
their absence without preserving their spellings.
