//! Audited Windows-only COM boundary for the TSF `ITextStoreACP` bridge.
//!
//! No type from this module may be stored in presentation, runtime-host, View,
//! replay, capture, or Agent observation data.

#![cfg(target_os = "windows")]
#![allow(clippy::inline_always, clippy::ref_as_ptr)]

use crate::text_input::windows_tsf::geometry::{
    TsfLayoutResult, TsfScreenRect, WindowsTsfGeometry,
};
use crate::text_input::windows_tsf::range::{TsfAcp, TsfAcpRange, TsfTextSnapshot};
use arcweft_presentation::text_input::{
    PlatformTextSelection, TextByteOffset, TextCommit, TextInputClientSnapshot,
    TextInputFocusGeneration, TextInputGeometrySnapshot, TextInputOperation,
    TextInputSecurityPolicy, TextInputSerial, TextRange, TextSelectionAffinity,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;
use thiserror::Error;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, E_NOTIMPL, HWND, RECT};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, FORMATETC, IDataObject,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_ThreadMgr, ITextStoreACP, ITextStoreACP_Impl, ITextStoreACPSink, ITfContext,
    ITfDocumentMgr, ITfThreadMgr, TEXT_STORE_LOCK_FLAGS, TF_E_DISCONNECTED, TS_AE_NONE, TS_ATTRVAL,
    TS_E_INVALIDPOS, TS_E_NOLAYOUT, TS_E_NOLOCK, TS_LC_CHANGE, TS_RT_PLAIN, TS_RUNINFO,
    TS_SELECTION_ACP, TS_STATUS, TS_TEXTCHANGE,
};
use windows_core::{
    BOOL, Error as WindowsError, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Ref,
    Result as WindowsResult, implement,
};

/// Safe error wrapper for the COM boundary.
#[derive(Debug, Error)]
pub enum TsfComError {
    #[error("Windows COM call failed: {0}")]
    Windows(#[from] WindowsError),
    #[error("Windows TSF bridge is destroyed")]
    Destroyed,
    #[error("Windows TSF bridge has no active text document")]
    NoActiveDocument,
    #[error("Windows TSF bridge state borrow conflict")]
    BorrowConflict,
}

/// Safe document update passed from `real_ime` into the COM text store.
#[derive(Clone, Debug)]
pub struct TsfDocumentUpdate {
    snapshot: TextInputClientSnapshot,
    geometry: Option<TextInputGeometrySnapshot>,
    generation: TextInputFocusGeneration,
    security: TextInputSecurityPolicy,
}

/// Mutating operations created by one TSF write callback.
#[derive(Clone, Debug)]
pub struct TsfOperationBatch {
    snapshot: TextInputClientSnapshot,
    generation: TextInputFocusGeneration,
    operations: Vec<TextInputOperation>,
}

/// Window-thread owner of TSF thread manager, document manager, and active context.
#[derive(Debug)]
pub struct WindowsTsfThreadContext {
    // RAII guard: COM must stay initialized until the TSF thread/document objects
    // owned by this context have been released.
    _apartment: CoApartment,
    // The active HWND is mirrored into `TsfSharedState` for callbacks; this field
    // keeps the owner identity visible on the lifetime owner itself.
    _hwnd: HWND,
    thread_mgr: ITfThreadMgr,
    client_id: u32,
    document_mgr: ITfDocumentMgr,
    document: Option<ActiveTsfDocument>,
    state: TsfSharedStateHandle,
}

#[derive(Debug)]
struct CoApartment;

#[derive(Debug)]
struct ActiveTsfDocument {
    // These COM references are intentionally retained while the document manager
    // has the context pushed, even though callbacks use the shared text-store state.
    _context: ITfContext,
    _edit_cookie: u32,
    _store: ITextStoreACP,
}

#[derive(Clone, Debug)]
struct TsfSharedState {
    hwnd: isize,
    destroyed: bool,
    active_lock: TsfLockKind,
    sink: Option<ITextStoreACPSink>,
    snapshot: Option<TextInputClientSnapshot>,
    geometry: Option<TextInputGeometrySnapshot>,
    generation: TextInputFocusGeneration,
    security: TextInputSecurityPolicy,
    pending: Vec<TsfOperationBatch>,
    next_serial: TextInputSerial,
    reconversion_available: bool,
}

type TsfSharedStateHandle = Rc<RefCell<TsfSharedState>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TsfLockKind {
    None,
    Read,
    ReadWrite,
}

#[implement(ITextStoreACP)]
struct ArcweftTextStore {
    state: TsfSharedStateHandle,
}

impl TsfDocumentUpdate {
    pub fn new(
        snapshot: &TextInputClientSnapshot,
        generation: TextInputFocusGeneration,
        security: TextInputSecurityPolicy,
    ) -> Self {
        Self {
            snapshot: snapshot.clone(),
            geometry: None,
            generation,
            security,
        }
    }

