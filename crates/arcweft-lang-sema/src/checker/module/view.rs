//! Semantic validation owned by authored View declarations.

use crate::checker::helpers::{
    entity_syntax_kind, ident_pattern_name, type_kind_label, type_ref_kind,
};
use crate::checker::{ActionParam, ActionSignature, EntityKind, TypeCheckError, TypeChecker};
use crate::dialogue_view::DialogueViewProjection;
use crate::types::TypeKind;
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::items::{EntityDeclItem, EntityDeclKind};
use arcweft_lang_syntax::ast::view::{ViewActionInvokeAction, ViewActionPayload, ViewBody};
use arcweft_lang_syntax::types::{FnParam, TypeRef, parse_fn_signature};
use std::collections::HashMap;

impl TypeChecker<'_> {
    pub(super) fn check_view_declaration(&mut self, item: &EntityDeclItem) {
        self.check_view_action_invokes(item);
        self.check_view_fx_applications(item);
        self.check_view_dialogue_text_sources(item);
    }

    fn check_view_dialogue_text_sources(&mut self, item: &EntityDeclItem) {
        if item.kind() != EntityDeclKind::View {
            return;
        }
        let Some(body) = item.view_body() else {
            self.errors.push(TypeCheckError::new(format!(
                "View `{}` has no structured declaration body",
                item.id().body()
            )));
            return;
        };
        let Some(signature) = body.signature() else {
            self.errors.push(TypeCheckError::new(format!(
                "View `{}` has an invalid parameter signature",
                item.id().body()
            )));
            return;
        };
        let Some(view) = body.view() else {
            self.errors.push(TypeCheckError::new(format!(
                "View `{}` has no structured View body",
                item.id().body()
            )));
            return;
        };
        let parameters = signature
            .param_groups()
            .iter()
            .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
            .filter_map(|parameter| {
                let name = parameter.pattern().simple_binding_name()?;
                let TypeRef::Path(type_name) = parameter.ty()?.value() else {
                    return None;
                };
                crate::types::direct_type_name(type_name).map(|type_name| (name, type_name))
            })
            .collect::<HashMap<_, _>>();

        self.check_view_dialogue_text_nodes(view, &parameters);
        self.check_view_dialogue_action_projections(view, &parameters);
    }

    fn check_view_dialogue_text_nodes(
        &mut self,
        view: &ViewBody,
        parameters: &HashMap<&str, &str>,
    ) {
        for text in view.text_nodes() {
            let Some(label) = text.source().dotted_selector_label() else {
                continue;
            };
            let Some((parameter, field)) = label.split_once('.') else {
                continue;
            };
            let Some(type_name) = parameters.get(parameter) else {
                continue;
            };
            let Some(model) = self.dialogue_view_models.model(type_name) else {
                continue;
            };
            let Some(projection) = model.projection(field) else {
                self.errors.push(TypeCheckError::new(format!(
                    "dialogue View parameter `{parameter}` has no runtime projection `{field}`"
                )));
                continue;
            };
            let rich = text.rich_surface().is_some();
            match projection {
                DialogueViewProjection::Content if !rich => {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue content projection `{label}` must be emitted by `RichText(...)`"
                    )));
                }
                DialogueViewProjection::Speaker if rich => {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue speaker projection `{label}` must be emitted by `Text(...)`"
                    )));
                }
                DialogueViewProjection::Speaker | DialogueViewProjection::Content => {}
                DialogueViewProjection::Occurrence
                | DialogueViewProjection::Stage
                | DialogueViewProjection::Reveal
                | DialogueViewProjection::PrimaryAction => {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue lifecycle projection `{label}` is not text content"
                    )));
                }
            }
        }
    }

    fn check_view_dialogue_action_projections(
        &mut self,
        view: &ViewBody,
        parameters: &HashMap<&str, &str>,
    ) {
        for action in view.action_projections() {
            let Some(label) = action.dotted_selector_label() else {
                continue;
            };
            let Some((parameter, field)) = label.split_once('.') else {
                continue;
            };
            let Some(type_name) = parameters.get(parameter) else {
                self.errors.push(TypeCheckError::new(format!(
                    "View action projection `{label}` has no matching parameter"
                )));
                continue;
            };
            let Some(model) = self.dialogue_view_models.model(type_name) else {
                self.errors.push(TypeCheckError::new(format!(
                    "View action projection `{label}` does not come from a dialogue View model"
                )));
                continue;
            };
            if model.projection(field) != Some(DialogueViewProjection::PrimaryAction) {
                self.errors.push(TypeCheckError::new(format!(
                    "View action projection `{label}` must select `primary_action`"
                )));
            }
        }
    }

    fn check_view_action_invokes(&mut self, item: &EntityDeclItem) {
        let Some(view) = item.view_body().and_then(|body| body.view()) else {
            return;
        };
        for action in view.action_invokes() {
            self.check_view_action_invoke(&action);
        }
    }

    fn check_view_fx_applications(&mut self, item: &EntityDeclItem) {
        let Some(view) = item.view_body().and_then(|body| body.view()) else {
            return;
        };
        for application in view.fx_applications() {
            self.fx
                .validate_view_application(application, &mut self.errors);
        }
    }

    fn check_view_action_invoke(&mut self, action: &ViewActionInvokeAction) {
        if entity_syntax_kind(action.action()) != Some(EntityKind::Action) {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke target `{}` must be an Action reference",
                action.action().canonical_body()
            )));
            return;
        }

        let action_id = action.action().canonical_body();
        let Some(signature) = self.action_signatures.get(&action_id).cloned() else {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke target `{action_id}` is not declared"
            )));
            return;
        };

        self.check_action_invoke_payload(&action_id, &signature, action);
    }

    fn check_action_invoke_payload(
        &mut self,
        action_id: &str,
        signature: &ActionSignature,
        action: &ViewActionInvokeAction,
    ) {
        let Some(payload) = action.payload() else {
            for param in signature
                .params()
                .iter()
                .filter(|param| !param.has_default())
            {
                self.errors.push(TypeCheckError::new(format!(
                    "action.invoke for `{action_id}` is missing payload `{}`",
                    action_param_label(param)
                )));
            }
            return;
        };

        let Some(payload_name) = action.payload_name() else {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke for `{action_id}` must name its payload"
            )));
            return;
        };

        let Some(param) = signature.param(payload_name) else {
            self.errors.push(TypeCheckError::new(format!(
                "action `{action_id}` does not declare payload `{payload_name}`"
            )));
            return;
        };

        let actual = action_payload_type(payload);
        if !self.types_compatible(param.ty(), &actual) {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke payload `{payload_name}` for `{action_id}` expects {}, but View payload has {}",
                type_kind_label(param.ty()),
                type_kind_label(&actual)
            )));
        }

        for missing in signature
            .params()
            .iter()
            .filter(|param| !param.has_default() && param.name() != payload_name)
        {
            self.errors.push(TypeCheckError::new(format!(
                "action.invoke for `{action_id}` is missing payload `{}`",
                action_param_label(missing)
            )));
        }
    }
}

