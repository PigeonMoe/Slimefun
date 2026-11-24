// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;

type Position = [i32; 3];

const INITIAL_CAPACITY: usize = 16;
const MIN_TREE_CAPACITY: usize = 64;
const TREE_TRANSITION: usize = 9;
const UNTREEIFY_SPLIT: usize = 6;

/// Iteration order of a default Java 25 HashMap<Location, _> or HashSet<Location>.
/// The network's table survives removals and does not shrink.
#[derive(Default)]
pub(super) struct PaperLocations {
    bins: Vec<Bin>,
    size: usize,
    next_identity: u64,
}

fn location_hash(world_uuid: u128, pos: Position) -> u32 {
    let world_hash = (0..4).fold(0, |hash, word| hash ^ ((world_uuid >> (word * 32)) as u32));
    let fold = |hash: u32, word| hash.wrapping_mul(19).wrapping_add(word);
    let mut hash = fold(3, world_hash);
    for coordinate in pos {
        let bits = f64::from(coordinate).to_bits();
        hash = fold(hash, (bits ^ (bits >> 32)) as u32);
    }
    hash = fold(fold(hash, 0), 0); // pitch and yaw of a block's Location
    hash ^ (hash >> 16)
}

impl PaperLocations {
    pub(super) fn insert(&mut self, world_uuid: u128, pos: Position) {
        self.insert_hashed(location_hash(world_uuid, pos), pos);
    }

    fn insert_hashed(&mut self, hash: u32, pos: Position) {
        let entry = Entry {
            pos,
            hash,
            identity: self.next_identity,
        };
        if self.bins.is_empty() {
            self.grow();
        }
        let index = self.index(entry.hash);
        let tree_transition = match &mut self.bins[index] {
            Bin::List(entries) => {
                if entries.iter().any(|e| e.pos == entry.pos) {
                    return;
                }
                entries.push(entry);
                entries.len() >= TREE_TRANSITION
            }
            Bin::Tree(tree) => {
                if tree.nodes.contains_key(&entry.pos) {
                    return;
                }
                tree.insert(entry);
                false
            }
        };
        if tree_transition {
            if self.bins.len() < MIN_TREE_CAPACITY {
                self.grow();
            } else if let Bin::List(entries) = std::mem::take(&mut self.bins[index]) {
                self.bins[index] = Bin::make_tree(entries);
            }
        }
        self.next_identity += 1;
        self.size += 1;
        if self.size > self.bins.len() / 4 * 3 {
            self.grow();
        }
    }

    pub(super) fn remove(&mut self, world_uuid: u128, pos: Position) {
        self.remove_entry(location_hash(world_uuid, pos), pos);
    }

    fn remove_entry(&mut self, hash: u32, pos: Position) {
        if self.bins.is_empty() {
            return;
        }
        let index = self.index(hash);
        let bin = &mut self.bins[index];
        match bin {
            Bin::List(entries) => {
                let Some(index) = entries.iter().position(|entry| entry.pos == pos) else {
                    return;
                };
                entries.remove(index);
            }
            Bin::Tree(tree) => {
                if !tree.nodes.contains_key(&pos) {
                    return;
                }
                if tree.small_for_removal() {
                    let mut entries = tree.entries();
                    entries.retain(|entry| entry.pos != pos);
                    *bin = Bin::List(entries);
                } else {
                    tree.remove(pos);
                }
            }
        }
        self.size -= 1;
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = Position> + '_ {
        self.bins.iter().flat_map(|bin| {
            let (list, tree) = match bin {
                Bin::List(entries) => (entries.as_slice(), &[][..]),
                Bin::Tree(entries) => (&[][..], entries.order.as_slice()),
            };
            list.iter()
                .map(|entry| entry.pos)
                .chain(tree.iter().copied())
        })
    }

    fn index(&self, hash: u32) -> usize {
        hash as usize & (self.bins.len() - 1)
    }

    fn grow(&mut self) {
        let old = std::mem::take(&mut self.bins);
        let width = old.len();
        self.bins
            .resize_with((2 * width).max(INITIAL_CAPACITY), Bin::default);
        for (index, bin) in old.into_iter().enumerate() {
            let (low, high) = bin.split(width);
            self.bins[index] = low;
            self.bins[index + width] = high;
        }
    }
}

