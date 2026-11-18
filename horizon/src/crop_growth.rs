// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockState, BlockUpdateFlags, Host, HostBlockStates, ItemResource, LevelRef, ServerHost, Tag,
    Transaction,
};

use crate::{
    android_items::PROGRAMMABLE_ANDROID_FARMER,
    items::{ADVANCED_CIRCUIT_BOARD, ELECTRIC_MOTOR, ELECTRO_MAGNET, Kind, item, text},
    network_recipes::BLISTERING_INGOT_3,
    recipes::{CraftingRecipe, Research, Stack},
};

use super::{
    animal_machines::ANIMAL_GROWTH_ACCELERATOR,
    tree_growth::{MENU_SLOTS, horizontal_square, is_fertilizer, particles},
};

type Position = [i32; 3];

item!(
    CROP_GROWTH_ACCELERATOR,
    "lime_terracotta",
    "Crop Growth Accelerator",
    "green",
    None,
    None
);
item!(
    CROP_GROWTH_ACCELERATOR_2,
    "lime_terracotta",
    "Crop Growth Accelerator (II)",
    "green",
    None,
    None
);

pub(crate) const CAPACITY: u32 = 1024;
const GROWTH: Research = Research {
    id: "crop_growth_accelerator",
    name: "Crop Growth Acceleration",
    cost: 40,
};
const GROWTH_2: Research = Research {
    id: "better_crop_growth_accelerator",
    name: "Upgraded Crop Growth Accelerator",
    cost: 44,
};

pub(crate) const RESEARCHES: &[Research] = &[GROWTH, GROWTH_2];

#[derive(Clone, Copy)]
pub(crate) struct Machine {
    pub item: Kind,
    pub title: &'static str,
    pub cost: u32,
    pub research: &'static Research,
    radius: i32,
    display_speed: u8,
}

const MACHINES: &[Machine] = &[
    Machine {
        item: CROP_GROWTH_ACCELERATOR,
        title: "Crop Growth Accelerator",
        cost: 25,
        research: &GROWTH,
        radius: 3,
        display_speed: 3,
    },
    Machine {
        item: CROP_GROWTH_ACCELERATOR_2,
        title: "Crop Growth Accelerator (II)",
        cost: 30,
        research: &GROWTH_2,
        radius: 4,
        display_speed: 4,
    },
];

pub(crate) fn machine(id: &str) -> Option<&'static Machine> {
    MACHINES.iter().find(|machine| machine.item.id() == id)
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            None,
            Some(BLISTERING_INGOT_3),
            None,
            Some(ELECTRIC_MOTOR),
            Some(PROGRAMMABLE_ANDROID_FARMER),
            Some(ELECTRIC_MOTOR),
            Some(ELECTRO_MAGNET),
            Some(ANIMAL_GROWTH_ACCELERATOR),
            Some(ELECTRO_MAGNET),
        ],
        output: Stack {
            kind: CROP_GROWTH_ACCELERATOR,
            count: 1,
        },
        research: &GROWTH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(BLISTERING_INGOT_3),
            None,
            Some(ELECTRIC_MOTOR),
            Some(CROP_GROWTH_ACCELERATOR),
            Some(ELECTRIC_MOTOR),
            Some(ELECTRO_MAGNET),
            Some(ADVANCED_CIRCUIT_BOARD),
            Some(ELECTRO_MAGNET),
        ],
        output: Stack {
            kind: CROP_GROWTH_ACCELERATOR_2,
            count: 1,
        },
        research: &GROWTH_2,
    },
];

const CROPS: &[&str] = &[
    "minecraft:beetroots",
    "minecraft:carrots",
    "minecraft:potatoes",
    "minecraft:wheat",
    "minecraft:melon_stem",
    "minecraft:pumpkin_stem",
    "minecraft:nether_wart",
    "minecraft:cocoa",
    "minecraft:sweet_berry_bush",
];

fn is_crop(name: &str, vanilla_crops_tag: bool) -> bool {
    vanilla_crops_tag || CROPS.contains(&name)
}

fn next_age(age: &str) -> Option<String> {
    Some(age.parse::<u32>().ok()?.checked_add(1)?.to_string())
}

#[derive(Clone, Copy)]
pub(crate) struct Growth {
    pos: Position,
    expected: BlockState,
    replacement: BlockState,
}

