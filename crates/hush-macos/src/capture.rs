//! Screenshots of one window, scaled and encoded as JPEG.
//!
//! The window server copies the window's own backing store, so the picture
//! is right even when the window is covered by others or on another Space,
//! and nothing on screen changes. Measured by open-computer-use on macOS 27:
//! 15 to 33 ms, where ScreenCaptureKit takes 87 to 155 ms and fails for
//! windows on inactive full-screen Spaces.

use std::ffi::c_void;
use std::ptr;

use hush_core::{Error, Image, Rect, Result};

use crate::private;

/// JPEG quality: small enough for a model to read cheaply, sharp enough for
/// text at 960 pixels wide.
const QUALITY: f64 = 0.6;

type Ref = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGImageGetWidth(image: Ref) -> usize;
    fn CGImageGetHeight(image: Ref) -> usize;
    fn CGImageCreateWithImageInRect(image: Ref, rect: CGRect) -> Ref;
    fn CGColorSpaceCreateDeviceRGB() -> Ref;
    fn CGColorSpaceCreateDeviceGray() -> Ref;
    fn CGBitmapContextGetData(context: Ref) -> *mut c_void;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: Ref,
        bitmap_info: u32,
    ) -> Ref;
    fn CGContextSetInterpolationQuality(context: Ref, quality: i32);
    fn CGContextDrawImage(context: Ref, rect: CGRect, image: Ref);
    fn CGBitmapContextCreateImage(context: Ref) -> Ref;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    static kCGImageDestinationLossyCompressionQuality: Ref;
    fn CGImageDestinationCreateWithData(data: Ref, kind: Ref, count: usize, options: Ref) -> Ref;
    fn CGImageDestinationAddImage(destination: Ref, image: Ref, properties: Ref);
    fn CGImageDestinationFinalize(destination: Ref) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFAllocatorDefault: Ref;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    fn CFRelease(object: Ref);
    fn CFDataCreateMutable(allocator: Ref, capacity: isize) -> Ref;
    fn CFDataGetLength(data: Ref) -> isize;
    fn CFDataGetBytePtr(data: Ref) -> *const u8;
    fn CFStringCreateWithCString(allocator: Ref, text: *const i8, encoding: u32) -> Ref;
    fn CFNumberCreate(allocator: Ref, kind: isize, value: *const c_void) -> Ref;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> Ref;
}

/// Releases a +1 Core Foundation reference when dropped.
struct Owned(Ref);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: we own exactly one reference to a live CF object.
            unsafe { CFRelease(self.0) };
        }
    }
}

/// `window_frame` is the window's size in points, `region` a part of it.
pub(crate) fn window(
    window_id: u32,
    window_frame: Rect,
    region: Option<Rect>,
    max_width: u32,
) -> Result<Image> {
    // SAFETY: no preconditions.
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err(Error::Permission(
            "see needs Screen Recording access. Turn it on for the app running Hush in System Settings > \
             Privacy & Security > Screen & System Audio Recording."
                .into(),
        ));
    }
    let image = Owned(private::capture_window(window_id).ok_or_else(|| {
        Error::Failed(
            "the window server returned no image for this window (is it minimized?)".into(),
        )
    })?);
    // SAFETY: `image` is a live CGImage for the rest of this function.
    unsafe {
        let (full_w, full_h) = (CGImageGetWidth(image.0), CGImageGetHeight(image.0));
        let scale = full_w as f64 / window_frame.w.max(1.0);
        let cropped = match region {
            Some(r) => {
                let crop = CGRect {
                    x: (r.x * scale).max(0.0),
                    y: (r.y * scale).max(0.0),
                    w: (r.w * scale).min(full_w as f64),
                    h: (r.h * scale).min(full_h as f64),
                };
                let cropped = CGImageCreateWithImageInRect(image.0, crop);
                if cropped.is_null() {
                    return Err(Error::Failed(
                        "that part of the window is outside the picture".into(),
                    ));
                }
                Some(Owned(cropped))
            }
            None => None,
        };
        let source = cropped.as_ref().map_or(image.0, |c| c.0);
        let scaled = scale_to(
            source,
            CGImageGetWidth(source),
            CGImageGetHeight(source),
            max_width as usize,
        )?;
        let out = Image {
            jpeg: encode(scaled.0)?,
            width: CGImageGetWidth(scaled.0) as u32,
            height: CGImageGetHeight(scaled.0) as u32,
        };
        Ok(out)
    }
}

