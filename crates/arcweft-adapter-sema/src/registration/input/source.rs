//! Deterministic synthetic source and exact type-node ranges.

use std::{collections::BTreeMap, fmt::Write as _};

use arcweft_lang_hir::symbol::CallablePackageId;
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallableGroupIndex, CallableName, CallableParameterIndex, CallablePath,
        EnvironmentCallableOwner, RustItemPath,
    },
    env::nominal::RustPackageId,
    registration::{
        EnvironmentPublicationItemId, EnvironmentTypeSite, EnvironmentTypeSiteRoot,
        EnvironmentTypeSiteStep,
    },
};
use arcweft_rust_abi::{
    ArcweftRustField, ArcweftRustPackageId, ArcweftRustStructShape, ArcweftRustTypeKind,
    ArcweftRustTypePath, ArcweftRustTypeRef, ArcweftRustVariantPayload,
};
use arcweft_source::SourceRange;

use arcweft_adapter_context::manifest::{
    AdapterCallablePath, AdapterFreeCallableKind, AdapterFunctionSignature, AdapterManifest,
    AdapterNominalOwner, AdapterNominalPath, AdapterNominalTypeRef, AdapterToolingSubject,
    AdapterTypeKind,
};

use super::{
    AdapterRegistrationFactsError, callable_name, digest, nominal_path, ordinal, overload,
    project_callable_path, project_symbol_path,
};

pub(in crate::registration) struct RenderedRegistrationSource {
    pub(in crate::registration) text: String,
    pub(in crate::registration) symbols: Vec<RenderedRegistrationSymbol>,
    pub(in crate::registration) map: RegistrationSourceMap,
}

pub(in crate::registration) struct RenderedRegistrationSymbol {
    pub(in crate::registration) path: arcweft_adapter_context::manifest::AdapterSymbolPath,
    pub(in crate::registration) spelling: String,
    pub(in crate::registration) range: SourceRange,
}

struct AdapterCallableLine<'a> {
    item: EnvironmentPublicationItemId,
    kind: &'static str,
    receiver: Option<&'a AdapterTypeKind>,
    name: String,
    overload_index: usize,
    signature: &'a AdapterFunctionSignature,
    effects: &'a [arcweft_adapter_context::manifest::AdapterEffectCapability],
}

#[derive(Default)]
pub(in crate::registration) struct RegistrationSourceMap {
    items: BTreeMap<EnvironmentPublicationItemId, SourceRange>,
    opaque_producers: BTreeMap<EnvironmentPublicationItemId, SourceRange>,
    types: BTreeMap<(EnvironmentPublicationItemId, EnvironmentTypeSite), SourceRange>,
}

impl RegistrationSourceMap {
    pub(super) fn item_range(
        &self,
        item: &EnvironmentPublicationItemId,
    ) -> Result<SourceRange, AdapterRegistrationFactsError> {
        self.items.get(item).copied().ok_or_else(|| {
            AdapterRegistrationFactsError::MissingItemSource {
                item: Box::new(item.clone()),
            }
        })
    }

    pub(super) fn type_range(
        &self,
        item: &EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        steps: &[EnvironmentTypeSiteStep],
    ) -> Result<SourceRange, AdapterRegistrationFactsError> {
        let site = EnvironmentTypeSite::new(root, steps.to_vec().into_boxed_slice());
        self.types
            .get(&(item.clone(), site.clone()))
            .copied()
            .ok_or_else(|| AdapterRegistrationFactsError::MissingTypeSourceSite {
                item: Box::new(item.clone()),
                site: Box::new(site),
            })
    }

    pub(super) fn opaque_producer_range(
        &self,
        item: &EnvironmentPublicationItemId,
    ) -> Result<SourceRange, AdapterRegistrationFactsError> {
        self.opaque_producers.get(item).copied().ok_or_else(|| {
            AdapterRegistrationFactsError::MissingOpaqueProducerSource {
                item: Box::new(item.clone()),
            }
        })
    }

    fn insert_item(
        &mut self,
        item: EnvironmentPublicationItemId,
        range: SourceRange,
    ) -> Result<(), AdapterRegistrationFactsError> {
        if self.items.insert(item.clone(), range).is_some() {
            return Err(AdapterRegistrationFactsError::DuplicateItemSource {
                item: Box::new(item),
            });
        }
        Ok(())
    }

    fn insert_type(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        steps: &[EnvironmentTypeSiteStep],
        range: SourceRange,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let site = EnvironmentTypeSite::new(root, steps.to_vec().into_boxed_slice());
        if self
            .types
            .insert((item.clone(), site.clone()), range)
            .is_some()
        {
            return Err(AdapterRegistrationFactsError::DuplicateTypeSourceSite {
                item: Box::new(item.clone()),
                site: Box::new(site),
            });
        }
        Ok(())
    }