impl Machine {
    pub(crate) fn stage(
        self,
        host: &mut dyn ServerHost,
        tx: &Transaction,
        level: LevelRef,
        origin: Position,
    ) -> Result<Growth, ()> {
        let (input, fertilizer) = (0..MENU_SLOTS.len())
            .find_map(|slot| {
                let place = super::place(level, origin, slot);
                let stack = place
                    .stack(host)
                    .filter(|stack| is_fertilizer(host, stack))?;
                Some((place, stack))
            })
            .ok_or(())?;

        // Paper displays a speed by tier, then advances one crop by one age in its tick.
        let growth = horizontal_square(origin, self.radius)
            .find_map(|pos| {
                let growth = growth_at(host, level, pos)?;
                level
                    .allow_inventory_transfer(host, origin, pos, &fertilizer)
                    .then_some(growth)
            })
            .ok_or(())?;

        (input.extract(host, tx, &ItemResource::of(&fertilizer), 1) == 1)
            .then_some(growth)
            .ok_or(())
    }
}

fn growth_at(host: &dyn Host, level: LevelRef, pos: Position) -> Option<Growth> {
    let expected = level.block_state(host, pos)?;
    let block = expected.block();
    if !is_crop(
        &block.id(host),
        host.block_in_tag(block, "minecraft:crops") == Some(true),
    ) {
        return None;
    }
    let metadata = host.block_states();
    let higher = next_age(expected.property(&metadata, "age").ok()??)?;
    let replacement = expected.with(&metadata, "age", &higher).ok()?;
    Some(Growth {
        pos,
        expected,
        replacement,
    })
}

impl Growth {
    pub(crate) fn apply(self, host: &mut dyn ServerHost, level: LevelRef) -> Result<(), ()> {
        if level.block_state(host, self.pos) != Some(self.expected) {
            return Err(());
        }
        // Paper's setBlockData uses normal neighbor physics and sends the new state.
        level
            .set_block(
                host,
                self.pos,
                self.replacement,
                BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
            )
            .map_err(|_| ())?
            .then_some(())
            .ok_or(())
    }

