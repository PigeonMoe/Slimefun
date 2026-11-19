// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, Host, ItemStack, Tag};

use crate::{
    armor_forge::ELYTRA_SCALE,
    backpacks::ESSENCE_OF_AFTERLIFE,
    factory::experience_collector::FLASK_OF_KNOWLEDGE,
    items::{Kind, item},
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    SOULBOUND_SWORD,
    "diamond_sword",
    "Soulbound Sword",
    "red",
    None,
    None
);
item!(
    SOULBOUND_TRIDENT,
    "trident",
    "Soulbound Trident",
    "red",
    None,
    None
);
item!(SOULBOUND_BOW, "bow", "Soulbound Bow", "red", None, None);
item!(
    SOULBOUND_PICKAXE,
    "diamond_pickaxe",
    "Soulbound Pickaxe",
    "red",
    None,
    None
);
item!(
    SOULBOUND_AXE,
    "diamond_axe",
    "Soulbound Axe",
    "red",
    None,
    None
);
item!(
    SOULBOUND_SHOVEL,
    "diamond_shovel",
    "Soulbound Shovel",
    "red",
    None,
    None
);
item!(
    SOULBOUND_HOE,
    "diamond_hoe",
    "Soulbound Hoe",
    "red",
    None,
    None
);
item!(
    SOULBOUND_HELMET,
    "diamond_helmet",
    "Soulbound Helmet",
    "red",
    None,
    None
);
item!(
    SOULBOUND_CHESTPLATE,
    "diamond_chestplate",
    "Soulbound Chestplate",
    "red",
    None,
    None
);
item!(
    SOULBOUND_LEGGINGS,
    "diamond_leggings",
    "Soulbound Leggings",
    "red",
    None,
    None
);
item!(
    SOULBOUND_BOOTS,
    "diamond_boots",
    "Soulbound Boots",
    "red",
    None,
    None
);
item!(
    SOULBOUND_ELYTRA,
    "elytra",
    "Soulbound Elytra",
    "red",
    None,
    None
);
item!(
    INFUSED_ELYTRA,
    "elytra",
    "Infused Elytra",
    "dark_purple",
    None,
    None
);

const BOUND_WEAPONS: Research = Research {
    id: "bound_weapons",
    name: "Soulbound Weapons",
    cost: 29,
};
const BOUND_TOOLS: Research = Research {
    id: "bound_tools",
    name: "Soulbound Tools",
    cost: 29,
};
const BOUND_ARMOR: Research = Research {
    id: "bound_armor",
    name: "Soulbound Armor",
    cost: 29,
};
const SPECIAL_ELYTRAS: Research = Research {
    id: "special_elytras",
    name: "Special Elytras",
    cost: 30,
};

pub(crate) const RESEARCHES: &[&Research] =
    &[&BOUND_WEAPONS, &BOUND_TOOLS, &BOUND_ARMOR, &SPECIAL_ELYTRAS];

const ESSENCE: Option<Kind> = Some(ESSENCE_OF_AFTERLIFE);
const FLASK: Option<Kind> = Some(FLASK_OF_KNOWLEDGE);
const SCALE: Option<Kind> = Some(ELYTRA_SCALE);
const ELYTRA: Option<Kind> = Some(Kind::Vanilla("minecraft:elytra"));

const fn bind(item: Kind, research: &'static Research) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            None,
            ESSENCE,
            None,
            None,
            Some(Kind::Vanilla(item.material())),
            None,
            None,
            ESSENCE,
            None,
        ],
        output: Stack {
            kind: item,
            count: 1,
        },
        research,
    }
}

pub(crate) const MAGIC_RECIPES: &[CraftingRecipe] = &[
    bind(SOULBOUND_SWORD, &BOUND_WEAPONS),
    bind(SOULBOUND_TRIDENT, &BOUND_WEAPONS),
    bind(SOULBOUND_BOW, &BOUND_WEAPONS),
    bind(SOULBOUND_PICKAXE, &BOUND_TOOLS),
    bind(SOULBOUND_AXE, &BOUND_TOOLS),
    bind(SOULBOUND_SHOVEL, &BOUND_TOOLS),
    bind(SOULBOUND_HOE, &BOUND_TOOLS),
    bind(SOULBOUND_HELMET, &BOUND_ARMOR),
    bind(SOULBOUND_CHESTPLATE, &BOUND_ARMOR),
    bind(SOULBOUND_LEGGINGS, &BOUND_ARMOR),
    bind(SOULBOUND_BOOTS, &BOUND_ARMOR),
];

pub(crate) const ALTAR_RECIPES: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            FLASK, SCALE, FLASK, FLASK, ELYTRA, FLASK, FLASK, SCALE, FLASK,
        ],
        output: Stack {
            kind: INFUSED_ELYTRA,
            count: 1,
        },
        research: &SPECIAL_ELYTRAS,
    },
    CraftingRecipe {
        inputs: [
            FLASK, ESSENCE, FLASK, SCALE, ELYTRA, SCALE, FLASK, ESSENCE, FLASK,
        ],
        output: Stack {
            kind: SOULBOUND_ELYTRA,
            count: 1,
        },
        research: &SPECIAL_ELYTRAS,
    },
];

pub(crate) fn keeps_on_death(host: &dyn Host, stack: &ItemStack) -> bool {
    SOULBOUND_ELYTRA.matches(host, stack)
        || MAGIC_RECIPES
            .iter()
            .any(|recipe| recipe.output.kind.matches(host, stack))
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == INFUSED_ELYTRA.id() {
        components.insert(
            "minecraft:enchantments".into(),
            Tag::compound([("minecraft:mending", Tag::Int(1))]),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn original_gear_recipes_keep_only_real_soulbound_items() {
        let mut host = FakeHost::new();
        let registered = MAGIC_RECIPES
            .iter()
            .chain(ALTAR_RECIPES)
            .map(|recipe| recipe.output.kind.material());
        host.registries
            .resolve_items(registered.map(|material| Identifier::parse(material).unwrap()))
            .unwrap();
        for recipe in MAGIC_RECIPES {
            let item = recipe.output.kind;
            assert_eq!(recipe.inputs[4], Some(Kind::Vanilla(item.material())));
            assert_eq!([recipe.inputs[1], recipe.inputs[7]], [ESSENCE, ESSENCE]);
            assert!(keeps_on_death(&host, &item.stack(&host, 1).unwrap()));
            assert!(!keeps_on_death(
                &host,
                &Kind::Vanilla(item.material()).stack(&host, 1).unwrap()
            ));
        }
        assert!(keeps_on_death(
            &host,
            &SOULBOUND_ELYTRA.stack(&host, 1).unwrap()
        ));
        let infused = INFUSED_ELYTRA.stack(&host, 1).unwrap();
        assert!(!keeps_on_death(&host, &infused));
        assert_eq!(
            infused
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:enchantments".into()),
            Some(&Tag::compound([("minecraft:mending", Tag::Int(1))]))
        );
    }
}
