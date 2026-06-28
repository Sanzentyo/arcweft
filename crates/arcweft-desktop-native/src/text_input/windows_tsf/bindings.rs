//! Windows-only TSF type aliases.
//!
//! This module intentionally does not construct or expose COM objects through
//! normalized Arcweft payloads. Actual COM bootstrap remains adapter-local.

#![allow(dead_code)]

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::TextServices::{
    ITextStoreACP, ITfContext, ITfDisplayAttributeMgr, ITfDocumentMgr, ITfFnReconversion,
    ITfFunctionProvider, ITfThreadMgr,
};

pub(crate) type TsfThreadManager = ITfThreadMgr;
pub(crate) type TsfDocumentManager = ITfDocumentMgr;
pub(crate) type TsfContext = ITfContext;
pub(crate) type TsfTextStore = ITextStoreACP;
pub(crate) type TsfFunctionProvider = ITfFunctionProvider;
pub(crate) type TsfReconversion = ITfFnReconversion;
pub(crate) type TsfDisplayAttributeManager = ITfDisplayAttributeMgr;
pub(crate) type TsfClientId = u32;
pub(crate) type TsfEditCookie = u32;
pub(crate) type TsfWindow = HWND;
pub(crate) type NativeRect = RECT;
