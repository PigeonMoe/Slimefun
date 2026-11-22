// SPDX-License-Identifier: GPL-3.0-only

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{Network, Node, Position};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Role {
    Regulator,
    Relay,
    Terminal,
}

type Topology = BTreeMap<Position, (Network, Role)>;
type Group = (Position, Network, Vec<Position>);

#[derive(Default)]
pub(super) struct Discovery {
    topology: Topology,
    graphs: BTreeMap<Position, Graph>,
    energy_order: Vec<Position>,
    cargo_order: Vec<Position>,
}

impl Discovery {
    pub(super) fn discover(
        &mut self,
        active: &BTreeMap<Position, (&Node, u32)>,
        budget: u32,
        mut is_loaded: impl FnMut(Position) -> bool,
    ) -> Vec<Group> {
        let mut topology: Topology = active
            .iter()
            .filter_map(|(&pos, &(node, _))| {
                let family = node.family();
                if family == Network::Independent {
                    return None;
                }
                let role = if node.regulator() {
                    Role::Regulator
                } else if node.relays() {
                    Role::Relay
                } else {
                    Role::Terminal
                };
                Some((pos, (family, role)))
            })
            .collect();
        // Unloading a chunk does not unregister Paper's networks. Keep
        // positions already observed there until a loaded block disproves them.
        for (&pos, &role) in &self.topology {
            if !topology.contains_key(&pos) && !is_loaded(pos) {
                topology.insert(pos, role);
            }
        }
        self.update(topology);

        // An explicitly disabled root has no registered Paper BlockTicker.
        // The existing positions still classify normally for passive routing.
        let mut groups = Vec::new();
        for (family, root_id) in [
            (Network::Energy, "ENERGY_REGULATOR"),
            (Network::Cargo, "CARGO_MANAGER"),
        ] {
            if crate::availability::globally_enabled(root_id) {
                groups.extend(self.discover_family(active, family, budget));
            }
        }
        groups.sort_unstable_by_key(|(root, _, _)| *root);
        groups
    }

    fn discover_family(
        &mut self,
        active: &BTreeMap<Position, (&Node, u32)>,
        family: Network,
        budget: u32,
    ) -> Vec<Group> {
        let order = match family {
            Network::Energy => &mut self.energy_order,
            Network::Cargo => &mut self.cargo_order,
            Network::Independent => return Vec::new(),
        };
        let mut unclaimed: BTreeSet<_> = active
            .keys()
            .filter_map(|&pos| {
                (self.topology.get(&pos) == Some(&(family, Role::Regulator))).then_some(pos)
            })
            .collect();
        if unclaimed.is_empty() {
            return Vec::new();
        }
        let mut conflicts = BTreeMap::<Position, usize>::new();
        let mut claimed_members = BTreeSet::new();
        let mut groups: Vec<Group> = Vec::new();
        let mut added = Vec::new();
        let mut registered = order.iter().copied();
        while let Some(root) = registered.next().or_else(|| unclaimed.first().copied()) {
            let graph = self.graphs.entry(root).or_insert_with(|| {
                added.push(root);
                Graph::new(root)
            });
            let group = if let Some(&group) = conflicts.get(&root) {
                // A registered network whose root now reports a conflict still
                // holds its previously discovered positions, but does not tick.
                group
            } else {
                let mut members = Vec::new();
                if unclaimed.remove(&root) {
                    graph.advance(&self.topology, root, family, budget);
                    members.extend(graph.members.iter().copied().filter(|pos| {
                        active.get(pos).is_some_and(|(node, _)| {
                            *pos == root || (!node.regulator() && claimed_members.insert(*pos))
                        })
                    }));
                }
                let group = groups.len();
                groups.push((root, family, members));
                group
            };
            // Paper registers positions on queueing, before classification.
            let reached: Vec<_> = unclaimed.intersection(&graph.seen).copied().collect();
            for pos in reached {
                unclaimed.remove(&pos);
                conflicts.insert(pos, group);
                groups[group].2.push(pos);
            }
        }
        order.extend(added);
        groups.retain(|(_, _, members)| !members.is_empty());
        for (_, _, members) in &mut groups {
            members.sort_unstable();
        }
        groups
    }

    pub(super) fn retire_regulator(&mut self, pos: Position) {
        self.graphs.remove(&pos);
        self.energy_order.retain(|&root| root != pos);
        self.cargo_order.retain(|&root| root != pos);
    }

    pub(super) fn cargo_due(&mut self, root: Position, delay: u32) -> bool {
        let Some(graph) = self.graphs.get_mut(&root) else {
            return false;
        };
        // Paper counts only connected ticker calls on this CargoNet, and
        // recreating that net discards the previous counter.
        if graph.cargo_elapsed < delay {
            graph.cargo_elapsed += 1;
            return false;
        }
        graph.cargo_elapsed = 0;
        true
    }

