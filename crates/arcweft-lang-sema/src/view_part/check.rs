use std::collections::{BTreeMap, BTreeSet};

use arcweft_id::PublicId;
use arcweft_lang_hir::{
    model::HirModule,
    view_part::{HirViewPartOwner, HirViewPartTargetKind},
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_view::{ViewPartLocalName, ViewPartName};

use super::{
    CheckedViewId, CheckedViewLocalPart, CheckedViewPartCatalog, CheckedViewPartExport,
    CheckedViewPartExportSource, CheckedViewPartId, CheckedViewPartOccurrenceShape,
    CheckedViewPartOwner, CheckedViewPartRef, CheckedViewPartTargetKind, ViewPartDiagnostic,
    ViewPartDiagnosticCode,
};

/// Checks private/public View-part namespaces without source I/O.
pub fn check_view_parts(module: &HirModule) -> (CheckedViewPartCatalog, Vec<ViewPartDiagnostic>) {
    let mut diagnostics = Vec::new();
    let owners = module
        .view_parts()
        .iter()
        .filter_map(|owner| check_owner(owner, &mut diagnostics))
        .collect();
    (CheckedViewPartCatalog::new(owners), diagnostics)
}

fn check_owner(
    owner: &HirViewPartOwner,
    diagnostics: &mut Vec<ViewPartDiagnostic>,
) -> Option<CheckedViewPartOwner> {
    let owner_text = if owner.view().body().starts_with("view.") {
        owner.view().body().to_owned()
    } else {
        format!("view.{}", owner.view().body())
    };
    let owner_evidence = owner
        .local_parts()
        .first()
        .map(|part| part.operand_span().clone())
        .or_else(|| {
            owner
                .exports()
                .first()
                .map(|export| export.declaration_span().clone())
        })
        .expect("HIR View-part owners contain at least one source-bound record");
    let owner_id = PublicId::try_new(owner_text.clone()).map_or_else(
        |_| {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::InvalidOwner,
                format!("View `{owner_text}` is not a valid checked public identity"),
                owner_evidence,
                None,
            ));
            None
        },
        |id| Some(CheckedViewId::from_public_id(id)),
    )?;
    let source = owner.source().clone();
    let module = owner
        .module()
        .cloned()
        .unwrap_or_else(CanonicalModulePath::crate_root);
    let local_parts = check_local_parts(owner, &owner_id, diagnostics);
    let exports = check_exports(owner, &owner_id, &local_parts, diagnostics);
    Some(CheckedViewPartOwner::new(
        owner_id,
        module,
        owner.visibility(),
        owner.range(),
        source,
        local_parts,
        exports,
    ))
}

fn check_local_parts(
    owner: &HirViewPartOwner,
    owner_id: &CheckedViewId,
    diagnostics: &mut Vec<ViewPartDiagnostic>,
) -> Vec<CheckedViewLocalPart> {
    let mut parts = BTreeMap::new();
    for part in owner.local_parts() {
        let Ok(name) = ViewPartLocalName::try_new(part.name().to_owned()) else {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::InvalidLocalName,
                format!("`{}` is not a valid private View part name", part.name()),
                part.operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        };
        if parts.insert(name.clone(), part).is_some() {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::DuplicateLocalTarget,
                format!(
                    "private View part `{}` is declared more than once",
                    part.name()
                ),
                part.operand_span().clone(),
                Some(owner_id.clone()),
            ));
        }
    }
    parts
        .into_iter()
        .enumerate()
        .filter_map(|(index, (name, part))| {
            let Ok(index) = u32::try_from(index) else {
                diagnostics.push(ViewPartDiagnostic::new(
                    ViewPartDiagnosticCode::PartIdOverflow,
                    "View has more private parts than the checked u32 identity space",
                    part.operand_span().clone(),
                    Some(owner_id.clone()),
                ));
                return None;
            };
            Some(CheckedViewLocalPart::new(
                CheckedViewPartId::new(index),
                name,
                target_kind(part.target_kind()),
                CheckedViewPartOccurrenceShape::new(
                    part.occurrence().can_be_absent(),
                    part.occurrence().can_repeat(),
                ),
                part.modifier_span().clone(),
                part.operand_span().clone(),
            ))
        })
        .collect()
}

fn check_exports(
    owner: &HirViewPartOwner,
    owner_id: &CheckedViewId,
    local_parts: &[CheckedViewLocalPart],
    diagnostics: &mut Vec<ViewPartDiagnostic>,
) -> Vec<CheckedViewPartExport> {
    let locals = local_parts
        .iter()
        .map(|part| (part.name().clone(), part))
        .collect::<BTreeMap<_, _>>();
    let mut exported_targets = BTreeSet::new();
    let mut public_names = BTreeSet::new();
    let mut exports = Vec::new();
    for export in owner.exports() {
        let Ok(local_name) = ViewPartLocalName::try_new(export.local_name().to_owned()) else {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::InvalidLocalName,
                format!(
                    "`{}` is not a valid private View part name",
                    export.local_name()
                ),
                export.local_operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        };
        let Ok(public_name) = ViewPartName::try_new(export.public_name().to_owned()) else {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::InvalidPublicName,
                format!(
                    "`{}` is not a valid public View part name",
                    export.public_name()
                ),
                export.public_operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        };
        let Some(target) = locals.get(&local_name) else {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::MissingLocalTarget,
                format!(
                    "export references missing private View part `{}`",
                    export.local_name()
                ),
                export.local_operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        };
        if target.target_kind() == CheckedViewPartTargetKind::ViewCall {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::UnsupportedCallViewExport,
                format!(
                    "nested View call part `{}` cannot be re-exported",
                    export.local_name()
                ),
                export.local_operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        }
        if !exported_targets.insert(target.id()) {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::DuplicateExportTarget,
                format!(
                    "private View part `{}` is exported more than once",
                    export.local_name()
                ),
                export.local_operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        }
        if !public_names.insert(public_name.clone()) {
            diagnostics.push(ViewPartDiagnostic::new(
                ViewPartDiagnosticCode::DuplicatePublicName,
                format!(
                    "public View part `{}` is exported more than once",
                    export.public_name()
                ),
                export.public_operand_span().clone(),
                Some(owner_id.clone()),
            ));
            continue;
        }
        exports.push(CheckedViewPartExport::new(
            owner_id.clone(),
            CheckedViewPartRef::new(owner_id.clone(), target.id()),
            local_name,
            public_name,
            CheckedViewPartExportSource::new(
                export.declaration_span().clone(),
                export.local_operand_span().clone(),
                export.public_operand_span().clone(),
            ),
        ));
    }
    exports.sort_by(|left, right| left.public_name().cmp(right.public_name()));
    exports
}

const fn target_kind(kind: HirViewPartTargetKind) -> CheckedViewPartTargetKind {
    match kind {
        HirViewPartTargetKind::Element => CheckedViewPartTargetKind::Element,
        HirViewPartTargetKind::Text => CheckedViewPartTargetKind::Text,
        HirViewPartTargetKind::Image => CheckedViewPartTargetKind::Image,
        HirViewPartTargetKind::ViewCall => CheckedViewPartTargetKind::ViewCall,
    }
}
