use crate::glyph::GlyphArea;

/// Extension trait shape for a renderer that can consume pre-laid glyph areas.
///
/// Real glyphon integration needs access to `TextRenderer` internals. This trait
/// documents the long-term API without depending on glyphon in the design crate.
pub trait TextRendererGlyphAreaExt {
    type Error;

    fn prepare_glyph_areas<'a>(
        &mut self,
        areas: impl IntoIterator<Item = GlyphArea<'a>>,
    ) -> Result<(), Self::Error>;
}

/// A minimal test renderer used by design tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CountingRenderer {
    pub prepared_glyphs: usize,
}

impl TextRendererGlyphAreaExt for CountingRenderer {
    type Error = core::convert::Infallible;

    fn prepare_glyph_areas<'a>(
        &mut self,
        areas: impl IntoIterator<Item = GlyphArea<'a>>,
    ) -> Result<(), Self::Error> {
        self.prepared_glyphs += areas
            .into_iter()
            .map(|area| area.visible_glyphs().count())
            .sum::<usize>();
        Ok(())
    }
}
