//! Shared rich-text tag vocabulary.

/// Canonical style-stack family for an authored rich-text tag or alias.
///
/// Syntax validation and retained-text rendering both use this inventory so
/// an accepted end tag cannot resolve to a different runtime family.
#[must_use]
pub fn canonical_tag_name(name: &str) -> &str {
    match name {
        "" | "/" => "/",
        "i" | "italic" | "oblique" | "slant" | "opacity" | "alpha" | "layer" | "object_layer"
        | "meta" | "metadata" | "data" | "z" | "z_index" | "style" => "style",
        "vertical"
        | "vertical_rl"
        | "vertical_lr"
        | "horizontal_tb"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character"
        | "layout" => "layout",
        "offset" | "pos" | "rotate" | "scale" | "skew" | "transform" => "transform",
        "wave" | "shake" | "arc" | "spin" | "pulse" | "motion" | "typewriter" | "jitter"
        | "shader" | "host" | "effect" | "fx" => "effect",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_tag_name;

    #[test]
    fn aliases_resolve_to_their_style_stack_family() {
        for (authored, canonical) in [
            ("slant", "style"),
            ("dir", "layout"),
            ("skew", "transform"),
            ("fx", "effect"),
            ("custom", "custom"),
        ] {
            assert_eq!(canonical_tag_name(authored), canonical);
        }
    }
}
