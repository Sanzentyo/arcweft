use arcweft_lang_syntax::{
    ast::items::{EntityDeclKind, Item},
    parser::parse_source,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct DeclaredImageObject {
    id: String,
    args: Vec<String>,
}

impl DeclaredImageObject {
    pub(in crate::app) fn args(&self) -> &[String] {
        &self.args
    }
}

#[cfg(feature = "native-capture")]
pub(in crate::app) fn load_declared_image_objects(
    source_path: &std::path::Path,
) -> Result<BTreeMap<String, DeclaredImageObject>, String> {
    let source = std::fs::read_to_string(source_path).map_err(|error| {
        format!(
            "failed to read image object declarations from {}: {error}",
            source_path.display()
        )
    })?;
    Ok(parse_declared_image_objects(&source))
}

pub(in crate::app) fn parse_declared_image_objects(
    source: &str,
) -> BTreeMap<String, DeclaredImageObject> {
    parse_source(source)
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| {
            let Item::EntityDecl(item) = item else {
                return None;
            };
            (item.kind() == EntityDeclKind::Image).then(|| {
                let id = item.id().body().to_owned();
                (
                    id.clone(),
                    DeclaredImageObject {
                        args: image_decl_body_args(&id, item.body().unwrap_or_default()),
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

#[cfg(feature = "native-capture")]
pub(in crate::app) fn public_image_ref_arg(arg: &str) -> Option<String> {
    let value = public_id_arg(arg)?;
    value.starts_with("image.").then_some(value)
}

pub(in crate::app) fn public_asset_ref_arg(arg: &str) -> Option<String> {
    let value = public_id_arg(arg)?;
    value.starts_with("asset.").then_some(value)
}

fn image_decl_body_args(id: &str, body: &str) -> Vec<String> {
    let mut args = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            Some(format!(
                "{} = {}",
                name.trim(),
                value.trim().trim_end_matches(',')
            ))
        })
        .collect::<Vec<_>>();
    if declaration_arg_value(&args, "id").is_none() {
        args.insert(0, format!("id = @{id}"));
    }
    args
}

#[cfg(feature = "native-capture")]
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

#[cfg(feature = "native-capture")]
pub(in crate::app) fn runtime_arg_name(arg: &str) -> Option<&str> {
    arg.split_once(" = ").map(|(name, _)| name.trim())
}

pub(in crate::app) fn declaration_arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

fn public_id_arg(arg: &str) -> Option<String> {
    let value = arg.trim().trim_matches('"').trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_declared_image_object_args_and_default_id() {
        let declarations = parse_declared_image_objects(
            r"
pub image @image.sample.pulse {
    asset = @asset.bg.pulse
    x = 12px
    y = 34px
    width = 56px
    height = 78px
}
",
        );
        let declaration = declarations
            .get("image.sample.pulse")
            .expect("declared image object is indexed");

        assert_eq!(declaration.id, "image.sample.pulse");
        assert_eq!(
            declaration.args(),
            &[
                "id = @image.sample.pulse".to_owned(),
                "asset = @asset.bg.pulse".to_owned(),
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

    #[cfg(feature = "native-capture")]
    #[test]
    fn merge_declared_image_args_lets_call_site_override_named_fields() {
        let declaration = DeclaredImageObject {
            id: "image.sample.pulse".to_owned(),
            args: vec![
                "id = @image.sample.pulse".to_owned(),
                "asset = @asset.bg.pulse".to_owned(),
                "x = 12px".to_owned(),
                "opacity = 0.5".to_owned(),
            ],
        };

        assert_eq!(
            merge_declared_image_args(
                &declaration,
                ["opacity = 1".to_owned(), "param.role = override".to_owned()]
            ),
            vec![
                "id = @image.sample.pulse".to_owned(),
                "asset = @asset.bg.pulse".to_owned(),
                "x = 12px".to_owned(),
                "opacity = 1".to_owned(),
                "param.role = override".to_owned(),
            ]
        );
    }
}
