# arcw entries, capabilities, virtual paths, and host effects

This document defines the current entry, capability, virtual-path, and host
effect grammar.

## Entry declarations

`flow` remains the core story/control unit, but it is no longer the program entry point. Entry points are explicit.

```text
EntryDecl := Visibility? 'entry' EntryKind EntryId EntryBlock
EntryKind := 'game' | 'editor' | 'cli' | 'server' | 'activity'
           | 'test' | 'bench' | 'agent' | Ident
EntryId   := EntityRef
EntryBlock := '{' EntryItem* '}'
EntryItem := StatefulRole | AgentRole | 'goto' EntityRef
           | RouteDecl | EntryOption
StatefulRole := 'state' '=' Type
              | 'initializer' '=' ProjectSymbolPath
              | 'event' '=' Type
              | 'reducer' '=' ProjectSymbolPath
AgentRole := 'controller' '=' ProjectSymbolPath
RouteDecl := 'route' HttpMethod String '->' EntityRef RouteArgList?
RouteArgList := '(' RouteArg (',' RouteArg)* ')'
RouteArg := Ident '=' ':' Ident
EntryOption := Ident '=' Expr
HttpMethod := 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'
```

### Game entry

```arcw
struct GameState {
    score: i32
}

enum GameEvent {
    Start
}

fn initial_game_state() -> GameState
effects {}
{
    GameState { score = 0 }
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    Ok(Reduction.unchanged(state))
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}

flow @flow.opening opening(state: GameState) {
    log.info("game started")
    return "ok"
}
```

`game`, `editor`, and `test` entries each require exactly one `state`,
`initializer`, `event`, `reducer`, and initial `goto`. These roles resolve to
ordinary nominal types, functions, and flows. There is no separate state or
reducer callable family.

An Agent entry binds an ordinary function in the same way:

```arcw
#[budget(timeout = 20s, steps = 100000usize)]
fn opening_smoke() -> Result<Unit, AgentError>
effects { agent.observe }
{
    agent.observe()
    Ok(())
}

entry agent @entry.agent.opening_smoke {
    controller = opening_smoke
}
```

Launch profiles select one complete `entry.*` ID and expected kind. They do
not bind source roles; profile keys named `state`, `initializer`, `event`,
`reducer`, or `controller` are invalid. This keeps role identity in checked
source and leaves launch manifests responsible only for selection and host
policy.

### CLI entry

```arcw
entry cli @entry.cli.main {
    goto @flow.cli_main
}

flow @flow.cli_main main effects { stdio.write, process.exit } {
    let args = cli.args()
    if args.len() < 2 {
        cli.stderr("usage: greet NAME")
        return cli.exit(2)
    }
    cli.stdout("hello {args[1]}")
    return cli.exit(0)
}
```

### Server entry

```arcw
entry server @entry.http {
    route GET "/health" -> @flow.health
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.health() -> String {
    return "ok"
}

flow @flow.hello(name: String) -> String {
    return "hello {name}"
}
```

Route path parameters are not ambient locals. A route must explicitly bind path
captures to flow parameters with `name = :path_param`. This keeps the target
flow reusable in tests and non-server profiles, and lets the checker validate
that a route supplies the parameters required by the flow signature.

## Capabilities

Host I/O is declared by capability, not by direct core APIs.

```text
ExternCapabilityDecl := Visibility? 'extern' 'capability' CapabilityId CapabilityBlock
CapabilityBlock := '{' CapabilityItem* '}'
CapabilityItem := TypeDecl | CapabilityFnDecl
CapabilityFnDecl := 'fn' Ident GenericParams? ParamGroup+ ReturnType? EffectClause?
EffectClause := 'effects' '{' EffectExprList? '}'
EffectExprList := Expr (',' Expr)* ','?
```

There is no capability policy declaration. The capability's abstract interface
is the set of its type and function members. Each function's external effects
are declared by that function's `effects` clause. The selected launch profile
and adapter determine which effects and host calls the target provides. These
facts are checked together; they are not copied into a source or manifest
policy record.

