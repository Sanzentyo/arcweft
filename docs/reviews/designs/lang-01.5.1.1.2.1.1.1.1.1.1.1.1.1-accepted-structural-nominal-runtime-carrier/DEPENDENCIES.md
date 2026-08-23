# Owners, consumers, and dependency direction

## Crate direction

The selected path preserves the current Cargo graph:

    arcweft-core
       ^        ^
       |        |
    arcweft-lang-sema   arcweft-runtime-plan
       ^                 ^
       +------ arcweft-compiler

Adapter-sema depends on sema and supplies publication inputs. Runtime-driver
depends on core and consumes the AWBC snapshot API. Core does not import sema,
compiler, runtime-plan, adapter, or driver. Runtime-plan does not import sema
or compiler.

The core-owned RuntimeNominalSchemaGraph is an inert typed proof. Sema
constructs it, compiler transports it after stamp validation, and the existing
core RuntimePlanBuilder consumes it into the existing type/domain tables.
The graph is then dropped. It is not a fourth runtime table.

## Owner/consumer matrix

| Fact | Sole owner | Producers | Consumers | Forbidden duplicate |
|---|---|---|---|---|
| nominal identity/arity/role | sema AcceptedNominalCatalog | registration | resolver, final projection | metadata-only lookup |
| Rust order/templates | sema AcceptedRustTypeMetadataCatalog | adapter-sema/registration | final projection | compiler metadata copy |
| joined generation | sema RegisteredTypeCheckEnv/FinalSemanticAnalysis | registrar/analyzer | compiler, classifier | digest fallback |
| instantiated identity | existing TypeKind transcript | sema | schema graph, compiler | source-derived ID |
| schema/layout | core RuntimeNominalSchemaGraph/TypeLayoutHash | final analysis | compiler, plan, differential tests | layer-local hash helper |
| executable types | core RuntimePlanTypeTable | plan builder | evaluator, AWBC | retained schema side table |
| record fields | core record domain/layout | graph/plan builder | constructors, selectors, AWBC | name-to-ordinal map |
| variant cases | core variant domain/variant_case | graph/plan builder | Match, constructors, AWBC | copied ordinal table |
| live record | core RuntimeValue::NominalRecord | checked constructors | evaluator, digest, snapshot | carrier wrapper |
| live variant | core RuntimeValue::Variant | checked constructors | evaluator, Match, snapshot | second enum type |
| AWBC rows/tags | core AwbcProgram/codec | AWBC lowerer | verifier, VM, restore | adapter tag registry |
| snapshot DTO | core AwbcRuntimeValueSnapshot | AwbcProgram walk | candidate restore | generic serde authority |

## Exact cross-crate reachability

1. Adapter-sema calls only typed sema publication constructors.
2. Registrar constructs both immutable catalogs and proves the bijective join
   before RegisteredTypeCheckEnv exists.
3. Final analysis borrows RegisteredSemanticWorld and calls the public
   validated core schema-graph constructor.
4. Compiler validates the private-field sema projection and hands its borrowed
   core graph to runtime-plan staging; it never rebuilds names.
5. Runtime-plan facts retain core IDs, layouts, ordered fields/cases, and HIR
   provenance only where project-owned. They cannot name sema types.
6. Core plan admission atomically rewrites semantic references to plan-local
   IDs and discards the graph.
7. AWBC lowering reads only sealed RuntimePlan tables. Restore reads only the
   current AwbcProgram. Neither reaches upward to sema.

## Consumer completeness

Implementation updates all current call sites in adapter-sema publication;
sema registration/resolution/metadata/final-analysis/ownership; compiler type,
select, construction, pattern, constant, and variant lowering; runtime-plan
semantic facts/native lowering/AWBC lowering; core checked types, Match,
nominal records, pure runtime, canonical identity, and RuntimeValue variant
sites; AWBC schema/codec/verifier/VM/fiber/type projection/product/task
snapshot sites; and runtime-driver candidate restore.

A compile-clean cut cannot leave an old constructor, old enum spelling, copied
field/case name, context-free snapshot conversion, or structural fail-closed
success branch behind.
