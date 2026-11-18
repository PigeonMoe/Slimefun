// SPDX-License-Identifier: GPL-3.0-only

use mod_api::Tag;

use crate::{
    fuel_generators::COAL_GENERATOR,
    heated_pressure::PLASTIC_SHEET,
    items::{BASIC_CIRCUIT_BOARD, ELECTRIC_MOTOR, HeadTexture, Kind, STEEL_INGOT, item, text},
    network_recipes::ANDROID_MEMORY_CORE,
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

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let function = match id {
        "PROGRAMMABLE_ANDROID" => "None",
        "PROGRAMMABLE_ANDROID_FARMER" => "Farming",
        _ => return None,
    };
    Some(Tag::List(vec![
        Tag::String("".into()),
        text(&format!("⇨ Function: {function}"), "gray", false),
        text("⇨ Fuel Efficiency: 1.0x", "gray", false),
    ]))
}
