// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeSet,
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use mod_api::{
    BlockState, BreakBlockEvent, ComponentId, Compound, Flow, Hand, Host, HostTransaction,
    Identifier, ItemPlace, ItemResource, ItemStack, LevelRef, PlayerRef, ServerHost, SoundSource,
    Tag, Text,
};

use crate::{
    android_machine::Position,
    items::{FERROSILICON, GILDED_IRON, Kind, SYNTHETIC_DIAMOND, item, text},
    network_runtime,
    random::{Rolls, natural_drop_position},
    recipes::{CraftingRecipe, Research, Stack},
    research,
    smelters_pickaxe::{enchantment, fortune_amount, ore},
};

item!(
    PICKAXE_OF_VEIN_MINING,
    "diamond_pickaxe",
    "Pickaxe of Vein Mining",
    "yellow",
    None,
    None
);
item!(
    PICKAXE_OF_THE_SEEKER,
    "diamond_pickaxe",
    "Pickaxe of the Seeker",
    "green",
    None,
    None
);

pub(crate) const VEIN_RESEARCH: Research = Research {
    id: "pickaxe_of_vein_mining",
    name: "Pickaxe of Vein Mining",
    cost: 29,
};
pub(crate) const SEEKER_RESEARCH: Research = Research {
    id: "pickaxe_of_the_seeker",
    name: "Pickaxe of the Seeker",
    cost: 19,
};

const EMERALD_ORE: Option<Kind> = Some(Kind::Vanilla("minecraft:emerald_ore"));
const COMPASS: Option<Kind> = Some(Kind::Vanilla("minecraft:compass"));

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            EMERALD_ORE,
            Some(SYNTHETIC_DIAMOND),
            EMERALD_ORE,
            None,
            Some(GILDED_IRON),
            None,
            None,
            Some(GILDED_IRON),
            None,
        ],
        output: Stack {
            kind: PICKAXE_OF_VEIN_MINING,
            count: 1,
        },
        research: &VEIN_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            COMPASS,
            Some(SYNTHETIC_DIAMOND),
            COMPASS,
            None,
            Some(FERROSILICON),
            None,
            None,
            Some(FERROSILICON),
            None,
        ],
        output: Stack {
            kind: PICKAXE_OF_THE_SEEKER,
            count: 1,
        },
        research: &SEEKER_RESEARCH,
    },
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let lore = match id {
        "PICKAXE_OF_VEIN_MINING" => vec![
            Tag::String("".into()),
            text("This Pickaxe will dig out", "white", false),
            text("whole Veins of Ores...", "white", false),
        ],
        "PICKAXE_OF_THE_SEEKER" => vec![
            text("Will always point you to the nearest Ore", "white", false),
            text("but might get damaged when doing it", "white", false),
            Tag::String("".into()),
            Tag::compound([
                ("text", Tag::String("".into())),
                ("italic", Tag::Byte(0)),
                (
                    "extra",
                    Tag::List(vec![
                        text("Right Click", "yellow", false),
                        text(" to be pointed to the nearest Ore", "gray", false),
                    ]),
                ),
            ]),
        ],
        _ => return,
    };
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

// Dough Vein.find traverses recursively in this order, including four horizontal diagonals.
const NEIGHBORS: [Position; 10] = [
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, -1],
    [1, 0, 0],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, -1],
    [-1, 0, -1],
    [1, 0, 1],
    [-1, 0, 1],
];
const DEFAULT_MAX_BLOCKS: u32 = 16;
const DEFAULT_RANGE: u32 = 5;

fn vein<T>(
    first: Position,
    maximum: usize,
    mut state_at: impl FnMut(Position) -> Option<T>,
) -> Vec<(Position, T)> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![first];
    let mut matches = Vec::new();
    while matches.len() < maximum {
        let Some(pos) = stack.pop() else {
            break;
        };
        if !seen.insert(pos) {
            continue;
        }
        let Some(state) = state_at(pos) else {
            continue;
        };
        matches.push((pos, state));
        stack.extend(NEIGHBORS.iter().rev().map(|&[dx, dy, dz]| {
            [
                pos[0].saturating_add(dx),
                pos[1].saturating_add(dy),
                pos[2].saturating_add(dz),
            ]
        }));
    }
    matches
}

fn ore_at(host: &dyn Host, level: LevelRef, pos: Position) -> Option<BlockState> {
    let state = level.block_state(host, pos)?;
    ore(&state.block().id(host)).then_some(state)
}

struct Mining<'a> {
    active: &'a Mutex<BTreeSet<u128>>,
    player: u128,
}

impl Drop for Mining<'_> {
    fn drop(&mut self) {
        self.active.lock().unwrap().remove(&self.player);
    }
}