    fn update(&mut self, topology: Topology) {
        if self.topology == topology {
            return;
        }
        let changes: BTreeSet<_> = self
            .topology
            .keys()
            .chain(topology.keys())
            .copied()
            .filter(|pos| self.topology.get(pos) != topology.get(pos))
            .collect();
        self.graphs.retain(|start, graph| {
            let Some(&(family, Role::Regulator)) = topology.get(start) else {
                return false;
            };
            if self.topology.get(start) != topology.get(start)
                || changes.iter().any(|pos| {
                    graph.members.contains(pos)
                        && matches!(
                            self.topology.get(pos),
                            Some((old, Role::Regulator | Role::Relay)) if *old == family
                        )
                })
            {
                return false;
            }
            for &pos in &changes {
                if graph.seen.contains(&pos)
                    && [self.topology.get(&pos), topology.get(&pos)]
                        .into_iter()
                        .flatten()
                        .any(|&(changed, _)| changed == family)
                {
                    graph.members.remove(&pos);
                    graph.queue(pos);
                }
            }
            true
        });
        for order in [&mut self.energy_order, &mut self.cargo_order] {
            order.retain(|root| self.graphs.contains_key(root));
        }
        self.topology = topology;
    }
}

struct Graph {
    seen: BTreeSet<Position>,
    queued: BTreeSet<Position>,
    pending: VecDeque<Position>,
    members: BTreeSet<Position>,
    cargo_elapsed: u32,
}

impl Graph {
    fn new(start: Position) -> Self {
        Self {
            seen: BTreeSet::from([start]),
            queued: BTreeSet::from([start]),
            pending: VecDeque::from([start]),
            members: BTreeSet::from([start]),
            cargo_elapsed: 0,
        }
    }

    fn queue(&mut self, pos: Position) {
        if self.queued.insert(pos) {
            self.pending.push_back(pos);
        }
    }

