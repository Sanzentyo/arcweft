# arcw entries, capabilities, virtual paths, and host effects

This document proposes grammar changes that should land destructively. No legacy spellings, aliases, or migration wrappers are added.

## Entry declarations

`flow` remains the core story/control unit, but it is no longer the program entry point. Entry points are explicit.

```text
EntryDecl := Visibility? 'entry' EntryKind EntryId? EntryBlock
EntryKind := 'game' | 'cli' | 'server' | 'activity' | 'test' | 'bench' | Ident
EntryId   := EntityRef | RelativeId | FamilyRelativeEntityRef
EntryBlock := '{' EntryItem* '}'
EntryItem := 'start' EntityRef | 'run' EntityRef | RouteDecl | EntryOption
RouteDecl := 'route' HttpMethod String '->' EntityRef RouteArgList?
RouteArgList := '(' RouteArg (',' RouteArg)* ')'
RouteArg := Ident '=' ':' Ident
HttpMethod := 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'
```

### Game entry

```arcw
entry game @entry.main {
    start @flow.opening
}

flow @flow.opening opening {
    log.info("game started")
    return "ok"
}
```

### CLI entry

```arcw
entry cli @entry.main {
    run @flow.cli_main
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
CapabilityItem := CapabilityFnDecl | TypeDecl | CapabilityPolicyDecl
CapabilityFnDecl := 'fn' Ident GenericParams? ParamGroup+ ReturnType? EffectClause?
EffectClause := 'effects' '{' CapabilityEffect* '}'
```

### File system

```arcw
extern capability fs {
    type FsError

    fn read_text(path: VirtualPath) -> Need<String, FsError>
        effects { fs.read }

    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError>
        effects { fs.write }
}
```

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

    fn exit(code: Int) -> Never
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

