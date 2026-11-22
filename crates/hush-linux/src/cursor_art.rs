//! Draws the agent cursor of a recording into one frame, the same art as on
//! macOS (see `hush_core::cursor` for where it is and how it moves): a
//! rounded arrow in the agent's color with a white outline and a soft
//! shadow, a ring that spreads from the tip on each click, and a name tag.

use std::sync::OnceLock;
use std::time::Duration;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use hush_core::cursor::{Gesture, Mark, Pose};
use tiny_skia::{
    Color, FillRule, GradientStop, LineJoin, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    SpreadMode, Stroke, Transform,
};

/// Agent colors, picked by the label so one agent keeps its color.
const PALETTE: [(f32, f32, f32); 6] = [
    (0.486, 0.361, 1.0),   // violet
    (0.024, 0.714, 0.831), // cyan
    (0.063, 0.725, 0.506), // emerald
    (0.961, 0.620, 0.043), // amber
    (0.925, 0.282, 0.600), // pink
    (0.231, 0.510, 0.965), // blue
];

/// The tag's underline: cyan, violet, pink.
const UNDERLINE: [(f32, f32, f32); 3] = [
    (0.133, 0.827, 0.933),
    (0.655, 0.545, 0.980),
    (0.957, 0.447, 0.714),
];

const RIPPLE: Duration = Duration::from_millis(450);
const TEXT_SIZE: f32 = 12.0;

/// The arrow's corners, tip at the origin, with each corner's radius. The
/// macOS cursor and the brand art use the same outline.
const ARROW: [(f32, f32, f32); 7] = [
    (0.0, 0.0, 1.3),
    (0.0, 16.6, 1.5),
    (4.1, 12.9, 0.7),
    (6.7, 18.9, 1.0),
    (9.3, 17.8, 1.0),
    (6.8, 11.9, 0.7),
    (12.3, 11.9, 1.5),
];

pub(crate) type Tint = (f32, f32, f32);

/// The color for `label`, the same one macOS picks.
pub(crate) fn color(label: &str) -> Tint {
    let hash = label
        .bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    PALETTE[hash as usize % PALETTE.len()]
}

fn rgba(c: Tint, alpha: f32) -> Color {
    Color::from_rgba(c.0, c.1, c.2, alpha.clamp(0.0, 1.0)).unwrap_or(Color::BLACK)
}

fn paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

/// Draws the cursor at `pose` (window pixels, y down) into `frame`.
pub(crate) fn draw(
    frame: &mut Pixmap,
    pose: Pose,
    marks: &[Mark],
    t: Duration,
    label: &str,
    tint: Tint,
) {
    let opacity = pose.opacity as f32;
    ripples(frame, marks, t, tint, opacity);
    arrow(frame, pose, tint, opacity);
    if !label.is_empty() {
        name_tag(frame, pose, label, tint, opacity);
    }
}

fn ripples(frame: &mut Pixmap, marks: &[Mark], t: Duration, tint: Tint, opacity: f32) {
    for mark in marks {
        if mark.gesture != Gesture::Click || t < mark.at || t > mark.at + RIPPLE {
            continue;
        }
        let u = ((t - mark.at).as_secs_f64() / RIPPLE.as_secs_f64()) as f32;
        let eased = 1.0 - (1.0 - u).powi(3);
        let radius = 4.0 + 20.0 * eased;
        let Some(circle) = PathBuilder::from_circle(mark.x as f32, mark.y as f32, radius) else {
            continue;
        };
        let stroke = Stroke {
            width: 2.0,
            ..Stroke::default()
        };
        let paint = paint(rgba(tint, 0.55 * (1.0 - u) * opacity));
        frame.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
    }
}

/// A closed outline through `corners`, each rounded by its radius (a
/// quadratic curve through the corner, which at these sizes looks the same
/// as the circular arc macOS draws).
fn rounded(corners: &[(f32, f32, f32)]) -> Option<tiny_skia::Path> {
    let n = corners.len();
    let toward = |from: (f32, f32, f32), to: (f32, f32, f32), by: f32| {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
        let k = (by / length).min(0.5);
        (from.0 + dx * k, from.1 + dy * k)
    };
    let mut path = PathBuilder::new();
    for i in 0..n {
        let corner = corners[i];
        let (before, after) = (corners[(i + n - 1) % n], corners[(i + 1) % n]);
        let (a, b) = (
            toward(corner, before, corner.2),
            toward(corner, after, corner.2),
        );
        if i == 0 {
            path.move_to(a.0, a.1);
        } else {
            path.line_to(a.0, a.1);
        }
        path.quad_to(corner.0, corner.1, b.0, b.1);
    }
    path.close();
    path.finish()
}

