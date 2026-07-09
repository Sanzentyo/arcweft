# Seq06.12 future work

1. Bind seq06.11 retained interaction state into CSS pseudo-state evaluation for
   `:hover`, `:focus`, `:active`, and `:disabled` in normal player frames.
2. Lower CSS custom properties into `StyleTokenBinding` only after defining the
   precedence model between Arcweft tokens, CSS variables, and View-local
   style patches.
3. Extract full Takumi computed style snapshots instead of coverage-only
   declaration evidence.
4. Add a retained container-query dependency graph and invalidation model before
   enabling `@container`.
5. Promote grid from structured diagnostic to supported layout only after native
   and web visual fixtures prove stable parity.
6. Add media-query runtime branching for viewport and accessibility preferences,
   using `PresentationEnvironment` as the shared native/web data source.
7. Expand visual smoke from fixture-manifest validation to actual generated
   PNG/JSON comparisons once this patch is applied to a full checkout with the
   renderer toolchain available.
8. Keep transitions, keyframes, animation timelines, and advanced effects under
   seq06.13 rather than hiding them in this coverage package.
