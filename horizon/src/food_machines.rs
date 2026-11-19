// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockDropsEvent, BlockEntityRef, Compound,
    Holder, Host, HostSide, InteractionResult, ItemPart, ItemStack, LevelHost,
    OnBlockPlacedByEvent, ServerHost, Tag, Text, UseOnEvent,
};

use crate::{
    advanced_processing::{Family, Machine, Plan, plan},
    items::{
        BILLON_INGOT, DAMASCUS_STEEL_INGOT, ELECTRIC_MOTOR, ELECTRO_MAGNET, HARDENED_METAL_INGOT,
        HeadTexture, Kind, REINFORCED_ALLOY_INGOT, SALT, SILVER_INGOT, SMALL_CAPACITOR, TIN_INGOT,
        item,
    },
    medical::HEAVY_CREAM,
    network_recipes::MEDIUM_CAPACITOR,
    recipes::{CraftingRecipe, INGREDIENTS_AND_CHEESE, Research, Stack},
    research,
};

const EMPTY_CAN_SKIN: HeadTexture = HeadTexture {
    profile: [-1830501148, -700106553, -1775432466, -858511153],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzk0ZGE5N2YwODBlMzk1Yjg0MmM0Y2M4MmE4NDA4MjNkNGRiZDhjYTY4OGEyMDY4NTNlNTc4M2U0YmZkYzAxMiJ9fX0=",
};
const FILLED_CAN_SKIN: HeadTexture = HeadTexture {
    profile: [-964490542, -627163557, -1928828027, 1840099870],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2I0MzllM2Y1YWNiZWU5YmU0YzQyNTkyODlkNmQ5ZjM1YzYzNWZmYTY2MTExNDY4N2IzZWE2ZGRhOGM3OSJ9fX0=",
};

item!(
    CAN,
    "player_head",
    "Tin Can",
    "white",
    None,
    Some(EMPTY_CAN_SKIN)
);
item!(
    CHEESE,
    "player_head",
    "Cheese",
    "white",
    None,
    Some(HeadTexture {
        profile: [1572421042, 286602775, -1346730702, -80625394],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvMzRmZWJiYzE1ZDFkNGNjNjJiZWRjNWQ3YTJiNmYwZjQ2Y2Q1YjA2OTZhODg0ZGU3NWUyODllMzVjYmI1M2EwIn19fQ==",
    })
);
item!(
    BUTTER,
    "player_head",
    "Butter",
    "white",
    None,
    Some(HeadTexture {
        profile: [-1901572948, -172215052, -1347621804, 527753420],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYjY2YjE5ZjdkNjM1ZDAzNDczODkxZGYzMzAxN2M1NDkzNjMyMDlhOGY2MzI4YTg1NDJjMjEzZDA4NTI1ZSJ9fX0=",
    })
);
item!(
    FOOD_FABRICATOR,
    "green_stained_glass",
    "Food Fabricator",
    "red",
    None,
    None
);
item!(
    FOOD_FABRICATOR_2,
    "green_stained_glass",
    "Food Fabricator (II)",
    "red",
    None,
    None
);
item!(
    FOOD_COMPOSTER,
    "green_terracotta",
    "Food Composter",
    "red",
    None,
    None
);
item!(
    FOOD_COMPOSTER_2,
    "green_terracotta",
    "Food Composter (II)",
    "red",
    None,
    None
);

#[derive(Clone, Copy)]
pub(crate) struct Produce {
    pub crop: Kind,
    pub food: Kind,
    pub fertilizer: Kind,
    content: &'static str,
}

macro_rules! produce {
    ($item:literal, $food:ident, $fertilizer:ident, $label:literal) => {{
        item!(
            $food,
            "player_head",
            "Organic Food",
            "green",
            None,
            Some(FILLED_CAN_SKIN)
        );
        item!(
            $fertilizer,
            "player_head",
            "Organic Fertilizer",
            "green",
            None,
            Some(FILLED_CAN_SKIN)
        );
        Produce {
            crop: Kind::Vanilla(concat!("minecraft:", $item)),
            food: $food,
            fertilizer: $fertilizer,
            content: $label,
        }
    }};
}

pub(crate) const WHEAT: Produce = produce!("wheat", ORGANIC_FOOD_WHEAT, FERTILIZER_WHEAT, "Wheat");