    fn advance(&mut self, topology: &Topology, root: Position, family: Network, budget: u32) {
        let range = match family {
            Network::Energy => 7,
            Network::Cargo => 6,
            Network::Independent => return,
        };
        for _ in 0..budget {
            let Some(pos) = self.pending.pop_front() else {
                break;
            };
            self.queued.remove(&pos);
            let Some(&(kind, role)) = topology.get(&pos) else {
                continue;
            };
            // Only CargoNet classifies another regulator as a relay.
            if kind != family
                || (family == Network::Energy && role == Role::Regulator && pos != root)
            {
                continue;
            }
            self.members.insert(pos);
            if role == Role::Terminal {
                continue;
            }
            // Paper visits both directions on each axis, farthest first, counting air work too.
            for axis in 0..3 {
                for sign in [1, -1] {
                    for distance in (1..=range).rev() {
                        let Some(value) = pos[axis].checked_add(sign * distance) else {
                            continue;
                        };
                        let mut neighbor = pos;
                        neighbor[axis] = value;
                        if self.seen.insert(neighbor) {
                            self.queue(neighbor);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networks::Component;

    fn node(id: &str) -> Node {
        Node {
            id: id.to_string(),
            component: Component::for_item(id).unwrap(),
            channel: 0,
            passes_unfiltered: true,
        }
    }

    #[test]
    fn a_later_lower_regulator_shows_conflict_and_cannot_extend_the_existing_energy_mesh() {
        let regulator = node("ENERGY_REGULATOR");
        let connector = node("ENERGY_CONNECTOR");
        let capacitor = node("SMALL_CAPACITOR");
        let root = [12, 70, 0];
        let relay = [6, 70, 0];
        let original_cell = [6, 70, 7];
        let second = [0, 70, 0];
        let distant_cell = [-6, 70, 0];
        let mut active = BTreeMap::from([
            (root, (&regulator, 0)),
            (relay, (&connector, 0)),
            (original_cell, (&capacitor, 38)),
        ]);
        let mut discovery = Discovery::default();
        assert_eq!(
            discovery.discover(&active, 200, |_| true)[0].2,
            [relay, original_cell, root]
        );

        active.insert(second, (&regulator, 0));
        active.insert(distant_cell, (&capacitor, 7));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, root);
        assert_eq!(groups[0].2, [second, relay, original_cell, root]);

        active.remove(&root);
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, second);
        assert_eq!(groups[0].2, [distant_cell, second, relay, original_cell]);
    }

    #[test]
    fn an_older_cargo_manager_keeps_control_and_the_conflicting_manager_extends_the_mesh() {
        let manager = node("CARGO_MANAGER");
        let terminal = node("CARGO_NODE_OUTPUT");
        let older = [29, 70, 0];
        let later = [23, 70, 0];
        let near = [29, 70, 6];
        let far = [17, 70, 0];
        let mut active = BTreeMap::from([(older, (&manager, 0)), (near, (&terminal, 0))]);
        let mut discovery = Discovery::default();
        assert_eq!(
            discovery.discover(&active, 200, |_| true)[0].2,
            [older, near]
        );

        active.insert(later, (&manager, 0));
        active.insert(far, (&terminal, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, older);
        assert_eq!(groups[0].2, [far, later, older, near]);

        active.remove(&older);
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, later);
        assert_eq!(groups[0].2, [far, later]);
    }

    #[test]
    fn a_parked_cargo_manager_keeps_conflicts_and_resumes_its_existing_tick_counter() {
        let manager = node("CARGO_MANAGER");
        let terminal = node("CARGO_NODE_INPUT");
        let older = [33, 70, 0];
        let later = [31, 70, 0];
        let input = [30, 70, 0];
        let mut active = BTreeMap::from([(older, (&manager, 0)), (input, (&terminal, 0))]);
        let mut discovery = Discovery::default();
        assert_eq!(
            discovery.discover(&active, 200, |_| true)[0].2,
            [input, older]
        );
        assert!(!discovery.cargo_due(older, 1));

        active.remove(&older);
        active.insert(later, (&manager, 0));
        let groups = discovery.discover(&active, 200, |pos| pos[0].div_euclid(16) != 2);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, older);
        assert_eq!(groups[0].2, [later]);
        assert!(!discovery.cargo_due(later, 0));

        active.insert(older, (&manager, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups[0].0, older);
        assert_eq!(groups[0].2, [input, later, older]);
        assert!(discovery.cargo_due(older, 1));

        assert!(!discovery.cargo_due(older, 1));
        discovery.retire_regulator(older);
        active.remove(&later);
        discovery.discover(&active, 200, |_| true);
        assert!(!discovery.cargo_due(older, 1));
    }

    #[test]
    fn queued_but_unclassified_regulator_positions_already_have_an_owner() {
        let regulator = node("ENERGY_REGULATOR");
        let root = [5, 70, 0];
        let new = [0, 70, 0];
        let mut active = BTreeMap::from([(root, (&regulator, 0))]);
        let mut discovery = Discovery::default();
        assert_eq!(discovery.discover(&active, 1, |_| true)[0].2, [root]);

        active.insert(new, (&regulator, 0));
        let groups = discovery.discover(&active, 1, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, root);
        assert_eq!(groups[0].2, [new, root]);
    }

    #[test]
    fn an_unloaded_regulator_keeps_ownership_without_powering_or_reserving_loaded_members() {
        let regulator = node("ENERGY_REGULATOR");
        let capacitor = node("SMALL_CAPACITOR");
        let older = [33, 70, 0];
        let later = [31, 70, 0];
        let cell = [30, 70, 0];
        let mut active = BTreeMap::from([(older, (&regulator, 0)), (cell, (&capacitor, 42))]);
        let mut discovery = Discovery::default();
        assert_eq!(
            discovery.discover(&active, 200, |_| true)[0].2,
            [cell, older]
        );

        active.remove(&older);
        active.insert(later, (&regulator, 0));
        let groups = discovery.discover(&active, 200, |pos| pos[0].div_euclid(16) != 2);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, older);
        // The scheduler only emits conflicts for this parked owner's loaded roots.
        assert_eq!(groups[0].2, [later]);

        active.insert(older, (&regulator, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, older);
        assert_eq!(groups[0].2, [cell, later, older]);

        active.remove(&older);
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, later);
        assert_eq!(groups[0].2, [cell, later]);
    }

    #[test]
    fn a_split_releases_rebuilt_roots_but_preserves_other_registered_energy_nets() {
        let regulator = node("ENERGY_REGULATOR");
        let connector = node("ENERGY_CONNECTOR");
        let cell = node("SMALL_CAPACITOR");
        let first = [14, 70, 0];
        let second = [0, 70, 0];
        let near_second = [0, 70, 7];
        let bridge = [7, 70, 0];
        let mut active = BTreeMap::from([(first, (&regulator, 0))]);
        let mut discovery = Discovery::default();
        discovery.discover(&active, 200, |_| true);
        active.insert(second, (&regulator, 0));
        assert_eq!(discovery.discover(&active, 200, |_| true).len(), 2);

        active.insert(bridge, (&connector, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, first);

        // The dormant second network keeps discovered positions until it ticks.
        active.insert(near_second, (&regulator, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].2, [second, near_second, bridge, first]);

        active.remove(&bridge);
        active.insert(near_second, (&cell, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].2, [second, near_second]);
        assert_eq!(groups[1].2, [first]);

        active.insert(bridge, (&connector, 0));
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, second);

        // A registration at the same position can replace a regulator between
        // discovery rounds. Its fresh marker must not inherit the old age.
        discovery.retire_regulator(second);
        let groups = discovery.discover(&active, 200, |_| true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, first);
    }
}
