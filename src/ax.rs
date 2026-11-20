//! Safe wrappers around the Accessibility (AX) API.
//!
//! All `unsafe` AX calls live here. Every call is defensive: any AX error is
//! turned into `None`/`Err` rather than a panic, and each app element gets a
//! short messaging timeout so an unresponsive app can't hang Kevro.

use std::ffi::c_void;
use std::fmt;
use std::ptr::{self, NonNull};

use objc2_app_kit::NSWorkspace;
use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElement, AXValue, AXValueType,
    kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFEqual, CFRetained, CFString, CFType, CGPoint, CGSize,
};

use crate::geom::Rect;

/// Seconds to wait for an app to answer an AX request.
const MESSAGING_TIMEOUT: f32 = 1.0;

/// Points of mismatch tolerated after setting a frame (apps clamp sizes).
const SET_TOLERANCE: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxErr {
    NotTrusted,
    NoFrontmostApp,
    NoFocusedWindow,
    Call(&'static str, i32),
}

impl fmt::Display for AxErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AxErr::NotTrusted => write!(f, "Accessibility permission not granted"),
            AxErr::NoFrontmostApp => write!(f, "no frontmost app"),
            AxErr::NoFocusedWindow => write!(f, "the frontmost app has no focused window"),
            AxErr::Call(what, code) => write!(f, "AX call {what} failed (AXError {code})"),
        }
    }
}

/// Is this process trusted for Accessibility?
pub fn is_trusted() -> bool {
    // SAFETY: no arguments, always safe to call.
    unsafe { AXIsProcessTrusted() }
}

/// Like `is_trusted`, but if not trusted asks macOS to show its
/// "would like to control this computer" prompt.
pub fn is_trusted_with_prompt() -> bool {
    // SAFETY: kAXTrustedCheckOptionPrompt is a valid static CFString; the
    // dictionary holds CF objects and lives for the duration of the call.
    unsafe {
        let key: &CFString = kAXTrustedCheckOptionPrompt;
        let value = CFBoolean::new(true);
        let dict = CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[value]);
        AXIsProcessTrustedWithOptions(Some(dict.as_opaque()))
    }
}

fn attr(name: &'static str) -> CFRetained<CFString> {
    CFString::from_static_str(name)
}

/// An AX element (app or window) plus the PID that owns it.
#[derive(Debug, Clone)]
pub struct Element {
    el: CFRetained<AXUIElement>,
    pid: i32,
}

pub type Window = Element;

impl PartialEq for Element {
    fn eq(&self, other: &Self) -> bool {
        // AX elements obtained separately for the same UI object are CFEqual.
        self.pid == other.pid && CFEqual(Some(&*self.el), Some(&*other.el))
    }
}

