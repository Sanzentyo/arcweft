use arcweft_adapter_context::manifest::{
    AdapterEffectCapability, AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall,
    AdapterManifest, AdapterTypeKind,
};
use arcweft_rust_abi::{
    ArcweftRustManifest, ArcweftRustPackage, ArcweftRustTypeDecl, ArcweftRustTypeKind,
    ArcweftRustVariant,
};

pub const DESKTOP_PLATFORM_ADAPTER_ID: &str = "desktop-platform";
pub const DESKTOP_OWNED_WINDOW_ADAPTER_ID: &str = "desktop-window-owned";
pub const DESKTOP_FILES_READ_ADAPTER_ID: &str = "desktop-files-user-read";
pub const DESKTOP_FILES_WRITE_ADAPTER_ID: &str = "desktop-files-user-write";
pub const DESKTOP_KNOWN_READ_ADAPTER_ID: &str = "desktop-files-known-read";
pub const DESKTOP_KNOWN_WRITE_ADAPTER_ID: &str = "desktop-files-known-write";
pub const DESKTOP_GLOBAL_POINTER_OBSERVE_ADAPTER_ID: &str = "desktop-pointer-global-observe";
pub const DESKTOP_GLOBAL_POINTER_CONTROL_ADAPTER_ID: &str = "desktop-pointer-global-control";
pub const DESKTOP_EXTERNAL_OBSERVE_ADAPTER_ID: &str = "desktop-window-external-observe";
pub const DESKTOP_EXTERNAL_CONTROL_ADAPTER_ID: &str = "desktop-window-external-control";

pub const DESKTOP_CAPABILITIES_CALL: &str = "desktop.platform.capabilities";
pub const DESKTOP_OWNED_WINDOW_SET_TITLE_CALL: &str = "desktop.window.owned.set_title";
pub const DESKTOP_OWNED_WINDOW_SET_BOUNDS_CALL: &str = "desktop.window.owned.set_bounds";
pub const DESKTOP_OWNED_WINDOW_SET_MODE_CALL: &str = "desktop.window.owned.set_mode";
pub const DESKTOP_OWNED_WINDOW_REQUEST_FOCUS_CALL: &str = "desktop.window.owned.request_focus";
pub const DESKTOP_OWNED_WINDOW_REQUEST_CLOSE_CALL: &str = "desktop.window.owned.request_close";
pub const DESKTOP_OWNED_CURSOR_SET_ICON_CALL: &str = "desktop.cursor.owned.set_icon";
pub const DESKTOP_OWNED_CURSOR_SET_VISIBLE_CALL: &str = "desktop.cursor.owned.set_visible";
pub const DESKTOP_OWNED_CURSOR_SET_POSITION_CALL: &str = "desktop.cursor.owned.set_position";
pub const DESKTOP_WINDOW_MODE_TYPE: &str = "WindowMode";
pub const DESKTOP_CURSOR_ICON_TYPE: &str = "CursorIcon";
pub const DESKTOP_FILES_READ_CALL: &str = "desktop.files.user.read";
pub const DESKTOP_FILES_WRITE_CALL: &str = "desktop.files.user.write";
pub const DESKTOP_KNOWN_READ_CALL: &str = "desktop.files.known.read";
pub const DESKTOP_KNOWN_WRITE_CALL: &str = "desktop.files.known.write";
pub const DESKTOP_GLOBAL_POINTER_OBSERVE_CALL: &str = "desktop.pointer.global.observe";
pub const DESKTOP_GLOBAL_POINTER_CONTROL_CALL: &str = "desktop.pointer.global.control";
pub const DESKTOP_EXTERNAL_OBSERVE_CALL: &str = "desktop.window.external.observe";
pub const DESKTOP_EXTERNAL_CONTROL_CALL: &str = "desktop.window.external.control";

pub fn desktop_platform_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_PLATFORM_ADAPTER_ID,
        "Desktop Platform Capabilities",
        "desktop.platform.read",
        [DESKTOP_CAPABILITIES_CALL],
    )
}

