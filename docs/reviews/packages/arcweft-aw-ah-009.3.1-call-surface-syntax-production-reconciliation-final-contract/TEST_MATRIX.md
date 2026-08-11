# Test matrix

## 1. Test conventions

All ranges are half-open UTF-8 byte ranges in the exact source shown. Spaces, newlines, and multibyte characters count as authored bytes. Assertions inspect typed accessors, not debug strings or source scans.

`P` means `CallSurfaceSyntax::Parenthesized`; `C` means `CallSurfaceSyntax::CallbackBlock`. A recovered expression test uses the full-source/recovering parser and separately asserts that the strict fragment parser reports an error.

## 2. Exact parenthesized call ranges

### P-01 empty

Source:

```text
f()
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,3)` |
| callee | `[0,1)` |
| open `(` | `[1,2)` |
| arguments | empty |
| separators | empty |
| trailing comma | none |
| close `)` | `[2,3)` |
| content | `[2,2)` |

Test name: `parenthesized_empty_call_has_exact_ranges`.

### P-02 positional with UTF-8

Source:

```text
f(α, "猫")
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,12)` |
| callee | `[0,1)` |
| open | `[1,2)` |
| argument 0 full/value | `[2,4)` / `[2,4)` (`α`) |
| separator 0 | `[4,5)` |
| argument 1 full/value | `[6,11)` / `[6,11)` (`"猫"`) |
| close | `[11,12)` |

Test name: `parenthesized_positional_utf8_ranges_are_bytes`.

### P-03 named

Source:

```text
paint(look = .smile)
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,20)` |
| callee | `[0,5)` |
| open | `[5,6)` |
| argument full | `[6,19)` |
| name | `[6,10)` |
| equals | `[11,12)` |
| value | `[13,19)` |
| close | `[19,20)` |

Test name: `parenthesized_named_argument_ranges_exclude_trivia`.

### P-04 postfix spread

Source:

```text
log(fields...)
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,14)` |
| callee | `[0,3)` |
| open | `[3,4)` |
| argument full | `[4,13)` |
| value | `[4,10)` |
| ellipsis | `[10,13)` |
| close | `[13,14)` |

Test name: `parenthesized_postfix_spread_has_exact_ellipsis`.

### P-05 trailing comma

Source:

```text
f(α,)
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,6)` |
| open | `[1,2)` |
| argument | `[2,4)` |
| between-argument separators | empty |
| trailing comma | `[4,5)` |
| close | `[5,6)` |

Test name: `parenthesized_trailing_comma_is_not_separator`.

### P-06 nested

Source:

```text
outer(inner(猫), β)
```

Expected outer call:

| Field | Range/value |
|---|---|
| call | `[0,21)` |
| callee | `[0,5)` |
| open | `[5,6)` |
| argument 0 | `[6,16)` |
| separator | `[16,17)` |
| argument 1 | `[18,20)` (`β`) |
| close | `[20,21)` |

Expected inner call:

| Field | Range/value |
|---|---|
| call | `[6,16)` |
| callee | `[6,11)` |
| open | `[11,12)` |
| argument | `[12,15)` (`猫`) |
| close | `[15,16)` |

Test names: `nested_parenthesized_calls_keep_independent_ranges` and `signature_cursor_selects_innermost_parenthesized_list`.

## 3. Recovery boundaries

### R-01 missing close at expression end

Source:

```text
f(α, β
```

Expected recovering parse:

| Field | Range/value |
|---|---|
| call | `[0,8)` |
| callee | `[0,1)` |
| open | `[1,2)` |
| argument 0 | `[2,4)` |
| separator | `[4,5)` |
| argument 1 | `[6,8)` |
| terminator | `RecoveredMissing` |
| insertion | `8` |
| boundary | `EndOfExpression` |
| close | none |
| missing-close primary | `[8,8)` |
| missing-close related | `[1,2)` |

The strict fragment parser returns the missing-close error and no successful strict result. The full parser retains the typed call and diagnostic.

Test name: `missing_close_retains_parenthesized_call_at_owner_end`.

### R-02 missing close before owner token

Fixture: an expression argument in an owning bracket or outer call where the inner `)` is missing and the next authored token is the owner's `]`, `}`, outer comma, or `)`.

Expected:

- `RecoveredMissing.insertion == boundary.range.start()`;
- `CallRecoveryBoundarySyntax::Token` has the exact typed token and range;
- the boundary token is not consumed by the inner call parser;
- the outer node parses and retains its own delimiter/range;
- no close-paren range exists on the inner list.

Direct cases:

1. `[f(α, β]` with `CloseBracket` boundary;
2. `{ value: f(α, β }` with `CloseBrace` boundary;
3. `f(α, β; next())` in a statement owner with `Semicolon` boundary;
4. `alice(look = .smile: hello` with `Colon` boundary in the speaker-head fixture.

Test names begin `missing_close_stops_before_...` for each boundary kind.

### R-03 isolated malformed argument

Source:

```text
f(α, @@@, β)
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,14)` |
| open | `[1,2)` |
| argument 0 | `[2,4)`, parsed positional |
| separator 0 | `[4,5)` |
| argument 1 | `[6,9)`, recovered positional |
| argument 1 value | `Expr::Raw("@@@")` |
| recovery diagnostic | `[6,9)` |
| separator 1 | `[9,10)` |
| argument 2 | `[11,13)`, parsed positional |
| close | `[13,14)` |

The full parser retains the typed call and one malformed-argument diagnostic. Signature help remains available for all three slots. The strict fragment parser rejects the expression.

Test name: `malformed_middle_argument_recovers_one_exact_slot`.

### R-04 named malformed value

Source:

```text
f(look = @@@, stage = main)
```

Expected: the first syntax entry remains `Named` with exact name/equals ranges, its value is `Expr::Raw("@@@")`, and its recovery range is exactly the malformed value. The second named argument parses normally.

Test name: `malformed_named_value_preserves_named_form`.

### R-05 recovery rejection boundaries

The following remain ordinary grammar errors and do not create empty or phantom entries:

```text
f(,x)
f(x,,y)
f(x y)
f(name =)
f(...)
```

Tests assert the structured parser error and absence of a successfully retained call for the invalid owner position. They do not search source code for removed spellings.

Test names begin `call_rejects_...`.

## 4. Callback-block exact syntax

### C-01 implicit zero parameters

Source:

```text
items.tap { emit() }
```

Expected outer callback call:

| Field | Range/value |
|---|---|
| call | `[0,20)` |
| callee | `[0,9)` |
| open brace | `[10,11)` |
| parameter header | `ImplicitZero` |
| body | `[12,18)` |
| close brace | `[19,20)` |
| closure | `[10,20)` |
| semantic args | exactly one positional closure |

The nested `emit()` call is `P` with callee `[12,16)`, open `[16,17)`, and close `[17,18)`.

Test name: `callback_block_zero_params_has_exact_braces_and_body`.

### C-02 one parameter

Source:

```text
items.map { item => item.label }
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,32)` |
| callee | `[0,9)` |
| open brace | `[10,11)` |
| parameter 0 full/pattern | `[12,16)` / `[12,16)` |
| parameter separators | empty |
| fat arrow | `[17,19)` |
| body | `[20,30)` |
| close brace | `[31,32)` |

Test name: `callback_block_one_param_has_exact_header`.

### C-03 multiple parameters

Source:

```text
items.zip { item, index => item.label(index) }
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,46)` |
| callee | `[0,9)` |
| open brace | `[10,11)` |
| parameter 0 | `[12,16)` |
| parameter comma | `[16,17)` |
| parameter 1 | `[18,23)` |
| fat arrow | `[24,26)` |
| body | `[27,44)` |
| close brace | `[45,46)` |

The nested `item.label(index)` call is `P` with callee `[27,37)`, open `[37,38)`, argument `[38,43)`, and close `[43,44)`.

Test name: `callback_block_multiple_params_and_nested_call_keep_ranges`.

### C-04 typed parameter

Source:

```text
items.map { item: Label => item.text }
```

Expected:

| Field | Range/value |
|---|---|
| call | `[0,38)` |
| callee | `[0,9)` |
| open brace | `[10,11)` |
| parameter full | `[12,23)` |
| pattern | `[12,16)` |
| colon | `[16,17)` |
| type | `[18,23)` |
| fat arrow | `[24,26)` |
| body | `[27,36)` |
| close brace | `[37,38)` |

Test name: `callback_block_typed_param_has_exact_type_ascription`.

### C-05 multi-statement body and selected callback

Source:

```text
Button("Send").on_click {
  let label = name.text
  action.invoke(value = label)
}
```

Expected:

| Field | Range/value |
|---|---|
| inner `Button("Send")` call | `[0,14)` |
| outer callback callee | `[0,23)` |
| outer open brace | `[24,25)` |
| outer body | `[28,80)` |
| outer close brace | `[81,82)` |
| outer callback call | `[0,82)` |
| nested `action.invoke(...)` | `[52,80)` |
| nested open/close | `[65,66)` / `[79,80)` |
| nested named value | `[74,79)` |

Test names:

- `selected_callback_after_parenthesized_call_keeps_both_surfaces`;
- `callback_multistatement_body_range_excludes_brace_trivia`;
- `callback_outer_signature_help_is_not_applicable`;
- `nested_parenthesized_call_inside_callback_is_applicable`.

### C-06 malformed callback grammar

The following remain rejected under current grammar:

```text
items.map { => item }
items.map { item => }
items.map { item, => item }
items.map { item: => item }
items.map { item => item
items.map { }
```

An unclosed callback never produces a fabricated close brace. Tests assert current structured parser diagnostics and no typed callback-call surface.

Test names begin `callback_rejects_...`.

## 5. Dialogue and speaker surfaces

### D-01 colon speaker with options

Source fixture contains:

```text
alice(look = .smile, voice = "猫"): hello
```

Assert:

- `SpeakerLineSurface::argument_list()` is `Some`;
- parens, separator, named key/equal/value ranges are document-absolute UTF-8 bytes;
- `LineOptions` values correspond one-for-one with syntax entries;
- the colon remains owned by the speaker surface and is not in the argument list;
- signature help is applicable inside the list and not in the dialogue content.

Test name: `speaker_line_owns_exact_parenthesized_option_list`.

### D-02 speaker shorthand without parentheses

Source:

```text
alice: hello
```

Assert `argument_list() == None` and signature help is `NotApplicable`. No empty `ArgumentListSyntax` is synthesized.

Test name: `speaker_line_without_parentheses_has_no_argument_list`.

### D-03 content call

Source fixture contains:

```text
alice.say(look = .smile)[hello]
```

Assert `ContentCallSurface` owns exact callee, list, and content ranges. The list is the carrier consumed by signature help. The content brackets are not treated as call delimiters.

Test name: `content_call_owns_exact_argument_list_surface`.

### D-04 content shorthand

Source:

```text
alice[hello]
```

Assert `argument_list() == None` and signature help is `NotApplicable` for the content call head.

Test name: `content_shorthand_has_no_fake_empty_argument_list`.

## 6. Parser-only and generated construction

### G-01 no public source-call constructor

A downstream compile-pass fixture can pattern-match `Expr::Call(call)` and use accessors. A compile-fail documentation/UI test attempts to call removed `Expr::call`, `Expr::selected_call`, and a `CallExpr` struct literal from another crate and fails because no public construction route exists.

This is a Rust visibility/API test, not a checked-in source spelling scan.

Test name: `source_call_payload_is_read_only_outside_syntax_crate`.

### G-02 generated runtime call

Construct the existing source-independent `arcweft_core::value::RuntimeExpr::Call` through its current semantic owner. Assert it lowers/evaluates according to the existing runtime test fixture and that its type contains no `TextRange`, `ArgumentListSyntax`, or `CallSurfaceSyntax` input.

Test name: `generated_runtime_call_requires_no_authored_surface`.

### G-03 authored tests parse source

Migrate every former direct source-AST construction test to parse a literal or attached fragment. Assert the parsed call has exact syntax. There is no test-only unchecked call constructor.

Test name: `authored_call_fixtures_are_parser_constructed`.

## 7. HIR preservation

### H-01 parenthesized clone/lower

Parse P-02 and lower through the current HIR path. Assert the HIR-retained `Expr::Call` has a `Parenthesized` surface equal to the syntax AST, including multibyte ranges and separator position.

Test name: `hir_preserves_parenthesized_call_surface`.

### H-02 recovered parenthesized clone/lower

Lower R-01 through the full-source recovery path. Assert `RecoveredMissing`, insertion, boundary, arguments, and parser diagnostic remain associated with the document/HIR result.

Test name: `hir_preserves_recovered_argument_list_boundary`.

### H-03 callback clone/lower