impl Element {
    pub fn application(pid: i32) -> Element {
        // SAFETY: creating an AX element for any PID is safe; invalid PIDs
        // just yield errors on use.
        let el = unsafe { AXUIElement::new_application(pid) };
        unsafe { el.set_messaging_timeout(MESSAGING_TIMEOUT) };
        Element { el, pid }
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    fn copy(&self, name: &'static str) -> Result<CFRetained<CFType>, AxErr> {
        let mut out: *const CFType = ptr::null();
        // SAFETY: `out` is a valid out-pointer; on success AX returns a +1
        // retained CF object which we take ownership of.
        let err = unsafe { self.el.copy_attribute_value(&attr(name), NonNull::from(&mut out)) };
        if err != AXError::Success {
            return Err(AxErr::Call(name, err.0));
        }
        NonNull::new(out.cast_mut())
            .map(|p| unsafe { CFRetained::from_raw(p) })
            .ok_or(AxErr::Call(name, -1))
    }

    fn set(&self, name: &'static str, value: &CFType) -> Result<(), AxErr> {
        // SAFETY: element and value are valid CF objects.
        let err = unsafe { self.el.set_attribute_value(&attr(name), value) };
        if err == AXError::Success { Ok(()) } else { Err(AxErr::Call(name, err.0)) }
    }

    fn child(&self, name: &'static str) -> Option<Element> {
        let v = self.copy(name).ok()?;
        let el = v.downcast::<AXUIElement>().ok()?;
        Some(Element { el, pid: self.pid })
    }

    pub fn string(&self, name: &'static str) -> Option<String> {
        self.copy(name).ok()?.downcast::<CFString>().ok().map(|s| s.to_string())
    }

    fn bool(&self, name: &'static str) -> Option<bool> {
        self.copy(name).ok()?.downcast::<CFBoolean>().ok().map(|b| b.as_bool())
    }

    fn ax_value<T: Default>(&self, name: &'static str, ty: AXValueType) -> Option<T> {
        let v = self.copy(name).ok()?.downcast::<AXValue>().ok()?;
        let mut out = T::default();
        // SAFETY: `out` has the C layout matching `ty` (CGPoint/CGSize).
        let ok = unsafe { v.value(ty, NonNull::from(&mut out).cast::<c_void>()) };
        ok.then_some(out)
    }

    fn set_ax_value<T>(&self, name: &'static str, ty: AXValueType, mut value: T) -> Result<(), AxErr> {
        // SAFETY: `value` has the C layout matching `ty`; AXValueCreate copies it.
        let v = unsafe { AXValue::new(ty, NonNull::from(&mut value).cast::<c_void>()) }
            .ok_or(AxErr::Call(name, -1))?;
        self.set(name, &v)
    }

    // ---- Windows -------------------------------------------------------

    pub fn frame(&self) -> Option<Rect> {
        let p: CGPoint = self.ax_value("AXPosition", AXValueType::CGPoint)?;
        let s: CGSize = self.ax_value("AXSize", AXValueType::CGSize)?;
        Some(Rect::new(p.x, p.y, s.width, s.height))
    }

    fn set_position(&self, r: &Rect) -> Result<(), AxErr> {
        self.set_ax_value("AXPosition", AXValueType::CGPoint, CGPoint { x: r.x, y: r.y })
    }

    fn set_size(&self, r: &Rect) -> Result<(), AxErr> {
        self.set_ax_value("AXSize", AXValueType::CGSize, CGSize { width: r.w, height: r.h })
    }

    /// Move and resize the window. Uses size → position → size because many
    /// apps clamp a new size to the current screen before moving. Returns the
    /// frame the window actually ended up with.
    pub fn set_frame(&self, target: Rect) -> Result<Rect, AxErr> {
        // Apps with AXEnhancedUserInterface on (set by VoiceOver and some
        // automation tools) animate and fight frame changes. Turn it off
        // briefly, as Rectangle does.
        let app = Element::application(self.pid);
        let enhanced = app.bool("AXEnhancedUserInterface") == Some(true);
        if enhanced {
            let _ = app.set("AXEnhancedUserInterface", &CFBoolean::new(false));
        }

        let result = (|| {
            let _ = self.set_size(&target);
            self.set_position(&target)?;
            self.set_size(&target)?;
            Ok(self.frame().unwrap_or(target))
        })();

        if enhanced {
            let _ = app.set("AXEnhancedUserInterface", &CFBoolean::new(true));
        }
        let actual = result?;
        if !actual.approx_eq(&target, SET_TOLERANCE) {
            tracing::debug!("window settled at {actual:?} instead of {target:?} (app clamped it)");
        }
        Ok(actual)
    }

    pub fn title(&self) -> Option<String> {
        self.string("AXTitle")
    }

    /// A normal document/app window (not a sheet, dialog, palette or popover).
    pub fn is_standard(&self) -> bool {
        self.string("AXSubrole").as_deref() == Some("AXStandardWindow")
    }

    pub fn is_minimized(&self) -> bool {
        self.bool("AXMinimized") == Some(true)
    }

    /// Bring this window to the front of its app and make it the main window.
    pub fn raise(&self) {
        // SAFETY: valid element and action name.
        let err = unsafe { self.el.perform_action(&attr("AXRaise")) };
        if err != AXError::Success {
            tracing::debug!("AXRaise failed: {}", err.0);
        }
        let _ = self.set("AXMain", &CFBoolean::new(true));
    }

    // ---- Apps ------------------------------------------------------------

    /// All windows of this app element.
    pub fn windows(&self) -> Vec<Window> {
        let Ok(v) = self.copy("AXWindows") else { return Vec::new() };
        let Ok(arr) = v.downcast::<CFArray>() else { return Vec::new() };
        // SAFETY: AXWindows is documented as an array of AXUIElements.
        let arr = unsafe { arr.cast_unchecked::<AXUIElement>() };
        arr.iter().map(|el| Element { el, pid: self.pid }).collect()
    }
}

/// The focused window of the frontmost app.
pub fn focused_window() -> Result<Window, AxErr> {
    if !is_trusted() {
        return Err(AxErr::NotTrusted);
    }
    let pid = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|a| a.processIdentifier())
        .ok_or(AxErr::NoFrontmostApp)?;
    let app = Element::application(pid);
    app.child("AXFocusedWindow")
        .or_else(|| app.child("AXMainWindow"))
        .or_else(|| app.windows().into_iter().find(|w| !w.is_minimized()))
        .ok_or(AxErr::NoFocusedWindow)
}