pub(crate) struct OreTools {
    max_blocks: AtomicU32,
    max_range: AtomicU32,
    mining: Mutex<BTreeSet<u128>>,
}

impl Default for OreTools {
    fn default() -> Self {
        Self {
            max_blocks: AtomicU32::new(DEFAULT_MAX_BLOCKS),
            max_range: AtomicU32::new(DEFAULT_RANGE),
            mining: Mutex::new(BTreeSet::new()),
        }
    }
}

fn unlocked(host: &mut dyn ServerHost, player: PlayerRef, required: &Research) -> bool {
    if research::can_use(host, player, required) {
        return true;
    }
    let _ = player.tell(
        host,
        Text::literal(format!(
            "Unlock {} with /sf research {}.",
            required.name, required.id
        )),
    );
    false
}

fn hand(player: PlayerRef, hand: Hand) -> ItemPlace {
    ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    }
}

impl OreTools {
    pub(crate) fn configure(&self, table: &toml::Table) {
        let value = |item: &str, setting: &str, default: u32| {
            table
                .get(item)
                .and_then(|section| section.get(setting))
                .and_then(toml::Value::as_integer)
                .filter(|value| (1..=i64::from(i32::MAX)).contains(value))
                .map_or(default, |value| value as u32)
        };
        self.max_blocks.store(
            value("pickaxe-of-vein-mining", "max-blocks", DEFAULT_MAX_BLOCKS),
            Ordering::Relaxed,
        );
        self.max_range.store(
            value("pickaxe-of-the-seeker", "max-range", DEFAULT_RANGE),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn on_break(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        event: &BreakBlockEvent<'_>,
        rolls: &Rolls,
    ) -> Flow {
        if player.is_fake(host) == Some(true)
            || self.mining.lock().unwrap().contains(&player.uuid())
        {
            return Flow::Continue;
        }
        let Some(tool) = hand(player, Hand::Main).stack(host) else {
            return Flow::Continue;
        };
        if PICKAXE_OF_THE_SEEKER.matches(host, &tool) {
            return if unlocked(host, player, &SEEKER_RESEARCH) {
                Flow::Continue
            } else {
                Flow::Cancel
            };
        }
        if !PICKAXE_OF_VEIN_MINING.matches(host, &tool) {
            return Flow::Continue;
        }
        if !unlocked(host, player, &VEIN_RESEARCH) {
            return Flow::Cancel;
        }
        let material = event.state.block().id(host);
        if !ore(&material) || network_runtime::has_block_marker(host, level, event.pos) {
            return Flow::Continue;
        }

        let silk = enchantment(host, &tool, "minecraft:silk_touch") > 0;
        let amount = fortune_amount(
            enchantment(host, &tool, "minecraft:fortune"),
            silk,
            &material,
            |bound| rolls.next(host, level, bound).unwrap_or(0),
        );
        let points = vein(
            event.pos,
            self.max_blocks.load(Ordering::Relaxed) as usize,
            |pos| ore_at(host, level, pos),
        );

        let _ = self.mining.lock().unwrap().insert(player.uuid());
        let _guard = Mining {
            active: &self.mining,
            player: player.uuid(),
        };
        for (pos, expected) in points {
            let completed = (pos != event.pos || expected == event.state)
                && break_member(host, player, level, &tool, pos, expected, silk, amount);
            if pos == event.pos && !completed {
                break;
            }
        }
        // Paper rereads the root as air after its handler. This is a scripted
        // vein break, with no second drop, experience or vanilla tool wear.
        Flow::Cancel
    }

    pub(crate) fn seek(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        used_hand: Hand,
        tool: &ItemStack,
        rolls: &Rolls,
    ) -> bool {
        if !PICKAXE_OF_THE_SEEKER.matches(host, tool) {
            return false;
        }
        if !unlocked(host, player, &SEEKER_RESEARCH) {
            return true;
        }
        let entity = player.entity();
        let (Some(level), Some(position)) = (player.level(host), entity.position(host)) else {
            return true;
        };
        let origin = position.map(|coordinate| coordinate.floor() as i32);
        let range = self.max_range.load(Ordering::Relaxed) as i32;
        if let Some(target) = nearest_ore(host, level, origin, range) {
            if let (Some(dimension), Some([yaw, _])) =
                (level.dimension().id(host), entity.rotation(host))
            {
                let rotation = direction(position, target, yaw);
                let _ = player.teleport_checked(host, &dimension, position, Some(rotation));
            }
        } else {
            let mut message = Text::literal("Cannot find any nearby ores!");
            message.style.color = Some("red".into());
            let _ = player.tell(host, message);
        }
        seeker_wear(host, player, level, used_hand, tool, rolls);
        true
    }
}

fn break_member(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    tool: &ItemStack,
    pos: Position,
    expected: BlockState,
    silk: bool,
    amount: u32,
) -> bool {
    if level.block_state(host, pos) != Some(expected)
        || network_runtime::has_block_marker(host, level, pos)
        || !level.allow_device_breaking(host, player.uuid(), pos)
    {
        return false;
    }
    let Some(drops) = member_drops(host, level, tool, pos, expected, silk, amount) else {
        return false;
    };
    host.transaction(|host, _| {
        for drop in drops {
            let [x, y, z] = natural_drop_position(host, level, pos);
            level
                .spawn_item(host, x, y, z, drop, 10)
                .map_err(|_| ())?
                .ok_or(())?;
        }
        // The engine performs the PurtmarsArea/vanilla veto with the acting
        // player and compares the old state again before committing removal.
        player
            .remove_block_without_drops(host, pos, expected)
            .then_some(())
            .ok_or(())
    })
    .is_ok()
}

fn member_drops(
    host: &mut dyn ServerHost,
    level: LevelRef,
    tool: &ItemStack,
    pos: Position,
    expected: BlockState,
    silk: bool,
    amount: u32,
) -> Option<Vec<ItemStack>> {
    if silk {
        let block = Identifier::parse(&expected.block().id(host)).ok()?;
        return Some(vec![ItemStack::new(host.item_key(&block)?, 1)?]);
    }
    let drops = level.block_drops(host, pos, Some(tool), None).ok()??;
    let mut actual = Vec::new();
    for drop in drops {
        let item = drop.item();
        if host
            .item_block(item)
            .is_some_and(|block| block.id(host) == item.id(host))
        {
            actual.push(drop);
            continue;
        }
        let copied = ItemResource::of(&drop);
        let limit = host.max_stack_size(item)?.max(1);
        let mut count = amount;
        while count != 0 {
            let stack = count.min(limit);
            actual.push(copied.to_stack(stack)?);
            count -= stack;
        }
    }
    Some(actual)
}

fn distance_squared(left: Position, right: Position) -> i128 {
    left.into_iter()
        .zip(right)
        .map(|(a, b)| (i128::from(a) - i128::from(b)).pow(2))
        .sum()
}

fn nearest(
    origin: Position,
    mut boxes: Vec<(Position, Position)>,
    mut is_ore: impl FnMut(Position) -> bool,
) -> Option<Position> {
    let lower_bound = |&(low, high): &(Position, Position)| -> i128 {
        distance_squared(
            origin,
            std::array::from_fn(|i| origin[i].clamp(low[i], high[i])),
        )
    };
    boxes.sort_by_key(lower_bound);
    let mut best = None;
    for (lo, hi) in boxes {
        if best.is_some_and(|(distance, _)| lower_bound(&(lo, hi)) > distance) {
            break;
        }
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let pos = [x, y, z];
                    let rank = (distance_squared(origin, pos), pos);
                    // Position breaks distance ties like Paper's x, y, z loops.
                    if best.is_none_or(|prev| rank < prev) && is_ore(pos) {
                        best = Some(rank);
                    }
                }
            }
        }
    }
    best.map(|(_, pos)| pos)
}