    fn insert_opaque_producer(
        &mut self,
        item: EnvironmentPublicationItemId,
        range: SourceRange,
    ) -> Result<(), AdapterRegistrationFactsError> {
        if self.opaque_producers.insert(item.clone(), range).is_some() {
            return Err(
                AdapterRegistrationFactsError::DuplicateOpaqueProducerSource {
                    item: Box::new(item),
                },
            );
        }
        Ok(())
    }
}

pub(in crate::registration) fn render(
    manifest: &AdapterManifest,
    owner: &EnvironmentCallableOwner,
) -> Result<RenderedRegistrationSource, AdapterRegistrationFactsError> {
    let adapter = AdapterPackageId::try_new(manifest.id().as_str())?;
    let callable_package = CallablePackageId::try_new(manifest.id().as_str())?;
    let mut renderer = Renderer::default();
    render_header_and_mounts(&mut renderer, manifest);
    render_nominal_declarations(&mut renderer, manifest, owner)?;
    render_rust_types(&mut renderer, manifest, &adapter)?;
    let rendered_symbols = render_symbols(&mut renderer, manifest, owner)?;
    render_methods(&mut renderer, manifest, owner)?;
    render_functions(&mut renderer, manifest, owner, &callable_package)?;
    render_rust_functions(&mut renderer, manifest, &adapter, &callable_package)?;
    render_effects_and_host_calls(&mut renderer, manifest, owner)?;
    render_tooling_docs(&mut renderer, manifest);

    Ok(RenderedRegistrationSource {
        text: renderer.text,
        symbols: rendered_symbols,
        map: renderer.map,
    })
}

fn render_header_and_mounts(renderer: &mut Renderer, manifest: &AdapterManifest) {
    renderer.line(|text| {
        text.push_str("adapter-manifest-v1 id=");
        scalar(text, manifest.id().as_str());
        text.push_str(" owner=");
        scalar(text, &format!("adapter:{}", manifest.id().as_str()));
        text.push_str(" display=");
        scalar(text, manifest.display_name());
    });
    for (package, prefix) in manifest.rust_package_mounts().iter() {
        renderer.line(|text| {
            text.push_str("mount package=");
            scalar(text, package.as_str());
            text.push_str(" prefix=");
            path_segments(
                text,
                prefix
                    .segments()
                    .iter()
                    .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str),
            );
        });
    }
}

fn render_nominal_declarations(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    owner: &EnvironmentCallableOwner,
) -> Result<(), AdapterRegistrationFactsError> {
    let mut declarations = manifest.nominal_declarations().iter().collect::<Vec<_>>();
    declarations.sort_by(|left, right| left.path().segments().cmp(right.path().segments()));
    for declaration in declarations {
        let item = EnvironmentPublicationItemId::AdapterNominal {
            owner: owner.clone(),
            path: nominal_path(declaration.path())?,
        };
        renderer.nominal_line(item, declaration)?;
    }
    Ok(())
}

fn render_rust_types(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    adapter: &AdapterPackageId,
) -> Result<(), AdapterRegistrationFactsError> {
    let mut rust_types = manifest.rust_types().iter().collect::<Vec<_>>();
    rust_types.sort_by(|left, right| {
        left.package().id.cmp(&right.package().id).then_with(|| {
            left.decl()
                .path
                .segments()
                .cmp(right.decl().path.segments())
        })
    });
    for rust_type in rust_types {
        renderer.rust_type_line(
            EnvironmentPublicationItemId::RustType {
                adapter: adapter.clone(),
                package: RustPackageId::try_new(rust_type.package().id.as_str())?,
                rust_item: RustItemPath::try_new(rust_type.decl().rust_path.as_str())?,
                accepted_path: nominal_path(rust_type.accepted_path())?,
            },
            rust_type,
        )?;
    }
    Ok(())
}

fn render_symbols(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    owner: &EnvironmentCallableOwner,
) -> Result<Vec<RenderedRegistrationSymbol>, AdapterRegistrationFactsError> {
    let mut symbols = manifest.symbols().iter().collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.path().cmp(right.path()));
    symbols
        .into_iter()
        .map(|symbol| {
            let item = EnvironmentPublicationItemId::AdapterSymbol {
                owner: owner.clone(),
                path: project_symbol_path(symbol.path())?,
            };
            let (spelling, range) = renderer.symbol_line(&item, symbol)?;
            Ok(RenderedRegistrationSymbol {
                path: symbol.path().clone(),
                spelling,
                range,
            })
        })
        .collect()
}

