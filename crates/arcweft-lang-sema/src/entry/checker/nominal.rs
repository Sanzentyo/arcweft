use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use arcweft_data::{FieldShape, TypeShape, VariantShape};
use arcweft_lang_hir::{
    model::{HirModule, HirTopLevelDecl},
    project::HirProject,
};
use arcweft_lang_syntax::{
    ast::{
        common::UseTreeKind,
        items::{EnumItem, StructItem, TypeAliasItem},
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
    },
    types::{AuthoredTypeRef, TypeRef},
};
use arcweft_source::SourceSpan;

use crate::types::TypeKind;

use super::{BoundNominalKind, BoundNominalTypeKey, source_span};

pub(super) struct NominalRecord<'a> {
    pub(super) key: BoundNominalTypeKey,
    module_path: CanonicalModulePath,
    module: &'a HirModule,
    declaration: NominalDeclaration<'a>,
    pub(super) source: SourceSpan,
}

enum NominalDeclaration<'a> {
    Struct(&'a StructItem),
    Enum(&'a EnumItem),
}

impl NominalRecord<'_> {
    pub(super) fn is_generic(&self) -> bool {
        match self.declaration {
            NominalDeclaration::Struct(item) => !item.generic_params().is_empty(),
            NominalDeclaration::Enum(item) => !item.generic_params().is_empty(),
        }
    }
}

struct AliasRecord<'a> {
    module_path: CanonicalModulePath,
    module: &'a HirModule,
    item: &'a TypeAliasItem,
    source: SourceSpan,
}

pub(super) struct NominalSchemaResolver<'a> {
    records: BTreeMap<(CanonicalModulePath, String), Vec<NominalRecord<'a>>>,
    aliases: BTreeMap<(CanonicalModulePath, String), Vec<AliasRecord<'a>>>,
}

pub(super) enum NominalResolutionError {
    Unknown,
    Alias(Vec<SourceSpan>),
    Ambiguous(Vec<SourceSpan>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NominalSchemaError {
    path: Vec<String>,
    reason: String,
}

impl NominalSchemaError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            path: Vec::new(),
            reason: reason.into(),
        }
    }

    fn within(mut self, segment: impl Into<String>) -> Self {
        self.path.insert(0, segment.into());
        self
    }
}

impl fmt::Display for NominalSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            formatter.write_str(&self.reason)
        } else {
            write!(formatter, "{}: {}", self.path.join(" -> "), self.reason)
        }
    }
}

impl<'a> NominalSchemaResolver<'a> {
    pub(super) fn new(project: &'a HirProject) -> Self {
        let mut records = BTreeMap::<_, Vec<_>>::new();
        let mut aliases = BTreeMap::<_, Vec<_>>::new();
        for (module_path, module) in project.modules() {
            for declaration in module.declarations() {
                let record = match declaration {
                    HirTopLevelDecl::Struct(item) => Some(NominalRecord {
                        key: BoundNominalTypeKey::new(
                            project.package().clone(),
                            module_path.clone(),
                            item.name(),
                            BoundNominalKind::Struct,
                        ),
                        module_path: module_path.clone(),
                        module,
                        declaration: NominalDeclaration::Struct(item),
                        source: source_span(module, *item.range()),
                    }),
                    HirTopLevelDecl::Enum(item) => Some(NominalRecord {
                        key: BoundNominalTypeKey::new(
                            project.package().clone(),
                            module_path.clone(),
                            item.name(),
                            BoundNominalKind::Enum,
                        ),
                        module_path: module_path.clone(),
                        module,
                        declaration: NominalDeclaration::Enum(item),
                        source: source_span(module, *item.range()),
                    }),
                    HirTopLevelDecl::TypeAlias(item) => {
                        aliases
                            .entry((module_path.clone(), item.name().to_owned()))
                            .or_default()
                            .push(AliasRecord {
                                module_path: module_path.clone(),
                                module,
                                item,
                                source: source_span(module, *item.range()),
                            });
                        None
                    }
                    _ => None,
                };
                if let Some(record) = record {
                    records
                        .entry((module_path.clone(), record.key.name().to_owned()))
                        .or_default()
                        .push(record);
                }
            }
        }
        Self { records, aliases }
    }

