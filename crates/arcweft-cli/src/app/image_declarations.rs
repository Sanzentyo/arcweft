use arcweft_lang_syntax::{
    ast::items::{EntityDeclKind, ImageDeclBody, Item},
    parser::parse_source,
};
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceRange,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct DeclaredImageObject {
    id: String,
    args: Vec<String>,
    declaration_anchor: SourceRange,
}

impl DeclaredImageObject {
    pub(in crate::app) fn id(&self) -> &str {
        &self.id
    }

    pub(in crate::app) fn args(&self) -> &[String] {
        &self.args
    }

    pub(in crate::app) fn missing_asset_diagnostic(&self, document: &SourceDocument) -> Diagnostic {
        let diagnostic = Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("image object `{}` is missing an asset reference", self.id),
        )
        .with_code("bundle.image.missing_asset_reference")
        .with_note("add an `asset = @asset:.*` field to the image declaration");
        match document.span(self.declaration_anchor) {
            Ok(span) => diagnostic.with_label(DiagnosticLabel::primary(
                span,
                Some("this image declaration requires an `asset` field".to_owned()),
            )),
            Err(_) => diagnostic,
        }
    }
}

pub(in crate::app) fn parse_declared_image_objects(
    document: &SourceDocument,
) -> BTreeMap<String, DeclaredImageObject> {
    parse_source(document.text())
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| {
            let Item::EntityDecl(item) = item else {
                return None;
            };
            (item.kind() == EntityDeclKind::Image).then(|| {
                let id = item.id().body().to_owned();
                let body = item
                    .image_body()
                    .expect("image declarations are parsed into typed image bodies");
                (
                    id.clone(),
                    DeclaredImageObject {
                        args: image_decl_body_args(&id, body),
                        declaration_anchor: SourceRange::new(
                            item.id().range().start(),
                            item.id().range().end(),
                        ),
                        id,
                    },
                )
            })
        })
        .collect()
}

pub(in crate::app) fn declared_image_asset_refs(
    declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Vec<String> {
    let mut refs = declarations
        .values()
        .filter_map(|declaration| declaration_arg_value(declaration.args(), "asset"))
        .filter_map(public_asset_ref_arg)
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

pub(in crate::app) fn public_asset_ref_arg(arg: &str) -> Option<String> {
    let value = public_id_arg(arg)?;
    value.starts_with("asset.").then_some(value)
}

fn image_decl_body_args(id: &str, body: &ImageDeclBody) -> Vec<String> {
    let mut args = body
        .fields()
        .iter()
        .map(|field| format!("{} = {}", field.name(), field.value_source()))
        .collect::<Vec<_>>();
    if declaration_arg_value(&args, "id").is_none() {
        args.insert(0, format!("id = @{id}"));
    }
    args
}

#[cfg(test)]
pub(in crate::app) fn merge_declared_image_args(
    declaration: &DeclaredImageObject,
    override_args: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let override_args = override_args.into_iter().collect::<Vec<_>>();
    let override_names = override_args
        .iter()
        .filter_map(|arg| runtime_arg_name(arg))
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    declaration
        .args()
        .iter()
        .filter(|arg| runtime_arg_name(arg).is_none_or(|name| !override_names.contains(name)))
        .cloned()
        .chain(override_args)
        .collect()
}

#[cfg(test)]
pub(in crate::app) fn runtime_arg_name(arg: &str) -> Option<&str> {
    arg.split_once(" = ").map(|(name, _)| name.trim())
}

pub(in crate::app) fn declaration_arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

pub(in crate::app) fn public_id_arg(arg: &str) -> Option<String> {
    let value = arg.trim().trim_matches('"').trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    let normalized = if let Some((family, suffix)) = value.split_once(":.") {
        if family.is_empty() || suffix.is_empty() {
            return None;
        }
        format!("{family}.{suffix}")
    } else {
        value.to_owned()
    };
    normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::diagnostics::render_plain_diagnostic;
    use arcweft_source::{SourceDocumentId, SourceName};

    fn document(source: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://image-declaration")
                .expect("test source identity"),
            SourceName::path("image-declaration.arcw"),
            source,
        )
        .expect("test source document")
    }

    #[test]
    fn parses_declared_image_object_args_and_default_id() {
        let document = document(
            r"
pub image @image.sample.pulse {
    asset = @asset:.bg.pulse
    x = 12px
    y = 34px
    width = 56px
    height = 78px
}
",
        );
        let declarations = parse_declared_image_objects(&document);
        let declaration = declarations
            .get("image.sample.pulse")
            .expect("declared image object is indexed");

        assert_eq!(declaration.id, "image.sample.pulse");
        assert_eq!(
            declaration.args(),
            &[
                "id = @image.sample.pulse".to_owned(),
                "asset = @asset:.bg.pulse".to_owned(),
                "x = 12px".to_owned(),
                "y = 34px".to_owned(),
                "width = 56px".to_owned(),
                "height = 78px".to_owned(),
            ]
        );
        assert_eq!(
            declared_image_asset_refs(&declarations),
            vec!["asset.bg.pulse".to_owned()]
        );
    }

    #[test]
    fn missing_asset_diagnostic_retains_stable_code_and_declaration_span() {
        let source = r"
pub image @image.sample.missing {
    x = 12px
    y = 34px
    width = 56px
    height = 78px
}
";
        let document = document(source);
        let declarations = parse_declared_image_objects(&document);
        let diagnostic = declarations["image.sample.missing"].missing_asset_diagnostic(&document);

        assert_eq!(
            diagnostic
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("bundle.image.missing_asset_reference")
        );
        assert_eq!(diagnostic.labels().len(), 1);
        assert_eq!(
            diagnostic.labels()[0].span().range().as_range(),
            source.find("@image.sample.missing").expect("image ID")
                ..source.find("@image.sample.missing").expect("image ID")
                    + "@image.sample.missing".len()
        );
        let rendered = render_plain_diagnostic(&document, &diagnostic);
        assert!(rendered.contains(
            "error[bundle.image.missing_asset_reference]: image object `image.sample.missing` is missing an asset reference"
        ));
        assert!(rendered.contains("pub image @image.sample.missing"));
        assert!(rendered.contains("this image declaration requires an `asset` field"));
    }

    #[cfg(feature = "native-capture")]
    #[test]
    fn merge_declared_image_args_lets_call_site_override_named_fields() {
        let declaration = DeclaredImageObject {
            id: "image.sample.pulse".to_owned(),
            args: vec![
                "id = @image.sample.pulse".to_owned(),
                "asset = @asset:.bg.pulse".to_owned(),
                "x = 12px".to_owned(),
                "opacity = 0.5".to_owned(),
            ],
            declaration_anchor: SourceRange::new(0, 0),
        };

        assert_eq!(
            merge_declared_image_args(
                &declaration,
                ["opacity = 1".to_owned(), "param.role = override".to_owned()]
            ),
            vec![
                "id = @image.sample.pulse".to_owned(),
                "asset = @asset:.bg.pulse".to_owned(),
                "x = 12px".to_owned(),
                "opacity = 1".to_owned(),
                "param.role = override".to_owned(),
            ]
        );
    }
}