fn render_methods(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    owner: &EnvironmentCallableOwner,
) -> Result<(), AdapterRegistrationFactsError> {
    let mut methods = manifest.methods().iter().collect::<Vec<_>>();
    methods.sort_by(|left, right| {
        digest::type_digest(left.receiver())
            .cmp(&digest::type_digest(right.receiver()))
            .then_with(|| left.callable_name().cmp(right.callable_name()))
            .then_with(|| left.overload().cmp(&right.overload()))
    });
    for (index, method) in methods.iter().enumerate() {
        renderer.adapter_callable_line(AdapterCallableLine {
            item: EnvironmentPublicationItemId::AdapterMethod {
                owner: owner.clone(),
                receiver: digest::type_digest(method.receiver()),
                method: callable_name(method.callable_name())?,
                overload: overload(method.overload().get())?,
                declaration_order: ordinal(index)?,
            },
            kind: "method",
            receiver: Some(method.receiver()),
            name: method.callable_name().as_str().to_owned(),
            overload_index: method.overload().get(),
            signature: method.signature(),
            effects: method.effects(),
        })?;
    }
    Ok(())
}

fn render_functions(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    owner: &EnvironmentCallableOwner,
    callable_package: &CallablePackageId,
) -> Result<(), AdapterRegistrationFactsError> {
    let mut functions = manifest.functions().iter().collect::<Vec<_>>();
    functions.sort_by(|left, right| {
        left.path()
            .segments()
            .cmp(right.path().segments())
            .then_with(|| left.overload().cmp(&right.overload()))
    });
    for function in functions {
        renderer.adapter_callable_line(AdapterCallableLine {
            item: EnvironmentPublicationItemId::AdapterFunction {
                owner: owner.clone(),
                path: project_callable_path(callable_package, function.path())?,
                overload: overload(function.overload().get())?,
            },
            kind: "function",
            receiver: None,
            name: callable_path_text(function.path()),
            overload_index: function.overload().get(),
            signature: function.signature(),
            effects: function.effects(),
        })?;
    }
    Ok(())
}

fn render_rust_functions(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    adapter: &AdapterPackageId,
    callable_package: &CallablePackageId,
) -> Result<(), AdapterRegistrationFactsError> {
    let mut functions = manifest.rust_functions().iter().collect::<Vec<_>>();
    functions.sort_by(|left, right| {
        left.package()
            .id
            .cmp(&right.package().id)
            .then_with(|| left.rust_path().cmp(right.rust_path()))
            .then_with(|| left.path().segments().cmp(right.path().segments()))
            .then_with(|| left.overload().cmp(&right.overload()))
    });
    for function in functions {
        renderer.adapter_callable_line(AdapterCallableLine {
            item: EnvironmentPublicationItemId::RustFunction {
                adapter: adapter.clone(),
                package: RustPackageId::try_new(function.package().id.as_str())?,
                rust_item: RustItemPath::try_new(function.rust_path())?,
                callable_path: project_callable_path(callable_package, function.path())?,
                overload: overload(function.overload().get())?,
            },
            kind: "rust-function",
            receiver: None,
            name: callable_path_text(function.path()),
            overload_index: function.overload().get(),
            signature: function.signature(),
            effects: function.effects(),
        })?;
    }
    Ok(())
}