/// A soft shadow: the shape stroked wide and faint a few times. Cheaper
/// than a blur and as good at cursor size.
fn shadow(frame: &mut Pixmap, path: &tiny_skia::Path, tint: Tint, alpha: f32, at: Transform) {
    for width in [9.0, 6.0, 3.0] {
        let stroke = Stroke {
            width,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        frame.stroke_path(path, &paint(rgba(tint, alpha)), &stroke, at, None);
    }
}

fn arrow(frame: &mut Pixmap, pose: Pose, tint: Tint, opacity: f32) {
    let Some(path) = rounded(&ARROW) else { return };
    let at = Transform::from_translate(pose.x as f32, pose.y as f32)
        .pre_rotate(pose.lean as f32)
        .pre_scale(pose.scale as f32, pose.scale as f32);
    shadow(
        frame,
        &path,
        (0.0, 0.0, 0.0),
        0.07 * opacity,
        at.pre_translate(0.0, 2.0),
    );
    frame.fill_path(
        &path,
        &paint(rgba(tint, opacity)),
        FillRule::Winding,
        at,
        None,
    );
    let stroke = Stroke {
        width: 1.6,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    frame.stroke_path(
        &path,
        &paint(rgba((1.0, 1.0, 1.0), opacity)),
        &stroke,
        at,
        None,
    );
}

/// The system's sans-serif font, if it has one (`fc-match` knows where).
fn font() -> Option<&'static FontVec> {
    static FONT: OnceLock<Option<FontVec>> = OnceLock::new();
    FONT.get_or_init(|| {
        let found = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", "sans-serif:style=Medium"])
            .output()
            .ok()?;
        let path = String::from_utf8(found.stdout).ok()?;
        FontVec::try_from_vec(std::fs::read(path.trim()).ok()?).ok()
    })
    .as_ref()
}

fn name_tag(frame: &mut Pixmap, pose: Pose, label: &str, tint: Tint, opacity: f32) {
    // Without a font there is no name to draw; the cursor still shows.
    let Some(font) = font() else { return };
    let scaled = font.as_scaled(PxScale::from(TEXT_SIZE * 1.3));
    let glyphs: Vec<_> = label.chars().map(|c| scaled.glyph_id(c)).collect();
    let width: f32 = glyphs.iter().map(|g| scaled.h_advance(*g)).sum::<f32>()
        + glyphs
            .windows(2)
            .map(|w| scaled.kern(w[0], w[1]))
            .sum::<f32>();
    let (ascent, descent) = (scaled.ascent(), -scaled.descent());
    let (pad_x, pad_y) = (9.0, 5.0);
    let (w, h) = (width + pad_x * 2.0, ascent + descent + pad_y * 2.0);
    // Right of the arrow, or left of it when that would leave the frame.
    let (px, py) = (pose.x as f32, pose.y as f32);
    let x = if px + 14.0 + w > frame.width() as f32 - 4.0 {
        px - 6.0 - w
    } else {
        px + 14.0
    };
    let y = py + 21.0;
    let r = 9.0f32.min(h / 2.0);
    let pill = [(x, y, r), (x + w, y, r), (x + w, y + h, r), (x, y + h, r)];
    let Some(path) = rounded(&pill) else { return };
    let at = Transform::identity();
    // A soft glow in the agent's color, then the dark pill.
    shadow(
        frame,
        &path,
        tint,
        0.12 * opacity,
        at.pre_translate(0.0, 4.0),
    );
    frame.fill_path(
        &path,
        &paint(rgba((0.08, 0.08, 0.09), 0.94 * opacity)),
        FillRule::Winding,
        at,
        None,
    );
    // The underline along the pill's bottom edge.
    let stops = UNDERLINE
        .iter()
        .zip([0.0, 0.55, 1.0])
        .map(|(c, at)| GradientStop::new(at, rgba(*c, opacity)))
        .collect();
    if let (Some(shader), Some(clip)) = (
        LinearGradient::new(
            Point::from_xy(x, 0.0),
            Point::from_xy(x + w, 0.0),
            stops,
            SpreadMode::Pad,
            at,
        ),
        tiny_skia::Rect::from_xywh(x + r / 2.0, y + h - 2.0, w - r, 2.0),
    ) {
        let underline = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        frame.fill_rect(clip, &underline, at, None);
    }
    // The name, white.
    let mut caret = x + pad_x;
    let baseline = y + pad_y + ascent;
    let mut previous = None;
    for glyph_id in glyphs {
        if let Some(previous) = previous {
            caret += scaled.kern(previous, glyph_id);
        }
        previous = Some(glyph_id);
        let glyph =
            glyph_id.with_scale_and_position(scaled.scale(), ab_glyph::point(caret, baseline));
        caret += scaled.h_advance(glyph_id);
        let Some(outline) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outline.px_bounds();
        outline.draw(|gx, gy, coverage| {
            let fx = bounds.min.x as i64 + i64::from(gx);
            let fy = bounds.min.y as i64 + i64::from(gy);
            blend_white(frame, fx, fy, coverage * opacity);
        });
    }
}

/// Mixes white over one pixel of an opaque frame.
fn blend_white(frame: &mut Pixmap, x: i64, y: i64, alpha: f32) {
    let (w, h) = (i64::from(frame.width()), i64::from(frame.height()));
    if x < 0 || y < 0 || x >= w || y >= h || alpha <= 0.0 {
        return;
    }
    let pixel = &mut frame.pixels_mut()[(y * w + x) as usize];
    let mix = |c: u8| (f32::from(c) + (255.0 - f32::from(c)) * alpha.min(1.0)).round() as u8;
    if let Some(mixed) = tiny_skia::PremultipliedColorU8::from_rgba(
        mix(pixel.red()),
        mix(pixel.green()),
        mix(pixel.blue()),
        255,
    ) {
        *pixel = mixed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_keeps_its_color() {
        assert_eq!(color("Claude"), color("Claude"));
        assert!(PALETTE.contains(&color("anything")));
    }

    #[test]
    fn the_arrow_is_drawn_at_its_tip() {
        let mut frame = Pixmap::new(60, 60).unwrap();
        frame.fill(Color::BLACK);
        let pose = Pose {
            x: 20.0,
            y: 20.0,
            lean: 0.0,
            scale: 1.0,
            opacity: 1.0,
        };
        draw(&mut frame, pose, &[], Duration::ZERO, "", color("x"));
        // Inside the arrow's body: the tint, not black.
        let inside = frame.pixel(23, 30).unwrap();
        assert!(inside.red() > 40 || inside.green() > 40 || inside.blue() > 40);
        // Far from it: untouched.
        assert_eq!(frame.pixel(55, 5).unwrap().red(), 0);
    }
}