pub(super) fn collect_action_signatures(
    module: &HirModule,
    errors: &mut Vec<TypeCheckError>,
) -> HashMap<String, ActionSignature> {
    module
        .declarations()
        .iter()
        .filter_map(|declaration| match declaration {
            HirTopLevelDecl::EntityDecl(item) if item.kind() == EntityDeclKind::Action => {
                Some(item)
            }
            _ => None,
        })
        .filter_map(|item| match action_signature_from_decl(item) {
            Ok(signature) => Some((item.id().body().to_owned(), signature)),
            Err(message) => {
                errors.push(TypeCheckError::new(format!(
                    "invalid action signature for `{}`: {message}",
                    item.id().body()
                )));
                None
            }
        })
        .collect()
}

fn action_signature_from_decl(item: &EntityDeclItem) -> Result<ActionSignature, String> {
    let signature_tail = item.signature_tail().trim();
    if signature_tail.is_empty() {
        return Ok(ActionSignature::new([]));
    }

    let signature = parse_fn_signature(&format!("fn action{signature_tail}"))
        .map_err(|error| error.to_string())?;
    if signature.return_type().is_some() {
        return Err("action declarations do not return values".to_owned());
    }

    let params = signature
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(action_param_from_fn_param)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ActionSignature::new(params))
}

fn action_param_from_fn_param(param: &FnParam) -> Result<ActionParam, String> {
    if param.is_rest() {
        return Err("action payload parameters cannot be rest parameters".to_owned());
    }
    if param.receiver_kind().is_some() {
        return Err("action payload parameters cannot include a receiver".to_owned());
    }
    let Some(name) = ident_pattern_name(param.pattern()) else {
        return Err("action payload parameters must use identifier patterns".to_owned());
    };
    let ty = param
        .ty()
        .ok_or_else(|| "action payload parameters must declare a type".to_owned())?;
    Ok(ActionParam::new(
        name,
        type_ref_kind(ty.value()),
        param.default().is_some(),
    ))
}

fn action_payload_type(payload: &ViewActionPayload) -> TypeKind {
    match payload {
        ViewActionPayload::LiteralString(_) | ViewActionPayload::TextControlProjection { .. } => {
            TypeKind::String
        }
    }
}

fn action_param_label(param: &ActionParam) -> String {
    format!("{}: {}", param.name(), type_kind_label(param.ty()))
}