pub fn desktop_owned_window_manifest() -> AdapterManifest {
    let effect = AdapterEffectCapability::new("desktop.window.owned.control");
    [
        owned_call(
            DESKTOP_OWNED_WINDOW_SET_TITLE_CALL,
            [param("title", AdapterTypeKind::String)],
        ),
        owned_call(
            DESKTOP_OWNED_WINDOW_SET_BOUNDS_CALL,
            [
                param("x", AdapterTypeKind::I32),
                param("y", AdapterTypeKind::I32),
                param("width", AdapterTypeKind::U32),
                param("height", AdapterTypeKind::U32),
            ],
        ),
        owned_call(DESKTOP_OWNED_WINDOW_SET_MODE_CALL, [window_mode_param()]),
        owned_call::<0>(DESKTOP_OWNED_WINDOW_REQUEST_FOCUS_CALL, []),
        owned_call::<0>(DESKTOP_OWNED_WINDOW_REQUEST_CLOSE_CALL, []),
        owned_call(DESKTOP_OWNED_CURSOR_SET_ICON_CALL, [cursor_icon_param()]),
        owned_call(
            DESKTOP_OWNED_CURSOR_SET_VISIBLE_CALL,
            [param("visible", AdapterTypeKind::Bool)],
        ),
        owned_call(
            DESKTOP_OWNED_CURSOR_SET_POSITION_CALL,
            [
                param("x", AdapterTypeKind::I32),
                param("y", AdapterTypeKind::I32),
            ],
        ),
    ]
    .into_iter()
    .fold(
        AdapterManifest::new(DESKTOP_OWNED_WINDOW_ADAPTER_ID, "Owned Desktop Window")
            .with_effect(effect.clone())
            .with_rust_manifest(&owned_window_rust_manifest()),
        |manifest, (call, signature)| {
            manifest
                .with_function_signature(
                    call,
                    AdapterFunctionSignature::new(
                        need_string_desktop_error(),
                        signature.params().iter().cloned(),
                    ),
                    [effect.clone()],
                )
                .with_host_call(AdapterHostCall::with_signature(
                    call,
                    signature,
                    [effect.clone()],
                ))
        },
    )
}

pub fn is_desktop_owned_window_host_call(host_call: &str) -> bool {
    matches!(
        host_call,
        DESKTOP_OWNED_WINDOW_SET_TITLE_CALL
            | DESKTOP_OWNED_WINDOW_SET_BOUNDS_CALL
            | DESKTOP_OWNED_WINDOW_SET_MODE_CALL
            | DESKTOP_OWNED_WINDOW_REQUEST_FOCUS_CALL
            | DESKTOP_OWNED_WINDOW_REQUEST_CLOSE_CALL
            | DESKTOP_OWNED_CURSOR_SET_ICON_CALL
            | DESKTOP_OWNED_CURSOR_SET_VISIBLE_CALL
            | DESKTOP_OWNED_CURSOR_SET_POSITION_CALL
    )
}

pub fn desktop_files_read_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_FILES_READ_ADAPTER_ID,
        "User-Selected Desktop Files (Read)",
        "desktop.files.user.read",
        [DESKTOP_FILES_READ_CALL],
    )
}

pub fn desktop_files_write_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_FILES_WRITE_ADAPTER_ID,
        "User-Selected Desktop Files (Write)",
        "desktop.files.user.write",
        [DESKTOP_FILES_WRITE_CALL],
    )
}

pub fn desktop_known_directory_read_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_KNOWN_READ_ADAPTER_ID,
        "Host-Allowlisted Known Directories (Read Grant)",
        "desktop.files.known.read",
        [DESKTOP_KNOWN_READ_CALL],
    )
}

pub fn desktop_known_directory_write_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_KNOWN_WRITE_ADAPTER_ID,
        "Host-Allowlisted Known Directories (Write Grant)",
        "desktop.files.known.write",
        [DESKTOP_KNOWN_WRITE_CALL],
    )
}

pub fn desktop_pointer_global_observe_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_GLOBAL_POINTER_OBSERVE_ADAPTER_ID,
        "Global Desktop Pointer Observation",
        "desktop.pointer.global.observe",
        [DESKTOP_GLOBAL_POINTER_OBSERVE_CALL],
    )
}

pub fn desktop_pointer_global_control_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_GLOBAL_POINTER_CONTROL_ADAPTER_ID,
        "Global Desktop Pointer Control",
        "desktop.pointer.global.control",
        [DESKTOP_GLOBAL_POINTER_CONTROL_CALL],
    )
}

