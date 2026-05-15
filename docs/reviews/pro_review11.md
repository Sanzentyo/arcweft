# Arcweft: canonical brace blocks, flat fence sugar, generic VM `thread`, line registry, marks, cleanup, and drop

## 0. Goal

This document defines the block syntax policy and line/thread execution model for Arcweft.

It integrates:

- canonical `{ ... }` blocks,
- indentation sugar with `:`,
- flat same-indentation fence sugar with `=== ... ===`,
- generic VM-level `thread`,
- dialogue `with { ... }` line plans,
- line-level `finally { ... }`,
- scoped `defer { ... }`,
- `'line.*` lifetime-registry access,
- `[mark .name]` inline dialogue marks,
- cleanup profiles,
- `drop`, `on_drop`, `expose`, and typestate-like handle states,
- implementation changes required in `Sanzentyo/arcweft`.

This document is normative for the new design. No backwards-compatibility with unfinished local `[hook]`, `hook a1:`, `using`, `state 'line ...`, or `scope ... until ...` drafts is required.

---

## 1. Block syntax policy

Arcweft supports three surface block styles:

```text
1. Brace canonical form
2. Indentation sugar
3. Flat fence sugar
```

Only the brace form is canonical.

### 1.1 Brace canonical form

Canonical:

```awft
alice(.smile, focus = .soft, cleanup = .fast_skip)[
    聞いて。[mark .release_focus]
    こっちをみて[r]
]
with {
    on .release_focus {
        'line.focus |> drop
    }

    finally {
        debug_log("line finalized")
    }
}
```

The brace form is canonical for:

```text
- grammar specification
- HIR/debug dumps
- generated source
- exact lowering docs
- formatter canonical mode
- VM trace explanations
```

### 1.2 Indentation sugar

Indentation sugar is allowed for authoring:

```awft
alice(.smile, focus = .soft, cleanup = .fast_skip):
    聞いて。[mark .release_focus]
    こっちをみて[r]
with:
    on .release_focus:
        'line.focus |> drop

    finally:
        debug_log("line finalized")
```

The indentation form must lower to the same AST/HIR as the brace form.

### 1.3 Flat fence sugar

Flat fence sugar is for scripts where indentation should not change.

```awft
=== line alice(.smile, focus = .soft, cleanup = .fast_skip) ===
聞いて。[mark .release_focus]
こっちをみて[r]

=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===

=== finally ===
debug_log("line finalized")
=== /finally ===
=== /with ===
=== /line ===
```

Flat fence sugar is also allowed as an attached `with` block after a normal line:

```awft
alice(.smile, focus = .soft)[
    聞いて。[mark .release_focus]
    こっちをみて[r]
]
=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
```

Flat fence sugar exists to support:

```text
- KAG-like line-oriented authoring
- migration from older visual-novel scripts
- generated scripts that should not care about indentation
- editing environments where indentation is fragile
- long dialogue files where section fences are visually clearer
```

It is not canonical. Formatter canonical mode expands it to braces.

---

## 2. Why choose `=== ... ===`

KAG/Kirikiri style authoring is line-oriented and tag-heavy. KAG is known as a Kirikiri Adventure Game system with BBCode/HTML-like tag commands, and `.ks` scripts are plain script files. Arcweft should not copy KAG syntax wholesale, but the idea of line-friendly script controls is useful.

Ren'Py is closer to indentation-sensitive screenplay syntax. NScripter is closer to flat BASIC-like command syntax. Arcweft can support both readability modes:

```text
- `{ ... }` canonical for compiler/formatter/HIR.
- `:` indentation sugar for Ren'Py-like authoring.
- `=== ... ===` fence sugar for KAG/NScripter-like flat authoring.
```

`===` is chosen because:

```text
- it is visually strong in long scenario files;
- it is unlikely to be confused with entity refs, tags, labels, or expressions;
- it can be restricted to beginning-of-line fences;
- it supports explicit close fences;
- it can be parsed without indentation sensitivity.
```

Rejected alternatives:

```text
@with / @on
  Conflicts visually with @entity references.

[with] ... [/with]
  Conflicts with dialogue control tags.

*with
  Too close to label/comment conventions in older script engines.

--- with ---
  Too common as prose separator and Markdown-like horizontal rule.

<<with>>
  More visually noisy and can conflict with generic/angle parsing.

=== with ===
  Best balance for Arcweft flat authoring sugar.
```

---

## 3. Flat fence grammar

### 3.1 Basic grammar

```text
FlatOpen  = BOL "===" WS FlatHead WS "===" EOL
FlatClose = BOL "===" WS "/" FlatName WS "===" EOL
```

Examples:

```awft
=== with ===
...
=== /with ===

=== on .release_focus ===
...
=== /on ===

=== thread motion ===
...
=== /thread ===
```

`BOL` means beginning of physical line after optional spaces. Recommended parser rule: allow leading whitespace but formatter should remove it in flat mode.

### 3.2 Flat heads

Allowed flat heads:

```text
line <speaker-or-callee>
with
init
thread [name]
defer
on <trigger>
finally
at(<expr>)
scope [name]
```

Future heads may include:

```text
choice
option
select
```

but this document only requires line-plan related heads.

### 3.3 Explicit closing required

Always require close fences.

```awft
=== on .release_focus ===
'line.focus |> drop
=== /on ===
```

Do not infer close from next header.

Reason:

```text
- no indentation means inferred nesting is too error-prone;
- explicit close fences give better diagnostics;
- generated scripts are easier to patch;
- unmatched blocks are easy to recover.
```

### 3.4 Close names

The close name must match the open block kind.

```awft
=== on .x ===
...
=== /thread ===
```

Diagnostic:

```text
error: flat block close mismatch; opened `on`, found `/thread`
```

For a named thread:

```awft
=== thread motion ===
...
=== /thread ===
```

The close name is the block kind, not the thread name.

### 3.5 Line block

A flat `line` block wraps dialogue content and optional with block.

```awft
=== line alice(.smile, focus = .soft) ===
聞いて。[mark .release_focus]
こっちをみて[r]

=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
=== /line ===
```

Lowering:

```awft
alice(.smile, focus = .soft)[
    聞いて。[mark .release_focus]
    こっちをみて[r]
]
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

### 3.6 Attached flat `with`

A flat `with` block can attach to the immediately preceding dialogue line/content call.

```awft
alice(.smile)[
    聞いて。[mark .release_focus]
]
=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
```

If no attachable line exists, error:

```text
error: flat `with` block has no preceding dialogue line
```

### 3.7 Text lines beginning with `===`

Inside a flat `line` block, a physical line that begins with `===` is interpreted as a fence.

To write literal text beginning with `===`, escape it:

```awft
\=== これは本文です
```

or use raw text:

```awft
[raw]
=== これは本文です
[/raw]
```

### 3.8 Flat blocks outside dialogue

Generic flow-level threads can use flat fences too:

```awft
=== thread preload_next ===
asset.preload(@asset.bg.school_classroom)
alice.preload(look = .smile, voices = auto)
=== /thread ===
```

Canonical lowering:

```awft
thread preload_next {
    asset.preload(@asset.bg.school_classroom)
    alice.preload(look = .smile, voices = auto)
}
```

---

## 3A. Flat fence close rules, precisely

Flat fences are not indentation-based. Every opening fence must be closed explicitly.

### 3A.1 Close token

The close token is:

```text
=== /<kind> ===
```

Examples:

```awft
=== with ===
...
=== /with ===

