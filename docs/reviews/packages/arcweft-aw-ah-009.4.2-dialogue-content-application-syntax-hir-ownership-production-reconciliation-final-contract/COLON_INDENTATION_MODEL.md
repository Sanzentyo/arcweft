# Exact colon indentation model

## 1. Public syntax shapes

All fields are private. Public methods are read-only accessors. Construction is
`pub(crate)` and checked. Syntax-only types derive no Serde traits.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueIndentation {
    Inline(DialogueInlineIndentation),
    Indented(DialogueIndentedIndentation),
    Missing(DialogueMissingIndentation),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueIndentationBytes(usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueIndentationPrefix {
    range: TextRange,
    width: DialogueIndentationBytes,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineEnding {
    range: TextRange,
    kind: DialogueLineEndingKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineEndingKind {
    Lf,
    CrLf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueInlineIndentation {
    head: DialogueIndentationPrefix,
    separator: TextRange,
    boundary: DialogueInlineBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueInlineBoundary {
    LineEnding(DialogueLineEnding),
    AttachedPlan { plan_syntax: SyntaxNodeId, at: usize },
    OwnerEnd { anchor: usize },
    EndOfDocument { anchor: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueIndentedIndentation {
    head: DialogueIndentationPrefix,
    head_line_ending: DialogueLineEnding,
    body: TextRange,
    base: DialogueIndentationPrefix,
    dedent: DialogueDedentBoundary,
    issues: Box<[DialogueIndentationIssue]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueMissingIndentation {
    head: DialogueIndentationPrefix,
    after_colon: DialogueMissingAfterColon,
    retained_trivia: Option<TextRange>,
    insertion: usize,
    boundary: DialogueMissingBoundary,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueMissingAfterColon {
    SameLine { separator: TextRange },
    NextLine { head_line_ending: DialogueLineEnding },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueMissingBoundary {
    Inline(DialogueInlineBoundary),
    Indented(DialogueDedentBoundary),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueDedentBoundary {
    DedentedLine {
        line_start: usize,
        indentation: DialogueIndentationPrefix,
    },
    AttachedPlan {
        plan_syntax: SyntaxNodeId,
        line_start: usize,
        indentation: DialogueIndentationPrefix,
    },
    OwnerEnd { anchor: usize },
    EndOfDocument { anchor: usize },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueIndentationIssue {
    Misaligned {
        indentation: DialogueIndentationPrefix,
        required: DialogueIndentationBytes,
    },
}
```

`DialogueIndentationBytes` exposes `get() -> usize`. Prefix, line-ending,
inline, indented, missing, boundary, and issue types expose one accessor per
field. No mutator exists.

## 2. Crate-private construction

```rust
impl DialogueIndentationPrefix {
    pub(crate) fn try_new(
        document: &SourceDocument,
        range: TextRange,
    ) -> Result<Self, DialogueSurfaceInvariantError>;
}

impl DialogueLineEnding {
    pub(crate) fn try_new(
        document: &SourceDocument,
        range: TextRange,
    ) -> Result<Self, DialogueSurfaceInvariantError>;
}

impl DialogueIndentation {
    pub(crate) fn try_inline(
        document: &SourceDocument,
        head: DialogueIndentationPrefix,
        separator: TextRange,
        boundary: DialogueInlineBoundary,
    ) -> Result<Self, DialogueSurfaceInvariantError>;

    pub(crate) fn try_indented(
        document: &SourceDocument,
        head: DialogueIndentationPrefix,
        head_line_ending: DialogueLineEnding,
        body: TextRange,
        base: DialogueIndentationPrefix,
        dedent: DialogueDedentBoundary,
        issues: Box<[DialogueIndentationIssue]>,
    ) -> Result<Self, DialogueSurfaceInvariantError>;

    pub(crate) fn try_missing(
        document: &SourceDocument,
        head: DialogueIndentationPrefix,
        after_colon: DialogueMissingAfterColon,
        retained_trivia: Option<TextRange>,
        insertion: usize,
        boundary: DialogueMissingBoundary,
    ) -> Result<Self, DialogueSurfaceInvariantError>;
}
```

`DialogueSurfaceInvariantError` is crate-private and typed. It has variants for
checked arithmetic overflow, non-UTF-8 boundary, out-of-document range,
non-whitespace prefix, width mismatch, invalid line-ending bytes, ordering,
containment, invalid base relation, issue outside body, and boundary mismatch.
It is an internal parser failure and is never converted into a user diagnostic.

## 3. Measurement and whitespace policy

Indentation is an existing source-byte concern, not a formatter display
concern:

- only ASCII space (`0x20`) and horizontal tab (`0x09`) are indentation bytes;
- width is exactly `range.end() - range.start()` after checked subtraction;
- each tab contributes one byte unit;
- mixed space/tab prefixes are valid;
- prefix equality for attachment uses both width and exact authored prefix
  bytes; equal width with different bytes does not attach a plan;
- non-ASCII whitespace is content and cannot occur in an indentation prefix;
- no Unicode normalization, visual-column calculation, or tab-stop expansion
  occurs.

The `head` prefix begins at the physical line start and ends at the first byte
of the target expression. The colon itself is not part of the indentation
carrier.

## 4. Inline form

After the colon, horizontal space/tab bytes form `separator`. If a semantic
content token appears before the current owner boundary or line ending, the
form is `Inline`.

The content surface stores a bounding range from the first semantic content
byte through the last semantic content byte. Leading separator and trailing
trivia/comments are excluded from that range but remain recoverable from the
source document. The `Inline` boundary records exactly why scanning stopped:
LF, CRLF, attached plan, owner end, or end of document.

LF occupies one byte. CRLF occupies two bytes and is one line-ending object.
A bare CR follows the existing lexer/recovery path and is not accepted as a
valid `DialogueLineEnding`.

If only horizontal trivia and/or a comment follows the colon before the line
ending, the parser continues to the indented-form decision. It does not create
empty inline content.

## 5. Indented form

The form is indented only when the colon's physical line ends in LF or CRLF and
a later meaningful line has an indentation width strictly greater than the
head width.

The algorithm is exact:

1. retain every blank and comment-only line after the head line ending;
2. ignore those lines when selecting the base;
3. the first meaningful line with width greater than the head establishes the
   entire authored `base` prefix;
4. every later meaningful line with width at least the base remains in the
   body;
5. a meaningful line with width at most the head ends the body at that line;
6. a meaningful line with `head.width < width < base.width` remains in the
   body, records `Misaligned`, and poisons the application;
7. blank and comment-only lines never dedent, regardless of their prefix;
8. an eligible `with:` or `with {}` line at the exact head prefix becomes
   `AttachedPlan`; otherwise the first dedented line remains outside the
   application.

`body` is the exact raw half-open source range beginning immediately after the
head line ending and ending immediately before the dedent/plan/owner/EOF
boundary. It includes original indentation bytes, LF/CRLF bytes, blank lines,
comment-only lines, internal comments, and trailing blank/comment-only lines.
The dedented line is not included.

The semantic `DialogueContent` projection removes exactly `base.width` leading
bytes from every clean meaningful content line. Extra indentation beyond the
base remains content. For a misaligned meaningful line, the projection removes
its complete observed indentation prefix, retains the content tokens, and
marks the application recovered; it never inserts or normalizes bytes. Blank
lines project through the existing dialogue line-break semantics. Tokens that
the lexer classifies as comments remain source trivia and do not become text.

The stored content site is the bounding source range from the first semantic
content token through the last. Exact per-part RichText/control/interpolation
ranges remain owned by the existing `DialogueContent` source map.

## 6. Empty and missing content

If no semantic content token exists—whether there are no bytes, only horizontal
trivia, only blank lines, only comment lines, or a dedent before content—the
surface uses `DialogueIndentation::Missing` and a missing `DialogueContentSite`.

`retained_trivia` is `None` when no trivia bytes exist and otherwise is their
exact authored range. `insertion` is the first byte at which content was
expected. It is retained as an insertion offset, not represented as an invented
content range.

Missing content emits the existing generic expected-content recovery at the
insertion site, marks the application recovered, and permits tooling HIR
publication. It does not make the application executable.

## 7. Comments, Unicode, and trailing trivia

- Unicode content bytes are copied neither into the indentation carrier nor
  into HIR display strings; source ranges point into the retained document.
- Comments before the first meaningful indented line remain in `body` and do
  not select `base`.
- Comments between content lines remain in `body` and do not alter dedent.
- Trailing comments and blank lines before a dedent remain in `body` but are
  excluded from the semantic content bounding range when no semantic token
  follows them.
- Inline trailing trivia remains in the source document and is excluded from
  the semantic content range.

## 8. Source-span projection

Syntax stores only `TextRange` and insertion offsets. HIR lowering obtains the
exact accepted `SourceDocumentIdentity` and revision from the lowering request,
checks every UTF-8 boundary and document limit, and asks the source owner to
construct `SourceSpan`. Missing content uses `HirInsertionPoint`. No payload
stores a fake zero span or uses a display string as authority.

## 9. Serialization and equality

All indentation types are session/source syntax only. They derive
`Clone`/`Debug`/`Eq`/`PartialEq`, plus `Copy`/`Hash`/ordering only for small
value types shown above. They do not implement Serde, persistence codecs, or
public raw constructors.
