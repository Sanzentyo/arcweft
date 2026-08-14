# Newtype and Serde grammars

`NEWTYPE_SERDE_GRAMMARS.csv` is normative. Operational admitted/borrowed types are not wire types. Raw wire DTOs are private, deserialize into candidates, invoke the final checked constructor, and never publish partially validated values. Table bounds that require an owner are checked during that owner's admission before any operational wrapper is issued.
