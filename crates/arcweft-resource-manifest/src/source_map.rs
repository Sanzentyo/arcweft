use arcweft_resource_model::{
    descriptor::{
        ResourceCodecSupport, ResourceEnumSchema, ResourceRecordSchema, ResourceTypeDescriptor,
        ResourceValueSchema, ResourceValueSchemaKind,
    },
    identity::{
        ResourceCodecId, ResourceFieldId, ResourceSchemaId, ResourceTypeId, ResourceVariantId,
    },
    value::{
        ResourceConstValue, ResourceValidationPathSegment, ResourceValueType,
        ResourceValueTypePath, ResourceValueTypePathSegment,
    },
};
use arcweft_source::SourceRange;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct JsonPath(Box<[JsonPathSegment]>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum JsonPathSegment {
    Field(Box<str>),
    Index(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonTokenRange {
    key: Option<SourceRange>,
    value: SourceRange,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceManifestSourceMap {
    lexical: BTreeMap<JsonPath, JsonTokenRange>,
    schemas: BTreeMap<ResourceSchemaId, ResourceSchemaSource>,
    resource_types: BTreeMap<ResourceTypeId, ResourceTypeSource>,
    codecs: BTreeMap<ResourceCodecId, ResourceCodecSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceSchemaSource {
    record: JsonTokenRange,
    identity: JsonTokenRange,
    nominal_type: JsonTokenRange,
    kind: ResourceValueSchemaKind,
    fields: BTreeMap<ResourceFieldId, ResourceFieldSource>,
    variants: BTreeMap<ResourceVariantId, ResourceVariantSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceFieldSource {
    record: JsonTokenRange,
    identity: JsonTokenRange,
    name: JsonTokenRange,
    value_type: JsonTokenRange,
    default: Option<JsonTokenRange>,
    value_type_paths: BTreeMap<ResourceValueTypePath, JsonTokenRange>,
    default_paths: BTreeMap<ResourceConstSourcePath, JsonTokenRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceVariantSource {
    record: JsonTokenRange,
    identity: JsonTokenRange,
    name: JsonTokenRange,
    payload: Option<JsonTokenRange>,
    value_type_paths: BTreeMap<ResourceValueTypePath, JsonTokenRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceTypeSource {
    record: JsonTokenRange,
    identity: JsonTokenRange,
    public_id_family: JsonTokenRange,
    family_group: JsonTokenRange,
    body_schema: JsonTokenRange,
    capabilities: JsonTokenRange,
    lowering_codec: JsonTokenRange,
    lowering_version: JsonTokenRange,
    descriptor_digest: JsonTokenRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceCodecSource {
    record: JsonTokenRange,
    identity: JsonTokenRange,
    versions: BTreeMap<u32, JsonTokenRange>,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceConstSourcePath(Box<[ResourceValidationPathSegment]>);

impl JsonPath {
    pub fn segments(&self) -> &[JsonPathSegment] {
        &self.0
    }
    pub(crate) fn field(&self, field: &str) -> Self {
        let mut path = self.0.to_vec();
        path.push(JsonPathSegment::Field(field.into()));
        Self(path.into_boxed_slice())
    }
    pub(crate) fn index(&self, index: usize) -> Self {
        let mut path = self.0.to_vec();
        path.push(JsonPathSegment::Index(
            u32::try_from(index).unwrap_or(u32::MAX),
        ));
        Self(path.into_boxed_slice())
    }
}

impl JsonTokenRange {
    pub const fn new(key: Option<SourceRange>, value: SourceRange) -> Self {
        Self { key, value }
    }
    pub const fn key(self) -> Option<SourceRange> {
        self.key
    }
    pub const fn value(self) -> SourceRange {
        self.value
    }
}

impl ResourceManifestSourceMap {
    pub fn token(&self, path: &JsonPath) -> Option<JsonTokenRange> {
        self.lexical.get(path).copied()
    }
    pub fn lexical(&self) -> &BTreeMap<JsonPath, JsonTokenRange> {
        &self.lexical
    }
    pub fn schemas(&self) -> &BTreeMap<ResourceSchemaId, ResourceSchemaSource> {
        &self.schemas
    }
    pub fn resource_types(&self) -> &BTreeMap<ResourceTypeId, ResourceTypeSource> {
        &self.resource_types
    }
    pub fn codecs(&self) -> &BTreeMap<ResourceCodecId, ResourceCodecSource> {
        &self.codecs
    }
    pub(crate) fn insert(&mut self, path: JsonPath, token: JsonTokenRange) {
        self.lexical.insert(path, token);
    }

    pub(crate) fn bind_semantics(
        &mut self,
        file: &crate::ResourceTypeManifestFileV1,
        json: &serde_json::Value,
    ) {
        self.bind_schema_semantics(file.schemas(), &json["schemas"]);
        self.bind_type_semantics(file.resource_types(), &json["resource_types"]);
        self.bind_codec_semantics(file.codecs(), json);
    }

    fn bind_schema_semantics(&mut self, schemas: &[ResourceValueSchema], json: &serde_json::Value) {
        let root = JsonPath::default();
        let authored = json
            .as_array()
            .expect("validated manifest schemas remain an array");
        for schema in schemas {
            let index = authored
                .iter()
                .position(|value| {
                    value["value"]["schema_id"].as_str() == Some(schema.id().as_str())
                })
                .expect("lowered schema retains its authored identity");
            let path = root.field("schemas").index(index);
            let content = path.field("value");
            let (kind, identity, fields, variants) =
                self.bind_one_schema(schema, &content, &authored[index]["value"]);
            self.schemas.insert(
                identity,
                ResourceSchemaSource {
                    record: self.required_token(&path),
                    identity: self.required_token(&content.field("schema_id")),
                    nominal_type: self.required_token(&content.field("nominal_type")),
                    kind,
                    fields,
                    variants,
                },
            );
        }
    }

    fn bind_one_schema(
        &self,
        schema: &ResourceValueSchema,
        content: &JsonPath,
        json: &serde_json::Value,
    ) -> (
        ResourceValueSchemaKind,
        ResourceSchemaId,
        BTreeMap<ResourceFieldId, ResourceFieldSource>,
        BTreeMap<ResourceVariantId, ResourceVariantSource>,
    ) {
        match schema {
            ResourceValueSchema::Record(schema) => self.bind_record_schema(schema, content, json),
            ResourceValueSchema::Enum(schema) => self.bind_enum_schema(schema, content, json),
        }
    }

    fn bind_record_schema(
        &self,
        schema: &ResourceRecordSchema,
        content: &JsonPath,
        json: &serde_json::Value,
    ) -> (
        ResourceValueSchemaKind,
        ResourceSchemaId,
        BTreeMap<ResourceFieldId, ResourceFieldSource>,
        BTreeMap<ResourceVariantId, ResourceVariantSource>,
    ) {
        let authored = json["fields"]
            .as_array()
            .expect("validated record fields remain an array");
        let fields = schema
            .fields()
            .iter()
            .map(|field| {
                let index = authored
                    .iter()
                    .position(|value| {
                        value["field_id"].as_u64() == Some(u64::from(field.id().get()))
                    })
                    .expect("lowered field retains its authored identity");
                let path = content.field("fields").index(index);
                let value_type_path = path.field("value_type");
                let mut value_type_paths = BTreeMap::new();
                bind_value_type_paths(
                    self,
                    field.value_type(),
                    &value_type_path,
                    vec![ResourceValueTypePathSegment::RecordField(field.id())],
                    &mut value_type_paths,
                );
                let default = field.default().map(|value| {
                    let default_path = path.field("default");
                    let mut default_paths = BTreeMap::new();
                    bind_const_paths(
                        self,
                        value,
                        &authored[index]["default"],
                        &default_path,
                        Vec::new(),
                        &mut default_paths,
                    );
                    (self.required_token(&default_path), default_paths)
                });
                (
                    field.id(),
                    ResourceFieldSource {
                        record: self.required_token(&path),
                        identity: self.required_token(&path.field("field_id")),
                        name: self.required_token(&path.field("name")),
                        value_type: self.required_token(&value_type_path),
                        default: default.as_ref().map(|(token, _)| *token),
                        value_type_paths,
                        default_paths: default.map_or_else(BTreeMap::new, |(_, paths)| paths),
                    },
                )
            })
            .collect();
        (
            ResourceValueSchemaKind::Record,
            schema.id().clone(),
            fields,
            BTreeMap::new(),
        )
    }

    fn bind_enum_schema(
        &self,
        schema: &ResourceEnumSchema,
        content: &JsonPath,
        json: &serde_json::Value,
    ) -> (
        ResourceValueSchemaKind,
        ResourceSchemaId,
        BTreeMap<ResourceFieldId, ResourceFieldSource>,
        BTreeMap<ResourceVariantId, ResourceVariantSource>,
    ) {
        let authored = json["variants"]
            .as_array()
            .expect("validated enum variants remain an array");
        let variants = schema
            .variants()
            .iter()
            .map(|variant| {
                let index = authored
                    .iter()
                    .position(|value| {
                        value["variant_id"].as_u64() == Some(u64::from(variant.id().get()))
                    })
                    .expect("lowered variant retains its authored identity");
                let path = content.field("variants").index(index);
                let payload_path = path.field("payload");
                let mut value_type_paths = BTreeMap::new();
                if let Some(payload) = variant.payload() {
                    bind_value_type_paths(
                        self,
                        payload,
                        &payload_path,
                        vec![ResourceValueTypePathSegment::EnumVariant(variant.id())],
                        &mut value_type_paths,
                    );
                }
                (
                    variant.id(),
                    ResourceVariantSource {
                        record: self.required_token(&path),
                        identity: self.required_token(&path.field("variant_id")),
                        name: self.required_token(&path.field("name")),
                        payload: variant
                            .payload()
                            .map(|_| self.required_token(&payload_path)),
                        value_type_paths,
                    },
                )
            })
            .collect();
        (
            ResourceValueSchemaKind::Enum,
            schema.id().clone(),
            BTreeMap::new(),
            variants,
        )
    }

    fn bind_type_semantics(
        &mut self,
        descriptors: &[ResourceTypeDescriptor],
        json: &serde_json::Value,
    ) {
        let root = JsonPath::default().field("resource_types");
        let authored = json
            .as_array()
            .expect("validated resource types remain an array");
        for descriptor in descriptors {
            let nominal = descriptor.type_id().nominal();
            let index = authored
                .iter()
                .position(|value| {
                    value["type_id"]["package"].as_str() == Some(nominal.package().as_str())
                        && value["type_id"]["module"].as_str() == Some(nominal.module().as_str())
                        && value["type_id"]["name"].as_str() == Some(nominal.name().as_str())
                })
                .expect("lowered resource type retains its authored identity");
            let path = root.index(index);
            self.resource_types.insert(
                descriptor.type_id().clone(),
                ResourceTypeSource {
                    record: self.required_token(&path),
                    identity: self.required_token(&path.field("type_id")),
                    public_id_family: self.required_token(&path.field("public_id_family")),
                    family_group: self.required_token(&path.field("family_group")),
                    body_schema: self.required_token(&path.field("body_schema")),
                    capabilities: self.required_token(&path.field("capabilities")),
                    lowering_codec: self.required_token(&path.field("lowering").field("codec_id")),
                    lowering_version: self
                        .required_token(&path.field("lowering").field("codec_version")),
                    descriptor_digest: self.required_token(&path.field("descriptor_digest")),
                },
            );
        }
    }

    fn bind_codec_semantics(&mut self, codecs: &[ResourceCodecSupport], json: &serde_json::Value) {
        let root = JsonPath::default().field("codecs");
        let authored = json["codecs"]
            .as_array()
            .expect("validated codecs remain an array");
        for codec in codecs {
            let index = authored
                .iter()
                .position(|value| value["codec_id"].as_str() == Some(codec.codec_id().as_str()))
                .expect("lowered codec retains its authored identity");
            let path = root.index(index);
            let versions = authored[index]["versions"]
                .as_array()
                .expect("validated codec has a versions array")
                .iter()
                .enumerate()
                .map(|(version_index, version)| {
                    (
                        u32::try_from(
                            version
                                .as_u64()
                                .expect("lowered codec version was an unsigned integer"),
                        )
                        .expect("lowered codec version fit u32"),
                        self.required_token(&path.field("versions").index(version_index)),
                    )
                })
                .collect();
            self.codecs.insert(
                codec.codec_id().clone(),
                ResourceCodecSource {
                    record: self.required_token(&path),
                    identity: self.required_token(&path.field("codec_id")),
                    versions,
                },
            );
        }
    }

    fn required_token(&self, path: &JsonPath) -> JsonTokenRange {
        *self
            .lexical
            .get(path)
            .expect("validated manifest shape has a lexical range for every semantic field")
    }
}

fn bind_value_type_paths(
    source_map: &ResourceManifestSourceMap,
    value_type: &ResourceValueType,
    path: &JsonPath,
    segments: Vec<ResourceValueTypePathSegment>,
    output: &mut BTreeMap<ResourceValueTypePath, JsonTokenRange>,
) {
    let target_path = match value_type {
        ResourceValueType::NominalRecord(_)
        | ResourceValueType::NominalEnum(_)
        | ResourceValueType::RetainedIdentityRef { .. }
        | ResourceValueType::Scalar(_) => path.field("value"),
        ResourceValueType::AssetRef { .. } => path.field("value").field("payload_kind"),
        ResourceValueType::ResourceRef { .. } => path.field("value").field("type_id"),
        ResourceValueType::Option(_)
        | ResourceValueType::Vec(_)
        | ResourceValueType::NonEmptyVec(_)
        | ResourceValueType::Map { .. }
        | ResourceValueType::ConstrainedScalar(_) => path.clone(),
    };
    output.insert(
        ResourceValueTypePath::new(segments.iter().copied()),
        source_map.required_token(&target_path),
    );
    match value_type {
        ResourceValueType::Option(value) => bind_child_value_type(
            source_map,
            value,
            &path.field("value"),
            segments,
            ResourceValueTypePathSegment::OptionValue,
            output,
        ),
        ResourceValueType::Vec(value) | ResourceValueType::NonEmptyVec(value) => {
            bind_child_value_type(
                source_map,
                value,
                &path.field("value"),
                segments,
                ResourceValueTypePathSegment::SequenceElement,
                output,
            );
        }
        ResourceValueType::Map { key, value } => {
            let content = path.field("value");
            bind_child_value_type(
                source_map,
                key,
                &content.field("key"),
                segments.clone(),
                ResourceValueTypePathSegment::MapKey,
                output,
            );
            bind_child_value_type(
                source_map,
                value,
                &content.field("value"),
                segments,
                ResourceValueTypePathSegment::MapValue,
                output,
            );
        }
        ResourceValueType::Scalar(_)
        | ResourceValueType::NominalRecord(_)
        | ResourceValueType::NominalEnum(_)
        | ResourceValueType::AssetRef { .. }
        | ResourceValueType::ResourceRef { .. }
        | ResourceValueType::ConstrainedScalar(_)
        | ResourceValueType::RetainedIdentityRef { .. } => {}
    }
}

fn bind_child_value_type(
    source_map: &ResourceManifestSourceMap,
    value_type: &ResourceValueType,
    path: &JsonPath,
    mut segments: Vec<ResourceValueTypePathSegment>,
    segment: ResourceValueTypePathSegment,
    output: &mut BTreeMap<ResourceValueTypePath, JsonTokenRange>,
) {
    segments.push(segment);
    bind_value_type_paths(source_map, value_type, path, segments, output);
}

fn bind_const_paths(
    source_map: &ResourceManifestSourceMap,
    value: &ResourceConstValue,
    json: &serde_json::Value,
    path: &JsonPath,
    segments: Vec<ResourceValidationPathSegment>,
    output: &mut BTreeMap<ResourceConstSourcePath, JsonTokenRange>,
) {
    output.insert(
        ResourceConstSourcePath::new(segments.iter().cloned()),
        source_map.required_token(path),
    );
    match value {
        ResourceConstValue::Option(Some(value)) => bind_const_child(
            source_map,
            value,
            &json["value"],
            &path.field("value"),
            segments,
            ResourceValidationPathSegment::OptionValue,
            output,
        ),
        ResourceConstValue::Sequence(values) => {
            for (index, value) in values.iter().enumerate() {
                bind_const_child(
                    source_map,
                    value,
                    &json["value"][index],
                    &path.field("value").index(index),
                    segments.clone(),
                    ResourceValidationPathSegment::SequenceIndex(index),
                    output,
                );
            }
        }
        ResourceConstValue::Map(map) => bind_map_const_paths(
            source_map,
            map,
            &json["value"],
            &path.field("value"),
            &segments,
            output,
        ),
        ResourceConstValue::Record(record) => {
            let fields = json["value"]["fields"]
                .as_array()
                .expect("validated record constant has fields");
            for (field_id, value) in record.fields() {
                let index = fields
                    .iter()
                    .position(|field| field["field_id"].as_u64() == Some(u64::from(field_id.get())))
                    .expect("lowered record field retains its authored source");
                let field_path = path
                    .field("value")
                    .field("fields")
                    .index(index)
                    .field("value");
                bind_const_child(
                    source_map,
                    value,
                    &fields[index]["value"],
                    &field_path,
                    segments.clone(),
                    ResourceValidationPathSegment::RecordField(*field_id),
                    output,
                );
            }
        }
        ResourceConstValue::Enum(value) => {
            if let Some(payload) = value.payload() {
                bind_const_child(
                    source_map,
                    payload,
                    &json["value"]["payload"],
                    &path.field("value").field("payload"),
                    segments,
                    ResourceValidationPathSegment::EnumPayload,
                    output,
                );
            }
        }
        ResourceConstValue::Option(None)
        | ResourceConstValue::Scalar(_)
        | ResourceConstValue::AssetRef(_)
        | ResourceConstValue::ResourceRef(_)
        | ResourceConstValue::RetainedIdentityRef { .. } => {}
    }
}

fn bind_map_const_paths(
    source_map: &ResourceManifestSourceMap,
    map: &arcweft_resource_model::value::ResourceMapValue,
    json: &serde_json::Value,
    path: &JsonPath,
    segments: &[ResourceValidationPathSegment],
    output: &mut BTreeMap<ResourceConstSourcePath, JsonTokenRange>,
) {
    let entries = json
        .as_array()
        .expect("validated map constant has an entry array");
    for (canonical_index, (key, value)) in map.entries().iter().enumerate() {
        let canonical_key = crate::encode::const_value(key);
        let authored_index = entries
            .iter()
            .position(|entry| entry["key"] == canonical_key)
            .unwrap_or(canonical_index);
        let entry_path = path.index(authored_index);
        bind_const_child(
            source_map,
            key,
            &entries[authored_index]["key"],
            &entry_path.field("key"),
            segments.to_vec(),
            ResourceValidationPathSegment::MapKey(canonical_index),
            output,
        );
        bind_const_child(
            source_map,
            value,
            &entries[authored_index]["value"],
            &entry_path.field("value"),
            segments.to_vec(),
            ResourceValidationPathSegment::MapValue(canonical_index),
            output,
        );
    }
}

fn bind_const_child(
    source_map: &ResourceManifestSourceMap,
    value: &ResourceConstValue,
    json: &serde_json::Value,
    path: &JsonPath,
    mut segments: Vec<ResourceValidationPathSegment>,
    segment: ResourceValidationPathSegment,
    output: &mut BTreeMap<ResourceConstSourcePath, JsonTokenRange>,
) {
    segments.push(segment);
    bind_const_paths(source_map, value, json, path, segments, output);
}

impl ResourceSchemaSource {
    pub const fn record(&self) -> JsonTokenRange {
        self.record
    }
    pub const fn identity(&self) -> JsonTokenRange {
        self.identity
    }
    pub const fn nominal_type(&self) -> JsonTokenRange {
        self.nominal_type
    }
    pub const fn kind(&self) -> ResourceValueSchemaKind {
        self.kind
    }
    pub fn fields(&self) -> &BTreeMap<ResourceFieldId, ResourceFieldSource> {
        &self.fields
    }
    pub fn variants(&self) -> &BTreeMap<ResourceVariantId, ResourceVariantSource> {
        &self.variants
    }
}

impl ResourceFieldSource {
    pub const fn record(&self) -> JsonTokenRange {
        self.record
    }
    pub const fn identity(&self) -> JsonTokenRange {
        self.identity
    }
    pub const fn name(&self) -> JsonTokenRange {
        self.name
    }
    pub const fn value_type(&self) -> JsonTokenRange {
        self.value_type
    }
    pub const fn default(&self) -> Option<JsonTokenRange> {
        self.default
    }
    pub fn value_type_paths(&self) -> &BTreeMap<ResourceValueTypePath, JsonTokenRange> {
        &self.value_type_paths
    }
    pub fn default_paths(&self) -> &BTreeMap<ResourceConstSourcePath, JsonTokenRange> {
        &self.default_paths
    }
}

impl ResourceVariantSource {
    pub const fn record(&self) -> JsonTokenRange {
        self.record
    }
    pub const fn identity(&self) -> JsonTokenRange {
        self.identity
    }
    pub const fn name(&self) -> JsonTokenRange {
        self.name
    }
    pub const fn payload(&self) -> Option<JsonTokenRange> {
        self.payload
    }
    pub fn value_type_paths(&self) -> &BTreeMap<ResourceValueTypePath, JsonTokenRange> {
        &self.value_type_paths
    }
}

impl ResourceTypeSource {
    pub const fn record(&self) -> JsonTokenRange {
        self.record
    }
    pub const fn identity(&self) -> JsonTokenRange {
        self.identity
    }
    pub const fn public_id_family(&self) -> JsonTokenRange {
        self.public_id_family
    }
    pub const fn family_group(&self) -> JsonTokenRange {
        self.family_group
    }
    pub const fn body_schema(&self) -> JsonTokenRange {
        self.body_schema
    }
    pub const fn capabilities(&self) -> JsonTokenRange {
        self.capabilities
    }
    pub const fn lowering_codec(&self) -> JsonTokenRange {
        self.lowering_codec
    }
    pub const fn lowering_version(&self) -> JsonTokenRange {
        self.lowering_version
    }
    pub const fn descriptor_digest(&self) -> JsonTokenRange {
        self.descriptor_digest
    }
}

impl ResourceCodecSource {
    pub const fn record(&self) -> JsonTokenRange {
        self.record
    }
    pub const fn identity(&self) -> JsonTokenRange {
        self.identity
    }
    pub fn versions(&self) -> &BTreeMap<u32, JsonTokenRange> {
        &self.versions
    }
}

impl ResourceConstSourcePath {
    pub fn new(segments: impl IntoIterator<Item = ResourceValidationPathSegment>) -> Self {
        Self(segments.into_iter().collect())
    }
    pub fn segments(&self) -> &[ResourceValidationPathSegment] {
        &self.0
    }
}
