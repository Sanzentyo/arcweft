# AWFT entries, capabilities, virtual paths, and host effects

This document proposes grammar changes that should land destructively. No legacy spellings, aliases, or migration wrappers are added.

## Entry declarations

`flow` remains the core story/control unit, but it is no longer the program entry point. Entry points are explicit.

```text
EntryDecl := Visibility? 'entry' EntryKind EntryId? EntryBlock
EntryKind := 'game' | 'cli' | 'server' | 'activity' | 'test' | 'bench' | Ident
EntryId   := EntityRef | RelativeId | FamilyRelativeEntityRef
EntryBlock := '{' EntryItem* '}'
EntryItem := 'start' EntityRef | 'run' EntityRef | RouteDecl | EntryOption
RouteDecl := 'route' HttpMethod String '->' EntityRef
HttpMethod := 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS'
```

### Game entry

```awft
entry game @entry.main {
    start @flow.opening
}

flow @flow.opening opening {
    log.info("game started")
    return "ok"
}
```

### CLI entry

```awft
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

```awft
entry server @entry.http {
    route GET "/health" -> @flow.health
    route GET "/hello/:name" -> @flow.hello
}

flow @flow.health(req: HttpRequest) -> HttpResponse effects { http.respond } {
    return http.text(status = 200, body = "ok")
}
```

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

```awft
extern capability fs {
    type FsError

    fn read_text(path: VirtualPath) -> Need<String, FsError>
        effects { fs.read }

    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError>
        effects { fs.write }
}
```

### Path virtualization

```awft
extern capability path {
    fn save(path: String) -> VirtualPath
    fn asset(path: String) -> VirtualPath
    fn temp(path: String) -> VirtualPath
    fn export(path: String) -> VirtualPath
}
```

Forbidden in AWFT source:

```awft
fs.read_text("/Users/me/profile.json")
fs.read_text("C:\\Users\\me\\profile.json")
```

Required:

```awft
fs.read_text(path.save("profile.json"))
```

### Stdio and process

```awft
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

```awft
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

```awft
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

```awft
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

Release builds may reject undeclared capability effects.
