# Open questions

These are external to `.1.3.1` and must be answered by accepted predecessor
contracts. They are not invitations for the task-plan implementation to guess.

1. From `.1.2`: what exact stable declaration/body path type is published for
   a View body containing Match, and which accepted field/case/function
   semantic identity types and byte transcripts does it expose?
2. From `.1.2`: which current checked body families become transcript-complete,
   and which remain typed fail-closed? This fixes the executable row visitor's
   success domain.
3. From `.1.4`: what exact lower/shared owner publishes `ViewMatchSiteId` and
   `CheckedViewMatchAdmissionDigest` so both compiler and bundle can name the
   actual types without a bundle-to-compiler dependency?
4. From `.1.4`: what exact compiler-local catalog key joins a runtime-plan task
   origin to one retained View operation, and what exact validated View product
   API consumes that join?
5. From `.1.4`: which View operations create task plans, which source-order
   coordinates they expose, and which operation/value-slot/capture facts are
   included in admission versus task-plan request/control semantics?

Finalization must replace this file with exactly `none` only after both
predecessor returns are ingested and these answers are reconciled against the
then-current full Git SHA.
