# Deterministic error precedence

## Catalog admission

1. catalog-local syntax and structural validation;
2. canonical identity/order/duplicate/limit checks;
3. canonical transcript construction under the fixed version-1 domain;
4. digest recomputation;
5. declared-versus-actual digest comparison;
6. generation identity comparison;
7. referenced View/character relationship checks;
8. atomic admitted-wrapper publication.

Character validation precedes View validation because `AdmittedGenerationCatalogs::try_admit` receives Character first and the request lists catalog-local failures before any cross-catalog relationship. Within each catalog, row order and field order are those defined in Decisions 01 and 02.

## Checked value and Choice

1. nesting and shared work budget;
2. outer runtime shape;
3. checked owner or integer width;
4. Variant owner;
5. Variant ordinal and name;
6. payload presence;
7. recursive payload;
8. every Choice alternative in source order under the same budget;
9. zero-match ordered branch evidence or first two matching indices;
10. nominal admitted-shape lookup and defining-order tree validation;
11. domain publication.

## `MakeRecord`

1. AWBC structural instruction/type/reference checks;
2. admitted generation and exact execution-site root coordinate;
3. project/producer domain selection;
4. exact nominal/semantic/layout lookup;
5. domain authorization membership;
6. field count, one-based field IDs, and checked values in defining layout order;
7. crate-private checked construction;
8. destination-register publication.

No later error is observable after an earlier failure; patch/restore/hot-swap candidates publish atomically.
