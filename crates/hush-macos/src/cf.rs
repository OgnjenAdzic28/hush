//! Reading Core Foundation values that come back from Accessibility.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXValue, AXValueType};
use objc2_core_foundation::{CFBoolean, CFNumber, CFRetained, CFString, CFType, CGPoint, CGSize};

pub(crate) fn string(value: &CFType) -> Option<String> {
    if let Some(text) = value.downcast_ref::<CFString>() {
        return Some(text.to_string());
    }
    // Some apps report numeric values (sliders, steppers) as numbers.
    number(value).map(|n| {
        if n.fract() == 0.0 && n.abs() < 1e15 {
            format!("{}", n as i64)
        } else {
            format!("{n:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        }
    })
}

pub(crate) fn number(value: &CFType) -> Option<f64> {
    if let Some(number) = value.downcast_ref::<CFNumber>() {
        return number
            .as_f64()
            .or_else(|| number.as_i64().map(|n| n as f64));
    }
    value
        .downcast_ref::<CFBoolean>()
        .map(|b| f64::from(u8::from(b.as_bool())))
}

pub(crate) fn boolean(value: &CFType) -> Option<bool> {
    if let Some(b) = value.downcast_ref::<CFBoolean>() {
        return Some(b.as_bool());
    }
    value
        .downcast_ref::<CFNumber>()
        .and_then(CFNumber::as_i64)
        .map(|n| n != 0)
}

pub(crate) fn point(value: &CFType) -> Option<CGPoint> {
    let mut out = CGPoint { x: 0.0, y: 0.0 };
    ax_value(value, AXValueType::CGPoint, NonNull::from(&mut out).cast()).then_some(out)
}

pub(crate) fn size(value: &CFType) -> Option<CGSize> {
    let mut out = CGSize {
        width: 0.0,
        height: 0.0,
    };
    ax_value(value, AXValueType::CGSize, NonNull::from(&mut out).cast()).then_some(out)
}

/// True when `value` is the placeholder AXUIElementCopyMultipleAttributeValues
/// puts where an attribute couldn't be read.
pub(crate) fn is_error(value: &CFType) -> bool {
    value
        .downcast_ref::<AXValue>()
        // SAFETY: `value` is a live AXValue; reading its type has no preconditions.
        .is_some_and(|v| unsafe { v.r#type() } == AXValueType::AXError)
}

fn ax_value(value: &CFType, kind: AXValueType, out: NonNull<c_void>) -> bool {
    let Some(v) = value.downcast_ref::<AXValue>() else {
        return false;
    };
    // SAFETY: `out` points at a value of the C type that `kind` names, and
    // AXValueGetValue checks the stored type before writing.
    unsafe { v.r#type() == kind && v.value(kind, out) }
}

pub(crate) fn cfstr(text: &str) -> CFRetained<CFString> {
    CFString::from_str(text)
}
