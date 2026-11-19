// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, Host, ItemPlace, PlayerRef, ServerHost, Tag};

use crate::{
    backpacks::CLOTH,
    items::{
        BILLON_INGOT, CARBON_CHUNK, CARBONADO, DAMASCUS_STEEL_INGOT, DURALUMIN_INGOT, Kind,
        REINFORCED_ALLOY_INGOT, SMALL_CAPACITOR, SOLDER_INGOT, STEEL_INGOT, STEEL_PLATE, item,
        item_id, text,
    },
    jet_boots::{CARBONADO_TOOLS, STEEL_THRUSTER},
    network_recipes::{LARGE_CAPACITOR, MEDIUM_CAPACITOR, POWER_CRYSTAL},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    sneak_equipment::{Drive, charged_thrust, research_for},
};

item!(
    DURALUMIN_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - I",
    "blue",
    None,
    None
);
item!(
    SOLDER_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - II",
    "blue",
    None,
    None
);
item!(
    BILLON_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - III",
    "blue",
    None,
    None
);
item!(
    STEEL_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - IV",
    "blue",
    None,
    None
);
item!(
    DAMASCUS_STEEL_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - V",
    "blue",
    None,
    None
);
item!(
    REINFORCED_ALLOY_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - VI",
    "blue",
    None,
    None
);
item!(
    CARBONADO_JETPACK,
    "leather_chestplate",
    "Electric Jetpack - VII",
    "blue",
    None,
    None
);
item!(
    ARMORED_JETPACK,
    "iron_chestplate",
    "Armored Jetpack",
    "blue",
    None,
    None
);
item!(
    PARACHUTE,
    "leather_chestplate",
    "Parachute",
    "white",
    None,
    None
);
item!(CHAIN, "string", "Chain", "aqua", None, None);

const JETPACKS: Research = Research {
    id: "jetpacks",
    name: "Jetpacks",
    cost: 22,
};
const ARMORED: Research = Research {
    id: "armored_jetpack",
    name: "Armored Jetpack",
    cost: 27,
};
const PARACHUTES: Research = Research {
    id: "parachute",
    name: "Parachute",
    cost: 15,
};
const GRAPPLING_HOOK: Research = Research {
    id: "grappling_hook",
    name: "Grappling Hook",
    cost: 15,
};

pub(crate) const RESEARCHES: &[&Research] = &[&JETPACKS, &ARMORED, &PARACHUTES, &GRAPPLING_HOOK];

pub(crate) struct Spec {
    item: Kind,
    tier: Option<&'static str>,
    material: &'static str,
    shoulders: Kind,
    torso: Kind,
    capacitor: Kind,
    thrust: f64,
    capacity: f32,
    dye: Option<i32>,
    research: &'static Research,
}

// SlimefunItemSetup registers tiers I–VII and an armored model. Carbonado uses Carbon Chunks
// across the shoulders and finished Carbonado only in the middle row.
macro_rules! silver {
    ($item:ident, $tier:literal, $material:literal, $ingot:ident, $capacitor:ident, $thrust:literal, $capacity:literal) => {
        Spec {
            item: $item,
            tier: Some($tier),
            material: $material,
            shoulders: $ingot,
            torso: $ingot,
            capacitor: $capacitor,
            thrust: $thrust,
            capacity: $capacity,
            dye: Some(0xc0c0c0),
            research: &JETPACKS,
        }
    };
}