/// Runs `read` on the window's picture in gray, resized to `width` x
/// `height` pixels and with its contrast raised: what reads best to text
/// recognition. On FilmCraft at twice its size, 25 of 36 known labels came
/// out exact this way, 24 in color, 16 at its own size.
pub(crate) fn with_picture<T>(
    window_id: u32,
    width: usize,
    height: usize,
    read: impl FnOnce(&objc2_core_graphics::CGImage) -> T,
) -> Result<T> {
    // SAFETY: no preconditions.
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err(Error::Permission(
            "reading this window's pixels needs Screen Recording access for the app running Hush"
                .into(),
        ));
    }
    let image = Owned(
        private::capture_window(window_id)
            .ok_or_else(|| Error::Failed("the window server returned no image".into()))?,
    );
    // SAFETY: `image` is a live CGImage; the gray copy lives until the end.
    unsafe {
        let gray = gray_with_contrast(image.0, width, height)?;
        Ok(read(&*gray.0.cast::<objc2_core_graphics::CGImage>()))
    }
}

/// Contrast factor around mid-gray (1.4 measured best of 1.0 to 2.0).
const CONTRAST: f32 = 1.4;

unsafe fn gray_with_contrast(image: Ref, width: usize, height: usize) -> Result<Owned> {
    // SAFETY: an 8-bit gray context CG allocates; its pixels are edited in
    // place before an image is made from them.
    unsafe {
        let space = Owned(CGColorSpaceCreateDeviceGray());
        let context = Owned(CGBitmapContextCreate(
            ptr::null_mut(),
            width,
            height,
            8,
            width,
            space.0,
            0,
        ));
        if context.0.is_null() {
            return Err(Error::Failed("couldn't create a drawing context".into()));
        }
        CGContextSetInterpolationQuality(context.0, 4);
        let rect = CGRect {
            x: 0.0,
            y: 0.0,
            w: width as f64,
            h: height as f64,
        };
        CGContextDrawImage(context.0, rect, image);
        let pixels = CGBitmapContextGetData(context.0).cast::<u8>();
        if !pixels.is_null() {
            for p in std::slice::from_raw_parts_mut(pixels, width * height) {
                *p = ((f32::from(*p) - 128.0) * CONTRAST + 128.0).clamp(0.0, 255.0) as u8;
            }
        }
        let gray = CGBitmapContextCreateImage(context.0);
        if gray.is_null() {
            return Err(Error::Failed("couldn't make the gray picture".into()));
        }
        Ok(Owned(gray))
    }
}

/// A copy at most `max_width` wide (the same image, retained, if it fits).
unsafe fn scale_to(image: Ref, width: usize, height: usize, max_width: usize) -> Result<Owned> {
    let target_w = width.min(max_width.max(1));
    let target_h = ((height as f64) * target_w as f64 / width.max(1) as f64)
        .round()
        .max(1.0) as usize;
    // SAFETY: forwarded from the caller.
    unsafe { resize(image, target_w, target_h) }
}

unsafe fn resize(image: Ref, target_w: usize, target_h: usize) -> Result<Owned> {
    // SAFETY: standard 8-bit RGBA context; CG allocates the pixels.
    unsafe {
        let space = Owned(CGColorSpaceCreateDeviceRGB());
        // kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big
        let context = Owned(CGBitmapContextCreate(
            ptr::null_mut(),
            target_w,
            target_h,
            8,
            0,
            space.0,
            1 | (4 << 12),
        ));
        if context.0.is_null() {
            return Err(Error::Failed("couldn't create a drawing context".into()));
        }
        CGContextSetInterpolationQuality(context.0, 4); // kCGInterpolationHigh
        let rect = CGRect {
            x: 0.0,
            y: 0.0,
            w: target_w as f64,
            h: target_h as f64,
        };
        CGContextDrawImage(context.0, rect, image);
        let scaled = CGBitmapContextCreateImage(context.0);
        if scaled.is_null() {
            return Err(Error::Failed("couldn't scale the picture".into()));
        }
        Ok(Owned(scaled))
    }
}

unsafe fn encode(image: Ref) -> Result<Vec<u8>> {
    // SAFETY: each object is created here and released by `Owned`.
    unsafe {
        let quality_value: f64 = QUALITY;
        let data = Owned(CFDataCreateMutable(kCFAllocatorDefault, 0));
        let kind = Owned(CFStringCreateWithCString(
            kCFAllocatorDefault,
            c"public.jpeg".as_ptr(),
            0x0800_0100,
        ));
        let quality = Owned(CFNumberCreate(
            kCFAllocatorDefault,
            13,
            (&raw const quality_value).cast(),
        ));
        let keys = [kCGImageDestinationLossyCompressionQuality];
        let values = [quality.0];
        let options = Owned(CFDictionaryCreate(
            kCFAllocatorDefault,
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        ));
        let destination = Owned(CGImageDestinationCreateWithData(
            data.0,
            kind.0,
            1,
            ptr::null_mut(),
        ));
        if destination.0.is_null() {
            return Err(Error::Failed("couldn't create a JPEG encoder".into()));
        }
        CGImageDestinationAddImage(destination.0, image, options.0);
        if !CGImageDestinationFinalize(destination.0) {
            return Err(Error::Failed("couldn't encode the picture".into()));
        }
        let length = CFDataGetLength(data.0) as usize;
        Ok(std::slice::from_raw_parts(CFDataGetBytePtr(data.0), length).to_vec())
    }
}
