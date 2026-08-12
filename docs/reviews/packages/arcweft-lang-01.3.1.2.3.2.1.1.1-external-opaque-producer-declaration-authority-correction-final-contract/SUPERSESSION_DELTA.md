# Narrow supersession delta against Lang-01.3.1.2.3.2.1.1

The parent remains normative except for these declaration/publication details:

1. producer-bearing accepted opaque rows are now fed by mandatory external
   descriptor evidence rather than an implementation-local assumption;
2. adapter-native and Rust-export producers have separate lower-layer validated
   newtypes and strict schema-2 wire fields;
3. adapter-sema owns the sole conversion/source boundary;
4. environment manifest and accepted catalog digest domains move to v2;
5. generated registration source moves to `adapter-manifest-v2` and carries
   producer payload ranges;
6. parent A1.2 is blocked until this package's G1–G5 gates complete.

Nothing in this package changes the parent's `RuntimeOpaqueTypeProducerId`,
`RuntimeOpaqueTypeAdmission`, `RuntimeOpaqueTypeOwner`, `RuntimeOpaqueValue`,
complete composite ownership, exact/producer-wide relation, fixed `std.*`
producers, checked-type paths/errors, ABI 1, codec 11, tags 16/23/18, save
schema 3, record layout, identity/slot/path, activation, View, or Stream.
