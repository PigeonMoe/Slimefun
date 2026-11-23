// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeSet, sync::Mutex};

use mod_api::{
    BreakBlockEvent, Compound, ExplosionCalculator, ExplosionInteraction, ExplosionSettings, Flow,
    Hand, LevelRef, PlayerRef, ServerHost, SoundSource, Tag,
};

use crate::{
    android_machine::{Position, inside_border},
    items::{FERROSILICON, Kind, SYNTHETIC_DIAMOND, item, text},
    ore_tools::{Mining, hand, paper_tool_wear, unlocked},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    EXPLOSIVE_PICKAXE,
    "diamond_pickaxe",
    "Explosive Pickaxe",
    "yellow",
    None,
    None
);
item!(
    EXPLOSIVE_SHOVEL,
    "diamond_shovel",
    "Explosive Shovel",
    "yellow",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "explosive_tools",
    name: "Explosive Tools",
    cost: 30,
};
const TNT: Option<Kind> = Some(Kind::Vanilla("minecraft:tnt"));
const DIAMOND: Option<Kind> = Some(SYNTHETIC_DIAMOND);
const HANDLE: Option<Kind> = Some(FERROSILICON);

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [TNT, DIAMOND, TNT, None, HANDLE, None, None, HANDLE, None],
        output: Stack {
            kind: EXPLOSIVE_PICKAXE,
            count: 1,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [None, DIAMOND, None, None, TNT, None, None, HANDLE, None],
        output: Stack {
            kind: EXPLOSIVE_SHOVEL,
            count: 1,
        },
        research: &RESEARCH,
    },
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let label = match id {
        "EXPLOSIVE_PICKAXE" => "of Blocks at once...",
        "EXPLOSIVE_SHOVEL" => "of diggable Blocks at once...",
        _ => return,
    };
    let blank = || Tag::String("".into());
    let mut lore = vec![
        blank(),
        text("Allows you to mine a good bit", "white", false),
        text(label, "white", false),
    ];
    if id == "EXPLOSIVE_PICKAXE" {
        lore.push(blank());
        lore.push(text("Works with Fortune", "dark_blue", false));
    }
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

#[derive(Clone, Copy)]
enum Tool {
    Pickaxe,
    Shovel,
}

impl Tool {
    const fn index(self) -> usize {
        self as usize
    }