    #[must_use]
    pub fn with_geometry(mut self, geometry: Option<&TextInputGeometrySnapshot>) -> Self {
        self.geometry = geometry.cloned();
        self
    }
}

impl TsfOperationBatch {
    pub fn snapshot(&self) -> &TextInputClientSnapshot {
        &self.snapshot
    }

    pub const fn generation(&self) -> TextInputFocusGeneration {
        self.generation
    }

    pub fn into_operations(self) -> Vec<TextInputOperation> {
        self.operations
    }
}

impl WindowsTsfThreadContext {
    pub fn activate(hwnd: HWND) -> Result<Self, TsfComError> {
        let apartment = CoApartment::initialize()?;
        let thread_mgr: ITfThreadMgr = unsafe {
            // SAFETY: COM apartment is initialized on this thread and CLSID_TF_ThreadMgr
            // is the documented TSF thread manager class.
            CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)?
        };
        let client_id = unsafe {
            // SAFETY: thread_mgr is the activated TSF thread manager for this COM apartment.
            thread_mgr.Activate()?
        };
        let document_mgr = unsafe {
            // SAFETY: `thread_mgr` is activated and owned by this context.
            thread_mgr.CreateDocumentMgr()?
        };
        let state = Rc::new(RefCell::new(TsfSharedState::new(hwnd)));
        Ok(Self {
            _apartment: apartment,
            _hwnd: hwnd,
            thread_mgr,
            client_id,
            document_mgr,
            document: None,
            state,
        })
    }

    pub fn focus_text_input(&mut self, update: TsfDocumentUpdate) -> Result<(), TsfComError> {
        self.blur()?;
        self.replace_state(update)?;
        let store_impl = ArcweftTextStore {
            state: Rc::clone(&self.state),
        };
        let store: ITextStoreACP = store_impl.into();
        let mut context = None;
        let mut edit_cookie = 0_u32;
        unsafe {
            // SAFETY: `store` is a COM object implementing ITextStoreACP. Context and
            // edit-cookie out pointers are valid for this call.
            self.document_mgr.CreateContext(
                self.client_id,
                0,
                &store,
                &raw mut context,
                &raw mut edit_cookie,
            )?;
        }
        let context = context.ok_or_else(|| WindowsError::from(E_FAIL))?;
        unsafe {
            // SAFETY: The context was just created for this document manager.
            self.document_mgr.Push(&context)?;
            self.thread_mgr.SetFocus(&self.document_mgr)?;
        }
        self.document = Some(ActiveTsfDocument {
            _context: context,
            _edit_cookie: edit_cookie,
            _store: store,
        });
        Ok(())
    }

    pub fn update_document(&mut self, update: TsfDocumentUpdate) -> Result<(), TsfComError> {
        self.replace_state(update)?;
        self.notify_layout_changed()?;
        Ok(())
    }

