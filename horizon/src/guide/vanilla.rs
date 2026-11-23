// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, Identifier, Ingredient, ItemResource, ItemStack, Recipe, RecipeRule};

const KINDS: &[&str] = &[
    "minecraft:crafting",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:smoking",
    "minecraft:campfire_cooking",
    "minecraft:stonecutting",
    "minecraft:smithing",
];
const CHOICE_TICKS: u8 = 14;

pub(super) struct Match {
    pub result: ItemStack,
    pub recipe: Recipe,
}

pub(super) fn recipes_for(host: &dyn Host, item: &ItemResource) -> Vec<Match> {
    let mut matches = Vec::new();
    for &kind in KINDS {
        for recipe in host.recipes_of_type(kind).iter() {
            let Some(result) = result(host, recipe).filter(|stack| item.matches(stack)) else {
                continue;
            };
            matches.push(Match {
                result,
                recipe: recipe.clone(),
            });
        }
    }
    matches
}

fn result(host: &dyn Host, recipe: &Recipe) -> Option<ItemStack> {
    match recipe.rule.as_ref() {
        Some(RecipeRule::Stonecutting { result, .. }) => Some(result.clone()),
        Some(RecipeRule::SmithingTransform { result, .. }) if result.item.as_deref().is_some() => {
            host.recipe_result(&recipe.json.get("result")?.to_string())
                .ok()
                .flatten()
        }
        Some(RecipeRule::Transmute { result, .. }) => {
            // Paper's CraftTransmuteRecipe indexes the replacement item as a
            // default stack. Dough does not have a visualization for that type.
            let key = host.item_key(&Identifier::parse(result.item.as_deref()?).ok()?)?;
            ItemStack::new(key, 1)
        }
        Some(RecipeRule::Dye { .. } | RecipeRule::Imbue { .. }) => host
            .recipe_result(&recipe.json.get("result")?.to_string())
            .ok()
            .flatten(),
        // Paper's Bukkit generic smithing trim result is AIR. Other dynamic
        // results likewise need concrete inputs and are absent from getRecipesFor.
        Some(_) => None,
        None => recipe.results.first().cloned(),
    }
}

pub(super) fn method(recipe: &Recipe) -> Option<(&'static str, &'static str)> {
    if matches!(
        recipe.rule,
        Some(RecipeRule::Transmute { .. } | RecipeRule::Dye { .. } | RecipeRule::Imbue { .. })
    ) {
        return None;
    }
    Some(match recipe.kind.as_str() {
        "minecraft:crafting" if shaped(recipe) => ("minecraft:crafting_table", "minecraft:shaped"),
        "minecraft:crafting" => ("minecraft:crafting_table", "minecraft:shapeless"),
        "minecraft:smelting" => ("minecraft:furnace", "minecraft:furnace"),
        "minecraft:blasting" => ("minecraft:blast_furnace", "minecraft:blasting"),
        "minecraft:smoking" => ("minecraft:smoker", "minecraft:smoking"),
        "minecraft:campfire_cooking" => ("minecraft:campfire", "minecraft:campfire"),
        "minecraft:stonecutting" => ("minecraft:stonecutter", "minecraft:stonecutting"),
        "minecraft:smithing" => ("minecraft:smithing_table", "minecraft:smithing"),
        _ => return None,
    })
}

fn shaped(recipe: &Recipe) -> bool {
    recipe.json.get("pattern").is_some()
}

fn ingredient_slots(recipe: &Recipe) -> Vec<(usize, &Ingredient)> {
    match recipe.rule.as_ref() {
        Some(RecipeRule::Stonecutting { input, .. }) => vec![(4, input)],
        // Dough's smithing view shows the base and addition in that order.
        Some(RecipeRule::SmithingTransform { base, addition, .. }) => {
            [(0, Some(base)), (1, addition.as_ref())]
                .into_iter()
                .filter_map(|(index, ingredient)| Some((index, ingredient?)))
                .collect()
        }
        Some(_) => Vec::new(),
        None if shaped(recipe) => pattern_cells(recipe)
            .into_iter()
            .zip(&recipe.ingredients)
            .collect(),
        None if recipe.ingredients.len() == 1 => vec![(4, &recipe.ingredients[0])],
        None => recipe.ingredients.iter().take(9).enumerate().collect(),
    }
}

fn pattern_cells(recipe: &Recipe) -> Vec<usize> {
    let Some(pattern) = recipe.json.get("pattern").and_then(|rows| rows.as_array()) else {
        return Vec::new();
    };
    let cells: Vec<_> = pattern
        .iter()
        .filter_map(|row| row.as_str())
        .enumerate()
        .flat_map(|(y, row)| {
            row.chars()
                .enumerate()
                .filter_map(move |(x, key)| (key != ' ').then_some((x, y)))
        })
        .collect();
    let Some(left) = cells.iter().map(|&(x, _)| x).min() else {
        return Vec::new();
    };
    let top = cells[0].1;
    // The engine shrinks leading/trailing empty rows and columns, like Bukkit's
    // shaped recipes. Empty cells within that shape still occupy their grid slots.
    cells
        .into_iter()
        .map(|(x, y)| (y - top) * 3 + x - left)
        .filter(|&cell| cell < 9)
        .collect()
}