=== on .release_focus ===
...
=== /on ===

=== thread motion ===
...
=== /thread ===

=== finally ===
...
=== /finally ===

=== defer ===
...
=== /defer ===

=== line alice(.smile) ===
...
=== /line ===
```

The closing name is the block kind, not the full head. For example, this is correct:

```awft
=== thread motion ===
...
=== /thread ===
```

Do not write:

```awft
=== /thread motion ===
```

### 3A.2 Nesting

Flat fences must be properly nested.

```awft
=== line alice(.smile) ===
聞いて。[mark .release_focus]

=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
=== /line ===
```

Invalid:

```awft
=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /with ===
=== /on ===
```

Diagnostic:

```text
error: flat fence close mismatch
opened: on
found: /with
help: close the `on` block first with `=== /on ===`
```

### 3A.3 Single-line flat blocks are not allowed for structured blocks

Do not use:

```awft
=== on .release_focus === 'line.focus |> drop === /on ===
```

Flat fence blocks are line-oriented. The body starts on the next physical line.

Use:

```awft
=== on .release_focus ===
'line.focus |> drop
=== /on ===
```

### 3A.4 Explicit close is required

Do not infer the end of one flat block from the next opening fence.

Invalid:

```awft
=== on .a ===
foo()
=== on .b ===
bar()
=== /on ===
```

This is ambiguous. The parser should report:

```text
error: flat block `on .a` must be closed before opening another sibling `on`
help: insert `=== /on ===` before `=== on .b ===`
```

Correct:

```awft
=== on .a ===
foo()
=== /on ===
=== on .b ===
bar()
=== /on ===
```

### 3A.5 Sibling shorthand is intentionally not supported

This document does not support KAG-like implicit sibling transitions such as:

```awft
=== on .a ===
foo()
=== on .b ===
bar()
```

because it makes error recovery and generated patches fragile. Always close explicitly.

### 3A.6 EOF recovery

If EOF is reached while flat blocks are open, the parser should close them for recovery but emit diagnostics for each missing close.

Example:

```awft
=== with ===
=== on .release_focus ===
'line.focus |> drop
```

Diagnostics:

```text
error: missing close fence `=== /on ===`
error: missing close fence `=== /with ===`
```

The recovered tree may still be used by LSP, formatter, and diagnostics.

### 3A.7 Optional auto-close for top-level file sections is rejected

Do not auto-close at `flow`, `fragment`, `mod`, or top-level item boundaries. If flat fences are used, the source must close them explicitly.

Reason:

```text
- explicit close makes copy/paste safe;
- LSP edits are local;
- formatter can round-trip style;
- diagnostics can point to the exact unclosed block.
```

### 3A.8 Literal `===`

Inside a flat `line` block, a physical line beginning with `===` is a fence.

Literal text must be escaped:

```awft
\=== これは本文です
```

or written in a raw span:

```awft
[raw]
=== これは本文です
[/raw]
```

### 3A.9 Formatter behavior

Formatter modes:

```text
fmt --canonical
  converts all flat fences to brace canonical form.

fmt --flat-script
  preserves flat fences and inserts missing explicit close fences only when recovery is unambiguous.

fmt --scenario-style
  may convert flat line blocks to indentation sugar, but only when indentation can be made unambiguous.
```


---

## 3B. Block style policy, lint, and rewriting

Arcweft should support all three block styles as source styles:

```text
brace       Canonical `{ ... }`
indent      Authoring sugar using `:`
flat        Authoring sugar using `=== ... ===`
```

The current implementation already stores source block style for line plans with `BlockStyle::Brace` and `BlockStyle::Indent`. Add `BlockStyle::Flat` for flat fence source preservation.

```rust
pub enum BlockStyle {
    Brace,
    Indent,
    Flat,
}
```

This style is formatting metadata. It must not change HIR or VM semantics.

### 3B.1 Project style configuration

Add a style policy file section, for example:

```toml
[fmt.blocks]
default = "brace"            # brace | indent | flat | preserve
line_plan = "indent"         # brace | indent | flat | preserve
dialogue_line = "indent"     # brace | indent | flat | preserve
thread = "brace"             # brace | indent | flat | preserve
defer = "brace"              # brace | indent | flat | preserve
on_handler = "indent"        # brace | indent | flat | preserve

[lint.blocks]
noncanonical = "allow"       # allow | warn | error
mixed_styles = "warn"        # allow | warn | error
flat_without_close = "error" # always error
prefer = "project"           # project | canonical | preserve
```

Recommended defaults:

```toml
[fmt.blocks]
default = "preserve"
line_plan = "indent"
dialogue_line = "indent"
thread = "brace"
defer = "brace"
on_handler = "indent"

[lint.blocks]
noncanonical = "allow"
mixed_styles = "warn"
flat_without_close = "error"
prefer = "project"
```

### 3B.2 Style-specific lints

Potential lint codes:

```text
BLOCK_STYLE_NONCANONICAL
  Source uses indent/flat when project requires brace.

BLOCK_STYLE_MIXED
  One line plan mixes brace, indent, and flat styles in a way the project forbids.

BLOCK_STYLE_FLAT_MISSING_CLOSE
  A flat block lacks explicit close.

BLOCK_STYLE_FLAT_CLOSE_MISMATCH
  A flat close fence closes the wrong block kind.

BLOCK_STYLE_FLAT_LITERAL_NEEDS_ESCAPE
  Dialogue text starts with `===` inside a flat line block.

BLOCK_STYLE_PREFER_INDENT
  Project prefers indentation sugar for dialogue line plans.

BLOCK_STYLE_PREFER_BRACE
  Project prefers canonical brace form.

BLOCK_STYLE_PREFER_FLAT
  Project prefers flat fence form for selected files.
```

### 3B.3 LSP code actions

The LSP should expose:

```text
- Convert block to brace style
- Convert block to indentation style
- Convert block to flat fence style
- Convert file to project block style
- Insert missing flat close fence
- Escape literal `===` in dialogue text
- Normalize local line plan style
```

Example:

```awft
alice(.smile):
    聞いて。[mark .release_focus]
with:
    on .release_focus:
        'line.focus |> drop
```

Code action: Convert to brace style:

```awft
alice(.smile)[
    聞いて。[mark .release_focus]
]
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

Code action: Convert to flat style:

```awft
=== line alice(.smile) ===
聞いて。[mark .release_focus]

=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
=== /line ===
```

### 3B.4 CLI commands

Add commands such as:

```bash
arcw fmt --block-style brace
arcw fmt --block-style indent
arcw fmt --block-style flat
arcw fmt --block-style preserve

arcw lint --rule block-style
arcw fix --block-style project
arcw fix --insert-flat-closes
```

### 3B.5 Style selection by file

Long scenario files may want flat style, while normal code files may want brace or indent.

```toml
[fmt.files]
"routes/**/*.awft" = { dialogue_line = "indent", line_plan = "indent" }
"script_flat/**/*.awft" = { dialogue_line = "flat", line_plan = "flat" }
"lib/**/*.awft" = { default = "brace" }
```

### 3B.6 Semantic equivalence requirement

These three forms must lower to the same HIR:

Brace:

```awft
alice(.smile)[
    聞いて。[mark .release_focus]
]
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

Indent:

```awft
alice(.smile):
    聞いて。[mark .release_focus]
with:
    on .release_focus:
        'line.focus |> drop
