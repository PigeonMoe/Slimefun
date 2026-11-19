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
}

impl Discovery {
    pub(super) fn discover(
        &mut self,
        active: &BTreeMap<Position, (&Node, u32)>,
        budget: u32,
    ) -> Vec<Group> {
        let topology = active
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
        self.update(topology);

        let mut claimed = BTreeSet::new();
        let mut groups = Vec::new();
        for (&pos, &(family, role)) in &self.topology {
            if role != Role::Regulator || !claimed.insert(pos) {
                continue;
            }
            let graph = self.graphs.entry(pos).or_insert_with(|| Graph::new(pos));
            graph.advance(&self.topology, family, budget);
            let group = graph
                .members
                .iter()
                .copied()
                .filter(|&member| member == pos || claimed.insert(member))
                .collect();
            groups.push((pos, family, group));
        }
        groups
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
        self.topology = topology;
    }
}

struct Graph {
    seen: BTreeSet<Position>,
    queued: BTreeSet<Position>,
    pending: VecDeque<Position>,
    members: BTreeSet<Position>,
}

impl Graph {
    fn new(start: Position) -> Self {
        Self {
            seen: BTreeSet::from([start]),
            queued: BTreeSet::from([start]),
            pending: VecDeque::from([start]),
            members: BTreeSet::from([start]),
        }
    }

    fn queue(&mut self, pos: Position) {
        if self.queued.insert(pos) {
            self.pending.push_back(pos);
        }
    }

    fn advance(&mut self, topology: &Topology, family: Network, budget: u32) {
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
            if kind != family {
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
