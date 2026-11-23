//! Running a batch of steps: route each one, wait for the app to settle,
//! stop at the first failure, and answer with what changed.

use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::fmt::Write;
use std::thread;
use std::time::{Duration, Instant};

use hush_core::{
    AppInfo, Condition, Diff, Element, Error, Key, KeyCombo, NamedKey, Op, Platform, Reason, Ref,
    Result, Role, Step, WindowInfo, timeline,
};

use hush_core::cursor::Gesture;

use crate::mirror::Mirror;
use crate::route::route;
use crate::{Engine, OUTLINE_BUDGET, key, wait};

/// The longest `pick` waits for the popup's menu to close after a choice.
const MENU_CLOSE: Duration = Duration::from_millis(1500);

fn escape() -> KeyCombo {
    KeyCombo {
        modifiers: Default::default(),
        key: Key::Named(NamedKey::Escape),
    }
}

/// How often a `wait` condition is checked when the app sends no events.
const CONDITION_POLL: Duration = Duration::from_millis(50);

impl<P: Platform> Engine<P> {
    /// Runs `steps` (one command each, see `hush_core::step::SYNTAX`) on the
    /// window and returns a report: steps done, time, and the change.
    pub fn act(&mut self, app: &str, window: Option<&str>, steps: &[String]) -> Result<String> {
        // Every step's syntax is checked before any runs. A step that names
        // elements by locator is parsed again just before it runs, against
        // the window as the earlier steps left it.
        let checked = steps
            .iter()
            .enumerate()
            .map(|(i, text)| {
                Step::check(text)
                    .map_err(|e| Error::Failed(format!("step {} (`{text}`): {e}", i + 1)))
            })
            .collect::<Result<Vec<(Step, bool)>>>()?;
        let parsed: Vec<Step> = checked.iter().map(|(step, _)| step.clone()).collect();
        if parsed.is_empty() {
            return Err(Error::Failed("act needs at least one step".into()));
        }

        if window.is_none() && parsed.iter().all(|s| matches!(s, Step::Menu { .. })) {
            let found = self.find_app(app)?;
            if self.platform.windows(&found)?.is_empty() {
                return self.menus_without_window(&found, &parsed, steps);
            }
        }
        let (app, window) = self.target(app, window)?;
        let key = key(&app, &window);
        if let Entry::Vacant(entry) = self.mirrors.entry(key) {
            let snapshot = self.platform.read(&app, &window)?;
            let mirror = entry.insert(Mirror::new(snapshot, &self.options));
            mirror.mark_seen();
        }

        let started = Instant::now();
        let mut done = 0;
        let mut stopped = None;
        let mut dirty = false;
        let mut last_settle = None;
        for (i, (step, located)) in checked.iter().enumerate() {
            let step = match located {
                true => self.locate(&app, &window, &steps[i], &mut dirty),
                false => Ok(step.clone()),
            };
            match step.and_then(|step| self.run(&app, &window, &step, &mut dirty)) {
                Ok(settled) => {
                    done += 1;
                    last_settle = settled.or(last_settle);
                }
                Err(error) => {
                    stopped = Some(format!(
                        "stopped at step {} (`{}`): {error}",
                        i + 1,
                        steps[i]
                    ));
                    break;
                }
            }
        }

        let mut out = format!(
            "# {done}/{} steps · {:.2} s",
            parsed.len(),
            started.elapsed().as_secs_f64()
        );
        if let Some(settle) = last_settle {
            let _ = write!(out, " · {settle}");
        }
        out.push('\n');
        if let Some(stopped) = stopped {
            let _ = writeln!(out, "# {stopped}");
        }
        // A step may close the window (Save, OK, Close); the report of what
        // ran still matters more than the read that can't happen.
        let snapshot = match self.platform.read(&app, &window) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.mirrors.remove(&key);
                let _ = writeln!(
                    out,
                    "# window \"{}\" is gone ({error}); observe the app again",
                    window.title
                );
                return Ok(out);
            }
        };
        let mirror = self.mirrors.get_mut(&key).expect("mirror created above");
        mirror.update(snapshot, &self.options);
        let diff = Diff::between(&mirror.seen, &mirror.current);
        if diff.is_empty() {
            out.push_str("# no visible change\n");
        } else if diff.replaces(&mirror.current) {
            out.push_str(&mirror.current.render_within(OUTLINE_BUDGET));
        } else {
            out.push_str(&diff.render());
        }
        mirror.mark_seen();
        Ok(out)
    }

    /// Runs one step. Returns how the app settled, when it was waited for.
    fn run(
        &mut self,
        app: &AppInfo,
        window: &WindowInfo,
        step: &Step,
        dirty: &mut bool,
    ) -> Result<Option<String>> {
        match step {
            Step::Wait { until, within } => {
                self.wait_for(app, window, until, *within, dirty)?;
                Ok(None)
            }
            Step::Drag { from, to } => {
                for target in [from, to] {
                    if let hush_core::Target::Ref(r) = target
                        && (*dirty || self.mirror(app, window).node(*r).is_none())
                    {
                        self.refresh(app, window, dirty)?;
                    }
                }
                let mirror = self.mirror(app, window);
                let node = |r: Ref| {
                    mirror.node(r).ok_or_else(|| {
                        Error::Stale(format!("{r} isn't in the window any more; observe again"))
                    })
                };
                let start = crate::route::point(from, &node)?;
                let mut end = crate::route::point(to, &node)?;
                // Onto a ref, the dragged item takes its place: the upper
                // part of the target when moving up, the lower part when
                // moving down, so a list drops it before or after.
                if let hush_core::Target::Ref(r) = to
                    && let Some(frame) = node(*r)?.frame
                {
                    end.1 += if end.1 < start.1 {
                        -frame.h / 4.0
                    } else {
                        frame.h / 4.0
                    };
                }
                let events = timeline::drag(start, end);
                let pace = self.paces.entry(app.pid.unwrap_or(0)).or_default();
                let pending = wait::begin(&self.platform, app, pace);
                let pressed = Instant::now();
                self.platform.play(app, window, &events)?;
                if self.is_recording(app, window) {
                    let released = Instant::now();
                    let started = self.recording_elapsed(released);
                    crate::record::mark(
                        &mut self.recording,
                        key(app, window),
                        pressed,
                        start,
                        Gesture::Drag {
                            to: end,
                            until: started,
                        },
                    );
                }
                let outcome = pending.finish(
                    &self.platform,
                    app,
                    self.paces.entry(app.pid.unwrap_or(0)).or_default(),
                );
                *dirty = true;
                Ok(Some(format!(
                    "dragged, settled in {} ms",
                    outcome.after.as_millis()
                )))
            }
            Step::Pick { r, option } => {
                let opened = Step::Click {
                    target: hush_core::Target::Ref(*r),
                    button: hush_core::Button::Left,
                    count: 1,
                };
                // The options are the menu items the press brought up; items
                // that were already there belong to other menus.
                let before: HashSet<Ref> = self
                    .menu_items(app, window, &HashSet::new())
                    .map(|(r, _)| r)
                    .collect();
                self.run(app, window, &opened, dirty)?;
                self.refresh(app, window, dirty)?;
                let mut exclude = before;
                if self.menu_items(app, window, &exclude).next().is_none() {
                    exclude.clear();
                }
                let item = self.find_option(app, window, option, &exclude);
                match item {
                    Some(item) => {
                        let chosen = Step::Click {
                            target: hush_core::Target::Ref(item),
                            button: hush_core::Button::Left,
                            count: 1,
                        };
                        let settled = self.run(app, window, &chosen, dirty)?;
                        // The app takes the choice when its menu finishes
                        // closing (an animation of about 200 ms on macOS),
                        // after its first burst of events; answering before
                        // that reports a menu that is about to vanish and a
                        // value that hasn't changed yet. A menu that stays
                        // open is closed so it doesn't float over the user.
                        if self
                            .wait_for(
                                app,
                                window,
                                &Condition::Gone(Element::Ref(item)),
                                MENU_CLOSE,
                                dirty,
                            )
                            .is_err()
                        {
                            let _ = self.platform.perform(
                                app,
                                window,
                                Op::Key {
                                    combo: escape(),
                                    repeat: 1,
                                },
                            );
                        }
                        Ok(settled)
                    }
                    None => {
                        let choices = self.option_names(app, window, &exclude);
                        let _ = self.platform.perform(
                            app,
                            window,
                            Op::Key {
                                combo: escape(),
                                repeat: 1,
                            },
                        );
                        Err(Error::NotFound(format!(
                            "no option \"{option}\" in {r}; it has {}",
                            choices.join(", ")
                        )))
                    }
                }
            }
            _ => {
                if *dirty || needs_refresh(step, self.mirror(app, window)) {
                    self.refresh(app, window, dirty)?;
                }
                let mirror = self
                    .mirrors
                    .get(&key(app, window))
                    .expect("act creates the mirror first");
                let node = |r: Ref| {
                    mirror.node(r).ok_or_else(|| {
                        Error::Stale(format!("{r} isn't in the window any more; observe again"))
                    })
                };
                let route = route(step, node)?;
                let gesture = gesture_at(step, &node);
                if let Some((point, gesture)) = gesture {
                    crate::record::mark(
                        &mut self.recording,
                        key(app, window),
                        Instant::now(),
                        point,
                        gesture,
                    );
                }
                let pace = self.paces.entry(app.pid.unwrap_or(0)).or_default();
                let pending = wait::begin(&self.platform, app, pace);
                match self.platform.perform(app, window, route.first) {
                    Err(Error::Unsupported(_)) if route.fallback.is_some() => {
                        self.platform
                            .perform(app, window, route.fallback.expect("checked"))?;
                    }
                    other => other?,
                }
                let outcome = pending.finish(
                    &self.platform,
                    app,
                    self.paces.entry(app.pid.unwrap_or(0)).or_default(),
                );
                if outcome.events > 0 {
                    *dirty = true;
                }
                let ms = outcome.after.as_millis();
                Ok(Some(match outcome.reason {
                    Reason::Quiet => format!("settled in {ms} ms"),
                    Reason::NoReaction => format!("no reaction within {ms} ms"),
                    Reason::Limit => format!("still busy after {ms} ms"),
                }))
            }
        }
    }

    fn wait_for(
        &mut self,
        app: &AppInfo,
        window: &WindowInfo,
        until: &Condition,
        within: Duration,
        dirty: &mut bool,
    ) -> Result<()> {
        let deadline = Instant::now() + within;
        if let Condition::Elapsed(time) = until {
            thread::sleep((*time).min(within));
            *dirty = true;
            return Ok(());
        }
        loop {
            if let Condition::Settled = until {
                let pace = self.paces.entry(app.pid.unwrap_or(0)).or_default();
                let pending = wait::begin(&self.platform, app, pace);
                pending.finish(
                    &self.platform,
                    app,
                    self.paces.entry(app.pid.unwrap_or(0)).or_default(),
                );
                *dirty = true;
                return Ok(());
            }
            let before = self.platform.activity(app);
            *dirty = true;
            self.refresh(app, window, dirty)?;
            if met(until, self.mirror(app, window)) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::Failed(format!(
                    "still waiting after {:.1} s",
                    within.as_secs_f64()
                )));
            }
            // Check again as soon as the app reports a change.
            let next = Instant::now() + CONDITION_POLL;
            while Instant::now() < next.min(deadline)
                && self.platform.activity(app).events == before.events
            {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }

    /// Parses `text` with its locators turned into refs of the window as it
    /// is now. A locator that finds nothing is tried once more on a fresh
    /// read, in case the app changed without saying so.
    fn locate(
        &mut self,
        app: &AppInfo,
        window: &WindowInfo,
        text: &str,
        dirty: &mut bool,
    ) -> Result<Step> {
        if *dirty {
            self.refresh(app, window, dirty)?;
        }
        let parse = |engine: &Self| {
            let lines = &engine.mirror(app, window).current.lines;
            Step::parse_with(text, &mut |locator| {
                locator.find(lines).map_err(|miss| miss.explain(locator))
            })
            .map_err(Error::NotFound)
        };
        match parse(self) {
            Err(_) => {
                self.refresh(app, window, dirty)?;
                parse(self)
            }
            found => found,
        }
    }

    fn refresh(&mut self, app: &AppInfo, window: &WindowInfo, dirty: &mut bool) -> Result<()> {
        let snapshot = self.platform.read(app, window)?;
        let options = self.options.clone();
        if let Some(mirror) = self.mirrors.get_mut(&key(app, window)) {
            mirror.update(snapshot, &options);
        }
        *dirty = false;
        Ok(())
    }

    fn mirror(&self, app: &AppInfo, window: &WindowInfo) -> &Mirror<P::Handle> {
        self.mirrors
            .get(&key(app, window))
            .expect("act creates the mirror first")
    }

    /// Menu items in the window, in outline order, leaving out `exclude`.
    fn menu_items<'m>(
        &'m self,
        app: &AppInfo,
        window: &WindowInfo,
        exclude: &'m HashSet<Ref>,
    ) -> impl Iterator<Item = (Ref, &'m str)> + 'm {
        self.mirror(app, window)
            .nodes()
            .filter(move |(r, node)| node.role == Role::MenuItem && !exclude.contains(r))
            .map(|(r, node)| (r, node.name.as_str()))
    }

    fn find_option(
        &self,
        app: &AppInfo,
        window: &WindowInfo,
        option: &str,
        exclude: &HashSet<Ref>,
    ) -> Option<Ref> {
        let wanted = option.to_lowercase();
        let items: Vec<(Ref, String)> = self
            .menu_items(app, window, exclude)
            .map(|(r, name)| (r, name.to_lowercase()))
            .collect();
        items
            .iter()
            .find(|(_, name)| *name == wanted)
            .or_else(|| items.iter().find(|(_, name)| name.contains(&wanted)))
            .map(|(r, _)| *r)
    }

    fn option_names(
        &self,
        app: &AppInfo,
        window: &WindowInfo,
        exclude: &HashSet<Ref>,
    ) -> Vec<String> {
        self.menu_items(app, window, exclude)
            .filter(|(_, name)| !name.is_empty())
            .map(|(_, name)| format!("\"{name}\""))
            .take(15)
            .collect()
    }
}

