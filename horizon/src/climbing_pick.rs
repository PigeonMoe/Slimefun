// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    Compound, GameMode, Hand, ItemPlace, LivingRef, PlayerRef, RightClickBlockEvent,
    ScheduledTaskEvent, ServerHost, Tag, TaskId, Text,
};

use crate::{
    hologram_projector::colored_lore,
    items::{HARDENED_METAL_INGOT, Kind, STEEL_INGOT, item},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    CLIMBING_PICK,
    "iron_pickaxe",
    "Climbing Pick",
    "aqua",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "climbing_pick",
    name: "Block Raider",
    cost: 20,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(STEEL_INGOT),
        Some(HARDENED_METAL_INGOT),
        Some(STEEL_INGOT),
        None,
        Some(Kind::Vanilla("minecraft:stick")),
        None,
        None,
        Some(Kind::Vanilla("minecraft:stick")),
        None,
    ],
    output: Stack {
        kind: CLIMBING_PICK,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) const DEFAULT_CONFIG: &str =
    "\n[climbing-pick]\ndual-wielding = true\ndamage-on-use = true\n";

const COLORS: &[&str] = &[
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

fn default_power(material: &str) -> Option<f64> {
    let material = material.strip_prefix("minecraft:")?;
    match material {
        "ice" | "packed_ice" | "frosted_ice" | "blue_ice" | "terracotta" | "stone" | "granite"
        | "andesite" | "diorite" | "netherrack" | "blackstone" | "basalt" | "calcite"
        | "deepslate" | "dripstone_block" | "smooth_basalt" | "tuff" => Some(1.0),
        "sand" | "red_sand" | "suspicious_sand" | "gravel" | "clay" => Some(0.6),
        other
            if other
                .strip_suffix("_terracotta")
                .is_some_and(|dye| COLORS.contains(&dye)) =>
        {
            Some(1.0)
        }
        other
            if other
                .strip_suffix("_concrete_powder")
                .is_some_and(|dye| COLORS.contains(&dye)) =>
        {
            Some(0.6)
        }
        _ => None,
    }
}

#[derive(Clone)]
struct Settings {
    dual_wielding: bool,
    damage_on_use: bool,
    launch_amounts: BTreeMap<String, f64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dual_wielding: true,
            damage_on_use: true,
            launch_amounts: BTreeMap::new(),
        }
    }
}

impl Settings {
    fn read(table: Option<&toml::Table>) -> Self {
        let mut settings = Self::default();
        let Some(table) = table else {
            return settings;
        };
        if let Some(value) = table.get("dual-wielding").and_then(toml::Value::as_bool) {
            settings.dual_wielding = value;
        }
        if let Some(value) = table.get("damage-on-use").and_then(toml::Value::as_bool) {
            settings.damage_on_use = value;
        }
        if let Some(surfaces) = table.get("launch-amounts").and_then(toml::Value::as_table) {
            for (material, power) in surfaces {
                let material = material.to_ascii_lowercase();
                let material = material.strip_prefix("minecraft:").unwrap_or(&material);
                let key = format!("minecraft:{material}");
                if let Some(power) = power
                    .as_float()
                    .or_else(|| power.as_integer().map(|n| n as f64))
                    && power.is_finite()
                    && power >= 0.0
                    && default_power(&key).is_some()
                {
                    settings.launch_amounts.insert(key, power);
                }
            }
        }
        settings
    }

    fn power(&self, material: &str, efficiency: u32) -> f64 {
        let Some(default) = default_power(material) else {
            return 0.0;
        };
        let power = self
            .launch_amounts
            .get(material)
            .copied()
            .unwrap_or(default);
        if power > 0.0 {
            power + f64::from(efficiency) * 0.125
        } else {
            0.0
        }
    }
}

#[derive(Default)]
pub(crate) struct Climbing {
    settings: Mutex<Settings>,
    cooldown: Mutex<BTreeMap<u128, TaskId>>,
}

impl Climbing {
    pub(crate) fn configure(&self, table: Option<&toml::Table>) {
        *self.settings.lock().unwrap() = Settings::read(table);
    }

