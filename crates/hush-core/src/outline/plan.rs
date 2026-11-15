//! Which nodes of a snapshot get a line, at what depth and under which name.

use super::OutlineOptions;
use crate::model::{Node, Rect, Role, Snapshot, States};

/// Which nodes get a line, at what depth and under which name.
pub(super) struct Plan {
    pub(super) include: Vec<bool>,
    pub(super) depth: Vec<u16>,
    /// Names made up for unnamed rows from the text inside them.
    pub(super) names: Vec<Option<String>>,
    pub(super) hidden_rows: Vec<usize>,
    pub(super) offscreen: Vec<usize>,
    /// Text already folded into a row's name.
    consumed: Vec<bool>,
}

impl Plan {
    fn name_of<'s, H>(&'s self, index: usize, snapshot: &'s Snapshot<H>) -> &'s str {
        self.names[index]
            .as_deref()
            .unwrap_or(snapshot.nodes[index].name.trim())
    }

    pub(super) fn new<H>(snapshot: &Snapshot<H>, options: &OutlineOptions) -> Plan {
        let count = snapshot.nodes.len();
        let mut plan = Plan {
            include: vec![false; count],
            depth: vec![0; count],
            names: vec![None; count],
            hidden_rows: vec![0; count],
            offscreen: vec![0; count],
            consumed: vec![false; count],
        };
        if count == 0 {
            return plan;
        }
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: snapshot.window.frame.w,
            h: snapshot.window.frame.h,
        };
        let mut walk = Walk {
            snapshot,
            options,
            bounds,
            plan: &mut plan,
        };
        walk.visit(
            0,
            Context {
                anchor: None,
                depth: 0,
                parent_name: "",
                in_control: false,
            },
        );
        plan.merge_text_runs(snapshot);
        plan
    }

    /// Consecutive text lines under the same line read as one: a
    /// paragraph, a chat message, a grid of letters. A run keeps the ref of
    /// its first text.
    fn merge_text_runs<H>(&mut self, snapshot: &Snapshot<H>) {
        let mut previous: Option<usize> = None;
        for index in 0..snapshot.nodes.len() {
            if !self.include[index] {
                continue;
            }
            let is_text = snapshot.nodes[index].role == Role::Text;
            if let Some(head) = previous
                && is_text
                && snapshot.nodes[head].role == Role::Text
                && self.depth[head] == self.depth[index]
                && self.line_parent(snapshot, head) == self.line_parent(snapshot, index)
            {
                let joined = format!(
                    "{} {}",
                    self.name_of(head, snapshot),
                    snapshot.nodes[index].name.trim()
                );
                self.names[head] = Some(joined);
                self.include[index] = false;
                continue;
            }
            previous = Some(index);
        }
    }

    fn line_parent<H>(&self, snapshot: &Snapshot<H>, index: usize) -> Option<usize> {
        let mut current = snapshot.nodes[index].parent;
        while let Some(parent) = current {
            if self.include[parent] {
                return Some(parent);
            }
            current = snapshot.nodes[parent].parent;
        }
        None
    }
}

struct Walk<'a, H> {
    snapshot: &'a Snapshot<H>,
    options: &'a OutlineOptions,
    bounds: Rect,
    plan: &'a mut Plan,
}

/// What a node inherits from the lines above it.
#[derive(Clone, Copy)]
struct Context<'n> {
    /// The nearest included ancestor, which elision counts attach to.
    anchor: Option<usize>,
    depth: u16,
    /// The name a child would repeat if it were a duplicate.
    parent_name: &'n str,
    /// Inside a button, link or other control, whose name already says
    /// what its icons and labels show.
    in_control: bool,
}

