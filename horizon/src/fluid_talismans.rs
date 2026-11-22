// SPDX-License-Identifier: GPL-3.0-only

use super::{COMMON_TALISMAN, LAVA_CRYSTAL, LUMP_3, Talisman, infused, recipe};
use crate::{
    items::{Kind, item},
    recipes::{CraftingRecipe, Research},
};

item!(
    LAVA_TALISMAN,
    "emerald",
    "Talisman of the Lava Walker",
    "green",
    None,
    None
);
item!(
    ENDER_LAVA_TALISMAN,
    "emerald",
    "Ender Talisman of the Lava Walker",
    "dark_purple",
    None,
    None
);
item!(
    WATER_TALISMAN,
    "emerald",
    "Talisman of the Water Breather",
    "green",
    None,
    None
);
item!(
    ENDER_WATER_TALISMAN,
    "emerald",
    "Ender Talisman of the Water Breather",
    "dark_purple",
    None,
    None
);

pub(super) const LAVA_RESEARCH: Research = Research {
    id: "lava_talisman",
    name: "Talisman of the Lava Walker",
    cost: 18,
};
pub(super) const WATER_RESEARCH: Research = Research {
    id: "water_talisman",
    name: "Talisman of the Water Breather",
    cost: 18,
};

pub(super) const LAVA_RECIPE: CraftingRecipe = contact_recipe(
    LAVA_TALISMAN,
    &LAVA_RESEARCH,
    LAVA_CRYSTAL,
    Kind::Vanilla("minecraft:lava_bucket"),
);
pub(super) const WATER_RECIPE: CraftingRecipe = contact_recipe(
    WATER_TALISMAN,
    &WATER_RESEARCH,
    Kind::Vanilla("minecraft:water_bucket"),
    Kind::Vanilla("minecraft:fishing_rod"),
);
pub(super) const ENDER_LAVA_RECIPE: CraftingRecipe = infused(ENDER_LAVA_TALISMAN, LAVA_TALISMAN, 4);
pub(super) const ENDER_WATER_RECIPE: CraftingRecipe =
    infused(ENDER_WATER_TALISMAN, WATER_TALISMAN, 4);

const fn contact_recipe(
    output: Kind,
    research: &'static Research,
    left: Kind,
    right: Kind,
) -> CraftingRecipe {
    recipe(
        output,
        research,
        4,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(left),
            Some(COMMON_TALISMAN),
            Some(right),
            LUMP_3,
            None,
            LUMP_3,
        ],
    )
}

pub(super) const LAVA: Talisman = Talisman {
    normal: LAVA_TALISMAN,
    ender: ENDER_LAVA_TALISMAN,
    research: &LAVA_RESEARCH,
    chance: 100,
    message: "Your Talisman saved you from burning to death",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it will",
        "give you Fire Resistance",
        "as soon as you touch Lava",
        "but will then be consumed",
    ],
};
pub(super) const WATER: Talisman = Talisman {
    normal: WATER_TALISMAN,
    ender: ENDER_WATER_TALISMAN,
    research: &WATER_RESEARCH,
    chance: 100,
    message: "Your Talisman saved you from drowning",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it will",
        "give you the ability",
        "to breath underwater as",
        "soon as you start drowning",
        "but will then be consumed",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::talismans::{ENDER, ENDER_RESEARCH, environment_effect, roll_succeeds};

    #[test]
    fn fluid_talisman_recipes_keep_the_original_ingredients_and_batch_sizes() {
        for (normal, ender, spec, left, right) in [
            (
                LAVA_RECIPE,
                ENDER_LAVA_RECIPE,
                &LAVA,
                LAVA_CRYSTAL,
                Kind::Vanilla("minecraft:lava_bucket"),
            ),
            (
                WATER_RECIPE,
                ENDER_WATER_RECIPE,
                &WATER,
                Kind::Vanilla("minecraft:water_bucket"),
                Kind::Vanilla("minecraft:fishing_rod"),
            ),
        ] {
            assert_eq!(normal.output.kind, spec.normal);
            assert_eq!(ender.output.kind, spec.ender);
            assert_eq!((normal.output.count, ender.output.count), (4, 4));
            assert_eq!(normal.research.id, spec.research.id);
            assert_eq!(normal.research.cost, 18);
            assert_eq!(ender.research.id, ENDER_RESEARCH.id);
            assert_eq!(
                normal.inputs,
                [
                    LUMP_3,
                    None,
                    LUMP_3,
                    Some(left),
                    Some(COMMON_TALISMAN),
                    Some(right),
                    LUMP_3,
                    None,
                    LUMP_3,
                ]
            );
            assert_eq!(
                ender.inputs,
                [
                    ENDER,
                    None,
                    ENDER,
                    None,
                    Some(spec.normal),
                    None,
                    ENDER,
                    None,
                    ENDER,
                ]
            );
        }
    }

    #[test]
    fn fluid_talismans_react_to_their_own_exact_paper_damage_cause() {
        for (cause, resource, spec) in [
            ("minecraft:lava", "minecraft:fire_resistance", &LAVA),
            ("minecraft:drown", "minecraft:water_breathing", &WATER),
        ] {
            let (actual, effect) = environment_effect(cause).unwrap();
            assert_eq!(actual.normal, spec.normal);
            assert_eq!(effect, resource);
            assert!(roll_succeeds(actual, 99));
        }
        for cause in [
            "minecraft:on_fire",
            "minecraft:hot_floor",
            "minecraft:dry_out",
        ] {
            assert!(environment_effect(cause).is_none(), "{cause}");
        }
    }
}