```

Flat:

```awft
=== line alice(.smile) ===
聞いて。[mark .release_focus]

=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
=== /line ===
```

The parser may preserve `BlockStyle`, but HIR and VM semantics must not branch on it.

### 3B.7 Warning vs rewrite policy

Do not hardcode style preference in the parser.

Parser responsibility:

```text
- accept valid supported styles;
- recover from malformed style;
- store source style when useful;
- emit syntax errors for invalid flat close/missing close.
```

Formatter responsibility:

```text
- rewrite to configured style;
- preserve style if configured;
- insert explicit flat closes only when safe.
```

Lint responsibility:

```text
- warn/error on non-project style;
- warn/error on mixed styles if configured;
- offer code actions.
```


---

## 3C. Unnamed scope in flat style

Unnamed scopes should be supported in flat style. Do not ban them.

Current Arcweft already represents scopes with an optional name, and the parser recognizes both named `scope name { ... }` and unnamed/bare scope forms. Flat style must not remove that expressiveness.

### 3C.1 Canonical forms

In statement/block-item position, an unnamed scope may be written as either a bare block or an explicit `scope` block:

```awft
{
    let tmp = compute()
    use_tmp(tmp)
}
```

```awft
scope {
    let tmp = compute()
    use_tmp(tmp)
}
```

For generated canonical output in statement/block-item position, prefer:

```awft
scope {
    ...
}
```

over a bare `{ ... }`, because `scope { ... }` has an explicit head and maps more directly to flat style. This equivalence does not apply globally to expression position; see section 3E.

### 3C.2 Flat unnamed scope

Flat unnamed scope:

```awft
=== scope ===
let tmp = compute()
use_tmp(tmp)
=== /scope ===
```

Canonical lowering:

```awft
scope {
    let tmp = compute()
    use_tmp(tmp)
}
```

### 3C.3 Flat named scope

Flat named scope:

```awft
=== scope rain ===
alice:
    雨、強くなってきたね。[p]
=== /scope ===
```

Canonical lowering:

```awft
scope rain {
    alice:
        雨、強くなってきたね。[p]
}
```

### 3C.4 Why not forbid unnamed scope?

Do not forbid it, because unnamed scope is useful for:

```text
- limiting temporary variables;
- grouping statements without creating a public ID namespace segment;
- creating a local defer/drop boundary;
- preserving current brace-style `{ ... }` semantics in flat files;
- formatter round-tripping from brace to flat without inventing names.
```

### 3C.5 Meaning of unnamed scope

Unnamed scope is a lexical/runtime block boundary.

It does:

```text
- introduce a lexical scope for locals;
- run local defers at the end of the scope;
- bound local MustDrop values;
- group statements for parsing and formatting.
```

It does not:

```text
- create a named ID namespace segment;
- create a new lifetime registry like `'line`;
- change dialogue line cleanup policy;
- create a thread parent distinct from the current runtime scope unless the VM chooses to model every block as a child lexical scope.
```

### 3C.6 Defer inside unnamed flat scope

```awft
=== scope ===
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))

defer {
    focus |> drop
}
=== /scope ===
```

The `defer` runs when the unnamed scope exits.

Flat form with flat defer:

```awft
=== scope ===
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))

=== defer ===
focus |> drop
=== /defer ===
=== /scope ===
```

### 3C.7 Empty head is not allowed

Do not use an empty flat block head.

Invalid:

```awft
=== ===
let tmp = compute()
=== / ===
```

Use:

```awft
=== scope ===
let tmp = compute()
=== /scope ===
```

### 3C.8 `block` alias is rejected

Do not add `=== block ===` as an alias.

Reason:

```text
- `scope` already exists in Arcweft;
- aliases increase formatter and lint complexity;
- `scope` communicates lexical scope and ID-scope behavior;
- `block` is too generic.
```

### 3C.9 Style lint for unnamed scopes

Project config can warn/error on unnamed scopes in flat files if a team wants named scopes only.

```toml
[lint.blocks]
unnamed_scope = "allow"        # allow | warn | error
unnamed_scope_flat = "allow"   # allow | warn | error
```

Recommended default:

```toml
[lint.blocks]
unnamed_scope = "allow"
unnamed_scope_flat = "allow"
```

If `unnamed_scope_flat = "warn"`:

```awft
=== scope ===
...
=== /scope ===
```

Diagnostic:

```text
warning: unnamed flat scope is allowed but project prefers named scopes
help: write `=== scope name ===` if this block should appear in traces or ID paths
```

### 3C.10 Formatter rules

Brace to flat:

```awft
scope {
    foo()
}
```

becomes:

```awft
=== scope ===
foo()
=== /scope ===
```

Named brace to flat:

```awft
scope rain {
    foo()
}
```

becomes:

```awft
=== scope rain ===
foo()
=== /scope ===
```

Flat to canonical brace:

```awft
=== scope ===
foo()
=== /scope ===
```

becomes:

```awft
scope {
    foo()
}
```


---

## 3D. Unnamed scope in indentation style

Unnamed scopes should also be supported in indentation style, but the spelling must be explicit.

Use:

```awft
scope:
    let tmp = compute()
    use_tmp(tmp)
```

Do not use a naked colon block:

```awft
:
    let tmp = compute()
```

Naked `:` is forbidden.

Reason:

```text
- it has no syntactic head;
- it is hard to recover from parser errors;
- it conflicts visually with speaker lines such as `alice:`;
- it is bad for formatter/LSP code actions;
- it makes copy/paste patches fragile.
```

### 3D.1 Canonical lowering

Indent unnamed scope:

```awft
scope:
    let tmp = compute()
    use_tmp(tmp)
```

lowers to canonical brace form:

```awft
scope {
    let tmp = compute()
    use_tmp(tmp)
}
```

Named indentation scope:

```awft
scope rain:
    alice:
        雨、強くなってきたね。[p]
```

lowers to:

```awft
scope rain {
    alice:
        雨、強くなってきたね。[p]
}
```

### 3D.2 Relationship to brace and flat forms

These three are equivalent:

Brace canonical:

```awft
scope {
    foo()
}
```

Indent sugar:

```awft
scope:
    foo()
```

Flat sugar:

```awft
=== scope ===
foo()
=== /scope ===
```

Named forms are also equivalent:

```awft
scope rain {
    foo()
}
```

```awft
scope rain:
    foo()
```

```awft
=== scope rain ===
foo()
=== /scope ===
```

### 3D.3 Where `scope:` is allowed

`scope:` is allowed as a block item in:

```text
- flow body
- line plan body
- thread body
- on-handler body
- defer body
- ordinary statement-block contexts where a scoped statement block is valid
```

It is not recommended as expression syntax.

For value-producing scope expressions, use canonical braces:

```awft
let value = scope {
    let x = compute()
    x + 1
}
```

Do not use:

```awft
let value = scope:
    let x = compute()
    x + 1
```

This restriction keeps assignment/expression parsing simple.

### 3D.4 Scope semantics

`scope:` is an unnamed lexical/runtime scope.

It does:

```text
- introduce a lexical scope for locals;
- run local `defer` statements at scope exit;
- bound local MustDrop values;
- group statements for parsing and formatting.
```

It does not:

```text
- create a named ID namespace segment;
- create a new lifetime registry like `'line`;
- change line cleanup policy;
- spawn a VM thread;
- change dialogue marker behavior.
```

### 3D.5 Defer inside indentation scope

```awft
scope:
    let focus =
        stage.focus(target = alice)
        |> on_drop(release(120ms))

    defer:
        focus |> drop
