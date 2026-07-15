//! Shared typed metadata for View-part language-server features.

use crate::{documents::DocumentSnapshot, profiles::LspProfile};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::analyze_types,
    view_part::{
        CheckedViewId, CheckedViewPartCatalog, CheckedViewPartId, CheckedViewPartOccurrenceShape,
        CheckedViewPartTargetKind,
    },
};
use arcweft_lang_syntax::{ast::common::TextRange, parser::parse_source};
use arcweft_view::{ViewLocalPartName, ViewPartName};
use lsp_types::{CompletionItem, CompletionItemKind, Documentation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ViewPartMetadataIndex {
    locals: Vec<LocalPartMetadata>,
    exports: Vec<ExportedPartMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalPartMetadata {
    owner: CheckedViewId,
    owner_range: TextRange,
    id: CheckedViewPartId,
    name: ViewLocalPartName,
    target_kind: CheckedViewPartTargetKind,
    occurrence: CheckedViewPartOccurrenceShape,
    definition: TextRange,
    references: Vec<TextRange>,
    exported_as: Option<ViewPartName>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExportedPartMetadata {
    owner: CheckedViewId,
    owner_range: TextRange,
    name: ViewPartName,
    local_name: ViewLocalPartName,
    definition: TextRange,
    references: Vec<TextRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ViewPartSymbol {
    Local {
        owner: CheckedViewId,
        id: CheckedViewPartId,
    },
    Exported {
        owner: CheckedViewId,
        name: ViewPartName,
    },
}

impl ViewPartMetadataIndex {
    pub(crate) fn for_document(profile: &LspProfile, document: &DocumentSnapshot) -> Option<Self> {
        Self::from_source(profile, document.text())
    }

    fn from_source(profile: &LspProfile, source: &str) -> Option<Self> {
        let parsed = parse_source(source);
        let hir = lower_to_hir(parsed.typed_tree()).ok()?;
        let report = analyze_types(&hir, &profile.typecheck_env());
        Some(Self::from_catalog(&report.view_part_catalog))
    }

    fn from_catalog(catalog: &CheckedViewPartCatalog) -> Self {
        let mut locals = Vec::new();
        let mut exports = Vec::new();
        for owner in catalog.owners() {
            for local in owner.local_parts() {
                let matching_export = owner
                    .exports()
                    .iter()
                    .find(|export| export.target().part() == local.id());
                let mut references = vec![local.name_range()];
                if let Some(export) = matching_export {
                    references.push(export.source().local_range());
                }
                locals.push(LocalPartMetadata {
                    owner: owner.id().clone(),
                    owner_range: owner.range(),
                    id: local.id(),
                    name: local.name().clone(),
                    target_kind: local.target_kind(),
                    occurrence: local.occurrence(),
                    definition: local.name_range(),
                    references,
                    exported_as: matching_export.map(|export| export.public_name().clone()),
                });
            }
            exports.extend(owner.exports().iter().map(|export| ExportedPartMetadata {
                owner: owner.id().clone(),
                owner_range: owner.range(),
                name: export.public_name().clone(),
                local_name: export.local_name().clone(),
                definition: export.source().public_range(),
                references: vec![export.source().public_range()],
            }));
        }
        Self { locals, exports }
    }

    pub(crate) fn hover(&self, offset: usize) -> Option<String> {
        match self.symbol_at(offset)? {
            ViewPartSymbol::Local { owner, id } => {
                let local = self.local(&owner, id)?;
                let cardinality = match (
                    local.occurrence.can_be_absent(),
                    local.occurrence.can_repeat(),
                ) {
                    (false, false) => "exactly one mounted occurrence",
                    (true, false) => "zero or one mounted occurrence",
                    (_, true) => "zero or many mounted occurrences",
                };
                let export = local.exported_as.as_ref().map_or_else(
                    || "private; not exported".to_owned(),
                    |name| format!("exported as `{}`", name.public_id()),
                );
                Some(format!(
                    "private View part `{}`\n\nOwner: `{}`  \nTarget: {}  \nOccurrence: {cardinality}  \n{export}",
                    local.name.public_id(),
                    owner.public_id(),
                    target_kind_label(local.target_kind),
                ))
            }
            ViewPartSymbol::Exported { owner, name } => {
                let export = self.exported(&owner, &name)?;
                Some(format!(
                    "exported View part `{}`\n\nOwner: `{}`  \nPrivate target: `{}`  \nBoundary: direct caller only; no re-export or private traversal",
                    export.name.public_id(),
                    owner.public_id(),
                    export.local_name.public_id(),
                ))
            }
        }
    }

    pub(crate) fn definitions(&self, offset: usize) -> Vec<TextRange> {
        match self.symbol_at(offset) {
            Some(ViewPartSymbol::Local { owner, id }) => self
                .local(&owner, id)
                .map(|local| vec![local.definition])
                .unwrap_or_default(),
            Some(ViewPartSymbol::Exported { owner, name }) => self
                .exported(&owner, &name)
                .map(|export| vec![export.definition])
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    pub(crate) fn references(&self, offset: usize) -> Vec<TextRange> {
        match self.symbol_at(offset) {
            Some(ViewPartSymbol::Local { owner, id }) => self
                .local(&owner, id)
                .map(|local| local.references.clone())
                .unwrap_or_default(),
            Some(ViewPartSymbol::Exported { owner, name }) => self
                .exported(&owner, &name)
                .map(|export| export.references.clone())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    pub(crate) fn completions(&self, source: &str, offset: usize) -> Vec<CompletionItem> {
        let line_start = source[..offset.min(source.len())]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = source
            .get(line_start..offset.min(source.len()))
            .unwrap_or_default()
            .trim_start();
        let tokens = prefix.split_whitespace().collect::<Vec<_>>();
        if tokens == ["export"] || (tokens == ["export", ""]) {
            return vec![keyword_completion("part")];
        }
        if tokens.first().copied() != Some("export") || tokens.get(1).copied() != Some("part") {
            return Vec::new();
        }
        let Some(owner_range) = self.owner_range_at(offset) else {
            return Vec::new();
        };
        let has_trailing_space = prefix.chars().last().is_some_and(char::is_whitespace);
        if tokens.len() == 2 || (tokens.len() == 3 && !has_trailing_space) {
            let partial = tokens.get(2).copied().unwrap_or_default();
            return self
                .locals
                .iter()
                .filter(|local| local.owner_range == owner_range)
                .filter(|local| local.exported_as.is_none())
                .filter(|local| local.target_kind != CheckedViewPartTargetKind::ViewCall)
                .filter(|local| local.name.public_id().as_str().starts_with(partial))
                .map(|local| CompletionItem {
                    label: local.name.public_id().as_str().to_owned(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(format!("private View part in {}", local.owner.public_id())),
                    documentation: Some(Documentation::String(
                        "Unexported owner-local target eligible for `export part`.".to_owned(),
                    )),
                    ..CompletionItem::default()
                })
                .collect();
        }
        if tokens.len() == 3 && has_trailing_space {
            return vec![keyword_completion("as")];
        }
        Vec::new()
    }

    fn owner_range_at(&self, offset: usize) -> Option<TextRange> {
        self.locals
            .iter()
            .map(|local| local.owner_range)
            .chain(self.exports.iter().map(|export| export.owner_range))
            .filter(|range| contains(*range, offset))
            .min_by_key(|range| range.end().saturating_sub(range.start()))
    }

    fn symbol_at(&self, offset: usize) -> Option<ViewPartSymbol> {
        let local = self.locals.iter().find(|local| {
            local
                .references
                .iter()
                .any(|range| contains(*range, offset))
        });
        if let Some(local) = local {
            return Some(ViewPartSymbol::Local {
                owner: local.owner.clone(),
                id: local.id,
            });
        }
        self.exports
            .iter()
            .find(|export| {
                export
                    .references
                    .iter()
                    .any(|range| contains(*range, offset))
            })
            .map(|export| ViewPartSymbol::Exported {
                owner: export.owner.clone(),
                name: export.name.clone(),
            })
    }

    fn local(&self, owner: &CheckedViewId, id: CheckedViewPartId) -> Option<&LocalPartMetadata> {
        self.locals
            .iter()
            .find(|local| &local.owner == owner && local.id == id)
    }

    fn exported(
        &self,
        owner: &CheckedViewId,
        name: &ViewPartName,
    ) -> Option<&ExportedPartMetadata> {
        self.exports
            .iter()
            .find(|export| &export.owner == owner && &export.name == name)
    }
}

fn keyword_completion(keyword: &str) -> CompletionItem {
    CompletionItem {
        label: keyword.to_owned(),
        kind: Some(CompletionItemKind::KEYWORD),
        insert_text: Some(keyword.to_owned()),
        ..CompletionItem::default()
    }
}

fn contains(range: TextRange, offset: usize) -> bool {
    range.start() <= offset && offset <= range.end()
}

const fn target_kind_label(kind: CheckedViewPartTargetKind) -> &'static str {
    match kind {
        CheckedViewPartTargetKind::Element => "element",
        CheckedViewPartTargetKind::Text => "text",
        CheckedViewPartTargetKind::Image => "image",
        CheckedViewPartTargetKind::ViewCall => "nested View call",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_runtime_host::RuntimeHostRunnerKind;

    const SOURCE: &str =
        "pub view Card() {\n    export part title as heading\n    Text(\"Title\").part(title)\n}\n";

    fn metadata() -> ViewPartMetadataIndex {
        ViewPartMetadataIndex::from_source(
            &LspProfile::default_for_runner(RuntimeHostRunnerKind::Native),
            SOURCE,
        )
        .expect("typed View-part metadata")
    }

    #[test]
    fn local_and_public_symbols_keep_disjoint_definitions_and_references() {
        let metadata = metadata();
        let target = SOURCE.rfind("title").expect("local definition");
        let local_use = SOURCE.find("title as").expect("export local use");
        let public = SOURCE.find("heading").expect("public definition");

        assert_eq!(metadata.definitions(local_use)[0].start(), target);
        assert_eq!(metadata.references(target).len(), 2);
        assert_eq!(metadata.definitions(public)[0].start(), public);
        assert_eq!(metadata.references(public).len(), 1);
        assert!(
            metadata
                .hover(public)
                .unwrap()
                .contains("direct caller only")
        );
    }

    #[test]
    fn export_completion_uses_unexported_owner_local_inventory() {
        let source = "pub view Card() {\n    export part \n    Text(\"A\").part(alpha)\n    Text(\"B\").part(beta)\n}\n";
        let metadata = ViewPartMetadataIndex::from_source(
            &LspProfile::default_for_runner(RuntimeHostRunnerKind::Native),
            source,
        )
        .expect("typed View-part metadata");
        let offset = source.find("export part ").unwrap() + "export part ".len();
        let labels = metadata
            .completions(source, offset)
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();
        assert_eq!(labels, ["alpha", "beta"]);
    }
}