    pub(super) fn resolve_nominal(
        &'a self,
        current: &CanonicalModulePath,
        module: &HirModule,
        raw: &str,
    ) -> Result<&'a NominalRecord<'a>, NominalResolutionError> {
        let visible = visible_type_keys(current, module, raw);
        let mut candidates = visible
            .iter()
            .flat_map(|key| self.records.get(key).into_iter().flatten())
            .collect::<Vec<_>>();
        let mut aliases = visible
            .into_iter()
            .flat_map(|key| self.aliases.get(&key).into_iter().flatten())
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.key.cmp(&right.key));
        candidates.dedup_by(|left, right| left.key == right.key);
        aliases.sort_by(|left, right| {
            left.module_path
                .cmp(&right.module_path)
                .then_with(|| left.item.name().cmp(right.item.name()))
        });
        aliases.dedup_by(|left, right| {
            left.module_path == right.module_path && left.item.name() == right.item.name()
        });
        match (candidates.as_slice(), aliases.as_slice()) {
            ([record], []) => Ok(*record),
            ([], []) => Err(NominalResolutionError::Unknown),
            ([], aliases) => Err(NominalResolutionError::Alias(
                aliases.iter().map(|alias| alias.source.clone()).collect(),
            )),
            _ => Err(NominalResolutionError::Ambiguous(
                candidates
                    .iter()
                    .map(|record| record.source.clone())
                    .chain(aliases.iter().map(|alias| alias.source.clone()))
                    .collect(),
            )),
        }
    }

    pub(super) fn schema(
        &self,
        record: &NominalRecord<'_>,
    ) -> Result<TypeShape, NominalSchemaError> {
        self.schema_with_stack(record, &mut BTreeSet::new())
    }

    fn schema_with_stack(
        &self,
        record: &NominalRecord<'_>,
        stack: &mut BTreeSet<BoundNominalTypeKey>,
    ) -> Result<TypeShape, NominalSchemaError> {
        if !stack.insert(record.key.clone()) {
            return Ok(TypeShape::Named(canonical_nominal_name(&record.key)));
        }
        let result = match record.declaration {
            NominalDeclaration::Struct(item) => item
                .fields()
                .iter()
                .map(|field| {
                    self.type_shape(
                        &record.module_path,
                        record.module,
                        field.ty().value(),
                        stack,
                        &mut BTreeSet::new(),
                    )
                    .map_err(|error| error.within(format!("field `{}`", field.name())))
                    .map(|shape| FieldShape::new(field.name(), field.name(), shape))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|fields| TypeShape::record(canonical_nominal_name(&record.key), fields)),
            NominalDeclaration::Enum(item) => item
                .variants()
                .iter()
                .map(|variant| {
                    let unit = VariantShape::unit(variant.name(), variant.name());
                    let Some(payload) = variant.payload() else {
                        return Ok(unit);
                    };
                    self.type_shape(
                        &record.module_path,
                        record.module,
                        payload.value(),
                        stack,
                        &mut BTreeSet::new(),
                    )
                    .map_err(|error| error.within(format!("variant `{}` payload", variant.name())))
                    .map(|shape| unit.with_payload(shape))
                })
                .collect::<Result<Vec<_>, NominalSchemaError>>()
                .map(|variants| {
                    TypeShape::enumeration(canonical_nominal_name(&record.key), variants)
                }),
        };
        stack.remove(&record.key);
        result
    }

    fn type_shape(
        &self,
        current: &CanonicalModulePath,
        module: &HirModule,
        ty: &TypeRef,
        nominal_stack: &mut BTreeSet<BoundNominalTypeKey>,
        alias_stack: &mut BTreeSet<(CanonicalModulePath, String)>,
    ) -> Result<TypeShape, NominalSchemaError> {
        if let TypeRef::Path(path) = ty {
            let path_label = path.canonical_string();
            if let Ok(nominal) = self.resolve_nominal(current, module, &path_label) {
                return self.schema_with_stack(nominal, nominal_stack);
            }
            if let Some(alias) = self
                .resolve_alias(current, module, &path_label)
                .map_err(NominalSchemaError::new)?
            {
                let key = (alias.module_path.clone(), alias.item.name().to_owned());
                if !alias_stack.insert(key.clone()) {
                    return Err(NominalSchemaError::new(format!(
                        "recursive type alias `{}`",
                        alias.item.name()
                    )));
                }
                let shape = self.type_shape(
                    &alias.module_path,
                    alias.module,
                    alias.item.target().value(),
                    nominal_stack,
                    alias_stack,
                );
                alias_stack.remove(&key);
                return shape;
            }
        }

        self.type_kind_shape(
            current,
            module,
            TypeKind::from(ty),
            nominal_stack,
            alias_stack,
        )
    }

    fn type_kind_shape(
        &self,
        current: &CanonicalModulePath,
        module: &HirModule,
        kind: TypeKind,
        nominal_stack: &mut BTreeSet<BoundNominalTypeKey>,
        alias_stack: &mut BTreeSet<(CanonicalModulePath, String)>,
    ) -> Result<TypeShape, NominalSchemaError> {
        Ok(match kind {
            TypeKind::Unit => TypeShape::Unit,
            TypeKind::Bool => TypeShape::Bool,
            TypeKind::I8 => TypeShape::I8,
            TypeKind::I16 => TypeShape::I16,
            TypeKind::I32 => TypeShape::I32,
            TypeKind::I64 => TypeShape::I64,
            TypeKind::I128 => TypeShape::I128,
            TypeKind::ISize => TypeShape::Isize,
            TypeKind::U8 => TypeShape::U8,
            TypeKind::U16 => TypeShape::U16,
            TypeKind::U32 => TypeShape::U32,
            TypeKind::U64 => TypeShape::U64,
            TypeKind::U128 => TypeShape::U128,
            TypeKind::USize => TypeShape::Usize,
            TypeKind::F32 => TypeShape::F32,
            TypeKind::F64 => TypeShape::F64,
            TypeKind::String => TypeShape::String,
            TypeKind::Char => TypeShape::Char,
            TypeKind::Bytes => TypeShape::Bytes {
                format: arcweft_data::BytesFormat::Binary,
            },
            TypeKind::Option(inner) => TypeShape::option(
                self.type_kind_shape(current, module, *inner, nominal_stack, alias_stack)
                    .map_err(|error| error.within("optional value"))?,
            ),
            TypeKind::Vec(inner) | TypeKind::Seq(inner) => TypeShape::seq(
                self.type_kind_shape(current, module, *inner, nominal_stack, alias_stack)
                    .map_err(|error| error.within("sequence item"))?,
            ),
            TypeKind::Map { key, value, .. } => TypeShape::map(
                self.type_kind_shape(current, module, *key, nominal_stack, alias_stack)
                    .map_err(|error| error.within("map key"))?,
                self.type_kind_shape(current, module, *value, nominal_stack, alias_stack)
                    .map_err(|error| error.within("map value"))?,
            ),
            TypeKind::Named(path) => {
                if let Ok(nominal) = self.resolve_nominal(current, module, &path) {
                    self.schema_with_stack(nominal, nominal_stack)?
                } else if let Some(alias) = self
                    .resolve_alias(current, module, &path)
                    .map_err(NominalSchemaError::new)?
                {
                    let key = (alias.module_path.clone(), alias.item.name().to_owned());
                    if !alias_stack.insert(key.clone()) {
                        return Err(NominalSchemaError::new(format!(
                            "recursive type alias `{}`",
                            alias.item.name()
                        )));
                    }
                    let shape = self.type_shape(
                        &alias.module_path,
                        alias.module,
                        alias.item.target().value(),
                        nominal_stack,
                        alias_stack,
                    );
                    alias_stack.remove(&key);
                    shape?
                } else {
                    return Err(NominalSchemaError::new(format!(
                        "unresolved data type `{path}`"
                    )));
                }
            }
            unsupported => {
                return Err(NominalSchemaError::new(format!(
                    "semantic type `{}` is not a canonical data shape",
                    unsupported.source_label()
                )));
            }
        })
    }

    pub(super) fn resolve_alias_target(
        &self,
        current: &CanonicalModulePath,
        module: &HirModule,
        raw: &str,
    ) -> Result<Option<(&CanonicalModulePath, &HirModule, &AuthoredTypeRef, String)>, String> {
        self.resolve_alias(current, module, raw).map(|alias| {
            alias.map(|alias| {
                (
                    &alias.module_path,
                    alias.module,
                    alias.item.target(),
                    alias.item.name().to_owned(),
                )
            })
        })
    }

    fn resolve_alias(
        &self,
        current: &CanonicalModulePath,
        module: &HirModule,
        raw: &str,
    ) -> Result<Option<&AliasRecord<'a>>, String> {
        let candidates = visible_type_keys(current, module, raw)
            .into_iter()
            .flat_map(|key| self.aliases.get(&key).into_iter().flatten())
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [alias] => Ok(Some(*alias)),
            [] => Ok(None),
            _ => Err(format!("type path `{raw}` is ambiguous")),
        }
    }
}

