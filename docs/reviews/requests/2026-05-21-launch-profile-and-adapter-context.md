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

The immediate issue was the earlier `route_params.name` server-flow style. It
is no longer accepted, because route path captures are not Arcweft language
builtins. Server routes now bind path captures to flow parameters explicitly.

That is safer than putting route captures directly into `arcweft-lang-sema` as
ambient symbols and keeps server flows reusable outside the server adapter.

## Current state in this branch

- A project-shaped sample exists under `samples/visual-novel-mini/`.
- `arcweft-lang-syntax` parses speaker preset calls such as:

```arcw
alice(face=.smile, voice=auto, window=@textbox:.side)
```

- `arcweft-lang-sema` rejects unresolved bare atoms such as `face=smile` until
  an option/schema/atom registry exists.
- `.smile` and `.worried` continue to work as short variant-style atoms.
- `arcw check samples/visual-novel-mini/src/server.arcw` accepts explicit
  route-to-flow parameter bindings.
- `arcw serve samples/visual-novel-mini/src/server.arcw --adapter native-http`
  binds path captures to those flow parameters at request time.
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

### Option C: Put adapter context into `.arcw` entry declarations

Make source entries explicitly declare their adapter context:

```arcw
entry server @entry.http adapter native_http {
    inject request: HttpRequestContext
    route GET "/hello/:name" -> @flow.hello(name = :name)
}
```

Pros:

- Source is self-describing.
- No separate manifest is needed for small projects.
- LSP can edit the entry and injected symbols together.

Cons:

- Source may become host-profile heavy.
- Product/dev/local adapter differences can clutter `.arcw`.
- Needs a clean story for secrets, ports, env vars, and build profiles.

## Recommended direction

Adopt Option B as the main direction:

```text
LaunchProfile is the canonical model.
Dedicated commands are aliases.
AdapterContext is data selected by the profile.
```

Then allow a limited source-level shorthand only when useful:

```arcw
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}
```

The manifest/profile can attach `native-http` and provide injected symbols.

## Resolution from pro review 23

`docs/reviews/pro_review23.md` confirms Option B:

```text
LaunchProfile is the canonical model.
Dedicated commands are aliases.
AdapterContext is data selected by the profile.
```

The implementation now adds `arcweft-launch`, a Sans I/O manifest/profile model
for `arcw.toml`, and routes source-loading CLI commands through a shared source
selection step:

```bash
arcw check <file.arcw>
arcw check --manifest arcw.toml --profile server.dev
arcw serve --manifest arcw.toml --profile server.dev
arcw cli --manifest arcw.toml --profile cli.main -- ARGS...
arcw test --manifest arcw.toml --profile test.opening
arcw bench --manifest arcw.toml --profile bench.opening
```

Direct source mode remains strict. Profile mode applies the selected adapter
context before semantic checking. Route path captures are explicit
route-to-flow bindings; `route_params` is intentionally not an adapter symbol.

## Decisions needed

1. Should `arcw serve` remain a first-class command, or become a thin alias for
   `arcw run --profile ...`?
2. Should adapter-injected symbols live in:
   - project manifest / launch profile,
   - `.arcw` entry declarations,
   - adapter metadata crate,
   - or a combination?
3. Should generic `arcw check` accept adapter context when a file contains
   `entry server`, or should adapter context only be available through a selected
   profile/adapter?
4. Should route captures be conventional injected bindings, or explicit flow
   parameters?

```arcw
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.hello hello(name: String) {
    return name
}
```

5. Should speaker option atoms require explicit registries before bare forms
   like `face=smile` are accepted?

Current patch chooses the strict answer for 3 and 5:

```text
generic check: no adapter-injected symbols
serve/native-http: adapter context applies only for adapter symbols such as request
bare unresolved atoms: rejected
short variant atoms: accepted
```

The current implementation keeps those choices. It additionally rejects unknown
profile adapters before checking source, so adapter availability is explicit
project data rather than an ambient language assumption.


