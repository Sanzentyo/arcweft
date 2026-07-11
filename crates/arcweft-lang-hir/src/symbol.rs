//! Original callable declaration identities shared by linking and lowering.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use arcweft_lang_syntax::ast::{
    common::{UseItem, UseTreeKind, Visibility},
    module_path::{
        CanonicalModulePath, ModulePath, ModulePathError, ModulePathRoot, ModuleSegment,
    },
};
use thiserror::Error;

use crate::{model::HirFunction, project::HirProject};

/// Canonical package component of a callable declaration identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallablePackageId(String);

/// Identity of the original callable declaration before imports or re-exports.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: ModuleSegment,
}

/// One callable declaration indexed independently from all import aliases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSymbol {
    declaration: CallableDeclarationId,
    visibility: Option<Visibility>,
    fx: bool,
}

/// Module-aware callable bindings for one package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSymbolTable {
    package: CallablePackageId,
    modules: BTreeSet<CanonicalModulePath>,
    symbols: BTreeMap<CallableDeclarationId, CallableSymbol>,
    scopes: BTreeMap<CanonicalModulePath, BTreeMap<String, Vec<ScopeBinding>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopeBinding {
    target: SymbolTarget,
    visibility: Option<Visibility>,
    owner: CanonicalModulePath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SymbolTarget {
    Callable(CallableDeclarationId),
    Module(CanonicalModulePath),
}

/// Invalid canonical callable package identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallablePackageIdError {
    #[error("callable package identity cannot be empty")]
    Empty,
    #[error("callable package identity must contain only letters, digits, `_`, `-`, or `.`")]
    Invalid,
}

/// A function cannot be assigned an original declaration identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableDeclarationIdError {
    #[error("callable `{name}` has no canonical declaration module")]
    MissingModule { name: String },
    #[error(transparent)]
    InvalidName(#[from] ModulePathError),
}

/// Module-link failure while building callable aliases.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableLinkError {
    #[error("module `{module}` declares callable `{name}` more than once")]
    DuplicateDeclaration {
        module: CanonicalModulePath,
        name: String,
    },
    #[error("module `{module}` cannot resolve import `{import}`")]
    UnknownImport {
        module: CanonicalModulePath,
        import: String,
    },
    #[error("module `{module}` cannot access import `{import}`")]
    InaccessibleImport {
        module: CanonicalModulePath,
        import: String,
    },
    #[error("module `{module}` cannot widen the visibility of import `{import}`")]
    VisibilityEscalation {
        module: CanonicalModulePath,
        import: String,
    },
    #[error("module `{module}` imports ambiguous symbol `{import}`")]
    AmbiguousImport {
        module: CanonicalModulePath,
        import: String,
        candidates: Vec<CallableDeclarationId>,
    },
    #[error("module `{module}` has invalid import `{import}`: {reason}")]
    InvalidImportPath {
        module: CanonicalModulePath,
        import: String,
        reason: ModulePathError,
    },
    #[error(transparent)]
    InvalidDeclaration(#[from] CallableDeclarationIdError),
}

