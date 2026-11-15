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
    /// Elements still there but in a new place among the rest (a reordered
    /// list), each with the ref now just before it (`None`: now first).
    pub moved: Vec<(Line, Option<Ref>)>,
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
            moved: moved(old, new),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.header.is_none()
            && self.focus.is_none()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.added.is_empty()
            && self.moved.is_empty()
    }

    /// True when most of the window changed (a new page, a new pane), so the
    /// full outline is shorter to read than the list of changes.
    pub fn replaces(&self, new: &View) -> bool {
        let touched = self.removed.len() + self.changed.len() + self.added.len() + self.moved.len();
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
        for (line, after) in &self.moved {
            match after {
                Some(r) => {
                    let _ = writeln!(out, "↕ {} {} · now after {r}", line.r, line.head);
                }
                None => {
                    let _ = writeln!(out, "↕ {} {} · now first", line.r, line.head);
                }
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

/// Lines in both views whose order among the others changed. The lines
/// that keep their relative order are the longest run in increasing old
/// position; everything else in both views moved.
fn moved(old: &View, new: &View) -> Vec<(Line, Option<Ref>)> {
    let position: HashMap<Ref, usize> = old
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| (line.r, i))
        .collect();
    let common: Vec<(usize, usize)> = new
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| position.get(&line.r).map(|&was| (i, was)))
        .collect();
    let stays = longest_increasing(&common.iter().map(|&(_, was)| was).collect::<Vec<_>>());
    common
        .iter()
        .enumerate()
        .filter(|(k, _)| !stays[*k])
        .map(|(_, &(i, _))| {
            let after = i.checked_sub(1).map(|p| new.lines[p].r);
            (new.lines[i].clone(), after)
        })
        .collect()
}

/// Marks one longest strictly increasing subsequence of `values`.
fn longest_increasing(values: &[usize]) -> Vec<bool> {
    // tails[k]: index of the smallest last value of an increasing run of length k + 1.
    let mut tails: Vec<usize> = Vec::new();
    let mut previous: Vec<Option<usize>> = vec![None; values.len()];
    for (i, &v) in values.iter().enumerate() {
        let k = tails.partition_point(|&t| values[t] < v);
        previous[i] = k.checked_sub(1).map(|k| tails[k]);
        if k == tails.len() {
            tails.push(i);
        } else {
            tails[k] = i;
        }
    }
    let mut keep = vec![false; values.len()];
    let mut at = tails.last().copied();
    while let Some(i) = at {
        keep[i] = true;
        at = previous[i];
    }
    keep
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
    fn a_reordered_list_reports_only_what_moved() {
        let mut table = RefTable::default();
        let options = OutlineOptions::default();
        let rows = |order: [(u32, &str); 4]| {
            let mut nodes = vec![node(1, Role::Window, "W", None)];
            nodes.extend(order.map(|(h, name)| node(h, Role::Row, name, Some(0))));
            snapshot(nodes)
        };
        let (old, _) = View::build(
            &rows([(2, "Send"), (3, "Print"), (4, "Update"), (5, "Order")]),
            &mut table,
            &options,
        );
        let (new, _) = View::build(
            &rows([(3, "Print"), (2, "Send"), (4, "Update"), (5, "Order")]),
            &mut table,
            &options,
        );
        let diff = Diff::between(&old, &new);
        // A swap reads as either row moving; one line is enough.
        assert_eq!(diff.render(), "↕ i2 row \"Print\" · now after e1\n");

        let (last, _) = View::build(
            &rows([(5, "Order"), (3, "Print"), (2, "Send"), (4, "Update")]),
            &mut table,
            &options,
        );
        assert_eq!(
            Diff::between(&new, &last).render(),
            "↕ i4 row \"Order\" · now after e1\n"
        );
    }

    #[test]
    fn longest_increasing_keeps_the_most_in_place() {
        assert_eq!(longest_increasing(&[1, 0, 2, 3]), [false, true, true, true]);
        assert_eq!(longest_increasing(&[3, 0, 1, 2]), [false, true, true, true]);
        assert_eq!(longest_increasing(&[]), Vec::<bool>::new());
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