    pub fn update_geometry(
        &mut self,
        geometry: &TextInputGeometrySnapshot,
    ) -> Result<(), TsfComError> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| TsfComError::BorrowConflict)?;
        state.geometry = Some(
            if state.security == TextInputSecurityPolicy::SecureRedacted {
                state.security.redact_geometry(geometry)
            } else {
                geometry.clone()
            },
        );
        drop(state);
        self.notify_layout_changed()
    }

    pub fn blur(&mut self) -> Result<(), TsfComError> {
        if let Some(_document) = self.document.take() {
            unsafe {
                // SAFETY: The document manager belongs to this thread context. Pop is
                // valid if a context is on the stack; TSF returns an error otherwise,
                // which is intentionally ignored during idempotent blur.
                let _ = self.document_mgr.Pop(0);
            }
        }
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| TsfComError::BorrowConflict)?;
        state.snapshot = None;
        state.geometry = None;
        state.sink = None;
        state.active_lock = TsfLockKind::None;
        Ok(())
    }

    pub fn drain_operations(&mut self) -> Vec<TsfOperationBatch> {
        self.state
            .try_borrow_mut()
            .map(|mut state| std::mem::take(&mut state.pending))
            .unwrap_or_default()
    }

    pub fn has_layout(&self) -> bool {
        self.state
            .try_borrow()
            .is_ok_and(|state| state.geometry.is_some())
    }

    pub fn reconversion_available(&self) -> bool {
        self.state
            .try_borrow()
            .is_ok_and(|state| state.reconversion_available)
    }

    pub fn next_arcweft_serial_hint(&self) -> TextInputSerial {
        self.state
            .try_borrow()
            .map_or(TextInputSerial(1), |state| state.next_serial)
    }

    fn replace_state(&mut self, update: TsfDocumentUpdate) -> Result<(), TsfComError> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| TsfComError::BorrowConflict)?;
        state.destroyed = false;
        state.generation = update.generation;
        state.security = update.security;
        state.snapshot = Some(match update.security {
            TextInputSecurityPolicy::Plain => update.snapshot,
            TextInputSecurityPolicy::SecureRedacted => {
                update.security.redact_snapshot(&update.snapshot)
            }
        });
        state.geometry = update
            .geometry
            .map(|geometry| update.security.redact_geometry(&geometry));
        Ok(())
    }

    fn notify_layout_changed(&self) -> Result<(), TsfComError> {
        let sink = self
            .state
            .try_borrow()
            .map_err(|_| TsfComError::BorrowConflict)?
            .sink
            .clone();
        if let Some(sink) = sink {
            unsafe {
                // SAFETY: The sink is a COM reference previously supplied by TSF via
                // AdviseSink and retained until UnadviseSink/blur.
                let _ = sink.OnLayoutChange(TS_LC_CHANGE, 0);
            }
        }
        Ok(())
    }
}

impl Drop for WindowsTsfThreadContext {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.destroyed = true;
            state.sink = None;
            state.pending.clear();
        }
        let _ = self.blur();
        unsafe {
            // SAFETY: thread_mgr was activated by this context. Errors during drop are
            // ignored because shutdown is best-effort and no Rust state is mutated after.
            let _ = self.thread_mgr.Deactivate();
        }
    }
}

impl CoApartment {
    fn initialize() -> Result<Self, TsfComError> {
        unsafe {
            // SAFETY: Initializes COM for the current thread before any COM object is
            // created. Matching CoUninitialize is in Drop.
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}

impl Drop for CoApartment {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: Called once for a successfully initialized apartment on the same
            // owning thread during context drop.
            CoUninitialize();
        }
    }
}

impl TsfSharedState {
    fn new(hwnd: HWND) -> Self {
        Self {
            hwnd: hwnd.0 as isize,
            destroyed: false,
            active_lock: TsfLockKind::None,
            sink: None,
            snapshot: None,
            geometry: None,
            generation: TextInputFocusGeneration::default(),
            security: TextInputSecurityPolicy::Plain,
            pending: Vec::new(),
            next_serial: TextInputSerial(1),
            reconversion_available: false,
        }
    }

    fn snapshot(&self) -> WindowsResult<&TextInputClientSnapshot> {
        if self.destroyed {
            return Err(WindowsError::from(TF_E_DISCONNECTED));
        }
        self.snapshot
            .as_ref()
            .ok_or_else(|| WindowsError::from(TF_E_DISCONNECTED))
    }

    fn require_read(&self) -> WindowsResult<()> {
        if matches!(self.active_lock, TsfLockKind::Read | TsfLockKind::ReadWrite) {
            Ok(())
        } else {
            Err(WindowsError::from(TS_E_NOLOCK))
        }
    }

    fn require_write(&self) -> WindowsResult<()> {
        if self.active_lock == TsfLockKind::ReadWrite {
            Ok(())
        } else {
            Err(WindowsError::from(TS_E_NOLOCK))
        }
    }

