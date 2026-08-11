# Image, resource, and animation contract

## Classification

`Image` is not a View constructor and is not added to `ViewElementKind`. The
canonical rendering operation remains `ViewInstruction::EmitImage`. The authored
value is a typed `ResourceRef<Image>` (or an expression returning that exact type)
and is represented in product data by `ViewImageBindingResource`.

## Identity reaching runtime

Runtime receives and validates:

1. `EntityId` of the accepted `res` declaration;
2. its public `PublicId`;
3. exact `ResourceTypeId` for Image;
4. resource declaration digest;
5. image descriptor digest;
6. `ResourceTypeRegistryDigest`;
7. active bundle artifact/generation;
8. generated decoded image-object identity.

The first three are the existing `ResourceRefValue`. The remaining fields bind it
to the exact compiled artifact. An asset path, source spelling, URL, ordinal, or
untyped image ID cannot substitute.

## Static and dynamic selection

- Constant reference: product stores `ViewResolvedResourceRef` and runtime acquires
  it directly.
- Program reference: AWBC returns the accepted nominal `ResourceRef<Image>`;
  `ViewValueProjection::ResourceRef` verifies exact type/layout and joins to the
  validated resolved-resource table.
- Mismatched resource family/type fails at semantic or product validation.
- A runtime value naming a resource absent from the candidate artifact fails before
  candidate frame publication.
- A reference bound to an old registry/declaration/descriptor digest fails stale.

## Formats and animation

- PNG: current still image.
- JPEG: current still image.
- GIF: current image resource; animated when the decoded descriptor contains an
  animation timeline.
- WebP: current image resource; still or animated according to the decoded
  descriptor.
- APNG: not accepted by the current image owner and not added here. It fails the
  typed resource/image decoder, not View parsing.

The decoder's typed animation descriptor owns frame durations, loop behavior, and
frame count. View does not inspect file suffixes. Runtime uses the session logical
clock and deterministic animation cursor. Static certification may freeze which
resource is selected but may not freeze or remove animation time, frame selection,
resource lease, visibility, or save/replay behavior.

## Immutability proof

A resource contributes `ImmutableResource` evidence only when the accepted
resource registry proves that the declaration value, descriptor, payload digest,
and relevant variant selection are immutable for the artifact generation. Merely
being a constant source expression is insufficient. The certificate includes the
resource identity and all digests. Hot replacement invalidates the certificate if
any digest changes.

## Rejected alternatives

- Adding `Image` to `ViewElementKind` would duplicate the typed resource owner.
- Treating every resource as `Presentable` guesses a trait contract not selected by
  the resource system.
- Converting a resource to String or an ordinal and looking it up at runtime loses
  type and generation identity.
- Taking the current static image field when a dynamic expression is present
  changes authored semantics.