/// A step names a ref the mirror doesn't know (it appeared after the last
/// read), so the window is read again before routing.
fn needs_refresh<H: Clone + Eq + std::hash::Hash>(step: &Step, mirror: &Mirror<H>) -> bool {
    let r = match step {
        Step::Click {
            target: hush_core::Target::Ref(r),
            ..
        }
        | Step::Scroll {
            target: hush_core::Target::Ref(r),
            ..
        }
        | Step::Set { r, .. }
        | Step::Select { r }
        | Step::Focus { r }
        | Step::Show { r }
        | Step::Pick { r, .. } => *r,
        Step::Type { into: Some(r), .. } => *r,
        _ => return false,
    };
    mirror.node(r).is_none()
}

fn met<H: Clone + Eq + std::hash::Hash>(until: &Condition, mirror: &Mirror<H>) -> bool {
    match until {
        Condition::Exists(element) => found(element, mirror),
        Condition::Gone(element) => !found(element, mirror),
        Condition::Text(text) => {
            let wanted = text.to_lowercase();
            mirror.snapshot().nodes.iter().any(|node| {
                node.name.to_lowercase().contains(&wanted)
                    || node
                        .value
                        .as_deref()
                        .is_some_and(|v| v.to_lowercase().contains(&wanted))
            })
        }
        Condition::Settled | Condition::Elapsed(_) => true,
    }
}

