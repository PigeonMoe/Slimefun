// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    GameMode, HostBlockStates, LevelRef, LivingRef, PlayerRef, ServerHost, SoundSource, Tag,
};

use crate::{
    ancient_altar::ANCIENT_RUNE_EARTH,
    armor_forge::ANCIENT_RUNE_ENDER,
    items::{ENDER_LUMP_2, Kind, MAGIC_LUMP_3, MAGNET, item, text},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    INFUSED_HOPPER,
    "hopper",
    "Infused Hopper",
    "dark_purple",
    None,
    None
);
item!(
    INFUSED_MAGNET,
    "player_head",
    "Infused Magnet",
    "green",
    None,
    match MAGNET {
        Kind::Slimefun(magnet) => match &magnet.texture {
            Some(texture) => Some(crate::items::HeadTexture {
                profile: texture.profile,
                value: texture.value
            }),
            None => None,
        },
        Kind::Vanilla(_) => None,
    }
);

pub(crate) const HOPPER_RESEARCH: Research = Research {
    id: "infused_hopper",
    name: "Infused Hopper",
    cost: 22,
};
pub(crate) const MAGNET_RESEARCH: Research = Research {
    id: "infused_magnet",
    name: "Infused Magnets",
    cost: 18,
};

const OBSIDIAN: Option<Kind> = Some(Kind::Vanilla("minecraft:obsidian"));
const HOPPER: Option<Kind> = Some(Kind::Vanilla("minecraft:hopper"));
pub(crate) const ALTAR: CraftingRecipe = CraftingRecipe {
    inputs: [
        OBSIDIAN,
        Some(ANCIENT_RUNE_EARTH),
        HOPPER,
        Some(ANCIENT_RUNE_ENDER),
        Some(INFUSED_MAGNET),
        Some(ANCIENT_RUNE_ENDER),
        HOPPER,
        Some(ANCIENT_RUNE_EARTH),
        OBSIDIAN,
    ],
    output: Stack {
        kind: INFUSED_HOPPER,
        count: 1,
    },
    research: &HOPPER_RESEARCH,
};
pub(crate) const MAGIC: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(MAGIC_LUMP_3),
        None,
        Some(MAGIC_LUMP_3),
        Some(ENDER_LUMP_2),
        Some(MAGNET),
        Some(ENDER_LUMP_2),
        Some(MAGIC_LUMP_3),
        None,
        Some(MAGIC_LUMP_3),
    ],
    output: Stack {
        kind: INFUSED_MAGNET,
        count: 1,
    },
    research: &MAGNET_RESEARCH,
};

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let lines: &[&str] = match id {
        "INFUSED_HOPPER" => &[
            "",
            "Automatically picks up nearby Items in a 7x7x7",
            "Radius when placed.",
        ],
        "INFUSED_MAGNET" => &[
            "",
            "Magical infused Magnets",
            "attract nearby Items",
            "as long as it is somewhere in",
            "your Inventory",
            "",
            "Hold Shift to pick up nearby Items",
        ],
        _ => return None,
    };
    Some(Tag::List(
        lines
            .iter()
            .map(|line| text(line, "white", false))
            .collect(),
    ))
}

#[derive(Clone, Copy)]
pub(crate) struct Settings {
    radius: f64,
    magnet_radius: f64,
    silent: bool,
    toggleable: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            radius: 3.5,
            magnet_radius: 6.0,
            silent: false,
            toggleable: false,
        }
    }
}

impl Settings {
    pub(crate) fn read_config(data: &toml::Table) -> Self {
        let mut result = Self::default();
        let radius = |section: &toml::Value, field| {
            section
                .get(field)
                .and_then(|value| {
                    value
                        .as_float()
                        .or_else(|| value.as_integer().map(|n| n as f64))
                })
                .filter(|n| n.is_finite())
                .map(|n| n.max(0.1))
        };
        if let Some(hopper) = data.get("infused-hopper") {
            result.radius = radius(hopper, "radius").unwrap_or(result.radius);
            result.silent = hopper
                .get("silent")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            result.toggleable = hopper
                .get("toggleable-with-redstone")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
        }
        if let Some(magnet) = data.get("infused-magnet") {
            result.magnet_radius = radius(magnet, "pickup-radius").unwrap_or(result.magnet_radius);
        }
        result
    }
}

#[derive(Clone, Copy)]
struct Capture {
    level: LevelRef,
    next: i64,
    radius: f64,
}

#[derive(Default)]
pub(crate) struct Magnets(Mutex<BTreeMap<u128, Capture>>);