```

Canonical form:

```awft
scope {
    let focus =
        stage.focus(target = alice)
        |> on_drop(release(120ms))

    defer {
        focus |> drop
    }
}
```

### 3D.6 Lint configuration

Add indentation-specific unnamed-scope lint knobs:

```toml
[lint.blocks]
unnamed_scope = "allow"          # allow | warn | error
unnamed_scope_indent = "allow"   # allow | warn | error
unnamed_scope_flat = "allow"     # allow | warn | error
naked_colon_scope = "error"      # always error in practice
```

Recommended default:

```toml
[lint.blocks]
unnamed_scope = "allow"
unnamed_scope_indent = "allow"
unnamed_scope_flat = "allow"
naked_colon_scope = "error"
```

If a project prefers named scopes in author-written source:

```toml
[lint.blocks]
unnamed_scope_indent = "warn"
```

Diagnostic:

```text
warning: unnamed indentation scope is allowed but project prefers named scopes
help: write `scope name:` if this block should appear in traces or ID paths
```

### 3D.7 Formatter rules

Brace to indent:

```awft
scope {
    foo()
}
```

becomes:

```awft
scope:
    foo()
```

Indent to canonical brace:

```awft
scope:
    foo()
```

becomes:

```awft
scope {
    foo()
}
```

Indent to flat:

```awft
scope:
    foo()
```

becomes:

```awft
=== scope ===
foo()
=== /scope ===
```

### 3D.8 Edge cases

#### `scope :`

Do not allow a space before the colon in the block head.

Invalid:

```awft
scope :
    foo()
```

Use:

```awft
scope:
    foo()
```

#### `scope name :`

Also invalid for consistency.

Use:

```awft
scope name:
    foo()
```

#### `scope:` after dialogue text

This is parsed as a scope item, not dialogue text, only when it appears in a flow/plan statement position.

Inside dialogue text:

```awft
alice:
    scope:
    これは本文です。[p]
```

`scope:` is text because it is inside the dialogue body, unless the dialogue body has ended by indentation or bracket close.

#### Empty scope

Allowed but lintable.

```awft
scope:
```

Diagnostic if configured:

```text
warning: empty scope has no effect
```


---

## 3E. Are `{ ... }` and `scope { ... }` equivalent?

They are equivalent only in **statement/block-item position**.

Do not say they are globally identical. The language must distinguish these cases:

```text
statement item position:
  { ... }      == scope { ... }      // bare scope sugar

expression position:
  { ... }      == block expression
  scope { ... } == explicit scope expression
```

### 3E.1 Statement item position

In flow body, line-plan body, thread body, `on` body, and `defer` body, a bare block item is an unnamed scope.

Brace style:

```awft
{
    let tmp = compute()
    use_tmp(tmp)
}
```

is sugar for:

```awft
scope {
    let tmp = compute()
    use_tmp(tmp)
}
```

Indent style:

```awft
scope:
    let tmp = compute()
    use_tmp(tmp)
```

Flat style:

```awft
=== scope ===
let tmp = compute()
use_tmp(tmp)
=== /scope ===
```

All three represent an unnamed lexical/runtime scope item.

### 3E.2 Expression position

In expression position, `{ ... }` is a block expression.

```awft
let value = {
    let x = compute()
    x + 1
}
```

This is not automatically rewritten to `scope { ... }`.

If an explicit scope expression is desired, write:

```awft
let value = scope {
    let x = compute()
    x + 1
}
```

For an unnamed scope expression, the value semantics are usually the same as a block expression, but the syntax node is different and tooling may preserve it.

```text
{ ... }
  Expr::Block

scope { ... }
  Expr::ScopeBlock or Stmt::LetScope / ScopeExprBlock in current parser-facing structures
```

### 3E.3 Named scope is never equivalent to bare `{ ... }`

```awft
scope rain {
    ...
}
```

is not equivalent to:

```awft
{
    ...
}
```

because a named scope may:

```text
- add an ID namespace segment;
- appear in traces/debug output;
- affect relative ID generation;
- provide a stable grouping name for tools.
```

This aligns with the current ID/scoping design where named scopes are lexical scopes and ID namespaces.

### 3E.4 Formatter policy

Formatter should use these rules:

```text
statement item:
  bare `{ ... }` may format to `scope { ... }` if explicit scope style is enabled.

expression:
  `{ ... }` stays `{ ... }` unless the source explicitly used `scope { ... }`
  or project policy requests explicit scope expressions.

named scope:
  always preserve `scope name`.
```

Example config:

```toml
[fmt.blocks]
bare_scope_statement = "scope_keyword"  # preserve | bare | scope_keyword
scope_expression = "preserve"           # preserve | block_when_unnamed | explicit_scope
```

Recommended defaults:

```toml
[fmt.blocks]
bare_scope_statement = "scope_keyword"
scope_expression = "preserve"
```

### 3E.5 Lints

```toml
[lint.blocks]
bare_scope_statement = "allow"       # allow | warn | error
explicit_unnamed_scope_expr = "allow" # allow | warn | error
```

Possible lints:

```text
BARE_SCOPE_STATEMENT
  statement-position `{ ... }` could be written `scope { ... }` for clarity.

UNNEEDED_SCOPE_EXPR
  `scope { ... }` in expression position has no name and no tool-visible reason;
  `{ ... }` would be equivalent for value semantics.
```

### 3E.6 Why this distinction matters

If `{ ... }` and `scope { ... }` are treated as globally identical, several edge cases become ambiguous:

```awft
let value = {
    foo()
}
```

Is this a value-producing block expression or a scope item? It must be an expression.

```awft
let value = scope {
    foo()
}
```

This is an explicit scope expression.

```awft
{
    foo()
}
```

As a flow item, this is an unnamed scope item.

Therefore, the equivalence is context-sensitive and should be documented as such.

---

## 4. Canonical block constructs

### 4.1 Canonical forms

```text
with { ... }
init { ... }
thread name { ... }
defer { ... }
on .mark { ... }
on line.end { ... }
finally { ... }
at(0.42s) { ... }
```

### 4.2 Indentation sugar

```text
with:
init:
thread name:
defer:
on .mark:
on line.end:
finally:
at(0.42s):
scope:
scope name:
```

### 4.3 Flat fence sugar

```text
=== with === ... === /with ===
=== init === ... === /init ===
=== thread name === ... === /thread ===
=== defer === ... === /defer ===
=== on .mark === ... === /on ===
=== finally === ... === /finally ===
=== at(0.42s) === ... === /at ===
```

### 4.4 Formatter rules

```text
fmt --canonical
  expands indentation and flat fence sugar to brace form.

fmt --scenario-style
  may preserve or emit `:` sugar for dialogue-heavy files.

fmt --flat-script
  may emit `=== ... ===` fences for long flat scenario scripts.

HIR / VM / diagnostics
  should not distinguish brace, colon, or flat-fence semantics.
