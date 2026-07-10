use std::collections::BTreeSet;

use arcweft_data::DataFormat;

#[test]
fn inventory_metadata_is_unique_and_canonical_names_round_trip() {
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut media_types = BTreeSet::new();

    for format in DataFormat::ALL {
        assert!(names.insert(format.variant_name()));
        assert!(ids.insert(format.id()));
        assert!(media_types.insert(format.media_type()));
        assert_eq!(
            DataFormat::from_variant_name(format.variant_name()),
            Some(format)
        );
        assert_eq!(DataFormat::from_id(format.id()), Some(format));
    }
}

#[test]
fn format_lookups_are_canonical_and_case_sensitive() {
    assert_eq!(
        DataFormat::from_variant_name("Json"),
        Some(DataFormat::Json)
    );
    assert_eq!(DataFormat::from_variant_name("json"), None);
    assert_eq!(DataFormat::from_variant_name("DataFormat.Json"), None);
    assert_eq!(DataFormat::from_id("yaml"), Some(DataFormat::Yaml));
    assert_eq!(DataFormat::from_id("YAML"), None);
    assert_eq!(DataFormat::from_id("yml"), None);
}