    fn push_operations(&mut self, operations: Vec<TextInputOperation>) -> WindowsResult<()> {
        let snapshot = self.snapshot()?.clone();
        self.pending.push(TsfOperationBatch {
            snapshot,
            generation: self.generation,
            operations,
        });
        self.next_serial = self.next_serial.next();
        Ok(())
    }
}

#[allow(non_snake_case)]
impl ITextStoreACP_Impl for ArcweftTextStore_Impl {
    fn AdviseSink(
        &self,
        riid: *const GUID,
        punk: Ref<'_, IUnknown>,
        _dwmask: u32,
    ) -> WindowsResult<()> {
        if riid.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let Some(punk) = punk.as_ref() else {
            return Err(WindowsError::from(E_INVALIDARG));
        };
        let sink: ITextStoreACPSink = punk.cast()?;
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.sink = Some(sink);
        Ok(())
    }

    fn UnadviseSink(&self, punk: Ref<'_, IUnknown>) -> WindowsResult<()> {
        let Some(_punk) = punk.as_ref() else {
            return Err(WindowsError::from(E_INVALIDARG));
        };
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.sink = None;
        Ok(())
    }

    fn RequestLock(&self, dwlockflags: u32) -> WindowsResult<HRESULT> {
        let sink = {
            let mut state = self
                .state
                .try_borrow_mut()
                .map_err(|_| WindowsError::from(E_FAIL))?;
            state.active_lock = if dwlockflags & 0x2 != 0 {
                TsfLockKind::ReadWrite
            } else {
                TsfLockKind::Read
            };
            state.sink.clone()
        };
        let result = if let Some(sink) = sink {
            unsafe {
                // SAFETY: sink is a retained COM reference supplied by TSF.
                sink.OnLockGranted(TEXT_STORE_LOCK_FLAGS(dwlockflags))
            }
        } else {
            Err(WindowsError::from(TF_E_DISCONNECTED))
        };
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.active_lock = TsfLockKind::None;
        Ok(result.map_or_else(|error| error.code(), |()| HRESULT(0)))
    }

    fn GetStatus(&self) -> WindowsResult<TS_STATUS> {
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_read()?;
        Ok(TS_STATUS {
            dwDynamicFlags: 0,
            dwStaticFlags: 0,
        })
    }

    fn QueryInsert(
        &self,
        acpTestStart: i32,
        acpTestEnd: i32,
        cch: u32,
        pacpResultStart: *mut i32,
        pacpResultEnd: *mut i32,
    ) -> WindowsResult<()> {
        if pacpResultStart.is_null() || pacpResultEnd.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        let snapshot = state.snapshot()?;
        let tsf = TsfTextSnapshot::plain(snapshot.revision(), snapshot.surrounding_text());
        let range = TsfAcpRange::new(TsfAcp(acpTestStart), TsfAcp(acpTestEnd));
        range
            .to_canonical_byte_range(&tsf, snapshot.revision())
            .map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
        unsafe {
            // SAFETY: out pointers are checked non-null.
            *pacpResultStart = acpTestStart;
            *pacpResultEnd = acpTestStart.saturating_add(i32::try_from(cch).unwrap_or(i32::MAX));
        }
        Ok(())
    }

    fn GetSelection(
        &self,
        _ulIndex: u32,
        _ulCount: u32,
        pSelection: *mut TS_SELECTION_ACP,
        pcFetched: *mut u32,
    ) -> WindowsResult<()> {
        if pSelection.is_null() || pcFetched.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_read()?;
        let snapshot = state.snapshot()?;
        let selection = snapshot.selection();
        unsafe {
            // SAFETY: out pointers are checked non-null.
            (*pSelection).acpStart = i32::try_from(selection.start().0).unwrap_or(i32::MAX);
            (*pSelection).acpEnd = i32::try_from(selection.end().0).unwrap_or(i32::MAX);
            (*pSelection).style.ase = TS_AE_NONE;
            (*pSelection).style.fInterimChar = BOOL(0);
            *pcFetched = 1;
        }
        Ok(())
    }