```

---

## 5. Main execution decisions

### 5.1 `finally` belongs to the line

Use `finally { ... }` only as a line-final block inside a dialogue line plan.

```awft
alice(.smile, focus = .soft)[
    聞いて。[mark .release_focus]
]
with {
    on .release_focus {
        'line.focus |> drop
    }

    finally {
        debug_log("line finished or was interrupted")
    }
}
```

`finally` runs exactly once when the line ends, regardless of how it ends:

```text
- normal completion
- skip
- cancel
- goto
- return
- error propagation
- parent flow cancellation
```

`finally` runs after child threads have been cancelled/joined and their `defer` stacks have run, but before remaining automatic line-registry drops are finalized.

### 5.2 `thread` uses `defer`, not thread-local `finally`

Thread-local cancellation safety is written with `defer`.

```awft
thread motion {
    let lease =
        alice.stage.lease()
        |> on_drop(release)

    defer {
        lease |> drop
    }

    alice.stage.apply(.motion.nod)
    wait mark .release_focus
    alice.stage.apply(.stage.expr.smile)
}
```

Do not implement:

```awft
thread motion {
    finally {
        ...
    }
}
```

Thread-local finalization is `defer`; line-level finalization is `finally`.

### 5.3 Why choose `defer`

```text
finally
  Good for one enclosing lifecycle; confusing if repeated per thread and line.

cleanup
  Conflicts with line cleanup policy.

guard
  Too type-system-like and less common as source syntax.

using
  Rejected because Arcweft already has `use`.

defer
  Existing Arcweft AST already has Stmt::Defer(Expr).
  Familiar from Go/Zig/Swift-like cleanup semantics.
  Naturally stack-ordered.
  Works inside threads, init, on-handlers, and flow scopes.
```

---

## 6. Generic `thread`

`thread` is a generic VM-level structured-concurrency construct, not a `with:`-only feature.

### 6.1 Statement form

```awft
thread preload_next {
    asset.preload(@asset.bg.school_classroom)
    alice.preload(look = .smile, voices = auto)
}
```

Indentation sugar:

```awft
thread preload_next:
    asset.preload(@asset.bg.school_classroom)
    alice.preload(look = .smile, voices = auto)
```

Flat fence sugar:

```awft
=== thread preload_next ===
asset.preload(@asset.bg.school_classroom)
alice.preload(look = .smile, voices = auto)
=== /thread ===
```

### 6.2 Expression form

For explicit joining or result capture:

```awft
let t = thread compute_score {
    route_score(state)
}

let score = await t
```

Recommended semantic type:

```awft
ThreadHandle<T, E = ThreadError>
```

or:

```awft
Need<T, ThreadError>
```

The final choice should align with `Need` and `await with`.

### 6.3 Parent scope

```text
thread in flow body
  parent = current flow fiber / lexical flow scope

thread in named scope
  parent = that lexical scope

thread inside dialogue with
  parent = current dialogue line lifetime `'line`

thread inside on .mark
  parent = current dialogue line, spawned at event time

thread in task fn
  parent = that task function call

thread in pure fn
  disallowed unless function is explicitly effectful
```

### 6.4 Detached form

Detached thread must be explicit.

```awft
thread detached analytics {
    telemetry.record(route_id)
}
```

Detached thread restrictions:

```text
- cannot capture non-static borrowed values;
- cannot capture line-owned handles;
- cannot capture MustDrop values unless explicitly detached/moved;
- requires a capability such as effects { thread.detach }.
```

---

## 7. Relationship to current `spawn`

The current AST has `Stmt::Spawn(Expr)`. This should be clarified.

Recommended direction:

```text
thread
  structured VM child task with parent lifetime and guaranteed finalization.

spawn
  either deprecated, or reserved for explicit detached/unstructured task requests.
```

Migration:

```awft
spawn expr
```

becomes either:

```awft
thread {
    expr
}
```

or:

```awft
thread detached {
    expr
}
```

depending on intended lifetime.

---

## 8. `with` as line-scoped specialization

Inside dialogue:

```awft
alice(.smile, voice = auto, focus = .soft)[
    聞いて。[mark .release_focus]
]
with {
    thread motion {
        alice.stage.apply(.motion.nod)
    }

    on .release_focus {
        'line.focus |> drop
    }
}
```

The `thread motion { ... }` is a normal `thread`; its parent scope is `'line`.

Line-scoped defaults:

```text
parent scope     = current dialogue line
cleanup policy   = line cleanup profile
registry access  = `'line.*`
start time       = after init and at line start unless spawned from event handler
finalization     = before line scope closes
```

---

## 9. `init`

`init { ... }` is line-plan synchronous setup, not a general flow construct.

It runs before:

```text
- dialogue content reveal
- line voice playback
- user line threads
- marker traversal
- visible line-start effects
```

Common effects should not require `init`.

Use line options:

```awft
alice(.smile, focus = .soft)[
    聞いて。[p]
]
```

Use `init` for conditional or advanced setup:

```awft
alice(.smile, cleanup = .fast_skip)[
    聞いて。[mark .release_focus]
]
with {
    init {
        if state.flags.focus_alice {
            'line.focus <-
                stage.focus(target = alice, others = .blur(8px) & .dim(35%))
                |> on_drop(release(120ms))
        }
    }

    on .release_focus {
        if let Some(f) = 'line.focus? {
            f |> drop
        }
    }
}
```

---

## 10. VM execution model

Arcweft core is Sans I/O. The VM computes state and effect requests; it does not directly perform GPU/audio/filesystem/OS effects.

A line is a structured VM task group:

```text
LineTask
├─ InitTask
├─ ContentTask
├─ VoiceTask
├─ UserThreadTask[]
├─ EventHandlerTask[]
├─ TimedCueTask[]
└─ CleanupTask
```

A flow can also have structured task groups:

```text
FlowFiber
├─ MainFlowTask
├─ ScopedThreadTask[]
├─ AwaitContinuation[]
└─ CleanupTask
```

Required runtime properties:

```text
- deterministic logical clock
- stable ordering of simultaneous effects
- no direct side effects in core
- effect requests only
- replayable FrameInput / FrameOutput
- child tasks cannot outlive the parent scope unless explicitly detached
```

Effect order for the same logical tick:

```text
1. logical time
2. parent scope order
3. event source order
4. thread declaration order
5. per-task sequence number
```

---

## 11. Structured concurrency

`thread` follows Rust-like scoped concurrency.

```text
- A scope owns all non-detached threads spawned inside it.
- A child thread cannot outlive its parent scope by accident.
- Live child threads are joined/cancelled/finalized at parent cleanup.
- Captured values must be safe to send/share across VM tasks.
- Unique MustDrop handles cannot be captured by multiple live threads.
- `defer` discharges moved MustDrop values in a thread or scope.
```

### 11.1 Move into a thread

```awft
with {
    init {
        let lease =
            alice.stage.lease()
            |> on_drop(release)
    }

    thread motion {
        let lease = move lease

        defer {
            lease |> drop
        }

        alice.stage.apply(.motion.nod)
    }
}
```

After `move lease`, the outer `lease` is unavailable.

### 11.2 Shared handle

```awft
with {
    init {
        let focus =
            stage.focus(target = alice)
            |> on_drop(release(120ms))
            |> share
    }

    thread a {
        focus.request(.dim(30%))
    }

    thread b {
        focus.request(.blur(8px))
    }
}
```

A unique handle cannot be implicitly shared.

### 11.3 Capture edge case

Disallowed by default:

```awft
with {
    init {
        let focus =
            stage.focus(target = alice)
            |> on_drop(release(120ms))
    }

    thread a {
        focus.request(.blur(8px))
    }

    on .release_focus {
        focus |> drop
    }
}
```

Diagnostic:

```text
error: thread `a` may use `focus` after another handler drops it
help: move `focus` into the thread and drop it with `defer`
help: or convert it to a shared/cancellable handle
```

---

## 12. `defer`

### 12.1 Basic `defer`

```awft
let lease =
    alice.stage.lease()
    |> on_drop(release)

