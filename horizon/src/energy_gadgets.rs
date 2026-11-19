// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{ItemStack, LevelRef, LightLayer, PlayerRef, ServerHost, Tag, Text};

use crate::{
    factory::charging_bench,
    items::{
        COPPER_INGOT, GOLD_6K, REDSTONE_ALLOY, REINFORCED_ALLOY_INGOT, SOLAR_PANEL, item, text,
    },
    network_recipes::MEDIUM_CAPACITOR,
    network_runtime,
    recipes::{CraftingRecipe, Research, SOLAR_PANEL_AND_HELMET_RESEARCH, Stack},
    research,
};

item!(MULTIMETER, "clock", "Multimeter", "yellow", None, None);
item!(
    SOLAR_HELMET,
    "iron_helmet",
    "Solar Helmet",
    "aqua",
    None,
    None
);

pub(crate) const MULTIMETER_RESEARCH: Research = Research {
    id: "multimeter",
    name: "Power Measurement",
    cost: 10,
};

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(COPPER_INGOT),
            None,
            Some(COPPER_INGOT),
            None,
            Some(REDSTONE_ALLOY),
            None,
            None,
            Some(GOLD_6K),
            None,
        ],
        output: Stack {
            kind: MULTIMETER,
            count: 1,
        },
        research: &MULTIMETER_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(REINFORCED_ALLOY_INGOT),
            Some(SOLAR_PANEL),
            Some(REINFORCED_ALLOY_INGOT),
            Some(REINFORCED_ALLOY_INGOT),
            None,
            Some(REINFORCED_ALLOY_INGOT),
            Some(MEDIUM_CAPACITOR),
            None,
            Some(MEDIUM_CAPACITOR),
        ],
        output: Stack {
            kind: SOLAR_HELMET,
            count: 1,
        },
        research: &SOLAR_PANEL_AND_HELMET_RESEARCH,
    },
];

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let detail = if id == MULTIMETER.id() {
        vec![
            text("Measures the Amount of stored", "white", false),
            text("Energy in a Block", "white", false),
        ]
    } else if id == SOLAR_HELMET.id() {
        vec![text("Charges held Items and Armor", "green", true)]
    } else {
        return None;
    };
    Some(Tag::List(
        std::iter::once(text("", "white", false))
            .chain(detail)
            .collect(),
    ))
}

pub(crate) fn measure(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: [i32; 3],
    held: &ItemStack,
) -> bool {
    if !MULTIMETER.matches(host, held) || !research::can_use(host, player, &MULTIMETER_RESEARCH) {
        return false;
    }
    let Some((stored, capacity)) = network_runtime::energy_buffer(host, level, pos) else {
        return false;
    };
    let color = |value: String, shade: &str| {
        let mut result = Text::literal(value);
        result.style.color = Some(shade.to_owned());
        result
    };
    let _ = player.tell(host, Text::literal(""));
    let _ = player.tell(
        host,
        color("Stored Energy: ".into(), "aqua")
            .append(color(
                format!("{} J", network_runtime::compact_energy(u64::from(stored))),
                "dark_aqua",
            ))
            .append(color(" / ".into(), "aqua"))
            .append(color(
                format!("{} J", network_runtime::compact_energy(u64::from(capacity))),
                "dark_aqua",
            )),
    );
    let _ = player.tell(host, Text::literal(""));
    true
}

#[derive(Clone, Copy)]
struct SolarSettings {
    enabled: bool,
    interval: i64,
    amount: f32,
}

impl Default for SolarSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 10,
            amount: 0.1,
        }
    }
}

#[derive(Default)]
pub(crate) struct SolarHelmet(Mutex<SolarSettings>);

impl SolarHelmet {
    pub(crate) fn configure(&self, table: Option<&toml::Value>) {
        let mut config = SolarSettings::default();
        if let Some(table) = table {
            if let Some(enabled) = table
                .get("enable-armor-effects")
                .and_then(toml::Value::as_bool)
            {
                config.enabled = enabled;
            }
            if let Some(interval) = table
                .get("armor-update-interval")
                .and_then(toml::Value::as_integer)
            {
                config.interval = interval.max(1);
            }
            let amount = table
                .get("charge-amount")
                .and_then(|n| n.as_float().or_else(|| n.as_integer().map(|n| n as f64)));
            if let Some(amount) = amount
                .filter(|amount| amount.is_finite())
                .map(|n| n.max(0.01) as f32)
            {
                config.amount = amount.min(f32::MAX);
            }
        }
        *self.0.lock().unwrap() = config;
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let config = *self.0.lock().unwrap();
        if !config.enabled
            || !level
                .game_time(host)
                .is_some_and(|tick| tick.rem_euclid(config.interval) == 0)
            || !level
                .dimension_type(host)
                .is_some_and(|kind| kind.has_skylight && kind.skybox == "overworld")
            || !level.day_time(host).is_some_and(sunlight_hours)
        {
            return;
        }
        for player in level.online_players(host).unwrap_or_default() {
            let Some(slots) = player.inventory_slots(host) else {
                continue;
            };
            let wearing = slots
                .iter()
                .any(|(slot, stack)| *slot == 39 && SOLAR_HELMET.matches(host, stack));
            if !wearing || !research::can_use(host, player, &SOLAR_PANEL_AND_HELMET_RESEARCH) {
                continue;
            }
            let Some(eyes) = player.entity().eye_position(host) else {
                continue;
            };
            if level.brightness(host, LightLayer::Sky, eyes.map(|n| n.floor() as i32)) != Some(15) {
                continue;
            }
            let mainhand = player
                .selected_slot(host)
                .and_then(|n| u32::try_from(n).ok());
            for (slot, stack) in slots
                .into_iter()
                .filter(|(slot, _)| matches!(*slot, 36..=38 | 40) || mainhand == Some(*slot))
            {
                if let Some(charged) = charging_bench::recharge(host, &stack, config.amount) {
                    let _ = player.set_inventory_slot(host, slot, Some(charged));
                }
            }
        }
    }
}

fn sunlight_hours(day: i64) -> bool {
    let time = day.rem_euclid(24_000);
    !(12_300..=23_850).contains(&time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_helmet_matches_papers_sunrise_and_sunset_boundaries() {
        for (tick, sunlight) in [
            (0, true),
            (12_299, true),
            (12_300, false),
            (23_850, false),
            (23_851, true),
            (36_300, false),
            (-149, true),
        ] {
            assert_eq!(sunlight_hours(tick), sunlight);
        }
    }
}
