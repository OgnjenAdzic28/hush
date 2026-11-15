//! The engine against a scripted fake platform: a window whose contents
//! change when the engine acts on it, and which reports an event per change.

use std::cell::RefCell;
use std::time::Instant;

use hush_core::{
    Actions, Activity, AppInfo, Error, Image, Node, Op, Platform, Rect, Result, Role,
    ScriptLanguage, Snapshot, States, WindowInfo,
};

use crate::{Engine, ObserveOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum H {
    Window,
    Save,
    Name,
    Sheet,
    Font,
    Menu,
    Helvetica,
    Courier,
    Stray,
}

#[derive(Default)]
struct World {
    saved: bool,
    name: String,
    font: &'static str,
    menu_open: bool,
    press_unsupported: bool,
    /// A menu item with the same name in another menu of the window.
    stray_item: bool,
    /// Saving closes the window.
    closes_on_save: bool,
    /// Choosing an option leaves the popup's menu open until Escape.
    menu_sticks: bool,
    closed: bool,
    ops: Vec<String>,
    activity: Activity,
}

struct Fake {
    world: RefCell<World>,
}

fn app() -> AppInfo {
    AppInfo {
        name: "Editor".into(),
        pid: Some(42),
        id: Some("com.example.editor".into()),
        path: None,
        frontmost: false,
    }
}

fn window() -> WindowInfo {
    WindowInfo {
        id: 7,
        title: "Doc".into(),
        frame: Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        },
        main: true,
        minimized: false,
    }
}

fn element(handle: H, role: Role, name: &str, parent: Option<usize>) -> Node<H> {
    Node {
        handle,
        role,
        name: name.into(),
        value: None,
        frame: Some(Rect {
            x: 10.0,
            y: 10.0,
            w: 100.0,
            h: 20.0,
        }),
        states: States::default(),
        actions: match role {
            Role::Button | Role::PopUp | Role::MenuItem => Actions::PRESS,
            _ => Actions::default(),
        },
        parent,
        unread: 0,
    }
}

impl Fake {
    fn new() -> Fake {
        Fake {
            world: RefCell::new(World {
                font: "Courier",
                ..World::default()
            }),
        }
    }

    fn changed(&self, op: String) {
        let mut world = self.world.borrow_mut();
        world.ops.push(op);
        world.activity.events += 1;
        world.activity.last = Some(Instant::now());
    }

    fn ops(&self) -> Vec<String> {
        self.world.borrow().ops.clone()
    }
}

impl Platform for Fake {
    type Handle = H;

    fn apps(&self, _query: Option<&str>) -> Result<Vec<AppInfo>> {
        Ok(vec![app()])
    }

    fn windows(&self, _app: &AppInfo) -> Result<Vec<WindowInfo>> {
        Ok(vec![window()])
    }

    fn read(&self, app: &AppInfo, window: &WindowInfo) -> Result<Snapshot<H>> {
        let world = self.world.borrow();
        if world.closed {
            return Err(Error::Stale("window \"Doc\" is gone".into()));
        }
        let mut nodes = vec![element(H::Window, Role::Window, "Doc", None)];
        if world.stray_item {
            nodes.push(element(H::Stray, Role::MenuItem, "Helvetica", Some(0)));
        }
        let mut name = element(H::Name, Role::TextField, "Name", Some(0));
        name.value = Some(world.name.clone());
        nodes.push(name);
        let mut font = element(H::Font, Role::PopUp, "Font", Some(0));
        font.value = Some(world.font.into());
        let font_index = nodes.len();
        nodes.push(font);
        if world.menu_open {
            nodes.push(element(H::Menu, Role::Menu, "", Some(font_index)));
            nodes.push(element(
                H::Helvetica,
                Role::MenuItem,
                "Helvetica",
                Some(font_index + 1),
            ));
            nodes.push(element(
                H::Courier,
                Role::MenuItem,
                "Courier",
                Some(font_index + 1),
            ));
        }
        if world.saved {
            nodes.push(element(H::Sheet, Role::Sheet, "Saved", Some(0)));
        } else {
            nodes.push(element(H::Save, Role::Button, "Save", Some(0)));
        }
        Ok(Snapshot::new(
            app.clone(),
            window.clone(),
            nodes,
            vec!["File".into()],
        ))
    }

