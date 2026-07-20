use arcweft_id::{EntityId, PublicId};
use arcweft_manifest_model::PackageId;
use arcweft_resource_model::identity::{
    NominalTypeId, ResourceDeclarationIdentity, ResourceFieldId, ResourceIdentityClass,
    ResourceIdentityErrorKind, ResourceModulePath, ResourcePublicIdFamily, ResourceSchemaId,
    ResourceTypeId, ResourceTypeName,
};

#[test]
fn declaration_identity_preserves_internal_public_and_nominal_axes() {
    let entity = EntityId::try_new("resource-weather-sun").unwrap();
    let public = PublicId::try_new("weather.sun").unwrap();
    let resource_type = resource_type("WeatherIcon");
    let identity =
        ResourceDeclarationIdentity::new(entity.clone(), public.clone(), resource_type.clone());

    assert_eq!(identity.entity_id(), &entity);
    assert_eq!(identity.public_id(), &public);
    assert_eq!(identity.type_id(), &resource_type);
}

#[test]
fn stable_text_identities_reject_noncanonical_spellings() {
    let schema_error = ResourceSchemaId::try_new("Example.Schema").unwrap_err();
    assert_eq!(schema_error.class(), ResourceIdentityClass::SchemaId);
    assert_eq!(schema_error.kind(), ResourceIdentityErrorKind::NonCanonical);

    let family_error = ResourcePublicIdFamily::try_new("audio.voice").unwrap_err();
    assert_eq!(family_error.class(), ResourceIdentityClass::Family);
    assert_eq!(family_error.kind(), ResourceIdentityErrorKind::NonCanonical);
}

#[test]
fn zero_is_reserved_for_stable_numeric_identities() {
    let error = ResourceFieldId::try_new(0).unwrap_err();
    assert_eq!(error.class(), ResourceIdentityClass::FieldId);
    assert_eq!(error.kind(), ResourceIdentityErrorKind::Zero);
}

fn resource_type(name: &str) -> ResourceTypeId {
    ResourceTypeId::new(NominalTypeId::new(
        PackageId::new("com.example.weather").unwrap(),
        ResourceModulePath::try_new("extension").unwrap(),
        ResourceTypeName::try_new(name).unwrap(),
    ))
}