const SPECS: [Spec; 8] = [
    silver!(
        DURALUMIN_JETPACK,
        "I",
        "Duralumin",
        DURALUMIN_INGOT,
        SMALL_CAPACITOR,
        0.35,
        20.0
    ),
    silver!(
        SOLDER_JETPACK,
        "II",
        "Solder",
        SOLDER_INGOT,
        SMALL_CAPACITOR,
        0.4,
        30.0
    ),
    silver!(
        BILLON_JETPACK,
        "III",
        "Billon",
        BILLON_INGOT,
        SMALL_CAPACITOR,
        0.45,
        45.0
    ),
    silver!(
        STEEL_JETPACK,
        "IV",
        "Steel",
        STEEL_INGOT,
        SMALL_CAPACITOR,
        0.5,
        60.0
    ),
    silver!(
        DAMASCUS_STEEL_JETPACK,
        "V",
        "Damascus Steel",
        DAMASCUS_STEEL_INGOT,
        SMALL_CAPACITOR,
        0.55,
        75.0
    ),
    silver!(
        REINFORCED_ALLOY_JETPACK,
        "VI",
        "Reinforced Alloy",
        REINFORCED_ALLOY_INGOT,
        MEDIUM_CAPACITOR,
        0.6,
        100.0
    ),
    Spec {
        item: CARBONADO_JETPACK,
        tier: Some("VII"),
        material: "Carbonado",
        shoulders: CARBON_CHUNK,
        torso: CARBONADO,
        capacitor: LARGE_CAPACITOR,
        thrust: 0.7,
        capacity: 150.0,
        dye: Some(0x000000),
        research: &CARBONADO_TOOLS,
    },
    Spec {
        item: ARMORED_JETPACK,
        tier: None,
        material: "Steel",
        shoulders: STEEL_PLATE,
        torso: STEEL_PLATE,
        capacitor: MEDIUM_CAPACITOR,
        thrust: 0.5,
        capacity: 50.0,
        dye: None,
        research: &ARMORED,
    },
];

const fn recipe(spec: &Spec) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            Some(spec.shoulders),
            None,
            Some(spec.shoulders),
            Some(spec.torso),
            Some(POWER_CRYSTAL),
            Some(spec.torso),
            Some(STEEL_THRUSTER),
            Some(spec.capacitor),
            Some(STEEL_THRUSTER),
        ],
        output: Stack {
            kind: spec.item,
            count: 1,
        },
        research: spec.research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    recipe(&SPECS[0]),
    recipe(&SPECS[1]),
    recipe(&SPECS[2]),
    recipe(&SPECS[3]),
    recipe(&SPECS[4]),
    recipe(&SPECS[5]),
    recipe(&SPECS[6]),
    recipe(&SPECS[7]),
    CraftingRecipe {
        inputs: [
            Some(CLOTH),
            Some(CLOTH),
            Some(CLOTH),
            Some(CHAIN),
            None,
            Some(CHAIN),
            None,
            None,
            None,
        ],
        output: Stack {
            kind: PARACHUTE,
            count: 1,
        },
        research: &PARACHUTES,
    },
    CraftingRecipe {
        inputs: [
            None,
            None,
            Some(STEEL_INGOT),
            None,
            Some(STEEL_INGOT),
            None,
            Some(STEEL_INGOT),
            None,
            None,
        ],
        output: Stack {
            kind: CHAIN,
            count: 8,
        },
        research: &GRAPPLING_HOOK,
    },
];

fn spec(id: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|spec| spec.item.id() == id)
}

pub(crate) fn charge_spec(id: &str) -> Option<(&'static str, f32)> {
    spec(id).map(|spec| (spec.item.material(), spec.capacity))
}

