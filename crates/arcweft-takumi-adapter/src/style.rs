use crate::diagnostic::{TakumiAdapterError, TakumiDiagnostic};
use takumi::prelude::StyleSheet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCssFeature {
    SolidRect,
    RoundedRect,
    Border,
    Image,
    LinearGradient,
    Clip,
    Opacity,
    Transform,
    TextPlaceholder,
    BoxShadow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssInvalidationClass {
    PaintOnly,
    LayoutScene,
    Resource,
    Compositing,
    BackdropCompositing,
    MaskCompositing,
    ClipGeometry,
    UnsupportedDirect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssPropertyClass {
    PaintOnly,
    Layout,
    Resource,
    Compositing,
    BackdropCompositing,
    MaskCompositing,
    ClipGeometry,
    UnsupportedDirect,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectCssSupport {
    diagnostics: Vec<TakumiDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TakumiCssBundle {
    stylesheets: Vec<String>,
}

impl CssPropertyClass {
    pub fn classify(name: &str) -> Self {
        match normalize_property_name(name).as_str() {
            "opacity"
            | "transform"
            | "translate"
            | "rotate"
            | "scale"
            | "background-color"
            | "border-color"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
            | "box-shadow" => Self::PaintOnly,
            "background-image" | "background" | "src" | "mask-image" => Self::Resource,
            "filter" | "mix-blend-mode" | "isolation" => Self::Compositing,
            "backdrop-filter" => Self::BackdropCompositing,
            "mask" | "mask-size" | "mask-position" | "mask-repeat" | "mask-mode"
            | "mask-origin" | "mask-clip" | "mask-composite" => Self::MaskCompositing,
            "clip-path" | "clip-rule" => Self::ClipGeometry,
            _ => Self::Layout,
        }
    }

    pub fn invalidation(self) -> CssInvalidationClass {
        match self {
            Self::PaintOnly => CssInvalidationClass::PaintOnly,
            Self::Layout => CssInvalidationClass::LayoutScene,
            Self::Resource => CssInvalidationClass::Resource,
            Self::Compositing => CssInvalidationClass::Compositing,
            Self::BackdropCompositing => CssInvalidationClass::BackdropCompositing,
            Self::MaskCompositing => CssInvalidationClass::MaskCompositing,
            Self::ClipGeometry => CssInvalidationClass::ClipGeometry,
            Self::UnsupportedDirect => CssInvalidationClass::UnsupportedDirect,
        }
    }

    pub fn requires_compositing_scene(self) -> bool {
        matches!(
            self,
            Self::Compositing
                | Self::BackdropCompositing
                | Self::MaskCompositing
                | Self::ClipGeometry
        )
    }

    pub fn requires_resource_revision(self) -> bool {
        self == Self::Resource
    }
}

impl DirectCssSupport {
    pub fn diagnose_css(css: &str) -> Self {
        let diagnostics = unsupported_compositing_values(css)
            .into_iter()
            .map(TakumiDiagnostic::unsupported_css)
            .collect();
        Self { diagnostics }
    }

    pub fn diagnostics(&self) -> &[TakumiDiagnostic] {
        &self.diagnostics
    }

    pub fn is_direct_wgpu_ready(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn implementation_ready_features() -> &'static [DirectCssFeature] {
        &[
            DirectCssFeature::SolidRect,
            DirectCssFeature::RoundedRect,
            DirectCssFeature::Border,
            DirectCssFeature::Image,
            DirectCssFeature::LinearGradient,
            DirectCssFeature::Clip,
            DirectCssFeature::Opacity,
            DirectCssFeature::Transform,
            DirectCssFeature::TextPlaceholder,
        ]
    }
}

impl TakumiCssBundle {
    pub fn new(stylesheets: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            stylesheets: stylesheets.into_iter().map(Into::into).collect(),
        }
    }

    pub fn stylesheets(&self) -> &[String] {
        &self.stylesheets
    }

    pub fn parse(&self) -> Result<StyleSheet, TakumiAdapterError> {
        StyleSheet::parse_list(&self.stylesheets)
            .map_err(|error| TakumiAdapterError::css_parse(error.to_string()))
    }

    pub fn direct_support(&self) -> DirectCssSupport {
        DirectCssSupport {
            diagnostics: self
                .stylesheets
                .iter()
                .flat_map(|css| DirectCssSupport::diagnose_css(css).diagnostics)
                .collect(),
        }
    }
}

fn unsupported_compositing_values(css: &str) -> Vec<String> {
    css_declarations(css)
        .filter_map(|(name, value)| unsupported_compositing_value(name, value))
        .collect()
}

fn unsupported_compositing_value(name: &str, value: &str) -> Option<String> {
    let name = normalize_property_name(name);
    let value = value.trim().to_ascii_lowercase();
    match name.as_str() {
        "filter" | "backdrop-filter" if value.contains("url(") => {
            Some(format!("{name}: filter-url-reference"))
        }
        "clip-path" if value.starts_with("url(") => Some("clip-path: url-reference".to_owned()),
        "mask-image" | "mask" if value.contains("element(") => {
            Some(format!("{name}: element-reference"))
        }
        _ => None,
    }
}

fn css_declarations(css: &str) -> impl Iterator<Item = (&str, &str)> {
    css.split(['{', ';', '}'])
        .filter_map(|chunk| chunk.split_once(':'))
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn normalize_property_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::TakumiDiagnosticCode;

    #[test]
    fn paint_only_properties_do_not_force_layout_scene_rebuild() {
        assert_eq!(
            CssPropertyClass::classify("opacity").invalidation(),
            CssInvalidationClass::PaintOnly
        );
        assert_eq!(
            CssPropertyClass::classify("transform").invalidation(),
            CssInvalidationClass::PaintOnly
        );
        assert_eq!(
            CssPropertyClass::classify("display").invalidation(),
            CssInvalidationClass::LayoutScene
        );
    }

    #[test]
    fn compositing_properties_are_not_generic_unsupported_direct() {
        assert_eq!(
            CssPropertyClass::classify("filter").invalidation(),
            CssInvalidationClass::Compositing
        );
        assert_eq!(
            CssPropertyClass::classify("backdrop-filter").invalidation(),
            CssInvalidationClass::BackdropCompositing
        );
        assert_eq!(
            CssPropertyClass::classify("mask-size").invalidation(),
            CssInvalidationClass::MaskCompositing
        );
        assert_eq!(
            CssPropertyClass::classify("clip-path").invalidation(),
            CssInvalidationClass::ClipGeometry
        );
        assert_eq!(
            CssPropertyClass::classify("mix-blend-mode").invalidation(),
            CssInvalidationClass::Compositing
        );
    }

    #[test]
    fn mask_image_url_requires_resource_revision() {
        let classification = CssPropertyClass::classify("mask-image");

        assert_eq!(
            classification.invalidation(),
            CssInvalidationClass::Resource
        );
        assert!(classification.requires_resource_revision());
    }

    #[test]
    fn representable_compositing_css_is_not_reported_as_unsupported_direct() {
        let support = DirectCssSupport::diagnose_css(
            ".card { filter: blur(8px); backdrop-filter: brightness(0.8); clip-path: inset(4px); mix-blend-mode: multiply; mask-image: url(mask.png); }",
        );

        assert!(support.is_direct_wgpu_ready());
        assert!(support.diagnostics().is_empty());
    }

    #[test]
    fn unsupported_compositing_values_are_diagnostic_not_raster_fallback() {
        let support = DirectCssSupport::diagnose_css(".card { filter: url(#goo); opacity: 0.8; }");

        assert!(!support.is_direct_wgpu_ready());
        assert_eq!(
            support.diagnostics()[0].code(),
            TakumiDiagnosticCode::UnsupportedDirectCss
        );
        assert!(
            support.diagnostics()[0]
                .message()
                .contains("filter-url-reference")
        );
    }
}
