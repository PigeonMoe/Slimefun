// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Flow, Host, Identifier, ItemStack, LivingEntityUseItemFinishEvent,
    LivingEntityUseItemStartEvent, ServerHost, Text,
};

use crate::{
    advanced_processing::{Family, Machine, Plan},
    heated_pressure::HEATED_PRESSURE_CHAMBER,
    items::{
        ALUMINUM_BRASS_INGOT, ALUMINUM_INGOT, CARBON, CARBON_CHUNK, CARBONADO, COMPRESSED_CARBON,
        ELECTRIC_MOTOR, ELECTRO_MAGNET, HEATING_COIL, HeadTexture, Kind, RAW_CARBONADO,
        REINFORCED_ALLOY_INGOT, SILVER_INGOT, SYNTHETIC_DIAMOND, item,
    },
    network_recipes::MEDIUM_CAPACITOR,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    CARBON_PRESS,
    "black_stained_glass",
    "Carbon Press",
    "red",
    None,
    None
);
item!(
    CARBON_PRESS_2,
    "black_stained_glass",
    "Carbon Press - II",
    "red",
    None,
    None
);
item!(
    CARBON_PRESS_3,
    "black_stained_glass",
    "Carbon Press - III",
    "red",
    None,
    None
);
item!(
    FREEZER,
    "light_blue_stained_glass",
    "Freezer",
    "aqua",
    None,
    None
);
item!(
    FREEZER_2,
    "light_blue_stained_glass",
    "Freezer (II)",
    "aqua",
    None,
    None
);
item!(
    FREEZER_3,
    "light_blue_stained_glass",
    "Freezer (III)",
    "aqua",
    None,
    None
);
item!(AUTO_DRIER, "smoker", "Auto Drier", "gold", None, None);
item!(BEEF_JERKY, "cooked_beef", "Beef Jerky", "gold", None, None);
item!(
    PORK_JERKY,
    "cooked_porkchop",
    "Pork Jerky",
    "gold",
    None,
    None
);
item!(
    CHICKEN_JERKY,
    "cooked_chicken",
    "Chicken Jerky",
    "gold",
    None,
    None
);
item!(
    MUTTON_JERKY,
    "cooked_mutton",
    "Mutton Jerky",
    "gold",
    None,
    None
);
item!(
    RABBIT_JERKY,
    "cooked_rabbit",
    "Rabbit Jerky",
    "gold",
    None,
    None
);
item!(FISH_JERKY, "cooked_cod", "Fish Jerky", "gold", None, None);
item!(
    COOLING_UNIT,
    "player_head",
    "Cooling Unit",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [1841106021, -168742009, -1734244900, 776181976],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzc1NGJhZDg2Yzk5ZGY3ODBjODg5YTEwOThmNzc2NDhlYWQ3Mzg1Y2MxZGRiMDkzZGE1YTdkOGM0YzJhZTU0ZCJ9fX0="
    })
);
// Paper's saved ID has retained this spelling.
item!(
    REACTOR_COLLANT_CELL,
    "player_head",
    "Reactor Coolant Cell",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [1366973696, 2138782559, -1502215589, 1161661665],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2RlNDA3M2JlNDBjYjNkZWIzMTBhMGJlOTU5YjRjYWM2OGU4MjUzNzI3MjhmYWZiNmMyOTczZTRlN2MzMyJ9fX0="
    })
);

const PRESS: Research = Research {
    id: "carbon_press",
    name: "Carbon Press",
    cost: 28,
};
const BETTER_PRESS: Research = Research {
    id: "better_carbon_press",
    name: "Upgraded Carbon Press",
    cost: 26,
};
const ULTIMATE_PRESS: Research = Research {
    id: "high_tier_carbon_press",
    name: "Ultimate Carbon Press",
    cost: 32,
};
const FREEZING: Research = Research {
    id: "freezer",
    name: "Mr Freeze",
    cost: 20,
};
const BETTER_FREEZING: Research = Research {
    id: "better_freezer",
    name: "Upgraded Freezer",
    cost: 29,
};
const DRYING: Research = Research {
    id: "auto_drier",
    name: "A Dry Day",
    cost: 15,
};
const COOLING: Research = Research {
    id: "cooler",
    name: "Portable Beverages",
    cost: 24,
};
pub(crate) const JERKY: Research = Research {
    id: "meat_jerky",
    name: "Jerkys",
    cost: 2,
};