pub(crate) fn decorate(id: &str, data: &mut Compound) {
    let blank = || Tag::String("".into());
    let crouch = || {
        line([
            text("Crouch", "yellow", false),
            text(" to use", "gray", false),
        ])
    };
    if id == PARACHUTE.id() {
        let mut name = text("Parachute", "white", false);
        if let Tag::Compound(style) = &mut name {
            style.insert("bold".into(), Tag::Byte(1));
        }
        data.insert("minecraft:custom_name".into(), name);
        data.insert("minecraft:dyed_color".into(), Tag::Int(0xffffff));
        data.insert("minecraft:lore".into(), Tag::List(vec![blank(), crouch()]));
        return;
    }
    let Some(spec) = spec(id) else {
        return;
    };
    let arrow = || text("⇨ ", "dark_gray", false);
    let material = line([
        arrow(),
        text("Material: ", "gray", false),
        text(spec.material, "aqua", false),
    ]);
    let [first, second] = if let Some(tier) = spec.tier {
        data.insert(
            "minecraft:custom_name".into(),
            line([
                text("Electric Jetpack", "blue", false),
                text(" - ", "gray", false),
                text(tier, "yellow", false),
            ]),
        );
        [blank(), material]
    } else {
        [material, blank()]
    };
    if let Some(color) = spec.dye {
        data.insert("minecraft:dyed_color".into(), Tag::Int(color));
    }
    let lore = vec![
        first,
        second,
        line([
            arrow(),
            text("⚡ ", "yellow", false),
            text(&format!("0 / {} J", spec.capacity as u32), "gray", false),
        ]),
        line([
            arrow(),
            text("Thrust: ", "gray", false),
            text(&spec.thrust.to_string(), "red", false),
        ]),
        blank(),
        crouch(),
    ];
    data.insert("minecraft:lore".into(), Tag::List(lore));
}

fn line<const N: usize>(parts: [Tag; N]) -> Tag {
    Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(parts.into())),
    ])
}

const PERIOD: u32 = 3;
const COST: f32 = 0.08;

fn thrust(strength: f64, [x, y, z]: [f64; 3]) -> [f64; 3] {
    let steering = f64::from(0.2_f32);
    [x * steering, strength + y * steering, z * steering]
}

fn chest(player: PlayerRef) -> ItemPlace {
    ItemPlace::PlayerSlot {
        player: player.uuid(),
        slot: 102,
    }
}

fn pack(host: &dyn Host, stack: &mod_api::ItemStack) -> Option<&'static Spec> {
    let spec = spec(&item_id(stack)?)?;
    spec.item.matches(host, stack).then_some(spec)
}

#[derive(Clone, Copy)]
pub(crate) enum Flight {
    Jetpack(&'static Spec),
    Parachute,
}

impl Drive for Flight {
    const PERIOD: u32 = PERIOD;

    fn begin(host: &mut dyn ServerHost, player: PlayerRef) -> Option<Self> {
        let stack = chest(player).stack(host)?;
        if let Some(spec) = pack(host, &stack) {
            return research_for(host, player, spec.research).then_some(Self::Jetpack(spec));
        }
        if PARACHUTE.matches(host, &stack) && research_for(host, player, &PARACHUTES) {
            return Some(Self::Parachute);
        }
        None
    }