// The Fabricator and Composter register the same crops in the same order.
pub(crate) const PRODUCE: &[Produce] = &[
    WHEAT,
    produce!("carrot", ORGANIC_FOOD_CARROT, FERTILIZER_CARROT, "Carrots"),
    produce!("potato", ORGANIC_FOOD_POTATO, FERTILIZER_POTATO, "Potatoes"),
    produce!("wheat_seeds", ORGANIC_FOOD_SEEDS, FERTILIZER_SEEDS, "Seeds"),
    produce!(
        "beetroot",
        ORGANIC_FOOD_BEETROOT,
        FERTILIZER_BEETROOT,
        "Beetroot"
    ),
    produce!("melon_slice", ORGANIC_FOOD_MELON, FERTILIZER_MELON, "Melon"),
    produce!("apple", ORGANIC_FOOD_APPLE, FERTILIZER_APPLE, "Apple"),
    produce!(
        "dried_kelp",
        ORGANIC_FOOD_KELP,
        FERTILIZER_KELP,
        "Dried Kelp"
    ),
    produce!(
        "cocoa_beans",
        ORGANIC_FOOD_COCOA,
        FERTILIZER_COCOA,
        "Cocoa Beans"
    ),
    produce!(
        "sweet_berries",
        ORGANIC_FOOD_SWEET_BERRIES,
        FERTILIZER_SWEET_BERRIES,
        "Sweet Berries"
    ),
    produce!(
        "seagrass",
        ORGANIC_FOOD_SEAGRASS,
        FERTILIZER_SEAGRASS,
        "Seagrass"
    ),
];

pub(crate) fn lore(id: &str) -> Option<Tag> {
    if !id.starts_with("ORGANIC_FOOD_") && !id.starts_with("FERTILIZER_") {
        return None;
    }
    let variant = PRODUCE
        .iter()
        .find(|row| row.food.id() == id || row.fertilizer.id() == id)?;
    let content = if id == "FERTILIZER_COCOA" {
        "Cocoa beans"
    } else {
        variant.content
    };
    Some(Tag::List(vec![Tag::compound([
        ("text", Tag::String("Content: ".into())),
        ("color", Tag::String("gray".into())),
        ("italic", Tag::Byte(0)),
        (
            "extra",
            Tag::List(vec![Tag::compound([
                ("text", Tag::String(content.into())),
                ("color", Tag::String("blue".into())),
            ])]),
        ),
    ])]))
}

fn product(id: &str) -> Option<(Kind, &'static Research)> {
    if id.starts_with("ORGANIC_FOOD_") {
        PRODUCE
            .iter()
            .find(|produce| produce.food.id() == id)
            .map(|produce| (produce.food, &ORGANIC_FOOD))
    } else if id.starts_with("FERTILIZER_") {
        PRODUCE
            .iter()
            .find(|produce| produce.fertilizer.id() == id)
            .map(|produce| (produce.fertilizer, &ORGANIC_FERTILIZER))
    } else {
        None
    }
}

static PLACED_PRODUCE: AttachmentType<String> = AttachmentType {
    namespace: "slimefun",
    name: "placed_produce",
    default: String::new,
    codec: Some(AttachmentCodec {
        encode: |id| {
            Ok(Compound::from([(
                "id".into(),
                Tag::String(id.as_str().into()),
            )]))
        },
        decode: |data| {
            saved_product(data)
                .map(|kind| kind.id().to_owned())
                .ok_or(AttachmentError::Decode)
        },
    }),
    copy_on_death: false,
    sync: false,
};

fn saved_product(data: &Compound) -> Option<Kind> {
    let Tag::String(id) = data.get(&"id".into())? else {
        return None;
    };
    product(&id.to_string().ok()?).map(|(kind, _)| kind)
}

pub(crate) struct Placement;

#[mod_api::horizon_part]
impl ItemPart for Placement {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let id = crate::items::item_id(event.stack)?;
        if id == CAN.id() {
            return Some(InteractionResult::Fail);
        }
        let (kind, unlock) = product(&id)?;
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        if kind.matches(host, event.stack) && !research::can_use(host, player, unlock) {
            let message = format!("Unlock {} with /sf research {}.", unlock.name, unlock.id);
            let _ = player.tell(host, Text::literal(message));
            return Some(InteractionResult::Fail);
        }
        None
    }

    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if let Some(player) = event.placer
            && let Some(id) = crate::items::item_id(event.stack)
            && let Some((kind, unlock)) = product(&id)
            && kind.matches(host, event.stack)
            && research::can_use(host, player, unlock)
        {
            let _ = PLACED_PRODUCE.set(host, Holder::BlockEntity(event.pos), id);
        }
    }
}

fn placed(level: &dyn LevelHost, entity: BlockEntityRef) -> Option<Kind> {
    level
        .block_entity_saved_attachment(entity, PLACED_PRODUCE.namespace, PLACED_PRODUCE.name)
        .as_ref()
        .and_then(saved_product)
}

pub(crate) fn marked(level: &dyn LevelHost, entity: BlockEntityRef) -> bool {
    placed(level, entity).is_some()
}

