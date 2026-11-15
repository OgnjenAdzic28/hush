//! The compact text an agent reads instead of a raw tree or a screenshot.
//!
//! One line per element worth acting on or reading, indented under the
//! containers that give it meaning. Decoration, unnamed wrappers, duplicate
//! labels and anything outside the window are dropped. Lines are byte-stable
//! for the same UI so prompt caching keeps working across calls.

use std::fmt::Write;
use std::hash::Hash;

use crate::model::{Role, Snapshot, States};
use crate::refs::{Ref, RefTable};

mod plan;
mod text;

use plan::Plan;
use text::{head, header, tail};

#[derive(Clone, Debug)]
pub struct OutlineOptions {
    /// Rows shown per list, table or tree before the rest are counted.
    pub max_rows: usize,
    /// Characters of a name or value before it is cut.
    pub max_text: usize,
}

impl Default for OutlineOptions {
    fn default() -> Self {
        OutlineOptions {
            max_rows: 25,
            max_text: 80,
        }
    }
}

/// One rendered element. `head` names it, `tail` is its current value and
/// state; a diff reports a changed tail as `old → new`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub r: Ref,
    pub depth: u16,
    pub head: String,
    pub tail: String,
}

/// A snapshot as the agent sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub header: String,
    pub lines: Vec<Line>,
    /// Elements left out, as `(ref of the container, text)`.
    pub elided: Vec<(Ref, String)>,
    pub focused: Option<Ref>,
}

impl View {
    /// Builds the view and assigns refs. The returned refs are indexed like
    /// `snapshot.nodes`, so the caller can map a ref back to a node.
    pub fn build<H: Clone + Eq + Hash>(
        snapshot: &Snapshot<H>,
        refs: &mut RefTable<H>,
        options: &OutlineOptions,
    ) -> (View, Vec<Option<Ref>>) {
        let plan = Plan::new(snapshot, options);
        let assigned = refs.assign(snapshot, &plan.include, &plan.names);

        let mut view = View {
            header: header(snapshot),
            ..View::default()
        };
        for (index, node) in snapshot.nodes.iter().enumerate() {
            let Some(r) = assigned[index] else { continue };
            let name = plan.names[index].as_deref().unwrap_or(&node.name);
            view.lines.push(Line {
                r,
                depth: plan.depth[index],
                head: head(node.role, name, options.max_text),
                tail: tail(node, name, options.max_text),
            });
            if node.states.has(States::FOCUSED) {
                view.focused = Some(r);
            }
            if plan.hidden_rows[index] > 0 {
                let what = match node.role {
                    Role::List | Role::Table | Role::Outline => "rows",
                    _ => "not read",
                };
                view.elided
                    .push((r, format!("{} more {what}", plan.hidden_rows[index])));
            }
            if plan.offscreen[index] > 0 {
                view.elided.push((
                    r,
                    format!("{} more outside the window", plan.offscreen[index]),
                ));
            }
        }
        (view, assigned)
    }

    pub fn render(&self) -> String {
        let mut out = self.header.clone();
        if let Some(focused) = self.focused {
            let _ = write!(out, " · focus {focused}");
        }
        out.push('\n');
        // Notes about left-out elements go after the container's last line.
        let mut open: Vec<(u16, Ref)> = Vec::new();
        for line in &self.lines {
            while let Some(&(depth, r)) = open.last() {
                if line.depth > depth {
                    break;
                }
                self.render_notes(&mut out, r, depth);
                open.pop();
            }
            render_line(&mut out, line, "");
            if self.elided.iter().any(|(r, _)| *r == line.r) {
                open.push((line.depth, line.r));
            }
        }
        while let Some((depth, r)) = open.pop() {
            self.render_notes(&mut out, r, depth);
        }
        out
    }

    fn render_notes(&self, out: &mut String, container: Ref, depth: u16) {
        let indent = "  ".repeat(usize::from(depth) + 1);
        for (_, text) in self.elided.iter().filter(|(r, _)| *r == container) {
            let _ = writeln!(out, "{indent}… {text}");
        }
    }
}

pub(crate) fn render_line(out: &mut String, line: &Line, marker: &str) {
    let indent = "  ".repeat(usize::from(line.depth));
    let _ = writeln!(out, "{marker}{indent}{} {}{}", line.r, line.head, line.tail);
}

