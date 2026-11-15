//! Short refs (`b3`, `t1`) that stay the same while an element exists.
//!
//! Raw accessibility handles are the first key. Some toolkits (SwiftUI,
//! React, Chromium after a re-render) replace an element with an identical
//! new one; those get their old ref back when role, name, parent ref and
//! position among identical siblings all match. Without that, refs expire on
//! every change and the agent has to look again, which is the main reason raw
//! accessibility trees lose to screenshots in published comparisons.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use crate::model::{Role, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ref {
    prefix: u8,
    number: u32,
}

impl Ref {
    /// Parses `b3`: one lowercase letter, then a number from 1.
    pub fn parse(text: &str) -> Option<Ref> {
        let (&prefix, digits) = text.as_bytes().split_first()?;
        if !prefix.is_ascii_lowercase()
            || digits.is_empty()
            || !digits.iter().all(u8::is_ascii_digit)
        {
            return None;
        }
        let number: u32 = std::str::from_utf8(digits).ok()?.parse().ok()?;
        (number > 0).then_some(Ref { prefix, number })
    }
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.prefix as char, self.number)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Signature {
    role: Role,
    name: String,
    parent: Option<Ref>,
    ordinal: u32,
}

#[derive(Debug)]
struct Entry<H> {
    handle: H,
    signature: Signature,
}

/// Refs for one window. Keep one table per window for as long as the window
/// is being watched.
#[derive(Debug)]
pub struct RefTable<H> {
    by_handle: HashMap<H, Ref>,
    entries: HashMap<Ref, Entry<H>>,
    next: [u32; 26],
}

impl<H> Default for RefTable<H> {
    fn default() -> Self {
        RefTable {
            by_handle: HashMap::new(),
            entries: HashMap::new(),
            next: [0; 26],
        }
    }
}

impl<H: Clone + Eq + Hash> RefTable<H> {
    /// Gives every node marked in `include` a ref, keeping earlier refs where
    /// the element is the same. `names` overrides a node's name where the
    /// outline shows another one (an unnamed row named by its text), so a
    /// rebuilt element is matched by what the agent saw. Returns one entry per
    /// node in the snapshot. Refs of elements that are gone are forgotten.
    pub fn assign(
        &mut self,
        snapshot: &Snapshot<H>,
        include: &[bool],
        names: &[Option<String>],
    ) -> Vec<Option<Ref>> {
        let nodes = &snapshot.nodes;
        let mut refs: Vec<Option<Ref>> = vec![None; nodes.len()];
        let mut claimed = HashSet::new();

        for (index, node) in nodes.iter().enumerate() {
            if !include[index] {
                continue;
            }
            if let Some(&known) = self.by_handle.get(&node.handle)
                && claimed.insert(known)
            {
                refs[index] = Some(known);
            }
        }

        let mut orphans: HashMap<Signature, Vec<Ref>> = HashMap::new();
        for (&known, entry) in &self.entries {
            if !claimed.contains(&known) {
                orphans
                    .entry(entry.signature.clone())
                    .or_default()
                    .push(known);
            }
        }
        // Reuse the oldest ref first when several identical elements vanished.
        for candidates in orphans.values_mut() {
            candidates.sort_unstable_by(|a, b| b.cmp(a));
        }

        let mut ordinals: HashMap<(Option<Ref>, Role, &str), u32> = HashMap::new();
        let mut entries = HashMap::with_capacity(self.entries.len());
        for (index, node) in nodes.iter().enumerate() {
            if !include[index] {
                continue;
            }
            let parent = nearest_included_ref(snapshot, &refs, index);
            let name = names[index].as_deref().unwrap_or(&node.name);
            let ordinal = ordinals.entry((parent, node.role, name)).or_insert(0);
            *ordinal += 1;
            let signature = Signature {
                role: node.role,
                name: name.to_owned(),
                parent,
                ordinal: *ordinal,
            };
            let assigned = match refs[index] {
                Some(known) => known,
                None => match orphans.get_mut(&signature).and_then(Vec::pop) {
                    Some(reused) => reused,
                    None => self.fresh(node.role),
                },
            };
            refs[index] = Some(assigned);
            entries.insert(
                assigned,
                Entry {
                    handle: node.handle.clone(),
                    signature,
                },
            );
        }

        self.by_handle = entries
            .iter()
            .map(|(&r, entry)| (entry.handle.clone(), r))
            .collect();
        self.entries = entries;
        refs
    }