fn render_effects_and_host_calls(
    renderer: &mut Renderer,
    manifest: &AdapterManifest,
    owner: &EnvironmentCallableOwner,
) -> Result<(), AdapterRegistrationFactsError> {
    let mut effects = manifest.effects().iter().collect::<Vec<_>>();
    effects.sort();
    for effect in effects {
        renderer.line(|text| {
            text.push_str("effect id=");
            scalar(text, effect.as_str());
        });
    }
    let mut host_calls = manifest.host_calls().iter().collect::<Vec<_>>();
    host_calls.sort_by_key(|call| call.id());
    for call in host_calls {
        let path = CallablePath::try_new(
            call.id()
                .split('.')
                .map(|segment| CallableName::try_new(segment.to_owned()))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        renderer.host_call_line(
            EnvironmentPublicationItemId::AdapterHostCall {
                owner: owner.clone(),
                path,
            },
            call,
        )?;
    }
    Ok(())
}

fn render_tooling_docs(renderer: &mut Renderer, manifest: &AdapterManifest) {
    let mut docs = manifest.tooling_docs().iter().collect::<Vec<_>>();
    docs.sort_by_key(|doc| tooling_subject_key(doc.subject()));
    for doc in docs {
        renderer.line(|text| {
            text.push_str("tooling subject=");
            render_tooling_subject(text, doc.subject());
            text.push_str(" summary=");
            optional_scalar(text, doc.summary());
            text.push_str(" details=");
            optional_scalar(text, doc.details());
            text.push_str(" parameters=[");
            for (index, parameter) in doc.parameters().iter().enumerate() {
                if index > 0 {
                    text.push(',');
                }
                write!(
                    text,
                    "{}:{}=",
                    parameter.group().get(),
                    parameter.parameter().get()
                )
                .expect("writing to String cannot fail");
                scalar(text, parameter.text());
            }
            text.push(']');
        });
    }
}

#[derive(Default)]
struct Renderer {
    text: String,
    map: RegistrationSourceMap,
}

impl Renderer {
    fn line(&mut self, render: impl FnOnce(&mut String)) {
        render(&mut self.text);
        self.text.push('\n');
    }

    fn nominal_line(
        &mut self,
        item: EnvironmentPublicationItemId,
        declaration: &arcweft_adapter_context::manifest::AdapterNominalDeclaration,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let start = self.text.len();
        self.text.push_str("nominal path=");
        adapter_path(&mut self.text, declaration.path());
        write!(&mut self.text, " arity={}", declaration.arity())
            .expect("writing to String cannot fail");
        self.text.push_str(" producer=");
        let producer_range = scalar_payload(&mut self.text, declaration.opaque_producer().as_str());
        self.text.push_str(" visibility=");
        self.text.push_str(match declaration.visibility() {
            arcweft_adapter_context::manifest::AdapterNominalVisibility::Public => "public",
            arcweft_adapter_context::manifest::AdapterNominalVisibility::Private => "private",
        });
        self.text.push_str(" label=");
        scalar(&mut self.text, declaration.source_label());
        let end = self.text.len();
        self.text.push('\n');
        self.map
            .insert_item(item.clone(), SourceRange::new(start, end))?;
        self.map.insert_opaque_producer(item, producer_range)
    }

    fn symbol_line(
        &mut self,
        item: &EnvironmentPublicationItemId,
        symbol: &arcweft_adapter_context::manifest::AdapterSymbol,
    ) -> Result<(String, SourceRange), AdapterRegistrationFactsError> {
        let item_start = self.text.len();
        self.text.push_str("symbol path=");
        let path_start = self.text.len();
        let spelling = symbol.path().to_string();
        self.text.push_str(&spelling);
        let path_end = self.text.len();
        self.text.push_str(" type=");
        self.render_adapter_type(
            item,
            EnvironmentTypeSiteRoot::SymbolType,
            &mut Vec::new(),
            symbol.ty(),
        )?;
        let item_end = self.text.len();
        self.text.push('\n');
        self.map
            .insert_item(item.clone(), SourceRange::new(item_start, item_end))?;
        Ok((spelling, SourceRange::new(path_start, path_end)))
    }

    fn adapter_callable_line(
        &mut self,
        line: AdapterCallableLine<'_>,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let item_start = self.text.len();
        self.text.push_str(line.kind);
        if let Some(receiver) = line.receiver {
            self.text.push_str(" receiver=");
            self.render_adapter_type(
                &line.item,
                EnvironmentTypeSiteRoot::MethodReceiver,
                &mut Vec::new(),
                receiver,
            )?;
        }
        self.text.push_str(" name=");
        scalar(&mut self.text, &line.name);
        write!(
            &mut self.text,
            " overload={} signature=",
            line.overload_index
        )
        .expect("writing to String cannot fail");
        self.render_signature(&line.item, line.signature)?;
        render_effects(&mut self.text, line.effects);
        let item_end = self.text.len();
        self.text.push('\n');
        self.map
            .insert_item(line.item, SourceRange::new(item_start, item_end))
    }

    fn host_call_line(
        &mut self,
        item: EnvironmentPublicationItemId,
        call: &arcweft_adapter_context::manifest::AdapterHostCall,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let item_start = self.text.len();
        self.text.push_str("host-call id=");
        scalar(&mut self.text, call.id());
        self.text.push_str(" signature=");
        self.render_signature(&item, call.signature())?;
        if let Some(domain_error) = call.domain_error() {
            self.text.push_str(" domain-error=");
            self.render_adapter_type(
                &item,
                EnvironmentTypeSiteRoot::HostCallDomainError,
                &mut Vec::new(),
                domain_error,
            )?;
        }
        render_effects(&mut self.text, call.effects());
        let item_end = self.text.len();
        self.text.push('\n');
        self.map
            .insert_item(item, SourceRange::new(item_start, item_end))
    }

    fn render_signature(
        &mut self,
        item: &EnvironmentPublicationItemId,
        signature: &AdapterFunctionSignature,
    ) -> Result<(), AdapterRegistrationFactsError> {
        self.text.push('(');
        for (group_offset, group) in signature.groups().iter().enumerate() {
            if group_offset > 0 {
                self.text.push_str(")(");
            }
            let group_index = CallableGroupIndex::try_from_usize(group.index().get())?;
            for (parameter_offset, parameter) in group.parameters().iter().enumerate() {
                if parameter_offset > 0 {
                    self.text.push_str(", ");
                }
                if let Some(name) = parameter.name() {
                    scalar(&mut self.text, name.as_str());
                    self.text.push(':');
                }
                self.render_adapter_type(
                    item,
                    EnvironmentTypeSiteRoot::Parameter {
                        group: group_index,
                        parameter: CallableParameterIndex::try_from_usize(parameter.index().get())?,
                    },
                    &mut Vec::new(),
                    parameter.ty(),
                )?;
            }
        }
        self.text.push_str(")->");
        self.render_adapter_type(
            item,
            EnvironmentTypeSiteRoot::Result,
            &mut Vec::new(),
            signature.return_type(),
        )
    }

    fn render_adapter_type(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        ty: &AdapterTypeKind,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let start = self.text.len();
        match ty {
            AdapterTypeKind::Unit => self.text.push_str("Unit"),
            AdapterTypeKind::Bool => self.text.push_str("bool"),
            AdapterTypeKind::I8 => self.text.push_str("i8"),
            AdapterTypeKind::I16 => self.text.push_str("i16"),
            AdapterTypeKind::I32 => self.text.push_str("i32"),
            AdapterTypeKind::I64 => self.text.push_str("i64"),
            AdapterTypeKind::I128 => self.text.push_str("i128"),
            AdapterTypeKind::ISize => self.text.push_str("isize"),
            AdapterTypeKind::U8 => self.text.push_str("u8"),
            AdapterTypeKind::U16 => self.text.push_str("u16"),
            AdapterTypeKind::U32 => self.text.push_str("u32"),
            AdapterTypeKind::U64 => self.text.push_str("u64"),
            AdapterTypeKind::U128 => self.text.push_str("u128"),
            AdapterTypeKind::USize => self.text.push_str("usize"),
            AdapterTypeKind::F32 => self.text.push_str("f32"),
            AdapterTypeKind::F64 => self.text.push_str("f64"),
            AdapterTypeKind::String => self.text.push_str("String"),
            AdapterTypeKind::Char => self.text.push_str("char"),
            AdapterTypeKind::Vec { item: child } => {
                self.text.push_str("Vec<");
                self.render_adapter_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::VecItem,
                    child,
                )?;
                self.text.push('>');
            }
            AdapterTypeKind::Seq { item: child } => {
                self.text.push_str("Seq<");
                self.render_adapter_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::SeqItem,
                    child,
                )?;
                self.text.push('>');
            }
            AdapterTypeKind::Option { item: child } => {
                self.text.push_str("Option<");
                self.render_adapter_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::OptionItem,
                    child,
                )?;
                self.text.push('>');
            }
            AdapterTypeKind::Result { ok, error } => {
                self.text.push_str("Result<");
                self.render_adapter_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::ResultOk,
                    ok,
                )?;
                self.text.push(',');
                self.render_adapter_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::ResultError,
                    error,
                )?;
                self.text.push('>');
            }
            AdapterTypeKind::Tuple { items } => {
                self.render_adapter_tuple(item, &root, steps, items)?;
            }
            AdapterTypeKind::Need { item: payload } => {
                self.render_adapter_need(item, &root, steps, payload)?;
            }
            AdapterTypeKind::Nominal { nominal } => {
                self.render_adapter_nominal(item, &root, steps, nominal)?;
            }
        }
        let end = self.text.len();
        self.map
            .insert_type(item, root, steps, SourceRange::new(start, end))
    }

    fn render_adapter_tuple(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: &EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        items: &[AdapterTypeKind],
    ) -> Result<(), AdapterRegistrationFactsError> {
        self.text.push('(');
        for (index, child) in items.iter().enumerate() {
            if index > 0 {
                self.text.push(',');
            }
            self.render_adapter_child(
                item,
                root.to_owned(),
                steps,
                EnvironmentTypeSiteStep::TupleItem(u16::try_from(index).map_err(|_| {
                    AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index }
                })?),
                child,
            )?;
        }
        self.text.push(')');
        Ok(())
    }

    fn render_adapter_need(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: &EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        payload: &AdapterTypeKind,
    ) -> Result<(), AdapterRegistrationFactsError> {
        self.text.push_str("Need<");
        self.render_adapter_child(
            item,
            root.to_owned(),
            steps,
            EnvironmentTypeSiteStep::NeedItem,
            payload,
        )?;
        self.text.push('>');
        Ok(())
    }

    fn render_adapter_nominal(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: &EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        nominal: &AdapterNominalTypeRef,
    ) -> Result<(), AdapterRegistrationFactsError> {
        match nominal.owner() {
            AdapterNominalOwner::Standard => self.text.push_str("standard"),
            AdapterNominalOwner::Environment { owner } => {
                self.text.push_str("environment[");
                scalar(&mut self.text, owner.as_str());
            }
            AdapterNominalOwner::RustPackage { package } => {
                self.text.push_str("rust[");
                scalar(&mut self.text, package.as_str());
            }
        }
        self.text.push_str("]::");
        adapter_path(&mut self.text, nominal.path());
        if !nominal.arguments().is_empty() {
            self.text.push('<');
            for (index, child) in nominal.arguments().iter().enumerate() {
                if index > 0 {
                    self.text.push(',');
                }
                self.render_adapter_child(
                    item,
                    root.to_owned(),
                    steps,
                    EnvironmentTypeSiteStep::NominalArgument(u16::try_from(index).map_err(
                        |_| AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index },
                    )?),
                    child,
                )?;
            }
            self.text.push('>');
        }
        Ok(())
    }

    fn render_adapter_child(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        step: EnvironmentTypeSiteStep,
        child: &AdapterTypeKind,
    ) -> Result<(), AdapterRegistrationFactsError> {
        steps.push(step);
        let result = self.render_adapter_type(item, root, steps, child);
        steps.pop();
        result
    }

    fn rust_type_line(
        &mut self,
        item: EnvironmentPublicationItemId,
        rust_type: &arcweft_adapter_context::manifest::AdapterRustType,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let start = self.text.len();
        self.text.push_str("rust-type package=");
        scalar(&mut self.text, rust_type.package().id.as_str());
        self.text.push_str(" accepted=");
        adapter_path(&mut self.text, rust_type.accepted_path());
        self.text.push_str(" producer=");
        let producer_range = scalar_payload(&mut self.text, rust_type.opaque_producer().as_str());
        self.text.push_str(" rust-item=");
        scalar(&mut self.text, &rust_type.decl().rust_path);
        self.text.push_str(" shape=");
        self.render_rust_metadata(&item, &rust_type.decl().kind)?;
        let end = self.text.len();
        self.text.push('\n');
        self.map
            .insert_item(item.clone(), SourceRange::new(start, end))?;
        self.map.insert_opaque_producer(item, producer_range)
    }

    fn render_rust_metadata(
        &mut self,
        item: &EnvironmentPublicationItemId,
        kind: &ArcweftRustTypeKind,
    ) -> Result<(), AdapterRegistrationFactsError> {
        match kind {
            ArcweftRustTypeKind::Struct { shape } => match shape {
                ArcweftRustStructShape::Unit => self.text.push_str("struct"),
                ArcweftRustStructShape::Tuple { fields } => {
                    self.text.push_str("struct(");
                    for (index, field) in fields.iter().enumerate() {
                        if index > 0 {
                            self.text.push(',');
                        }
                        self.render_rust_type(
                            item,
                            EnvironmentTypeSiteRoot::RustStructTupleField {
                                field: u16::try_from(index).map_err(|_| {
                                    AdapterRegistrationFactsError::RustFieldIndexOverflow {
                                        value: index,
                                    }
                                })?,
                            },
                            &mut Vec::new(),
                            field,
                        )?;
                    }
                    self.text.push(')');
                }
                ArcweftRustStructShape::Record { fields } => {
                    self.text.push_str("struct{");
                    self.render_rust_record_fields(item, None, fields)?;
                    self.text.push('}');
                }
            },
            ArcweftRustTypeKind::Enum { variants } => {
                self.text.push_str("enum{");
                for (variant_index, variant) in variants.iter().enumerate() {
                    if variant_index > 0 {
                        self.text.push(',');
                    }
                    scalar(&mut self.text, &variant.name);
                    match &variant.payload {
                        ArcweftRustVariantPayload::Unit => {}
                        ArcweftRustVariantPayload::Tuple { fields } => {
                            self.text.push('(');
                            for (index, field) in fields.iter().enumerate() {
                                if index > 0 {
                                    self.text.push(',');
                                }
                                self.render_rust_type(
                                    item,
                                    EnvironmentTypeSiteRoot::RustEnumTupleField {
                                        variant: variant.name.clone(),
                                        field: u16::try_from(index).map_err(|_| {
                                            AdapterRegistrationFactsError::RustFieldIndexOverflow {
                                                value: index,
                                            }
                                        })?,
                                    },
                                    &mut Vec::new(),
                                    field,
                                )?;
                            }
                            self.text.push(')');
                        }
                        ArcweftRustVariantPayload::Record { fields } => {
                            self.text.push('{');
                            self.render_rust_record_fields(item, Some(&variant.name), fields)?;
                            self.text.push('}');
                        }
                    }
                }
                self.text.push('}');
            }
            ArcweftRustTypeKind::Newtype { inner } => {
                self.text.push_str("newtype(");
                self.render_rust_type(
                    item,
                    EnvironmentTypeSiteRoot::RustNewtypeInner,
                    &mut Vec::new(),
                    inner,
                )?;
                self.text.push(')');
            }
        }
        Ok(())
    }

    fn render_rust_record_fields(
        &mut self,
        item: &EnvironmentPublicationItemId,
        variant: Option<&str>,
        fields: &[ArcweftRustField],
    ) -> Result<(), AdapterRegistrationFactsError> {
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                self.text.push(',');
            }
            scalar(&mut self.text, &field.name);
            self.text.push(':');
            let root = variant.map_or_else(
                || EnvironmentTypeSiteRoot::RustStructRecordField {
                    field: field.name.clone(),
                },
                |variant| EnvironmentTypeSiteRoot::RustEnumRecordField {
                    variant: variant.to_owned(),
                    field: field.name.clone(),
                },
            );
            self.render_rust_type(item, root, &mut Vec::new(), &field.ty)?;
        }
        Ok(())
    }

    fn render_rust_type(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        ty: &ArcweftRustTypeRef,
    ) -> Result<(), AdapterRegistrationFactsError> {
        let start = self.text.len();
        match ty {
            ArcweftRustTypeRef::Unit => self.text.push_str("Unit"),
            ArcweftRustTypeRef::Bool => self.text.push_str("bool"),
            ArcweftRustTypeRef::I8 => self.text.push_str("i8"),
            ArcweftRustTypeRef::I16 => self.text.push_str("i16"),
            ArcweftRustTypeRef::I32 => self.text.push_str("i32"),
            ArcweftRustTypeRef::I64 => self.text.push_str("i64"),
            ArcweftRustTypeRef::I128 => self.text.push_str("i128"),
            ArcweftRustTypeRef::ISize => self.text.push_str("isize"),
            ArcweftRustTypeRef::U8 => self.text.push_str("u8"),
            ArcweftRustTypeRef::U16 => self.text.push_str("u16"),
            ArcweftRustTypeRef::U32 => self.text.push_str("u32"),
            ArcweftRustTypeRef::U64 => self.text.push_str("u64"),
            ArcweftRustTypeRef::U128 => self.text.push_str("u128"),
            ArcweftRustTypeRef::USize => self.text.push_str("usize"),
            ArcweftRustTypeRef::F32 => self.text.push_str("f32"),
            ArcweftRustTypeRef::F64 => self.text.push_str("f64"),
            ArcweftRustTypeRef::String => self.text.push_str("String"),
            ArcweftRustTypeRef::Char => self.text.push_str("char"),
            ArcweftRustTypeRef::Vec { item: child }
            | ArcweftRustTypeRef::Seq { item: child }
            | ArcweftRustTypeRef::Option { item: child } => {
                let (name, step) = match ty {
                    ArcweftRustTypeRef::Vec { .. } => ("Vec", EnvironmentTypeSiteStep::VecItem),
                    ArcweftRustTypeRef::Seq { .. } => ("Seq", EnvironmentTypeSiteStep::SeqItem),
                    ArcweftRustTypeRef::Option { .. } => {
                        ("Option", EnvironmentTypeSiteStep::OptionItem)
                    }
                    _ => unreachable!(),
                };
                self.text.push_str(name);
                self.text.push('<');
                self.render_rust_child(item, root.clone(), steps, step, child)?;
                self.text.push('>');
            }
            ArcweftRustTypeRef::Result { ok, error } => {
                self.text.push_str("Result<");
                self.render_rust_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::ResultOk,
                    ok,
                )?;
                self.text.push(',');
                self.render_rust_child(
                    item,
                    root.clone(),
                    steps,
                    EnvironmentTypeSiteStep::ResultError,
                    error,
                )?;
                self.text.push('>');
            }
            ArcweftRustTypeRef::Tuple { items } => {
                self.render_rust_tuple(item, &root, steps, items)?;
            }
            ArcweftRustTypeRef::Nominal {
                package,
                path,
                arguments,
            } => self.render_rust_nominal(item, &root, steps, package, path, arguments)?,
            ArcweftRustTypeRef::TypeParameter { index } => {
                write!(&mut self.text, "T{}", index.get()).expect("writing to String cannot fail");
            }
        }
        let end = self.text.len();
        self.map
            .insert_type(item, root, steps, SourceRange::new(start, end))
    }

    fn render_rust_tuple(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: &EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        items: &[ArcweftRustTypeRef],
    ) -> Result<(), AdapterRegistrationFactsError> {
        self.text.push('(');
        for (index, child) in items.iter().enumerate() {
            if index > 0 {
                self.text.push(',');
            }
            self.render_rust_child(
                item,
                root.to_owned(),
                steps,
                EnvironmentTypeSiteStep::TupleItem(u16::try_from(index).map_err(|_| {
                    AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index }
                })?),
                child,
            )?;
        }
        self.text.push(')');
        Ok(())
    }

    fn render_rust_nominal(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: &EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        package: &ArcweftRustPackageId,
        path: &ArcweftRustTypePath,
        arguments: &[ArcweftRustTypeRef],
    ) -> Result<(), AdapterRegistrationFactsError> {
        self.text.push_str("rust[");
        scalar(&mut self.text, package.as_str());
        self.text.push_str("]::");
        path_segments(
            &mut self.text,
            path.segments()
                .iter()
                .map(arcweft_rust_abi::ArcweftRustTypePathSegment::as_str),
        );
        if !arguments.is_empty() {
            self.text.push('<');
            for (index, child) in arguments.iter().enumerate() {
                if index > 0 {
                    self.text.push(',');
                }
                self.render_rust_child(
                    item,
                    root.to_owned(),
                    steps,
                    EnvironmentTypeSiteStep::NominalArgument(u16::try_from(index).map_err(
                        |_| AdapterRegistrationFactsError::TypeSiteIndexOverflow { value: index },
                    )?),
                    child,
                )?;
            }
            self.text.push('>');
        }
        Ok(())
    }

    fn render_rust_child(
        &mut self,
        item: &EnvironmentPublicationItemId,
        root: EnvironmentTypeSiteRoot,
        steps: &mut Vec<EnvironmentTypeSiteStep>,
        step: EnvironmentTypeSiteStep,
        child: &ArcweftRustTypeRef,
    ) -> Result<(), AdapterRegistrationFactsError> {
        steps.push(step);
        let result = self.render_rust_type(item, root, steps, child);
        steps.pop();
        result
    }
}

