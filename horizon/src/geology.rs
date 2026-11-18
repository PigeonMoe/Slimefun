// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, HostTransaction,
    LevelRef, MenuReplace, MenuSlotPlace, PlayerRef, ServerHost, Tag, Text,
};

use crate::{
    heated_pressure::BUCKET_OF_OIL,
    items::{
        ADVANCED_CIRCUIT_BOARD, BRONZE_INGOT, CARBON, CARBONADO, CORINTHIAN_BRONZE_INGOT,
        ELECTRIC_MOTOR, ELECTRO_MAGNET, HeadTexture, Kind, NICKEL_INGOT, STEEL_INGOT, item,
    },
    network_recipes::{BLISTERING_INGOT_3, MEDIUM_CAPACITOR},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

const REQUIRED_COMPLEXITY: u32 = 600;
const OIL_KEY: &str = "slimefun-oil";
const IMPORT_KEY: &str = "paper_import";

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

pub(crate) const RESEARCHES: &[Research] = &[GPS_SETUP, UPGRADED, ENERGIZED, SCANNING, OIL];

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
        "OIL_PUMP" => Some(OIL_PUMP),
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
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reserve {
    remaining: u32,
    paper_import: Option<u32>,
}

fn encode_reserve(value: &Reserve) -> Result<Compound, AttachmentError> {
    let number = |n: u32| {
        i32::try_from(n)
            .map(Tag::Int)
            .map_err(|_| AttachmentError::Decode)
    };
    let mut saved = Compound::from([(OIL_KEY.into(), number(value.remaining)?)]);
    if let Some(imported) = value.paper_import {
        saved.insert(IMPORT_KEY.into(), number(imported)?);
    }
    Ok(saved)
}

fn decode_reserve(value: &Compound) -> Result<Reserve, AttachmentError> {
    let number = |key: &str| match value.get(&key.into()) {
        Some(Tag::Int(value)) => u32::try_from(*value).map_err(|_| AttachmentError::Decode),
        _ => Err(AttachmentError::Decode),
    };
    let remaining = number(OIL_KEY)?;
    let paper_import = value
        .contains_key(&IMPORT_KEY.into())
        .then(|| number(IMPORT_KEY))
        .transpose()?;
    if paper_import.is_some_and(|original| remaining > original) {
        return Err(AttachmentError::Decode);
    }
    Ok(Reserve {
        remaining,
        paper_import,
    })
}

static DEPOSIT: AttachmentType<Reserve> = AttachmentType {
    namespace: "slimefun",
    name: "geo_oil",
    default: || Reserve {
        remaining: 0,
        paper_import: None,
    },
    codec: Some(AttachmentCodec {
        encode: encode_reserve,
        decode: decode_reserve,
    }),
    copy_on_death: false,
    sync: false,
};

pub(crate) fn chunk([x, _, z]: [i32; 3]) -> [i32; 2] {
    [x.div_euclid(16), z.div_euclid(16)]
}

fn holder(host: &dyn Host, level: LevelRef, chunk: [i32; 2]) -> Result<Holder, &'static str> {
    if !level.in_place(host) || host.chunk(level, chunk).is_none() {
        return Err("The GEO chunk must be loaded in the active dimension.");
    }
    Ok(Holder::Chunk(chunk))
}

pub(crate) fn supply(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Result<Option<u32>, &'static str> {
    DEPOSIT
        .get_existing(host, holder(host, level, chunk(pos))?)
        .map(|reserve| reserve.map(|reserve| reserve.remaining))
        .map_err(|_| "The GEO oil reserve could not be read.")
}

/// Caller journals this reservation with the consumed bucket and the pending output.
pub(crate) fn reserve_one(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Result<(), ()> {
    let holder = holder(host, level, chunk(pos)).map_err(|_| ())?;
    let mut saved = DEPOSIT
        .get_existing(host, holder)
        .map_err(|_| ())?
        .ok_or(())?;
    saved.remaining = saved.remaining.checked_sub(1).ok_or(())?;
    DEPOSIT.set(host, holder, saved).map_err(|_| ())
}

pub(crate) fn restore(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    imported: u32,
) -> Result<u32, &'static str> {
    let holder = holder(host, level, chunk(pos))?;
    host.transaction(|host, _| {
        let previous = DEPOSIT
            .get_existing(host, holder)
            .map_err(|_| "The GEO oil reserve could not be read.")?;
        if let Some(remaining) = imported_remaining(previous, imported)? {
            return Ok(remaining);
        }
        DEPOSIT
            .set(
                host,
                holder,
                Reserve {
                    remaining: imported,
                    paper_import: Some(imported),
                },
            )
            .map_err(|_| "The Paper GEO oil reserve could not be saved.")?;
        Ok(imported)
    })
}

fn imported_remaining(
    previous: Option<Reserve>,
    imported: u32,
) -> Result<Option<u32>, &'static str> {
    match previous {
        Some(saved) if saved.paper_import == Some(imported) => Ok(Some(saved.remaining)),
        Some(_) => Err("This chunk already has a different oil reserve."),
        None => Ok(None),
    }
}