pub fn desktop_external_observe_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_EXTERNAL_OBSERVE_ADAPTER_ID,
        "External Window Observation",
        "desktop.window.external.observe",
        [DESKTOP_EXTERNAL_OBSERVE_CALL],
    )
}

pub fn desktop_external_control_manifest() -> AdapterManifest {
    manifest(
        DESKTOP_EXTERNAL_CONTROL_ADAPTER_ID,
        "External Window Control",
        "desktop.window.external.control",
        [DESKTOP_EXTERNAL_CONTROL_CALL],
    )
}

pub fn standard_desktop_manifests() -> Vec<AdapterManifest> {
    vec![
        desktop_platform_manifest(),
        desktop_owned_window_manifest(),
        desktop_files_read_manifest(),
    ]
}

pub fn all_desktop_manifests() -> Vec<AdapterManifest> {
    vec![
        desktop_platform_manifest(),
        desktop_owned_window_manifest(),
        desktop_files_read_manifest(),
        desktop_files_write_manifest(),
        desktop_known_directory_read_manifest(),
        desktop_known_directory_write_manifest(),
        desktop_pointer_global_observe_manifest(),
        desktop_pointer_global_control_manifest(),
        desktop_external_observe_manifest(),
        desktop_external_control_manifest(),
    ]
}

fn manifest<const N: usize>(
    id: &str,
    display_name: &str,
    effect_id: &str,
    host_calls: [&str; N],
) -> AdapterManifest {
    let effect = AdapterEffectCapability::new(effect_id);
    host_calls.into_iter().fold(
        AdapterManifest::new(id, display_name).with_effect(effect.clone()),
        |manifest, host_call| {
            manifest.with_host_call(AdapterHostCall::new(host_call, [effect.clone()]))
        },
    )
}

fn owned_call<const N: usize>(
    call: &'static str,
    params: [AdapterFunctionParam; N],
) -> (&'static str, AdapterFunctionSignature) {
    (
        call,
        AdapterFunctionSignature::new(AdapterTypeKind::String, params),
    )
}

fn param(name: &'static str, ty: AdapterTypeKind) -> AdapterFunctionParam {
    AdapterFunctionParam::required(name, ty)
}

fn window_mode_param() -> AdapterFunctionParam {
    param(
        "mode",
        AdapterTypeKind::Named(DESKTOP_WINDOW_MODE_TYPE.to_owned()),
    )
}

fn cursor_icon_param() -> AdapterFunctionParam {
    param(
        "icon",
        AdapterTypeKind::Named(DESKTOP_CURSOR_ICON_TYPE.to_owned()),
    )
}

fn owned_window_rust_manifest() -> ArcweftRustManifest {
    ArcweftRustManifest::new(ArcweftRustPackage {
        name: "arcweft-adapter-desktop".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        metadata_hash: None,
    })
    .with_type(unit_enum_type(
        DESKTOP_WINDOW_MODE_TYPE,
        "arcweft_desktop_contract::WindowMode",
        [
            "Normal",
            "Minimized",
            "Maximized",
            "BorderlessFullscreen",
            "Fullscreen",
        ],
    ))
    .with_type(unit_enum_type(
        DESKTOP_CURSOR_ICON_TYPE,
        "arcweft_desktop_contract::CursorIcon",
        [
            "Default",
            "Pointer",
            "Text",
            "Crosshair",
            "Move",
            "NotAllowed",
            "Wait",
            "Progress",
            "Help",
            "ZoomIn",
            "ZoomOut",
            "Grab",
            "Grabbing",
            "ResizeHorizontal",
            "ResizeVertical",
            "ResizeDiagonalNorthEastSouthWest",
            "ResizeDiagonalNorthWestSouthEast",
            "Hidden",
        ],
    ))
}

fn unit_enum_type<const N: usize>(
    name: &str,
    rust_path: &str,
    variants: [&str; N],
) -> ArcweftRustTypeDecl {
    ArcweftRustTypeDecl {
        name: name.to_owned(),
        rust_path: rust_path.to_owned(),
        kind: ArcweftRustTypeKind::Enum {
            variants: variants
                .into_iter()
                .map(|name| ArcweftRustVariant {
                    name: name.to_owned(),
                    fields: Vec::new(),
                })
                .collect(),
        },
    }
}

