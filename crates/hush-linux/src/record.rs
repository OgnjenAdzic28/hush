//! Recording a window on the lane to a movie, with the agent cursor drawn
//! in afterwards.
//!
//! While recording, a thread grabs the window from Hush's display about 30
//! times a second (the same grab as `see`, so the user's screen is never
//! touched) and keeps each frame as a JPEG on disk. On stop, every frame
//! gets the cursor for its moment (the cursor glides toward actions that
//! hadn't happened yet when the frame was taken, so it can only be drawn
//! afterwards) and goes to ffmpeg, which writes an H.264 movie.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hush_core::cursor::{self, Mark};
use hush_core::{Error, Result};
use tiny_skia::{Color, Pixmap};
use x11rb::protocol::xproto::Window;

use crate::cursor_art;
use crate::screen::{self, Screen};

/// Frames a second. A grab of a 1080p window takes about 10 ms on Xvfb.
const FPS: f64 = 30.0;
/// JPEG quality of the frames kept until stop.
const QUALITY: u8 = 92;
/// What shows where a frame is smaller than the first one.
const BACKDROP: [u8; 3] = [11, 11, 14];
/// A recording nobody stops ends here, so its frames can't fill the disk.
const LONGEST: Duration = Duration::from_secs(600);

struct Live {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Shots>,
}

struct Shots {
    dir: PathBuf,
    frames: Vec<(Duration, PathBuf)>,
    size: (usize, usize),
}

static LIVE: Mutex<Option<HashMap<u64, Live>>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn failed(what: impl std::fmt::Display) -> Error {
    Error::Failed(format!("recording: {what}"))
}

fn grab(screen: &Screen, window: Window, size: (u32, u32)) -> Result<(Vec<u8>, usize, usize)> {
    let (rgb, w, h) = screen.grab(window, size, None).map_err(failed)?;
    Ok((screen::encode(&rgb, w, h, QUALITY).map_err(failed)?, w, h))
}

/// Starts recording `window` (`size` pixels) on the lane. Fails at once
/// when it can't be grabbed, or when ffmpeg, which writes the movie, is
/// missing.
pub(crate) fn start(screen: Arc<Screen>, window: Window, size: (u32, u32)) -> Result<u64> {
    if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .status()
        .is_err()
    {
        return Err(Error::Unsupported(
            "recording on Linux needs ffmpeg: install it (sudo apt install ffmpeg, or dnf install ffmpeg)".into(),
        ));
    }
    let started = Instant::now();
    let (first, width, height) = grab(&screen, window, size)?;
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("hush-recording-{}-{id}", std::process::id()));
    // New and private: the temp folder is shared with other users, who
    // must neither read the frames nor plant a folder or link there first.
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .map_err(failed)?;
    let path = dir.join("0.jpg");
    std::fs::write(&path, first).map_err(failed)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    let thread = thread::Builder::new()
        .name("hush-recording".into())
        .spawn(move || {
            let mut shots = Shots {
                dir,
                frames: vec![(Duration::ZERO, path)],
                size: (width, height),
            };
            let mut n = 1u64;
            while !stopping.load(Ordering::SeqCst) && started.elapsed() < LONGEST {
                let due = started + Duration::from_secs_f64(n as f64 / FPS);
                n += 1;
                let now = Instant::now();
                if due > now {
                    // Paces the capture loop; a frame is due then.
                    thread::sleep(due - now);
                }
                let at = started.elapsed();
                if let Ok((jpeg, _, _)) = grab(&screen, window, size) {
                    let path = shots.dir.join(format!("{}.jpg", shots.frames.len()));
                    if std::fs::write(&path, jpeg).is_ok() {
                        shots.frames.push((at, path));
                    }
                }
            }
            shots
        })
        .map_err(failed)?;
    LIVE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(id, Live { stop, thread });
    Ok(id)
}

fn take(id: u64) -> Option<Live> {
    LIVE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .and_then(|live| live.remove(&id))
}

/// Stops the recording and deletes its frames: its session ended without
/// stopping it.
pub(crate) fn cancel(id: u64) {
    if let Some(live) = take(id) {
        live.stop.store(true, Ordering::SeqCst);
        if let Ok(shots) = live.thread.join() {
            let _ = std::fs::remove_dir_all(&shots.dir);
        }
    }
}