    pub(crate) fn particles(self, host: &mut dyn ServerHost, level: LevelRef) {
        particles(host, level, self.pos);
    }
}

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let machine = machine(id)?;
    let details = |line: Tag| {
        Tag::compound([
            ("text", Tag::String("".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![text("⇨ ", "dark_gray", false), line]),
            ),
        ])
    };
    let power = |line: String| {
        details(Tag::compound([
            ("text", Tag::String("⚡ ".into())),
            ("color", Tag::String("yellow".into())),
            ("extra", Tag::List(vec![text(&line, "gray", false)])),
        ]))
    };
    let width = machine.radius * 2 + 1;
    Some(Tag::List(vec![
        Tag::String("".into()),
        Tag::compound([
            ("text", Tag::String("Runs on ".into())),
            ("color", Tag::String("white".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![text("Organic Fertilizer", "green", false)]),
            ),
        ]),
        Tag::String("".into()),
        text("End-Game Machine", "dark_red", false),
        details(text(&format!("Radius: {width}x{width}"), "gray", false)),
        details(Tag::compound([
            ("text", Tag::String("Speed: ".into())),
            ("color", Tag::String("gray".into())),
            (
                "extra",
                Tag::List(vec![text(
                    &format!("{}/time", machine.display_speed),
                    "green",
                    false,
                )]),
            ),
        ])),
        power(format!("{CAPACITY} J Buffer")),
        power(format!("{} J/s", machine.cost * 2)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_ranges_scan_the_same_level_from_low_x_then_low_z() {
        for (machine, radius, cost, research, count) in [
            (&MACHINES[0], 3, 25, "crop_growth_accelerator", 49),
            (&MACHINES[1], 4, 30, "better_crop_growth_accelerator", 81),
        ] {
            assert_eq!(machine.cost, cost);
            assert_eq!(machine.research.id, research);
            let positions: Vec<_> = horizontal_square([12, 65, -6], machine.radius).collect();
            assert_eq!(positions.len(), count);
            assert_eq!(positions[0], [12 - radius, 65, -6 - radius]);
            assert_eq!(
                positions[(2 * radius + 1) as usize],
                [13 - radius, 65, -6 - radius]
            );
            assert_eq!(positions[count - 1], [12 + radius, 65, -6 + radius]);
        }
    }

    #[test]
    fn crop_tiers_are_placeable_food_machines_with_the_original_prerequisites() {
        use crate::{
            android_items::{PROGRAMMABLE_ANDROID, PROGRAMMABLE_ANDROID_FARMER},
            factory::ElectricMachine,
            heated_pressure::{PLASTIC_RESEARCH, PLASTIC_SHEET},
            networks::Component,
            recipes,
        };

        let recipe = |kind: Kind| {
            recipes::crafting()
                .find(|recipe| recipe.output.kind == kind)
                .unwrap()
                .inputs
                .map(|input| input.map(Kind::id))
        };
        assert_eq!(
            recipe(PROGRAMMABLE_ANDROID),
            [
                Some("PLASTIC_SHEET"),
                Some("ANDROID_MEMORY_CORE"),
                Some("PLASTIC_SHEET"),
                Some("COAL_GENERATOR"),
                Some("ELECTRIC_MOTOR"),
                Some("minecraft:chest"),
                Some("PLASTIC_SHEET"),
                Some("PLASTIC_SHEET"),
                Some("PLASTIC_SHEET"),
            ]
        );
        assert_eq!(
            recipe(PROGRAMMABLE_ANDROID_FARMER),
            [
                None,
                None,
                None,
                Some("minecraft:diamond_hoe"),
                Some("PROGRAMMABLE_ANDROID"),
                Some("minecraft:diamond_hoe"),
                None,
                Some("ELECTRIC_MOTOR"),
                None,
            ]
        );
        assert_eq!(
            recipe(CROP_GROWTH_ACCELERATOR),
            [
                None,
                Some("BLISTERING_INGOT_3"),
                None,
                Some("ELECTRIC_MOTOR"),
                Some("PROGRAMMABLE_ANDROID_FARMER"),
                Some("ELECTRIC_MOTOR"),
                Some("ELECTRO_MAGNET"),
                Some("ANIMAL_GROWTH_ACCELERATOR"),
                Some("ELECTRO_MAGNET"),
            ]
        );
        assert_eq!(
            recipe(CROP_GROWTH_ACCELERATOR_2),
            [
                None,
                Some("BLISTERING_INGOT_3"),
                None,
                Some("ELECTRIC_MOTOR"),
                Some("CROP_GROWTH_ACCELERATOR"),
                Some("ELECTRIC_MOTOR"),
                Some("ELECTRO_MAGNET"),
                Some("ADVANCED_CIRCUIT_BOARD"),
                Some("ELECTRO_MAGNET"),
            ]
        );
        assert_eq!(
            recipes::output_research(PLASTIC_SHEET).unwrap().id,
            PLASTIC_RESEARCH.id
        );
        for (id, cost) in [
            (CROP_GROWTH_ACCELERATOR.id(), 40),
            (CROP_GROWTH_ACCELERATOR_2.id(), 44),
        ] {
            let factory = ElectricMachine::for_item(id).unwrap();
            assert!(matches!(
                Component::for_item(id),
                Some(Component::ElectricMachine {
                    capacity: 1024,
                    carrier: "minecraft:lime_terracotta"
                })
            ));
            assert_eq!(factory.research.cost, cost);
            assert_eq!(factory.menu_slots(), &[10, 11, 12, 13, 14, 15, 16]);
            assert!((0..7).all(|slot| factory.input_slot(slot)));
        }
    }

    #[test]
    fn paper_crops_include_explicit_ageable_blocks_and_the_optional_host_tag() {
        assert_eq!(CROPS.len(), 9);
        for crop in [
            "minecraft:nether_wart",
            "minecraft:cocoa",
            "minecraft:sweet_berry_bush",
        ] {
            assert!(is_crop(crop, false));
        }
        assert!(is_crop("minecraft:torchflower_crop", true));
        assert!(!is_crop("minecraft:oak_sapling", false));
        assert!(!is_crop("minecraft:ageable_example", false));
        assert_eq!(next_age("2").as_deref(), Some("3"));
        assert_eq!(next_age("6").as_deref(), Some("7"));
        assert_eq!(next_age("broken"), None);
        assert_eq!(next_age("4294967295"), None);
    }
}
