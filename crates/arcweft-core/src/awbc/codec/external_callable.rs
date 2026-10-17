//! Allocation-budgeted external declaration linkage in the version-one codec.

use super::{
    AwbcCodecError,
    wire::{Reader, Wire, Writer},
};
use crate::{
    entry::RuntimeCallableId,
    value::{
        RuntimeExternalCallable, RuntimeExternalCallableDeclaration, RuntimeExternalCallableExport,
        RuntimeExternalCallableProvider, RuntimeStandardCallableProvider,
    },
};

impl Wire for RuntimeExternalCallable {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.identity().write_wire(writer)?;
        match self.declaration().provider() {
            RuntimeExternalCallableProvider::Standard(provider) => {
                0_u8.write_wire(writer)?;
                provider.semantic_tag().write_wire(writer)?;
            }
            RuntimeExternalCallableProvider::Adapter(package) => {
                1_u8.write_wire(writer)?;
                package.write_wire(writer)?;
            }
        }
        match self.declaration().export() {
            RuntimeExternalCallableExport::Function(path) => {
                0_u8.write_wire(writer)?;
                writer.write_table(path)?;
            }
            RuntimeExternalCallableExport::Method { receiver, name } => {
                1_u8.write_wire(writer)?;
                receiver.write_wire(writer)?;
                name.write_wire(writer)?;
            }
            RuntimeExternalCallableExport::RustFunction(path) => {
                2_u8.write_wire(writer)?;
                writer.write_table(path)?;
            }
        }
        self.declaration().overload().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let identity = RuntimeCallableId::read_wire(reader)?;
        let offset = reader.offset();
        let provider = match u8::read_wire(reader)? {
            0 => {
                let tag = u8::read_wire(reader)?;
                RuntimeExternalCallableProvider::Standard(
                    RuntimeStandardCallableProvider::from_semantic_tag(tag).ok_or(
                        AwbcCodecError::UnknownTag {
                            kind: "standard callable provider",
                            tag,
                            offset,
                        },
                    )?,
                )
            }
            1 => RuntimeExternalCallableProvider::Adapter(String::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "external callable provider",
                    tag,
                    offset,
                });
            }
        };
        let offset = reader.offset();
        let export = match u8::read_wire(reader)? {
            tag @ (0 | 2) => {
                let path = reader
                    .read_table::<String>(
                        "external callable path",
                        reader.budget().collection_items,
                    )?
                    .into_boxed_slice();
                if tag == 0 {
                    RuntimeExternalCallableExport::Function(path)
                } else {
                    RuntimeExternalCallableExport::RustFunction(path)
                }
            }
            1 => RuntimeExternalCallableExport::Method {
                receiver: <[u8; 32]>::read_wire(reader)?,
                name: String::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "external callable export",
                    tag,
                    offset,
                });
            }
        };
        let overload = u16::read_wire(reader)?;
        let declaration = RuntimeExternalCallableDeclaration::try_new(provider, export, overload)
            .map_err(|error| AwbcCodecError::InvalidMetadata {
            kind: "external callable declaration",
            message: error.to_string(),
            offset,
        })?;
        Ok(Self::new(identity, declaration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{awbc::codec::AwbcDecodeBudget, value::RuntimeCallTarget};

    #[test]
    fn external_callable_codec_retains_checked_identity_provider_kind_and_overload() {
        use RuntimeStandardCallableProvider as Standard;
        let providers = [
            Standard::Core,
            Standard::SansIo,
            Standard::NativeHttp,
            Standard::InferenceTensor,
            Standard::SystemInfo,
            Standard::NativeFile,
            Standard::Math,
        ]
        .map(RuntimeExternalCallableProvider::Standard)
        .into_iter()
        .chain([RuntimeExternalCallableProvider::Adapter(
            "third-party".into(),
        )]);
        for provider in providers {
            for export in [
                RuntimeExternalCallableExport::Function(["api".into(), "run".into()].into()),
                RuntimeExternalCallableExport::RustFunction(["rust".into(), "run".into()].into()),
                RuntimeExternalCallableExport::Method {
                    receiver: [0x84; 32],
                    name: "run".into(),
                },
            ] {
                let target = RuntimeCallTarget::External(RuntimeExternalCallable::new(
                    RuntimeCallableId::from_checked_digest([0x91; 32]),
                    RuntimeExternalCallableDeclaration::try_new(provider.clone(), export, 37)
                        .unwrap(),
                ));
                let mut writer = Writer::default();
                target.write_wire(&mut writer).unwrap();
                let bytes = writer.into_bytes();
                let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
                assert_eq!(RuntimeCallTarget::read_wire(&mut reader).unwrap(), target);
                assert_eq!(reader.offset(), bytes.len());
                assert_eq!(
                    serde_json::from_str::<RuntimeCallTarget>(
                        &serde_json::to_string(&target).unwrap()
                    )
                    .unwrap(),
                    target
                );
            }
        }
    }

    #[test]
    fn external_callable_codec_rejects_unknown_provider_and_preflights_export_storage() {
        let target = RuntimeExternalCallable::new(
            RuntimeCallableId::from_checked_digest([0x92; 32]),
            RuntimeExternalCallableDeclaration::try_new(
                RuntimeExternalCallableProvider::Standard(
                    RuntimeStandardCallableProvider::InferenceTensor,
                ),
                RuntimeExternalCallableExport::Function(["infer".into(), "add_f32".into()].into()),
                0,
            )
            .unwrap(),
        );
        let mut writer = Writer::default();
        target.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let budget = AwbcDecodeBudget {
            collection_items: 1,
            ..AwbcDecodeBudget::default()
        };
        assert!(matches!(
            RuntimeExternalCallable::read_wire(&mut Reader::new(&bytes, &budget)),
            Err(AwbcCodecError::BudgetExceeded { .. })
        ));
        let mut prefix = Writer::default();
        target.identity().write_wire(&mut prefix).unwrap();
        let mut invalid = bytes;
        invalid[prefix.len()] = 0xff;
        assert!(matches!(
            RuntimeExternalCallable::read_wire(&mut Reader::new(
                &invalid,
                &AwbcDecodeBudget::default()
            )),
            Err(AwbcCodecError::UnknownTag {
                kind: "external callable provider",
                tag: 0xff,
                ..
            })
        ));
        let mut json = serde_json::to_value(&target).unwrap();
        json["declaration"]["export"]["Function"] = serde_json::json!([]);
        assert!(serde_json::from_value::<RuntimeExternalCallable>(json).is_err());
    }
}
