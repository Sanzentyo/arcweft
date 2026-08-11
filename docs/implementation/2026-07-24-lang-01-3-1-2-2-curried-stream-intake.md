# Lang-01.3.1.2.2 curried Stream intake

Date: 2026-07-24

Baseline: `7aefd1daeeb04d165151f78965bee0b5033afa4c`

## Accepted packages

- Lang-01.3.1.2.1 typed Stream runtime/wire correction:
  `docs/reviews/packages/zips/Lang-01.3.1.2.1-typed-stream-runtime-wire-contract-correction-final-contract.zip`
  (`SHA-256 66809A1280A507F69BB78D9DF3BF7AF227A91CD68B86CF8771CBF9EE20AA856A`).
- Lang-01.3.1.2.2 curried external Stream correction:
  `docs/reviews/packages/zips/arcweft-lang-01.3.1.2.2-curried-external-stream-final-contract.zip`
  (`SHA-256 D1BD7FB5301509CA88BE7C9D3662942CA88472D11143499C0C3067D626DF9418`).

Both packages are final and useful implementation inputs. The later package
correctly requires all accepted callable groups to survive into one canonical
coordinate/value product and makes `RuntimeFunctionValue` the sole owner of
external Stream partials.

Both extracted manifests were verified on 2026-07-24: all 17 parent entries
and all 40 child entries matched `MANIFEST.sha256`.

## Repository adjudication

The child package's short identity names do not authorize parallel aliases.
They are interpreted through the parent final owners:

- stable definition identity: `RuntimeStreamDefinitionKey`;
- RuntimePlan/AWBC table index: `RuntimeStreamDefinitionId`;
- generation: `StreamGeneration`;
- allocation ordinal: `StreamInstanceOrdinal`;
- complete instance identity: `StreamInstanceKey`; and
- runtime type layout: the existing `TypeLayoutHash`.

The later Lang-01.3.1.2.1 reconciliation supersedes provisional Lang-01.1.1
Stream public/wire shapes while retaining the ordinary-function, direct-frame,
typed-await, CFG, and own-scope-yield semantic substrate. The repository will
therefore remove `FunctionKind` and old authored role branches before the final
Stream runtime cut, rather than repair or copy them into the new runtime model.

## Blocking opcode inconsistency

> Resolved on 2026-07-25 by the accepted
> [Lang-01.3.1.2.2.1 wire-allocation return](2026-07-25-lang-01-3-1-2-2-1-curried-stream-wire-intake.md).
> The historical conflict below records why that correction was requested; it
> is no longer an active design blocker.

The two accepted packages allocate codec-8 instruction bytes incompatibly:

```text
Lang-01.3.1.2.1: 0x27 OpenStream, 0x28 FinishStream
Lang-01.3.1.2.2: 0x27 ApplyExternalStreamGroup, 0x28 OpenStream
```

The child preserves unrelated parent lifecycle choices but does not allocate
the retained `FinishStream`. The parent's pre-implementation collision rule is
conditioned on current `main` having consumed a proposed value; it does not
resolve two accepted packages assigning different meanings. Canonical artifact
bytes, verifier dispatch, VM dispatch, and hard-rejection vectors therefore
remain design-blocked rather than locally guessed.

The independently throwable correction is:

- [Lang-01.3.1.2.2.1 curried Stream wire-allocation reconciliation](../reviews/requests/2026-07-24-lang-01.3.1.2.2.1-curried-stream-wire-allocation-reconciliation.md)

The request has returned and must not be dispatched again.

## Work that continues independently

- deletion-driven ordinary-function migration and `FunctionKind` removal;
- direct-frame suspension verification on the existing codec-stable substrate;
- shared external-capability callable evidence that does not allocate Stream
  runtime/wire types; and
- other returned dependency contracts that do not publish the conflicting
  codec-8 instruction table.

No Stream compatibility alias, flat external argument sidecar, dual codec,
Source replacement shim, source gate, CSS path, or Takumi path is introduced.
