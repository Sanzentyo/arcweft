# Proof concurrency v6.1.1 — Stage 1 remaining-row audit

Date: 2026-07-20

## Outcome

No additional Stage 1 private shadow-grammar row is implementation-ready at
this revision.  The only Stage 1 row with a settled final private grammar,
typed `res`, was completed by the Lang 01.4 Cut 1a implementation.  The three
remaining legacy declarations cannot be made more structured without either
preserving a source form whose final owner is elsewhere or creating a forbidden
dual reader.

This is an evidence audit only.  It changes no parser, CST, public AST, HIR,
or runtime contract.

## Row matrix

| Stage 1 row | Current production state | Final-direction evidence | Decision |
| --- | --- | --- | --- |
| `extern mod` | `item.rs` recognizes `ExternModuleItem`; `document.rs` deliberately leaves it a generic logical-line wrapper. | Lang 01.5 moves concrete adapter/module binding to generated metadata and the build profile.  Lang 01.5.1 records that old launch/profile readers are still live; adding the final manifest decoder first would create a dual reader, while deleting them first would lose runtime inputs. | Do not add a private `ExternModuleItem` shadow node.  Implement only as part of the atomic single-manifest decoder and consumer migration. |
| `dialogue defaults` | `item.rs` recognizes `DialogueDefaultsItem`; it is likewise not a typed private grammar descendant in `document.rs`. | Lang 01.5 assigns style/default data to typed style/View ownership and selection to build profiles.  The current implementation note records that launch/profile consumption of dialogue defaults remains active.  AW-AH-009.4 explicitly keeps runtime-driver ownership separate from source grammar. | Do not add a private `DialogueDefaultsItem` shadow node.  Delete the surface only with the same atomic metadata/profile migration. |
| live `source` | `item.rs` recognizes `SourceItem`; the accepted private document grammar does not model it as a typed descendant. | Lang 01.3.1 fixes the destination as ordinary `fn -> Stream<T, E>` and removes both the keyword and `Source<T, E>`.  Lang 01.3.1.2.1 records unresolved mutually inconsistent callable, instance, replay, policy, save/AWBC, and adapter-wire shapes. | Do not add a private `SourceItem` shadow node.  It would commit an obsolete surface just before its removal.  Wait for the corrected typed Stream runtime/wire contract. |
| public entity/resource families | Lang 01.4 Cut 1a already provides the sole private `res` grammar: typed nominal head, explicit ref, fields, diagnostics, and recovery. | The Cut 1a implementation note states that public AST/HIR migration and individual legacy-family removal are later cuts. | This Stage 1 grammar row is complete.  Do not add parallel shadows for the legacy individual family keywords. |

## Why no Stage 2 implementation can start independently

Stage 2 attaches public identity to the accepted grammar.  It cannot safely
start while `extern mod`, `dialogue defaults`, and `source` still enter the
document as legacy generic wrappers, because assigning identities to those
forms would make the provisional surfaces part of the public identity contract.
That is precisely the compatibility commitment the final-contract policy
forbids.

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

## Completion boundary

This audit intentionally does not introduce a removed-syntax recognizer,
diagnostic, alias, compatibility path, or source-text gate.  The legacy forms
will disappear through ordinary grammar removal at their respective atomic
migration boundaries, then existing parser/compiler rejection tests can cover
their absence without preserving their spellings.
