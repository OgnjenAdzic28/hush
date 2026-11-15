//! A thin, safe wrapper over AXUIElement. Every call here is one round trip
//! to the target app, so callers batch reads with `values`.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr::{self, NonNull};

use objc2_application_services::{AXCopyMultipleAttributeOptions, AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType};

use crate::cf;

/// One live accessibility element. Equal elements are the same UI object
/// (`CFEqual`), which is what refs key on.
#[derive(Clone)]
pub struct Element(CFRetained<AXUIElement>);

impl PartialEq for Element {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl Eq for Element {}

impl Hash for Element {
    fn hash<S: Hasher>(&self, state: &mut S) {
        (*self.0).hash(state);
    }
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Element({:p})", CFRetained::as_ptr(&self.0))
    }
}

impl Element {
    pub(crate) fn application(pid: i32) -> Element {
        // SAFETY: creating an element for a pid has no preconditions; a bad
        // pid yields an element whose calls fail.
        Element(unsafe { AXUIElement::new_application(pid) })
    }

    pub(crate) fn system_wide() -> Element {
        // SAFETY: no preconditions.
        Element(unsafe { AXUIElement::new_system_wide() })
    }

    pub(crate) fn raw(&self) -> &AXUIElement {
        &self.0
    }

    /// Bounds how long any call to an unresponsive app can block. Set on the
    /// system-wide element it becomes the default for every element.
    pub(crate) fn set_timeout(&self, seconds: f32) {
        // SAFETY: plain setter on a live element.
        let _ = unsafe { self.0.set_messaging_timeout(seconds) };
    }

    pub(crate) fn value(&self, attribute: &str) -> Result<CFRetained<CFType>, AXError> {
        let name = cf::cfstr(attribute);
        let mut out: *const CFType = ptr::null();
        // SAFETY: `out` is a valid out-pointer; on success it holds a +1 reference.
        let status = unsafe { self.0.copy_attribute_value(&name, NonNull::from(&mut out)) };
        match NonNull::new(out.cast_mut()) {
            // SAFETY: the copy rule gave us ownership of one reference.
            Some(value) if status == AXError::Success => Ok(unsafe { CFRetained::from_raw(value) }),
            _ if status == AXError::Success => Err(AXError::NoValue),
            _ => Err(status),
        }
    }

    /// Several attributes in one round trip. Missing ones come back as `None`.
    pub(crate) fn values(&self, attributes: &CFArray<CFString>) -> Vec<Option<CFRetained<CFType>>> {
        let mut out: *const CFArray = ptr::null();
        // SAFETY: `attributes` is an array of CFStrings and `out` a valid out-pointer.
        let status = unsafe {
            self.0.copy_multiple_attribute_values(
                attributes.as_opaque(),
                AXCopyMultipleAttributeOptions::empty(),
                NonNull::from(&mut out),
            )
        };
        let wanted = attributes.len();
        let Some(array) = NonNull::new(out.cast_mut()).filter(|_| status == AXError::Success)
        else {
            return vec![None; wanted];
        };
        // SAFETY: copy rule, as above; the array holds CF values.
        let array: CFRetained<CFArray<CFType>> = unsafe { CFRetained::from_raw(array.cast()) };
        (0..wanted)
            .map(|i| array.get(i).filter(|v| !cf::is_error(v)))
            .collect()
    }

    pub(crate) fn string(&self, attribute: &str) -> Option<String> {
        self.value(attribute).ok().and_then(|v| cf::string(&v))
    }

    pub(crate) fn element(&self, attribute: &str) -> Option<Element> {
        let value = self.value(attribute).ok()?;
        value.downcast::<AXUIElement>().ok().map(Element)
    }

    pub(crate) fn elements(&self, attribute: &str) -> Vec<Element> {
        self.value(attribute)
            .ok()
            .map(|v| elements_of(&v))
            .unwrap_or_default()
    }

    /// How many values an array attribute holds, without fetching them.
    pub(crate) fn count(&self, attribute: &str) -> Option<usize> {
        let name = cf::cfstr(attribute);
        let mut count = 0;
        // SAFETY: valid out-pointer.
        let status = unsafe {
            self.0
                .attribute_value_count(&name, NonNull::from(&mut count))
        };
        (status == AXError::Success).then(|| usize::try_from(count).unwrap_or(0))
    }

    pub(crate) fn pid(&self) -> Option<i32> {
        let mut pid = 0;
        // SAFETY: valid out-pointer.
        let status = unsafe { self.0.pid(NonNull::from(&mut pid)) };
        (status == AXError::Success).then_some(pid)
    }
}

/// The elements in an array value (AXChildren, AXWindows, AXVisibleRows).
pub(crate) fn elements_of(value: &CFType) -> Vec<Element> {
    let Some(array) = value.downcast_ref::<CFArray>() else {
        return Vec::new();
    };
    // SAFETY: we only read; each item is checked to be an AXUIElement.
    let array: &CFArray<CFType> = unsafe { array.cast_unchecked() };
    array
        .iter()
        .filter_map(|item| item.downcast::<AXUIElement>().ok().map(Element))
        .collect()
}

pub(crate) fn trusted() -> bool {
    // SAFETY: no preconditions.
    unsafe { objc2_application_services::AXIsProcessTrusted() }
}
