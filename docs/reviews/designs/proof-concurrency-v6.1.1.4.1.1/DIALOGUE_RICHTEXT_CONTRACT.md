# Dialogue and RichText contract

## Ownership and locality

`HirDialogueContentApplication` remains the AW-AH-009.4.2 expression payload. It owns exactly one `HirDialogueContent`. That content owns two ordered slices: `nodes` and `tags`. `HirDialogueContentId` is the owner ExprId; node/tag/argument IDs add deterministic contiguous ordinals. Start-tag nodes reference entries in the content-owned tag slice; each tag owns its ordered argument slice. These identities are stable only for that immutable HIR snapshot and cannot cross module/content boundaries. Nested interpolation, call, condition, and coordinate values are ordinary same-arena ExprIds.

## Exhaustive content projection

Attached syntax tokens project as follows:

- text -> Text(decoded UTF-8);
- raw text -> Raw(exact semantic text, no delimiter/source spelling);
- escape -> Escape(decoded char);
- ruby -> Ruby(decoded base, decoded ruby);
- authored/inferred start tags -> corresponding StartTag node and a `HirRichTextTag`;
- authored/inferred end tags -> corresponding EndTag node;
- interpolation -> same-arena ExprId;
- point/reset controls -> typed Control;
- marks -> Mark(HirName);
- explicit/inferred line/paragraph/page boundaries -> LineBreak;
- classified malformed content -> Error with typed issue;
- unclassified syntax -> poisoned Error node, never a fake Text default.

`SpeakerLine`, string `ContentCall`, source-coordinate maps, and a second expression arena are not HIR alternatives.

## Tag identity

Canonical builtins are the exact family-bearing `HirBuiltinRichTextTag` inventory in `RUST_SCHEMAS.md`. It projects the current presentation owners one-to-one: direct styles (8), style selectors (5), layout selectors (7), transforms (4), object (1), and builtin Fx (10), plus closed controls, host events, and conditionals. Source spellings such as `r|br`, `clear|er|cm`, `i|italic`, `oblique|slant`, `vertical|vertical_rl`, and other grammar-owned aliases normalize before HIR; aliases are not semantic variants or compatibility maps. An arbitrary inferred marker uses `Marker(HirName)`. A registered project tag stores its declaration ItemId; an external tag stores a typed external project segment plus typed path. An unresolved tag stores a validated segment and typed issue, not raw source.

## Tag payload

`Arguments` retains ordinary tag arguments. `FxCall` and `DialogueCall` each point to a same-arena Call expression. `DialogueCall` here is retained and observable. It is not an ordinary `HirExprKind` variant. `Condition` points to a same-arena expression and must check as Bool. `None` is explicit.

## Arguments and checked values

Arguments have stable `HirRichTextArgumentId`s and are Positional, Named, or Invalid. A valid value is the opaque decoded UTF-8 `HirRichTextValue`; there is no `Missing` valid-value variant. Quote style, equals sign, encoded spelling, and source ranges live in source components. Invalid arguments preserve one of EmptyKey, InvalidKey, InvalidEscape, UnterminatedQuote, KeyTooLong, ValueTooLong, MissingValue, or DecoderFailure. No invalid value defaults to an empty string, zero, false, or enum default.

`arcweft-lang-sema::rich_text` owns `CheckedRichTextValueId` and the checked typed value table. Runtime-plan consumes only checked IDs. HIR does not depend on presentation, renderer, CSS, Takumi, or a backend.

## Limits and atomicity

- 4,096 tag nodes per DialogueContent;
- 32,768 argument records per DialogueContent;
- 16,384 encoded bytes per tag body;
- 32 arguments per tag/Fx/dialogue inline call;
- 4,096 encoded bytes and 4,096 decoded bytes per value;
- 64 bytes per key.

Syntax charges body/key/encoded/decoded byte limits before publishing a typed tag. Over-limit markup is recovered as Text plus one typed diagnostic; no tag/argument identity exists. The HIR transaction charges tag and argument counts before allocating IDs/source rows. The checker charges nested-call depth, diagnostics, and checked-value slots. The decoder charges decoded bytes before returning a value. Exact succeeds. One-over aborts the complete owning tag/content transaction or executes the specified syntax Text recovery; it never truncates or publishes a partial tag.

Ordinary call limits remain separate: 128 call/callback args, 32 nested calls, 256 recovery nodes, 128 diagnostics, and at most two candidate attempts/retained results. RichText calls use 32 args while sharing the same call model.

## Source roles

Every authored tag has Whole, TagName, TagPayload, and per-argument Whole/Name/Value source rows. Inferred tags/end tags use synthetic source rows at the parser insertion point and remain marked inferred. Text/raw/escape/ruby/interpolation/control/mark/line-break/error nodes have their exact typed roles. No consumer reconstructs component identity from vector position alone; it queries parent + typed role + ordinal.

## Recovery and consumers

Known tag families remain `HirRichTextTag` with unresolved/invalid identity or payload and poison. Unclassified content becomes `HirDialogueNodeKind::Error`. LSP, formatter, Agent/debug, checker, runtime-plan, cache, and project publication query the same content/tag/argument IDs and source table. No syntax clone or fallback reader survives the authority switch.
