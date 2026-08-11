# Recovery, ambiguity, synchronization, and limits

## 1. Classification

Top-level classification runs after contiguous outer prefixes and optional visibility. The next significant token selects exactly one current item grammar.

- Exact `character`, `view`, `action`, `activity`, `signal`, `metric`, and `layer` heads enter their dedicated declarations.
- `res` enters only `ResourceDeclarationItem`.
- `asset`, `content`, `source`, old configured-resource heads, `dialogue defaults`, and `extern mod` do not enter a retained declaration grammar.
- A dotted expression/call such as `action.send(...)`, `view.mount(...)`, or `activity.run(...)` is not a declaration merely because its first identifier equals a family keyword.
- Statement-context `signal` is classified only inside a statement/body grammar. At regular-project top level, a non-declaration is `ErrorItem`, never executable top-level flow.
- `metric` consumes exactly one closed kind token before optional ID/name. An unknown word is a typed `MetricKind` recovery diagnostic and is not accepted as an extension.

## 2. Common header recovery

| Condition | Typed evidence | Diagnostic |
|---|---|---|
| missing ordinary name | zero-width `MissingName` | `syntax.declaration.missing_name` |
| keyword/dotted/invalid name | `MissingName` plus `ErrorNode` for spelling | `syntax.declaration.invalid_name` |
| lone/malformed `@` | `DeclarationPublicId` + `MissingDeclarationId`/`ErrorNode` | `syntax.declaration.malformed_id` |
| relative declaration ID | `DeclarationPublicId` retaining token | `syntax.declaration.relative_id` |
| wrong family absolute ID | `WrongFamilyReference` under public ID | `syntax.declaration.wrong_family_id` |
| family-inappropriate tail | typed recovery child | family code or `syntax.declaration.unexpected_header` |
| trailing syntax | `ErrorNode` | `syntax.declaration.trailing_syntax` |

The exact token or zero-width insertion point is primary. Wrong-family diagnostics retain the family keyword as a related range.

## 3. Family recovery

### Character

- missing alias: zero-width `MissingName`, `syntax.character.missing_alias`;
- duplicate display member: both typed members, duplicate primary and first related, `syntax.character.duplicate_member`;
- unknown body member: `ErrorDeclarationMember`, `syntax.character.unknown_member`;
- missing body/close: typed missing body/delimiter with declaration-boundary synchronization.

### View

- absent `()` produces a typed empty missing fixed group and `syntax.view.missing_parameters`;
- invalid parameter pattern remains a common typed pattern and `syntax.view.invalid_parameter`;
- authored return arrow: error child and `syntax.view.return_not_allowed`;
- malformed export retains `ViewExportDeclaration`, missing part/as/public children, and exact export diagnostics;
- misplaced export remains typed and `syntax.view.misplaced_export`;
- invalid value is an `ErrorExpression` and `syntax.view.invalid_value`;
- nested missing delimiters stop before the next top-level declaration.

### Action

- absent `()` produces a typed missing group and `syntax.action.missing_parameters`;
- invalid binding pattern remains typed and `syntax.action.invalid_parameter`;
- missing `: Type` creates `MissingType` and `syntax.parameter.missing_type`;
- default expression is retained under recovery and `syntax.action.default_not_allowed`;
- return arrow and body produce `syntax.action.return_not_allowed` / `syntax.action.body_not_allowed`.

### Activity

- unknown/duplicate/out-of-order sections retain typed/error members with first-related evidence;
- missing section braces produce `MissingBody` and `syntax.activity.missing_section_body`;
- missing/duplicate port names/types and initializers retain typed ports and exact diagnostics;
- contract unknown clauses use `ErrorDeclarationMember`; missing condition uses `MissingExpression`;
- a `requires` after `ensures` remains typed and `syntax.activity.contract_order`.

### Signal

- missing colon or type creates zero-width recovery with `syntax.signal.missing_colon` / `syntax.signal.missing_type`;
- initializer/body/policy tail is retained as error and `syntax.signal.initializer_not_allowed` or trailing-syntax diagnostics;
- observable head/arity errors are semantic and attach to the typed type node.

