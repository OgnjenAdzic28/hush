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
        walk.visit(0, None, 0, "");
        plan
    }
}

struct Walk<'a, H> {
    snapshot: &'a Snapshot<H>,
    options: &'a OutlineOptions,
    bounds: Rect,
    plan: &'a mut Plan,
}

impl<H> Walk<'_, H> {
    /// `anchor` is the nearest included ancestor (for elision counts),
    /// `parent_name` the name a child would repeat if it were a duplicate.
    fn visit(&mut self, index: usize, anchor: Option<usize>, depth: u16, parent_name: &str) {
        let node = &self.snapshot.nodes[index];
        let is_root = index == 0;
        if self.plan.consumed[index] || (!is_root && node.states.has(States::HIDDEN)) {
            return;
        }
        if !is_root && let Some(frame) = node.frame {
            if frame.is_empty() && node.role != Role::Row {
                return;
            }
            if !frame.is_empty() && !frame.intersects(&self.bounds) {
                if let Some(anchor) = anchor {
                    self.plan.offscreen[anchor] += 1;
                }
                return;
            }
        }

        let mut name = node.name.trim().to_owned();
        if node.role == Role::Row && name.is_empty() {
            name = self.row_label(index);
        }
        let include = is_root || self.worth_a_line(node, &name, parent_name);
        if include {
            self.plan.include[index] = true;
            self.plan.depth[index] = depth;
            if name != node.name {
                self.plan.names[index] = Some(name.clone());
            }
        }

        let (child_anchor, child_depth, child_parent_name) = if include {
            (Some(index), depth + 1, name.as_str())
        } else {
            (anchor, depth, parent_name)
        };
        let is_rows_container = matches!(node.role, Role::List | Role::Table | Role::Outline);
        let mut rows = 0;
        let mut previous_text = String::new();
        for &child in self.snapshot.children(index) {
            let child_node = &self.snapshot.nodes[child];
            if is_rows_container && child_node.role == Role::Row {
                rows += 1;
                if rows > self.options.max_rows {
                    self.plan.hidden_rows[child_anchor.unwrap_or(index)] += 1;
                    continue;
                }
            }
            if child_node.role == Role::Text {
                let text = child_node.name.trim();
                if text == previous_text {
                    continue;
                }
                previous_text = text.to_owned();
            }
            self.visit(child, child_anchor, child_depth, child_parent_name);
        }
    }

    fn worth_a_line(&self, node: &Node<H>, name: &str, parent_name: &str) -> bool {
        match node.role {
            Role::Text | Role::Heading | Role::Image => !name.is_empty() && name != parent_name,
            role if role.is_transparent() => !name.is_empty() && name != parent_name,
            _ => true,
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