pub(crate) const RESEARCHES: &[Research] = &[
    PRESS,
    BETTER_PRESS,
    ULTIMATE_PRESS,
    FREEZING,
    BETTER_FREEZING,
    DRYING,
    COOLING,
    JERKY,
];

pub(crate) const MACHINES: &[Machine] = &[
    Machine {
        item: CARBON_PRESS,
        title: "Carbon Press",
        family: Family::CarbonPress,
        capacity: 256,
        cost: 10,
        speed: 1,
        research: &PRESS,
    },
    Machine {
        item: CARBON_PRESS_2,
        title: "Carbon Press - II",
        family: Family::CarbonPress,
        capacity: 512,
        cost: 25,
        speed: 3,
        research: &BETTER_PRESS,
    },
    Machine {
        item: CARBON_PRESS_3,
        title: "Carbon Press - III",
        family: Family::CarbonPress,
        capacity: 512,
        cost: 90,
        speed: 15,
        research: &ULTIMATE_PRESS,
    },
    Machine {
        item: FREEZER,
        title: "Freezer",
        family: Family::Freezer,
        capacity: 256,
        cost: 9,
        speed: 1,
        research: &FREEZING,
    },
    Machine {
        item: FREEZER_2,
        title: "Freezer (II)",
        family: Family::Freezer,
        capacity: 256,
        cost: 15,
        speed: 2,
        research: &BETTER_FREEZING,
    },
    Machine {
        item: FREEZER_3,
        title: "Freezer (III)",
        family: Family::Freezer,
        capacity: 256,
        cost: 21,
        speed: 3,
        research: &BETTER_FREEZING,
    },
    Machine {
        item: AUTO_DRIER,
        title: "Auto Drier",
        family: Family::AutoDrier,
        capacity: 128,
        cost: 5,
        speed: 1,
        research: &DRYING,
    },
];

const fn v(id: &'static str) -> Option<Kind> {
    Some(Kind::Vanilla(id))
}

macro_rules! craft {
    ($item:ident, $research:ident; $($input:expr),+ $(,)?) => {
        CraftingRecipe {
            inputs: [$($input),+],
            output: Stack { kind: $item, count: 1 },
            research: &$research,
        }
    };
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft!(CARBON_PRESS, PRESS;
        Some(CARBON), Some(ELECTRIC_MOTOR), Some(CARBON),
        Some(CARBON), Some(HEATED_PRESSURE_CHAMBER), Some(CARBON),
        Some(HEATING_COIL), Some(CARBONADO), Some(HEATING_COIL)
    ),
    craft!(CARBON_PRESS_2, BETTER_PRESS;
        Some(CARBONADO), Some(ELECTRIC_MOTOR), Some(CARBONADO),
        Some(CARBON), Some(CARBON_PRESS), Some(CARBON),
        Some(HEATING_COIL), Some(ELECTRO_MAGNET), Some(HEATING_COIL)
    ),
    craft!(CARBON_PRESS_3, ULTIMATE_PRESS;
        Some(CARBONADO), Some(ELECTRIC_MOTOR), Some(CARBONADO),
        Some(REINFORCED_ALLOY_INGOT), Some(CARBON_PRESS_2), Some(REINFORCED_ALLOY_INGOT),
        Some(HEATING_COIL), Some(ELECTRO_MAGNET), Some(HEATING_COIL)
    ),
    craft!(FREEZER, FREEZING;
        None, Some(SILVER_INGOT), None,
        Some(ELECTRIC_MOTOR), v("minecraft:packed_ice"), Some(ELECTRIC_MOTOR),
        Some(COOLING_UNIT), Some(MEDIUM_CAPACITOR), Some(COOLING_UNIT)
    ),
    craft!(FREEZER_2, BETTER_FREEZING;
        None, Some(SILVER_INGOT), None,
        Some(ELECTRO_MAGNET), Some(FREEZER), Some(ELECTRO_MAGNET),
        Some(COOLING_UNIT), Some(ALUMINUM_BRASS_INGOT), Some(COOLING_UNIT)
    ),
    craft!(FREEZER_3, BETTER_FREEZING;
        Some(REINFORCED_ALLOY_INGOT), Some(REINFORCED_ALLOY_INGOT), Some(REINFORCED_ALLOY_INGOT),
        Some(ELECTRO_MAGNET), Some(FREEZER_2), Some(ELECTRO_MAGNET),
        Some(COOLING_UNIT), Some(COOLING_UNIT), Some(COOLING_UNIT)
    ),
    craft!(AUTO_DRIER, DRYING;
        None, None, None,
        Some(HEATING_COIL), v("minecraft:smoker"), Some(HEATING_COIL),
        None, v("minecraft:campfire"), None
    ),
    craft!(COOLING_UNIT, COOLING;
        v("minecraft:ice"), v("minecraft:ice"), v("minecraft:ice"),
        Some(ALUMINUM_INGOT), Some(ELECTRIC_MOTOR), Some(ALUMINUM_INGOT),
        v("minecraft:ice"), v("minecraft:ice"), v("minecraft:ice")
    ),
    jerky(BEEF_JERKY, "minecraft:cooked_beef"),
    jerky(PORK_JERKY, "minecraft:cooked_porkchop"),
    jerky(CHICKEN_JERKY, "minecraft:cooked_chicken"),
    jerky(MUTTON_JERKY, "minecraft:cooked_mutton"),
    jerky(RABBIT_JERKY, "minecraft:cooked_rabbit"),
    jerky(FISH_JERKY, "minecraft:cooked_cod"),
];