#[derive(Clone, Copy)]
struct Entry {
    pos: Position,
    hash: u32,
    identity: u64,
}

impl Entry {
    fn key(self) -> (i32, u64) {
        // Location is non-Comparable. Java uses the JVM object's identity for
        // equal full hashes in a tree bin. A migrated save cannot recover that
        // identity; use the local insertion sequence only in this ambiguous case.
        (self.hash as i32, self.identity)
    }
}

enum Bin {
    List(Vec<Entry>),
    Tree(Box<TreeBin>),
}

impl Default for Bin {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

impl Bin {
    fn make_tree(entries: Vec<Entry>) -> Self {
        let mut tree = TreeBin::default();
        for entry in entries {
            tree.append_tree_node(entry);
        }
        tree.put_root_first();
        Self::Tree(Box::new(tree))
    }

    fn split(self, old_width: usize) -> (Self, Self) {
        let (entries, tree) = match self {
            Self::List(entries) => (entries, None),
            Self::Tree(tree) => (tree.entries(), Some(tree)),
        };
        let (low, high): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|entry| entry.hash as usize & old_width == 0);
        let Some(tree) = tree else {
            return (Self::List(low), Self::List(high));
        };
        if high.is_empty() && low.len() > UNTREEIFY_SPLIT {
            return (Self::Tree(tree), Self::default());
        }
        if low.is_empty() && high.len() > UNTREEIFY_SPLIT {
            return (Self::default(), Self::Tree(tree));
        }
        let rebuild = |entries: Vec<Entry>| {
            if entries.len() <= UNTREEIFY_SPLIT {
                Self::List(entries)
            } else {
                Self::make_tree(entries)
            }
        };
        (rebuild(low), rebuild(high))
    }
}

type Link = Option<Position>;

struct TreeNode {
    entry: Entry,
    parent: Link,
    children: [Link; 2],
    red: bool,
}

/// Red-black tree for a crowded bin; iteration is through its separate linked
/// order, with the tree root first, and not an in-order tree traversal.
#[derive(Default)]
struct TreeBin {
    nodes: BTreeMap<Position, TreeNode>,
    order: Vec<Position>,
    root: Link,
}

impl TreeBin {
    fn entries(&self) -> Vec<Entry> {
        self.order.iter().map(|pos| self.nodes[pos].entry).collect()
    }

    fn parent(&self, pos: Link) -> Link {
        pos.and_then(|pos| self.nodes[&pos].parent)
    }

    fn child(&self, pos: Link, side: usize) -> Link {
        pos.and_then(|pos| self.nodes[&pos].children[side])
    }

    fn side(&self, parent: Position, child: Position) -> usize {
        usize::from(self.nodes[&parent].children[1] == Some(child))
    }

    fn red(&self, pos: Link) -> bool {
        pos.is_some_and(|pos| self.nodes[&pos].red)
    }

    fn paint(&mut self, pos: Link, red: bool) {
        if let Some(pos) = pos {
            self.nodes.get_mut(&pos).unwrap().red = red;
        }
    }

    fn attach(&mut self, parent: Link, side: usize, child: Link) {
        if let Some(parent) = parent {
            self.nodes.get_mut(&parent).unwrap().children[side] = child;
        } else {
            self.root = child;
        }
        if let Some(child) = child {
            self.nodes.get_mut(&child).unwrap().parent = parent;
        }
    }

    fn replace(&mut self, old: Position, new: Link) {
        let parent = self.parent(Some(old));
        let side = parent.map_or(0, |parent| self.side(parent, old));
        self.attach(parent, side, new);
    }

    fn rotate(&mut self, pivot: Position, side: usize) {
        let top = self.child(Some(pivot), 1 - side).unwrap();
        let middle = self.child(Some(top), side);
        self.replace(pivot, Some(top));
        self.attach(Some(pivot), 1 - side, middle);
        self.attach(Some(top), side, Some(pivot));
    }

