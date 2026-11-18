// SPDX-License-Identifier: GPL-3.0-only

//! Additional network diagrams and prerequisites from Paper SlimefunItemSetup.

use crate::{
    items::{self, HeadTexture, Kind, item},
    recipes::{CAPACITORS_RESEARCH, CraftingRecipe, POWER_ITEMS, Research, SmeltingRecipe, Stack},
};

item!(
    ANDROID_MEMORY_CORE,
    "player_head",
    "Android Memory Core",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [728422456, 1251750602, -1768418679, 1199211128],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q3OGYyYjdlNWU3NTYzOWVhN2ZiNzk2YzM1ZDM2NGM0ZGYyOGI0MjQzZTY2Yjc2Mjc3YWFkY2Q2MjYxMzM3In19fQ=="
    })
);
item!(
    BIG_CAPACITOR,
    "player_head",
    "Big Energy Capacitor",
    "green",
    None,
    Some(HeadTexture {
        profile: [-1509172905, -1359070495, -1407770378, -974355123],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0="
    })
);
item!(
    BLISTERING_INGOT_3,
    "gold_ingot",
    "Blistering Ingot",
    "gold",
    None,
    None
);
item!(
    CARBONADO_EDGED_CAPACITOR,
    "player_head",
    "Carbonado Edged Energy Capacitor",
    "green",
    None,
    Some(HeadTexture {
        profile: [-1509172905, -1359070495, -1407770378, -974355123],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0="
    })
);
item!(
    CARGO_MANAGER,
    "player_head",
    "Cargo Manager",
    "gold",
    None,
    Some(HeadTexture {
        profile: [1959847083, -50643370, -2034878792, -1859188648],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2U1MTBiYzg1MzYyYTEzMGE2ZmY5ZDkxZmYxMWQ2ZmE0NmQ3ZDE5MTJhMzQzMWY3NTE1NThlZjNjNGQ5YzIifX19"
    })
);
item!(
    CARGO_MOTOR,
    "player_head",
    "Cargo Motor",
    "dark_aqua",
    None,
    Some(HeadTexture {
        profile: [-2604251, 820655241, -1078036115, -491951808],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzhlNDdmOTlhYmNkNjQ1YTNlZjExMjJjOWQ4NTBhOTgxOTc5ZjQzMWJhMjkzMjU1YzE2ODBlOTFhYjExN2VkMzUifX19"
    })
);
item!(
    CARGO_NODE,
    "player_head",
    "Cargo Node (Connector)",
    "gray",
    None,
    Some(HeadTexture {
        profile: [-1611077357, -207605189, -1607479981, 642933381],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzA3YjdlZjZmZDc4NjQ4NjVjMzFjMWRjODdiZWQyNGFiNTk3MzU3OWY1YzY2MzhmZWNiOGRlZGViNDQzZmYwIn19fQ=="
    })
);
item!(
    CARGO_NODE_INPUT,
    "player_head",
    "Cargo Node (Input)",
    "gray",
    None,
    Some(HeadTexture {
        profile: [436854851, -1360383854, -1739883701, 809562090],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE2ZDFjMWE2OWEzZGU5ZmVjOTYyYTc3YmYzYjJlMzc2ZGQyNWM4NzNhM2Q4ZjE0ZjFkZDM0NWRhZTRjNCJ9fX0="
    })
);
item!(
    CARGO_NODE_OUTPUT,
    "player_head",
    "Cargo Node (Output)",
    "gray",
    None,
    Some(HeadTexture {
        profile: [-1525964209, -268288364, -1566029886, 411029171],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzU1YjIxZmQ0ODBjMWM0M2JmM2I5Zjg0MmM4NjliZGMzYmM1YWNjMjU5OWJmMmViNmI4YTFjOTVkY2U5NzhmIn19fQ=="
    })
);
item!(
    CARGO_NODE_OUTPUT_ADVANCED,
    "player_head",
    "Advanced Cargo Node (Output)",
    "gold",
    None,
    Some(HeadTexture {
        profile: [-1525964209, -268288364, -1566029886, 411029171],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzU1YjIxZmQ0ODBjMWM0M2JmM2I5Zjg0MmM4NjliZGMzYmM1YWNjMjU5OWJmMmViNmI4YTFjOTVkY2U5NzhmIn19fQ=="
    })
);
item!(
    ENERGIZED_CAPACITOR,
    "player_head",
    "Energized Energy Capacitor",
    "green",
    None,
    Some(HeadTexture {
        profile: [-1509172905, -1359070495, -1407770378, -974355123],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0="
    })
);
item!(
    HARDENED_GLASS,
    "light_gray_stained_glass",
    "Hardened Glass",
    "gray",
    None,
    None
);
item!(
    HOLOGRAM_PROJECTOR,
    "quartz_slab",
    "Hologram Projector",
    "aqua",
    None,
    None
);
item!(
    LARGE_CAPACITOR,
    "player_head",
    "Large Energy Capacitor",
    "green",
    None,
    Some(HeadTexture {
        profile: [-1509172905, -1359070495, -1407770378, -974355123],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0="
    })
);
item!(
    MEDIUM_CAPACITOR,
    "player_head",
    "Medium Energy Capacitor",
    "green",
    None,
    Some(HeadTexture {
        profile: [-1509172905, -1359070495, -1407770378, -974355123],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0="
    })
);
item!(
    POWER_CRYSTAL,
    "player_head",
    "Power Crystal",
    "red",
    None,
    Some(HeadTexture {
        profile: [-2065599999, 1400388830, -1310799597, -1321239554],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzUzYzFiMDM2YjZlMDM1MTdiMjg1YTgxMWJkODVlNzNmNWFiZmRhY2MxZGRmOTBkZmY5NjJlMTgwOTM0ZTMifX19"
    })
);
item!(
    SOLAR_GENERATOR_2,
    "daylight_detector",
    "Advanced Solar Generator",
    "red",
    None,
    None
);
item!(
    SOLAR_GENERATOR_3,
    "daylight_detector",
    "Carbonado Solar Generator",
    "dark_red",
    None,
    None
);
item!(
    SOLAR_GENERATOR_4,
    "daylight_detector",
    "Energized Solar Generator",
    "yellow",
    None,
    None
);
item!(
    SYNTHETIC_SAPPHIRE,
    "player_head",
    "Synthetic Sapphire",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [290563905, 1131099174, -2038908784, 1662118940],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2UzNTAzMmY0ZDdkMDFkZThlYzk5ZDg5Zjg3MjMwMTJkNGU3NGZhNzMwMjJjNGZhY2YxYjU3YzdmZjZmZjAifX19"
    })
);

