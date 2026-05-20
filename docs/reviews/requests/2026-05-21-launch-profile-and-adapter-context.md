# Request: Launch Profiles, Adapter Contexts, and Dedicated CLI Commands

## Why this needs a decision

The current implementation can parse and check a project-shaped sample, and
`arcw serve` can validate a server entry with native HTTP adapter context.
However, the shape exposes a larger design question:

```text
If `arcw serve` / `arcw cli` need special command paths and special injected
symbols, Arcweft source and project metadata are probably not expressive enough
yet.
```

The immediate issue was `route_params.name` in a server flow. It should not be
accepted by the language checker globally, because `route_params` is not an
Arcweft language builtin. It is a binding injected by a server adapter. The
current patch therefore keeps generic `arcw check` strict and only makes
`arcw serve --adapter native-http` type-check with a native HTTP adapter context.

That is safer than putting `route_params` directly into `arcweft-lang-sema`, but
it is still an interim design.

## Current state in this branch

- A project-shaped sample exists under `samples/visual-novel-mini/`.
- `arcweft-lang-syntax` parses speaker preset calls such as:

```awft
alice(face=.smile, voice=auto, window=@textbox:.side)
```

- `arcweft-lang-sema` rejects unresolved bare atoms such as `face=smile` until
  an option/schema/atom registry exists.
- `.smile` and `.worried` continue to work as short variant-style atoms.
- `arcw check samples/visual-novel-mini/src/server.awft` rejects
  `route_params` as an unknown symbol.
- `arcw serve samples/visual-novel-mini/src/server.awft --adapter native-http`
  accepts it through adapter-provided type context.
- A small `arcweft-adapter-context` crate holds the native HTTP adapter semantic
  context so it is not hard-coded in `arcweft-lang-sema`.

## Design concern

Dedicated commands currently do more than choose execution mode:

```text
arcw run    -> generic checked runtime execution
arcw cli    -> CLI entry selection plus CLI-specific runtime bindings
arcw serve  -> server entry selection plus HTTP-specific runtime bindings
arcw test   -> script test collection and execution
arcw bench  -> script bench validation/execution
```

This suggests the source/project model lacks a unified launch concept. If
`server`, `cli`, `game`, `test`, and `bench` are all entry/profile variants,
then special top-level commands should eventually become thin aliases rather
than separate semantic paths.

## Options

### Option A: Keep dedicated commands as first-class

Keep `arcw serve`, `arcw cli`, `arcw test`, and `arcw bench` as distinct command
families. Each command chooses its adapter context and runner behavior.

Pros:

- Simple user-facing commands.
- Easy to keep server-specific flags like `--listen`.
- Low implementation risk.

Cons:

- Semantics are spread across commands.
- LSP, verifier, and formatter need to know command-specific contexts.
- Source/manifests remain less expressive than the runtime.

### Option B: Introduce launch profiles and make dedicated commands aliases

Add a project-level `LaunchProfile` / manifest model, and let all execution
flow through one path:

```bash
arcw run --profile server.dev
arcw run --profile cli.main -- ARGS...
arcw run --profile game.main
arcw run --profile test.opening
arcw run --profile bench.opening
```

Dedicated commands can remain as convenience aliases:

```bash
arcw serve --entry http --adapter native-http
# lowers to:
arcw run --profile server.http.native
```

Pros:

- One semantic execution path.
- Adapter context becomes project/profile data.
- LSP and verifier can reason about the same profile model.
- Better fit for agent editing and project manifests.

Cons:

- Requires manifest/profile design before more runtime work.
- Some command UX needs redesign.

### Option C: Put adapter context into `.awft` entry declarations

Make source entries explicitly declare their adapter context:

```awft
entry server @entry.http adapter native_http {
    inject request: HttpRequestContext
    inject route_params: Map<String, String>
    route GET "/hello/:name" -> @flow.hello
}
```

Pros:

- Source is self-describing.
- No separate manifest is needed for small projects.
- LSP can edit the entry and injected symbols together.

Cons:

- Source may become host-profile heavy.
- Product/dev/local adapter differences can clutter `.awft`.
- Needs a clean story for secrets, ports, env vars, and build profiles.

## Recommended direction

Adopt Option B as the main direction:

```text
LaunchProfile is the canonical model.
Dedicated commands are aliases.
AdapterContext is data selected by the profile.
```

Then allow a limited source-level shorthand only when useful:

```awft
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello
}
```

The manifest/profile can attach `native-http` and provide injected symbols.

## Decisions needed

1. Should `arcw serve` remain a first-class command, or become a thin alias for
   `arcw run --profile ...`?
2. Should adapter-injected symbols live in:
   - project manifest / launch profile,
   - `.awft` entry declarations,
   - adapter metadata crate,
   - or a combination?
3. Should generic `arcw check` accept adapter context when a file contains
   `entry server`, or should adapter context only be available through a selected
   profile/adapter?
4. Should `route_params` be a conventional injected binding, or should route
   params be explicit flow parameters?

```awft
flow @flow.hello hello(route_params: RouteParams) {
    return route_params.name
}
```

5. Should speaker option atoms require explicit registries before bare forms
   like `face=smile` are accepted?

Current patch chooses the strict answer for 3 and 5:

```text
generic check: no adapter-injected symbols
serve/native-http: adapter context applies
bare unresolved atoms: rejected
short variant atoms: accepted
```

