// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockEntityRef, Compound, Hand, ItemPlace, ItemStack, LevelRef, MenuReplace, MenuSlotPlace,
    PlayerRef, ServerHost, Tag, Text,
};

use crate::{
    geo_deposits,
    items::{
        ADVANCED_CIRCUIT_BOARD, BASIC_CIRCUIT_BOARD, BRONZE_INGOT, CARBON, CARBONADO,
        CORINTHIAN_BRONZE_INGOT, ELECTRIC_MOTOR, ELECTRO_MAGNET, HeadTexture, Kind, NICKEL_INGOT,
        REDSTONE_ALLOY, REINFORCED_ALLOY_INGOT, SOLDER_INGOT, STEEL_INGOT, item,
    },
    network_recipes::{BLISTERING_INGOT_3, MEDIUM_CAPACITOR},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

const REQUIRED_COMPLEXITY: u32 = 600;

const GPS_SKIN: HeadTexture = HeadTexture {
    profile: [-1803799176, -734907139, -1390473943, -44097310],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2IwYzljMWEwMjJmNDBiNzNmMTRiNGNiYTM3YzcxOGM2YTUzM2YzYTI4NjRiNjUzNmQ1ZjQ1NjkzNGNjMWYifX19",
};
const SCANNER_SKIN: HeadTexture = HeadTexture {
    profile: [-731123774, 187709386, -1706573353, 251083501],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzJhZDhjZmViMzg3YTU2ZTNlNWJjZjg1MzQ1ZDZhNDE3YjI0MjI5Mzg4N2RiM2NlM2JhOTFmYTQwOWIyNTRiODYifX19",
};
const PUMP_SKIN: HeadTexture = HeadTexture {
    profile: [318624450, 1283537195, -1709521441, 456229524],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2FmZTFhMDQwYTQyNWUzMWE0NmQ0ZjlhOWI5ODA2ZmEyZjBjNDdlZTg0NzExY2MxOTMyZmQ4YWIzMmIyZDAzOCJ9fX0=",
};

const MINER_SKIN: HeadTexture = HeadTexture {
    profile: [1408434130, -1764738568, -1106303375, -217260035],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYTM3NzQxZjc2NGRkM2RkN2FkYWViNDNiNjNkMzk1OWViNzBlNWViMjhmMTVkNmIzNGNhYjM0YTFkMWY2MDM4NyJ9fX0=",
};

item!(
    GEO_MINER,
    "player_head",
    "GEO Miner",
    "gold",
    None,
    Some(MINER_SKIN)
);
item!(
    PORTABLE_GEO_SCANNER,
    "clock",
    "Portable Geo-Scanner",
    "aqua",
    None,
    None
);
item!(
    GPS_MARKER_TOOL,
    "redstone_torch",
    "GPS Marker Tool",
    "aqua",
    None,
    None
);

item!(
    GPS_TRANSMITTER,
    "player_head",
    "GPS Transmitter",
    "aqua",
    None,
    Some(GPS_SKIN)
);
item!(
    GPS_TRANSMITTER_2,
    "player_head",
    "Advanced GPS Transmitter",
    "red",
    None,
    Some(GPS_SKIN)
);
item!(
    GPS_TRANSMITTER_3,
    "player_head",
    "Carbonado GPS Transmitter",
    "dark_red",
    None,
    Some(GPS_SKIN)
);
item!(
    GPS_TRANSMITTER_4,
    "player_head",
    "Energized GPS Transmitter",
    "yellow",
    None,
    Some(GPS_SKIN)
);
item!(
    GPS_GEO_SCANNER,
    "player_head",
    "GPS Geo-Scanner",
    "aqua",
    None,
    Some(SCANNER_SKIN)
);
item!(
    OIL_PUMP,
    "player_head",
    "Oil Pump",
    "dark_red",
    None,
    Some(PUMP_SKIN)
);

pub(crate) const GPS_SETUP: Research = Research {
    id: "gps_setup",
    name: "Basic GPS Setup",
    cost: 28,
};
pub(crate) const UPGRADED: Research = Research {
    id: "better_gps_transmitters",
    name: "Upgraded Transmitters",
    cost: 36,
};
pub(crate) const ENERGIZED: Research = Research {
    id: "energized_gps_transmitter",
    name: "Top Tier Transmitter",
    cost: 44,
};
pub(crate) const SCANNING: Research = Research {
    id: "geo_scanner",
    name: "GEO-Scans",
    cost: 30,
};
pub(crate) const OIL: Research = Research {
    id: "oil",
    name: "Oil",
    cost: 30,
};

pub(crate) const MINER: Research = Research {
    id: "geo_miner",
    name: "GEO-Miner",
    cost: 24,
};

pub(crate) const RESEARCHES: &[Research] = &[GPS_SETUP, UPGRADED, ENERGIZED, SCANNING, OIL, MINER];

pub(crate) const MINER_CAPACITY: u32 = 512;
pub(crate) const MINER_COST: u32 = 24;
pub(crate) const MINER_WORK_TICKS: u32 = 14;
pub(crate) const MINER_OUTPUT: &[usize] = &[29, 30, 31, 32, 33, 38, 39, 40, 41, 42];
pub(crate) const MINER_SLOT_KEYS: &[&str] = &[
    "geo_29", "geo_30", "geo_31", "geo_32", "geo_33", "geo_38", "geo_39", "geo_40", "geo_41",
    "geo_42",
];
pub(crate) const MINER_BORDER: &[usize] = &[
    0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 26, 27, 35, 36, 44, 45, 53,
];
pub(crate) const MINER_OUTPUT_BORDER: &[usize] = &[
    19, 20, 21, 22, 23, 24, 25, 28, 34, 37, 43, 46, 47, 48, 49, 50, 51, 52,
];

pub(crate) const PUMP_CAPACITY: u32 = 256;
pub(crate) const PUMP_COST: u32 = 14;
pub(crate) const PUMP_WORK_TICKS: u32 = 52;

#[derive(Clone, Copy)]
pub(crate) struct Transmitter {
    pub item: Kind,
    pub capacity: u32,
    pub cost: u32,
    scale: u32,
    base: u32,
}

const TRANSMITTERS: &[Transmitter] = &[
    Transmitter {
        item: GPS_TRANSMITTER,
        capacity: 16,
        cost: 1,
        scale: 1,
        base: 0,
    },
    Transmitter {
        item: GPS_TRANSMITTER_2,
        capacity: 64,
        cost: 3,
        scale: 4,
        base: 100,
    },
    Transmitter {
        item: GPS_TRANSMITTER_3,
        capacity: 256,
        cost: 11,
        scale: 16,
        base: 500,
    },
    Transmitter {
        item: GPS_TRANSMITTER_4,
        capacity: 1024,
        cost: 43,
        scale: 64,
        base: 2100,
    },
];

impl Transmitter {
    pub(crate) fn strength(self, y: i32) -> u32 {
        (y.max(0) as u32)
            .saturating_mul(self.scale)
            .saturating_add(self.base)
    }
}

pub(crate) fn transmitter(id: &str) -> Option<&'static Transmitter> {
    TRANSMITTERS
        .iter()
        .find(|transmitter| transmitter.item.id() == id)
}