    fn play(
        &self,
        _app: &AppInfo,
        _window: &WindowInfo,
        events: &[hush_core::Timed],
    ) -> Result<hush_core::Played> {
        let first = events.first().map(|e| e.input);
        let last = events.last().map(|e| e.input);
        self.changed(format!(
            "Play {} events {first:?} .. {last:?}",
            events.len()
        ));
        Ok(hush_core::Played::default())
    }

    fn activity(&self, _app: &AppInfo) -> Activity {
        self.world.borrow().activity
    }

    fn perform(&self, _app: &AppInfo, _window: &WindowInfo, op: Op<'_, H>) -> Result<()> {
        let text = format!("{op:?}");
        match op {
            Op::Press(_) if self.world.borrow().press_unsupported => {
                return Err(Error::Unsupported("no press here".into()));
            }
            Op::Press(H::Save) | Op::Click { .. } => {
                let mut world = self.world.borrow_mut();
                world.saved = true;
                world.closed = world.closes_on_save;
            }
            Op::Press(H::Font) => self.world.borrow_mut().menu_open = true,
            Op::Press(H::Helvetica) => {
                let mut world = self.world.borrow_mut();
                world.font = "Helvetica";
                world.menu_open = world.menu_sticks;
            }
            Op::Key { .. } => self.world.borrow_mut().menu_open = false,
            Op::Type { text, .. } => self.world.borrow_mut().name.push_str(text),
            _ => {}
        }
        self.changed(text);
        Ok(())
    }

    fn capture(
        &self,
        _window: &WindowInfo,
        _region: Option<Rect>,
        _max_width: u32,
    ) -> Result<Image> {
        Err(Error::Unsupported("no screen".into()))
    }

    fn script(&self, _app: &AppInfo, _source: &str, _language: ScriptLanguage) -> Result<String> {
        Ok("done".into())
    }
}

fn steps(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|s| s.to_string()).collect()
}

#[test]
fn observe_shows_the_outline_once_then_only_changes() {
    let mut engine = Engine::new(Fake::new());
    let first = engine.observe("Editor", ObserveOptions::default()).unwrap();
    assert!(first.starts_with("# Editor · window \"Doc\""), "{first}");
    assert!(first.contains("b1 button \"Save\""), "{first}");

    let again = engine.observe("Editor", ObserveOptions::default()).unwrap();
    assert_eq!(again, "# Editor · no change since the last look\n");

    engine.platform().world.borrow_mut().saved = true;
    let changed = engine.observe("Editor", ObserveOptions::default()).unwrap();
    assert_eq!(changed, "- b1 button \"Save\"\n+ e2 sheet \"Saved\"\n");
}

#[test]
fn act_runs_the_batch_and_answers_with_the_change() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    let report = engine
        .act("Editor", None, &steps(&["type t1 \"Q4\"", "click b1"]))
        .unwrap();
    assert!(report.starts_with("# 2/2 steps · "), "{report}");
    assert!(
        report.contains("~ t1 field \"Name\": = \"\" → = \"Q4\""),
        "{report}"
    );
    assert!(report.contains("- b1 button \"Save\""), "{report}");
    assert_eq!(
        engine.platform().ops(),
        ["Type { text: \"Q4\", into: Some(Name) }", "Press(Save)"]
    );
}

#[test]
fn a_refused_press_falls_back_to_a_pointer_click_on_the_element() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    engine.platform().world.borrow_mut().press_unsupported = true;
    let report = engine.act("Editor", None, &steps(&["click b1"])).unwrap();
    assert!(report.contains("+ e2 sheet \"Saved\""), "{report}");
    assert_eq!(
        engine.platform().ops(),
        ["Click { x: 60.0, y: 20.0, button: Left, count: 1 }"]
    );
}