defer {
    lease |> drop
}
```

`defer` registers cleanup for the current runtime scope.

### 12.2 Defer order

Defers run in reverse registration order.

```awft
defer { log("A") }
defer { log("B") }
```

Cleanup order:

```text
B
A
```

### 12.3 Defer in thread

```awft
thread motion {
    let lease =
        alice.stage.lease()
        |> on_drop(release)

    defer {
        lease |> drop
    }

    alice.stage.apply(.motion.nod)
}
```

When the thread completes or is cancelled, its defer stack runs.

### 12.4 Defer in event handler

```awft
on .release_focus {
    let temp =
        stage.temp_effect(...)
        |> on_drop(cancel)

    defer {
        temp |> drop
    }

    temp.play()
}
```

The defer runs when the handler task exits.

### 12.5 Defer in line `init`

```awft
init {
    let focus =
        stage.focus(target = alice)
        |> on_drop(release(120ms))

    defer {
        focus |> drop
    }
}
```

This defer is attached to the `init` task, not the whole line. Usually this is not what you want for line-long state. For line-long values, store them in `'line.*` or let the line option create them.

Recommended:

```awft
init {
    'line.focus <-
        stage.focus(target = alice)
        |> on_drop(release(120ms))
}
```

### 12.6 Defer restrictions

`defer` must be bounded and cleanup-safe.

Disallow by default:

```text
- unbounded wait
- unbounded await
- spawning scoped child threads
- consuming values already moved elsewhere
```

---

## 13. Line-level `finally`

### 13.1 Purpose

`finally { ... }` is one line-final block.

```awft
with {
    finally {
        debug_log("line done")
    }
}
```

There should be at most one `finally` block per line plan.

If multiple are written:

```awft
finally { a() }
finally { b() }
```

Either error or merge in source order. Recommended: error.

```text
error: duplicate line `finally` block
help: combine the finalizers into one `finally { ... }`
```

### 13.2 Execution order

Line cleanup order:

```text
1. enter cleanup mode
2. process pending marks according to cleanup profile
3. cancel/join live child threads
4. run each thread's defer stack
5. run line-level finally
6. drop remaining line-owned registry handles, e.g. `'line.focus`
7. drop remaining MustDrop locals in reverse creation order
8. unregister handlers, subscriptions, exposed state
9. close line lifetime
```

### 13.3 Why before automatic line drops

`finally` should still be able to inspect and explicitly release line values:

```awft
finally {
    if let Some(f) = 'line.focus? {
        f |> drop(release(0ms))
    }
}
```

After `finally`, automatic registry finalizers handle anything still live.

---

## 14. Lifetime registry access

### 14.1 Use `'line.*`

Use:

```awft
'line.focus |> drop
```

Do not use:

```awft
line.focus |> drop
```

`'line.focus` means the value is stored in the current line lifetime registry.

### 14.2 Optional keys

If a key is statically guaranteed:

```awft
'line.focus : FocusHandle
```

If not guaranteed:

```awft
'line.focus? : Option<FocusHandle>
```

Static guarantee:

```awft
alice(.smile, focus = .soft)[
    聞いて。[mark .release_focus]
]
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

Conditional key:

```awft
with {
    init {
        if state.flags.focus_alice {
            'line.focus <-
                stage.focus(target = alice)
                |> on_drop(release(120ms))
        }
    }

    on .release_focus {
        if let Some(f) = 'line.focus? {
            f |> drop
        }
    }
}
```

Unsafe:

```awft
on .release_focus {
    'line.focus |> drop
}
```

if the key is conditional.

Diagnostic:

```text
error: lifetime key `'line.focus` is not statically guaranteed
help: use `'line.focus?` and handle Option<FocusHandle>
```

### 14.3 Generic lifetime registries

Possible future forms:

```awft
'flow.preload_task
'scene.bgm
'thread.cache
```

Do not invent them implicitly. A registry exists only if the runtime scope defines one.

Line registry is special because every dialogue line has one.

---

## 15. Inline marks and `on`

### 15.1 Marker tag

Use `[]` for dialogue control tags:

```awft
[mark .release_focus]
```

Use `#[...]` only for expression interpolation:

```awft
#[player_name]
#[fmt(score)]
```

Delete these forms:

```awft
[hook release_focus]
#[hook release_focus]
#[mark release_focus]
hook release_focus:
```

### 15.2 Handler

```awft
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

### 15.3 Marker without handler

Allowed, because marks are useful for:

```text
- transcript anchors
- test anchors
- voice marker matching
- future tool-generated handlers
- `wait mark .name` in threads
```

Optional lint:

```text
warning: unused mark `.release_focus`
```

### 15.4 Handler without marker

Error unless the trigger is not a marker.

```awft
with {
    on .missing {
        ...
    }
}
```

Diagnostic:

```text
error: marker handler `.missing` has no matching `[mark .missing]`
help: add `[mark .missing]` to the dialogue text
help: or use a non-marker trigger such as `on line.end { ... }`
```

### 15.5 Duplicate markers

Default: duplicate marker names in one line are errors.

```awft
[mark .beat]
...
[mark .beat]
```

Diagnostic:

```text
error: duplicate mark `.beat` in one dialogue line
```

If repeatable marks are needed, add explicit repeat syntax later.

---

## 16. Cleanup policy

Cleanup is a line option or project default, not a `with` statement.

```awft
alice(.smile, cleanup = .fast_skip)[
    聞いて。[p]
]
```

Project default:

```toml
[dialogue.cleanup.default]
pending_marks = "run"
visual = "snap"
audio = "stop_now"
threads = "cancel"
```

Profile:

```awft
pub cleanup profile @cleanup.fast_skip {
    pending_marks = run
    visual = ignore
    audio = stop_now
    threads = cancel
}
```

Fields:

```text
pending_marks
  run        Run pending marker handlers in cleanup mode.
  drop_only  Run only drop/state-cleanup portions.
  skip       Do not run pending marker handlers; finalizers still run.

visual
  normal
  snap
  suppress_transient
  ignore

audio
  normal
  stop_now
  suppress_new
  ignore

threads
  join
  cancel
  detach_error
```

---

## 17. Focus as a built-in line option

Common focus should not require `init`.

```awft
alice(.smile, focus = .soft)[
    聞いて。[p]
]
```

`focus = .soft` means:

```text
- Resolve `.soft` as a FocusProfile.
- Create a line-owned FocusHandle before presentation begins.
- Store it at `'line.focus`.
- Apply its enter behavior at line start.
- Apply release behavior on drop / cleanup.
```

Focus profile:

```awft
pub focus profile @focus.soft {
    target = speaker
    others = .blur(8px) & .dim(35%)
    enter = 180ms
    release = 120ms
    cleanup_visual = snap
}
```

Early release:

```awft
alice(.smile, focus = .soft, cleanup = .fast_skip)[
    聞いて。[mark .release_focus]
]
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

Multiple focus handles:

```awft
alice(.smile, focus = { main = .soft, bg = .background_dim })[
    聞いて。[mark .release_main]
]
with {
    on .release_main {
        'line.focus.main |> drop
    }
}
```

