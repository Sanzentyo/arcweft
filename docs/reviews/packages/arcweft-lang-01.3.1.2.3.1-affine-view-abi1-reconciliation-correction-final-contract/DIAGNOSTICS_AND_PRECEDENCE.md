# Diagnostics and failure precedence

## New or corrected diagnostics

| Code | Meaning |
|---|---|
| `runtime.restore.execution_already_active` | another driver in the same execution domain owns the execution lease |
| `runtime.restore.activation_holder_mismatch` | replacement driver does not own the exact current lease |
| `runtime.restore.affine_allocator_cursor_invalid` | execution/cursor/order/exhaustion evidence invalid |
| `runtime.restore.affine_allocator_cursor_reuse` | first post-restore mint would reuse an issued ordinal |
| `sema.view.value.affine_not_allowed` | retained/render View boundary type is affine or may contain an affine leaf |
| `compiler.view.input.transfer_invalid` | role/source/transfer combination is not one of the closed accepted combinations |
| `bundle.view.input.ownership_mismatch` | serialized input ownership disagrees with runtime type/layout or AWBC function facts |
| `save.view.affine_retained_value` | tampered retained View slot contains affine evidence |
| `bundle.view.static.requirement_missing_certificate` | authored requirement has no exact certificate/fragment |
| `bundle.view.static.authored_origin_without_requirement` | authored origin has no serialized requirement |
| `bundle.view.static.requirement_mismatch` | subject/digest/origin/revision mismatch |
| `bundle.view.static.fragment_partial_overlap` | certified spans cross without strict containment |
| `runtime.view.static.fragment_dispatch_invalid` | runtime catalog violates validated ancestry/outermost rule |

## First-failure order

1. envelope/version/canonical framing;
2. exact artifact and ABI-1/codec-8 identity;
3. global limits and unknown/duplicate fields;
4. runtime type/layout and static ownership recomputation;
5. allocator snapshot;
6. owner/token/Stream reciprocity;
7. View value role/source/transfer/ownership;
8. static requirement row;
9. fragment/certificate digest and span ancestry;
10. save generation/resource joins;
11. activation-domain holder/epoch recheck;
12. publication.

Within one class, canonical typed path/order selects the first error. No hash-map iteration or aggregate diagnostic set may affect result.