fn need_string_desktop_error() -> AdapterTypeKind {
    AdapterTypeKind::Need {
        ready: Box::new(AdapterTypeKind::String),
        error: Box::new(AdapterTypeKind::Named("DesktopError".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn logical_manifests_have_unique_ids_and_host_call_owners() {
        let manifests = all_desktop_manifests();
        let ids = manifests
            .iter()
            .map(|manifest| manifest.id().as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), ids.iter().collect::<BTreeSet<_>>().len());

        let calls = manifests
            .into_iter()
            .flat_map(|manifest| {
                manifest
                    .host_calls()
                    .iter()
                    .map(|call| call.id().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), calls.iter().collect::<BTreeSet<_>>().len());
    }

    #[test]
    fn standard_profile_excludes_non_user_selected_and_global_authority() {
        let ids = standard_desktop_manifests()
            .into_iter()
            .map(|manifest| manifest.id().as_str().to_owned())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains(DESKTOP_PLATFORM_ADAPTER_ID));
        assert!(ids.contains(DESKTOP_OWNED_WINDOW_ADAPTER_ID));
        assert!(ids.contains(DESKTOP_FILES_READ_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_FILES_WRITE_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_KNOWN_READ_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_KNOWN_WRITE_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_GLOBAL_POINTER_OBSERVE_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_GLOBAL_POINTER_CONTROL_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_EXTERNAL_OBSERVE_ADAPTER_ID));
        assert!(!ids.contains(DESKTOP_EXTERNAL_CONTROL_ADAPTER_ID));
    }

    #[test]
    fn owned_window_manifest_exports_typed_source_functions_and_host_calls() {
        let manifest = desktop_owned_window_manifest();
        let set_bounds = manifest
            .functions()
            .iter()
            .find(|function| function.name() == DESKTOP_OWNED_WINDOW_SET_BOUNDS_CALL)
            .expect("set_bounds function");
        assert_eq!(set_bounds.signature().params().len(), 4);
        assert_eq!(
            set_bounds.signature().return_type(),
            &AdapterTypeKind::Need {
                ready: Box::new(AdapterTypeKind::String),
                error: Box::new(AdapterTypeKind::Named("DesktopError".to_owned())),
            }
        );

        let set_mode = manifest
            .functions()
            .iter()
            .find(|function| function.name() == DESKTOP_OWNED_WINDOW_SET_MODE_CALL)
            .expect("set_mode function");
        assert_eq!(
            set_mode.signature().params()[0].ty(),
            &AdapterTypeKind::Named(DESKTOP_WINDOW_MODE_TYPE.to_owned())
        );

        let set_icon = manifest
            .functions()
            .iter()
            .find(|function| function.name() == DESKTOP_OWNED_CURSOR_SET_ICON_CALL)
            .expect("set_icon function");
        assert_eq!(
            set_icon.signature().params()[0].ty(),
            &AdapterTypeKind::Named(DESKTOP_CURSOR_ICON_TYPE.to_owned())
        );

        let request_close = manifest
            .host_calls()
            .iter()
            .find(|call| call.id() == DESKTOP_OWNED_WINDOW_REQUEST_CLOSE_CALL)
            .expect("request_close host call");
        assert!(request_close.signature().params().is_empty());
        assert_eq!(
            request_close.signature().return_type(),
            &AdapterTypeKind::String
        );

        assert!(manifest.rust_types().iter().any(|ty| {
            ty.decl().name == DESKTOP_WINDOW_MODE_TYPE
                && enum_contains(ty.decl(), ["Fullscreen", "BorderlessFullscreen"])
        }));
        assert!(manifest.rust_types().iter().any(|ty| {
            ty.decl().name == DESKTOP_CURSOR_ICON_TYPE
                && enum_contains(ty.decl(), ["Pointer", "Default"])
        }));
    }

    fn enum_contains<const N: usize>(decl: &ArcweftRustTypeDecl, expected: [&str; N]) -> bool {
        let ArcweftRustTypeKind::Enum { variants } = &decl.kind else {
            return false;
        };
        expected
            .into_iter()
            .all(|name| variants.iter().any(|variant| variant.name == name))
    }
}