pub(crate) fn restore_drop(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) {
    if let Some(kind) = event.block_entity.and_then(|entity| placed(level, entity))
        && let Some(key) = level.item_key_by_name(kind.material())
        && let Some(drop) = event.drops.iter_mut().find(|drop| drop.stack.item() == key)
    {
        kind.decorate(&mut drop.stack);
    }
}

pub(crate) const FIRST_AID: Research = Research {
    id: "first_aid",
    name: "First Aid",
    cost: 2,
};
const ORGANIC_FOOD: Research = Research {
    id: "organic_food",
    name: "Organic Food",
    cost: 25,
};
const ORGANIC_FERTILIZER: Research = Research {
    id: "organic_fertilizer",
    name: "Organic Fertilizer",
    cost: 36,
};
const BETTER_FABRICATION: Research = Research {
    id: "better_food_fabricator",
    name: "Upgraded Food Fabrication",
    cost: 28,
};

pub(crate) const RESEARCHES: &[Research] = &[
    FIRST_AID,
    ORGANIC_FOOD,
    ORGANIC_FERTILIZER,
    BETTER_FABRICATION,
];

pub(crate) const MACHINES: &[Machine] = &[
    Machine {
        item: FOOD_FABRICATOR,
        title: "Food Fabricator",
        family: Family::FoodFabricator,
        capacity: 256,
        cost: 7,
        speed: 1,
        research: &ORGANIC_FOOD,
    },
    Machine {
        item: FOOD_FABRICATOR_2,
        title: "Food Fabricator (II)",
        family: Family::FoodFabricator,
        capacity: 512,
        cost: 24,
        speed: 6,
        research: &BETTER_FABRICATION,
    },
    Machine {
        item: FOOD_COMPOSTER,
        title: "Food Composter",
        family: Family::FoodComposter,
        capacity: 256,
        cost: 8,
        speed: 1,
        research: &ORGANIC_FERTILIZER,
    },
    Machine {
        item: FOOD_COMPOSTER_2,
        title: "Food Composter (II)",
        family: Family::FoodComposter,
        capacity: 512,
        cost: 26,
        speed: 10,
        research: &BETTER_FABRICATION,
    },
];

const fn stack(kind: Kind, count: u32) -> Stack {
    Stack { kind, count }
}

