// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use mod_api::{
    Host, Identifier, Ingredient, ItemResource, ItemStack, LevelRef, Recipe, RecipeInput,
};

const CRAFTING: &str = "minecraft:crafting";

pub(super) struct VanillaRecipe {
    id: String,
    ingredients: Vec<Ingredient>,
    layout: [Option<usize>; 9],
    output: ItemStack,
    shaped: bool,
}

impl VanillaRecipe {
    fn read(recipe: &Recipe) -> Option<Self> {
        let serializer = recipe.json.get("type")?.as_str()?;
        let shaped = match serializer.strip_prefix("minecraft:").unwrap_or(serializer) {
            "crafting_shaped" => true,
            "crafting_shapeless" => false,
            _ => return None,
        };
        let output = recipe.results.first()?.clone();
        let layout = if shaped {
            let rows = recipe.json.get("pattern")?.as_array()?;
            shaped_grid(
                rows.iter()
                    .map(|row| row.as_str())
                    .collect::<Option<Vec<_>>>()?,
                recipe.ingredients.len(),
            )?
        } else {
            if !(1..=9).contains(&recipe.ingredients.len()) {
                return None;
            }
            std::array::from_fn(|slot| (slot < recipe.ingredients.len()).then_some(slot))
        };
        Some(Self {
            id: recipe.id.clone(),
            ingredients: recipe.ingredients.clone(),
            layout,
            output,
            shaped,
        })
    }

    pub(super) fn input(&self) -> [Option<&Ingredient>; 9] {
        self.layout
            .map(|index| index.map(|index| &self.ingredients[index]))
    }

    pub(super) fn preview(&self, host: &dyn Host, frame: usize) -> [Option<ItemStack>; 9] {
        let layout = if !self.shaped && self.ingredients.len() == 1 {
            // Paper displays a single shapeless choice at the center.
            std::array::from_fn(|slot| (slot == 4).then_some(0))
        } else {
            self.layout
        };
        layout.map(|index| {
            let ingredient = &self.ingredients[index?];
            let material = &ingredient.order[frame.checked_rem(ingredient.order.len())?];
            let id = Identifier::parse(material).ok()?;
            let stack = ItemStack::new(host.item_key(&id)?, 1)?;
            ingredient.test(host, &stack).then_some(stack)
        })
    }

    pub(super) fn output(&self) -> ItemStack {
        self.output.clone()
    }

    pub(super) fn animated(&self) -> bool {
        self.ingredients
            .iter()
            .any(|ingredient| ingredient.order.len() > 1)
    }

    pub(super) fn assemble(
        &self,
        host: &dyn Host,
        level: LevelRef,
        slots: Vec<Option<ItemStack>>,
    ) -> Option<ItemStack> {
        let input = RecipeInput::Crafting {
            width: 3,
            height: 3,
            slots,
        };
        let kind = Identifier::parse(CRAFTING).ok()?;
        let matched = level.recipe_for(host, &kind, &input).ok()??;
        if matched.recipe.to_string() == self.id {
            Some(matched.output)
        } else {
            // The Paper menu selects one exact recipe. The engine may prefer a
            // different recipe with the same layout. The selected typed predicates
            // were already checked on this grid, so retain its declared result.
            Some(self.output())
        }
    }
}

fn shaped_grid(rows: Vec<&str>, ingredients: usize) -> Option<[Option<usize>; 9]> {
    if !(1..=3).contains(&rows.len()) || !(1..=9).contains(&ingredients) {
        return None;
    }
    let width = rows[0].chars().count();
    if !(1..=3).contains(&width) || rows.iter().any(|row| row.chars().count() != width) {
        return None;
    }
    let mut index = 0;
    let mut layout = [None; 9];
    for (y, row) in rows.into_iter().enumerate() {
        for (x, cell) in row.chars().enumerate() {
            if cell != ' ' {
                layout[y * 3 + x] = Some(index);
                index += 1;
            }
        }
    }
    (index == ingredients).then_some(layout)
}

type Recipes = Arc<BTreeMap<String, Arc<VanillaRecipe>>>;
static RECIPES: Mutex<Option<Recipes>> = Mutex::new(None);

pub(super) fn invalidate() {
    *RECIPES.lock().unwrap() = None;
}

fn recipes(host: &dyn Host) -> Recipes {
    if let Some(saved) = RECIPES.lock().unwrap().clone() {
        return saved;
    }
    let loaded = Arc::new(
        host.recipes_of_type(CRAFTING)
            .iter()
            .filter_map(|recipe| {
                let recipe = VanillaRecipe::read(recipe)?;
                Some((recipe.id.clone(), Arc::new(recipe)))
            })
            .collect(),
    );
    RECIPES.lock().unwrap().get_or_insert(loaded).clone()
}

pub(super) fn find(host: &dyn Host, id: &str) -> Option<Arc<VanillaRecipe>> {
    recipes(host).get(id).cloned()
}

pub(super) fn candidates(host: &dyn Host, sample: &ItemStack) -> Vec<String> {
    // Paper reads the server recipe registry without using the player's recipe book.
    choices(recipes(host).values(), sample)
}

fn choices<'a>(
    recipes: impl Iterator<Item = &'a Arc<VanillaRecipe>>,
    sample: &ItemStack,
) -> Vec<String> {
    let result = ItemResource::of(sample);
    recipes
        .filter(|recipe| result.matches(&recipe.output))
        .map(|recipe| recipe.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Compound, Tag, testing::FakeHost};

    #[test]
    fn vanilla_shaped_patterns_keep_empty_cells_and_dispenser_order() {
        let rows = vec![" AB", "C A"];
        assert_eq!(
            shaped_grid(rows.clone(), 4),
            Some([
                None,
                Some(0),
                Some(1),
                Some(2),
                None,
                Some(3),
                None,
                None,
                None
            ])
        );
        assert!(shaped_grid(rows, 3).is_none());
        assert!(shaped_grid(vec!["a", "bb"], 3).is_none());
    }

    #[test]
    fn selection_checks_result_components_across_the_server_recipe_list() {
        let mut host = FakeHost::new();
        let stick = Identifier::parse("minecraft:stick").unwrap();
        host.registries.resolve_items([stick.clone()]).unwrap();
        let stick = (&host as &dyn Host).item_key(&stick).unwrap();
        let plain = ItemStack::new(stick, 1).unwrap();
        let mut renamed = ItemStack::new(stick, 3).unwrap();
        renamed
            .set_components(Compound::from([(
                "minecraft:custom_name".into(),
                Tag::String("Custom Stick".into()),
            )]))
            .unwrap();
        let make = |id, output| {
            let source = r#"{"type":"minecraft:crafting_shapeless"}"#.parse().unwrap();
            let mut recipe = Recipe::new(id, CRAFTING, Arc::new(source));
            recipe
                .ingredients
                .push(Ingredient::of(vec!["minecraft:bamboo".into()]));
            recipe.results.push(output);
            Arc::new(VanillaRecipe::read(&recipe).unwrap())
        };
        let list = [
            make("demo:a", plain.clone()),
            make("demo:b", plain.clone()),
            make("demo:c", renamed.clone()),
        ];
        assert_eq!(choices(list.iter(), &plain), ["demo:a", "demo:b"]);
        assert_eq!(choices(list.iter(), &renamed), ["demo:c"]);
    }
}