pub(crate) fn network_item(id: &str) -> Option<Kind> {
    match id {
        "GPS_GEO_SCANNER" => Some(GPS_GEO_SCANNER),
        "GPS_CONTROL_PANEL" => Some(crate::gps_controls::GPS_CONTROL_PANEL),
        "OIL_PUMP" => Some(OIL_PUMP),
        "GEO_MINER" => Some(GEO_MINER),
        "PORTABLE_GEO_SCANNER" => Some(PORTABLE_GEO_SCANNER),
        "GPS_MARKER_TOOL" => Some(GPS_MARKER_TOOL),
        _ => transmitter(id).map(|transmitter| transmitter.item),
    }
}

const fn craft(
    kind: Kind,
    research: &'static Research,
    inputs: [Option<Kind>; 9],
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack { kind, count: 1 },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        GPS_TRANSMITTER,
        &GPS_SETUP,
        [
            None,
            None,
            Some(ELECTRO_MAGNET),
            Some(STEEL_INGOT),
            Some(ADVANCED_CIRCUIT_BOARD),
            Some(STEEL_INGOT),
            Some(STEEL_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(STEEL_INGOT),
        ],
    ),
    craft(
        GPS_TRANSMITTER_2,
        &UPGRADED,
        [
            Some(GPS_TRANSMITTER),
            Some(BRONZE_INGOT),
            Some(GPS_TRANSMITTER),
            Some(BRONZE_INGOT),
            Some(CARBON),
            Some(BRONZE_INGOT),
            Some(GPS_TRANSMITTER),
            Some(BRONZE_INGOT),
            Some(GPS_TRANSMITTER),
        ],
    ),
    craft(
        GPS_TRANSMITTER_3,
        &UPGRADED,
        [
            Some(GPS_TRANSMITTER_2),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(GPS_TRANSMITTER_2),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(CARBONADO),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(GPS_TRANSMITTER_2),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(GPS_TRANSMITTER_2),
        ],
    ),
    craft(
        GPS_TRANSMITTER_4,
        &ENERGIZED,
        [
            Some(GPS_TRANSMITTER_3),
            Some(BLISTERING_INGOT_3),
            Some(GPS_TRANSMITTER_3),
            Some(NICKEL_INGOT),
            Some(CARBONADO),
            Some(NICKEL_INGOT),
            Some(GPS_TRANSMITTER_3),
            Some(BLISTERING_INGOT_3),
            Some(GPS_TRANSMITTER_3),
        ],
    ),
    craft(
        GPS_GEO_SCANNER,
        &SCANNING,
        [
            None,
            None,
            Some(ELECTRO_MAGNET),
            None,
            Some(STEEL_INGOT),
            Some(STEEL_INGOT),
            Some(ELECTRO_MAGNET),
            Some(ELECTRIC_MOTOR),
            Some(ELECTRO_MAGNET),
        ],
    ),
    craft(
        OIL_PUMP,
        &OIL,
        [
            Some(STEEL_INGOT),
            Some(MEDIUM_CAPACITOR),
            Some(STEEL_INGOT),
            Some(STEEL_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(STEEL_INGOT),
            None,
            Some(Kind::Vanilla("minecraft:bucket")),
            None,
        ],
    ),
    craft(
        GEO_MINER,
        &MINER,
        [
            Some(Kind::Vanilla("minecraft:diamond_pickaxe")),
            Some(MEDIUM_CAPACITOR),
            Some(Kind::Vanilla("minecraft:diamond_pickaxe")),
            Some(REINFORCED_ALLOY_INGOT),
            Some(OIL_PUMP),
            Some(REINFORCED_ALLOY_INGOT),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
    ),
    craft(
        GPS_MARKER_TOOL,
        &GPS_SETUP,
        [
            None,
            Some(ELECTRO_MAGNET),
            None,
            Some(Kind::Vanilla("minecraft:lapis_lazuli")),
            Some(BASIC_CIRCUIT_BOARD),
            Some(Kind::Vanilla("minecraft:lapis_lazuli")),
            Some(Kind::Vanilla("minecraft:redstone")),
            Some(REDSTONE_ALLOY),
            Some(Kind::Vanilla("minecraft:redstone")),
        ],
    ),
    craft(
        PORTABLE_GEO_SCANNER,
        &SCANNING,
        [
            Some(ELECTRO_MAGNET),
            Some(Kind::Vanilla("minecraft:compass")),
            Some(ELECTRO_MAGNET),
            Some(STEEL_INGOT),
            Some(GPS_MARKER_TOOL),
            Some(STEEL_INGOT),
            Some(SOLDER_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(SOLDER_INGOT),
        ],
    ),
];

pub(crate) fn open_scanner(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: [i32; 3],
    entity: Option<BlockEntityRef>,
    complexity: u32,
    rolls: &Rolls,
) {
    if !research::can_use(host, player, &SCANNING) {
        let _ = player.tell(
            host,
            Text::literal("Unlock GEO-Scans with /sf research geo_scanner."),
        );
        return;
    }
    if complexity < REQUIRED_COMPLEXITY {
        let _ = player.tell(host, Text::literal(format!(
            "A GEO scan requires {REQUIRED_COMPLEXITY} GPS complexity. Your network has {complexity}."
        )));
        return;
    }
    let deposits = match geo_deposits::scan(host, level, pos, rolls) {
        Ok(deposits) => deposits,
        Err(error) => {
            let _ = player.tell(host, Text::literal(error));
            return;
        }
    };
    let [x, z] = geo_deposits::chunk(pos);
    let mut filler = Kind::Vanilla("minecraft:gray_stained_glass_pane").stack(host, 1);
    if let Some(pane) = &mut filler {
        describe(pane, " ", &[]);
    }
    let mut items = vec![filler; 54];
    let dimension = level.dimension().id(host).unwrap_or_default();
    items[4] = Kind::Vanilla("minecraft:map").stack(host, 1);
    if let Some(stack) = &mut items[4] {
        describe(stack, "Chunk", &[dimension, format!("X: {x} Z: {z}")]);
    }
    for (slot, (resource, remaining)) in (10..).zip(deposits) {
        let kind = resource.item();
        let Some(mut item) = kind.stack(host, 1) else {
            continue;
        };
        let limit = crate::factory::max_stack(host, &mod_api::ItemResource::of(&item));
        let _ = item.set_count(remaining.clamp(1, limit));
        let unit = if remaining == 1 { "unit" } else { "units" };
        describe(&mut item, resource.name(), &[format!("{remaining} {unit}")]);
        items[slot] = Some(item);
    }
    let mut title = Text::literal("GEO Scan Results");
    title.style.color = Some("dark_red".into());
    let opened = match entity {
        Some(entity) => {
            host.open_block_inventory_menu(player, entity, 6, title, items, MenuReplace::CloseFirst)
        }
        None => host.open_menu(player.uuid(), 6, title, items, MenuReplace::CloseFirst),
    };
    if let Ok(Some(window)) = opened
        && player
            .back_mod_menu_slots(host, window, vec![MenuSlotPlace::Decoration; 54])
            .is_err()
    {
        host.close_menu(player.uuid());
    }
}

pub(crate) fn held_scanner_item(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
) -> Option<Kind> {
    let stack = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    }
    .stack(host)?;
    [PORTABLE_GEO_SCANNER, GPS_MARKER_TOOL]
        .into_iter()
        .find(|item| item.matches(host, &stack))
}

fn describe(item: &mut ItemStack, name: &str, lore: &[String]) {
    let styled =
        |text: &str| Tag::compound([("text", Tag::String(text.into())), ("italic", Tag::Byte(0))]);
    let _ = item.apply_components(&Compound::from([
        ("minecraft:custom_name".into(), styled(name)),
        (
            "minecraft:lore".into(),
            Tag::List(lore.iter().map(|line| styled(line)).collect()),
        ),
    ]));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transmitter_signal_and_penalties_match_the_four_paper_tiers() {
        let stats: Vec<_> = TRANSMITTERS
            .iter()
            .map(|spec| {
                (
                    spec.capacity,
                    spec.cost,
                    spec.strength(-4),
                    spec.strength(125),
                )
            })
            .collect();
        assert_eq!(
            stats,
            [
                (16, 1, 0, 125),
                (64, 3, 100, 600),
                (256, 11, 500, 2500),
                (1024, 43, 2100, 10100)
            ]
        );
    }
}