pub(super) struct Choice {
    pub slot: usize,
    pub items: Vec<ItemStack>,
}

pub(super) fn choices(host: &dyn Host, recipe: &Recipe) -> Vec<Choice> {
    ingredient_slots(recipe)
        .into_iter()
        .filter_map(|(index, ingredient)| {
            let items: Vec<_> = ingredient
                .order
                .iter()
                .filter_map(|id| {
                    let key = host.item_key(&Identifier::parse(id).ok()?)?;
                    ItemStack::new(key, 1)
                })
                .collect();
            (!items.is_empty()).then_some(Choice {
                slot: super::INGREDIENT_SLOTS[index],
                items,
            })
        })
        .collect()
}

pub(super) struct Rotation {
    choices: Vec<(usize, usize, Vec<ItemStack>)>,
    slots: Vec<Option<ItemStack>>,
    elapsed: u8,
}

impl Rotation {
    pub(super) fn new(choices: Vec<Choice>, slots: &[Option<ItemStack>]) -> Option<Self> {
        if choices.is_empty() {
            return None;
        }
        Some(Self {
            choices: choices
                .into_iter()
                .map(|choice| (choice.slot, 0, choice.items))
                .collect(),
            slots: slots.to_vec(),
            elapsed: 0,
        })
    }

    pub(super) fn advance(&mut self) -> Option<Vec<Option<ItemStack>>> {
        self.elapsed += 1;
        if self.elapsed < CHOICE_TICKS {
            return None;
        }
        self.elapsed = 0;
        for (slot, index, choices) in &mut self.choices {
            *index = (*index + 1) % choices.len();
            self.slots[*slot] = Some(choices[*index].clone());
        }
        Some(self.slots.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn shaped_cells_preserve_holes_after_bukkits_pattern_shrink() {
        let mut recipe = Recipe::new(
            "test:sparse",
            "minecraft:crafting",
            Arc::new(json!({
                "pattern": ["   ", " A ", " AB"],
            })),
        );
        recipe.ingredients = vec![Ingredient::of(vec!["minecraft:stone".into()]); 3];
        let slots: Vec<_> = ingredient_slots(&recipe)
            .into_iter()
            .map(|(slot, _)| slot)
            .collect();
        assert_eq!(slots, [0, 3, 4]);
        assert_eq!(
            method(&recipe),
            Some(("minecraft:crafting_table", "minecraft:shaped"))
        );
    }

    #[test]
    fn smithing_hides_the_template_and_cooking_centers_its_input() {
        let mut recipe = Recipe::new("test:smoking", "minecraft:smoking", Arc::new(json!({})));
        let base = Ingredient::of(vec!["minecraft:cod".into()]);
        recipe.ingredients.push(base.clone());
        assert_eq!(ingredient_slots(&recipe), vec![(4, &base)]);
        assert_eq!(
            method(&recipe),
            Some(("minecraft:smoker", "minecraft:smoking"))
        );

        let mut recipe = Recipe::new("test:smithing", "minecraft:smithing", Arc::new(json!({})));
        let template = Ingredient::of(vec!["minecraft:netherite_upgrade_smithing_template".into()]);
        let addition = Ingredient::of(vec!["minecraft:netherite_ingot".into()]);
        recipe.rule = Some(RecipeRule::SmithingTransform {
            template: Some(template),
            base: base.clone(),
            addition: Some(addition.clone()),
            result: mod_api::RecipeTemplate::new(
                Some("minecraft:netherite_pickaxe".into()),
                1,
                None,
            ),
        });
        assert_eq!(ingredient_slots(&recipe), vec![(0, &base), (1, &addition)]);
        assert_eq!(
            method(&recipe),
            Some(("minecraft:smithing_table", "minecraft:smithing"))
        );
    }

    #[test]
    fn material_choices_rotate_at_paper_interval_and_wrap() {
        mod_api::__private::initialize().unwrap();
        let mut host = FakeHost::new();
        let ids = ["minecraft:oak_planks", "minecraft:birch_planks"];
        host.registries
            .resolve_items(ids.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        let mut recipe = Recipe::new("test:planks", "minecraft:crafting", Arc::new(json!({})));
        recipe
            .ingredients
            .push(Ingredient::of(ids.map(str::to_owned).to_vec()));
        let choices = choices(&host, &recipe);
        let slot = super::super::INGREDIENT_SLOTS[4];
        assert_eq!(choices[0].slot, slot);
        let mut slots = vec![None; 54];
        slots[slot] = choices[0].items.first().cloned();
        let mut rotation = Rotation::new(choices, &slots).unwrap();
        for expected in [ids[1], ids[0]] {
            for _ in 1..CHOICE_TICKS {
                assert!(rotation.advance().is_none());
            }
            let visible = rotation.advance().unwrap();
            assert_eq!(visible[slot].as_ref().unwrap().item().id(&host), expected);
        }
    }
}