fn default_supply(biome: &str) -> u32 {
    match biome {
        "minecraft:beach" | "minecraft:stony_shore" => 6,
        "minecraft:river" => 16,
        "minecraft:swamp" => 20,
        "minecraft:ice_spikes"
        | "minecraft:frozen_ocean"
        | "minecraft:frozen_river"
        | "minecraft:frozen_peaks"
        | "minecraft:snowy_slopes" => 24,
        "minecraft:badlands" | "minecraft:wooded_badlands" | "minecraft:eroded_badlands" => 40,
        "minecraft:desert" => 45,
        "minecraft:ocean"
        | "minecraft:cold_ocean"
        | "minecraft:warm_ocean"
        | "minecraft:lukewarm_ocean" => 64,
        "minecraft:deep_ocean" | "minecraft:deep_cold_ocean" | "minecraft:deep_lukewarm_ocean" => {
            72
        }
        "minecraft:windswept_hills"
        | "minecraft:windswept_gravelly_hills"
        | "minecraft:jagged_peaks" => 20,
        "minecraft:snowy_plains" | "minecraft:snowy_taiga" => 16,
        "minecraft:mushroom_fields" => 20,
        _ => 10,
    }
}

fn generated_supply(
    dimension: &str,
    biome: &str,
    roll: impl FnOnce(u32) -> Option<u32>,
) -> Option<u32> {
    if dimension != "minecraft:overworld" {
        return Some(0);
    }
    Some(default_supply(biome) + roll(8).filter(|&roll| roll < 8)?)
}

fn scan(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    rolls: &Rolls,
) -> Result<u32, &'static str> {
    let [x, z] = chunk(pos);
    let holder = holder(host, level, [x, z])?;
    host.transaction(|host, _| {
        if let Some(saved) = DEPOSIT
            .get_existing(host, holder)
            .map_err(|_| "The GEO oil reserve could not be read.")?
        {
            return Ok(saved.remaining);
        }
        let biome = level
            .biome_at(host, [x * 16, pos[1], z * 16])
            .ok_or("The GEO biome sample is not loaded.")?;
        let dimension = level
            .dimension()
            .id(host)
            .ok_or("The GEO dimension is unknown.")?;
        let remaining =
            generated_supply(&dimension, &biome, |bound| rolls.next(host, level, bound))
                .ok_or("Could not sample a GEO oil reserve.")?;
        DEPOSIT
            .set(
                host,
                holder,
                Reserve {
                    remaining,
                    paper_import: None,
                },
            )
            .map_err(|_| "Could not save the GEO oil reserve.")?;
        Ok(remaining)
    })
}

pub(crate) fn open_scanner(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: [i32; 3],
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
    let remaining = match scan(host, level, pos, rolls) {
        Ok(remaining) => remaining,
        Err(error) => {
            let _ = player.tell(host, Text::literal(error));
            return;
        }
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return;
    };
    let mut items = vec![None; 54];
    let [x, z] = chunk(pos);
    items[4] = Kind::Vanilla("minecraft:map").stack(host, 1);
    if let Some(info) = &mut items[4] {
        let _ = info.apply_components(&Compound::from([(
            "minecraft:custom_name".into(),
            Tag::String(format!("Chunk {x}, {z}").as_str().into()),
        )]));
    }
    items[10] = BUCKET_OF_OIL.stack(host, remaining.clamp(1, 64));
    if let Some(oil) = &mut items[10] {
        let _ = oil.apply_components(&Compound::from([(
            "minecraft:custom_name".into(),
            Tag::String(format!("Oil · {remaining} units").as_str().into()),
        )]));
    }
    if let Ok(Some(window)) = host.open_block_inventory_menu(
        player,
        entity,
        6,
        Text::literal("GEO Scan Results"),
        items,
        MenuReplace::CloseFirst,
    ) && player
        .back_mod_menu_slots(host, window, vec![MenuSlotPlace::Decoration; 54])
        .is_err()
    {
        host.close_menu(player.uuid());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oil_reserves_follow_paper_biomes_and_one_sided_uniform_deviation() {
        assert_eq!(chunk([-1, 80, -17]), [-1, -2]);
        let overworld = "minecraft:overworld";
        for (biome, base) in [
            ("minecraft:plains", 10),
            ("minecraft:stony_shore", 6),
            ("minecraft:snowy_taiga", 16),
            ("minecraft:jagged_peaks", 20),
            ("minecraft:frozen_ocean", 24),
            ("minecraft:eroded_badlands", 40),
            ("minecraft:desert", 45),
            ("minecraft:warm_ocean", 64),
            ("minecraft:deep_cold_ocean", 72),
        ] {
            let values: Vec<_> = (0..8)
                .map(|value| {
                    generated_supply(overworld, biome, |bound| {
                        assert_eq!(bound, 8);
                        Some(value)
                    })
                    .unwrap()
                })
                .collect();
            assert_eq!(values, (base..base + 8).collect::<Vec<_>>());
        }
        for dim in [
            "minecraft:the_nether",
            "minecraft:the_end",
            "example:custom",
        ] {
            assert_eq!(
                generated_supply(dim, "minecraft:desert", |_| unreachable!()),
                Some(0)
            );
        }
    }

    #[test]
    fn depleted_paper_oil_is_persisted_and_import_cannot_refill_it() {
        let exhausted = Reserve {
            remaining: 0,
            paper_import: Some(8),
        };
        let serialized = encode_reserve(&exhausted).unwrap();
        assert_eq!(serialized.get(&OIL_KEY.into()), Some(&Tag::Int(0)));
        assert_eq!(decode_reserve(&serialized).unwrap(), exhausted);
        assert_eq!(imported_remaining(Some(exhausted), 8), Ok(Some(0)));
        assert!(imported_remaining(Some(exhausted), 9).is_err());
        assert_eq!(imported_remaining(None, 0), Ok(None));
        assert!(
            imported_remaining(
                Some(Reserve {
                    remaining: 13,
                    paper_import: None
                }),
                13
            )
            .is_err()
        );
    }

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