    fn insert_node(&mut self, entry: Entry) -> Link {
        let mut visited = self.root;
        let mut parent = None;
        let mut side = 0;
        while let Some(pos) = visited {
            parent = Some(pos);
            side = usize::from(entry.key() > self.nodes[&pos].entry.key());
            visited = self.child(parent, side);
        }
        self.nodes.insert(
            entry.pos,
            TreeNode {
                entry,
                parent,
                children: [None, None],
                red: true,
            },
        );
        self.attach(parent, side, Some(entry.pos));
        self.balance_insert(entry.pos);
        parent
    }

    fn append_tree_node(&mut self, entry: Entry) {
        self.insert_node(entry);
        self.order.push(entry.pos);
    }

    fn insert(&mut self, entry: Entry) {
        let parent = self.insert_node(entry).unwrap();
        let index = self.order.iter().position(|pos| *pos == parent).unwrap();
        self.order.insert(index + 1, entry.pos);
        self.put_root_first();
    }

    fn put_root_first(&mut self) {
        if let Some(root) = self.root {
            let index = self.order.iter().position(|pos| *pos == root).unwrap();
            self.order[..=index].rotate_right(1);
        }
    }

    fn balance_insert(&mut self, mut pos: Position) {
        while let Some(parent) = self.parent(Some(pos)).filter(|&pos| self.red(Some(pos))) {
            let grand = self.parent(Some(parent)).unwrap();
            let side = self.side(grand, parent);
            let uncle = self.child(Some(grand), 1 - side);
            if self.red(uncle) {
                self.paint(Some(parent), false);
                self.paint(uncle, false);
                self.paint(Some(grand), true);
                pos = grand;
            } else {
                if self.side(parent, pos) != side {
                    pos = parent;
                    self.rotate(parent, side);
                }
                self.paint(self.parent(Some(pos)), false);
                self.paint(Some(grand), true);
                self.rotate(grand, 1 - side);
            }
        }
        self.paint(self.root, false);
    }

    fn small_for_removal(&self) -> bool {
        self.child(self.root, 1).is_none() || self.child(self.child(self.root, 0), 0).is_none()
    }

    fn remove(&mut self, pos: Position) {
        let left = self.child(Some(pos), 0);
        let right = self.child(Some(pos), 1);
        let mut removed_red = self.red(Some(pos));
        let (replacement, parent, side) = if let (Some(left), Some(right)) = (left, right) {
            let mut successor = right;
            while let Some(next) = self.child(Some(successor), 0) {
                successor = next;
            }
            removed_red = self.red(Some(successor));
            let replacement = self.child(Some(successor), 1);
            let (parent, side) = if successor == right {
                (Some(successor), 1)
            } else {
                let parent = self.parent(Some(successor));
                self.replace(successor, replacement);
                self.attach(Some(successor), 1, Some(right));
                (parent, 0)
            };
            self.replace(pos, Some(successor));
            self.attach(Some(successor), 0, Some(left));
            self.paint(Some(successor), self.red(Some(pos)));
            (replacement, parent, side)
        } else {
            let replacement = left.or(right);
            let parent = self.parent(Some(pos));
            let side = parent.map_or(0, |parent| self.side(parent, pos));
            self.replace(pos, replacement);
            (replacement, parent, side)
        };
        self.nodes.remove(&pos);
        self.order.retain(|&entry| entry != pos);
        if !removed_red {
            self.balance_remove(replacement, parent, side);
        }
        self.put_root_first();
    }