const fn jerky(item: Kind, meat: &'static str) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            Some(crate::items::SALT),
            v(meat),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        output: Stack {
            kind: item,
            count: 1,
        },
        research: &JERKY,
    }
}

fn is_jerky(host: &dyn Host, stack: &ItemStack) -> bool {
    [
        BEEF_JERKY,
        PORK_JERKY,
        CHICKEN_JERKY,
        MUTTON_JERKY,
        RABBIT_JERKY,
        FISH_JERKY,
    ]
    .into_iter()
    .any(|jerky| jerky.matches(host, stack))
}

pub(crate) fn start_eating_jerky(
    host: &mut dyn ServerHost,
    event: &LivingEntityUseItemStartEvent<'_>,
) -> Flow {
    if event.entity.is_none()
        && is_jerky(host, event.stack)
        && let Some(player) = host.players().acting_player()
        && !crate::research::can_use(host, player, &JERKY)
    {
        let _ = player.tell(
            host,
            Text::literal("Unlock this research with /sf research meat_jerky."),
        );
        return Flow::Cancel;
    }
    Flow::Continue
}

pub(crate) fn finish_eating_jerky(
    host: &mut dyn ServerHost,
    event: &LivingEntityUseItemFinishEvent<'_>,
) {
    if event.entity.is_none()
        && is_jerky(host, event.stack)
        && let Some(player) = host.players().acting_player()
        && let Some(food) = player.food(host)
    {
        let _ = player.set_saturation(host, food.saturation + 6.0);
    }
}

pub(crate) fn item(id: &str) -> Option<Kind> {
    [
        COOLING_UNIT,
        REACTOR_COLLANT_CELL,
        BEEF_JERKY,
        PORK_JERKY,
        CHICKEN_JERKY,
        MUTTON_JERKY,
        RABBIT_JERKY,
        FISH_JERKY,
    ]
    .into_iter()
    .find(|item| item.id() == id)
}

pub(crate) struct Recipe {
    pub seconds: u32,
    pub inputs: &'static [Stack],
    pub outputs: &'static [Stack],
}