---

## 18. Drop and typestate

### 18.1 `on_drop`

```awft
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))
```

### 18.2 `expose`

```awft
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))
    |> expose(@state.opening.alice_focus)
```

### 18.3 `drop`

`drop` is a compiler intrinsic.

```awft
'line.focus |> drop
drop('line.focus)
```

Preferred source style:

```awft
'line.focus |> drop
```

Override drop policy:

```awft
'line.focus |> drop(release(40ms))
```

### 18.4 Typestate semantics

Drop should be represented as a typestate transition.

```text
FocusHandle<Live> |> drop -> FocusHandle<Dropped>
```

Use-after-drop is an error:

```awft
'line.focus |> drop
'line.focus.release()
```

Diagnostic:

```text
error: use of dropped value `'line.focus`
```

### 18.5 `let _ = value`

Plain values may be discarded.

MustDrop values should reject `let _ = ...`.

```awft
let _ = stage.focus(target = alice)
```

Diagnostic:

```text
error: MustDrop value should be explicitly dropped or scoped
help: use `value |> drop`, `drop(value)`, or attach `|> on_drop(...)`
```

---

## 19. Stage look, portrait look, and focus

Stage and portrait are separate targets.

```awft
alice(.smile)[
    おはよう。[p]
]
```

A common look may affect both stage and portrait.

Stage only:

```awft
alice(stage = .expr.worried)[
    ……大丈夫。[p]
]
```

Portrait only:

```awft
alice(portrait = .icon.wink)[
    ひみつだよ。[p]
]
```

Combined:

```awft
alice(.stage.expr.angry & .portrait.icon.smile)[
    怒ってないよ。[p]
]
```

Use only `&` as the patch merge operator.

```awft
.smile & .casual & .motion.nod
```

`+`, `|`, and `||` are not used for patch composition.

---

## 20. Grammar summary

### 20.1 Generic thread

Canonical statement form:

```text
ThreadStmt =
  "thread" ThreadModifier? ThreadName? "{" Block "}"
```

Indentation sugar:

```text
ThreadStmtSugar =
  "thread" ThreadModifier? ThreadName? ":" IndentedBlock
```

Flat fence sugar:

```text
FlatThread =
  "=== thread" ThreadModifier? ThreadName? "===" Lines "=== /thread ==="
```

Expression form:

```text
ThreadExpr =
  "thread" ThreadModifier? ThreadName? "{" Block "}"
```

### 20.2 Defer

Canonical block form:

```text
DeferBlock =
  "defer" "{" Block "}"
```

Single-expression form:

```text
DeferStmt =
  "defer" Expr
```

Indentation sugar:

```text
DeferBlockSugar =
  "defer" ":" IndentedBlock
```

Flat fence sugar:

```text
FlatDefer =
  "=== defer ===" Lines "=== /defer ==="
```

### 20.3 Line plan

Canonical:

```text
LinePlan =
  "with" "{" WithItem* "}"
```

Indentation sugar:

```text
LinePlanSugar =
  "with" ":" IndentedBlock
```

Flat fence sugar:

```text
FlatLinePlan =
  "=== with ===" FlatWithItem* "=== /with ==="
```

Items:

```text
WithItem =
    InitBlock
  | ThreadBlock
  | OnBlock
  | AtBlock
  | FinallyBlock
  | Let
  | Out
  | Expr
```

### 20.4 Deleted forms

Do not implement:

```awft
using ...
state 'line focus drop = ...
scope focus_fx until hook release_focus:
cleanup on skip:
hook release_focus:
[hook release_focus]
#[mark release_focus]
thread motion {
    finally { ... }
}
```

---

## 21. Current implementation gaps in Sanzentyo/arcweft

### 21.1 Current good direction

The current `LinePlan` is already documented as canonical `with { ... }` plus `with:` indentation sugar, and `BlockStyle` already distinguishes `Brace` and `Indent`. Keep this model and add `Flat` if flat fences need formatting preservation.

Current:

```rust
pub enum BlockStyle {
    Brace,
    Indent,
}
```

Needed:

```rust
pub enum BlockStyle {
    Brace,
    Indent,
    Flat,
}
```

`Flat` is formatting metadata only. It must not affect semantics.

### 21.2 Current LinePlanItem mismatch

Current `LinePlanItem::Thread` has a `finally: Vec<Stmt>` field.

Needed change:

```rust
pub enum LinePlanItem {
    Init(Vec<Stmt>),

    Thread {
        name: Option<String>,
        body: Vec<Stmt>,
    },

    On {
        trigger: Expr,
        body: Vec<Stmt>,
    },

    Finally(Vec<Stmt>),