impl Magnets {
    pub(crate) fn toggle(
        &self,
        host: &dyn ServerHost,
        player: PlayerRef,
        sneaking: bool,
        settings: Settings,
    ) {
        if !sneaking {
            self.stop(player.uuid());
            return;
        }
        let allowed = player.inventory_slots(host).is_some_and(|items| {
            items
                .iter()
                .any(|(_, stack)| INFUSED_MAGNET.matches(host, stack))
        });
        if !allowed || !research::can_use(host, player, &MAGNET_RESEARCH) {
            return;
        }
        let Some(level) = player.entity().level(host) else {
            return;
        };
        let Some(next) = level.game_time(host) else {
            return;
        };
        self.0.lock().unwrap().insert(
            player.uuid(),
            Capture {
                level,
                next,
                radius: settings.magnet_radius,
            },
        );
    }

    pub(crate) fn stop(&self, player: u128) {
        self.0.lock().unwrap().remove(&player);
    }
    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let Some(now) = level.game_time(host) else {
            return;
        };
        for player in level.online_players(host).unwrap_or_default() {
            let radius = {
                let mut active = self.0.lock().unwrap();
                let Some(capture) = active.get_mut(&player.uuid()) else {
                    continue;
                };
                if capture.level == level && capture.next > now {
                    continue;
                }
                capture.level = level;
                capture.next = now.saturating_add(8);
                capture.radius
            };
            let entity = player.entity();
            if entity.is_crouching(host) != Some(true)
                || LivingRef::new(entity)
                    .health(host)
                    .is_some_and(|health| health <= 0.0)
            {
                self.stop(player.uuid());
                continue;
            }
            if player.game_mode(host) == Some(GameMode::Spectator) {
                continue;
            }
            let Some(position) = entity.position(host) else {
                continue;
            };
            let Some(bounds) = entity.bounding_box(host) else {
                continue;
            };
            if pull(host, level, bounds, position, radius, 0.3, None) {
                let _ = player.play_sound(
                    host,
                    "minecraft:entity.enderman.teleport",
                    SoundSource::Players,
                    0.25,
                    0.9,
                );
            }
        }
    }
}

pub(crate) fn tick_hopper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    settings: Settings,
) {
    if settings.toggleable
        && !level.block_state(host, pos).is_some_and(|hopper| {
            hopper
                .property(&host.block_states(), "enabled")
                .ok()
                .flatten()
                .as_deref()
                == Some("true")
        })
    {
        return;
    }
    let [x, y, z] = pos.map(f64::from);
    let position = [x + 0.5, y + 1.2, z + 0.5];
    if pull(
        host,
        level,
        [position; 2],
        position,
        settings.radius,
        0.25,
        Some([0.0, 0.1, 0.0]),
    ) && !settings.silent
    {
        let _ = level.play_sound(
            host,
            "minecraft:entity.enderman.teleport",
            [x, y, z],
            SoundSource::Blocks,
            0.5,
            2.0,
        );
    }
}

fn pull(
    host: &mut dyn ServerHost,
    level: LevelRef,
    [min, max]: [[f64; 3]; 2],
    target: [f64; 3],
    radius: f64,
    min_distance_squared: f64,
    motion: Option<[f64; 3]>,
) -> bool {
    let nearby = level
        .entities_in(host, min.map(|n| n - radius), max.map(|n| n + radius))
        .unwrap_or_default();
    let mut moved = false;
    for entity in nearby {
        let Some(item) = host.item_entity(entity) else {
            continue;
        };
        if item.pickup_delay(host) != Some(0)
            || !entity.position(host).is_some_and(|from| {
                from.into_iter()
                    .zip(target)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    > min_distance_squared
            })
        {
            continue;
        }
        if let Some(motion) = motion {
            entity.set_velocity(host, motion);
        }
        moved |= entity.teleport_to(host, target, None);
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_gadget_defaults_and_toggle_apply_separately() {
        let defaults = Settings::default();
        assert_eq!(defaults.radius, 3.5);
        assert_eq!(defaults.magnet_radius, 6.0);
        assert!(!defaults.silent && !defaults.toggleable);
        let config: toml::Table = "[infused-hopper]\nradius = 8\nsilent=true\ntoggleable-with-redstone=true\n[infused-magnet]\npickup-radius=0.0".parse().unwrap();
        let applied = Settings::read_config(&config);
        assert_eq!(applied.radius, 8.0);
        assert_eq!(applied.magnet_radius, 0.1);
        assert!(applied.silent && applied.toggleable);
    }

    #[test]
    fn hopper_and_magnet_are_crafted_at_the_original_distinct_stations() {
        let altar = crate::ancient_altar::CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == INFUSED_HOPPER)
            .unwrap();
        assert_eq!(altar.inputs[4], Some(INFUSED_MAGNET));
        assert_eq!(altar.inputs[3], Some(ANCIENT_RUNE_ENDER));
        assert_eq!(altar.inputs[5], Some(ANCIENT_RUNE_ENDER));
        assert_eq!(altar.research.cost, 22);
        let magnet = crate::recipes::magic_crafting()
            .find(|recipe| recipe.output.kind == INFUSED_MAGNET)
            .unwrap();
        assert_eq!(magnet.inputs[4], Some(MAGNET));
        assert_eq!(magnet.research.cost, 18);
    }
}
