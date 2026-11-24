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
    /// The app has no windows until File > New opens one.
    windowless: bool,
    /// The app is installed but not running until it is launched.
    stopped: bool,
    /// cmd+n opens a second window, which becomes the main one.
    opens_on_cmd_n: bool,
    /// The second window is open; its Name field holds this text.
    second: Option<String>,
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
        let mut app = app();
        if self.world.borrow().stopped {
            app.pid = None;
        }
        Ok(vec![app])
    }

    fn launch(&self, _app: &AppInfo) -> Result<()> {
        self.world.borrow_mut().stopped = false;
        self.changed("Launch".into());
        Ok(())
    }

    fn windows(&self, _app: &AppInfo) -> Result<Vec<WindowInfo>> {
        if self.world.borrow().windowless {
            return Ok(Vec::new());
        }
        let mut windows = vec![window()];
        if self.world.borrow().second.is_some() {
            windows[0].main = false;
            windows.push(WindowInfo {
                id: 8,
                title: "Doc 2".into(),
                main: true,
                ..window()
            });
        }
        Ok(windows)
    }

    fn press_menu(&self, _app: &AppInfo, path: &[String]) -> Result<()> {
        if path != ["File", "New"] {
            return Err(Error::NotFound(format!(
                "no menu item {}",
                path.join(" > ")
            )));
        }
        self.world.borrow_mut().windowless = false;
        self.changed(format!("PressMenu({})", path.join(" > ")));
        Ok(())
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
        name.value = Some(match (window.id, &world.second) {
            (8, Some(second)) => second.clone(),
            _ => world.name.clone(),
        });
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

    fn perform(&self, _app: &AppInfo, window: &WindowInfo, op: Op<'_, H>) -> Result<()> {
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
            Op::Key { combo, .. } => {
                let mut world = self.world.borrow_mut();
                world.menu_open = false;
                if world.opens_on_cmd_n && combo.to_string() == "cmd+n" {
                    world.second = Some(String::new());
                }
            }
            Op::Type { text, .. } => {
                let mut world = self.world.borrow_mut();
                match (window.id, &mut world.second) {
                    (8, Some(second)) => second.push_str(text),
                    _ => world.name.push_str(text),
                }
            }
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

    fn record_start(&self, window: &WindowInfo) -> Result<u64> {
        self.changed(format!("RecordStart({})", window.id));
        Ok(window.id)
    }

    fn record_cancel(&self, id: u64) {
        self.changed(format!("RecordCancel({id})"));
    }

    fn record_stop(
        &self,
        id: u64,
        _marks: &[hush_core::cursor::Mark],
        _label: &str,
        _out: &std::path::Path,
    ) -> Result<String> {
        self.changed(format!("RecordStop({id})"));
        Ok("wrote the movie".into())
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

#[test]
fn a_menu_step_works_on_an_app_with_no_windows() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().windowless = true;
    let report = engine
        .act("Editor", None, &steps(&["menu File > New"]))
        .unwrap();
    assert!(report.starts_with("# 1/1 steps · "), "{report}");
    assert!(report.contains("window \"Doc\""), "{report}");
    assert_eq!(engine.platform().ops(), ["PressMenu(File > New)"]);
}

#[test]
fn other_steps_still_need_a_window() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().windowless = true;
    let error = engine
        .act("Editor", None, &steps(&["menu File > New", "click b1"]))
        .unwrap_err();
    assert!(error.to_string().contains("no open windows"), "{error}");
    assert!(engine.platform().ops().is_empty());
}

#[test]
fn a_menu_that_opens_nothing_says_so() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().windowless = true;
    let report = engine
        .act("Editor", None, &steps(&["menu Edit > Nothing"]))
        .unwrap();
    assert!(report.starts_with("# 0/1 steps · "), "{report}");
    assert!(report.contains("stopped at step 1"), "{report}");
    assert!(
        report.contains("Editor still has no open windows"),
        "{report}"
    );
}

#[test]
fn observe_starts_an_installed_app_named_exactly() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().stopped = true;
    let text = engine.observe("editor", ObserveOptions::default()).unwrap();
    assert!(
        text.starts_with("# started Editor in the background\n# Editor"),
        "{text}"
    );
    assert_eq!(engine.platform().ops(), ["Launch"]);
}

#[test]
fn a_partial_name_never_starts_an_app() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().stopped = true;
    let error = engine
        .observe("Edit", ObserveOptions::default())
        .unwrap_err();
    assert!(error.to_string().contains("isn't running"), "{error}");
    assert!(engine.platform().ops().is_empty());
}

