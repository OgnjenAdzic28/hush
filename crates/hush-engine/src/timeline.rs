//! Playing a timeline on a window (see `hush_core::timeline`).

use hush_core::{Platform, Result, timeline};

use crate::Engine;

impl<P: Platform> Engine<P> {
    /// Plays the events of `lines` on the window and reports how many ran,
    /// for how long and how close to their times. The window isn't read
    /// afterwards; observe it to see the result.
    pub fn timeline(
        &mut self,
        app: &str,
        window: Option<&str>,
        lines: &[String],
    ) -> Result<String> {
        let events = timeline::parse(lines).map_err(hush_core::Error::Failed)?;
        let (app, window) = self.target(app, window)?;
        let length = events.last().map(|e| e.at).unwrap_or_default();
        let played = self.platform.play(&app, &window, &events)?;
        Ok(format!("# {}\n", played.summary(length)))
    }
}
