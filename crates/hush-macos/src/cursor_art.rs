//! Draws the agent cursor of a recording into one frame (see
//! `hush_core::cursor` for where it is and how it moves).
//!
//! A rounded arrow in the agent's color with a white outline and a soft
//! shadow, a name label below and to the right of it, and a ring that spreads
//! from the tip on each click. The label stays upright while the arrow
//! leans.

use std::ffi::c_void;
use std::time::Duration;

use hush_core::cursor::{Gesture, Mark, Pose};

type Ref = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    w: f64,
    h: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}

/// Agent colors, picked by the label so one agent keeps its color.
const PALETTE: [(f64, f64, f64); 6] = [
    (0.486, 0.361, 1.0),   // violet
    (0.024, 0.714, 0.831), // cyan
    (0.063, 0.725, 0.506), // emerald
    (0.961, 0.620, 0.043), // amber
    (0.925, 0.282, 0.600), // pink
    (0.231, 0.510, 0.965), // blue
];

/// The label's underline gradient: cyan, violet, pink.
const UNDERLINE: [f64; 12] = [
    0.133, 0.827, 0.933, 1.0, 0.655, 0.545, 0.980, 1.0, 0.957, 0.447, 0.714, 1.0,
];

const RIPPLE: Duration = Duration::from_millis(450);

/// The color for `label`.
pub(crate) fn color(label: &str) -> (f64, f64, f64) {
    let hash = label
        .bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    PALETTE[hash as usize % PALETTE.len()]
}

/// Draws the cursor at `pose` into `context`, whose coordinates are window
/// points with y pointing down; `width` is the window's, so the label can
/// move to the cursor's left near the right edge.
///
/// # Safety
/// `context` must be a live CGContext set up that way.
pub(crate) unsafe fn draw(
    context: Ref,
    pose: Pose,
    marks: &[Mark],
    t: Duration,
    label: &str,
    tint: (f64, f64, f64),
    width: f64,
) {
    // SAFETY: every call draws into the caller's live context; paths, colors
    // and text objects are created and released here.
    unsafe {
        CGContextSaveGState(context);
        CGContextSetAlpha(context, pose.opacity);
        ripples(context, marks, t, tint);
        arrow(context, pose, tint);
        if !label.is_empty() {
            name_tag(context, pose, label, tint, width);
        }
        CGContextRestoreGState(context);
    }
}

unsafe fn ripples(context: Ref, marks: &[Mark], t: Duration, tint: (f64, f64, f64)) {
    for mark in marks {
        if mark.gesture != Gesture::Click || t < mark.at || t > mark.at + RIPPLE {
            continue;
        }
        let u = (t - mark.at).as_secs_f64() / RIPPLE.as_secs_f64();
        let eased = 1.0 - (1.0 - u).powi(3);
        let radius = 4.0 + 20.0 * eased;
        // SAFETY: as in `draw`.
        unsafe {
            CGContextSetRGBStrokeColor(context, tint.0, tint.1, tint.2, 0.55 * (1.0 - u));
            CGContextSetLineWidth(context, 2.0);
            CGContextStrokeEllipseInRect(
                context,
                Rect {
                    origin: Point {
                        x: mark.x - radius,
                        y: mark.y - radius,
                    },
                    size: Size {
                        w: radius * 2.0,
                        h: radius * 2.0,
                    },
                },
            );
        }
    }
}

