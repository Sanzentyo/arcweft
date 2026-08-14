# Retained `.1.2.1` substrate

The following parent decisions are incorporated without a parallel model:

- one mandatory serialized `RuntimeGenerationContractDeclaration` shared by raw `RuntimePlan` and `AwbcProgram`;
- one non-Serde `AdmittedRuntimeGeneration` reused by admitted plan and plan-paired AWBC, with standalone AWBC admitted from its complete embedded declaration;
- raw plan/program quarantine until whole-artifact admission;
- `RuntimeGenerationIdentity = BLAKE3("arcweft.runtime-generation-contract.v1\0" || canonical_body_v1)`;
- project and producer root declarations sorted by their typed 32-byte IDs;
- independent producer roots and exact derived-versus-claimed nominal authorization equality;
- `RuntimeCharacterCatalogDigest`, `RuntimeViewCatalogDigest`, and computed `CharacterDialogueRuntimeCustomFieldDigest` in the existing generation body;
- CharacterDialogue producer exactly `std.character_dialogue`, derived Style, nested Option/voice representation, and generation-bound Character/View wrappers;
- producer lookup returns a borrowed admitted-shape view, not an exclusive credential;
- `RuntimeNominalRecordValue::try_from_accepted_layout` remains crate-private;
- canonical checked-type tags remain: Never 00, Unit 01, Bool 02, Signed 03, Unsigned 04, F32 05, F64 06, String 07, Char 08, Duration 09, EntityRef 0a, Bytes 0b, Sequence 10, Tuple 11, Choice 12, Nominal 13, Opaque 14, Variant 15, Result 16, Option 17;
- common scalar grammar remains `u8` tags/bools/options and little-endian `u32` lengths/counts/indices;
- custom-field digest domain remains `arcweft.character-dialogue-runtime-custom-fields.v1\0`, derived from ordered field ID, exact checked type, clearability, and accepted `RuntimeViewId` values;
- all Arcweft-owned versions remain exactly 1.

The child definitions in this package fill the missing catalog transcripts, role rows, root-site correlation, construction domain, and typed validation/error boundaries. They do not alter the parent generation body or introduce another digest relation.
