//! What changed between two views of the same window, keyed by ref.

use std::collections::HashMap;
use std::fmt::Write;

use crate::outline::{Line, View, render_line};
use crate::refs::Ref;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub header: Option<String>,
    pub focus: Option<Option<Ref>>,
    pub removed: Vec<Line>,
    pub changed: Vec<(Line, Line)>,
    pub added: Vec<Line>,
}

impl Diff {
    pub fn between(old: &View, new: &View) -> Diff {
        let before: HashMap<Ref, &Line> = old.lines.iter().map(|line| (line.r, line)).collect();
        let after: HashMap<Ref, &Line> = new.lines.iter().map(|line| (line.r, line)).collect();

        let removed = old
            .lines
            .iter()
            .filter(|line| !after.contains_key(&line.r))
            .cloned()
            .collect();
        let mut changed = Vec::new();
        let mut added = Vec::new();
        for line in &new.lines {
            match before.get(&line.r) {
                None => added.push(line.clone()),
                Some(previous) if previous.head != line.head || previous.tail != line.tail => {
                    changed.push(((*previous).clone(), line.clone()));
                }
                Some(_) => {}
            }
        }
        Diff {
            header: (old.header != new.header).then(|| new.header.clone()),
            focus: (old.focused != new.focused).then_some(new.focused),
            removed,
            changed,
            added,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.header.is_none()
            && self.focus.is_none()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.added.is_empty()
    }

    /// True when most of the window changed (a new page, a new pane), so the
    /// full outline is shorter to read than the list of changes.
    pub fn replaces(&self, new: &View) -> bool {
        let touched = self.removed.len() + self.changed.len() + self.added.len();
        touched > 8 && touched * 2 > new.lines.len()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Some(header) = &self.header {
            out.push_str(header);
            out.push('\n');
        }
        if let Some(focus) = self.focus {
            match focus {
                Some(r) => {
                    let _ = writeln!(out, "# focus {r}");
                }
                None => out.push_str("# focus none\n"),
            }
        }
        for line in &self.removed {
            let _ = writeln!(out, "- {} {}", line.r, line.head);
        }
        for (old, new) in &self.changed {
            if old.head == new.head {
                let _ = writeln!(
                    out,
                    "~ {} {}:{} →{}",
                    new.r,
                    new.head,
                    or_dash(&old.tail),
                    or_dash(&new.tail)
                );
            } else {
                let _ = writeln!(
                    out,
                    "~ {} {}{} (was {})",
                    new.r, new.head, new.tail, old.head
                );
            }
        }
        let base = self.added.iter().map(|line| line.depth).min().unwrap_or(0);
        for line in &self.added {
            let relative = Line {
                depth: line.depth - base,
                ..line.clone()
            };
            render_line(&mut out, &relative, "+ ");
        }
        out
    }
}

fn or_dash(tail: &str) -> &str {
    if tail.is_empty() { " -" } else { tail }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Role, States};
    use crate::outline::OutlineOptions;
    use crate::refs::RefTable;
    use crate::test_support::{node, snapshot};

    #[test]
    fn reports_only_what_changed() {
        let mut table = RefTable::default();
        let options = OutlineOptions::default();
        let mut field = node(2, Role::TextField, "Name", Some(0));
        field.value = Some("Q".into());
        let before = snapshot(vec![
            node(1, Role::Window, "W", None),
            field.clone(),
            node(3, Role::Button, "Bold", Some(0)),
        ]);
        let (old, _) = View::build(&before, &mut table, &options);

        field.value = Some("Q4".into());
        let mut save = node(4, Role::Button, "Save", Some(2));
        save.states = States::DISABLED;
        let after = snapshot(vec![
            node(1, Role::Window, "W", None),
            field,
            node(5, Role::Sheet, "Save changes?", Some(0)),
            save,
        ]);
        let (new, _) = View::build(&after, &mut table, &options);

        assert_eq!(
            Diff::between(&old, &new).render(),
            "- b1 button \"Bold\"\n\
             ~ t1 field \"Name\": = \"Q\" → = \"Q4\"\n\
             + e2 sheet \"Save changes?\"\n\
             +   b2 button \"Save\" [disabled]\n"
        );
    }

    #[test]
    fn identical_views_have_no_diff() {
        let ui = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(2, Role::Button, "OK", Some(0)),
        ]);
        let mut table = RefTable::default();
        let (a, _) = View::build(&ui, &mut table, &OutlineOptions::default());
        let (b, _) = View::build(&ui, &mut table, &OutlineOptions::default());
        assert!(Diff::between(&a, &b).is_empty());
    }
}