/// Rough token count for budgets and reports (about four characters a token
/// for this kind of text on current tokenizers).
pub fn approx_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{framed, node, snapshot};

    fn outline(snapshot: &Snapshot<u32>) -> String {
        View::build(
            snapshot,
            &mut RefTable::default(),
            &OutlineOptions::default(),
        )
        .0
        .render()
    }

    #[test]
    fn drops_wrappers_and_duplicate_labels_and_names_rows_by_their_text() {
        let mut save = node(3, Role::Button, "Save", Some(1));
        save.states = States::DISABLED;
        let mut field = node(5, Role::TextField, "Name", Some(1));
        field.value = Some("Quarterly report".into());
        field.states = States::FOCUSED;
        let ui = snapshot(vec![
            node(1, Role::Window, "Doc", None),
            node(2, Role::Group, "", Some(0)),
            save,
            node(4, Role::Text, "Save", Some(2)),
            field,
            node(6, Role::List, "Files", Some(0)),
            node(7, Role::Row, "", Some(5)),
            node(8, Role::Cell, "", Some(6)),
            node(9, Role::Text, "Invoice.pdf", Some(7)),
            node(10, Role::Text, "2 MB", Some(7)),
            node(11, Role::Image, "", Some(0)),
        ]);
        assert_eq!(
            outline(&ui),
            "# App · window \"Doc\" 800x600 · focus t1\n\
             e1 window \"Doc\"\n\
             \x20 b1 button \"Save\" [disabled]\n\
             \x20 t1 field \"Name\" = \"Quarterly report\"\n\
             \x20 e2 list \"Files\"\n\
             \x20   i1 row \"Invoice.pdf · 2 MB\"\n"
        );
    }

    #[test]
    fn counts_rows_past_the_limit_and_elements_outside_the_window() {
        let mut nodes = vec![
            node(1, Role::Window, "W", None),
            node(2, Role::List, "", Some(0)),
        ];
        for i in 0..30 {
            nodes.push(node(10 + i, Role::Row, &format!("r{i}"), Some(1)));
        }
        nodes.push(framed(node(99, Role::Button, "Far", Some(0)), 0.0, 5000.0));
        let text = outline(&snapshot(nodes));
        assert!(text.contains("i25 row \"r24\"\n"), "{text}");
        assert!(!text.contains("r25"), "{text}");
        assert!(text.contains("    … 5 more rows\n"), "{text}");
        assert!(text.contains("  … 1 more outside the window\n"), "{text}");
    }

    #[test]
    fn folds_what_a_control_or_a_labelled_group_already_says() {
        let mut offscreen = node(9, Role::Button, "Below", Some(0));
        offscreen.states = States::OFFSCREEN;
        let mut table = node(10, Role::Table, "Mail", Some(0));
        table.unread = 40;
        let ui = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(2, Role::Link, "Inbox (3 unread)", Some(0)),
            node(3, Role::Image, "Inbox icon", Some(1)),
            node(4, Role::Text, "3", Some(1)),
            node(5, Role::Group, "alice , lunch at noon? , 9:41", Some(0)),
            node(6, Role::Text, "lunch at noon?", Some(4)),
            node(7, Role::Group, "", Some(0)),
            node(8, Role::Text, "M", Some(6)),
            offscreen,
            table,
            node(11, Role::Group, "", Some(0)),
            node(12, Role::Text, "U", Some(10)),
            node(13, Role::Group, "", Some(0)),
            node(14, Role::Text, "S", Some(12)),
        ]);
        assert_eq!(
            outline(&ui),
            "# App · window \"W\" 800x600\n\
             e1 window \"W\"\n\
             \x20 l1 link \"Inbox (3 unread)\"\n\
             \x20 e2 group \"alice , lunch at noon? , 9:41\"\n\
             \x20 e3 text \"M\"\n\
             \x20 e4 table \"Mail\"\n\
             \x20   … 40 more rows\n\
             \x20 e5 text \"U S\"\n\
             \x20 … 1 more outside the window\n"
        );
    }

    #[test]
    fn never_prints_a_password() {
        let mut secret = node(2, Role::SecureField, "Password", Some(0));
        secret.value = Some("hunter2".into());
        let text = outline(&snapshot(vec![node(1, Role::Window, "W", None), secret]));
        assert!(text.contains("password \"Password\" = ••••"), "{text}");
        assert!(!text.contains("hunter2"));
    }

    #[test]
    fn long_values_are_cut_with_their_length() {
        let mut area = node(2, Role::TextArea, "", Some(0));
        area.value = Some("x".repeat(200));
        let text = outline(&snapshot(vec![node(1, Role::Window, "W", None), area]));
        assert!(
            text.contains(&format!("textarea = \"{}…\" (200 chars)", "x".repeat(80))),
            "{text}"
        );
    }
}