    // ...
}
```

Thread-local cleanup should be represented by `Stmt::Defer(Expr)` and a future `Stmt::DeferBlock(Vec<Stmt>)`, not by `Thread { finally }`.

### 21.3 Current parser mismatch

Current parser recognizes colon heads such as `init:`, `thread:`, `on ...:`, and `finally:` inside line plans, and parses brace items too.

Needed change:

```text
- keep brace and colon parsing;
- add flat fence parsing;
- move `finally` out of thread body parsing and into line plan item parsing;
- add `defer { ... }` / `defer:` / flat `=== defer ===`;
- avoid thread-local finally parsing.
```

### 21.4 Dialogue tokens

Current `DialogueToken` has:

```text
Text
Raw
Tag
EndTag
Expr
Ruby
Escape
```

Needed addition:

```text
Mark(LineMark)
```

or a semantic pass that recognizes `Tag { name = "mark" }`.

Recommended: add `Mark`, because marker matching is semantic and should not hide inside generic tags.

### 21.5 Line options

Current `LineOptions` has:

```text
id
text_key
voice
window
source_locale
hooks
style
args
```

Needed additions:

```text
look
stage
portrait
focus
cleanup
```

### 21.6 Expression parser

Needed expression support:

```awft
'line.focus
'line.focus?
'line.focus.main
```

Potential AST:

```rust
Expr::LifetimePath {
    lifetime: String,
    path: Vec<String>,
    optional: bool,
}
```

Careful conflict:

```awft
out 'label expr
break 'label expr
continue 'label
```

In control-transfer statement position, apostrophe labels remain labels.  
In expression position, apostrophe path is lifetime registry access.

### 21.7 Type checker

Needed checks:

```text
- lifetime registry guaranteed-key analysis
- Option typing for unproven registry keys
- MustDrop tracking
- use-after-drop
- double-drop
- thread capture safety
- defer stack boundedness
- line-level finally uniqueness
- concurrent exclusive-axis write detection
- pending mark cleanup traversal
```

### 21.8 VM runtime

Needed runtime model:

```text
- VM child task groups
- line task group
- deterministic effect merge
- scoped thread finalization
- line finally
- defer stack execution
- cleanup policy execution
- pending marker traversal
```

Core must remain Sans I/O: no direct OS/GPU/audio side effects.

---

## 22. Required docs changes

### 22.1 `docs/01-language/dialogue-character-methods-and-textbox.md`

Update:

```text
- face -> look
- add stage/portrait/focus/cleanup
- use `'line.focus`, not `line.focus`
- remove local `[hook ...]`
- add `[mark .name]` + `on .name { ... }`
- show brace canonical first, colon sugar second
- mention flat fence sugar only as authoring format
```

### 22.2 `docs/01-language/dialogue-calls-scopes-cancellation.md`

Update:

```text
- add `init { ... }`
- add `thread { ... }`
- add `defer { ... }`
- add line-level `finally { ... }`
- add `on ... { ... }`
- remove thread-local `finally`
- remove `hook a1:`
- remove `cleanup on skip:`
- move cleanup policy into line options/profiles
- show brace canonical first
```

### 22.3 `docs/01-language/dialogue-control-tags-and-ruby.md`

Update:

```text
- define `[mark .name]`
- define `[]` as dialogue control tags
- define `#[...]` as expression interpolation only
- delete `[hook name]` local marker usage
- define escaping for literal `===` in flat line blocks
```

### 22.4 `docs/02-runtime/core.md`

Update:

```text
- add generic VM task group model
- add scoped `thread`
- add line task group
- add line-level finally and thread defer
- state that `thread` outputs are deterministic effect requests
- retain Sans I/O boundary
```

### 22.5 `docs/02-runtime/hooks-memoization.md`

Update:

```text
- distinguish top-level runtime hooks from line-local `on` handlers
- do not treat line-local `on` as global HookTable entries unless lowered as scoped hook records
```

### 22.6 `docs/03-presentation/character-stage.md`

Update:

```text
- separate stage look and portrait look
- add `LookPatch` / `CharacterPatch`
- add focus profile
- add Live2D motion/param support
- use @ refs, not # refs
```

### 22.7 `docs/03-presentation/audio.md`

Update:

```text
- replace `play voice ...` with `voice(...)` / `alice.voice(...)`
- replace # refs with @ refs
- distinguish dialogue voice option from standalone voice playback
```

---

## 23. Edge cases

### 23.1 Flat fence in text

Inside a flat line block:

```awft
=== line alice ===
\=== これは本文です
=== /line ===
```

Without escape, `===` at beginning of line is a fence.

### 23.2 Flat close mismatch

```awft
=== on .x ===
...
=== /thread ===
```

Error.

### 23.3 Flat unclosed block

Recover at next top-level fence or EOF, then emit diagnostic.

### 23.4 Mark in raw text

```awft
[raw][mark .x][/raw]
```

This is text, not a mark.

### 23.5 Escaped mark

```awft
\[mark .x]
```

This is literal text.

### 23.6 Mark inside interpolation

```awft
#[some_content]
```

Static marks inside runtime-generated content are not recognized unless expression has explicit `ContentWithMarks` type. Default: no static mark.

### 23.7 Localized text missing required mark

If source has:

```awft
[mark .release_focus]
```

and localized text omits it, fail or warn according to policy.

Recommended default:

```text
error if mark is referenced by `on .release_focus` or `wait mark .release_focus`
```

### 23.8 Handler without mark

Error unless trigger is non-marker.

### 23.9 Duplicate mark

Error by default.

### 23.10 Optional registry drop

Prefer explicit handling:

```awft
if let Some(f) = 'line.focus? {
    f |> drop
}
```

Do not silently define `drop(Option<T>)` unless the function is explicitly named:

```awft
'line.focus? |> drop_optional
```

### 23.11 Detached thread captures line key

Error.

### 23.12 Thread captures line key and cleanup drops it

Unsafe unless shared/cancellable.

### 23.13 `defer` awaits unbounded Need

Error.

### 23.14 `finally` awaits unbounded Need

Error.

### 23.15 Thread in pure function

Error unless function is effectful.

### 23.16 `thread` in `init`

Top-level `thread` declarations inside `with` start after `init`. A nested `thread` expression inside `init` should either be disallowed or scheduled after `init` completes. Recommended: disallow nested line threads in `init` for phase 1.

---

## 24. Recommended examples

### 24.1 Canonical simple line

```awft
alice(.smile)[
    おはよう。[p]
]
```

### 24.2 Indentation sugar simple line

```awft
alice(.smile):
    おはよう。[p]
```

### 24.3 Flat simple line

```awft
=== line alice(.smile) ===
おはよう。[p]
=== /line ===
```

### 24.4 Built-in focus

```awft
alice(.smile, focus = .soft)[
    聞いて。[p]
]
```

### 24.5 Focus release at mark

```awft
alice(.smile, focus = .soft, cleanup = .fast_skip)[
    聞いて。[mark .release_focus]
    こっちをみて[r]
]
with {
    on .release_focus {
        'line.focus |> drop
    }
}
```

### 24.6 Flat focus release

```awft
=== line alice(.smile, focus = .soft, cleanup = .fast_skip) ===
聞いて。[mark .release_focus]
こっちをみて[r]

=== with ===
=== on .release_focus ===
'line.focus |> drop
=== /on ===
=== /with ===
=== /line ===
```

### 24.7 Conditional focus

```awft
alice(.smile, cleanup = .fast_skip)[
    聞いて。[mark .release_focus]
]
with {
    init {
        if state.flags.focus_alice {
            'line.focus <-
                stage.focus(target = alice, others = .blur(8px) & .dim(35%))
                |> on_drop(release(120ms))
        }
    }

    on .release_focus {
        if let Some(f) = 'line.focus? {
            f |> drop
        }
    }
}
```

### 24.8 Concurrent line behavior

```awft
alice(.smile, voice = auto, focus = .soft, cleanup = .fast_skip)[
    聞いて。[mark .release_focus]
    こっちをみて[r]
]
with {
    thread motion {
        alice.stage.apply(.motion.nod)
        wait 0.35s
        alice.stage.apply(.stage.expr.worried)

        defer {
            alice.stage.apply(.motion.idle)
        }
    }

    thread portrait {
        wait mark .release_focus
        alice.portrait(.portrait.icon.wink)
    }

    on .release_focus {
        'line.focus |> drop
    }

    finally {
        debug_log("line finished")
    }
}
```

### 24.9 Flow-level generic thread

```awft
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    thread preload_next {
        asset.preload(@asset.bg.school_classroom)
        alice.preload(look = .smile, voices = auto)
    }

    bg(@asset.bg.school_classroom)

    alice(.smile)[
        おはよう。[p]
    ]
}
```

### 24.10 Flat flow-level thread

```awft
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
=== thread preload_next ===
asset.preload(@asset.bg.school_classroom)
alice.preload(look = .smile, voices = auto)
=== /thread ===

    bg(@asset.bg.school_classroom)

    alice(.smile)[
        おはよう。[p]
    ]
}
```

Formatter should either normalize this to braces or flat-script style consistently.

---

## 25. Implementation order

1. Confirm brace canonical policy in docs.
2. Add `BlockStyle::Flat` if flat style preservation is needed.
3. Add flat fence parser at CST/line-event layer.
4. Add `[mark .name]` to dialogue text model.
5. Add `'line.*` lifetime registry expression syntax.
6. Add `look`, `stage`, `portrait`, `focus`, `cleanup` to `LineOptions`.
7. Move line-plan `finally` out of `Thread`.
8. Add line-level `LinePlanItem::Finally`.
9. Add `defer { ... }` / `defer:` / flat `=== defer ===`.
10. Decide `Stmt::DeferBlock` vs encoding defer block as `Stmt::Defer(Expr::Block)`.
11. Add generic `thread` AST/parser support outside `with`.
12. Add VM task group model and scoped cleanup.
13. Add cleanup profiles.
14. Add typechecker pass for registry key guarantees and `Option`.
15. Add MustDrop/drop checker with typestate semantics.
16. Add thread capture and concurrent-effect conflict checks.
17. Update docs listed above.