const fn s(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

macro_rules! stack {
    ($material:literal, $count:literal) => {
        s(Kind::Vanilla(concat!("minecraft:", $material)), $count)
    };
}

macro_rules! recipe {
    ($seconds:literal; [$($input:expr),+]; [$($output:expr),+]) => {
        Recipe { seconds: $seconds, inputs: &[$($input),+], outputs: &[$($output),+] }
    };
}

const CARBON_RECIPES: &[Recipe] = &[
    recipe!(15; [stack!("charcoal", 4)]; [stack!("coal", 1)]),
    recipe!(20; [stack!("coal", 8)]; [s(CARBON, 1)]),
    recipe!(180; [stack!("coal_block", 8)]; [s(CARBON, 9)]),
    recipe!(30; [s(CARBON, 4)]; [s(COMPRESSED_CARBON, 1)]),
    recipe!(60; [s(CARBON_CHUNK, 1), s(SYNTHETIC_DIAMOND, 1)]; [s(RAW_CARBONADO, 1)]),
    recipe!(60; [s(CARBON_CHUNK, 1)]; [s(SYNTHETIC_DIAMOND, 1)]),
    recipe!(90; [s(RAW_CARBONADO, 1)]; [s(CARBONADO, 1)]),
];

const CONDENSE_FAST: &[Recipe] = &[
    recipe!(4; [stack!("ice", 1)]; [stack!("packed_ice", 1)]),
    recipe!(6; [stack!("packed_ice", 1)]; [stack!("blue_ice", 1)]),
];
const CONDENSE_VANILLA: &[Recipe] = &[
    recipe!(4; [stack!("ice", 9)]; [stack!("packed_ice", 1)]),
    recipe!(6; [stack!("packed_ice", 9)]; [stack!("blue_ice", 1)]),
];
const FREEZER_RECIPES: &[Recipe] = &[
    recipe!(2; [stack!("water_bucket", 1)]; [stack!("bucket", 1), stack!("ice", 1)]),
    recipe!(8; [stack!("lava_bucket", 1)]; [stack!("bucket", 1), stack!("obsidian", 1)]),
    recipe!(8; [stack!("blue_ice", 1)]; [s(REACTOR_COLLANT_CELL, 1)]),
    recipe!(6; [stack!("snow_block", 2)]; [stack!("ice", 1)]),
    recipe!(6; [stack!("magma_cream", 1)]; [stack!("slime_ball", 1)]),
    recipe!(6; [stack!("magma_block", 2)]; [stack!("slime_block", 1)]),
];
const DRIER_RECIPES: &[Recipe] = &[
    recipe!(6; [stack!("rotten_flesh", 1)]; [stack!("leather", 1)]),
    recipe!(6; [stack!("wet_sponge", 1)]; [stack!("sponge", 1)]),
    recipe!(6; [stack!("kelp", 1)]; [stack!("dried_kelp", 1)]),
    recipe!(6; [stack!("potion", 1)]; [stack!("glass_bottle", 1)]),
    recipe!(6; [stack!("splash_potion", 1)]; [stack!("glass_bottle", 1)]),
    recipe!(6; [stack!("lingering_potion", 1)]; [stack!("glass_bottle", 1)]),
    recipe!(6; [stack!("water_bucket", 1)]; [stack!("bucket", 1)]),
    recipe!(6; [stack!("cooked_beef", 1)]; [s(BEEF_JERKY, 1)]),
    recipe!(6; [stack!("cooked_porkchop", 1)]; [s(PORK_JERKY, 1)]),
    recipe!(6; [stack!("cooked_chicken", 1)]; [s(CHICKEN_JERKY, 1)]),
    recipe!(6; [stack!("cooked_mutton", 1)]; [s(MUTTON_JERKY, 1)]),
    recipe!(6; [stack!("cooked_rabbit", 1)]; [s(RABBIT_JERKY, 1)]),
    recipe!(6; [stack!("cooked_cod", 1)]; [s(FISH_JERKY, 1)]),
    recipe!(6; [stack!("cooked_salmon", 1)]; [s(FISH_JERKY, 1)]),
    recipe!(6; [stack!("mud", 1)]; [stack!("clay", 1)]),
];

pub(crate) fn recipes(
    family: Family,
    vanilla_ratios: bool,
) -> impl Iterator<Item = &'static Recipe> {
    let (first, rest) = match family {
        Family::CarbonPress => (CARBON_RECIPES, &[][..]),
        Family::Freezer if vanilla_ratios => (CONDENSE_VANILLA, FREEZER_RECIPES),
        Family::Freezer => (CONDENSE_FAST, FREEZER_RECIPES),
        Family::AutoDrier => (DRIER_RECIPES, &[][..]),
        _ => (&[][..], &[][..]),
    };
    first.iter().chain(rest)
}

pub(crate) fn dry_plant(host: &dyn Host, inputs: &[Option<ItemStack>]) -> Option<Plan> {
    let tags = host.tags();
    for (name, output) in [("minecraft:saplings", 2), ("minecraft:leaves", 1)] {
        let Some(tag) = tags.key(&Identifier::parse(name).ok()?) else {
            continue;
        };
        if let Some((slot, input)) = inputs.iter().enumerate().find_map(|(slot, input)| {
            let input = input.as_ref()?;
            (input.components().is_none()
                && input.damage_component().is_none()
                && tags.is_in_tag(input.item(), tag) == Some(true))
            .then_some((slot, input))
        }) {
            return Some(Plan {
                consumed: vec![(slot, input.clone(), 1)],
                outputs: vec![Kind::Vanilla("minecraft:stick").stack(host, output)?],
                work_ticks: 12,
            });
        }
    }
    None
}