pub(crate) const CAPACITOR_TEXTURES: [HeadTexture; 4] = [
    HeadTexture {
        profile: [-1509172905, -1359070495, -1407770378, -974355123],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzkxMzYxZTU3NmI0OTNjYmZkZmFlMzI4NjYxY2VkZDFhZGQ1NWZhYjRlNWViNDE4YjkyY2ViZjYyNzVmOGJiNCJ9fX0=",
    },
    HeadTexture {
        profile: [124150014, -2103951607, -2111859237, 893405653],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzMwNTMyMzM5NGE3ZDkxYmZiMzNkZjA2ZDkyYjYzY2I0MTRlZjgwZjA1NGQwNDczNGVhMDE1YTIzYzUzOSJ9fX0=",
    },
    HeadTexture {
        profile: [-1304901074, 741293198, -2099514610, -824503617],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzU1ODQ0MzJhZjZmMzgyMTY3MTIwMjU4ZDFlZWU4Yzg3YzZlNzVkOWU0NzllN2IwZDRjN2I2YWQ0OGNmZWVmIn19fQ==",
    },
    HeadTexture {
        profile: [1629418641, -1175438373, -1421709656, 1415411605],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzdhMjU2OTQxNWMxNGUzMWM5OGVjOTkzYTJmOTllNmQ2NDg0NmRiMzY3YTEzYjE5OTk2NWFkOTljNDM4Yzg2YyJ9fX0=",
    },
];

pub(crate) fn network_item(id: &str) -> Option<Kind> {
    [
        MEDIUM_CAPACITOR,
        BIG_CAPACITOR,
        LARGE_CAPACITOR,
        CARBONADO_EDGED_CAPACITOR,
        ENERGIZED_CAPACITOR,
        SOLAR_GENERATOR_2,
        SOLAR_GENERATOR_3,
        SOLAR_GENERATOR_4,
        CARGO_MANAGER,
        CARGO_NODE,
        CARGO_NODE_INPUT,
        CARGO_NODE_OUTPUT,
        CARGO_NODE_OUTPUT_ADVANCED,
    ]
    .into_iter()
    .find(|item| item.id() == id)
}