    fn balance_remove(&mut self, mut pos: Link, mut parent: Link, mut side: usize) {
        while pos != self.root && !self.red(pos) {
            let above = parent.unwrap();
            let mut sibling = self.child(parent, 1 - side);
            if self.red(sibling) {
                self.paint(sibling, false);
                self.paint(parent, true);
                self.rotate(above, side);
                sibling = self.child(parent, 1 - side);
            }
            if !self.red(self.child(sibling, 0)) && !self.red(self.child(sibling, 1)) {
                self.paint(sibling, true);
                pos = parent;
                parent = self.parent(pos);
                side = parent.map_or(0, |parent| self.side(parent, above));
            } else {
                if !self.red(self.child(sibling, 1 - side)) {
                    self.paint(self.child(sibling, side), false);
                    self.paint(sibling, true);
                    self.rotate(sibling.unwrap(), 1 - side);
                    sibling = self.child(parent, 1 - side);
                }
                self.paint(sibling, self.red(parent));
                self.paint(parent, false);
                self.paint(self.child(sibling, 1 - side), false);
                self.rotate(above, side);
                pos = self.root;
            }
        }
        self.paint(pos, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAPER_WORLD: u128 = 0x752d5d2e_80e8_4d64_bc4d_4c60790f6bd4;

    #[test]
    fn real_paper_overworld_routes_to_the_positive_x_location_first() {
        let a = [9043, 245, 9121];
        let b = [9045, 245, 9121];
        assert_eq!(location_hash(PAPER_WORLD, a), 0x7769_230c);
        assert_eq!(location_hash(PAPER_WORLD, b), 0x7966_1c03);
        for insertion in [[a, b], [b, a]] {
            let mut locations = PaperLocations::default();
            for pos in insertion {
                locations.insert(PAPER_WORLD, pos);
            }
            assert_eq!(locations.iter().collect::<Vec<_>>(), [b, a]);
        }
    }

    #[test]
    fn list_collisions_and_resize_preserve_java_insertion_and_removal_order() {
        let mut locations = PaperLocations::default();
        for x in [17, 1, 33] {
            locations.insert_hashed(x as u32, [x, 0, 0]);
        }
        assert_eq!(
            locations.iter().map(|p| p[0]).collect::<Vec<_>>(),
            [17, 1, 33]
        );
        locations.remove_entry(1, [1, 0, 0]);
        locations.insert_hashed(1, [1, 0, 0]);
        assert_eq!(
            locations.iter().map(|p| p[0]).collect::<Vec<_>>(),
            [17, 33, 1]
        );
        for x in 4..14 {
            locations.insert_hashed(x as u32, [x, 0, 0]);
        }
        assert_eq!(locations.bins.len(), 32);
        for x in 4..14 {
            locations.remove_entry(x as u32, [x, 0, 0]);
        }
        assert_eq!(locations.bins.len(), 32);
        assert_eq!(
            locations.iter().map(|p| p[0]).collect::<Vec<_>>(),
            [33, 1, 17]
        );
    }

    #[test]
    fn java_tree_bin_keeps_linked_order_when_its_root_moves() {
        // OpenJDK 25 HashMap and HashSet, with distinct hashes in one bucket.
        let mut locations = PaperLocations::default();
        let hash = |id: i32| id as u32 * 64 + 41;
        let order = |locations: &PaperLocations| locations.iter().map(|p| p[0]).collect::<Vec<_>>();
        for id in [214, 210, 17, 46, 222, 115, 81, 253, 254, 159, 203, 58] {
            locations.insert_hashed(hash(id), [id, 0, 0]);
        }
        assert_eq!(locations.bins.len(), 64);
        assert!(matches!(locations.bins[41], Bin::Tree(_)));
        assert_eq!(
            order(&locations),
            [210, 214, 17, 46, 222, 115, 81, 58, 253, 254, 159, 203]
        );
        locations.remove_entry(hash(210), [210, 0, 0]);
        locations.insert_hashed(hash(210), [210, 0, 0]);
        locations.remove_entry(hash(254), [254, 0, 0]);
        assert_eq!(
            order(&locations),
            [115, 214, 17, 46, 222, 81, 58, 253, 159, 203, 210]
        );
    }

    #[test]
    fn equal_full_hashes_keep_every_distinct_block() {
        let mut locations = PaperLocations::default();
        for x in (0..18).rev() {
            locations.insert_hashed(123, [x, 0, 0]);
        }
        locations.remove_entry(123, [5, 0, 0]);
        for x in 0..18 {
            locations.insert_hashed(123, [x, 0, 0]);
        }
        let mut positions = locations.iter().map(|p| p[0]).collect::<Vec<_>>();
        positions.sort_unstable();
        assert_eq!(positions, (0..18).collect::<Vec<_>>());
    }
}
