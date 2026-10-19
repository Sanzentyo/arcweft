# Product AWFB bytecode after seq-01.5

New product `.awfb` files use `executable_payload = "awbc_v1"` in the product manifest and store `ProgramBytecode` as canonical AWBC bytes produced by `AwbcProgram::encode_canonical()`. Structured `BytecodeProgram` remains an inspection/export surface only and is not a product runtime payload. Old AWFB product bytecode wrappers that embed structured MessagePack or compact sidecars are rejected by product decoders with typed diagnostics.


## Producer contract after seq-01.6

Normal source, project, profile, watch, native-run, and web-run product builds
lower canonical AWBC in the compiler-side runtime-plan lowering layer before
bundle encoding. The shared builder attaches the resulting `AwbcProgram`
through `ArcweftBundle::with_product_awbc`; the bundle codec does not depend on
compiler, filesystem, host, signing, clock, or platform crates.

AWBC lowering diagnostics are reported at the product build phase. The codec's
missing-AWBC diagnostic remains a defensive invariant for manually constructed
bundles, not the normal producer error path. Source/dev execution may continue
to use structured bytecode, but an `.awfb` Game product has no structured
executable fallback.

## Original program Source

Compiler-side AWBC lowering retains the complete original RuntimePlan Source,
including all executable inventories and original Task definitions, even for an
entry-selected artifact. It never reconstructs that Source from emitted bytecode.
`RuntimeTaskSourceArtifact` owns immutable shared bytes and an untrusted original
executable E assertion. Cloning retains the same packet without copying its bytes.

Canonical AWBC v1 writes the Source option immediately after its fixed header.
A present Source writes the 32 E assertion bytes followed by a canonical length
and the strict version-one complete Task Source frame. The binary reader checks
the declared byte bound and preflights the borrowed complete Source grammar and
construction footprint before retaining its bytes or reading later AWBC tables.
Line handle producer storage is bounded by the owning encoded handle domain's
actual producer labels; the general string limit does not multiply that footprint.
The borrowed site kind determines that storage count and checks the producer
limit during preflight. A more permissive limit does not increase the count.
Original type, producer, value class, persistence and site shape are checked
before copying a derived opaque site owner.
The unadmitted schema represents absent Source explicitly; absence grants no
original-program or Task authority. Ordinary compiler lowering attaches Source.

Source transport and wire preflight do not issue a key. Original Source admission
uses one constrained semantic meter and one retained-value allowance across
preflight, genuine namespace construction and the common seal. That seal compares
the recomputed E before completing or publishing Task rows, compares each Task
assertion, and requires the actual upper View authority for View-bound rows.
The completed plan retains that seal's original construction namespace. A Task
assertion resolves only an already issued row and its actual function endpoint;
it does not recover a namespace from request metadata. Executable data equality
does not transfer a program's Source authority.
Execution and restore must bind their original Task rows and exact program lease
through this admitted Source; copying an asserted digest is not admission.

AWBC executable identity includes the original E assertion, which must pass that
admission, rather than raw Source wire spelling. Source bytes preserve diagnostic
labels in addition to semantic declarations. An identity over assertion data is
not validation evidence for malformed Source or a forged E assertion.

## Function input semantic origins

Each positional AWBC function input retains a mandatory `RuntimeLocalOrigin`
alongside its source role, transfer operation, pattern, and ownership requirement.
Binding coordinates, whole-formal identities, evaluated-result definitions, and
generated local coordinates remain distinct origin domains. Canonical AWBC v1
encodes them with tags `0`, `1`, `2`, and `3`, each followed by exactly 32 identity
bytes. Decoders reject unknown tags, truncated identities, and missing origins;
there is no default identity or legacy reader.

Supplied and captured whole formals require a parameter origin. Generated
formatter, TaskRequest, and Line thunks preserve the captured Core declaration's
origin, including a whole formal or generated coordinate, while recording the
actual capture transfer separately. Table positions, frame registers, closed
types, runtime values, and debug labels do not replace semantic origins.

The input-origin metadata transports identity. It does not by itself prove the
complete function body transcript or whole-image seal.

## Function definition identities

Every AWBC function retains a mandatory `RuntimeFunctionDefinitionIdentity`.
Canonical AWBC v1 writes its fixed 32 identity bytes immediately after the
semantic-role tag. Binary truncation and missing JSON definitions are rejected;
there is no default definition or legacy reader.

Ordinary functions, helpers, methods, and Flow functions preserve the accepted
Core definition. Formatter and TaskRequest recipes have an accepted expression
owner. Generated Line functions have the accepted Line group as their parent.
The owning typed recipe derives a child definition from its parent and its
closed purpose: formatter parameter, TaskRequest, Line activation, source node,
source cancellation rule, or cleanup exit. Source ordinals refer to the parent's
declaration graph before AWBC allocation. Closed types, body bytes, frame and
function table positions, and debug labels do not rename that definition.

Definition identity is distinct from the actual body transcript and complete
function/image seal. Transporting it through a codec is not admission evidence.
