// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ComponentId, Compound, Host, Identifier, ItemResource, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine, Plan},
    items::{ELECTRIC_MOTOR, HEATING_COIL, Kind, REINFORCED_PLATE, item},
    recipes::{CraftingRecipe, Research, Stack},
};

item!(AUTO_BREWER, "smoker", "Auto Brewer", "gold", None, None);

pub(crate) const RESEARCH: Research = Research {
    id: "auto_brewer",
    name: "Industrial Brewery",
    cost: 30,
};

pub(crate) const MACHINE: Machine = Machine {
    item: AUTO_BREWER,
    title: "Auto Brewer",
    family: Family::AutoBrewer,
    capacity: 128,
    cost: 6,
    speed: 1,
    research: &RESEARCH,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(HEATING_COIL),
        None,
        Some(REINFORCED_PLATE),
        Some(Kind::Vanilla("minecraft:brewing_stand")),
        Some(REINFORCED_PLATE),
        None,
        Some(ELECTRIC_MOTOR),
        None,
    ],
    output: Stack {
        kind: AUTO_BREWER,
        count: 1,
    },
    research: &RESEARCH,
};

const POTION: &str = "minecraft:potion";
const SPLASH: &str = "minecraft:splash_potion";
const LINGERING: &str = "minecraft:lingering_potion";
const CONTENTS: &str = "minecraft:potion_contents";

pub(crate) const AWKWARD_RECIPES: &[(&str, &str)] = &[
    ("minecraft:sugar", "minecraft:swiftness"),
    ("minecraft:rabbit_foot", "minecraft:leaping"),
    ("minecraft:blaze_powder", "minecraft:strength"),
    ("minecraft:glistering_melon_slice", "minecraft:healing"),
    ("minecraft:spider_eye", "minecraft:poison"),
    ("minecraft:ghast_tear", "minecraft:regeneration"),
    ("minecraft:magma_cream", "minecraft:fire_resistance"),
    ("minecraft:pufferfish", "minecraft:water_breathing"),
    ("minecraft:golden_carrot", "minecraft:night_vision"),
    ("minecraft:turtle_helmet", "minecraft:turtle_master"),
    ("minecraft:phantom_membrane", "minecraft:slow_falling"),
];

const FERMENTATIONS: &[(&str, &str)] = &[
    ("minecraft:swiftness", "minecraft:slowness"),
    ("minecraft:leaping", "minecraft:slowness"),
    ("minecraft:healing", "minecraft:harming"),
    ("minecraft:poison", "minecraft:harming"),
    ("minecraft:night_vision", "minecraft:invisibility"),
];

// Bukkit exposes these flags through PotionType.InternalPotionData. Values
// were read from the Paper 26.2-132 registries, including the long/strong types.
const EXTENDABLE_ONLY: &[&str] = &[
    "minecraft:fire_resistance",
    "minecraft:invisibility",
    "minecraft:night_vision",
    "minecraft:slow_falling",
    "minecraft:water_breathing",
    "minecraft:weakness",
];
const UPGRADEABLE_ONLY: &[&str] = &["minecraft:harming", "minecraft:healing"];