#[test]
fn the_batch_stops_at_the_first_failure_and_still_reports_the_change() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    let report = engine
        .act(
            "Editor",
            None,
            &steps(&["click b1", "click b1", "type t1 \"never\""]),
        )
        .unwrap();
    assert!(report.starts_with("# 1/3 steps"), "{report}");
    assert!(
        report.contains("stopped at step 2 (`click b1`): stale: b1 isn't in the window"),
        "{report}"
    );
    assert!(report.contains("+ e2 sheet \"Saved\""), "{report}");
    assert!(
        !engine
            .platform()
            .ops()
            .iter()
            .any(|op| op.contains("never"))
    );
}

#[test]
fn pick_opens_a_popup_and_chooses_the_option_by_name() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    let report = engine
        .act("Editor", None, &steps(&["pick p1 helvetica"]))
        .unwrap();
    assert!(
        report.contains("~ p1 popup \"Font\": = \"Courier\" → = \"Helvetica\""),
        "{report}"
    );
    assert_eq!(engine.platform().ops(), ["Press(Font)", "Press(Helvetica)"]);
}

#[test]
fn bad_steps_are_rejected_before_anything_runs() {
    let mut engine = Engine::new(Fake::new());
    let error = engine
        .act("Editor", None, &steps(&["click b1", "fly away"]))
        .unwrap_err();
    assert!(error.to_string().contains("step 2 (`fly away`)"), "{error}");
    assert!(engine.platform().ops().is_empty());
}

#[test]
fn wait_for_text_returns_once_it_appears() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().saved = true;
    let report = engine
        .act("Editor", None, &steps(&["wait text \"saved\" within 1s"]))
        .unwrap();
    assert!(report.starts_with("# 1/1 steps"), "{report}");
    let timeout = engine
        .act("Editor", None, &steps(&["wait text \"nope\" within 0.1s"]))
        .unwrap();
    assert!(timeout.contains("stopped at step 1"), "{timeout}");
}

#[test]
fn looking_at_one_part_does_not_hide_changes_elsewhere() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    engine.platform().world.borrow_mut().saved = true;
    let part = ObserveOptions {
        focus: Some(hush_core::Ref::parse("t1").unwrap()),
        ..ObserveOptions::default()
    };
    engine.observe("Editor", part).unwrap();
    let next = engine.observe("Editor", ObserveOptions::default()).unwrap();
    assert!(next.contains("- b1 button \"Save\""), "{next}");
}

#[test]
fn a_step_that_closes_the_window_still_gets_its_report() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    engine.platform().world.borrow_mut().closes_on_save = true;
    let report = engine.act("Editor", None, &steps(&["click b1"])).unwrap();
    assert!(report.starts_with("# 1/1 steps"), "{report}");
    assert!(report.contains("window \"Doc\" is gone"), "{report}");
}

#[test]
fn pick_chooses_from_the_menu_it_opened_not_another_one() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().stray_item = true;
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    engine
        .act("Editor", None, &steps(&["pick p1 Helvetica"]))
        .unwrap();
    assert_eq!(engine.platform().ops(), ["Press(Font)", "Press(Helvetica)"]);
}

#[test]
fn a_menu_left_open_after_pick_is_closed_so_it_cannot_cover_the_user() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().menu_sticks = true;
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    let report = engine
        .act("Editor", None, &steps(&["pick p1 Helvetica"]))
        .unwrap();
    assert_eq!(
        engine.platform().ops().last().unwrap(),
        "Key { combo: KeyCombo { modifiers: Modifiers { cmd: false, shift: false, alt: false, ctrl: false, function: false }, key: Named(Escape) }, repeat: 1 }"
    );
    assert!(!report.contains("menuitem"), "{report}");
}

#[test]
fn drag_plays_a_press_glide_and_release_between_the_two_elements() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    let report = engine
        .act("Editor", None, &steps(&["drag b1 to 300,200"]))
        .unwrap();
    assert!(report.contains("dragged"), "{report}");
    let ops = engine.platform().ops();
    let play = ops
        .iter()
        .find(|op| op.starts_with("Play"))
        .expect("a drag plays a timeline");
    // Every element of the fake sits at 10,10 sized 100x20: its center is 60,20.
    assert!(play.contains("PointerDown { x: 60.0, y: 20.0"), "{play}");
    assert!(play.contains("PointerUp { x: 300.0, y: 200.0"), "{play}");
}