fn render_unmapped_adapter_type(text: &mut String, ty: &AdapterTypeKind) {
    let digest = digest::type_digest(ty);
    text.push_str("type#");
    for byte in digest.as_bytes() {
        write!(text, "{byte:02x}").expect("writing to String cannot fail");
    }
}

fn render_effects(
    text: &mut String,
    effects: &[arcweft_adapter_context::manifest::AdapterEffectCapability],
) {
    let mut effects = effects
        .iter()
        .map(arcweft_adapter_context::manifest::AdapterEffectCapability::as_str)
        .collect::<Vec<_>>();
    effects.sort_unstable();
    text.push_str(" effects=[");
    for (index, effect) in effects.into_iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        scalar(text, effect);
    }
    text.push(']');
}

fn render_tooling_subject(text: &mut String, subject: &AdapterToolingSubject) {
    match subject {
        AdapterToolingSubject::Free {
            kind,
            path,
            overload,
        } => {
            text.push_str(match kind {
                AdapterFreeCallableKind::Function => "function:",
                AdapterFreeCallableKind::RustFunction => "rust-function:",
            });
            scalar(text, &callable_path_text(path));
            write!(text, "#{}", overload.get()).expect("writing to String cannot fail");
        }
        AdapterToolingSubject::Method {
            receiver,
            name,
            overload,
        } => {
            render_unmapped_adapter_type(text, receiver);
            text.push('.');
            scalar(text, name.as_str());
            write!(text, "#{}", overload.get()).expect("writing to String cannot fail");
        }
    }
}