### Metric

- missing/unknown kind is a typed `MetricKind` recovery node and exact diagnostic;
- missing colon/type/body uses typed zero-width/missing nodes;
- duplicate/out-of-order members and duplicate labels retain all typed nodes with related first ranges;
- non-string unit remains a typed expression/error child and `syntax.metric.unit_not_string`;
- empty/non-sequence/missing buckets retain typed expression/list/missing children and exact diagnostics;
- unknown member uses `ErrorDeclarationMember`.

### Layer

- missing/unknown kind uses `LayerKindNode` recovery and exact diagnostic;
- unknown/duplicate/conflicting member retains `LayerMember` or `ErrorDeclarationMember` with first-related evidence;
- missing assignment/value emits `MissingMemberValue` at the exact insertion point;
- closed policy word errors remain `LayerPolicyValue` recovery, never strings;
- absolute wrong-family references use `WrongFamilyReference`; relative/imported wrong-family results are semantic resolver errors;
- project parent cycle/content conflict errors attach to the exact member reference(s).

## 4. Poison and executability

A declaration is `Poisoned` when any error-severity diagnostic is structurally owned by its item, header, body, parameter, member, or typed descendant. Poison is a typed parse/lowering fact, not a later source scan.

- Attached AST remains available to formatter/LSP/diagnostics.
- HIR lowering allocates the source slot and an error item that preserves exact syntax identity and diagnostics.
- No retained family payload, project symbol, callable/channel facet, registry entry, runtime-plan node, or bundle product is published from the poisoned declaration.
- Upstream poison suppresses duplicate downstream diagnostics; consumers retain the typed poison cause.

Warnings/lints do not poison unless the common policy explicitly classifies them as errors. Optional omission and defaulting never count as recovery.

## 5. Synchronization

- Bodyless Action/Signal synchronization ends at semicolon, newline, or EOF. Family-inappropriate syntax on that logical line is consumed into the declaration's recovery node only.
- Braced declarations track common delimiter depth. On an unclosed body, the parser stops before a token sequence that is independently classifiable as a top-level item at depth zero and the top-level declaration boundary. Its outer prefixes remain available to the next item.
- A malformed nested expression emits its own missing delimiters, then the enclosing body emits its missing close; both stop before the following declaration.
- Unknown body members stop at the member logical terminator or section close, whichever comes first.
- A malformed/removed top-level item is one ordinary `ErrorItem`; its recovery cannot absorb the following valid declaration.
- Recovery uses token kinds, delimiter depth, indentation/logical-line structure, and the current top-level classifier. It never searches source text for a spelling.

## 6. Inclusive syntax limits

| `SyntaxLimit` | Maximum |
|---|---:|
| PrefixDepth | 64 |
| ContractClauses | 64 |
| FixedParameters | 256 |
| DeclarationMembers | 1,024 |
| ActivityPorts | 256 |
| MetricLabels | 64 |
| MetricBuckets | 1,024 |
| ViewExports | 256 |
| LayerMembers | 64 |
| TopLevelItems | 16,384 |
| Statements | 65,536 |
| Expressions | 262,144 |
| TypeNodes | 131,072 |
| PatternNodes | 131,072 |
| IdentityBearingNodes | 1,048,576 |
| Diagnostics | 1,024 |

Application:

- View and Action share `FixedParameters`.
- Activity input plus output share one `ActivityPorts` counter. Requires plus ensures share one `ContractClauses` counter.
- All body entries consume `DeclarationMembers` in addition to a narrower family counter where applicable.
- Exact maximum succeeds. The first attempted allocation beyond the maximum is fatal for the full syntax transaction.

## 7. Transaction failure

A recoverable syntax error commits the lossless tree, missing nodes, and diagnostics. A fatal budget, event/tree allocation, checked-range, identity-space, or attachment failure commits none of:

- source generation advancement;
- green tree or typed index;
- syntax node IDs;
- diagnostics;
- attachment tables; or
- cache entries.

The next successful parse from the unchanged pre-failure snapshot receives the same generation and IDs it would have received if the failed transaction had never run.
