# Cross-cutting language decisions

The maintained chapters below own the current language and runtime contracts.
This page is an index, not a second syntax authority or an implementation-phase
plan. The shared authoring surface is defined by
[Converged Language, Content, and Presentation Surface](../01-language/converged-language-surface.md).

| Concern | Maintained authority |
|---|---|
| Scenario syntax, attributes, and Dialogue/RichText surface | [Converged authoring surface](../01-language/converged-language-surface.md), [Flow-integrated scenario syntax](../01-language/scenario-surface-syntax.md) |
| Grammar, structured patterns, expressions, statements, and semicolons | [Grammar Summary](../01-language/grammar.md), [Patterns and Structured Bindings](../01-language/patterns-and-bindings.md), [Expression Control Flow](../01-language/expression-control-flow.md), [Semicolon Policy](../01-language/semicolon-policy.md) |
| Control transfers and bottom type | [Control transfer](../01-language/control-transfer-return-out-yield.md), [Never / Bottom Type](../01-language/never-bottom-type.md) |
| IDs, references, and lexical scopes | [ID and references](../01-language/ids-and-references.md), [Block scopes](../01-language/block-scopes.md) |
| Module and import paths | [Modules](../01-language/modules.md) |
| CharacterDialogue construction and content application | [CharacterDialogue authoring](../01-language/character-dialogue.md), [Dialogue calls, scopes, and cancellation](../01-language/dialogue-calls-scopes-cancellation.md) |
| Primitive and standard types | [Literals and primitive types](../01-language/literals-and-primitives.md), [Standard Types and Prelude](../01-language/standard-types-and-prelude.md) |
| Await, `Need`, `Result`, `Option`, errors, and context | [Await and Need](../01-language/await-need-result.md), [Result / Option and context](../01-language/result-option-context.md), [Error trace and context](../01-language/error-trace-context.md) |
| Checked runtime behavior and execution boundary | [Control flow, patterns, and semantic input](../02-runtime/control-flow-runtime.md), [RuntimeStep and Executors](../02-runtime/runtime-step-and-executors.md) |
