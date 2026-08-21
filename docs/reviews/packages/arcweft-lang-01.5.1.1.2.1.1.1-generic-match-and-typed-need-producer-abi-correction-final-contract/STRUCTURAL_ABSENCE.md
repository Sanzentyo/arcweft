# Structural absence contract

Implementation acceptance must prove these shapes absent from production APIs/AST/schema/generated artifacts/codecs/docs after final cut.

## View boundary

- no AwbcRegisterId, AwbcTypeId, RuntimeValue, or copied core type/digest map in arcweft-view Match rows;
- no public ViewMatchSelection containing values;
- no arcweft-core dependency in arcweft-view Cargo.toml;
- no View VM or presentation-value Match fallback.

## Match authority and projection

- no live Match retaining CheckedExpressionResolution::Structural;
- no duplicate arm/binding/type/effect arrays in CheckedViewCatalog;
- no duplicate TypeKind/effect/per-arm-coverage fields inside CheckedMatch;
- no arm_expression, source-range arm identity, or inferred-type TypeId in normative Match facts;
- no runtime-plan dependency on sema/View/bundle and no selector builder accepting CheckedMatch directly;
- no generated selector using AwbcTerminator::Match or AwbcMatchArm.guard;
- no unresolved ownership/runtime type placeholder.

## Need authority

- no payloadless AwbcRuntimeType::NeedHandle;
- no RuntimeUnsupportedTypeShape::Need after direct RuntimeCheckedType projection;
- no RuntimeValue::String branch accepted for NeedHandle;
- no String-to-NeedId conversion in await_target or equivalent;
- no AwbcTaskPlan.need_id or NamedTaskSpec.need_id;
- no second endpoint/resolver table;
- no generic serde path fabricating a live Need handle;
- no source/string/mount/observer identity in NeedId;
- no old View Await authority/readers/save rows.

## Bundle/wire/version discipline

- no omitted state/result/payload digest or source-role table in ViewReactiveBindingSectionV1;
- no noncanonical/unknown/duplicate section row accepted;
- no Arcweft-owned marker other than 1;
- no compatibility reader, alias, old/new discriminator, migration fallback, or dual model;
- no dependency cycle or forbidden edge;
- no extension trait used instead of inherent Arcweft-owned enum implementation;
- no production TODO/TBD/FIXME/placeholder owner/empty catalog for this feature.

`tools/validate_package.py` checks corresponding design-level absences. Implementation CI must add source/API/schema/generated searches named in structural test rows.
