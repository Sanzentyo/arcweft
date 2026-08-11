# Consumer and deletion inventory

## Affine/ABI consumers

- `arcweft-core::value` ownership, slot, environment, sequence, iterator, pattern, payload, constant table;
- structured engine evaluation, scope cleanup, suspension, child transfer, mailbox;
- `arcweft-core::awbc` schema/codec/verifier/VM/fiber/product-step/snapshot;
- runtime codegen/accelerator/compiled-region exchange;
- runtime-plan operand/capture/pattern lowering;
- runtime-driver snapshot/save/restore/hot swap;
- Stream handle/table/open/apply/finish and all parent codecs/tests;
- host/native/Web/Agent payload boundaries.

Delete all direct `RuntimeValue: Clone`/Serde use, ABI-2 labels, per-driver activation bypasses, independent-value drop commit, and restore cursor inference.

## View consumers

- final semantic View catalog and checked value types;
- compiler synthetic AWBC generation and cross-section refs;
- `arcweft-view` instruction/value program resource;
- bundle ViewProgram/ViewText/Input/Style codecs and content root;
- runtime-driver View catalog/evaluator/mount state/handler/direct-await;
- session save/restore/hot replacement;
- native/Web/headless/Agent/MCP observations;
- generated artifacts and compiler tests.

Delete unowned input bindings, may-be-affine retained admission, live binding save, static origin without requirement rows, and ambiguous fragment selection.

## Future surface non-interference

This cut does not implement `mount`, `ViewHandle`, Action `emit/receive`, shared View parser reconstruction, `AwaitView` surface changes, Dialogue content/Ruby, try/pipe, Choice, Style naming, or `@family:.` changes. Future scoped handles must use the generic affine owner plus a dedicated presentation lifecycle domain; they must not reopen unrestricted retained View render state.