    fn pulse(self, host: &mut dyn ServerHost, player: PlayerRef, _: &Rolls) -> bool {
        let entity = player.entity();
        match self {
            Flight::Parachute => {
                // Paper checks the parachute when beginning the crouch only.
                if !entity.set_velocity(host, [0.0, -0.1, 0.0]) {
                    return false;
                }
                let _ = entity.reset_fall_distance(host);
                true
            }
            Flight::Jetpack(spec) => {
                let chest = chest(player);
                let Some(stack) = chest.stack(host) else {
                    return true;
                };
                if pack(host, &stack).is_none() {
                    return false;
                }
                charged_thrust(
                    host,
                    entity,
                    (chest, &stack, COST),
                    |host, _| Some(thrust(spec.thrust, entity.view_vector(host)?)),
                    "minecraft:entity.generic.explode",
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn jetpack_uses_the_paper_recipes_capacities_and_research() {
        let paper = [
            (
                "DURALUMIN_INGOT",
                "DURALUMIN_INGOT",
                "SMALL_CAPACITOR",
                20.0,
                0.35,
                "jetpacks",
            ),
            (
                "SOLDER_INGOT",
                "SOLDER_INGOT",
                "SMALL_CAPACITOR",
                30.0,
                0.4,
                "jetpacks",
            ),
            (
                "BILLON_INGOT",
                "BILLON_INGOT",
                "SMALL_CAPACITOR",
                45.0,
                0.45,
                "jetpacks",
            ),
            (
                "STEEL_INGOT",
                "STEEL_INGOT",
                "SMALL_CAPACITOR",
                60.0,
                0.5,
                "jetpacks",
            ),
            (
                "DAMASCUS_STEEL_INGOT",
                "DAMASCUS_STEEL_INGOT",
                "SMALL_CAPACITOR",
                75.0,
                0.55,
                "jetpacks",
            ),
            (
                "REINFORCED_ALLOY_INGOT",
                "REINFORCED_ALLOY_INGOT",
                "MEDIUM_CAPACITOR",
                100.0,
                0.6,
                "jetpacks",
            ),
            (
                "CARBON_CHUNK",
                "CARBONADO",
                "LARGE_CAPACITOR",
                150.0,
                0.7,
                "carbonado_tools",
            ),
            (
                "STEEL_PLATE",
                "STEEL_PLATE",
                "MEDIUM_CAPACITOR",
                50.0,
                0.5,
                "armored_jetpack",
            ),
        ];
        for ((spec, recipe), (shoulders, torso, capacitor, capacity, strength, research)) in
            SPECS.iter().zip(CRAFTING).zip(paper)
        {
            assert_eq!(spec.item, recipe.output.kind);
            assert_eq!(recipe.research.id, research);
            assert_eq!(spec.thrust, strength);
            assert_eq!(
                charge_spec(spec.item.id()),
                Some((spec.item.material(), capacity))
            );
            assert_eq!(
                recipe.inputs.map(|input| input.map_or("", Kind::id)),
                [
                    shoulders,
                    "",
                    shoulders,
                    torso,
                    "POWER_CRYSTAL",
                    torso,
                    "STEEL_THRUSTER",
                    capacitor,
                    "STEEL_THRUSTER"
                ],
            );
        }
        assert_eq!(CRAFTING[8].output.kind, PARACHUTE);
        assert_eq!(
            CRAFTING[8].inputs.map(|input| input.map_or("", Kind::id)),
            ["CLOTH", "CLOTH", "CLOTH", "CHAIN", "", "CHAIN", "", "", ""],
        );
        assert_eq!(CRAFTING[9].output.kind, CHAIN);
        assert_eq!(CRAFTING[9].output.count, 8);
        assert_eq!(
            CRAFTING[9].inputs.map(|input| input.map_or("", Kind::id)),
            [
                "",
                "",
                "STEEL_INGOT",
                "",
                "STEEL_INGOT",
                "",
                "STEEL_INGOT",
                "",
                ""
            ],
        );
    }

    #[test]
    fn the_jetpack_combines_eye_steering_with_vertical_thrust_and_drains_float_charge() {
        assert_eq!(PERIOD, 3);
        assert_eq!(
            thrust(0.35, [1.0, 0.0, 0.0]),
            [f64::from(0.2_f32), 0.35, 0.0]
        );
        assert_eq!(
            thrust(0.7, [0.0, -1.0, 0.0]),
            [0.0, 0.7 - f64::from(0.2_f32), 0.0]
        );

        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:leather_chestplate", "minecraft:iron_chestplate"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        for kind in [BILLON_JETPACK, ARMORED_JETPACK] {
            let bare = kind.stack(&host, 1).unwrap();
            assert!(pack(&host, &bare).is_some());
            let filled = crate::factory::charging_bench::recharge(&host, &bare, 1.0).unwrap();
            let spent = crate::factory::charging_bench::discharge(&host, &filled, COST).unwrap();
            let components = spent.components().unwrap();
            let sc = components
                .as_nbt()
                .get(&"minecraft:custom_data".into())
                .unwrap();
            assert_eq!(
                sc.get("PublicBukkitValues")
                    .unwrap()
                    .get("slimefun:item_charge"),
                Some(&Tag::Float(0.92_f32.to_bits())),
            );
        }
    }
}