const fn craft(
    output: Stack,
    research: &'static Research,
    inputs: [Option<Kind>; 9],
) -> CraftingRecipe {
    CraftingRecipe {
        output,
        research,
        inputs,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        stack(CAN, 8),
        &FIRST_AID,
        [
            Some(TIN_INGOT),
            Some(TIN_INGOT),
            Some(TIN_INGOT),
            Some(TIN_INGOT),
            None,
            Some(TIN_INGOT),
            Some(TIN_INGOT),
            Some(TIN_INGOT),
            Some(TIN_INGOT),
        ],
    ),
    craft(
        stack(FOOD_FABRICATOR, 1),
        &ORGANIC_FOOD,
        [
            Some(BILLON_INGOT),
            Some(SILVER_INGOT),
            Some(BILLON_INGOT),
            Some(CAN),
            Some(SMALL_CAPACITOR),
            Some(CAN),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
    ),
    craft(
        stack(FOOD_COMPOSTER, 1),
        &ORGANIC_FERTILIZER,
        [
            Some(DAMASCUS_STEEL_INGOT),
            Some(FOOD_FABRICATOR),
            Some(DAMASCUS_STEEL_INGOT),
            Some(CAN),
            Some(MEDIUM_CAPACITOR),
            Some(CAN),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
    ),
    craft(
        stack(FOOD_FABRICATOR_2, 1),
        &BETTER_FABRICATION,
        [
            Some(HARDENED_METAL_INGOT),
            Some(REINFORCED_ALLOY_INGOT),
            Some(HARDENED_METAL_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(FOOD_FABRICATOR),
            Some(ELECTRIC_MOTOR),
            None,
            Some(ELECTRO_MAGNET),
            None,
        ],
    ),
    craft(
        stack(FOOD_COMPOSTER_2, 1),
        &BETTER_FABRICATION,
        [
            Some(HARDENED_METAL_INGOT),
            Some(REINFORCED_ALLOY_INGOT),
            Some(HARDENED_METAL_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(FOOD_COMPOSTER),
            Some(ELECTRIC_MOTOR),
            None,
            Some(ELECTRO_MAGNET),
            None,
        ],
    ),
    craft(
        stack(CHEESE, 1),
        &INGREDIENTS_AND_CHEESE,
        [
            Some(Kind::Vanilla("minecraft:milk_bucket")),
            Some(SALT),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
    ),
    craft(
        stack(BUTTER, 1),
        &INGREDIENTS_AND_CHEESE,
        [Some(HEAVY_CREAM), Some(SALT), None, None, None, None, None, None, None],
    ),
];

pub(crate) fn next(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
) -> Option<Plan> {
    match machine.family {
        Family::FoodFabricator => PRODUCE.iter().find_map(|produce| {
            plan(
                host,
                inputs,
                &[stack(CAN, 1), stack(produce.crop, 1)],
                &[stack(produce.food, 2)],
                24 / machine.speed,
            )
        }),
        Family::FoodComposter => PRODUCE.iter().find_map(|produce| {
            plan(
                host,
                inputs,
                &[stack(produce.food, 1)],
                &[stack(produce.fertilizer, 2)],
                60 / machine.speed,
            )
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use mod_api::{Identifier, testing::FakeHost};

    use super::*;

    #[test]
    fn cheese_and_butter_use_the_paper_slots_salt_and_the_original_item_carrier() {
        for (kind, dairy) in [
            (CHEESE, Kind::Vanilla("minecraft:milk_bucket")),
            (BUTTER, HEAVY_CREAM),
        ] {
            let recipe = CRAFTING.iter().find(|recipe| recipe.output.kind == kind).unwrap();
            assert_eq!(recipe.inputs[0..2], [Some(dairy), Some(SALT)]);
            assert!(recipe.inputs[2..].iter().all(Option::is_none));
            assert_eq!(recipe.output.count, 1);
            assert_eq!(recipe.research.id, "ingredients_and_cheese");
            assert_eq!(crate::items::network_item(kind.id()), Some(kind));
            assert_eq!(
                crate::networks::Component::for_item(kind.id()),
                Some(crate::networks::Component::IngredientHead)
            );
            assert_eq!(kind.material(), "minecraft:player_head");
        }
    }

    #[test]
    fn fabricators_and_composters_preserve_all_paper_variants_and_batch_sizes() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                std::iter::once(CAN)
                    .chain(PRODUCE.iter().map(|produce| produce.crop))
                    .map(|item| Identifier::parse(item.material()).unwrap()),
            )
            .unwrap();
        assert_eq!(PRODUCE.len(), 11);
        let fab = crate::advanced_processing::machine("FOOD_FABRICATOR_2").unwrap();
        let comp = crate::advanced_processing::machine("FOOD_COMPOSTER_2").unwrap();
        let can = CAN.stack(&host, 14).unwrap();
        assert_eq!(
            (fab.capacity, fab.cost, comp.capacity, comp.cost),
            (512, 24, 512, 26)
        );

        for produce in PRODUCE {
            let crop = produce.crop.stack(&host, 3);
            assert!(next(&host, fab, &[crop.clone(), None]).is_none());
            let fabricated = next(&host, fab, &[crop, Some(can.clone())]).unwrap();
            assert_eq!(fabricated.work_ticks, 4);
            assert_eq!(
                fabricated
                    .consumed
                    .iter()
                    .map(|(slot, _, count)| (*slot, *count))
                    .collect::<Vec<_>>(),
                [(1, 1), (0, 1)]
            );
            let food = &fabricated.outputs[0];
            assert!(produce.food.matches(&host, food));
            assert_eq!(food.count(), 2);
            assert_eq!(
                food.components()
                    .unwrap()
                    .as_nbt()
                    .get(&"minecraft:lore".into()),
                lore(produce.food.id()).as_ref(),
            );
            let composted = next(&host, comp, &[None, Some(food.clone())]).unwrap();
            assert_eq!(composted.work_ticks, 6);
            assert_eq!((composted.consumed[0].0, composted.consumed[0].2), (1, 1));
            assert!(produce.fertilizer.matches(&host, &composted.outputs[0]));
            assert_eq!(composted.outputs[0].count(), 2);
            assert!(next(&host, comp, &[Some(composted.outputs[0].clone()), None]).is_none());
        }
    }

    #[test]
    fn the_paper_can_ring_uses_eight_tin_ingots_and_is_not_a_placed_product() {
        let recipe = CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == CAN)
            .unwrap();
        assert_eq!(CAN.id(), "CAN");
        assert_eq!(recipe.output.count, 8);
        assert_eq!(recipe.research.id, "first_aid");
        for (position, ingredient) in recipe.inputs.iter().enumerate() {
            assert_eq!(*ingredient, (position != 4).then_some(TIN_INGOT));
        }

        let codec = PLACED_PRODUCE.codec.as_ref().unwrap();
        let data = (codec.encode)(&"FERTILIZER_CARROT".into()).unwrap();
        assert_eq!((codec.decode)(&data), Ok("FERTILIZER_CARROT".into()));
        assert!((codec.decode)(&(codec.encode)(&"CAN".into()).unwrap()).is_err());
    }
}