    fn SetSelection(&self, ulCount: u32, pSelection: *const TS_SELECTION_ACP) -> WindowsResult<()> {
        if ulCount != 1 || pSelection.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_write()?;
        let snapshot = state.snapshot()?.clone();
        let native = unsafe {
            // SAFETY: pSelection was checked non-null and TSF declared one item.
            *pSelection
        };
        let tsf = TsfTextSnapshot::plain(snapshot.revision(), snapshot.surrounding_text());
        let range = TsfAcpRange::new(TsfAcp(native.acpStart), TsfAcp(native.acpEnd))
            .to_canonical_byte_range(&tsf, snapshot.revision())
            .map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
        state.push_operations(vec![TextInputOperation::SetSelection(
            PlatformTextSelection::new(range, TextSelectionAffinity::Downstream),
        )])
    }

    fn GetText(
        &self,
        acpStart: i32,
        acpEnd: i32,
        pchPlain: PWSTR,
        cchPlainReq: u32,
        pcchPlainRet: *mut u32,
        prgRunInfo: *mut TS_RUNINFO,
        ulRunInfoReq: u32,
        pulRunInfoOut: *mut u32,
        pacpNext: *mut i32,
    ) -> WindowsResult<()> {
        if pcchPlainRet.is_null() || pulRunInfoOut.is_null() || pacpNext.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_read()?;
        let snapshot = state.snapshot()?;
        if state.security == TextInputSecurityPolicy::SecureRedacted {
            unsafe {
                // SAFETY: out pointers checked non-null.
                *pcchPlainRet = 0;
                *pulRunInfoOut = 0;
                *pacpNext = 0;
            }
            return Ok(());
        }
        let tsf = TsfTextSnapshot::plain(snapshot.revision(), snapshot.surrounding_text());
        let end = if acpEnd < 0 {
            i32::try_from(snapshot.surrounding_text().encode_utf16().count()).unwrap_or(i32::MAX)
        } else {
            acpEnd
        };
        let range = TsfAcpRange::new(TsfAcp(acpStart), TsfAcp(end))
            .to_canonical_byte_range(&tsf, snapshot.revision())
            .map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
        let text = slice_text(snapshot.surrounding_text(), range)?;
        let utf16 = text
            .encode_utf16()
            .take(cchPlainReq as usize)
            .collect::<Vec<_>>();
        if !pchPlain.is_null() && !utf16.is_empty() {
            unsafe {
                // SAFETY: TSF supplied a buffer for at least cchPlainReq UTF-16 units;
                // utf16 length is capped to that request.
                ptr::copy_nonoverlapping(utf16.as_ptr(), pchPlain.0, utf16.len());
            }
        }
        unsafe {
            // SAFETY: out pointers checked non-null.
            *pcchPlainRet = u32::try_from(utf16.len()).unwrap_or(u32::MAX);
            *pulRunInfoOut = 0;
            *pacpNext = acpStart.saturating_add(i32::try_from(utf16.len()).unwrap_or(i32::MAX));
            if !prgRunInfo.is_null() && ulRunInfoReq > 0 {
                (*prgRunInfo).uCount = u32::try_from(utf16.len()).unwrap_or(u32::MAX);
                (*prgRunInfo).r#type = TS_RT_PLAIN;
                *pulRunInfoOut = 1;
            }
        }
        Ok(())
    }

