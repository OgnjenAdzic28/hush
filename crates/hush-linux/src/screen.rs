//! The lane's X server: finding an app's window on it and capturing it.
//! Only ever connects to Hush's own display, never the user's.
//!
//! The lane has no window manager, so apps' windows pile up at its top left.
//! Capture reads each window's own pixels through the Composite extension,
//! which keeps every window drawn off screen, so a window under another
//! still pictures as itself.

use hush_core::{Image, Rect};
use x11rb::connection::Connection;
use x11rb::protocol::composite::{ConnectionExt as _, Redirect};
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, Drawable, ImageFormat, MapState, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::lane::Lane;

/// JPEG quality for `see`, the same as macOS: small, and legible to a model.
const QUALITY: u8 = 60;

pub(crate) struct Screen {
    connection: RustConnection,
    root: Window,
    net_wm_pid: u32,
}

impl std::fmt::Debug for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Screen").field("root", &self.root).finish()
    }
}

type X<T> = Result<T, String>;

fn x(error: impl std::fmt::Display) -> String {
    format!("Hush's display: {error}")
}

impl Screen {
    pub(crate) fn open(lane: Lane) -> X<Screen> {
        let (connection, screen) = x11rb::connect(Some(&lane.name())).map_err(x)?;
        let root = connection.setup().roots[screen].root;
        let net_wm_pid = connection
            .intern_atom(false, b"_NET_WM_PID")
            .map_err(x)?
            .reply()
            .map_err(x)?
            .atom;
        // Automatic redirection keeps showing windows as before; it lasts
        // while this connection does, and other hosts may ask for it too.
        connection
            .composite_query_version(0, 4)
            .map_err(x)?
            .reply()
            .map_err(x)?;
        connection
            .composite_redirect_subwindows(root, Redirect::AUTOMATIC)
            .map_err(x)?
            .check()
            .map_err(x)?;
        Ok(Screen {
            connection,
            root,
            net_wm_pid,
        })
    }

    /// Puts the accessibility bus's address on the root window, as the bus
    /// launcher does on an X desktop. Qt 5 reads it there before anything
    /// else; asking the session bus instead, it contacts the registry
    /// before it has connected and never registers.
    pub(crate) fn announce_bus(&self, address: &str) -> X<()> {
        let c = &self.connection;
        let atom = c
            .intern_atom(false, b"AT_SPI_BUS")
            .map_err(x)?
            .reply()
            .map_err(x)?
            .atom;
        c.change_property8(
            PropMode::REPLACE,
            self.root,
            atom,
            AtomEnum::STRING,
            address.as_bytes(),
        )
        .map_err(x)?
        .check()
        .map_err(x)
    }

    pub(crate) fn connection(&self) -> &RustConnection {
        &self.connection
    }

    pub(crate) fn root(&self) -> Window {
        self.root
    }

    /// The shown top-level window of `pid` nearest `frame` (the window's
    /// frame as AT-SPI reports it, in lane coordinates). The lane has no
    /// window manager, so top-level windows are the root's children.
    pub(crate) fn window_of(&self, pid: u32, frame: Rect) -> X<(Window, Rect)> {
        let children = self
            .connection
            .query_tree(self.root)
            .map_err(x)?
            .reply()
            .map_err(x)?
            .children;
        let mut best: Option<(f64, Window, Rect)> = None;
        for window in children {
            let Ok(Some(found)) = self.shown_frame_of(window, pid) else {
                continue;
            };
            let distance = (found.x - frame.x).abs()
                + (found.y - frame.y).abs()
                + (found.w - frame.w).abs()
                + (found.h - frame.h).abs();
            if best.is_none_or(|(d, _, _)| distance < d) {
                best = Some((distance, window, found));
            }
        }
        best.map(|(_, window, found)| (window, found))
            .ok_or_else(|| "the app has no window on Hush's display".to_owned())
    }

    /// Whether `pid` has a top-level window on this display. The proof an
    /// app runs here when its environment can't say: Chrome writes its
    /// process title over it.
    pub(crate) fn has_window_of(&self, pid: u32) -> bool {
        let c = &self.connection;
        let Ok(Ok(tree)) = c.query_tree(self.root).map(|r| r.reply()) else {
            return false;
        };
        let asked: Vec<_> = tree
            .children
            .iter()
            .filter_map(|w| {
                c.get_property(false, *w, self.net_wm_pid, AtomEnum::CARDINAL, 0, 1)
                    .ok()
            })
            .collect();
        asked.into_iter().any(|cookie| {
            cookie
                .reply()
                .is_ok_and(|owner| owner.value32().and_then(|mut v| v.next()) == Some(pid))
        })
    }

