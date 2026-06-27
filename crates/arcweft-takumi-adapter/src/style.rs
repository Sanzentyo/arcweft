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
    UnsupportedDirect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssPropertyClass {
    PaintOnly,
    Layout,
    Resource,
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
            "filter" | "backdrop-filter" | "clip-path" | "mix-blend-mode" | "mask"
            | "mask-size" | "mask-position" | "mask-repeat" => Self::UnsupportedDirect,
            _ => Self::Layout,
        }
    }

    pub fn invalidation(self) -> CssInvalidationClass {
        match self {
            Self::PaintOnly => CssInvalidationClass::PaintOnly,
            Self::Layout => CssInvalidationClass::LayoutScene,
            Self::Resource => CssInvalidationClass::Resource,
            Self::UnsupportedDirect => CssInvalidationClass::UnsupportedDirect,
        }
    }
}

impl DirectCssSupport {
    pub fn diagnose_css(css: &str) -> Self {
        let diagnostics = unsupported_properties(css)
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

fn unsupported_properties(css: &str) -> Vec<String> {
    [
        "filter",
        "backdrop-filter",
        "clip-path",
        "mix-blend-mode",
        "mask",
        "mask-size",
        "mask-position",
        "mask-repeat",
    ]
    .into_iter()
    .filter(|property| contains_css_property(css, property))
    .map(str::to_owned)
    .collect()
}

fn contains_css_property(css: &str, property: &str) -> bool {
    css.split(['{', ';', '}'])
        .filter_map(|chunk| chunk.split_once(':').map(|(name, _)| name.trim()))
        .map(normalize_property_name)
        .any(|name| name == property)
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
    fn unsupported_css_is_diagnostic_not_raster_fallback() {
        let support = DirectCssSupport::diagnose_css(".card { filter: blur(8px); opacity: 0.8; }");

        assert!(!support.is_direct_wgpu_ready());
        assert_eq!(
            support.diagnostics()[0].code(),
            TakumiDiagnosticCode::UnsupportedDirectCss
        );
        assert!(support.diagnostics()[0].message().contains("filter"));
    }
}
