use super::module_path::{ModulePath, ModulePathError, ModuleSegment};
use core::{ops::Range, str::FromStr};
use thiserror::Error;

/// Half-open byte range in the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRange {
    start: usize,
    end: usize,
}

/// `mod game.routes.opening`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDecl {
    path: String,
    range: TextRange,
}

/// `use` import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseItem {
    visibility: Option<Visibility>,
    tree: UseTree,
    range: TextRange,
}

/// Typed `use` tree syntax with the module prefix pre-parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseTree {
    source: String,
    module_path_prefix: ModulePath,
    exact_module_prefix: bool,
    kind: UseTreeKind,
}

/// Typed selection performed by one `use` tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UseTreeKind {
    /// A path whose final segment is resolved as either a module or an item.
    Path {
        path: ModulePath,
        alias: Option<ModuleSegment>,
    },
    /// Every visible item exported by one module.
    Glob { module: ModulePath },
    /// An explicit set of names exported by one module.
    Group {
        module: ModulePath,
        names: Vec<UseName>,
    },
}

/// One name selected from a grouped `use` tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseName {
    name: ModuleSegment,
    alias: Option<ModuleSegment>,
}

/// Invalid typed `use` tree.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum UseTreeError {
    #[error(transparent)]
    ModulePath(#[from] ModulePathError),
    #[error("grouped use tree `{spelling}` must end with `}}`")]
    UnterminatedGroup { spelling: String },
    #[error("grouped use tree `{spelling}` must select at least one name")]
    EmptyGroup { spelling: String },
    #[error("grouped use tree `{spelling}` contains an empty name")]
    EmptyGroupName { spelling: String },
    #[error("use binding `{binding}` contains more than one `as` alias")]
    MultipleAliases { binding: String },
}

/// Markdown documentation comment collected from consecutive `///` lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocBlock {
    text: String,
    range: TextRange,
}

/// Arcweft visibility qualifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Crate,
    Super,
}

impl TextRange {
    /// Builds a half-open byte range.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Start byte offset.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// End byte offset.
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Converts to the standard range type.
    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }
}

impl ModuleDecl {
    pub(crate) const fn new(path: String, range: TextRange) -> Self {
        Self { path, range }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the typed path consumed by project loading and HIR tooling.
    pub fn module_path(&self) -> Result<ModulePath, ModulePathError> {
        ModulePath::from_str(&self.path)
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl UseItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        tree: UseTree,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            tree,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn tree(&self) -> &UseTree {
        &self.tree
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl UseTree {
    /// Parses a normalized import tree and extracts its module prefix.
    pub fn parse(source: impl Into<String>) -> Result<Self, UseTreeError> {
        let source = normalize_parent_module_root(&source.into());
        let (module_path_prefix, exact_module_prefix, kind) =
            if let Some(group_start) = source.find(".{") {
                let Some(group_source) = source
                    .get(group_start + 2..)
                    .and_then(|tail| tail.strip_suffix('}'))
                else {
                    return Err(UseTreeError::UnterminatedGroup { spelling: source });
                };
                let module = ModulePath::from_str(source[..group_start].trim())?;
                let bindings = group_source.split(',').map(str::trim).collect::<Vec<_>>();
                let trailing_comma = bindings.last().is_some_and(|binding| binding.is_empty());
                let selected = if trailing_comma {
                    &bindings[..bindings.len().saturating_sub(1)]
                } else {
                    bindings.as_slice()
                };
                if selected.iter().any(|binding| binding.is_empty()) {
                    return Err(UseTreeError::EmptyGroupName { spelling: source });
                }
                let names = selected
                    .iter()
                    .copied()
                    .map(parse_use_name)
                    .collect::<Result<Vec<_>, _>>()?;
                if names.is_empty() {
                    return Err(UseTreeError::EmptyGroup { spelling: source });
                }
                (module.clone(), true, UseTreeKind::Group { module, names })
            } else if let Some(module_source) = source.strip_suffix(".*") {
                let module = ModulePath::from_str(module_source.trim())?;
                (module.clone(), true, UseTreeKind::Glob { module })
            } else {
                let (path_source, alias) = parse_use_binding(&source)?;
                let path = ModulePath::from_str(path_source)?;
                (path.clone(), false, UseTreeKind::Path { path, alias })
            };
        Ok(Self {
            source,
            module_path_prefix,
            exact_module_prefix,
            kind,
        })
    }

    /// Normalized source spelling of the use tree.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the extracted prefix is syntactically known to name a module.
    pub const fn module_path_is_exact(&self) -> bool {
        self.exact_module_prefix
    }

    /// Returns the longest syntactic module prefix of this use tree.
    ///
    /// The project loader resolves this prefix against existing module paths,
    /// walking one parent when the final segment can be an imported item.
    pub fn module_path_prefix(&self) -> &ModulePath {
        &self.module_path_prefix
    }

    /// Structured path, glob, or grouped selection represented by this tree.
    pub const fn kind(&self) -> &UseTreeKind {
        &self.kind
    }
}

fn normalize_parent_module_root(path: &str) -> String {
    path.strip_prefix("parent.")
        .map_or_else(|| path.to_owned(), |tail| format!("super.{tail}"))
}

impl UseName {
    pub fn name(&self) -> &ModuleSegment {
        &self.name
    }

    pub const fn alias(&self) -> Option<&ModuleSegment> {
        self.alias.as_ref()
    }

    /// Name introduced into the importing module.
    pub fn binding_name(&self) -> &ModuleSegment {
        self.alias.as_ref().unwrap_or(&self.name)
    }
}

fn parse_use_name(binding: &str) -> Result<UseName, UseTreeError> {
    let (name, alias) = parse_use_binding(binding)?;
    Ok(UseName {
        name: ModuleSegment::new(name.to_owned())?,
        alias,
    })
}

fn parse_use_binding(binding: &str) -> Result<(&str, Option<ModuleSegment>), UseTreeError> {
    let mut pieces = binding.split(" as ");
    let name = pieces.next().unwrap_or_default().trim();
    let alias = pieces
        .next()
        .map(str::trim)
        .map(|alias| ModuleSegment::new(alias.to_owned()))
        .transpose()?;
    if pieces.next().is_some() {
        return Err(UseTreeError::MultipleAliases {
            binding: binding.to_owned(),
        });
    }
    Ok((name, alias))
}

impl DocBlock {
    pub(crate) const fn new(text: String, range: TextRange) -> Self {
        Self { text, range }
    }

    /// Markdown text without the leading `///` markers.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Source range covered by the whole doc block.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