fn nearest_ore(host: &dyn Host, level: LevelRef, origin: Position, range: i32) -> Option<Position> {
    let lo = origin.map(|v| v.saturating_sub(range));
    let hi = origin.map(|v| v.saturating_add(range));
    let (base_y, height) = level.height(host)?;
    let low_y = lo[1].max(base_y);
    let high_y = hi[1].min(base_y.checked_add(height)?.checked_sub(1)?);
    if low_y > high_y {
        return None;
    }
    let columns = level.loaded_chunks(host)?;
    let boxes = columns
        .into_iter()
        .filter_map(|chunk| {
            let [x, z] = chunk.pos().map(|axis| axis.saturating_mul(16));
            let low = [x.max(lo[0]), low_y, z.max(lo[2])];
            let high = [
                x.saturating_add(15).min(hi[0]),
                high_y,
                z.saturating_add(15).min(hi[2]),
            ];
            (low[0] <= high[0] && low[2] <= high[2]).then_some((low, high))
        })
        .collect();
    nearest(origin, boxes, |pos| ore_at(host, level, pos).is_some())
}

fn direction(at: [f64; 3], destination: Position, existing_yaw: f32) -> [f32; 2] {
    let dx = f64::from(destination[0]) + 0.5 - at[0];
    let dz = f64::from(destination[2]) + 0.5 - at[2];
    let horizontal = dx.hypot(dz);
    // Paper uses a point half a block below the ore's y coordinate.
    let vertical = f64::from(destination[1]) - 0.5 - at[1];
    let yaw = if horizontal == 0.0 {
        existing_yaw // A vertically aligned ore does not supply a heading.
    } else {
        let angle = -(dx / horizontal).asin().to_degrees() as f32;
        if dz < 0.0 { 180.0 - angle } else { angle }
    };
    [yaw, -vertical.atan2(horizontal).to_degrees() as f32]
}