    fn SetText(
        &self,
        _dwFlags: u32,
        acpStart: i32,
        acpEnd: i32,
        pchText: &PCWSTR,
        cch: u32,
    ) -> WindowsResult<TS_TEXTCHANGE> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_write()?;
        let snapshot = state.snapshot()?.clone();
        let tsf = TsfTextSnapshot::plain(snapshot.revision(), snapshot.surrounding_text());
        let replacement = TsfAcpRange::new(TsfAcp(acpStart), TsfAcp(acpEnd))
            .to_canonical_byte_range(&tsf, snapshot.revision())
            .map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
        let text = read_pcwstr(pchText, cch)?;
        state.push_operations(vec![TextInputOperation::Commit(
            TextCommit::new(text).with_replacement(replacement),
        )])?;
        Ok(TS_TEXTCHANGE {
            acpStart,
            acpOldEnd: acpEnd,
            acpNewEnd: acpStart.saturating_add(i32::try_from(cch).unwrap_or(i32::MAX)),
        })
    }

    fn GetEndACP(&self) -> WindowsResult<i32> {
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_read()?;
        let snapshot = state.snapshot()?;
        let len = if state.security == TextInputSecurityPolicy::SecureRedacted {
            0
        } else {
            snapshot.surrounding_text().encode_utf16().count()
        };
        Ok(i32::try_from(len).unwrap_or(i32::MAX))
    }

    fn GetActiveView(&self) -> WindowsResult<u32> {
        Ok(0)
    }

    fn GetTextExt(
        &self,
        _vcView: u32,
        acpStart: i32,
        acpEnd: i32,
        prc: *mut RECT,
        pfClipped: *mut BOOL,
    ) -> WindowsResult<()> {
        if prc.is_null() || pfClipped.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_read()?;
        let snapshot = state.snapshot()?;
        let geometry = state
            .geometry
            .as_ref()
            .ok_or_else(|| WindowsError::from(TS_E_NOLAYOUT))?;
        let tsf = TsfTextSnapshot::plain(snapshot.revision(), snapshot.surrounding_text());
        let range = TsfAcpRange::new(TsfAcp(acpStart), TsfAcp(acpEnd))
            .to_canonical_byte_range(&tsf, snapshot.revision())
            .map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
        let result = WindowsTsfGeometry::new(state.security).text_ext(
            geometry,
            snapshot.revision(),
            range,
            false,
        );
        let (rect, clipped) = match result {
            TsfLayoutResult::Available { rect, clipped } => (rect, clipped),
            TsfLayoutResult::NoLayout
            | TsfLayoutResult::SecureRedacted
            | TsfLayoutResult::StaleRevision { .. } => {
                return Err(WindowsError::from(TS_E_NOLAYOUT));
            }
            TsfLayoutResult::InvalidRange => return Err(WindowsError::from(TS_E_INVALIDPOS)),
        };
        write_rect(prc, rect);
        unsafe {
            // SAFETY: `pfClipped` checked non-null.
            *pfClipped = BOOL(i32::from(clipped));
        }
        Ok(())
    }

    fn GetScreenExt(&self, _vcView: u32) -> WindowsResult<RECT> {
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        state.require_read()?;
        let geometry = state
            .geometry
            .as_ref()
            .ok_or_else(|| WindowsError::from(TS_E_NOLAYOUT))?;
        Ok(TsfScreenRect::enclosing(geometry.screen_control_rect()).into())
    }

    fn GetWnd(&self, _vcView: u32) -> WindowsResult<HWND> {
        let state = self
            .state
            .try_borrow()
            .map_err(|_| WindowsError::from(E_FAIL))?;
        Ok(HWND(state.hwnd as *mut c_void))
    }

    fn GetFormattedText(&self, _acpStart: i32, _acpEnd: i32) -> WindowsResult<IDataObject> {
        Err(WindowsError::from(E_NOTIMPL))
    }

    fn InsertEmbeddedAtSelection(
        &self,
        _dwFlags: u32,
        _pDataObject: Ref<'_, IDataObject>,
        _pacpStart: *mut i32,
        _pacpEnd: *mut i32,
        _pChange: *mut TS_TEXTCHANGE,
    ) -> WindowsResult<()> {
        Err(WindowsError::from(E_NOTIMPL))
    }

    fn InsertTextAtSelection(
        &self,
        dwFlags: u32,
        pchText: &PCWSTR,
        cch: u32,
        pacpStart: *mut i32,
        pacpEnd: *mut i32,
        pChange: *mut TS_TEXTCHANGE,
    ) -> WindowsResult<()> {
        if pacpStart.is_null() || pacpEnd.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        let acp = {
            let state = self
                .state
                .try_borrow()
                .map_err(|_| WindowsError::from(E_FAIL))?;
            let selection = state.snapshot()?.selection();
            i32::try_from(selection.start().0).unwrap_or(i32::MAX)
        };
        let change = self.SetText(dwFlags, acp, acp, pchText, cch)?;
        unsafe {
            // SAFETY: out pointers checked non-null.
            *pacpStart = acp;
            *pacpEnd = acp.saturating_add(i32::try_from(cch).unwrap_or(i32::MAX));
            if !pChange.is_null() {
                *pChange = change;
            }
        }
        Ok(())
    }

    fn QueryInsertEmbedded(
        &self,
        _pguidService: *const GUID,
        _pFormatEtc: *const FORMATETC,
    ) -> WindowsResult<BOOL> {
        Ok(BOOL(0))
    }

    fn RequestSupportedAttrs(
        &self,
        _dwFlags: u32,
        _cFilterAttrs: u32,
        _paFilterAttrs: *const GUID,
    ) -> WindowsResult<()> {
        Ok(())
    }

    fn RequestAttrsAtPosition(
        &self,
        _acpPos: i32,
        _cFilterAttrs: u32,
        _paFilterAttrs: *const GUID,
        _dwFlags: u32,
    ) -> WindowsResult<()> {
        Ok(())
    }

    fn RequestAttrsTransitioningAtPosition(
        &self,
        _acpPos: i32,
        _cFilterAttrs: u32,
        _paFilterAttrs: *const GUID,
        _dwFlags: u32,
    ) -> WindowsResult<()> {
        Ok(())
    }

    fn FindNextAttrTransition(
        &self,
        _acpStart: i32,
        acpHalt: i32,
        _cFilterAttrs: u32,
        _paFilterAttrs: *const GUID,
        _dwFlags: u32,
        pacpNext: *mut i32,
        pfFound: *mut BOOL,
        _plFoundOffset: *mut i32,
    ) -> WindowsResult<()> {
        if pacpNext.is_null() || pfFound.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        unsafe {
            // SAFETY: out pointers checked non-null.
            *pacpNext = acpHalt;
            *pfFound = BOOL(0);
        }
        Ok(())
    }

    fn RetrieveRequestedAttrs(
        &self,
        _ulCount: u32,
        _paAttrVals: *mut TS_ATTRVAL,
        pcFetched: *mut u32,
    ) -> WindowsResult<()> {
        if pcFetched.is_null() {
            return Err(WindowsError::from(E_INVALIDARG));
        }
        unsafe {
            // SAFETY: out pointer checked non-null.
            *pcFetched = 0;
        }
        Ok(())
    }

    fn GetACPFromPoint(
        &self,
        _vcView: u32,
        _pt: *const windows::Win32::Foundation::POINT,
        _dwFlags: u32,
    ) -> WindowsResult<i32> {
        Ok(0)
    }

    fn GetEmbedded(
        &self,
        _acpPos: i32,
        _rguidService: *const GUID,
        _riid: *const GUID,
    ) -> WindowsResult<IUnknown> {
        Err(WindowsError::from(E_NOTIMPL))
    }

    fn InsertEmbedded(
        &self,
        _dwFlags: u32,
        _acpStart: i32,
        _acpEnd: i32,
        _pDataObject: Ref<'_, IDataObject>,
    ) -> WindowsResult<TS_TEXTCHANGE> {
        Err(WindowsError::from(E_NOTIMPL))
    }
}