fn tooling_subject_key(subject: &AdapterToolingSubject) -> [u8; 32] {
    let mut text = String::new();
    render_tooling_subject(&mut text, subject);
    *blake3::hash(text.as_bytes()).as_bytes()
}

fn callable_path_text(path: &AdapterCallablePath) -> String {
    path.segments()
        .iter()
        .map(arcweft_adapter_context::manifest::AdapterCallableName::as_str)
        .collect::<Vec<_>>()
        .join("::")
}

fn adapter_path(text: &mut String, path: &AdapterNominalPath) {
    path_segments(
        text,
        path.segments()
            .iter()
            .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str),
    );
}

fn path_segments<'a>(text: &mut String, segments: impl Iterator<Item = &'a str>) {
    for (index, segment) in segments.enumerate() {
        if index > 0 {
            text.push_str("::");
        }
        scalar(text, segment);
    }
}

fn optional_scalar(text: &mut String, value: Option<&str>) {
    match value {
        Some(value) => scalar(text, value),
        None => text.push('-'),
    }
}

fn scalar(text: &mut String, value: &str) {
    write!(text, "{}:", value.len()).expect("writing to String cannot fail");
    text.push_str(value);
}

fn scalar_payload(text: &mut String, value: &str) -> SourceRange {
    write!(text, "{}:", value.len()).expect("writing to String cannot fail");
    let start = text.len();
    text.push_str(value);
    SourceRange::new(start, text.len())
}