    /// The handle behind a ref, if the element still existed at the last read.
    pub fn handle(&self, r: Ref) -> Option<&H> {
        self.entries.get(&r).map(|entry| &entry.handle)
    }

    fn fresh(&mut self, role: Role) -> Ref {
        let prefix = role.ref_prefix();
        let slot = &mut self.next[usize::from(prefix - b'a')];
        *slot += 1;
        Ref {
            prefix,
            number: *slot,
        }
    }
}

fn nearest_included_ref<H>(
    snapshot: &Snapshot<H>,
    refs: &[Option<Ref>],
    index: usize,
) -> Option<Ref> {
    let mut current = snapshot.nodes[index].parent;
    while let Some(parent) = current {
        if let Some(r) = refs[parent] {
            return Some(r);
        }
        current = snapshot.nodes[parent].parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{node, snapshot};

    fn all(snapshot: &Snapshot<u32>) -> Vec<bool> {
        vec![true; snapshot.nodes.len()]
    }

    fn refs_of(table: &mut RefTable<u32>, snapshot: &Snapshot<u32>) -> Vec<String> {
        table
            .assign(snapshot, &all(snapshot), &vec![None; snapshot.nodes.len()])
            .into_iter()
            .map(|r| r.map(|r| r.to_string()).unwrap_or_default())
            .collect()
    }

    #[test]
    fn parses_only_well_formed_refs() {
        assert_eq!(Ref::parse("b12").map(|r| r.to_string()), Some("b12".into()));
        for bad in ["", "b", "12", "B1", "b0", "b1x", "bb1"] {
            assert_eq!(Ref::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn same_handle_keeps_its_ref_when_others_appear() {
        let mut table = RefTable::default();
        let first = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(2, Role::Button, "Save", Some(0)),
        ]);
        assert_eq!(refs_of(&mut table, &first), ["e1", "b1"]);

        let second = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(3, Role::Button, "Cancel", Some(0)),
            node(2, Role::Button, "Save", Some(0)),
        ]);
        assert_eq!(refs_of(&mut table, &second), ["e1", "b2", "b1"]);
    }

    #[test]
    fn rebuilt_identical_element_gets_its_old_ref() {
        let mut table = RefTable::default();
        let first = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(2, Role::Button, "OK", Some(0)),
            node(3, Role::Button, "OK", Some(0)),
        ]);
        assert_eq!(refs_of(&mut table, &first), ["e1", "b1", "b2"]);

        // The toolkit replaced both buttons with new handles.
        let rebuilt = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(20, Role::Button, "OK", Some(0)),
            node(30, Role::Button, "OK", Some(0)),
        ]);
        assert_eq!(refs_of(&mut table, &rebuilt), ["e1", "b1", "b2"]);
    }

    #[test]
    fn rebuilt_unnamed_rows_follow_their_shown_name_not_their_position() {
        let mut table = RefTable::default();
        let rows = |handles: [u32; 2]| {
            snapshot(vec![
                node(1, Role::List, "", None),
                node(handles[0], Role::Row, "", Some(0)),
                node(handles[1], Role::Row, "", Some(0)),
            ])
        };
        let first = rows([2, 3]);
        let names = |a: &str, b: &str| vec![None, Some(a.to_owned()), Some(b.to_owned())];
        let all = vec![true; 3];
        let before = table.assign(&first, &all, &names("Invoice.pdf", "Notes.txt"));

        // Re-rendered with new handles and the rows in the other order.
        let rebuilt = rows([30, 20]);
        let after = table.assign(&rebuilt, &all, &names("Notes.txt", "Invoice.pdf"));
        assert_eq!(after[1], before[2]);
        assert_eq!(after[2], before[1]);
    }

    #[test]
    fn gone_elements_are_forgotten_and_not_resurrected_by_a_different_one() {
        let mut table = RefTable::default();
        let first = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(2, Role::Button, "Save", Some(0)),
        ]);
        refs_of(&mut table, &first);
        let changed = snapshot(vec![
            node(1, Role::Window, "W", None),
            node(4, Role::Button, "Delete", Some(0)),
        ]);
        assert_eq!(refs_of(&mut table, &changed), ["e1", "b2"]);
        assert_eq!(table.handle(Ref::parse("b1").unwrap()), None);
    }
}
