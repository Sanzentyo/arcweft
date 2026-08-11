# Tier-2 and cross-consumer closure matrix

The `T2-*` rows in `TEST_MATRIX.csv` are normative. This focused index records the
minimum consumer surfaces that must all use one accepted View runtime catalog.

| Surface | Required fixture | Required equality/failure evidence |
|---|---|---|
| Compiler | all current final-HIR execution shapes, constants, programs, defaults, exports, resources | one atomic product; no `MissingCheckedViewProjection`; exact source map |
| Bundle | AWVP + ViewText + Input + Style + resource + AWBC references | canonical bytes, strict unknown/duplicate/order rejection, digest/tamper closure |
| Runtime driver | constants, ordinary AWBC, direct await, handlers, parts, resources, certificates | one evaluator, transactional frame and lifecycle state |
| Headless | forced dynamic versus certified fixtures | canonical frame/input/observation/save equality |
| Native | text/input/layout/image/animation and errors | shared renderer-neutral frame and exact input outcomes |
| Web | the same fixture in browser runtime | no CSS/Takumi path; byte/semantic parity with Native/headless |
| Agent | frame/resource/input observations | redacted exact observation, no endpoint catalog/static-path bit |
| MCP | resource/semantic/part observations | same typed IDs and failures as Agent/runtime |
| Save/replay | dynamic/certified mounts, await, repeat, input, animation | schema-v2 semantic state only; path-independent bytes/result |
| Hot replacement | unchanged/changed/tampered program, proof, resource, export, parameter | classify from exact digests; stale/tampered candidate leaves old state complete |
| Generated artifacts | View node/program/certificate semantic binding | exact generation/artifact correlation; stale binding fails closed |
| Metadata/audit | normal dependency graph and changed/largest files | no lower-layer cycle or endpoint authority; zero structural errors |

Tier-2 is mandatory because this design changes compiler product, runtime execution,
input behavior, resources/animation, native/Web/headless/Agent/MCP observation,
save/replay, and hot replacement. A fast workspace suite alone does not close the
boundary.