fn lookup(recipes: &[(&str, &'static str)], input: &str) -> Option<&'static str> {
    recipes
        .iter()
        .find_map(|&(key, output)| (key == input).then_some(output))
}

fn combination<'a>(
    vessel: &'a str,
    potion: &'a str,
    ingredient: &str,
) -> Option<(&'a str, &'a str)> {
    Some(match (potion, ingredient) {
        ("minecraft:water", "minecraft:fermented_spider_eye") => (vessel, "minecraft:weakness"),
        ("minecraft:water", "minecraft:nether_wart") => (vessel, "minecraft:awkward"),
        ("minecraft:water", "minecraft:gunpowder") if vessel == POTION => (SPLASH, potion),
        ("minecraft:water", "minecraft:dragon_breath") if vessel == SPLASH => (LINGERING, potion),
        ("minecraft:water", _) => return None,
        (_, "minecraft:fermented_spider_eye") => (vessel, lookup(FERMENTATIONS, potion)?),
        // Paper's modern branch assigns the same PotionType while consuming the modifier.
        (_, "minecraft:redstone") if EXTENDABLE_ONLY.contains(&potion) => (vessel, potion),
        (_, "minecraft:glowstone_dust") if UPGRADEABLE_ONLY.contains(&potion) => (vessel, potion),
        ("minecraft:awkward", _) => (vessel, lookup(AWKWARD_RECIPES, ingredient)?),
        _ => return None,
    })
}

pub(crate) fn next(host: &dyn Host, inputs: &[Option<ItemStack>]) -> Option<Plan> {
    let [Some(first), Some(second)] = inputs else {
        return None;
    };
    let first_id = first.item().id(host);
    let second_id = second.item().id(host);
    let (vessel, potion, ingredient, ingredient_id) =
        if matches!(first_id.as_str(), POTION | SPLASH | LINGERING) {
            (first_id.as_str(), first, second, second_id.as_str())
        } else if matches!(second_id.as_str(), POTION | SPLASH | LINGERING) {
            (second_id.as_str(), second, first, first_id.as_str())
        } else {
            return None;
        };
    if ingredient.components().is_some() || ingredient.damage_component().is_some() {
        return None;
    }
    let Tag::Compound(mut contents) =
        ItemResource::of(potion).value(host, &ComponentId::new(CONTENTS)?)?
    else {
        return None;
    };
    let Tag::String(base) = contents.get(&"potion".into())? else {
        return None;
    };
    let base = base.to_string().ok()?;
    let (brewed_vessel, brewed_potion) = combination(vessel, &base, ingredient_id)?;
    let key = host.item_key(&Identifier::parse(brewed_vessel).ok()?)?;
    let mut output = ItemStack::new(key, 1)?;
    if let Some(patch) = potion.components() {
        output.set_components(patch.as_nbt().clone()).ok()?;
    }
    if let Some(damage) = potion.damage_component() {
        output.set_damage(damage);
    }
    contents.insert("potion".into(), Tag::String(brewed_potion.into()));
    output
        .apply_components(&Compound::from([(
            CONTENTS.into(),
            Tag::Compound(contents),
        )]))
        .ok()?;
    Some(Plan {
        consumed: vec![(0, first.clone(), 1), (1, second.clone(), 1)],
        outputs: vec![output],
        work_ticks: 60,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn brewing_preserves_the_paper_modern_branch_for_potion_modifiers() {
        assert_eq!(
            combination(POTION, "minecraft:water", "minecraft:gunpowder"),
            Some((SPLASH, "minecraft:water"))
        );
        assert_eq!(
            combination(SPLASH, "minecraft:water", "minecraft:dragon_breath"),
            Some((LINGERING, "minecraft:water"))
        );
        assert_eq!(
            combination(
                LINGERING,
                "minecraft:poison",
                "minecraft:fermented_spider_eye"
            ),
            Some((LINGERING, "minecraft:harming"))
        );
        assert_eq!(
            combination(POTION, "minecraft:night_vision", "minecraft:redstone"),
            Some((POTION, "minecraft:night_vision"))
        );
        assert_eq!(
            combination(POTION, "minecraft:healing", "minecraft:glowstone_dust"),
            Some((POTION, "minecraft:healing"))
        );
        assert_eq!(
            combination(POTION, "minecraft:swiftness", "minecraft:redstone"),
            None
        );
        assert_eq!(
            combination(POTION, "minecraft:long_night_vision", "minecraft:redstone"),
            None
        );
        assert_eq!(
            combination(POTION, "minecraft:swiftness", "minecraft:gunpowder"),
            None
        );
    }

    #[test]
    fn brewing_preserves_owned_potion_metadata_and_refuses_modified_ingredients() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [POTION, "minecraft:nether_wart"].map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let mut bottle = Kind::Vanilla(POTION).stack(&host, 1).unwrap();
        let contents = |base: &str| {
            Tag::compound([
                ("potion", Tag::String(base.into())),
                ("custom_color", Tag::Int(100)),
            ])
        };
        let name = Tag::String("Ordinary test potion".into());
        bottle
            .set_components(Compound::from([
                (CONTENTS.into(), contents("minecraft:water")),
                ("minecraft:custom_name".into(), name.clone()),
            ]))
            .unwrap();
        let mut wart = Kind::Vanilla("minecraft:nether_wart")
            .stack(&host, 3)
            .unwrap();
        let recipe = next(&host, &[Some(wart.clone()), Some(bottle.clone())]).unwrap();
        assert_eq!(recipe.work_ticks, 60);
        assert_eq!(recipe.consumed[0].2, 1);
        assert_eq!(recipe.consumed[1].2, 1);
        let patch = recipe.outputs[0].components().unwrap().as_nbt();
        assert_eq!(
            patch.get(&CONTENTS.into()),
            Some(&contents("minecraft:awkward"))
        );
        assert_eq!(patch.get(&"minecraft:custom_name".into()), Some(&name));
        wart.set_components(Compound::from([("minecraft:custom_name".into(), name)]))
            .unwrap();
        assert!(next(&host, &[Some(wart), Some(bottle)]).is_none());
    }
}
