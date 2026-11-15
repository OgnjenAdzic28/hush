//! Fitting a big window into a token budget, and showing one part of it.
//!
//! The structure stays (toolbars, controls, the containers a person would
//! name) and bulk is folded: the middle of long lists first, keeping the
//! first few items and the last ones (in a chat or a log the newest is at
//! the end), then the largest remaining blocks. Every fold says how much it
//! hides and which ref to `observe` to open it.

use std::collections::HashMap;

use super::{Line, View};
use crate::refs::Ref;

/// Long lists keep this many items from the start and from the end.
const KEEP_FIRST: usize = 3;
const KEEP_LAST: usize = 8;
/// A list counts as long past this many items.
const LONG: usize = KEEP_FIRST + KEEP_LAST + 1;

impl View {
    /// The element and everything inside it, with depths starting at zero.
    pub fn subtree(&self, r: Ref) -> Option<View> {
        let start = self.lines.iter().position(|line| line.r == r)?;
        let base = self.lines[start].depth;
        let end = subtree_end(&self.lines, start);
        let lines: Vec<Line> = self.lines[start..end]
            .iter()
            .map(|line| Line {
                depth: line.depth - base,
                ..line.clone()
            })
            .collect();
        let inside = |r: &Ref| lines.iter().any(|line| line.r == *r);
        Some(View {
            header: self.header.clone(),
            elided: self
                .elided
                .iter()
                .filter(|(r, _)| inside(r))
                .cloned()
                .collect(),
            focused: self.focused.filter(inside),
            lines,
        })
    }

    /// The outline, folded until it fits in about `max_tokens`.
    pub fn render_within(&self, max_tokens: usize) -> String {
        let full = self.render();
        if super::approx_tokens(&full) <= max_tokens {
            return full;
        }
        let mut fold = Fold::new(self, max_tokens);
        fold.trim_long_lists();
        fold.fold_blocks(true);
        fold.fold_blocks(false);
        self.render_with(&fold.hidden, &fold.notes)
    }
}

struct Fold<'v> {
    lines: &'v [Line],
    end: Vec<usize>,
    size: Vec<usize>,
    hidden: Vec<bool>,
    notes: HashMap<usize, (u16, String)>,
    total: usize,
    target: usize,
}

