use arcweft_adapter_context::manifest::{
    AdapterEffectCapability, AdapterHostCall, AdapterManifest,
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
pub const DESKTOP_OWNED_WINDOW_CALL: &str = "desktop.window.owned.request";
pub const DESKTOP_OWNED_CURSOR_CALL: &str = "desktop.cursor.owned.request";
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
    manifest(
        DESKTOP_OWNED_WINDOW_ADAPTER_ID,
        "Owned Desktop Window",
        "desktop.window.owned.control",
        [DESKTOP_OWNED_WINDOW_CALL, DESKTOP_OWNED_CURSOR_CALL],
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
}
