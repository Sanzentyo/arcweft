use arcweft_desktop_contract::{
    DesktopRequest, DesktopResponse, OwnedWindowRequest, OwnedWindowResponse, ScaleFactor,
    WindowId, WindowMode, WindowScope, WindowSnapshot,
};
use arcweft_desktop_host::{DesktopHost, DesktopSubmission, MemoryDesktopBackend};

#[test]
fn main_thread_work_is_pending_until_the_event_loop_pumps() {
    let backend = MemoryDesktopBackend::new();
    backend.insert_window(WindowSnapshot {
        id: WindowId::new("owned-1").expect("valid id"),
        scope: WindowScope::Owned,
        title: Some("Arcweft".to_owned()),
        application_name: None,
        process_id: None,
        bounds: None,
        scale_factor: Some(ScaleFactor::ONE),
        mode: WindowMode::Normal,
        visible: Some(true),
        focused: Some(true),
    });
    let host = DesktopHost::bind_current_thread(backend);

    let DesktopSubmission::Pending(task) =
        host.submit(DesktopRequest::OwnedWindow(OwnedWindowRequest::List))
    else {
        panic!("owned window request must use the main-thread lane");
    };
    assert_eq!(host.pending_count(), 1);
    let report = host.pump_main_thread().expect("main-thread pump succeeds");
    assert_eq!(report.started, 1);
    assert_eq!(report.completed, 1);
    assert!(matches!(
        host.poll(task),
        Some(Ok(DesktopResponse::OwnedWindow(
            OwnedWindowResponse::Windows(_)
        )))
    ));
}
