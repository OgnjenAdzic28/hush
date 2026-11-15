//! What Hush remembers about one window between calls: the last snapshot,
//! its refs and the view the agent last saw.

use std::collections::HashMap;
use std::hash::Hash;

use hush_core::{Node, OutlineOptions, Ref, RefTable, Snapshot, View};

#[derive(Debug)]
pub(crate) struct Mirror<H> {
    refs: RefTable<H>,
    snapshot: Snapshot<H>,
    /// Snapshot index of each ref in the current view.
    index: HashMap<Ref, usize>,
    /// What the agent saw last; diffs are against this.
    pub(crate) seen: View,
    /// The view of the latest read (may be newer than `seen`).
    pub(crate) current: View,
}

impl<H: Clone + Eq + Hash> Mirror<H> {
    pub(crate) fn new(snapshot: Snapshot<H>, options: &OutlineOptions) -> Mirror<H> {
        let mut refs = RefTable::default();
        let (current, assigned) = View::build(&snapshot, &mut refs, options);
        Mirror {
            index: index_of(&assigned),
            refs,
            snapshot,
            seen: View::default(),
            current,
        }
    }

    /// Replaces the snapshot, keeping refs of elements that are still there.
    pub(crate) fn update(&mut self, snapshot: Snapshot<H>, options: &OutlineOptions) {
        let (current, assigned) = View::build(&snapshot, &mut self.refs, options);
        self.index = index_of(&assigned);
        self.snapshot = snapshot;
        self.current = current;
    }

    pub(crate) fn node(&self, r: Ref) -> Option<&Node<H>> {
        self.index.get(&r).map(|&i| &self.snapshot.nodes[i])
    }

    pub(crate) fn snapshot(&self) -> &Snapshot<H> {
        &self.snapshot
    }

    /// Refs of the current view, with their nodes.
    pub(crate) fn nodes(&self) -> impl Iterator<Item = (Ref, &Node<H>)> {
        self.index
            .iter()
            .map(|(&r, &i)| (r, &self.snapshot.nodes[i]))
    }

    pub(crate) fn mark_seen(&mut self) {
        self.seen = self.current.clone();
    }
}

fn index_of(assigned: &[Option<Ref>]) -> HashMap<Ref, usize> {
    assigned
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.map(|r| (r, i)))
        .collect()
}
