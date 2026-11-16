//! Recording a window to a movie, with the agent cursor drawn in afterwards.
//!
//! While recording, a thread captures the window about 30 times a second
//! (the same capture as `see`, so the lane and hidden Spaces work and the
//! user's screen is never touched) and keeps each frame as a JPEG on disk.
//! On stop, every frame gets the cursor for its moment (the cursor glides
//! toward actions that hadn't happened yet when the frame was taken, so it
//! can only be drawn afterwards) and goes into an H.264 movie through
//! AVAssetWriter.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hush_core::cursor::{self, Mark};
use hush_core::{Error, Rect, Result};
use objc2::AnyThread;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus,
    AVFileTypeMPEG4, AVMediaTypeVideo, AVVideoAverageBitRateKey, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoCompressionPropertiesKey, AVVideoHeightKey, AVVideoWidthKey,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{CMTime, kCMTimeZero};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferPool,
    CVPixelBufferUnlockBaseAddress, kCVPixelBufferHeightKey, kCVPixelBufferPixelFormatTypeKey,
    kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::{capture, cursor_art};

/// Frames a second. One capture takes 15 to 33 ms, so 30 leaves room.
const FPS: f64 = 30.0;
/// What shows through the window's transparent corners.
const BACKDROP: (f64, f64, f64) = (0.043, 0.043, 0.055);
/// JPEG quality of the frames kept until stop.
const QUALITY: f64 = 0.92;
/// Bits a second per pixel of the movie: sharp interface text at 1080p.
const BITS_PER_PIXEL: f64 = 6.0;
/// A recording nobody stops ends here, so its frames can't fill the disk
/// (about 10 GB an hour of 1080p JPEGs).
const LONGEST: Duration = Duration::from_secs(600);

struct Live {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Shots>,
    /// The window's size in points, for mapping marks onto frames.
    points: (f64, f64),
}

struct Shots {
    dir: PathBuf,
    frames: Vec<(Duration, PathBuf)>,
    size: (usize, usize),
}

static LIVE: Mutex<Option<HashMap<u64, Live>>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Starts recording the window. Fails at once when it can't be captured.
pub(crate) fn start(window_id: u32, frame: Rect) -> Result<u64> {
    let started = Instant::now();
    let (first, width, height) = capture::frame(window_id, QUALITY, BACKDROP)?;
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("hush-recording-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| Error::Failed(format!("recording folder: {e}")))?;
    let path = dir.join("0.jpg");
    std::fs::write(&path, first).map_err(|e| Error::Failed(format!("recording frame: {e}")))?;
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
                if let Ok((jpeg, w, h)) = capture::frame(window_id, QUALITY, BACKDROP)
                    && (w, h) == shots.size
                {
                    let path = shots.dir.join(format!("{}.jpg", shots.frames.len()));
                    if std::fs::write(&path, jpeg).is_ok() {
                        shots.frames.push((at, path));
                    }
                }
            }
            shots
        })
        .map_err(|e| Error::Failed(format!("couldn't start recording: {e}")))?;
    LIVE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(
            id,
            Live {
                stop,
                thread,
                points: (frame.w, frame.h),
            },
        );
    Ok(id)
}

/// Stops the recording and deletes its frames: its session ended without
/// stopping it.
pub(crate) fn cancel(id: u64) {
    let live = LIVE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .and_then(|live| live.remove(&id));
    if let Some(live) = live {
        live.stop.store(true, Ordering::SeqCst);
        if let Ok(shots) = live.thread.join() {
            let _ = std::fs::remove_dir_all(&shots.dir);
        }
    }
}