impl From<TsfScreenRect> for RECT {
    fn from(rect: TsfScreenRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

fn slice_text(text: &str, range: TextRange<TextByteOffset>) -> WindowsResult<&str> {
    let start =
        usize::try_from(range.start().0).map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
    let end = usize::try_from(range.end().0).map_err(|_| WindowsError::from(TS_E_INVALIDPOS))?;
    text.get(start..end)
        .ok_or_else(|| WindowsError::from(TS_E_INVALIDPOS))
}

fn read_pcwstr(value: &PCWSTR, len: u32) -> WindowsResult<String> {
    if len == 0 {
        return Ok(String::new());
    }
    if value.is_null() {
        return Err(WindowsError::from(E_INVALIDARG));
    }
    let len = usize::try_from(len).map_err(|_| WindowsError::from(E_INVALIDARG))?;
    let slice = unsafe {
        // SAFETY: caller supplied PCWSTR plus explicit length from TSF. We checked
        // non-null and convert exactly `len` UTF-16 code units.
        std::slice::from_raw_parts(value.0, len)
    };
    String::from_utf16(slice).map_err(|_| WindowsError::from(E_INVALIDARG))
}

fn write_rect(prc: *mut RECT, rect: TsfScreenRect) {
    unsafe {
        // SAFETY: all callers pass checked non-null RECT pointers.
        (*prc).left = rect.left;
        (*prc).top = rect.top;
        (*prc).right = rect.right;
        (*prc).bottom = rect.bottom;
    }
}

trait TextInputSerialNext {
    fn next(self) -> Self;
}

impl TextInputSerialNext for TextInputSerial {
    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
