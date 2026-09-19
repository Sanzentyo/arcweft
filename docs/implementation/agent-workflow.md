# Agent workflow and task prompts

On-demand guidance for instruction maintenance and difficult handoffs, not a
startup checklist. The [root instructions](../../AGENTS.md) own task priority,
default autonomy, product invariants, and Git permissions.

## Delegate outcomes, not a sequence of approvals

An implementation request delegates ordinary design and integration choices as
well as editing. Infer routine omitted details from the user's goal, maintained
contracts, and current owners. Choose a complete solution; do not ask the user to
pick between technically adequate designs merely to avoid responsibility.
Document a material tradeoff where it affects later work, then proceed.

This includes necessary owner methods, cross-crate consumer migrations, dependency
and feature changes consistent with project policy, tests, fixtures, generated
artifacts, maintained documentation, and repairs caused by the change. The scope
is the complete requested result, not unlimited repository improvement. Review-only
and design-only remain distinct assignments; "continue" preserves the established
goal and mode unless the user changes them.

Current explicit user direction supersedes repository process defaults and skill
guidance, within higher-priority instructions and actual tool authorization.
An instruction document is not an independent stakeholder with veto power.
Read applicable Rust skills completely as required; do not confuse full skill
reading with loading every linked library. Do not edit frozen skill copies in
returned packages to pretend an external installed skill has been updated.

## What does and does not need a question

Ordinary uncertainty is part of the job. Inspect the relevant evidence, choose a
reasonable interpretation consistent with the goal, and proceed. Missing prose,
a large migration, multiple designs, a failing regression test, and a mismatch
between an old package and a later accepted contract do not inherently require
approval. Resolve precedence and technical facts rather than treating every
conflict as an unavailable external authority.

A question is warranted only when the remaining answer is material and cannot
be established from accessible evidence or authority already granted: for example,
competing user intentions, an unspecified destructive external operation, or a
specific permission the user explicitly reserved. Previously granted permission
does not need to be granted again. Do not invent warnings or approval checklists
for speculative harms of ordinary repository development.

Repository-main-only work, a requested local hold, and explicit restrictions in
user-provided skills remain real choices to honor, not evidence that all Git or
Rust operations are dangerous. Tool approval, production deployment, spending,
credential/access-control changes, and loss of user data are not authorized just
by asking for a code change. Apply actual scope instead of a blanket ask-first rule.

When a rule really prevents progress, identify the affected action and cite the
exact file/section and relevant requirement, or name the missing external fact.
Distinguish its wording from your interpretation. Continue independent work and
ask only the consequential unresolved question; do not re-ask a settled one.

## Resolve design work instead of exporting it

Current code shows implementation state; maintained specifications and accepted
contracts show the target. Neither historical package wording nor an existing
implementation is automatically the final design. A concrete flaw within an
authorized correction/redesign can require changing a maintained contract;
explain the choice and migrate all affected producers, consumers, and evidence.
Do not silently override a requirement the current user intended to retain.

Use a follow-up request only for genuinely outside authority or a separate
assignment, not to transfer an unfinished technical decision back to the user.
A design-only ZIP still closes every mutually constraining design decision and
states its actual validation. Lack of a compiler does not excuse omitting the
ZIP, and successful ZIP creation does not prove runtime implementation readiness.
Use [review intake](../reviews/README.md) for actual package work.

## Validation and continuation

Choose evidence by the [test policy](test-execution-policy.md), not by an impulse
to run every available command. Preserve required behavioral coverage, reuse
unchanged passes, repair real regressions, and finish once sufficient evidence
exists. Do not turn every completed check into a reason to look for a new gate.

After compaction, compare current Git with the existing goal, contract, changed
paths, and recorded evidence. Reopen changed or missing context, not all historical
packages. Continue the next unresolved acceptance item; neither compaction nor a
completed intermediate commit needs a new "continue" from the user. Maintain one
useful task note when needed, not a ledger for every small action.

## Prompt examples

These examples are optional; a short request with established context is valid.
Do not ask the user to fill a template before beginning ordinary work.

### Implementation or correction

```text
Complete [outcome] against [contract/context]. Resolve the necessary technical
choices, migrate affected consumers, and fix regressions. Deliver the coherent
change with evidence for [observable acceptance]. Non-goals: [actual exclusions].
```

### Continuation

```text
Continue [goal/current note] to its remaining acceptance criteria. Reconcile
live Git state, reuse valid evidence, and finish without stopping at the first
patch or next commit. Keep the established implementation/design-only mode.
```

### Design only

```text
Design [boundary] and return [artifact]. Close the mutually constraining choices
against current owners and inherited requirements. Keep production unchanged;
include exactly what was and was not validated inside the requested ZIP.
```

## Maintain instructions by observed value

Keep durable product invariants and actual permissions in their owning policy;
remove obsolete model-workarounds rather than move them into another mandatory
file. Skills should add recurring repository-specific value with a narrow trigger
and on-demand references, not duplicate the root instructions or teach routine
coding. Generic process advice must not outrank the user's explicit task.

For instruction changes, review representative outcomes and stop decisions,
including false stops as well as unwanted actions. Check a simple edit, a broad
owner migration, a resolvable contract mismatch, a real permission boundary,
continuation, and requested artifact delivery. This is a policy review, not a
source-spelling CI gate or a claim that model performance has been measured.
Use a new rule only for an actual unmet requirement or observed failure.

Sources: OpenAI's
[Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
(2026-09-11) and the supplemental
[model guidance](https://developers.openai.com/api/docs/guides/latest-model).
These motivate removing obsolete scaffolding; the concrete delegation and
validation boundaries here are Arcweft policy decisions, not claims of model
infallibility or permission supplied by a model name.