/// Stops the recording and writes the movie to `out`, with the cursor drawn
/// from `marks` and labelled `label` (no label when empty).
pub(crate) fn stop(id: u64, marks: &[Mark], label: &str, out: &Path) -> Result<String> {
    let live = LIVE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .and_then(|live| live.remove(&id))
        .ok_or_else(|| Error::NotFound("no recording is running".into()))?;
    live.stop.store(true, Ordering::SeqCst);
    let shots = live
        .thread
        .join()
        .map_err(|_| Error::Failed("the recording thread panicked".into()))?;
    let written = write_movie(&shots, live.points, marks, label, out);
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

fn write_movie(
    shots: &Shots,
    points: (f64, f64),
    marks: &[Mark],
    label: &str,
    out: &Path,
) -> Result<(usize, usize)> {
    // H.264 wants even dimensions.
    let (width, height) = (shots.size.0 & !1, shots.size.1 & !1);
    let scale = shots.size.0 as f64 / points.0.max(1.0);
    let tint = cursor_art::color(label);
    let _ = std::fs::remove_file(out);
    let failed = |what: &str| Error::Failed(format!("couldn't write the movie: {what}"));
    // SAFETY: every Objective-C object is created here and used on this
    // thread; settings dictionaries hold the documented key and value types.
    unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&out.to_string_lossy()));
        let writer = AVAssetWriter::initWithURL_fileType_error(
            AVAssetWriter::alloc(),
            &url,
            AVFileTypeMPEG4.ok_or_else(|| failed("no MPEG-4 file type"))?,
        )
        .map_err(|e| failed(&e.localizedDescription().to_string()))?;
        let bitrate = NSNumber::new_f64(width as f64 * height as f64 * BITS_PER_PIXEL);
        let compression = NSDictionary::<NSString, AnyObject>::from_slices(
            &[AVVideoAverageBitRateKey.ok_or_else(|| failed("no bitrate key"))?],
            &[&*bitrate as &AnyObject],
        );
        let (w, h) = (NSNumber::new_usize(width), NSNumber::new_usize(height));
        let settings = NSDictionary::<NSString, AnyObject>::from_slices(
            &[
                AVVideoCodecKey.ok_or_else(|| failed("no codec key"))?,
                AVVideoWidthKey.ok_or_else(|| failed("no width key"))?,
                AVVideoHeightKey.ok_or_else(|| failed("no height key"))?,
                AVVideoCompressionPropertiesKey.ok_or_else(|| failed("no compression key"))?,
            ],
            &[
                AVVideoCodecTypeH264.ok_or_else(|| failed("no H.264 codec"))? as &AnyObject,
                &*w as &AnyObject,
                &*h as &AnyObject,
                &*compression as &AnyObject,
            ],
        );
        let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
            AVMediaTypeVideo.ok_or_else(|| failed("no video media type"))?,
            Some(&settings),
        );
        input.setExpectsMediaDataInRealTime(false);
        let format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
        let pixel_keys: [&NSString; 3] = [
            bridged(kCVPixelBufferPixelFormatTypeKey),
            bridged(kCVPixelBufferWidthKey),
            bridged(kCVPixelBufferHeightKey),
        ];
        let attributes = NSDictionary::<NSString, AnyObject>::from_slices(
            &pixel_keys,
            &[&*format as &AnyObject, &*w as &AnyObject, &*h as &AnyObject],
        );
        let adaptor = AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
            &input,
            Some(&attributes),
        );
        writer.addInput(&input);
        if !writer.startWriting() {
            return Err(failed("the writer didn't start"));
        }
        writer.startSessionAtSourceTime(kCMTimeZero);
        let pool = adaptor
            .pixelBufferPool()
            .ok_or_else(|| failed("no pixel buffer pool"))?;
        // A steady 30 frames a second: each shows the latest window frame
        // taken by then, with the cursor drawn for its own moment, so the
        // cursor moves smoothly even when captures came slower (several
        // windows recording at once).
        let end = shots.frames.last().map_or(Duration::ZERO, |(t, _)| *t);
        let count = (end.as_secs_f64() * FPS).floor() as u64 + 1;
        let mut shown: Option<(usize, *mut c_void)> = None;
        for n in 0..count {
            let t = Duration::from_secs_f64(n as f64 / FPS);
            let latest = shots
                .frames
                .partition_point(|(at, _)| *at <= t)
                .saturating_sub(1);
            if shown.is_none_or(|(index, _)| index != latest) {
                let Some(image) = std::fs::read(&shots.frames[latest].1)
                    .ok()
                    .and_then(|jpeg| decode(&jpeg))
                else {
                    continue;
                };
                if let Some((_, old)) = shown.replace((latest, image)) {
                    CGImageRelease(old);
                }
            }
            let Some((_, image)) = shown else { continue };
            let buffer = frame_buffer(&pool).ok_or_else(|| failed("no pixel buffer"))?;
            CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
            // kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little: BGRA.
            let context = CGBitmapContextCreate(
                CVPixelBufferGetBaseAddress(&buffer),
                width,
                height,
                8,
                CVPixelBufferGetBytesPerRow(&buffer),
                space,
                2 | (2 << 12),
            );
            CGColorSpaceRelease(space);
            if !context.is_null() {
                // The window's rounded corners are transparent, and pool
                // buffers come back with old pixels in them.
                CGContextSetRGBFillColor(context, BACKDROP.0, BACKDROP.1, BACKDROP.2, 1.0);
                CGContextFillRect(
                    context,
                    CGRect {
                        x: 0.0,
                        y: 0.0,
                        w: width as f64,
                        h: height as f64,
                    },
                );
                CGContextDrawImage(
                    context,
                    CGRect {
                        x: 0.0,
                        y: 0.0,
                        w: width as f64,
                        h: height as f64,
                    },
                    image,
                );
                if let Some(pose) = cursor::pose(marks, t) {
                    // Window points with y down, as marks are.
                    CGContextTranslateCTM(context, 0.0, height as f64);
                    CGContextScaleCTM(context, scale, -scale);
                    cursor_art::draw(context, pose, marks, t, label, tint, points.0);
                }
                CGContextRelease(context);
            }
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            while !input.isReadyForMoreMediaData() {
                // The encoder is behind; it takes a frame in a few ms.
                thread::sleep(Duration::from_millis(2));
            }
            let time = CMTime::new(n as i64, FPS as i32);
            if !adaptor.appendPixelBuffer_withPresentationTime(&buffer, time) {
                if let Some((_, image)) = shown {
                    CGImageRelease(image);
                }
                return Err(failed("a frame was refused"));
            }
        }
        if let Some((_, image)) = shown {
            CGImageRelease(image);
        }
        input.markAsFinished();
        #[allow(deprecated)]
        let finished = writer.finishWriting();
        if !finished || writer.status() != AVAssetWriterStatus::Completed {
            let reason = writer
                .error()
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "it didn't finish".into());
            return Err(failed(&reason));
        }
    }
    Ok((width, height))
}