    const fn section(self) -> &'static str {
        match self {
            Self::Pickaxe => "explosive-pickaxe",
            Self::Shovel => "explosive-shovel",
        }
    }

    fn includes(
        self,
        host: &dyn ServerHost,
        level: LevelRef,
        pos: Position,
        state: mod_api::BlockState,
    ) -> bool {
        let block = state.block();
        let name = block.id(host);
        match self {
            Tool::Pickaxe => pickaxe_material(&name) && inside_border(host, level, pos),
            Tool::Shovel => {
                shovel_material(&name) || host.block_in_tag(block, "minecraft:sand") == Some(true)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Rules {
    damage_on_use: bool,
    call_explosion_event: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            damage_on_use: true,
            call_explosion_event: false,
        }
    }
}

pub(crate) const DEFAULT_CONFIG: &str = "\n[explosive-pickaxe]\ndamage-on-use = true\ncall-explosion-event = false\n\n[explosive-shovel]\ndamage-on-use = true\ncall-explosion-event = false\n";

#[derive(Default)]
pub(crate) struct ExplosiveTools {
    rules: Mutex<[Rules; 2]>,
    active: Mutex<BTreeSet<u128>>,
}

impl ExplosiveTools {
    pub(crate) fn configure(&self, table: &toml::Table) {
        *self.rules.lock().unwrap() = [Tool::Pickaxe, Tool::Shovel].map(|tool| {
            let values = table.get(tool.section());
            let default = Rules::default();
            let get = |key, default| {
                values
                    .and_then(|section| section.get(key))
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(default)
            };
            Rules {
                damage_on_use: get("damage-on-use", default.damage_on_use),
                call_explosion_event: get("call-explosion-event", default.call_explosion_event),
            }
        });
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
            || self.active.lock().unwrap().contains(&player.uuid())
        {
            return Flow::Continue;
        }
        let Some(stack) = hand(player, Hand::Main).stack(host) else {
            return Flow::Continue;
        };
        let tool = if EXPLOSIVE_PICKAXE.matches(host, &stack) {
            Tool::Pickaxe
        } else if EXPLOSIVE_SHOVEL.matches(host, &stack) {
            Tool::Shovel
        } else {
            return Flow::Continue;
        };
        if !unlocked(host, player, &RESEARCH) {
            return Flow::Cancel;
        }
        if player.entity().is_shift_key_down(host) == Some(true) {
            return Flow::Continue;
        }

        let Some(_mining) = Mining::enter(&self.active, player.uuid()) else {
            return Flow::Continue;
        };

        let origin = event.pos.map(f64::from);
        let _ = level.explode(
            host,
            None,
            origin,
            0.0,
            false,
            ExplosionInteraction::Block,
            ExplosionSettings {
                calculator: Some(ExplosionCalculator {
                    damages_entities: false,
                    knockback_multiplier: Some(0.0),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let _ = level.play_sound(
            host,
            "minecraft:entity.generic.explode",
            origin,
            SoundSource::Blocks,
            0.2,
            1.0,
        );

        let rules = self.rules.lock().unwrap()[tool.index()];
        let positions = neighbors(event.pos);
        let positions = if rules.call_explosion_event {
            // Paper announces the original list before checking each position's normal
            // permissions. A removed position cannot be restored by the tool.
            let mut positions = level
                .filter_block_explosion(host, event.pos, &positions)
                .unwrap_or_default();
            positions.retain(|&pos| !crate::output_chest::discard_without_drops(host, level, pos));
            positions
        } else {
            positions
        };
        let mut held = Some(stack);
        for pos in positions {
            let Some(state) = level.block_state(host, pos) else {
                continue;
            };
            if tool.includes(host, level, pos, state)
                && level.allow_device_breaking(host, player.uuid(), pos)
                && player.break_block_naturally(host, pos, state, held.as_ref())
                && rules.damage_on_use
                && let Some(stack) = held.take()
            {
                held = paper_tool_wear(host, player, level, Hand::Main, &stack, rolls);
            }
        }
        Flow::Continue
    }
}

fn neighbors([x, y, z]: Position) -> Vec<Position> {
    let mut positions = Vec::with_capacity(26);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if (dx, dy, dz) != (0, 0, 0)
                    && let (Some(x), Some(y), Some(z)) =
                        (x.checked_add(dx), y.checked_add(dy), z.checked_add(dz))
                {
                    positions.push([x, y, z]);
                }
            }
        }
    }
    positions
}

// Paper's unbreakable-materials tag, plus the air and liquid state exclusions.
fn pickaxe_material(id: &str) -> bool {
    !matches!(
        id,
        "minecraft:air"
            | "minecraft:cave_air"
            | "minecraft:void_air"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:command_block"
            | "minecraft:chain_command_block"
            | "minecraft:repeating_command_block"
            | "minecraft:bedrock"
            | "minecraft:barrier"
            | "minecraft:nether_portal"
            | "minecraft:end_portal"
            | "minecraft:end_portal_frame"
            | "minecraft:end_gateway"
            | "minecraft:structure_block"
            | "minecraft:structure_void"
            | "minecraft:jigsaw"
    )
}

fn shovel_material(id: &str) -> bool {
    matches!(
        id,
        "minecraft:white_concrete_powder"
            | "minecraft:orange_concrete_powder"
            | "minecraft:magenta_concrete_powder"
            | "minecraft:light_blue_concrete_powder"
            | "minecraft:yellow_concrete_powder"
            | "minecraft:lime_concrete_powder"
            | "minecraft:pink_concrete_powder"
            | "minecraft:gray_concrete_powder"
            | "minecraft:light_gray_concrete_powder"
            | "minecraft:cyan_concrete_powder"
            | "minecraft:purple_concrete_powder"
            | "minecraft:blue_concrete_powder"
            | "minecraft:brown_concrete_powder"
            | "minecraft:green_concrete_powder"
            | "minecraft:red_concrete_powder"
            | "minecraft:black_concrete_powder"
            | "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:grass_block"
            | "minecraft:dirt_path"
            | "minecraft:rooted_dirt"
            | "minecraft:mud"
            | "minecraft:farmland"
            | "minecraft:podzol"
            | "minecraft:mycelium"
            | "minecraft:snow"
            | "minecraft:snow_block"
            | "minecraft:gravel"
            | "minecraft:clay"
            | "minecraft:soul_sand"
            | "minecraft:soul_soil"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrounding_positions_follow_paper_order_and_never_include_the_root() {
        let positions = neighbors([0; 3]);
        assert_eq!(positions.len(), 26);
        assert_eq!(positions[0], [-1; 3]);
        assert_eq!(positions[12], [0, 0, -1]);
        assert_eq!(positions[13], [0, 0, 1]);
        assert_eq!(positions[25], [1; 3]);
        assert_eq!(neighbors([i32::MAX; 3]).len(), 7);
    }

    #[test]
    fn material_filters_respect_pickaxe_blacklist_and_shovel_allowlist() {
        for id in [
            "water",
            "barrier",
            "end_portal_frame",
            "chain_command_block",
        ] {
            assert!(!pickaxe_material(&format!("minecraft:{id}")));
        }
        assert!(pickaxe_material("minecraft:ancient_debris"));
        for id in ["snow", "clay", "farmland", "black_concrete_powder"] {
            assert!(shovel_material(&format!("minecraft:{id}")));
        }
        for id in [
            "snowy_grass_block",
            "dirt_pathway",
            "packed_mud",
            "diamond_ore",
        ] {
            assert!(!shovel_material(&format!("minecraft:{id}")));
        }
    }

    #[test]
    fn the_two_tools_keep_independent_damage_and_filter_settings() {
        let config = toml::from_str(
            "[explosive-pickaxe]\ndamage-on-use = false\n[explosive-shovel]\ncall-explosion-event = true",
        ).unwrap();
        let tools = ExplosiveTools::default();
        tools.configure(&config);
        let [pickaxe, shovel] = *tools.rules.lock().unwrap();
        assert!(!pickaxe.damage_on_use && !pickaxe.call_explosion_event);
        assert!(shovel.damage_on_use && shovel.call_explosion_event);
    }
}