const fn vanilla(id: &'static str) -> Option<Kind> {
    Some(Kind::Vanilla(id))
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(items::BILLON_INGOT),
            Some(items::REDSTONE_ALLOY),
            Some(items::BILLON_INGOT),
            vanilla("minecraft:redstone"),
            Some(items::SMALL_CAPACITOR),
            vanilla("minecraft:redstone"),
            Some(items::BILLON_INGOT),
            Some(items::REDSTONE_ALLOY),
            Some(items::BILLON_INGOT),
        ],
        output: Stack {
            kind: MEDIUM_CAPACITOR,
            count: 1,
        },
        research: &CAPACITORS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(items::STEEL_INGOT),
            Some(items::REDSTONE_ALLOY),
            Some(items::STEEL_INGOT),
            vanilla("minecraft:redstone"),
            Some(MEDIUM_CAPACITOR),
            vanilla("minecraft:redstone"),
            Some(items::STEEL_INGOT),
            Some(items::REDSTONE_ALLOY),
            Some(items::STEEL_INGOT),
        ],
        output: Stack {
            kind: BIG_CAPACITOR,
            count: 1,
        },
        research: &CAPACITORS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(items::REINFORCED_ALLOY_INGOT),
            Some(items::REDSTONE_ALLOY),
            Some(items::REINFORCED_ALLOY_INGOT),
            vanilla("minecraft:redstone"),
            Some(BIG_CAPACITOR),
            vanilla("minecraft:redstone"),
            Some(items::REINFORCED_ALLOY_INGOT),
            Some(items::REDSTONE_ALLOY),
            Some(items::REINFORCED_ALLOY_INGOT),
        ],
        output: Stack {
            kind: LARGE_CAPACITOR,
            count: 1,
        },
        research: &HIGH_TIER_CAPACITORS,
    },
    CraftingRecipe {
        inputs: [
            Some(items::CARBONADO),
            Some(items::REDSTONE_ALLOY),
            Some(items::CARBONADO),
            vanilla("minecraft:redstone"),
            Some(LARGE_CAPACITOR),
            vanilla("minecraft:redstone"),
            Some(items::CARBONADO),
            Some(items::REDSTONE_ALLOY),
            Some(items::CARBONADO),
        ],
        output: Stack {
            kind: CARBONADO_EDGED_CAPACITOR,
            count: 1,
        },
        research: &HIGH_TIER_CAPACITORS,
    },
    CraftingRecipe {
        inputs: [
            Some(items::CARBONADO),
            Some(items::REDSTONE_ALLOY),
            Some(items::CARBONADO),
            vanilla("minecraft:nether_star"),
            Some(CARBONADO_EDGED_CAPACITOR),
            vanilla("minecraft:nether_star"),
            Some(items::CARBONADO),
            Some(items::REDSTONE_ALLOY),
            Some(items::CARBONADO),
        ],
        output: Stack {
            kind: ENERGIZED_CAPACITOR,
            count: 1,
        },
        research: &EVEN_HIGHER_TIER_CAPACITORS,
    },
    CraftingRecipe {
        inputs: [
            Some(items::SOLAR_GENERATOR),
            Some(items::ALUMINUM_INGOT),
            Some(items::SOLAR_GENERATOR),
            Some(items::ALUMINUM_INGOT),
            vanilla("minecraft:redstone"),
            Some(items::ALUMINUM_INGOT),
            Some(items::SOLAR_GENERATOR),
            Some(items::ALUMINUM_INGOT),
            Some(items::SOLAR_GENERATOR),
        ],
        output: Stack {
            kind: SOLAR_GENERATOR_2,
            count: 1,
        },
        research: &BETTER_SOLAR_GENERATORS,
    },
    CraftingRecipe {
        inputs: [
            Some(SOLAR_GENERATOR_2),
            Some(items::ALUMINUM_INGOT),
            Some(SOLAR_GENERATOR_2),
            Some(items::ALUMINUM_INGOT),
            Some(items::CARBONADO),
            Some(items::ALUMINUM_INGOT),
            Some(SOLAR_GENERATOR_2),
            Some(items::ALUMINUM_INGOT),
            Some(SOLAR_GENERATOR_2),
        ],
        output: Stack {
            kind: SOLAR_GENERATOR_3,
            count: 1,
        },
        research: &BETTER_SOLAR_GENERATORS,
    },
    CraftingRecipe {
        inputs: [
            Some(SOLAR_GENERATOR_3),
            Some(BLISTERING_INGOT_3),
            Some(SOLAR_GENERATOR_3),
            Some(BLISTERING_INGOT_3),
            Some(items::ELECTRO_MAGNET),
            Some(BLISTERING_INGOT_3),
            Some(SOLAR_GENERATOR_3),
            Some(BLISTERING_INGOT_3),
            Some(SOLAR_GENERATOR_3),
        ],
        output: Stack {
            kind: SOLAR_GENERATOR_4,
            count: 1,
        },
        research: &ENERGIZED_SOLAR_GENERATOR,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(HOLOGRAM_PROJECTOR),
            None,
            Some(items::REINFORCED_PLATE),
            Some(CARGO_MOTOR),
            Some(items::REINFORCED_PLATE),
            Some(items::ALUMINUM_BRONZE_INGOT),
            Some(ANDROID_MEMORY_CORE),
            Some(items::ALUMINUM_BRONZE_INGOT),
        ],
        output: Stack {
            kind: CARGO_MANAGER,
            count: 1,
        },
        research: &CARGO_BASICS,
    },
    CraftingRecipe {
        inputs: [
            Some(items::BRONZE_INGOT),
            Some(items::SILVER_INGOT),
            Some(items::BRONZE_INGOT),
            Some(items::SILVER_INGOT),
            Some(CARGO_MOTOR),
            Some(items::SILVER_INGOT),
            Some(items::BRONZE_INGOT),
            Some(items::SILVER_INGOT),
            Some(items::BRONZE_INGOT),
        ],
        output: Stack {
            kind: CARGO_NODE,
            count: 4,
        },
        research: &CARGO_BASICS,
    },
    CraftingRecipe {
        inputs: [
            Some(HARDENED_GLASS),
            Some(items::ELECTRO_MAGNET),
            Some(HARDENED_GLASS),
            Some(items::SILVER_INGOT),
            Some(items::ELECTRIC_MOTOR),
            Some(items::SILVER_INGOT),
            Some(HARDENED_GLASS),
            Some(items::ELECTRO_MAGNET),
            Some(HARDENED_GLASS),
        ],
        output: Stack {
            kind: CARGO_MOTOR,
            count: 4,
        },
        research: &CARGO_BASICS,
    },
    CraftingRecipe {
        inputs: [
            None,
            vanilla("minecraft:hopper"),
            None,
            Some(items::BILLON_INGOT),
            Some(CARGO_NODE),
            Some(items::BILLON_INGOT),
            None,
            vanilla("minecraft:hopper"),
            None,
        ],
        output: Stack {
            kind: CARGO_NODE_INPUT,
            count: 2,
        },
        research: &CARGO_NODES,
    },
    CraftingRecipe {
        inputs: [
            None,
            vanilla("minecraft:hopper"),
            None,
            Some(items::BRASS_INGOT),
            Some(CARGO_NODE),
            Some(items::BRASS_INGOT),
            None,
            vanilla("minecraft:hopper"),
            None,
        ],
        output: Stack {
            kind: CARGO_NODE_OUTPUT,
            count: 2,
        },
        research: &CARGO_NODES,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(CARGO_MOTOR),
            None,
            Some(items::COBALT_INGOT),
            Some(CARGO_NODE_OUTPUT),
            Some(items::COBALT_INGOT),
            None,
            Some(CARGO_MOTOR),
            None,
        ],
        output: Stack {
            kind: CARGO_NODE_OUTPUT_ADVANCED,
            count: 1,
        },
        research: &ADVANCED_OUTPUT_NODE,
    },
    CraftingRecipe {
        inputs: [
            vanilla("minecraft:redstone"),
            Some(SYNTHETIC_SAPPHIRE),
            vanilla("minecraft:redstone"),
            Some(SYNTHETIC_SAPPHIRE),
            Some(items::SYNTHETIC_DIAMOND),
            Some(SYNTHETIC_SAPPHIRE),
            vanilla("minecraft:redstone"),
            Some(SYNTHETIC_SAPPHIRE),
            vanilla("minecraft:redstone"),
        ],
        output: Stack {
            kind: POWER_CRYSTAL,
            count: 1,
        },
        research: &POWER_ITEMS,
    },
    CraftingRecipe {
        inputs: [
            vanilla("minecraft:glass"),
            vanilla("minecraft:glass"),
            vanilla("minecraft:glass"),
            vanilla("minecraft:glass"),
            Some(items::REINFORCED_PLATE),
            vanilla("minecraft:glass"),
            vanilla("minecraft:glass"),
            vanilla("minecraft:glass"),
            vanilla("minecraft:glass"),
        ],
        output: Stack {
            kind: HARDENED_GLASS,
            count: 16,
        },
        research: &HARDENED_GLASS_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(POWER_CRYSTAL),
            None,
            Some(items::ALUMINUM_BRASS_INGOT),
            Some(items::ELECTRIC_MOTOR),
            Some(items::ALUMINUM_BRASS_INGOT),
            None,
            Some(items::ALUMINUM_BRASS_INGOT),
            None,
        ],
        output: Stack {
            kind: HOLOGRAM_PROJECTOR,
            count: 3,
        },
        research: &HOLOGRAM_PROJECTOR_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(items::BRASS_INGOT),
            vanilla("minecraft:orange_stained_glass"),
            Some(items::BRASS_INGOT),
            Some(POWER_CRYSTAL),
            Some(items::TIN_DUST),
            Some(POWER_CRYSTAL),
            Some(items::BRASS_INGOT),
            vanilla("minecraft:orange_stained_glass"),
            Some(items::BRASS_INGOT),
        ],
        output: Stack {
            kind: ANDROID_MEMORY_CORE,
            count: 1,
        },
        research: &ANDROID_MEMORY_CORE_RESEARCH,
    },
];