impl<H> Walk<'_, H> {
    fn visit(&mut self, index: usize, context: Context<'_>) {
        let anchor = context.anchor;
        let node = &self.snapshot.nodes[index];
        let is_root = index == 0;
        if self.plan.consumed[index] || (!is_root && node.states.has(States::HIDDEN)) {
            return;
        }
        let outside = node.states.has(States::OFFSCREEN)
            || node
                .frame
                .is_some_and(|f| !f.is_empty() && !f.intersects(&self.bounds));
        if !is_root && outside {
            if let Some(anchor) = anchor {
                self.plan.offscreen[anchor] += 1;
            }
            return;
        }
        // A zero-size control can't be seen or clicked; a zero-size wrapper
        // may still hold visible children, so it is walked through.
        let empty = node.frame.is_some_and(|f| f.is_empty());
        if !is_root && empty && !node.role.is_transparent() && node.role != Role::Row {
            return;
        }

        let mut name = node.name.trim().to_owned();
        if node.role == Role::Row && name.is_empty() {
            name = self.row_label(index);
        }
        let include = is_root || worth_a_line(node, &name, &context);
        if include {
            self.plan.include[index] = true;
            self.plan.depth[index] = context.depth;
            if name != node.name {
                self.plan.names[index] = Some(name.clone());
            }
        }

        if node.unread > 0 {
            let owner = if include { Some(index) } else { anchor };
            self.plan.hidden_rows[owner.unwrap_or(index)] += node.unread;
        }

        let inner = if include {
            Context {
                anchor: Some(index),
                depth: context.depth + 1,
                parent_name: &name,
                in_control: context.in_control || node.role.is_control(),
            }
        } else {
            Context {
                in_control: context.in_control || node.role.is_control(),
                ..context
            }
        };
        let is_rows_container = matches!(node.role, Role::List | Role::Table | Role::Outline);
        let mut rows = 0;
        let mut previous_label = String::new();
        for &child in self.snapshot.children(index) {
            let child_node = &self.snapshot.nodes[child];
            if is_rows_container && child_node.role == Role::Row {
                rows += 1;
                if rows > self.options.max_rows {
                    self.plan.hidden_rows[inner.anchor.unwrap_or(index)] += 1;
                    continue;
                }
            }
            // An icon and its caption often carry the same words.
            if matches!(child_node.role, Role::Text | Role::Image) {
                let label = child_node.name.trim();
                if !label.is_empty() && label == previous_label {
                    continue;
                }
                previous_label = label.to_owned();
            }
            self.visit(child, inner);
        }
    }

    /// An unnamed row reads as the first three texts inside it, so
    /// `row "Invoice.pdf · 2 MB"` replaces a row plus its text lines.
    fn row_label(&mut self, row: usize) -> String {
        let mut parts: Vec<String> = Vec::new();
        self.collect_text(row, &mut parts);
        parts.join(" · ")
    }

    fn collect_text(&mut self, index: usize, parts: &mut Vec<String>) {
        for &child in self.snapshot.children(index) {
            let node = &self.snapshot.nodes[child];
            match node.role {
                Role::Text | Role::Heading => {
                    let text = node.name.trim();
                    if !text.is_empty() && parts.len() < 3 && !parts.iter().any(|p| p == text) {
                        parts.push(text.to_owned());
                    }
                    self.plan.consumed[child] = true;
                }
                Role::Cell | Role::Group | Role::Other => self.collect_text(child, parts),
                _ => {}
            }
        }
    }
}

fn worth_a_line<H>(node: &Node<H>, name: &str, context: &Context<'_>) -> bool {
    let repeats_parent = name == context.parent_name;
    match node.role {
        // Web apps often label a group with all the text inside it (a chat
        // message, a card); the text lines would say it all again.
        Role::Text | Role::Heading | Role::Image => {
            !context.in_control && !name.is_empty() && !context.parent_name.contains(name)
        }
        role if role.is_transparent() => !context.in_control && !name.is_empty() && !repeats_parent,
        role if role.is_control() && context.in_control => !repeats_parent,
        _ => true,
    }
}