/// Stops the recording and writes the movie to `out`, with the cursor drawn
/// from `marks` and labelled `label` (no label when empty).
pub(crate) fn stop(id: u64, marks: &[Mark], label: &str, out: &Path) -> Result<String> {
    let live = take(id).ok_or_else(|| Error::NotFound("no recording is running".into()))?;
    live.stop.store(true, Ordering::SeqCst);
    let shots = live
        .thread
        .join()
        .map_err(|_| failed("the recording thread panicked"))?;
    let written = write_movie(&shots, marks, label, out);
    let _ = std::fs::remove_dir_all(&shots.dir);
    let seconds = shots.frames.last().map_or(0.0, |(t, _)| t.as_secs_f64());
    written.map(|(w, h)| {
        format!(
            "wrote {} · {seconds:.1} s · {} frames at {w}x{h}",
            out.display(),
            shots.frames.len()
        )
    })
}

/// A kept frame, decoded and placed on a canvas of the movie's size.
fn canvas(path: &Path, width: usize, height: usize) -> Option<Pixmap> {
    let jpeg = std::fs::read(path).ok()?;
    let mut decoder = jpeg_decoder::Decoder::new(jpeg.as_slice());
    let rgb = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (fw, fh) = (usize::from(info.width), usize::from(info.height));
    let mut canvas = Pixmap::new(width as u32, height as u32)?;
    canvas.fill(Color::from_rgba8(
        BACKDROP[0],
        BACKDROP[1],
        BACKDROP[2],
        255,
    ));
    let data = canvas.data_mut();
    for y in 0..height.min(fh) {
        for x in 0..width.min(fw) {
            let (from, to) = ((y * fw + x) * 3, (y * width + x) * 4);
            data[to..to + 3].copy_from_slice(&rgb[from..from + 3]);
        }
    }
    Some(canvas)
}

fn write_movie(shots: &Shots, marks: &[Mark], label: &str, out: &Path) -> Result<(usize, usize)> {
    // H.264 wants even dimensions.
    let (width, height) = (shots.size.0 & !1, shots.size.1 & !1);
    let tint = cursor_art::color(label);
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
        ])
        .args([
            "-s",
            &format!("{width}x{height}"),
            "-framerate",
            &FPS.to_string(),
        ])
        .args([
            "-i", "-", "-c:v", "libx264", "-preset", "veryfast", "-crf", "18",
        ])
        .args(["-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(failed)?;
    let mut input = ffmpeg
        .stdin
        .take()
        .ok_or_else(|| failed("no pipe to ffmpeg"))?;
    // A steady 30 frames a second: each shows the latest window frame taken
    // by then, with the cursor drawn for its own moment, so the cursor moves
    // smoothly even when grabs came slower.
    let end = shots.frames.last().map_or(Duration::ZERO, |(t, _)| *t);
    let count = (end.as_secs_f64() * FPS).floor() as u64 + 1;
    let mut shown: Option<(usize, Pixmap)> = None;
    for n in 0..count {
        let t = Duration::from_secs_f64(n as f64 / FPS);
        let latest = shots
            .frames
            .partition_point(|(at, _)| *at <= t)
            .saturating_sub(1);
        if shown.as_ref().is_none_or(|(index, _)| *index != latest)
            && let Some(frame) = canvas(&shots.frames[latest].1, width, height)
        {
            shown = Some((latest, frame));
        }
        let Some((_, frame)) = &shown else { continue };
        let mut frame = frame.clone();
        if let Some(pose) = cursor::pose(marks, t) {
            cursor_art::draw(&mut frame, pose, marks, t, label, tint);
        }
        if input.write_all(frame.data()).is_err() {
            break;
        }
    }
    drop(input);
    let result = ffmpeg.wait_with_output().map_err(failed)?;
    if !result.status.success() {
        let reason = String::from_utf8_lossy(&result.stderr);
        return Err(failed(format!("ffmpeg failed: {}", reason.trim())));
    }
    Ok((width, height))
}