fn canonical_nominal_name(key: &BoundNominalTypeKey) -> String {
    let kind = match key.kind() {
        BoundNominalKind::Struct => "struct",
        BoundNominalKind::Enum => "enum",
    };
    format!(
        "package={};module={};kind={kind};name={}",
        key.package().as_str(),
        key.module(),
        key.name()
    )
}

fn visible_type_keys(
    current: &CanonicalModulePath,
    module: &HirModule,
    raw: &str,
) -> Vec<(CanonicalModulePath, String)> {
    let normalized = raw.replace("::", ".");
    let mut keys = Vec::new();
    if !normalized.contains('.') {
        keys.push((current.clone(), normalized.clone()));
        for import in module.uses() {
            match import.tree().kind() {
                UseTreeKind::Path { path, alias } => {
                    let binding = alias.as_ref().map_or_else(
                        || path.path().last_segment().as_str(),
                        |alias| alias.name().as_str(),
                    );
                    if binding == normalized
                        && let Some(key) = nominal_key_from_project_path(current, path.path())
                    {
                        keys.push(key);
                    }
                }
                UseTreeKind::Group {
                    module: path,
                    names,
                } => {
                    if let Some(target_module) = module_from_project_path(current, path.path()) {
                        for name in names {
                            if name.binding_name() == normalized {
                                keys.push((target_module.clone(), name.name().as_str().to_owned()));
                            }
                        }
                    }
                }
                UseTreeKind::Glob { module: path } => {
                    if let Some(target_module) = module_from_project_path(current, path.path()) {
                        keys.push((target_module, normalized.clone()));
                    }
                }
            }
        }
    } else if let Ok(path) = ProjectSymbolPath::from_str(&normalized)
        && let Some(key) = nominal_key_from_project_path(current, &path)
    {
        keys.push(key);
    }
    keys.sort();
    keys.dedup();
    keys
}

