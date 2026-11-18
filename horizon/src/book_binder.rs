// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;

use mod_api::{Compound, Host, Identifier, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine, Plan},
    enchantment_machines::{STORED, level, levels},
    items::{HARDENED_METAL_INGOT, Kind, SMALL_CAPACITOR, item, text},
    network_recipes::SYNTHETIC_SAPPHIRE,
    recipes::{CraftingRecipe, Research, Stack},
};

item!(BOOK_BINDER, "bookshelf", "Book Binder", "gold", None, None);

pub(crate) const RESEARCH: Research = Research {
    id: "book_binder",
    name: "Enchantment Book Binding",
    cost: 26,
};

pub(crate) const MACHINE: Machine = Machine {
    item: BOOK_BINDER,
    title: "Book Binder",
    family: Family::BookBinder,
    capacity: 256,
    cost: 16,
    speed: 1,
    research: &RESEARCH,
};

const ENCHANTING_TABLE: Kind = Kind::Vanilla("minecraft:enchanting_table");
const BOOKSHELF: Kind = Kind::Vanilla("minecraft:bookshelf");
const ENCHANTED_BOOK: Kind = Kind::Vanilla("minecraft:enchanted_book");

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(ENCHANTING_TABLE),
        None,
        Some(BOOKSHELF),
        Some(HARDENED_METAL_INGOT),
        Some(BOOKSHELF),
        Some(SYNTHETIC_SAPPHIRE),
        Some(SMALL_CAPACITOR),
        Some(SYNTHETIC_SAPPHIRE),
    ],
    output: Stack {
        kind: BOOK_BINDER,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore(id: &str) -> Option<Tag> {
    (id == BOOK_BINDER.id()).then(|| {
        Tag::List(vec![
            Tag::String("".into()),
            text("Binds multiple enchanted books into one.", "white", false),
            Tag::String("".into()),
            text("Medium Machine", "green", false),
            Tag::compound([
                ("text", Tag::String("".into())),
                ("italic", Tag::Byte(0)),
                (
                    "extra",
                    Tag::List(vec![
                        text("⇨ ", "dark_gray", false),
                        text("⚡ ", "yellow", false),
                        text("16 J/s", "gray", false),
                    ]),
                ),
            ]),
        ])
    })
}

#[derive(Clone, Copy)]
pub(crate) struct Rules {
    bypass: bool,
    custom_limit: bool,
    limit: u32,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            bypass: false,
            custom_limit: false,
            limit: 15,
        }
    }
}

impl Rules {
    pub(crate) fn read_config(config: &toml::Table) -> Self {
        let Some(values) = config.get("book-binder").and_then(toml::Value::as_table) else {
            return Self::default();
        };
        let defaults = Self::default();
        Self {
            bypass: values
                .get("bypass-vanilla-max-level")
                .and_then(toml::Value::as_bool)
                .unwrap_or(defaults.bypass),
            custom_limit: values
                .get("has-custom-max-level")
                .and_then(toml::Value::as_bool)
                .unwrap_or(defaults.custom_limit),
            limit: values
                .get("custom-max-level")
                .and_then(toml::Value::as_integer)
                .filter(|value| (0..=i64::from(i32::MAX)).contains(value))
                .map_or(defaults.limit, |value| value as u32),
        }
    }

    fn combine(self, first: u32, second: u32, vanilla_limit: u32) -> u32 {
        let higher = first.max(second);
        if !self.bypass && higher >= vanilla_limit {
            return vanilla_limit;
        }
        let next = higher + u32::from(first == second);
        if self.custom_limit {
            next.min(self.limit)
        } else {
            next
        }
    }
}

fn allowed_levels(
    host: &dyn Host,
    item: &ItemStack,
    rules: Rules,
) -> Option<BTreeMap<String, u32>> {
    levels(host, item, STORED)?
        .iter()
        .map(|(id, value)| {
            let id = id.to_string().ok()?;
            let amount = u32::try_from(level(value)?).ok()?;
            let vanilla_limit = host.enchantment_max_level(&id)?;
            let limit = if rules.bypass {
                rules.limit
            } else {
                vanilla_limit
            };
            (amount <= limit).then_some((id, amount))
        })
        .collect()
}

