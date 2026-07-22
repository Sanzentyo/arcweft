//! Structural promotion of expression paths into validated type paths.

use crate::{
    ast::{
        module_path::ModulePathRoot,
        symbol_path::{ProjectSymbolPath, ProjectSymbolPathError, ProjectSymbolSegment},
    },
    expr::DottedPath,
    types::TypePath,
};

impl TryFrom<&DottedPath> for TypePath {
    type Error = ProjectSymbolPathError;

    fn try_from(path: &DottedPath) -> Result<Self, Self::Error> {
        let segments = path.segments();
        let (root, first_symbol) = match segments.first().map(crate::expr::Name::as_str) {
            Some("crate") => (ModulePathRoot::Crate, 1),
            Some("self") => (ModulePathRoot::SelfModule, 1),
            Some("super") => {
                let levels = segments
                    .iter()
                    .take_while(|segment| segment.as_str() == "super")
                    .count();
                (ModulePathRoot::Super(levels), levels)
            }
            Some(_) => (ModulePathRoot::ImplicitCrate, 0),
            None => return Err(ProjectSymbolPathError::Empty),
        };
        let symbols = segments[first_symbol..]
            .iter()
            .map(|segment| ProjectSymbolSegment::try_new(segment.as_str().to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        ProjectSymbolPath::new(root, symbols).map(TypePath::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_expression_paths_promote_without_reparsing_display_text() {
        let cases = [
            ("Record", ModulePathRoot::ImplicitCrate, vec!["Record"]),
            (
                "crate.models.Record",
                ModulePathRoot::Crate,
                vec!["models", "Record"],
            ),
            ("self.Record", ModulePathRoot::SelfModule, vec!["Record"]),
            (
                "super.super.Record",
                ModulePathRoot::Super(2),
                vec!["Record"],
            ),
        ];
        for (source, expected_root, expected_segments) in cases {
            let path = DottedPath::parse_dotted(source);
            let promoted = TypePath::try_from(&path).expect(source);
            assert_eq!(promoted.root(), expected_root, "{source}");
            assert_eq!(
                promoted
                    .segments()
                    .iter()
                    .map(ProjectSymbolSegment::as_str)
                    .collect::<Vec<_>>(),
                expected_segments,
                "{source}"
            );
        }
    }

    #[test]
    fn root_only_expression_paths_do_not_fabricate_type_symbols() {
        for source in ["crate", "self", "super", "super.super"] {
            let path = DottedPath::parse_dotted(source);
            assert!(TypePath::try_from(&path).is_err(), "{source}");
        }
    }
}