Parse C-04 and C-05. Assert HIR preserves `CallbackBlock`, exact brace/header/body ranges, semantic one-closure argument shape, and nested `Parenthesized` calls.

Test name: `hir_preserves_callback_block_surface_and_nested_calls`.

### H-04 speaker/content surfaces

Lower D-01 and D-03. Assert `SpeakerLineSurface` and `ContentCallSurface` retain the exact shared `ArgumentListSyntax` without converting the special forms into ordinary calls.

Test name: `hir_preserves_special_form_argument_lists`.

## 8. Signature-help applicability

### S-01 parenthesized cursor boundaries

For P-03 assert:

- cursor `6` (immediately after `(`) is applicable with slot 0;
- cursor before/inside/after the named argument remains slot 0 through `19` (immediately before `)`);
- cursor `20` is not applicable;
- cursor before the opening parenthesis is not applicable.

Test name: `signature_cursor_uses_exact_parenthesized_bounds`.

### S-02 comma transition

For P-02 assert slot 0 at cursor `4` (before comma), slot 1 at cursor `5` (after comma), and slot 1 before close. For P-05 assert the cursor after trailing comma reports the empty next syntactic slot and the parent AW-AH-009.3 resolver applies its frozen parameter policy.

Test name: `active_argument_slot_transitions_at_comma_end`.

### S-03 recovered missing close

For R-01 assert applicability through cursor `8`, and inapplicability beyond the owner boundary. The resolver receives the same list carrier as a closed call.

Test name: `signature_help_applies_through_missing_close_insertion`.

### S-04 callback outer surface

For C-01 through C-05, every cursor whose innermost call is the outer callback returns `SignatureQueryOutcome::NotApplicable`. An instrumented resolver/cache counts zero candidate-resolution invocations and zero successful cache insertions.

Test name: `callback_surface_bypasses_signature_resolver`.

### S-05 nested call inside callback

For C-03 and C-05, cursors inside the nested parentheses invoke the one resolver exactly once and use the nested list. The outer callback remains irrelevant.

Test name: `nested_parenthesized_call_in_callback_uses_one_resolver`.

### S-06 parenthesized then selected callback

For C-05, assert the cursor in `Button("Send")` resolves that call, a cursor on `.on_click` or callback whitespace is not applicable, and a cursor in `action.invoke(...)` resolves the nested call.

Test name: `selected_callback_cursor_dispatch_is_surface_exact`.

## 9. Semantic and runtime non-regression

### M-01 semantic equivalence by surface

Parse semantically equivalent callable applications where one current grammar path is parenthesized and one is callback sugar. Assert ordinary checker/lowering consumes the same callee/argument meaning where applicable and does not branch on call surface. Callback remains one positional closure argument.

Test name: `checker_consumes_call_meaning_independent_of_surface`.

### M-02 exhaustive traversal

Run syntax, HIR, sema facts, effect, runtime-plan, verifier, labels, formatter, and CLI traversal fixtures containing both surface kinds. Assert no child is skipped and callback closure bodies are visited once.

Test name: `all_call_traversals_visit_final_private_payload`.

### M-03 static generic grammar

Parse every current valid static generic/turbofish call fixture through the Pratt grammar and assert exact parenthesized ranges. Invalid generic spellings produce ordinary parser diagnostics. No source-scanning fallback is invoked or tested.

Test name: `static_generic_calls_use_pratt_owned_argument_ranges`.

## 10. Limit, overflow, and atomicity coverage

The parent parser/signature limits remain unchanged. Add exact/one-over tests that include the new syntax carriers:

- exact maximum arguments succeeds and produces matching syntax entries;
- one over fails at the existing limit without a partial `CallExpr`;
- exact maximum callback parameters succeeds;
- one over fails without a partial callback surface;
- exact diagnostic limit with recovered arguments succeeds according to the parent limit;
- one over fails atomically;
- checked base/range addition overflow returns the existing fatal/invariant failure and creates no syntax value.

Test names begin `call_surface_exact_limit_` and `call_surface_one_over_`.

## 11. No-source-gate requirement

No test reads checked-in Rust or documentation and passes/fails by searching for `Expr::Call`, constructor names, helper names, file paths, or removed spellings. API visibility tests compile Rust fixtures; behavioral tests parse source and inspect typed results; migration completeness is enforced by compiler exhaustiveness, workspace Clippy/test, and the canonical structural audit.