/// A CoreFoundation string key as the Foundation string it bridges to.
fn bridged(key: &objc2_core_foundation::CFString) -> &NSString {
    // SAFETY: CFString and NSString are toll-free bridged.
    unsafe { &*(std::ptr::from_ref(key).cast::<NSString>()) }
}

fn frame_buffer(pool: &CVPixelBufferPool) -> Option<CFRetained<CVPixelBuffer>> {
    let mut raw: *mut CVPixelBuffer = std::ptr::null_mut();
    // SAFETY: a live pool and a valid out-pointer; a created buffer is +1.
    unsafe {
        CVPixelBufferPool::create_pixel_buffer(None, pool, NonNull::from(&mut raw));
        NonNull::new(raw).map(|raw| CFRetained::from_raw(raw))
    }
}

/// A +1 CGImage from JPEG bytes.
fn decode(jpeg: &[u8]) -> Option<*mut c_void> {
    // SAFETY: the data is copied; the source is released here.
    unsafe {
        let data = CFDataCreate(std::ptr::null_mut(), jpeg.as_ptr(), jpeg.len() as isize);
        let source = CGImageSourceCreateWithData(data, std::ptr::null_mut());
        CFRelease(data);
        if source.is_null() {
            return None;
        }
        let image = CGImageSourceCreateImageAtIndex(source, 0, std::ptr::null_mut());
        CFRelease(source);
        (!image.is_null()).then_some(image)
    }
}

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
    static kCGColorSpaceSRGB: *mut c_void;
    fn CGColorSpaceCreateWithName(name: *mut c_void) -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        row: usize,
        space: *mut c_void,
        info: u32,
    ) -> *mut c_void;
    fn CGContextRelease(context: *mut c_void);
    fn CGContextSetRGBFillColor(context: *mut c_void, r: f64, g: f64, b: f64, a: f64);
    fn CGContextFillRect(context: *mut c_void, rect: CGRect);
    fn CGContextDrawImage(context: *mut c_void, rect: CGRect, image: *mut c_void);
    fn CGContextTranslateCTM(context: *mut c_void, x: f64, y: f64);
    fn CGContextScaleCTM(context: *mut c_void, x: f64, y: f64);
    fn CGImageRelease(image: *mut c_void);
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageSourceCreateWithData(data: *mut c_void, options: *mut c_void) -> *mut c_void;
    fn CGImageSourceCreateImageAtIndex(
        source: *mut c_void,
        index: usize,
        options: *mut c_void,
    ) -> *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataCreate(allocator: *mut c_void, bytes: *const u8, length: isize) -> *mut c_void;
    fn CFRelease(object: *mut c_void);
}