    /// The window's frame in root coordinates, if it is shown and is `pid`'s.
    fn shown_frame_of(&self, window: Window, pid: u32) -> X<Option<Rect>> {
        let c = &self.connection;
        let attributes = c.get_window_attributes(window).map_err(x)?;
        let owner = c
            .get_property(false, window, self.net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .map_err(x)?;
        let geometry = c.get_geometry(window).map_err(x)?;
        let at = c
            .translate_coordinates(window, self.root, 0, 0)
            .map_err(x)?;
        if attributes.reply().map_err(x)?.map_state != MapState::VIEWABLE {
            return Ok(None);
        }
        let owner = owner.reply().map_err(x)?;
        if owner.value32().and_then(|mut v| v.next()) != Some(pid) {
            return Ok(None);
        }
        let geometry = geometry.reply().map_err(x)?;
        let at = at.reply().map_err(x)?;
        Ok(Some(Rect {
            x: at.dst_x.into(),
            y: at.dst_y.into(),
            w: geometry.width.into(),
            h: geometry.height.into(),
        }))
    }

    /// A JPEG of `window`, or a window-relative region of it, at most
    /// `max_width` pixels wide.
    pub(crate) fn capture(
        &self,
        window: Window,
        size: (u32, u32),
        region: Option<Rect>,
        max_width: u32,
    ) -> X<Image> {
        let (rgb, w, h) = self.grab(window, size, region)?;
        let (rgb, w, h) = shrink(rgb, w, h, max_width as usize);
        Ok(Image {
            jpeg: encode(&rgb, w, h, QUALITY)?,
            width: w as u32,
            height: h as u32,
        })
    }

    /// The pixels of `window` (or a region of it) as RGB, with their size.
    pub(crate) fn grab(
        &self,
        window: Window,
        size: (u32, u32),
        region: Option<Rect>,
    ) -> X<(Vec<u8>, usize, usize)> {
        let full = Rect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f64,
            h: size.1 as f64,
        };
        let area = clip(region.unwrap_or(full), full)
            .ok_or_else(|| "that region is outside the window".to_owned())?;
        let (width, height) = (area.w as u16, area.h as u16);
        let c = &self.connection;
        let pixmap = c.generate_id().map_err(x)?;
        c.composite_name_window_pixmap(window, pixmap)
            .map_err(x)?
            .check()
            .map_err(x)?;
        let image = self.read(pixmap, area, width, height);
        // Freed whatever the read did; a failure here only leaks an id.
        let _ = c.free_pixmap(pixmap);
        let image = image?;
        let (w, h) = (usize::from(width), usize::from(height));
        if image.data.len() < w * h * 4 {
            return Err(x(format!(
                "unexpected pixel format (depth {}, {} bytes for {w}x{h})",
                image.depth,
                image.data.len()
            )));
        }
        Ok((bgrx_to_rgb(&image.data, w * h), w, h))
    }

    fn read(
        &self,
        drawable: Drawable,
        area: Rect,
        width: u16,
        height: u16,
    ) -> X<x11rb::protocol::xproto::GetImageReply> {
        self.connection
            .get_image(
                ImageFormat::Z_PIXMAP,
                drawable,
                area.x as i16,
                area.y as i16,
                width,
                height,
                !0,
            )
            .map_err(x)?
            .reply()
            .map_err(x)
    }
}

/// RGB pixels as a JPEG.
pub(crate) fn encode(rgb: &[u8], w: usize, h: usize, quality: u8) -> X<Vec<u8>> {
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, quality)
        .encode(rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(x)?;
    Ok(jpeg)
}

/// `region` cut to `bounds`, in whole pixels; `None` when nothing is left.
fn clip(region: Rect, bounds: Rect) -> Option<Rect> {
    let x0 = region.x.max(bounds.x).floor();
    let y0 = region.y.max(bounds.y).floor();
    let x1 = (region.x + region.w).min(bounds.x + bounds.w).floor();
    let y1 = (region.y + region.h).min(bounds.y + bounds.h).floor();
    (x1 > x0 && y1 > y0).then_some(Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// X servers on little-endian machines (Xvfb included) hand back 24-bit
/// color as 32-bit blue, green, red, unused.
fn bgrx_to_rgb(data: &[u8], pixels: usize) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(pixels * 3);
    for px in data.as_chunks::<4>().0.iter().take(pixels) {
        rgb.extend_from_slice(&[px[2], px[1], px[0]]);
    }
    rgb
}

/// Scales down to at most `max_width` wide, averaging the source pixels each
/// output pixel covers. Never scales up.
fn shrink(rgb: Vec<u8>, w: usize, h: usize, max_width: usize) -> (Vec<u8>, usize, usize) {
    if max_width == 0 || w <= max_width {
        return (rgb, w, h);
    }
    let nw = max_width;
    let nh = (h * nw / w).max(1);
    let mut out = vec![0u8; nw * nh * 3];
    for oy in 0..nh {
        let (y0, y1) = (oy * h / nh, ((oy + 1) * h / nh).max(oy * h / nh + 1));
        for ox in 0..nw {
            let (x0, x1) = (ox * w / nw, ((ox + 1) * w / nw).max(ox * w / nw + 1));
            let mut sum = [0u32; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) * 3;
                    for c in 0..3 {
                        sum[c] += u32::from(rgb[i + c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (oy * nw + ox) * 3;
            for c in 0..3 {
                out[o + c] = (sum[c] / n) as u8;
            }
        }
    }
    (out, nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_averages_and_keeps_the_aspect() {
        // 4x2, left half black, right half white, shrunk to 2 wide.
        let mut rgb = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                rgb.extend_from_slice(&[v, v, v]);
            }
        }
        let (out, w, h) = shrink(rgb, 4, 2, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, vec![0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn regions_are_clipped_to_the_window() {
        let window = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let inside = clip(
            Rect {
                x: 90.0,
                y: 40.0,
                w: 30.0,
                h: 30.0,
            },
            window,
        )
        .unwrap();
        assert_eq!((inside.w, inside.h), (10.0, 10.0));
        assert!(
            clip(
                Rect {
                    x: 200.0,
                    y: 0.0,
                    w: 5.0,
                    h: 5.0
                },
                window
            )
            .is_none()
        );
    }
}