fn nominal_key_from_project_path(
    current: &CanonicalModulePath,
    path: &ProjectSymbolPath,
) -> Option<(CanonicalModulePath, String)> {
    let (leaf, qualifiers) = path.segments().split_last()?;
    let module = resolve_module_segments(current, path.root(), qualifiers)?;
    Some((module, leaf.as_str().to_owned()))
}

fn module_from_project_path(
    current: &CanonicalModulePath,
    path: &ProjectSymbolPath,
) -> Option<CanonicalModulePath> {
    resolve_module_segments(current, path.root(), path.segments())
}

fn resolve_module_segments(
    current: &CanonicalModulePath,
    root: ModulePathRoot,
    segments: &[ProjectSymbolSegment],
) -> Option<CanonicalModulePath> {
    let mut module = match root {
        ModulePathRoot::ImplicitCrate | ModulePathRoot::Crate => CanonicalModulePath::crate_root(),
        ModulePathRoot::SelfModule => current.clone(),
        ModulePathRoot::Super(levels) => {
            let mut module = current.clone();
            for _ in 0..levels {
                module = module.parent()?;
            }
            module
        }
    };
    for segment in segments {
        module = module.join(ModuleSegment::new(segment.as_str().to_owned()).ok()?);
    }
    Some(module)
}
