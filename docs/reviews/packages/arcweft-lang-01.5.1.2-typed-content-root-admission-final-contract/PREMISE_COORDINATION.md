# Coordination with the prior Lang-01.4.2 package

Prior archive SHA-256: `01f308c08fe818e247e41e94278eb2d69d5a12ac597794a9109390840c0d95d3`.

The archive was inspected as conversational premise. Its own `FINAL_CONTRACT.md` identifies itself as a generation fallback with no repository pin and no repository-specific wire decisions. Therefore this package inherits only the explicit coordination boundary and prohibitions, not unsupported data-shape claims.

For configured-resource content roots, the normative dependency is the **actual accepted current-repository product**:

1. a strict Lang-01.4.2 extension-manifest admission path publishes an immutable `ResourceTypeRegistry`;
2. accepted `res` declarations publish exact `ResourceDeclarationIdentity` values;
3. this contract resolves a non-built-in root only through that accepted registry/declaration index;
4. it never reads an extension manifest, accepts a raw family label as a declaration, widens `ResourceRef`, or introduces a second registry.