impl<'v> Fold<'v> {
    fn new(view: &'v View, max_tokens: usize) -> Fold<'v> {
        let lines = &view.lines;
        let size: Vec<usize> = lines
            .iter()
            .map(|l| 2 * usize::from(l.depth) + l.head.len() + l.tail.len() + 6)
            .collect();
        Fold {
            lines,
            end: (0..lines.len()).map(|i| subtree_end(lines, i)).collect(),
            total: size.iter().sum::<usize>() + view.header.len(),
            size,
            hidden: vec![false; lines.len()],
            notes: HashMap::new(),
            target: max_tokens * 4,
        }
    }

    fn over(&self) -> bool {
        self.total > self.target
    }

    fn children(&self, index: usize) -> Vec<usize> {
        let depth = self.lines[index].depth + 1;
        (index + 1..self.end[index])
            .filter(|&j| !self.hidden[j] && self.lines[j].depth == depth)
            .collect()
    }

    fn hide(&mut self, from: usize, to: usize) -> usize {
        let mut hidden = 0;
        for j in from..to {
            if !self.hidden[j] {
                self.hidden[j] = true;
                self.total -= self.size[j];
                hidden += 1;
            }
            // A note inside a hidden range would point at nothing.
            self.notes.remove(&j);
        }
        hidden
    }

    /// Hides the middle of the longest lists first.
    fn trim_long_lists(&mut self) {
        while self.over() {
            let longest = (0..self.lines.len())
                .filter(|&i| !self.hidden[i])
                .map(|i| (self.children(i), i))
                .filter(|(children, _)| children.len() > LONG)
                .max_by_key(|(children, _)| children.len());
            let Some((children, container)) = longest else {
                return;
            };
            let first_hidden = children[KEEP_FIRST];
            let last_hidden_end = self.end[children[children.len() - KEEP_LAST - 1]];
            let items = children.len() - KEEP_FIRST - KEEP_LAST;
            self.hide(first_hidden, last_hidden_end);
            let r = self.lines[container].r;
            let depth = self.lines[container].depth + 1;
            self.notes.insert(
                first_hidden,
                (depth, format!("{items} more items, observe {r} to see all")),
            );
            self.total += 40;
        }
    }

    /// Folds the biggest visible block into one line. With `modest`, only
    /// blocks under half of what's left qualify, so a fold never swallows
    /// most of the window while smaller ones would do.
    fn fold_blocks(&mut self, modest: bool) {
        while self.over() {
            let candidate = (0..self.lines.len())
                .filter(|&i| !self.hidden[i] && self.lines[i].depth > 0 && self.end[i] > i + 1)
                .map(|i| (self.inside(i), i))
                .filter(|&(inside, _)| inside > 0 && (!modest || inside * 2 < self.total))
                .max_by_key(|&(inside, _)| inside);
            let Some((_, block)) = candidate else { return };
            let count = self.hide(block + 1, self.end[block]);
            let r = self.lines[block].r;
            let depth = self.lines[block].depth + 1;
            self.notes.insert(
                block + 1,
                (
                    depth,
                    format!("{count} lines inside, observe {r} to see them"),
                ),
            );
            self.total += 40;
        }
    }

    /// Characters of the visible lines inside `index`.
    fn inside(&self, index: usize) -> usize {
        (index + 1..self.end[index])
            .filter(|&j| !self.hidden[j])
            .map(|j| self.size[j])
            .sum()
    }
}

fn subtree_end(lines: &[Line], start: usize) -> usize {
    let depth = lines[start].depth;
    lines[start + 1..]
        .iter()
        .position(|line| line.depth <= depth)
        .map_or(lines.len(), |offset| start + 1 + offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Role;
    use crate::outline::OutlineOptions;
    use crate::refs::RefTable;
    use crate::test_support::{node, snapshot};

    fn chat(messages: usize) -> View {
        let mut nodes = vec![
            node(1, Role::Window, "Chat", None),
            node(2, Role::Toolbar, "", Some(0)),
            node(3, Role::Button, "Back", Some(1)),
            node(4, Role::Group, "Messages", Some(0)),
        ];
        for i in 0..messages {
            let at = nodes.len();
            nodes.push(node(
                100 + i as u32 * 2,
                Role::Group,
                &format!("message {i}"),
                Some(3),
            ));
            nodes.push(node(
                101 + i as u32 * 2,
                Role::Button,
                &format!("React to message {i}"),
                Some(at),
            ));
        }
        nodes.push(node(9, Role::TextArea, "Message", Some(0)));
        View::build(
            &snapshot(nodes),
            &mut RefTable::default(),
            &OutlineOptions::default(),
        )
        .0
    }

    #[test]
    fn a_view_that_fits_is_unchanged() {
        let view = chat(3);
        assert_eq!(view.render_within(10_000), view.render());
    }

    #[test]
    fn long_lists_keep_their_ends_and_say_how_to_see_the_rest() {
        let text = chat(200).render_within(400);
        assert!(text.contains("button \"Back\""), "{text}");
        assert!(text.contains("textarea \"Message\""), "{text}");
        assert!(text.contains("message 0\""), "{text}");
        assert!(text.contains("message 199\""), "{text}");
        assert!(!text.contains("message 100\""), "{text}");
        assert!(
            text.contains("… 189 more items, observe e3 to see all"),
            "{text}"
        );
        assert!(
            crate::approx_tokens(&text) <= 400,
            "{}",
            crate::approx_tokens(&text)
        );
    }

    #[test]
    fn subtree_shows_one_part_from_depth_zero() {
        let view = chat(2);
        let messages = view
            .lines
            .iter()
            .find(|l| l.head.contains("Messages"))
            .unwrap()
            .r;
        let part = view.subtree(messages).unwrap();
        assert_eq!(part.lines.len(), 5);
        assert_eq!(part.lines[0].depth, 0);
        assert!(!part.render().contains("Back"));
    }
}
