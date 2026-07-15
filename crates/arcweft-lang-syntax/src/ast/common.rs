use super::{
    module_path::{ModulePath, ModulePathError},
    symbol_path::{
        ProjectSymbolPathError, ProjectSymbolSegment, SpannedProjectSymbolPath, UseAlias,
    },
};
use core::{ops::Range, str::FromStr};
use thiserror::Error;

/// Half-open byte range in the original source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    kind: UseTreeKind,
}

/// Typed selection performed by one `use` tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UseTreeKind {
    /// A path whose final segment is resolved as either a module or an item.
    Path {
        path: SpannedProjectSymbolPath,
        alias: Option<UseAlias>,
    },
    /// Every visible item exported by one module.
    Glob { module: SpannedProjectSymbolPath },
    /// An explicit set of names exported by one module.
    Group {
        module: SpannedProjectSymbolPath,
        names: Vec<UseName>,
    },
}

/// One name selected from a grouped `use` tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseName {
    name: ProjectSymbolSegment,
    name_range: TextRange,
    alias: Option<UseAlias>,
}

/// Invalid typed `use` tree.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum UseTreeError {
    #[error(transparent)]
    ModulePath(#[from] ModulePathError),
    #[error(transparent)]
    ProjectSymbolPath(#[from] ProjectSymbolPathError),
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
        let source = source.into();
        Self::parse_at(&source, 0)
    }

    /// Parses an import tree whose first byte has the supplied source offset.
    pub(crate) fn parse_at(source: &str, base: usize) -> Result<Self, UseTreeError> {
        let leading = source.len() - source.trim_start().len();
        let normalized = source.trim().to_owned();
        let source_base = base + leading;
        let kind = if let Some(group_start) = normalized.find(".{") {
            let Some(group_source) = normalized
                .get(group_start + 2..)
                .and_then(|tail| tail.strip_suffix('}'))
            else {
                return Err(UseTreeError::UnterminatedGroup {
                    spelling: normalized,
                });
            };
            let module =
                SpannedProjectSymbolPath::parse_at(&normalized[..group_start], source_base)?;
            let mut names = Vec::new();
            let mut relative = 0;
            for binding in group_source.split(',') {
                let binding_base = source_base + group_start + 2 + relative;
                if binding.trim().is_empty() {
                    if relative + binding.len() != group_source.len() {
                        return Err(UseTreeError::EmptyGroupName {
                            spelling: normalized,
                        });
                    }
                } else {
                    names.push(parse_use_name(binding, binding_base)?);
                }
                relative += binding.len() + 1;
            }
            if names.is_empty() {
                return Err(UseTreeError::EmptyGroup {
                    spelling: normalized,
                });
            }
            UseTreeKind::Group { module, names }
        } else if let Some(module_source) = normalized.strip_suffix(".*") {
            UseTreeKind::Glob {
                module: SpannedProjectSymbolPath::parse_at(module_source, source_base)?,
            }
        } else {
            let binding = parse_use_binding(&normalized, source_base)?;
            UseTreeKind::Path {
                path: SpannedProjectSymbolPath::parse_at(binding.name, binding.name_base)?,
                alias: binding.alias,
            }
        };
        Ok(Self {
            source: normalize_parent_module_root(&normalized),
            kind,
        })
    }

    /// Normalized source spelling of the use tree.
    pub fn source(&self) -> &str {
        &self.source
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
    pub const fn name(&self) -> &ProjectSymbolSegment {
        &self.name
    }

    pub const fn name_range(&self) -> TextRange {
        self.name_range
    }

    pub const fn alias(&self) -> Option<&UseAlias> {
        self.alias.as_ref()
    }

    /// Name introduced into the importing module.
    pub fn binding_name(&self) -> &str {
        self.alias
            .as_ref()
            .map_or_else(|| self.name.as_str(), |alias| alias.name().as_str())
    }
}

fn parse_use_name(binding: &str, base: usize) -> Result<UseName, UseTreeError> {
    let binding = parse_use_binding(binding, base)?;
    Ok(UseName {
        name: ProjectSymbolSegment::try_new(binding.name.to_owned())?,
        name_range: TextRange::new(binding.name_base, binding.name_base + binding.name.len()),
        alias: binding.alias,
    })
}

struct ParsedUseBinding<'a> {
    name: &'a str,
    name_base: usize,
    alias: Option<UseAlias>,
}

fn parse_use_binding(binding: &str, base: usize) -> Result<ParsedUseBinding<'_>, UseTreeError> {
    let leading = binding.len() - binding.trim_start().len();
    let binding = binding.trim();
    let mut aliases = binding.match_indices(" as ");
    let first_alias = aliases.next();
    if aliases.next().is_some() {
        return Err(UseTreeError::MultipleAliases {
            binding: binding.to_owned(),
        });
    }
    let binding_base = base + leading;
    let (name_source, alias) = match first_alias {
        Some((index, separator)) => {
            let name_source = &binding[..index];
            let alias_source = &binding[index + separator.len()..];
            let alias_leading = alias_source.len() - alias_source.trim_start().len();
            let alias_name = alias_source.trim();
            let alias_start = binding_base + index + separator.len() + alias_leading;
            let alias = UseAlias::new(
                super::module_path::ModuleSegment::new(alias_name.to_owned())?,
                TextRange::new(alias_start, alias_start + alias_name.len()),
            );
            (name_source, Some(alias))
        }
        None => (binding, None),
    };
    let name_leading = name_source.len() - name_source.trim_start().len();
    Ok(ParsedUseBinding {
        name: name_source.trim(),
        name_base: binding_base + name_leading,
        alias,
    })
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