pub(crate) fn next(host: &dyn Host, inputs: &[Option<ItemStack>], rules: Rules) -> Option<Plan> {
    let [Some(first), Some(second)] = inputs else {
        return None;
    };
    let book_key = host.item_key(&Identifier::parse(ENCHANTED_BOOK.material()).ok()?)?;
    if first.item() != book_key || second.item() != book_key {
        return None;
    }

    let first_levels = allowed_levels(host, first, rules)?;
    let second_levels = allowed_levels(host, second, rules)?;
    let mut combined = first_levels.clone();
    // CraftMetaEnchantedBook reads native books through EnchantmentMap, which
    // orders entries by the full NamespacedKey string.
    for (id, &amount) in &second_levels {
        if combined
            .keys()
            .any(|existing| existing != id && !host.enchantments_compatible(id, existing))
        {
            // Paper stops considering the second book's entries after its first conflict.
            break;
        }
        let amount = match combined.get(id) {
            Some(&previous) => rules.combine(previous, amount, host.enchantment_max_level(id)?),
            None => amount,
        };
        combined.insert(id.clone(), amount);
    }
    let work_ticks = 50u32.checked_mul(combined.len().try_into().ok()?)?;
    combined.retain(|_, amount| *amount > 0);
    if combined.is_empty() || combined == first_levels || combined == second_levels {
        return None;
    }
    // Paper compares ItemMeta before NMS converts it. The mutable native
    // component then clamps unsafe levels to 255 as it writes the item.
    for amount in combined.values_mut() {
        *amount = (*amount).min(255);
    }

    let mut output = ENCHANTED_BOOK.stack(host, 1)?;
    output
        .apply_components(&Compound::from([(
            STORED.into(),
            Tag::compound(
                combined
                    .iter()
                    .map(|(id, &amount)| (id.as_str(), Tag::Int(amount as i32))),
            ),
        )]))
        .ok()?;
    Some(Plan {
        consumed: vec![(0, first.clone(), 1), (1, second.clone(), 1)],
        outputs: vec![output],
        work_ticks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    const SHARPNESS: &str = "minecraft:sharpness";
    const SMITE: &str = "minecraft:smite";
    const MENDING: &str = "minecraft:mending";
    const UNBREAKING: &str = "minecraft:unbreaking";

    fn fixture() -> FakeHost {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:enchanted_book").unwrap()])
            .unwrap();
        host.registries
            .register_enchantment(SHARPNESS, 5)
            .register_enchantment(SMITE, 5)
            .register_enchantment(MENDING, 1)
            .register_enchantment(UNBREAKING, 3)
            .conflict_enchantments(SHARPNESS, SMITE);
        host
    }

    fn book(host: &dyn Host, entries: &[(&str, i32)], count: u32) -> ItemStack {
        let mut book = ENCHANTED_BOOK.stack(host, count).unwrap();
        book.apply_components(&Compound::from([(
            STORED.into(),
            Tag::compound(entries.iter().map(|&(id, amount)| (id, Tag::Int(amount)))),
        )]))
        .unwrap();
        book
    }

    #[test]
    fn binding_observes_paper_conflict_order_and_leaves_unchanged_books() {
        let host = fixture();
        let first = book(&host, &[(SHARPNESS, 3)], 1);
        let second = book(&host, &[(UNBREAKING, 1), (SMITE, 3), (MENDING, 1)], 1);
        let inputs = [Some(first.clone()), Some(second.clone())];
        let plan = next(&host, &inputs, Rules::default()).unwrap();
        assert_eq!(
            plan.outputs,
            [book(&host, &[(SHARPNESS, 3), (MENDING, 1)], 1)]
        );
        assert_eq!(plan.consumed, [(0, first.clone(), 1), (1, second, 1)]);
        assert_eq!(plan.work_ticks, 100);

        let duplicate = [Some(first.clone()), Some(first)];
        assert_eq!(
            next(&host, &duplicate, Rules::default()).unwrap().outputs,
            [book(&host, &[(SHARPNESS, 4)], 1)]
        );
        let at_limit = book(&host, &[(SHARPNESS, 5)], 1);
        assert!(
            next(
                &host,
                &[Some(at_limit.clone()), Some(at_limit)],
                Rules::default()
            )
            .is_none()
        );
    }

    #[test]
    fn binding_observes_configured_and_native_level_limits() {
        let host = fixture();
        let book_at_7 = book(&host, &[(SHARPNESS, 7)], 1);
        let inputs = [Some(book_at_7.clone()), Some(book_at_7)];
        assert!(next(&host, &inputs, Rules::default()).is_none());
        let configuration = "
            [book-binder]
            bypass-vanilla-max-level = true
            has-custom-max-level = true
            custom-max-level = 8
        "
        .parse::<toml::Table>()
        .unwrap();
        let rules = Rules::read_config(&configuration);
        let plan = next(&host, &inputs, rules).unwrap();
        assert_eq!(plan.outputs, [book(&host, &[(SHARPNESS, 8)], 1)]);
        let at_limit = book(&host, &[(SHARPNESS, 8)], 1);
        assert!(next(&host, &[Some(at_limit.clone()), Some(at_limit)], rules).is_none());

        let bypass = Rules {
            limit: 255,
            ..rules
        };
        let maximum = book(&host, &[(SHARPNESS, 255)], 1);
        assert!(
            next(
                &host,
                &[Some(maximum.clone()), Some(maximum.clone())],
                bypass
            )
            .is_none()
        );
        let unlimited = Rules {
            custom_limit: false,
            ..bypass
        };
        let plan = next(
            &host,
            &[Some(maximum.clone()), Some(maximum.clone())],
            unlimited,
        )
        .unwrap();
        assert_eq!(plan.outputs, [maximum]);
    }
}