pub(crate) const SAPPHIRE_SMELTING: SmeltingRecipe = SmeltingRecipe {
    inputs: &[
        Stack {
            kind: items::ALUMINUM_DUST,
            count: 1,
        },
        Stack {
            kind: Kind::Vanilla("minecraft:glass"),
            count: 1,
        },
        Stack {
            kind: Kind::Vanilla("minecraft:glass_pane"),
            count: 1,
        },
        Stack {
            kind: items::ALUMINUM_INGOT,
            count: 1,
        },
        Stack {
            kind: Kind::Vanilla("minecraft:lapis_lazuli"),
            count: 1,
        },
    ],
    output: Stack {
        kind: SYNTHETIC_SAPPHIRE,
        count: 1,
    },
};

pub(crate) const HIGH_TIER_CAPACITORS: Research = Research {
    id: "high_tier_capacitors",
    name: "Tier 2 Capacitors",
    cost: 32,
};

pub(crate) const EVEN_HIGHER_TIER_CAPACITORS: Research = Research {
    id: "even_higher_tier_capacitors",
    name: "Tier 3 Capacitors",
    cost: 40,
};

pub(crate) const BETTER_SOLAR_GENERATORS: Research = Research {
    id: "better_solar_generators",
    name: "Upgraded Solar Generators",
    cost: 28,
};

