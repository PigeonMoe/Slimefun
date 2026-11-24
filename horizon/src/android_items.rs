// SPDX-License-Identifier: GPL-3.0-only

use mod_api::Tag;

use crate::{
    android_program::{self, Mode, Tier},
    factory::reactors::NUCLEAR_REACTOR,
    fuel_generators::{COAL_GENERATOR, COMBUSTION_REACTOR},
    geology::{GPS_TRANSMITTER, GPS_TRANSMITTER_2, GPS_TRANSMITTER_3},
    heated_pressure::PLASTIC_SHEET,
    hologram_projector::colored_lore,
    items::{BASIC_CIRCUIT_BOARD, ELECTRIC_MOTOR, HeadTexture, Kind, STEEL_INGOT, item},
    network_recipes::{ANDROID_MEMORY_CORE, BLISTERING_INGOT_3, POWER_CRYSTAL},
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    PROGRAMMABLE_ANDROID,
    "player_head",
    "Programmable Android (Normal)",
    "red",
    None,
    Some(HeadTexture {
        profile: [-611664877, -1615382484, -1654842984, -1366702800],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzUwM2NiN2VkODQ1ZTdhNTA3ZjU2OWFmYzY0N2M0N2FjNDgzNzcxNDY1YzlhNjc5YTU0NTk0Yzc2YWZiYSJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_FARMER,
    "player_head",
    "Programmable Android (Farmer)",
    "red",
    None,
    Some(HeadTexture {
        profile: [1427415659, 644231234, -1727825369, 1915481921],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZjlkMzMzNTdlODQxODgyM2JmNzgzZGU5MmRlODAyOTFiNGViZDM5MmFlYzg3MDY2OThlMDY4OTZkNDk4ZjYifX19"
    })
);

item!(
    PROGRAMMABLE_ANDROID_MINER,
    "player_head",
    "Programmable Android (Miner)",
    "red",
    None,
    Some(HeadTexture {
        profile: [-1482628880, -941671533, -1131562688, -1328382864],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZTYzOGEyODU0MWFiM2FlMGE3MjNkNTU3ODczOGUwODc1ODM4OGVjNGMzMzI0N2JkNGNhMTM0ODJhZWYzMzQifX19"
    })
);
item!(
    PROGRAMMABLE_ANDROID_WOODCUTTER,
    "player_head",
    "Programmable Android (Woodcutter)",
    "red",
    None,
    Some(HeadTexture {
        profile: [-606384552, 1392787270, -2107758760, 747000699],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZDMyYTgxNDUxMDE0MjIwNTE2OWExYWQzMmYwYTc0NWYxOGU5Y2I2YzY2ZWU2NGVjYTJlNjViYWJkZWY5ZmYifX19"
    })
);
item!(
    PROGRAMMABLE_ANDROID_BUTCHER,
    "player_head",
    "Programmable Android (Butcher)",
    "red",
    None,
    Some(HeadTexture {
        profile: [630918957, 1839935957, -1716904371, 340024933],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvM2I0NzJkZjBhZDlhM2JlODhmMmU1ZDVkNDIyZDAyYjExNmQ2NGQ4ZGYxNDc1ZWQzMmU1NDZhZmM4NGIzMSJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_FISHERMAN,
    "player_head",
    "Programmable Android (Fisherman)",
    "red",
    None,
    Some(HeadTexture {
        profile: [1564292529, 1059009014, -1634795959, -458582326],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzQ1ZTg3MzNhNzMxMTQzMzNiOThiMzYwMTc1MTI0MTcyMmY0NzEzZTFhMWE1ZDM2ZmJiMTMyNDkzZjFjNyJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_2,
    "player_head",
    "Advanced Programmable Android (Normal)",
    "red",
    None,
    Some(HeadTexture {
        profile: [-611664877, -1615382484, -1654842984, -1366702800],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzUwM2NiN2VkODQ1ZTdhNTA3ZjU2OWFmYzY0N2M0N2FjNDgzNzcxNDY1YzlhNjc5YTU0NTk0Yzc2YWZiYSJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_2_FISHERMAN,
    "player_head",
    "Advanced Programmable Android (Fisherman)",
    "red",
    None,
    Some(HeadTexture {
        profile: [1564292529, 1059009014, -1634795959, -458582326],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzQ1ZTg3MzNhNzMxMTQzMzNiOThiMzYwMTc1MTI0MTcyMmY0NzEzZTFhMWE1ZDM2ZmJiMTMyNDkzZjFjNyJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_2_FARMER,
    "player_head",
    "Advanced Programmable Android (Farmer)",
    "red",
    None,
    Some(HeadTexture {
        profile: [1427415659, 644231234, -1727825369, 1915481921],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZjlkMzMzNTdlODQxODgyM2JmNzgzZGU5MmRlODAyOTFiNGViZDM5MmFlYzg3MDY2OThlMDY4OTZkNDk4ZjYifX19"
    })
);
item!(
    PROGRAMMABLE_ANDROID_2_BUTCHER,
    "player_head",
    "Advanced Programmable Android (Butcher)",
    "red",
    None,
    Some(HeadTexture {
        profile: [630918957, 1839935957, -1716904371, 340024933],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvM2I0NzJkZjBhZDlhM2JlODhmMmU1ZDVkNDIyZDAyYjExNmQ2NGQ4ZGYxNDc1ZWQzMmU1NDZhZmM4NGIzMSJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_3,
    "player_head",
    "Empowered Programmable Android (Normal)",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [-611664877, -1615382484, -1654842984, -1366702800],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzUwM2NiN2VkODQ1ZTdhNTA3ZjU2OWFmYzY0N2M0N2FjNDgzNzcxNDY1YzlhNjc5YTU0NTk0Yzc2YWZiYSJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_3_FISHERMAN,
    "player_head",
    "Empowered Programmable Android (Fisherman)",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [1564292529, 1059009014, -1634795959, -458582326],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzQ1ZTg3MzNhNzMxMTQzMzNiOThiMzYwMTc1MTI0MTcyMmY0NzEzZTFhMWE1ZDM2ZmJiMTMyNDkzZjFjNyJ9fX0="
    })
);
item!(
    PROGRAMMABLE_ANDROID_3_BUTCHER,
    "player_head",
    "Empowered Programmable Android (Butcher)",
    "yellow",
    None,
    Some(HeadTexture {
        profile: [630918957, 1839935957, -1716904371, 340024933],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvM2I0NzJkZjBhZDlhM2JlODhmMmU1ZDVkNDIyZDAyYjExNmQ2NGQ4ZGYxNDc1ZWQzMmU1NDZhZmM4NGIzMSJ9fX0="
    })
);

item!(
    ANDROID_INTERFACE_ITEMS,
    "dispenser",
    "Android Interface (Items)",
    "gray",
    None,
    None
);
item!(
    ANDROID_INTERFACE_FUEL,
    "dispenser",
    "Android Interface (Fuel)",
    "gray",
    None,
    None
);

pub(crate) const INTERFACES: Research = Research {
    id: "android_interfaces",
    name: "Android Interfaces",
    cost: 26,
};

pub(crate) const RESEARCH: Research = Research {
    id: "programmable_androids",
    name: "Programmable Androids",
    cost: 50,
};

const BUTCHER: Research = Research {
    id: "butcher_androids",
    name: "Butcher Androids",
    cost: 32,
};
const ADVANCED_BUTCHER: Research = Research {
    id: "advanced_butcher_android",
    name: "Advanced Androids - Butcher",
    cost: 30,
};
const EMPOWERED_BUTCHER: Research = Research {
    id: "empowered_butcher_android",
    name: "Empowered Androids - Butcher",
    cost: 30,
};
const ADVANCED: Research = Research {
    id: "advanced_android",
    name: "Advanced Androids",
    cost: 60,
};
const ADVANCED_FARMER: Research = Research {
    id: "advanced_farmer_android",
    name: "Advanced Androids - Farmer",
    cost: 30,
};
const ADVANCED_FISHERMAN: Research = Research {
    id: "advanced_fisherman_android",
    name: "Advanced Androids - Fisherman",
    cost: 30,
};
const EMPOWERED: Research = Research {
    id: "empowered_android",
    name: "Empowered Androids",
    cost: 60,
};
const EMPOWERED_FISHERMAN: Research = Research {
    id: "empowered_fisherman_android",
    name: "Empowered Androids - Fisherman",
    cost: 30,
};

pub(crate) const RESEARCHES: &[&Research] = &[
    &RESEARCH,
    &INTERFACES,
    &BUTCHER,
    &ADVANCED_BUTCHER,
    &EMPOWERED_BUTCHER,
    &ADVANCED,
    &ADVANCED_FISHERMAN,
    &ADVANCED_FARMER,
    &EMPOWERED,
    &EMPOWERED_FISHERMAN,
];

const ROD: Kind = Kind::Vanilla("minecraft:fishing_rod");
const PICKAXE: Kind = Kind::Vanilla("minecraft:diamond_pickaxe");
const AXE: Kind = Kind::Vanilla("minecraft:diamond_axe");
const HOE: Kind = Kind::Vanilla("minecraft:diamond_hoe");
const CHEST: Kind = Kind::Vanilla("minecraft:chest");

const fn fisherman(frame: Kind, kind: Kind, research: &'static Research) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(ROD),
            Some(frame),
            Some(ROD),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack { kind, count: 1 },
        research,
    }
}

const fn butcher(
    frame: Kind,
    gps: Kind,
    kind: Kind,
    research: &'static Research,
) -> CraftingRecipe {
    let sword = Kind::Vanilla("minecraft:diamond_sword");
    CraftingRecipe {
        inputs: [
            None,
            Some(gps),
            None,
            Some(sword),
            Some(frame),
            Some(sword),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack { kind, count: 1 },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(PLASTIC_SHEET),
            Some(STEEL_INGOT),
            Some(PLASTIC_SHEET),
            Some(STEEL_INGOT),
            Some(BASIC_CIRCUIT_BOARD),
            Some(Kind::Vanilla("minecraft:blue_stained_glass")),
            Some(PLASTIC_SHEET),
            Some(STEEL_INGOT),
            Some(PLASTIC_SHEET),
        ],
        output: Stack {
            kind: ANDROID_INTERFACE_ITEMS,
            count: 1,
        },
        research: &INTERFACES,
    },
    CraftingRecipe {
        inputs: [
            Some(PLASTIC_SHEET),
            Some(STEEL_INGOT),
            Some(PLASTIC_SHEET),
            Some(Kind::Vanilla("minecraft:red_stained_glass")),
            Some(BASIC_CIRCUIT_BOARD),
            Some(STEEL_INGOT),
            Some(PLASTIC_SHEET),
            Some(STEEL_INGOT),
            Some(PLASTIC_SHEET),
        ],
        output: Stack {
            kind: ANDROID_INTERFACE_FUEL,
            count: 1,
        },
        research: &INTERFACES,
    },
    CraftingRecipe {
        inputs: [
            Some(PLASTIC_SHEET),
            Some(ANDROID_MEMORY_CORE),
            Some(PLASTIC_SHEET),
            Some(COAL_GENERATOR),
            Some(ELECTRIC_MOTOR),
            Some(Kind::Vanilla("minecraft:chest")),
            Some(PLASTIC_SHEET),
            Some(PLASTIC_SHEET),
            Some(PLASTIC_SHEET),
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID,
            count: 1,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(Kind::Vanilla("minecraft:diamond_hoe")),
            Some(PROGRAMMABLE_ANDROID),
            Some(Kind::Vanilla("minecraft:diamond_hoe")),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_FARMER,
            count: 1,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(PICKAXE),
            Some(PROGRAMMABLE_ANDROID),
            Some(PICKAXE),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_MINER,
            count: 1,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(AXE),
            Some(PROGRAMMABLE_ANDROID),
            Some(AXE),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_WOODCUTTER,
            count: 1,
        },
        research: &RESEARCH,
    },
    fisherman(
        PROGRAMMABLE_ANDROID,
        PROGRAMMABLE_ANDROID_FISHERMAN,
        &RESEARCH,
    ),
    CraftingRecipe {
        inputs: [
            Some(PLASTIC_SHEET),
            Some(ANDROID_MEMORY_CORE),
            Some(PLASTIC_SHEET),
            Some(COMBUSTION_REACTOR),
            Some(PROGRAMMABLE_ANDROID),
            Some(CHEST),
            Some(PLASTIC_SHEET),
            Some(POWER_CRYSTAL),
            Some(PLASTIC_SHEET),
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_2,
            count: 1,
        },
        research: &ADVANCED,
    },
    fisherman(
        PROGRAMMABLE_ANDROID_2,
        PROGRAMMABLE_ANDROID_2_FISHERMAN,
        &ADVANCED_FISHERMAN,
    ),
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(HOE),
            Some(PROGRAMMABLE_ANDROID_2),
            Some(HOE),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_2_FARMER,
            count: 1,
        },
        research: &ADVANCED_FARMER,
    },
    CraftingRecipe {
        inputs: [
            Some(PLASTIC_SHEET),
            Some(ANDROID_MEMORY_CORE),
            Some(PLASTIC_SHEET),
            Some(NUCLEAR_REACTOR),
            Some(PROGRAMMABLE_ANDROID_2),
            Some(CHEST),
            Some(BLISTERING_INGOT_3),
            Some(POWER_CRYSTAL),
            Some(BLISTERING_INGOT_3),
        ],
        output: Stack {
            kind: PROGRAMMABLE_ANDROID_3,
            count: 1,
        },
        research: &EMPOWERED,
    },
    fisherman(
        PROGRAMMABLE_ANDROID_3,
        PROGRAMMABLE_ANDROID_3_FISHERMAN,
        &EMPOWERED_FISHERMAN,
    ),
    butcher(
        PROGRAMMABLE_ANDROID,
        GPS_TRANSMITTER,
        PROGRAMMABLE_ANDROID_BUTCHER,
        &BUTCHER,
    ),
    butcher(
        PROGRAMMABLE_ANDROID_2,
        GPS_TRANSMITTER_2,
        PROGRAMMABLE_ANDROID_2_BUTCHER,
        &ADVANCED_BUTCHER,
    ),
    butcher(
        PROGRAMMABLE_ANDROID_3,
        GPS_TRANSMITTER_3,
        PROGRAMMABLE_ANDROID_3_BUTCHER,
        &EMPOWERED_BUTCHER,
    ),
];

pub(crate) fn item(id: &str) -> Option<Kind> {
    [
        PROGRAMMABLE_ANDROID,
        PROGRAMMABLE_ANDROID_FARMER,
        ANDROID_INTERFACE_ITEMS,
        ANDROID_INTERFACE_FUEL,
        PROGRAMMABLE_ANDROID_MINER,
        PROGRAMMABLE_ANDROID_WOODCUTTER,
        PROGRAMMABLE_ANDROID_BUTCHER,
        PROGRAMMABLE_ANDROID_FISHERMAN,
        PROGRAMMABLE_ANDROID_2,
        PROGRAMMABLE_ANDROID_2_FISHERMAN,
        PROGRAMMABLE_ANDROID_2_FARMER,
        PROGRAMMABLE_ANDROID_2_BUTCHER,
        PROGRAMMABLE_ANDROID_3,
        PROGRAMMABLE_ANDROID_3_FISHERMAN,
        PROGRAMMABLE_ANDROID_3_BUTCHER,
    ]
    .into_iter()
    .find(|kind| kind.id() == id)
}

pub(crate) fn kind_and_research(mode: Mode) -> (Kind, &'static Research) {
    match mode {
        Mode::Normal(Tier::Solid) => (PROGRAMMABLE_ANDROID, &RESEARCH),
        Mode::Farmer => (PROGRAMMABLE_ANDROID_FARMER, &RESEARCH),
        Mode::AdvancedFarmer => (PROGRAMMABLE_ANDROID_2_FARMER, &ADVANCED_FARMER),
        Mode::Woodcutter => (PROGRAMMABLE_ANDROID_WOODCUTTER, &RESEARCH),
        Mode::Miner => (PROGRAMMABLE_ANDROID_MINER, &RESEARCH),
        Mode::Fisherman(Tier::Solid) => (PROGRAMMABLE_ANDROID_FISHERMAN, &RESEARCH),
        Mode::Normal(Tier::Liquid) => (PROGRAMMABLE_ANDROID_2, &ADVANCED),
        Mode::Fisherman(Tier::Liquid) => (PROGRAMMABLE_ANDROID_2_FISHERMAN, &ADVANCED_FISHERMAN),
        Mode::Normal(Tier::Nuclear) => (PROGRAMMABLE_ANDROID_3, &EMPOWERED),
        Mode::Fisherman(Tier::Nuclear) => (PROGRAMMABLE_ANDROID_3_FISHERMAN, &EMPOWERED_FISHERMAN),
        Mode::Butcher(Tier::Solid) => (PROGRAMMABLE_ANDROID_BUTCHER, &BUTCHER),
        Mode::Butcher(Tier::Liquid) => (PROGRAMMABLE_ANDROID_2_BUTCHER, &ADVANCED_BUTCHER),
        Mode::Butcher(Tier::Nuclear) => (PROGRAMMABLE_ANDROID_3_BUTCHER, &EMPOWERED_BUTCHER),
    }
}

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let mode = android_program::mode(id)?;
    let tier = mode.tier();
    let function = match mode {
        Mode::Normal(_) => "None",
        Mode::Farmer | Mode::AdvancedFarmer => "Farming",
        Mode::Woodcutter => "Woodcutting",
        Mode::Miner => "Mining",
        Mode::Fisherman(_) => "Fishing",
        Mode::Butcher(_) => "Slaughtering",
    };
    let mut lore = vec![
        colored_lore(""),
        colored_lore(&format!("&8⇨ &7Function: {function}")),
    ];
    if matches!(mode, Mode::Fisherman(_)) {
        lore.push(colored_lore(&format!(
            "&8⇨ &7Success Rate: {}%",
            10 * tier.number()
        )));
    }
    if matches!(mode, Mode::Butcher(_)) {
        let damage = if tier == Tier::Nuclear {
            20
        } else {
            4 * tier.number()
        };
        lore.push(colored_lore(&format!("&8⇨ &7Damage: {damage}")));
    }
    let efficiency = match tier {
        Tier::Solid => "1.0",
        Tier::Liquid => "1.5",
        Tier::Nuclear if matches!(mode, Mode::Fisherman(_) | Mode::Butcher(_)) => "8.0",
        Tier::Nuclear => "3.0",
    };
    lore.push(colored_lore(&format!("&8⇨ &7Fuel Efficiency: {efficiency}x")));
    if mode == Mode::AdvancedFarmer {
        lore.push(colored_lore("&8⇨ &7Can also harvest Plants from ExoticGarden"));
    }
    Some(Tag::List(lore))
}
