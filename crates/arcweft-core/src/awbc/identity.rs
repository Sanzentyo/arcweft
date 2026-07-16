//! Canonical executable identity for verified AWBC programs.

use super::codec::AwbcCodecError;
use super::schema::{AwbcDigest, AwbcProgram};

impl AwbcProgram {
    /// Returns a stable digest of every table that can affect Product AWBC
    /// execution.
    ///
    /// Source maps and display-map links are diagnostic and presentation
    /// metadata. They are intentionally excluded so source relocation and
    /// content-catalog refreshes do not masquerade as executable changes.
    /// Strings referenced only by excluded metadata are removed before the
    /// canonical encoding is hashed, which also makes the identity independent
    /// of their dense string-table positions.
    pub fn executable_identity(&self) -> Result<AwbcDigest, AwbcCodecError> {
        let mut executable = self.clone();
        executable.canonicalize_string_table();
        for block in &mut executable.blocks {
            block.source_map = None;
        }
        for content in &mut executable.content_units {
            content.display = None;
            content.source = None;
        }
        executable.display_map.clear();
        executable.source_map.clear();
        executable.retain_referenced_strings();
        executable
            .encode_canonical()
            .map(|bytes| AwbcDigest(*blake3::hash(&bytes).as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcCodeLocation, AwbcConstant, AwbcFunctionId, AwbcSafePointKind,
        AwbcSourceMapEntry, AwbcSourceMapId, AwbcStringId, AwbcTableRange, AwbcTerminator,
    };

    #[test]
    fn executable_identity_ignores_source_map_data_and_dense_string_positions() {
        let first = AwbcProgram {
            strings: vec!["source/a.arcw".to_owned()],
            source_map: vec![AwbcSourceMapEntry {
                location: AwbcCodeLocation::Block(AwbcBlockId(0)),
                source_file: AwbcStringId(0),
                start: 0,
                end: 4,
                anchor: None,
            }],
            blocks: vec![AwbcBlock {
                owner: AwbcFunctionId::default(),
                instructions: AwbcTableRange::default(),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::Return,
                source_map: Some(AwbcSourceMapId(0)),
            }],
            ..AwbcProgram::default()
        };

        let mut second = first.clone();
        second.strings = vec!["moved/source/b.arcw".to_owned()];
        second.source_map[0].source_file = AwbcStringId(0);
        second.source_map[0].start = 100;
        second.source_map[0].end = 140;

        assert_eq!(
            first.executable_identity().expect("first identity"),
            second.executable_identity().expect("second identity")
        );
    }

    #[test]
    fn executable_identity_includes_constant_pool_values() {
        let first = AwbcProgram {
            strings: vec!["alpha".to_owned()],
            constants: vec![AwbcConstant::String(AwbcStringId(0))],
            ..AwbcProgram::default()
        };

        let mut second = first.clone();
        second.strings = vec!["beta".to_owned()];
        second.constants[0] = AwbcConstant::String(AwbcStringId(0));

        assert_ne!(
            first.executable_identity().expect("first identity"),
            second.executable_identity().expect("second identity")
        );
    }
}