fn seeker_wear(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    used_hand: Hand,
    tool: &ItemStack,
    rolls: &Rolls,
) {
    let unbreaking = enchantment(host, tool, "minecraft:unbreaking");
    if unbreaking != 0 && rolls.next(host, level, unbreaking + 1) != Some(0) {
        return;
    }
    if ComponentId::new("minecraft:unbreakable")
        .is_some_and(|id| ItemResource::of(tool).value(host, &id).is_some())
    {
        return;
    }
    let Some(maximum) = host.max_damage(tool.item()).filter(|&max| max != 0) else {
        return;
    };
    let old = ItemResource::of(tool);
    let held = hand(player, used_hand);
    if tool.damage() >= maximum {
        if crate::item_clicks::consume_one(host, player, used_hand, tool) {
            let _ = player.play_sound(
                host,
                "minecraft:entity.item.break",
                SoundSource::Master,
                1.0,
                1.0,
            );
        }
    } else {
        let damage = tool.damage() + 1;
        let _: Result<(), ()> = host.transaction(|host, tx| {
            if held.resource(host).as_ref() != Some(&old) {
                return Err(());
            }
            held.edit(host, tx, |stack| stack.set_damage(damage))
                .then_some(())
                .ok_or(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vein_uses_paper_depth_first_neighbors_and_counts_start_toward_limit() {
        let inside = BTreeSet::from([
            [0, 0, 0],
            [0, 1, 0],
            [1, 1, -1],
            [-1, 0, 0],
            [1, 0, 1],
            [2, 2, 1],
        ]);
        let ran = |limit| {
            vein([0, 0, 0], limit, |pos| inside.contains(&pos).then_some(()))
                .into_iter()
                .map(|(pos, _)| pos)
                .collect::<Vec<_>>()
        };
        assert_eq!(ran(1), [[0, 0, 0]]);
        assert_eq!(ran(3), [[0, 0, 0], [0, 1, 0], [1, 1, -1]]);
        assert_eq!(
            ran(16),
            [[0, 0, 0], [0, 1, 0], [1, 1, -1], [-1, 0, 0], [1, 0, 1]]
        );
    }

    #[test]
    fn seeker_keeps_paper_distance_tie_order_across_chunks_and_handles_vertical_ore() {
        let ore = [[16, 64, 0], [14, 64, 0]];
        let boxes = vec![([16, 63, 0], [17, 65, 0]), ([14, 63, 0], [15, 65, 0])];
        assert_eq!(
            nearest([15, 64, 0], boxes, |pos| ore.contains(&pos)),
            Some([14, 64, 0])
        );
        let east = direction([10.5, 64.0, -5.5], [13, 64, -6], 130.0);
        assert_eq!(east[0], -90.0);
        assert!((east[1] - 9.462322).abs() < 0.0001);
        assert_eq!(
            direction([10.5, 64., -5.5], [10, 67, -6], 130.0),
            [130.0, -90.0]
        );
        assert_eq!(
            direction([10.5, 64., -5.5], [10, 62, -6], 130.0),
            [130.0, 90.0]
        );
    }

    #[test]
    fn recipes_and_ore_groups_copy_paper_and_settings_reject_bad_integers() {
        assert_eq!(
            CRAFTING[0].inputs.map(|item| item.map_or("", Kind::id)),
            [
                "minecraft:emerald_ore",
                "SYNTHETIC_DIAMOND",
                "minecraft:emerald_ore",
                "",
                "GILDED_IRON",
                "",
                "",
                "GILDED_IRON",
                ""
            ],
        );
        assert_eq!(
            CRAFTING[1].inputs.map(|item| item.map_or("", Kind::id)),
            [
                "minecraft:compass",
                "SYNTHETIC_DIAMOND",
                "minecraft:compass",
                "",
                "FERROSILICON",
                "",
                "",
                "FERROSILICON",
                ""
            ],
        );
        assert_eq!((VEIN_RESEARCH.cost, SEEKER_RESEARCH.cost), (29, 19));
        for id in [
            "copper_ore",
            "deepslate_redstone_ore",
            "nether_quartz_ore",
            "ancient_debris",
        ] {
            assert!(ore(&format!("minecraft:{id}")));
        }
        assert!(!ore("minecraft:gilded_blackstone"));
        let tools = OreTools::default();
        tools.configure(
            &"[pickaxe-of-the-seeker]\nmax-range=12\n[pickaxe-of-vein-mining]\nmax-blocks=0"
                .parse()
                .unwrap(),
        );
        assert_eq!(tools.max_range.load(Ordering::Relaxed), 12);
        assert_eq!(tools.max_blocks.load(Ordering::Relaxed), 16);
    }
}