/// Failure to resolve a callable reference in a linked module.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableResolutionError {
    #[error("callable `{reference}` is not visible from module `{module}`")]
    Unknown {
        module: CanonicalModulePath,
        reference: String,
    },
    #[error("callable `{reference}` is ambiguous from module `{module}`")]
    Ambiguous {
        module: CanonicalModulePath,
        reference: String,
        candidates: Vec<CallableDeclarationId>,
    },
    #[error("reference `{reference}` names a module rather than a callable")]
    NotCallable { reference: String },
    #[error("invalid callable reference `{reference}`: {reason}")]
    InvalidPath {
        reference: String,
        reason: ModulePathError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ImportResolutionError {
    Unknown,
    Inaccessible,
    VisibilityEscalation,
    Ambiguous(Vec<CallableDeclarationId>),
    InvalidPath(ModulePathError),
}

impl CallablePackageId {
    /// Creates a validated canonical package identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CallablePackageIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CallablePackageIdError::Empty);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
        {
            return Err(CallablePackageIdError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CallableDeclarationId {
    /// Creates an identity from the package and canonical declaration location.
    pub fn try_new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        name: impl Into<String>,
    ) -> Result<Self, CallableDeclarationIdError> {
        Ok(Self {
            package,
            module,
            name: ModuleSegment::new(name.into())?,
        })
    }

    /// Creates the identity of a HIR function whose module has been linked.
    pub fn for_function(
        package: &CallablePackageId,
        function: &HirFunction,
    ) -> Result<Self, CallableDeclarationIdError> {
        let module = function.module_path().cloned().ok_or_else(|| {
            CallableDeclarationIdError::MissingModule {
                name: function.name().to_owned(),
            }
        })?;
        Self::try_new(package.clone(), module, function.name())
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Original module-qualified function name without an import alias.
    pub fn qualified_name(&self) -> String {
        qualified_name(&self.module, self.name())
    }
}

impl CallableSymbol {
    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn is_fx(&self) -> bool {
        self.fx
    }
}

impl CallableSymbolTable {
    /// Links direct declarations and every typed import tree in one package.
    pub fn build(project: &HirProject) -> Result<Self, Vec<CallableLinkError>> {
        let modules = project
            .modules()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>();
        let mut table = Self {
            package: project.package().clone(),
            scopes: modules
                .iter()
                .cloned()
                .map(|module| (module, BTreeMap::new()))
                .collect(),
            modules,
            symbols: BTreeMap::new(),
        };
        let mut errors = Vec::new();
        for (module_path, module) in project.modules() {
            for function in module.functions() {
                let declaration =
                    match CallableDeclarationId::for_function(&table.package, function) {
                        Ok(declaration) => declaration,
                        Err(error) => {
                            errors.push(CallableLinkError::InvalidDeclaration(error));
                            continue;
                        }
                    };
                let binding = ScopeBinding {
                    target: SymbolTarget::Callable(declaration.clone()),
                    visibility: function.visibility(),
                    owner: module_path.clone(),
                };
                let scope = table.scopes.entry(module_path.clone()).or_default();
                let bindings = scope.entry(function.name().to_owned()).or_default();
                if bindings.iter().any(|existing| {
                    matches!(existing.target, SymbolTarget::Callable(_))
                        && existing.owner == *module_path
                }) {
                    errors.push(CallableLinkError::DuplicateDeclaration {
                        module: module_path.clone(),
                        name: function.name().to_owned(),
                    });
                    continue;
                }
                bindings.push(binding);
                table.symbols.insert(
                    declaration.clone(),
                    CallableSymbol {
                        declaration,
                        visibility: function.visibility(),
                        fx: function.has_attribute("fx"),
                    },
                );
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let imports = project
            .modules()
            .flat_map(|(module, hir)| {
                hir.uses()
                    .iter()
                    .cloned()
                    .map(|item| (module.clone(), item))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        loop {
            let mut changed = false;
            for (module, import) in &imports {
                if let Ok(bindings) = table.import_bindings(module, import) {
                    for (name, binding) in bindings {
                        changed |= table.insert_binding(module, name, binding);
                    }
                }
            }
            if !changed {
                break;
            }
        }

        for (module, import) in &imports {
            if let Err(error) = table.import_bindings(module, import)
                && !matches!(error, ImportResolutionError::Unknown)
            {
                errors.push(error.into_link_error(module, import));
            }
        }
        if errors.is_empty() {
            Ok(table)
        } else {
            Err(errors)
        }
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub fn symbols(&self) -> impl ExactSizeIterator<Item = &CallableSymbol> {
        self.symbols.values()
    }

    pub fn symbol(&self, declaration: &CallableDeclarationId) -> Option<&CallableSymbol> {
        self.symbols.get(declaration)
    }

    /// Resolves an ordinary or Fx callable through the same alias table.
    pub fn resolve(
        &self,
        module: &CanonicalModulePath,
        reference: &str,
    ) -> Result<&CallableSymbol, CallableResolutionError> {
        let path = reference.parse::<ModulePath>().map_err(|reason| {
            CallableResolutionError::InvalidPath {
                reference: reference.to_owned(),
                reason,
            }
        })?;
        let targets =
            if matches!(path.root(), ModulePathRoot::ImplicitCrate) && path.segments().len() == 1 {
                self.visible_bindings(module, module, path.segments()[0].as_str())
            } else {
                self.targets_for_path(module, &path)
                    .map_err(|error| error.into_resolution_error(module, reference))?
            };
        let mut declarations = targets
            .into_iter()
            .filter_map(|binding| match binding.target {
                SymbolTarget::Callable(declaration) => Some(declaration),
                SymbolTarget::Module(_) => None,
            })
            .collect::<Vec<_>>();
        declarations.sort();
        declarations.dedup();
        match declarations.as_slice() {
            [] => {
                let module_only = self.targets_for_path(module, &path).is_ok_and(|targets| {
                    targets
                        .iter()
                        .any(|binding| matches!(binding.target, SymbolTarget::Module(_)))
                });
                if module_only {
                    Err(CallableResolutionError::NotCallable {
                        reference: reference.to_owned(),
                    })
                } else {
                    Err(CallableResolutionError::Unknown {
                        module: module.clone(),
                        reference: reference.to_owned(),
                    })
                }
            }
            [declaration] => {
                self.symbols
                    .get(declaration)
                    .ok_or_else(|| CallableResolutionError::Unknown {
                        module: module.clone(),
                        reference: reference.to_owned(),
                    })
            }
            _ => Err(CallableResolutionError::Ambiguous {
                module: module.clone(),
                reference: reference.to_owned(),
                candidates: declarations,
            }),
        }
    }

    fn import_bindings(
        &self,
        importer: &CanonicalModulePath,
        import: &UseItem,
    ) -> Result<Vec<(String, ScopeBinding)>, ImportResolutionError> {
        match import.tree().kind() {
            UseTreeKind::Path { path, alias } => {
                let targets = match self.targets_for_path(importer, path) {
                    Ok(targets) => targets,
                    Err(ImportResolutionError::Unknown) => return Ok(Vec::new()),
                    Err(error) => return Err(error),
                };
                let name = alias.as_ref().map_or_else(
                    || path.last_segment().unwrap_or_default(),
                    ModuleSegment::as_str,
                );
                Self::bind_named_targets(importer, import, name, targets)
            }
            UseTreeKind::Glob { module } => {
                let module = self.module_for_path(importer, module)?;
                let scope = self
                    .scopes
                    .get(&module)
                    .ok_or(ImportResolutionError::Unknown)?;
                let mut bindings = Vec::new();
                for (name, candidates) in scope {
                    for candidate in candidates
                        .iter()
                        .filter(|binding| Self::binding_visible_from(binding, importer))
                    {
                        if Self::can_reexport(candidate.visibility, import.visibility()) {
                            bindings.push((
                                name.clone(),
                                candidate.rebound(importer, import.visibility()),
                            ));
                        }
                    }
                }
                Ok(bindings)
            }
            UseTreeKind::Group { module, names } => {
                let module = self.module_for_path(importer, module)?;
                let mut bindings = Vec::new();
                for selected in names {
                    let targets =
                        match self.targets_in_module(importer, &module, selected.name().as_str()) {
                            Ok(targets) => targets,
                            Err(ImportResolutionError::Unknown) => continue,
                            Err(error) => return Err(error),
                        };
                    bindings.extend(Self::bind_named_targets(
                        importer,
                        import,
                        selected.binding_name().as_str(),
                        targets,
                    )?);
                }
                Ok(bindings)
            }
        }
    }

    fn bind_named_targets(
        importer: &CanonicalModulePath,
        import: &UseItem,
        name: &str,
        targets: Vec<ScopeBinding>,
    ) -> Result<Vec<(String, ScopeBinding)>, ImportResolutionError> {
        let callable_ids = targets
            .iter()
            .filter_map(|binding| match &binding.target {
                SymbolTarget::Callable(declaration) => Some(declaration.clone()),
                SymbolTarget::Module(_) => None,
            })
            .collect::<BTreeSet<_>>();
        if callable_ids.len() > 1 {
            return Err(ImportResolutionError::Ambiguous(
                callable_ids.into_iter().collect(),
            ));
        }
        if targets
            .iter()
            .any(|target| !Self::can_reexport(target.visibility, import.visibility()))
        {
            return Err(ImportResolutionError::VisibilityEscalation);
        }
        Ok(targets
            .into_iter()
            .map(|target| {
                (
                    name.to_owned(),
                    target.rebound(importer, import.visibility()),
                )
            })
            .collect())
    }

    fn targets_for_path(
        &self,
        requester: &CanonicalModulePath,
        path: &ModulePath,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        let canonical = self.resolve_path(requester, path)?;
        if self.modules.contains(&canonical) {
            return Ok(vec![ScopeBinding {
                target: SymbolTarget::Module(canonical),
                visibility: Some(Visibility::Public),
                owner: requester.clone(),
            }]);
        }
        let Some(name) = canonical.last_segment() else {
            return Err(ImportResolutionError::Unknown);
        };
        let owner = canonical
            .parent()
            .unwrap_or_else(CanonicalModulePath::crate_root);
        self.targets_in_module(requester, &owner, name)
    }

    fn targets_in_module(
        &self,
        requester: &CanonicalModulePath,
        module: &CanonicalModulePath,
        name: &str,
    ) -> Result<Vec<ScopeBinding>, ImportResolutionError> {
        if !self.modules.contains(module) {
            return Err(ImportResolutionError::Unknown);
        }
        let child = module
            .join(ModuleSegment::new(name.to_owned()).map_err(ImportResolutionError::InvalidPath)?);
        let mut targets = self.visible_bindings(requester, module, name);
        if self.modules.contains(&child) {
            targets.push(ScopeBinding {
                target: SymbolTarget::Module(child),
                visibility: Some(Visibility::Public),
                owner: module.clone(),
            });
        }
        if !targets.is_empty() {
            return Ok(targets);
        }
        let exists = self
            .scopes
            .get(module)
            .and_then(|scope| scope.get(name))
            .is_some_and(|bindings| !bindings.is_empty());
        if exists {
            Err(ImportResolutionError::Inaccessible)
        } else {
            Err(ImportResolutionError::Unknown)
        }
    }

    fn visible_bindings(
        &self,
        requester: &CanonicalModulePath,
        module: &CanonicalModulePath,
        name: &str,
    ) -> Vec<ScopeBinding> {
        self.scopes
            .get(module)
            .and_then(|scope| scope.get(name))
            .into_iter()
            .flatten()
            .filter(|binding| Self::binding_visible_from(binding, requester))
            .cloned()
            .collect()
    }

    fn module_for_path(
        &self,
        requester: &CanonicalModulePath,
        path: &ModulePath,
    ) -> Result<CanonicalModulePath, ImportResolutionError> {
        let module = self.resolve_path(requester, path)?;
        self.modules
            .contains(&module)
            .then_some(module)
            .ok_or(ImportResolutionError::Unknown)
    }

    fn resolve_path(
        &self,
        requester: &CanonicalModulePath,
        path: &ModulePath,
    ) -> Result<CanonicalModulePath, ImportResolutionError> {
        if matches!(path.root(), ModulePathRoot::ImplicitCrate) && path.segments().len() > 1 {
            let first = path.segments()[0].as_str();
            let namespaces = self
                .visible_bindings(requester, requester, first)
                .into_iter()
                .filter_map(|binding| match binding.target {
                    SymbolTarget::Module(module) => Some(module),
                    SymbolTarget::Callable(_) => None,
                })
                .collect::<BTreeSet<_>>();
            if namespaces.len() > 1 {
                return Err(ImportResolutionError::Ambiguous(Vec::new()));
            }
            if let Some(mut module) = namespaces.into_iter().next() {
                for segment in &path.segments()[1..] {
                    module = module.join(segment.clone());
                }
                return Ok(module);
            }
        }
        path.resolve_from(requester)
            .map_err(ImportResolutionError::InvalidPath)
    }

    fn binding_visible_from(binding: &ScopeBinding, requester: &CanonicalModulePath) -> bool {
        if requester == &binding.owner {
            return true;
        }
        match binding.visibility {
            Some(Visibility::Public | Visibility::Crate) => true,
            Some(Visibility::Super) => {
                let parent = binding
                    .owner
                    .parent()
                    .unwrap_or_else(CanonicalModulePath::crate_root);
                requester.segments().starts_with(parent.segments())
            }
            None => false,
        }
    }

    fn can_reexport(target: Option<Visibility>, requested: Option<Visibility>) -> bool {
        match requested {
            None => true,
            Some(Visibility::Public) => matches!(target, Some(Visibility::Public)),
            Some(Visibility::Crate) => {
                matches!(target, Some(Visibility::Public | Visibility::Crate))
            }
            Some(Visibility::Super) => {
                matches!(
                    target,
                    Some(Visibility::Public | Visibility::Crate | Visibility::Super)
                )
            }
        }
    }

    fn insert_binding(
        &mut self,
        module: &CanonicalModulePath,
        name: String,
        binding: ScopeBinding,
    ) -> bool {
        let bindings = self
            .scopes
            .get_mut(module)
            .expect("every import owner has a symbol scope")
            .entry(name)
            .or_default();
        if bindings.contains(&binding) {
            false
        } else {
            bindings.push(binding);
            true
        }
    }
}

impl ScopeBinding {
    fn rebound(&self, owner: &CanonicalModulePath, visibility: Option<Visibility>) -> Self {
        Self {
            target: self.target.clone(),
            visibility,
            owner: owner.clone(),
        }
    }
}

impl ImportResolutionError {
    fn into_link_error(self, module: &CanonicalModulePath, import: &UseItem) -> CallableLinkError {
        let module = module.clone();
        let spelling = import.tree().source().to_owned();
        match self {
            Self::Unknown => CallableLinkError::UnknownImport {
                module,
                import: spelling,
            },
            Self::Inaccessible => CallableLinkError::InaccessibleImport {
                module,
                import: spelling,
            },
            Self::VisibilityEscalation => CallableLinkError::VisibilityEscalation {
                module,
                import: spelling,
            },
            Self::Ambiguous(candidates) => CallableLinkError::AmbiguousImport {
                module,
                import: spelling,
                candidates,
            },
            Self::InvalidPath(reason) => CallableLinkError::InvalidImportPath {
                module,
                import: spelling,
                reason,
            },
        }
    }

    fn into_resolution_error(
        self,
        module: &CanonicalModulePath,
        reference: &str,
    ) -> CallableResolutionError {
        match self {
            Self::Ambiguous(candidates) => CallableResolutionError::Ambiguous {
                module: module.clone(),
                reference: reference.to_owned(),
                candidates,
            },
            Self::InvalidPath(reason) => CallableResolutionError::InvalidPath {
                reference: reference.to_owned(),
                reason,
            },
            Self::Unknown | Self::Inaccessible | Self::VisibilityEscalation => {
                CallableResolutionError::Unknown {
                    module: module.clone(),
                    reference: reference.to_owned(),
                }
            }
        }
    }
}

impl fmt::Display for CallablePackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for CallableDeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}", self.package, self.qualified_name())
    }
}

pub(crate) fn qualified_name(module: &CanonicalModulePath, name: &str) -> String {
    let module_len = module
        .segments()
        .iter()
        .map(|segment| segment.as_str().len() + 1)
        .sum::<usize>();
    let mut qualified = String::with_capacity(module_len + name.len());
    for segment in module.segments() {
        qualified.push_str(segment.as_str());
        qualified.push('.');
    }
    qualified.push_str(name);
    qualified
}

#[cfg(test)]
mod tests {
    use super::{
        CallableDeclarationId, CallableLinkError, CallablePackageId, CallableResolutionError,
    };
    use crate::{
        lower::lower_to_hir,
        project::{HirProject, HirProjectModule},
    };
    use arcweft_lang_syntax::{
        ast::module_path::{CanonicalModulePath, ModuleSegment},
        parser::parse_source,
    };

    #[test]
    fn declaration_identity_uses_original_package_and_module() {
        let package = CallablePackageId::try_new("opening-game").unwrap();
        let module = CanonicalModulePath::crate_root()
            .join(ModuleSegment::new("view").unwrap())
            .join(ModuleSegment::new("effects").unwrap());
        let id = CallableDeclarationId::try_new(package, module, "notice").unwrap();

        assert_eq!(id.qualified_name(), "view.effects.notice");
        assert_eq!(id.to_string(), "opening-game::view.effects.notice");
    }

    #[test]
    fn direct_qualified_and_group_alias_resolve_to_one_declaration() {
        let project = project([
            (
                CanonicalModulePath::crate_root(),
                "use effects.{flash as pulse}\n",
            ),
            (
                module_path(["effects"]),
                "#[fx]\npub fn flash() -> Fx { Fx.text(weight = .strong) }\n",
            ),
        ]);
        let symbols = project.callable_symbols().unwrap();
        let direct = symbols
            .resolve(&CanonicalModulePath::crate_root(), "effects.flash")
            .unwrap();
        let imported = symbols
            .resolve(&CanonicalModulePath::crate_root(), "pulse")
            .unwrap();

        assert_eq!(direct.declaration(), imported.declaration());
        assert_eq!(direct.declaration().qualified_name(), "effects.flash");
        assert!(direct.is_fx());
    }

    #[test]
    fn public_reexport_alias_retains_original_declaration() {
        let project = project([
            (CanonicalModulePath::crate_root(), "use prelude.{shine}\n"),
            (module_path(["effects"]), "pub fn flash() -> i32 { 1 }\n"),
            (module_path(["prelude"]), "pub use effects.flash as shine\n"),
        ]);
        let symbols = project.callable_symbols().unwrap();
        let original = symbols
            .resolve(&CanonicalModulePath::crate_root(), "effects.flash")
            .unwrap();
        let reexport = symbols
            .resolve(&CanonicalModulePath::crate_root(), "shine")
            .unwrap();

        assert_eq!(original.declaration(), reexport.declaration());
        assert_eq!(reexport.declaration().qualified_name(), "effects.flash");
    }

    #[test]
    fn colliding_glob_imports_are_ambiguous_at_the_use_site() {
        let project = project([
            (
                CanonicalModulePath::crate_root(),
                "use first.*\nuse second.*\n",
            ),
            (module_path(["first"]), "pub fn flash() -> i32 { 1 }\n"),
            (module_path(["second"]), "pub fn flash() -> i32 { 2 }\n"),
        ]);
        let symbols = project.callable_symbols().unwrap();
        let error = symbols
            .resolve(&CanonicalModulePath::crate_root(), "flash")
            .unwrap_err();

        let CallableResolutionError::Ambiguous { candidates, .. } = error else {
            panic!("expected ambiguous callable resolution");
        };
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].qualified_name(), "first.flash");
        assert_eq!(candidates[1].qualified_name(), "second.flash");
    }

    #[test]
    fn private_callable_cannot_be_imported_from_another_module() {
        let project = project([
            (CanonicalModulePath::crate_root(), "use effects.{secret}\n"),
            (module_path(["effects"]), "fn secret() -> i32 { 1 }\n"),
        ]);
        let errors = project.callable_symbols().unwrap_err();

        assert!(matches!(
            errors.as_slice(),
            [CallableLinkError::InaccessibleImport { import, .. }]
                if import == "effects.{secret}"
        ));
    }

    #[test]
    fn public_reexport_cannot_widen_crate_visibility() {
        let project = project([
            (CanonicalModulePath::crate_root(), ""),
            (
                module_path(["effects"]),
                "pub(crate) fn internal() -> i32 { 1 }\n",
            ),
            (
                module_path(["prelude"]),
                "pub use effects.internal as exposed\n",
            ),
        ]);
        let errors = project.callable_symbols().unwrap_err();

        assert!(matches!(
            errors.as_slice(),
            [CallableLinkError::VisibilityEscalation { import, .. }]
                if import == "effects.internal as exposed"
        ));
    }

    fn project<const N: usize>(modules: [(CanonicalModulePath, &'static str); N]) -> HirProject {
        HirProject::new(
            "game",
            modules.into_iter().map(|(path, source)| {
                let parsed = parse_source(source);
                assert!(
                    parsed.errors().is_empty(),
                    "fixture must parse: {:?}",
                    parsed.errors()
                );
                let hir = lower_to_hir(parsed.typed_tree()).expect("fixture must lower");
                HirProjectModule::new(path, hir)
            }),
        )
        .unwrap()
    }

    fn module_path<const N: usize>(segments: [&str; N]) -> CanonicalModulePath {
        CanonicalModulePath::from_segments(
            segments
                .into_iter()
                .map(|segment| ModuleSegment::new(segment).unwrap()),
        )
    }
}
