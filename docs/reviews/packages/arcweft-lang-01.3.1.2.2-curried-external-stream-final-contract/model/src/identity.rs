pub const AWBC_ABI_VERSION: u32 = 2;
pub const AWBC_CODEC_VERSION: u16 = 8;
pub const SAVE_SCHEMA_VERSION: u32 = 2;
pub const AWBC_RUNTIME_TYPE_STREAM_HANDLE: u8 = 21;
pub const AWBC_RUNTIME_TYPE_EXTERNAL_STREAM_CALLABLE: u8 = 22;
pub const AWBC_CONSTANT_EXTERNAL_STREAM_CALLABLE: u8 = 18;
pub const AWBC_OPCODE_APPLY_EXTERNAL_STREAM_GROUP: u8 = 0x27;
pub const AWBC_OPCODE_OPEN_STREAM: u8 = 0x28;

macro_rules! digest_newtype {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub [u8; 32]);
    };
}

digest_newtype!(DefinitionId);
digest_newtype!(DeclarationDigest);
digest_newtype!(SignatureFingerprint);
digest_newtype!(DefaultFingerprint);
digest_newtype!(TypeLayoutHash);
digest_newtype!(ValueDigest);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GroupIndex(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParameterIndex(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Coordinate {
    pub group: GroupIndex,
    pub parameter: ParameterIndex,
}

impl Coordinate {
    #[must_use]
    pub const fn new(group: u16, parameter: u16) -> Self {
        Self {
            group: GroupIndex(group),
            parameter: ParameterIndex(parameter),
        }
    }
}
