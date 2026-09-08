# AW-AH-009.4.2.1 accepted attached-content declaration/runtime ABI resolution

- Date: 2026-09-01
- Status: `READY_FOR_IMPLEMENTATION`
- Method: direct Sol max repository resolution
- Baseline Git commit:
  `fed4102e50c90b06e2dbabf1e7259ee09352008d`
- Baseline checkout: dirty; current source was treated as implementation
  evidence and no pre-existing changes were discarded
- Resolves:
  `docs/reviews/requests/2026-09-01-aw-ah-009.4.2.1-project-callable-attached-content-declaration-and-runtime-abi-correction.md`
- Open questions: none

## Selected result

A project callable declares at most one dedicated trailing attached-content
parameter after all ordinary `()` parameter groups:

```arcw
fn surround(color: Color)[body: RichContent] -> DialogueContent {
    compose(color, body)
}

fn maybe()[body?: InlineContent] -> DialogueContent {
    match body {
        Some(content) => content
        None => empty_content()
    }
}

fn fallback()[body: DialogueContent = default_content()] -> DialogueContent {
    body
}
```

`InlineContent`, `RichContent`, and `DialogueContent` in this bracket are
closed admission-role tokens, not `TypeKind` references. The required binding
is a simple local name. `?` selects optional presence; `= expression` selects
defaulted presence; using both is invalid.

The declaration lowers to the sole project schema row:

```text
CallableAttachedContentParameter
  terminal group
  required | optional | defaulted
  Declared(Inline | Rich | Dialogue)
  RuntimeContent
```

The runtime slot is a distinct attached-content operand appended after the
receiver and ordinary runtime operands of the terminal group. It is not an
authored ordinary argument or a semantic-only operand. Structural presentation
rows retain no runtime slot.

Default expressions use a dedicated acyclic semantic transcript. Project
callable references terminate at final checked declaration, schema, execution,
and effect leaves; the checked callable catalog publishes each interface
digest once, only after the complete default batch is sealed. No preliminary
interface digest or recursive callable-body hashing exists.

Reveal-time content effects use the ordinary runtime function authority, but
the function-site body evolves from expression-only to
`Expression | Executable`. Executable sites own the exact effect set and flow
ops, lower to ordinary AWBC functions/`EmitEffect`, and are activated by the
native or AWBC reveal scheduler through the shared function-application frame
authority. The checked site owns the closed typed effect set; core's
`RuntimeFunctionEffectSet` is its sole runtime projection, and AWBC does not
guess a set from the operation. Content construction only snapshots captures.
The reveal activation key is the exact dialogue activation plus rebased effect
site. This callback path replaces, rather than supplements, the old line-task
direct effect action/schedule path.

The same executable-site authority closes declaration-owned defaults.
Nonterminal project applications produce a typed `ProjectContinuation` that
retains one checked lineage plus once-evaluated prefix values; they do not
reserve or return a function site. Only a terminal checked call owns an
`Invoke` outcome and names the closed
`(RuntimeCallableId, CallableInstantiationDigest, CallableGroupIndex)`
instance whose `RuntimeFunctionSite` executes the authored body.
`RuntimePureHelper` is not a project-call authority. A terminal instance may
use an expression body only when the final selected-body fold proves
ExpressionCompatible, NonSuspending, and an empty closed effect row. Any
selected project call is independently FlowRequired, and therefore uses the
same executable ProjectCall frame as an effectful or suspending body/default.
Runtime calls retain completed group and attached ABI position, and
native/AWBC invocation uses the ordinary function-frame return continuation.

Each runtime fragment additionally retains one generation-local source lookup
key and each runtime attached operand retains its exact normalized ABI type.
The lookup key is excluded from stable identity/digests; the ABI type lets an
omitted optional/defaulted body construct typed `None` without reopening the
callee schema.

`DialogueLine` is a different selected-call channel. Its line-owned semantic
content operand is not an attached body: selected-call preparation validates
the exact site family and HIR owner, then returns no attached operand and lets
the dialogue application seal own that semantic content.

## Why this form

- An item attribute cannot own a body local, source role, default expression,
  terminal group, or ABI destination without a copied side table.
- An ordinary `DialogueContent` parameter would allow parenthesized value
  supply, lose bracket identity, and incorrectly turn admission roles into
  runtime types.
- The trailing bracket is lossless, mirrors the selected call surface, gives
  the binding/default one signature owner, and makes terminal-group/ABI order
  deterministic.

See [FINAL_CONTRACT.md](FINAL_CONTRACT.md),
[RUST_SHAPES.md](RUST_SHAPES.md),
[VALIDATOR_MATRIX.md](VALIDATOR_MATRIX.md),
[ACCEPTANCE_MATRIX.md](ACCEPTANCE_MATRIX.md), and
[IMPLEMENTATION_ORDER.md](IMPLEMENTATION_ORDER.md).
