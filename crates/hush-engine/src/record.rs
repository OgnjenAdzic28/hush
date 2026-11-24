//! Recording a window while this session acts on it: the platform captures
//! the frames, the engine notes where each action happened (see
//! `hush_core::cursor`), and on stop the movie gets an agent cursor.

use std::path::PathBuf;
use std::time::Instant;

use hush_core::cursor::{Gesture, Mark};
use hush_core::{AppInfo, Error, Platform, Result, WindowInfo};

use crate::{Engine, key};

pub(crate) struct Recording {
    /// The recorded window, as mirrors are keyed.
    window: (u32, u64),
    id: u64,
    started: Instant,
    marks: Vec<Mark>,
    /// A test's recording, which moves when the test's steps move to a
    /// window they opened (see `follow`).
    follows: bool,
}

impl<P: Platform> Engine<P> {
    /// Starts recording the window; acts on it leave marks for the cursor.
    pub fn record_start(&mut self, app: &str, window: Option<&str>) -> Result<String> {
        self.start_recording(app, window, false)
    }

    /// Starts a test's recording, which follows the test's steps to a
    /// window they open.
    pub(crate) fn record_test(&mut self, app: &str, window: Option<&str>) -> Result<String> {
        self.start_recording(app, window, true)
    }

    fn start_recording(
        &mut self,
        app: &str,
        window: Option<&str>,
        follows: bool,
    ) -> Result<String> {
        if self.recording.is_some() {
            return Err(Error::Failed(
                "a recording is already running; stop it first".into(),
            ));
        }
        let (app, window) = self.target(app, window)?;
        let id = self.platform.record_start(&window)?;
        self.recording = Some(Recording {
            window: key(&app, &window),
            id,
            started: Instant::now(),
            marks: Vec::new(),
            follows,
        });
        Ok(format!(
            "recording {} · window \"{}\"; act on it, then stop with record stop",
            app.name, window.title
        ))
    }

    /// Stops the recording and writes the movie to `out` (default: a new
    /// file in ~/Movies), with an agent cursor labelled `label`.
    pub fn record_stop(&mut self, label: Option<&str>, out: Option<&str>) -> Result<String> {
        let recording = self
            .recording
            .take()
            .ok_or_else(|| Error::NotFound("no recording is running".into()))?;
        let out = match out {
            Some(path) => PathBuf::from(path),
            None => default_path(),
        };
        if let Some(folder) = out.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        self.platform.record_stop(
            recording.id,
            &recording.marks,
            label.unwrap_or("Hush"),
            &out,
        )
    }

    /// Moves a test's recording from `from` to `to`, the window the steps
    /// just moved to, so a failed test's movie shows the step that failed.
    /// A capture can't change windows, so the movie starts again at `to`.
    /// A recording the agent started stays on the window it chose.
    pub(crate) fn follow(&mut self, app: &AppInfo, from: &WindowInfo, to: &WindowInfo) {
        let Some(recording) = &self.recording else {
            return;
        };
        if !recording.follows || recording.window != key(app, from) {
            return;
        }
        self.platform.record_cancel(recording.id);
        self.recording = None;
        // A movie that can't start shouldn't fail the test it was for.
        if let Ok(id) = self.platform.record_start(to) {
            self.recording = Some(Recording {
                window: key(app, to),
                id,
                started: Instant::now(),
                marks: Vec::new(),
                follows: true,
            });
        }
    }

    /// `at` as time since the recording started.
    pub(crate) fn recording_elapsed(&self, at: Instant) -> std::time::Duration {
        self.recording.as_ref().map_or_else(Default::default, |r| {
            at.saturating_duration_since(r.started)
        })
    }

    /// Whether a recording is running, of any window.
    pub(crate) fn is_recording_any(&self) -> bool {
        self.recording.is_some()
    }

    /// Whether this window is being recorded.
    pub(crate) fn is_recording(&self, app: &AppInfo, window: &WindowInfo) -> bool {
        self.recording
            .as_ref()
            .is_some_and(|r| r.window == key(app, window))
    }
}

/// A session that ends without stopping its recording cancels it, so its
/// capture doesn't run on in the host.
impl<P: Platform> Drop for Engine<P> {
    fn drop(&mut self) {
        if let Some(recording) = self.recording.take() {
            self.platform.record_cancel(recording.id);
        }
    }
}

impl Recording {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

/// Notes an action at a window point, when `window` is the one recording.
/// Takes the field rather than the engine so it can run while a step still
/// borrows the window's mirror.
pub(crate) fn mark(
    recording: &mut Option<Recording>,
    window: (u32, u64),
    at: Instant,
    (x, y): (f64, f64),
    gesture: Gesture,
) {
    if let Some(recording) = recording
        && recording.window == window
    {
        recording.marks.push(Mark {
            at: at.saturating_duration_since(recording.started),
            x,
            y,
            gesture,
        });
    }
}

fn default_path() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(std::env::temp_dir, PathBuf::from);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    home.join("Movies").join(format!("hush-{stamp}.mp4"))
}