/// Whether the element is in the window. A locator without an index that
/// matches several elements counts: something it names is there. One whose
/// `[N]` is past the last match doesn't: the Nth isn't there.
fn found<H: Clone + Eq + std::hash::Hash>(element: &Element, mirror: &Mirror<H>) -> bool {
    match element {
        Element::Ref(r) => mirror.node(*r).is_some(),
        Element::Find(locator) => !matches!(
            locator.find(&mirror.current.lines),
            Err(hush_core::Miss::None(_) | hush_core::Miss::Past(_))
        ),
    }
}

/// Where a step acts and how, for a recording's cursor: clicks at their
/// point, and steps that type into, set or scroll an element at its center.
fn gesture_at<'a, H: 'a>(
    step: &Step,
    node: &impl Fn(Ref) -> Result<&'a hush_core::Node<H>>,
) -> Option<((f64, f64), Gesture)> {
    let center = |r: Ref| node(r).ok()?.frame.map(|f| f.center());
    match step {
        Step::Click { target, .. } => {
            Some((crate::route::point(target, node).ok()?, Gesture::Click))
        }
        Step::Scroll { target, .. } => {
            Some((crate::route::point(target, node).ok()?, Gesture::Point))
        }
        Step::Set { r, .. }
        | Step::Select { r }
        | Step::Focus { r }
        | Step::Show { r }
        | Step::Type { into: Some(r), .. } => Some((center(*r)?, Gesture::Point)),
        _ => None,
    }
}