Documentation comments, attributes, and visibility use the ordinary
declaration rules and attach to the capability, a type member, or a function
member.

### Canonical capability

```arcw
/// Filesystem operations supplied by the selected host adapter.
pub extern capability fs {
    /// Host-owned path identity.
    pub type Path

    /// Host-owned filesystem failure.
    pub type FsError

    /// Reads UTF-8 text from a host path.
    pub fn read_text(path: Path) -> Need<String, FsError>
        effects { fs.read }

    /// Writes UTF-8 text. Curried groups remain part of the function signature.
    pub fn write_text(path: Path)(text: String) -> Need<Unit, FsError>
        effects { fs.write }
}
```

### Multiple effects

```arcw
extern capability net {
    type Request
    type Response

    fn send(request: Request) -> Need<Response, NetError>
        effects { net.connect, net.send }
}
```

### No external effect

A capability function without an `effects` clause has an empty external effect
set under the existing function contract.

```arcw
extern capability clock {
    type Instant
    fn zero() -> Instant
}
```

### Target availability

Source declares what calling an operation means. It does not select the host.
The resolved profile selects one adapter; that adapter declares target-provided
effects and host calls. If a checked callable reaches an effect the selected
target cannot provide, semantic analysis reports `AWF-EFX-007`. If the selected
runtime runner does not implement a host call declared by the selected adapter,
runtime-host conformance reports `MissingHostCallImplementation` before
execution.

### Invalid members

A capability body accepts only `type` and `fn` declarations. Any other member
is an ordinary invalid capability member and is retained as lossless recovery
so following members and declarations remain parseable. There is no
compatibility or removed-spelling grammar.

### Path virtualization

```arcw
extern capability path {
    fn save(path: String) -> VirtualPath
    fn asset(path: String) -> VirtualPath
    fn temp(path: String) -> VirtualPath
    fn export(path: String) -> VirtualPath
}
```

Forbidden in arcw source:

```arcw
fs.read_text("<host-absolute-profile-path>")
```

Required:

```arcw
fs.read_text(path.save("profile.json"))
```

### Stdio and process

```arcw
extern capability cli {
    type IoError

    fn args() -> Vec<String>
    fn stdin_text() -> Need<String, IoError>

    fn stdout(text: String)
        effects { stdio.write }

    fn stderr(text: String)
        effects { stdio.write }

    fn exit(code: i32) -> Never
        effects { process.exit }
}
```

### HTTP

```arcw
extern capability http {
    type HttpRequest
    type HttpResponse
    type HttpError
    type RequestRef

    fn respond(req: RequestRef, response: HttpResponse)
        effects { http.respond }

    fn fetch(req: HttpRequest) -> Need<HttpResponse, HttpError>
        effects { network.http }
}
```

## Source declarations remain canonical

External live streams still use `source` declarations with explicit backpressure, replay, and privacy policy.

```arcw
source @source.http_requests: Source<HttpRequest, HttpError> {
    from http.requests(@entry.http)
    backpressure = bounded(capacity = 1024, overflow = drop_oldest)
    replay = event_only
    privacy = redacted

    on item req => yield req
    on error e => log.warn("http source error", error = e)
}
```

## Effects clauses

Effects are capability facts.

```arcw
flow @flow.save_profile save_profile
 effects { fs.read(save), fs.write(save), log.write }
{
    let profile = try await fs.read_text(path.save("profile.json")) with {
        error e => return "{}"
    }
    try await fs.write_text(path.save("backup.json"), profile) with {
        error e => log.warn("backup failed", error = e)
    }
    return profile
}
```

The current checker rejects calls to capability functions when their declared
effects are not present in the active flow/function effect scope. Filesystem
capabilities also reject direct OS absolute path string literals; construct a
`VirtualPath` with the `path` capability instead.