/// The arrow, tip at the origin, pointing up and to the left: the classic
/// pointer with a stem, every corner rounded. The brand art draws the same
/// outline, so recordings and the icon match.
unsafe fn arrow(context: Ref, pose: Pose, tint: (f64, f64, f64)) {
    let corners = [
        (0.0, 0.0, 1.3),
        (0.0, 16.6, 1.5),
        (4.1, 12.9, 0.7),
        (6.7, 18.9, 1.0),
        (9.3, 17.8, 1.0),
        (6.8, 11.9, 0.7),
        (12.3, 11.9, 1.5),
    ];
    // SAFETY: as in `draw`.
    unsafe {
        CGContextSaveGState(context);
        CGContextTranslateCTM(context, pose.x, pose.y);
        CGContextRotateCTM(context, pose.lean.to_radians());
        CGContextScaleCTM(context, pose.scale, pose.scale);
        let path = CGPathCreateMutable();
        let n = corners.len();
        let mid = |a: (f64, f64, f64), b: (f64, f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let start = mid(corners[n - 1], corners[0]);
        CGPathMoveToPoint(path, std::ptr::null(), start.0, start.1);
        for i in 0..n {
            let corner = corners[i];
            let next = mid(corner, corners[(i + 1) % n]);
            CGPathAddArcToPoint(
                path,
                std::ptr::null(),
                corner.0,
                corner.1,
                next.0,
                next.1,
                corner.2,
            );
        }
        CGPathCloseSubpath(path);
        let shadow = CGColorCreateSRGB(0.0, 0.0, 0.0, 0.32);
        CGContextSetShadowWithColor(context, Size { w: 0.0, h: 2.0 }, 5.0, shadow);
        CGContextAddPath(context, path);
        CGContextSetRGBFillColor(context, tint.0, tint.1, tint.2, 1.0);
        CGContextSetRGBStrokeColor(context, 1.0, 1.0, 1.0, 1.0);
        CGContextSetLineWidth(context, 1.6);
        CGContextSetLineJoin(context, 1);
        // kCGPathFillStroke
        CGContextDrawPath(context, 3);
        CGColorRelease(shadow);
        CGPathRelease(path);
        CGContextRestoreGState(context);
    }
}

unsafe fn name_tag(context: Ref, pose: Pose, label: &str, tint: (f64, f64, f64), window: f64) {
    // SAFETY: as in `draw`.
    unsafe {
        let font = CTFontCreateUIFontForLanguage(3, 12.0, std::ptr::null_mut());
        let text = cfstring(label);
        let keys = [
            kCTFontAttributeName,
            kCTForegroundColorFromContextAttributeName,
        ];
        let values = [font, kCFBooleanTrue];
        let attributes = CFDictionaryCreate(
            std::ptr::null_mut(),
            keys.as_ptr(),
            values.as_ptr(),
            2,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        );
        let string = CFAttributedStringCreate(std::ptr::null_mut(), text, attributes);
        let line = CTLineCreateWithAttributedString(string);
        let (mut ascent, mut descent, mut leading) = (0.0, 0.0, 0.0);
        let width = CTLineGetTypographicBounds(line, &mut ascent, &mut descent, &mut leading);
        let (pad_x, pad_y) = (9.0, 5.0);
        let tag_width = width + pad_x * 2.0;
        // Right of the arrow, or left of it when that would leave the window.
        let x = if pose.x + 14.0 + tag_width > window - 4.0 {
            pose.x - 6.0 - tag_width
        } else {
            pose.x + 14.0
        };
        let frame = Rect {
            origin: Point {
                x,
                y: pose.y + 21.0,
            },
            size: Size {
                w: width + pad_x * 2.0,
                h: ascent + descent + pad_y * 2.0,
            },
        };
        let pill = CGPathCreateWithRoundedRect(frame, 9.0, 9.0, std::ptr::null());

        // A soft glow in the agent's color, then the dark pill.
        CGContextSaveGState(context);
        let glow = CGColorCreateSRGB(tint.0, tint.1, tint.2, 0.45);
        CGContextSetShadowWithColor(context, Size { w: 0.0, h: 6.0 }, 14.0, glow);
        CGContextAddPath(context, pill);
        CGContextSetRGBFillColor(context, 0.08, 0.08, 0.09, 0.94);
        CGContextFillPath(context);
        CGColorRelease(glow);
        CGContextRestoreGState(context);

        // The underline: a 2-point gradient along the pill's bottom edge.
        CGContextSaveGState(context);
        CGContextAddPath(context, pill);
        CGContextClip(context);
        let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        let locations = [0.0, 0.55, 1.0];
        let gradient =
            CGGradientCreateWithColorComponents(space, UNDERLINE.as_ptr(), locations.as_ptr(), 3);
        let bottom = frame.origin.y + frame.size.h;
        CGContextClipToRect(
            context,
            Rect {
                origin: Point {
                    x: frame.origin.x,
                    y: bottom - 2.0,
                },
                size: Size {
                    w: frame.size.w,
                    h: 2.0,
                },
            },
        );
        CGContextDrawLinearGradient(
            context,
            gradient,
            Point {
                x: frame.origin.x,
                y: bottom,
            },
            Point {
                x: frame.origin.x + frame.size.w,
                y: bottom,
            },
            0,
        );
        CGGradientRelease(gradient);
        CGColorSpaceRelease(space);
        CGContextRestoreGState(context);

        // The name, upright in this y-down space.
        CGContextSaveGState(context);
        CGContextSetRGBFillColor(context, 1.0, 1.0, 1.0, 1.0);
        CGContextSetTextMatrix(
            context,
            Affine {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: -1.0,
                tx: 0.0,
                ty: 0.0,
            },
        );
        CGContextSetTextPosition(
            context,
            frame.origin.x + pad_x,
            frame.origin.y + pad_y + ascent,
        );
        CTLineDraw(line, context);
        CGContextRestoreGState(context);

        CGPathRelease(pill);
        CFRelease(line);
        CFRelease(string);
        CFRelease(attributes);
        CFRelease(text);
        CFRelease(font);
    }
}

unsafe fn cfstring(text: &str) -> Ref {
    let c = std::ffi::CString::new(text.replace('\0', "")).unwrap_or_default();
    // SAFETY: a valid C string; kCFStringEncodingUTF8.
    unsafe { CFStringCreateWithCString(std::ptr::null_mut(), c.as_ptr(), 0x0800_0100) }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static kCGColorSpaceSRGB: Ref;
    fn CGContextSaveGState(c: Ref);
    fn CGContextRestoreGState(c: Ref);
    fn CGContextSetAlpha(c: Ref, alpha: f64);
    fn CGContextTranslateCTM(c: Ref, x: f64, y: f64);
    fn CGContextRotateCTM(c: Ref, angle: f64);
    fn CGContextScaleCTM(c: Ref, x: f64, y: f64);
    fn CGContextAddPath(c: Ref, path: Ref);
    fn CGContextDrawPath(c: Ref, mode: i32);
    fn CGContextFillPath(c: Ref);
    fn CGContextClip(c: Ref);
    fn CGContextClipToRect(c: Ref, rect: Rect);
    fn CGContextSetRGBFillColor(c: Ref, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetRGBStrokeColor(c: Ref, r: f64, g: f64, b: f64, a: f64);
    fn CGContextSetLineWidth(c: Ref, width: f64);
    fn CGContextSetLineJoin(c: Ref, join: i32);
    fn CGContextStrokeEllipseInRect(c: Ref, rect: Rect);
    fn CGContextSetShadowWithColor(c: Ref, offset: Size, blur: f64, color: Ref);
    fn CGContextSetTextMatrix(c: Ref, t: Affine);
    fn CGContextSetTextPosition(c: Ref, x: f64, y: f64);
    fn CGContextDrawLinearGradient(c: Ref, g: Ref, start: Point, end: Point, options: u32);
    fn CGColorCreateSRGB(r: f64, g: f64, b: f64, a: f64) -> Ref;
    fn CGColorRelease(color: Ref);
    fn CGColorSpaceCreateWithName(name: Ref) -> Ref;
    fn CGColorSpaceRelease(space: Ref);
    fn CGGradientCreateWithColorComponents(
        space: Ref,
        components: *const f64,
        locations: *const f64,
        count: usize,
    ) -> Ref;
    fn CGGradientRelease(gradient: Ref);
    fn CGPathCreateMutable() -> Ref;
    fn CGPathCreateWithRoundedRect(rect: Rect, w: f64, h: f64, t: *const Affine) -> Ref;
    fn CGPathMoveToPoint(path: Ref, t: *const Affine, x: f64, y: f64);
    fn CGPathAddArcToPoint(
        path: Ref,
        t: *const Affine,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        radius: f64,
    );
    fn CGPathCloseSubpath(path: Ref);
    fn CGPathRelease(path: Ref);
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    static kCTFontAttributeName: Ref;
    static kCTForegroundColorFromContextAttributeName: Ref;
    fn CTFontCreateUIFontForLanguage(kind: u32, size: f64, language: Ref) -> Ref;
    fn CTLineCreateWithAttributedString(string: Ref) -> Ref;
    fn CTLineGetTypographicBounds(
        line: Ref,
        ascent: *mut f64,
        descent: *mut f64,
        leading: *mut f64,
    ) -> f64;
    fn CTLineDraw(line: Ref, context: Ref);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: Ref;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    fn CFRelease(object: Ref);
    fn CFStringCreateWithCString(allocator: Ref, text: *const i8, encoding: u32) -> Ref;
    fn CFAttributedStringCreate(allocator: Ref, text: Ref, attributes: Ref) -> Ref;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> Ref;
}