#[test]
fn act_doesnt_start_apps() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().stopped = true;
    let error = engine
        .act("Editor", None, &steps(&["click b1"]))
        .unwrap_err();
    assert!(
        error.to_string().contains("observe Editor starts it"),
        "{error}"
    );
    assert!(engine.platform().ops().is_empty());
}

#[test]
fn locators_act_on_a_window_nobody_observed_and_waits_find_what_appears() {
    let mut engine = Engine::new(Fake::new());
    let report = engine
        .act(
            "Editor",
            None,
            &steps(&[
                r#"type field "Name" "Q4""#,
                r#"click button "save""#,
                r#"wait sheet "Saved""#,
                r#"wait gone button "Save""#,
            ]),
        )
        .unwrap();
    assert!(report.starts_with("# 4/4 steps · "), "{report}");
    assert_eq!(
        engine.platform().ops(),
        ["Type { text: \"Q4\", into: Some(Name) }", "Press(Save)"]
    );
}

#[test]
fn a_locator_that_finds_nothing_says_what_is_there() {
    let mut engine = Engine::new(Fake::new());
    let report = engine
        .act("Editor", None, &steps(&[r#"click button "Export""#]))
        .unwrap();
    assert!(
        report.contains(r#"stopped at step 1 (`click button "Export"`): not found: no button "Export" in the window; it has b1 button "Save""#),
        "{report}"
    );
    assert!(engine.platform().ops().is_empty());
}

#[test]
fn an_index_past_the_last_match_is_not_there() {
    let mut engine = Engine::new(Fake::new());
    let waited = engine
        .act("Editor", None, &steps(&["wait button[2] within 100ms"]))
        .unwrap();
    assert!(waited.contains("still waiting"), "{waited}");
    let gone = engine
        .act(
            "Editor",
            None,
            &steps(&["wait gone button[2] within 100ms"]),
        )
        .unwrap();
    assert!(gone.starts_with("# 1/1 steps"), "{gone}");
}

#[test]
fn a_test_file_runs_its_steps_and_checks_in_a_fresh_session() {
    let mut engine = Engine::new(Fake::new());
    engine.observe("Editor", ObserveOptions::default()).unwrap();
    let run = engine
        .test(
            "# Saves the doc.\napp Editor\ntype field \"Name\" \"Q4\"\nexpect field \"Name\" = \"Q4\"\nclick button \"Save\"\nexpect sheet \"Saved\"\nexpect gone button \"Save\"\n",
            None,
        )
        .unwrap();
    assert!(run.failure.is_none(), "{run:?}");
    assert_eq!((run.ran, run.of), (6, 6));
    // The session the test started with was forgotten, so this is a first look.
    let after = engine.observe("Editor", ObserveOptions::default()).unwrap();
    assert!(after.starts_with("# Editor · window"), "{after}");
}

#[test]
fn a_failed_check_says_what_the_window_shows_instead() {
    let mut engine = Engine::new(Fake::new());
    let run = engine
        .test(
            "app Editor\ntype field \"Name\" \"Q4\"\nexpect field \"Name\" = \"Q5\" within 100ms\nclick button \"Save\"\n",
            None,
        )
        .unwrap();
    let failure = run.failure.expect("the check fails");
    assert_eq!(failure.line, 3);
    assert_eq!(
        failure.reason,
        r#"expected = "Q5", but t1 field "Name" = "Q4""#
    );
    assert!(failure.outline.unwrap().contains("b1 button \"Save\""));
    assert_eq!(run.ran, 2);
    assert!(!engine.platform().ops().iter().any(|op| op == "Press(Save)"));
}

#[test]
fn a_test_file_with_a_mistake_runs_nothing() {
    let mut engine = Engine::new(Fake::new());
    let error = engine
        .test("app Editor\nclick b1\n", None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("line 2"), "{error}");
    assert!(engine.platform().ops().is_empty());
}

#[test]
fn an_agents_run_saves_as_a_test_that_replays_in_a_new_session() {
    let mut agent = Engine::new(Fake::new());
    agent.observe("Editor", ObserveOptions::default()).unwrap();
    agent
        .act("Editor", None, &steps(&["type t1 \"Q4\"", "click b1"]))
        .unwrap();
    agent.act("Editor", None, &steps(&["wait e2"])).unwrap();
    let saved = agent.save().unwrap();
    assert_eq!(
        saved,
        "# Saved by hush save. Run it with hush test; add expect lines to check results.\n\
         app Editor\n\
         type field \"Name\" \"Q4\"\n\
         click button \"Save\"\n\
         wait sheet \"Saved\"\n"
    );
    assert!(agent.save().is_err(), "a save takes what it wrote");

    let mut replay = Engine::new(Fake::new());
    let run = replay.test(&saved, None).unwrap();
    assert!(run.failure.is_none(), "{run:?}");
    assert_eq!(
        replay.platform().ops(),
        ["Type { text: \"Q4\", into: Some(Name) }", "Press(Save)"]
    );
}

#[test]
fn a_read_only_app_can_be_observed_but_not_acted_on() {
    let mut engine = Engine::new(Fake::new());
    engine.set_limits(hush_core::Limits::parse("read Editor").unwrap());
    assert!(engine.observe("Editor", ObserveOptions::default()).is_ok());
    let refused = engine
        .act("Editor", None, &steps(&["click b1"]))
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("Editor is read-only for agents"),
        "{refused}"
    );
    assert!(
        engine
            .script("Editor", "beep", ScriptLanguage::AppleScript)
            .is_err()
    );
    assert!(engine.platform().ops().is_empty());
    assert!(
        engine
            .apps(None)
            .unwrap()
            .contains("Editor · running · read-only")
    );
}

#[test]
fn an_app_off_limits_is_neither_read_nor_started() {
    let fake = Fake::new();
    fake.world.borrow_mut().stopped = true;
    let mut engine = Engine::new(fake);
    engine.set_limits(hush_core::Limits::parse("only Calculator").unwrap());
    let refused = engine
        .observe("Editor", ObserveOptions::default())
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("Editor is off limits to agents"),
        "{refused}"
    );
    assert!(engine.platform().ops().is_empty(), "it wasn't launched");
}

#[test]
fn limits_turn_scripts_off_since_a_script_can_reach_any_app() {
    let mut engine = Engine::new(Fake::new());
    engine.set_limits(hush_core::Limits::parse("never Messages").unwrap());
    let refused = engine
        .script(
            "Editor",
            "tell application \"Messages\" to quit",
            ScriptLanguage::AppleScript,
        )
        .unwrap_err()
        .to_string();
    assert!(refused.contains("scripts are off"), "{refused}");
    engine.set_limits(hush_core::Limits::default());
    assert!(
        engine
            .script("Editor", "beep", ScriptLanguage::AppleScript)
            .is_ok()
    );
}

#[test]
fn steps_after_one_that_opens_a_window_go_to_that_window() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().opens_on_cmd_n = true;
    let report = engine
        .act(
            "Editor",
            None,
            &steps(&["key cmd+n", "type field \"Name\" \"Q4\""]),
        )
        .unwrap();
    assert!(report.starts_with("# 2/2 steps"), "{report}");
    assert!(
        report.contains("# step 1 opened window \"Doc 2\"; the steps after it ran there"),
        "{report}"
    );
    let world = engine.platform().world.borrow();
    assert_eq!(world.second.as_deref(), Some("Q4"));
    assert_eq!(world.name, "", "the first window wasn't typed into");
}

#[test]
fn a_named_window_keeps_the_batch_even_when_another_opens() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().opens_on_cmd_n = true;
    engine
        .act(
            "Editor",
            Some("Doc"),
            &steps(&["key cmd+n", "type field \"Name\" \"Q4\""]),
        )
        .unwrap();
    let world = engine.platform().world.borrow();
    assert_eq!(world.name, "Q4");
    assert_eq!(world.second.as_deref(), Some(""));
}