pub(crate) const ENERGIZED_SOLAR_GENERATOR: Research = Research {
    id: "energized_solar_generator",
    name: "Full-Time Solar Power",
    cost: 44,
};

pub(crate) const CARGO_BASICS: Research = Research {
    id: "cargo_basics",
    name: "Cargo Basics",
    cost: 30,
};

pub(crate) const CARGO_NODES: Research = Research {
    id: "cargo_nodes",
    name: "Cargo Setup",
    cost: 30,
};

pub(crate) const ADVANCED_OUTPUT_NODE: Research = Research {
    id: "advanced_output_node",
    name: "Advanced Output Node",
    cost: 24,
};

pub(crate) const HARDENED_GLASS_RESEARCH: Research = Research {
    id: "hardened_glass",
    name: "Withstanding Explosions",
    cost: 15,
};

pub(crate) const HOLOGRAM_PROJECTOR_RESEARCH: Research = Research {
    id: "hologram_projector",
    name: "Holograms",
    cost: 36,
};

pub(crate) const ANDROID_MEMORY_CORE_RESEARCH: Research = Research {
    id: "android_memory_core",
    name: "Memory Core",
    cost: 28,
};

pub(crate) const SYNTHETIC_SAPPHIRE_RESEARCH: Research = Research {
    id: "synthetic_sapphire",
    name: "Synthetic Sapphires",
    cost: 16,
};

pub(crate) const RESEARCHES: &[Research] = &[
    HIGH_TIER_CAPACITORS,
    EVEN_HIGHER_TIER_CAPACITORS,
    BETTER_SOLAR_GENERATORS,
    ENERGIZED_SOLAR_GENERATOR,
    CARGO_BASICS,
    CARGO_NODES,
    ADVANCED_OUTPUT_NODE,
    HARDENED_GLASS_RESEARCH,
    HOLOGRAM_PROJECTOR_RESEARCH,
    ANDROID_MEMORY_CORE_RESEARCH,
    SYNTHETIC_SAPPHIRE_RESEARCH,
];