    pub(crate) fn use_on(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        event: &RightClickBlockEvent<'_>,
        rolls: &Rolls,
    ) {
        let Some(tool) = event
            .click
            .stack
            .filter(|stack| CLIMBING_PICK.matches(host, stack))
        else {
            return;
        };
        let entity = player.entity();
        let pos = event.click.pos;
        let (Some(level), Some(position)) = (entity.level(host), entity.position(host)) else {
            return;
        };
        if !near_wall(position, pos) {
            return;
        }
        if !research::can_use(host, player, &RESEARCH) {
            let _ = player.tell(
                host,
                Text::literal("Unlock Block Raider with /sf research climbing_pick."),
            );
            return;
        }
        let held_hand = event.click.hand;
        let other_hand = match held_hand {
            Hand::Main => Hand::Off,
            Hand::Off => Hand::Main,
        };
        let settings = self.settings.lock().unwrap().clone();
        let held = |hand| ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let other = settings
            .dual_wielding
            .then(|| held(other_hand).stack(host))
            .flatten();
        if settings.dual_wielding
            && other
                .as_ref()
                .is_none_or(|item| !CLIMBING_PICK.matches(host, item))
        {
            let _ = player.tell(
                host,
                Text::literal("You must hold a Climbing Pick in both hands."),
            );
            return;
        }
        if event.click.hit.face[1] != 0 {
            return;
        }
        let Some(state) = level.block_state(host, pos) else {
            return;
        };
        let efficiency = crate::smelters_pickaxe::enchantment(host, tool, "minecraft:efficiency");
        let power = settings.power(&state.block().id(host), efficiency);
        if power <= 0.05 {
            if !settings.dual_wielding || held_hand == Hand::Main {
                let _ = player.tell(host, Text::literal("You cannot climb this material."));
            }
            return;
        }
        {
            let mut cooldown = self.cooldown.lock().unwrap();
            if cooldown.contains_key(&player.uuid()) {
                return;
            }
            let Ok(task) = host.schedule_task(4, 0) else {
                return;
            };
            cooldown.insert(player.uuid(), task);
        }
        if !entity.set_velocity(host, [0.0, power, 0.0]) {
            return;
        }
        let _ = entity.reset_fall_distance(host);
        let (swing, worn) = match other.as_ref() {
            Some(other) if rolls.next(host, level, 2) == Some(0) => (other_hand, other),
            _ => (held_hand, tool),
        };
        if settings.damage_on_use && player.game_mode(host) != Some(GameMode::Creative) {
            let _ = crate::ore_tools::paper_tool_wear(host, player, level, swing, worn, rolls);
        }
        let _ = level.block_break_effect(host, pos, state);
        let _ = LivingRef::new(entity).swing(host, swing, true);
    }

    pub(crate) fn due(&self, event: &ScheduledTaskEvent<'_>) {
        if event.owner == "slimefun" {
            self.cooldown
                .lock()
                .unwrap()
                .retain(|_, task| *task != event.id);
        }
    }

    pub(crate) fn clear(&self) {
        self.cooldown.lock().unwrap().clear();
    }
}

fn near_wall(player: [f64; 3], block: [i32; 3]) -> bool {
    let distance = player
        .into_iter()
        .zip(block)
        .map(|(player, block)| (player - (f64::from(block) + 0.5)).powi(2))
        .sum::<f64>();
    distance <= 4.4
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == CLIMBING_PICK.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(
                [
                    "",
                    "&fAllows you to climb certain surfaces",
                    "&fby right-clicking.",
                    "&fEnchant this pick with Efficiency to",
                    "&fclimb even faster!",
                ]
                .into_iter()
                .map(colored_lore)
                .collect(),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paces_match_paper_tags_overrides_and_efficiency() {
        let settings = Settings::read(Some(&toml::from_str::<toml::Table>(
            "dual-wielding = false\ndamage-on-use = false\n[launch-amounts]\nPACKED_ICE = 0.0\nRED_SAND = 0.55\nOAK_LOG = 3.0",
        ).unwrap()));
        assert_eq!(Settings::default().power("minecraft:packed_ice", 2), 1.25);
        assert_eq!(
            Settings::default().power("minecraft:yellow_concrete_powder", 0),
            0.6
        );
        assert_eq!(
            Settings::default().power("minecraft:white_glazed_terracotta", 1),
            0.0
        );
        assert_eq!(
            Settings::default().power("minecraft:suspicious_sand", 0),
            0.6
        );
        assert_eq!(settings.power("minecraft:packed_ice", 5), 0.0);
        assert_eq!(settings.power("minecraft:red_sand", 2), 0.8);
        assert_eq!(settings.power("minecraft:oak_log", 5), 0.0);
        assert!(!settings.dual_wielding && !settings.damage_on_use);
        assert!(near_wall([2.5, 70.0, 0.5], [0, 70, 0]));
        assert!(!near_wall([2.6, 70.0, 0.5], [0, 70, 0]));
    }
}