#[test]
fn a_tests_movie_follows_its_steps_to_a_window_they_open() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().opens_on_cmd_n = true;
    let run = engine
        .test(
            "app Editor\nkey cmd+n\ntype field \"Name\" \"Q4\"\nexpect field \"Name\" = \"Q5\" within 50ms\n",
            Some("/tmp/never-written.mp4"),
        )
        .unwrap();
    assert!(run.failure.is_some());
    let records: Vec<String> = engine
        .platform()
        .ops()
        .into_iter()
        .filter(|op| op.starts_with("Record"))
        .collect();
    assert_eq!(
        records,
        [
            "RecordStart(7)",
            "RecordCancel(7)",
            "RecordStart(8)",
            "RecordStop(8)"
        ]
    );
}

#[test]
fn a_recording_the_agent_started_stays_on_its_window() {
    let mut engine = Engine::new(Fake::new());
    engine.platform().world.borrow_mut().opens_on_cmd_n = true;
    engine.record_start("Editor", None).unwrap();
    engine
        .act(
            "Editor",
            None,
            &steps(&["key cmd+n", "type field \"Name\" \"Q4\""]),
        )
        .unwrap();
    engine
        .record_stop(None, Some("/tmp/never-written.mp4"))
        .unwrap();
    let records: Vec<String> = engine
        .platform()
        .ops()
        .into_iter()
        .filter(|op| op.starts_with("Record"))
        .collect();
    assert_eq!(records, ["RecordStart(7)", "RecordStop(7)"]);
}
