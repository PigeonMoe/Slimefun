// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{LazyLock, RwLock},
};

use mod_api::{ItemStack, PlayerRef, ServerHost, Text};

use crate::{
    items::{Kind, item_id},
    ticker_policy::Tickers,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Denied {
    Item,
    World,
}

impl Denied {
    pub(crate) fn tell(self, host: &mut dyn ServerHost, player: PlayerRef) {
        let message = match self {
            Self::Item => "This item has been disabled.",
            Self::World => "This item has been disabled in this world.",
        };
        let _ = player.tell(host, Text::literal(message));
    }
}

#[derive(Debug, Default)]
struct Rules {
    disabled_items: BTreeSet<String>,
    worlds: BTreeMap<String, World>,
}

#[derive(Debug)]
struct World {
    enabled: bool,
    stock_enabled: bool,
    disabled_items: BTreeSet<String>,
}

static RULES: LazyLock<RwLock<Rules>> = LazyLock::new(|| RwLock::new(Rules::default()));

impl Rules {
    fn read(table: &toml::Table) -> Self {
        let disabled_items = table
            .get("item-settings")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
            .filter(|(id, value)| {
                id.as_str() != "FILLED_FLASK_OF_KNOWLEDGE"
                    && value.get("enabled").and_then(toml::Value::as_bool) == Some(false)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let worlds = table
            .get("worlds")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
            .filter_map(|(dimension, value)| {
                let table = value.as_table()?;
                let enabled = table
                    .get("enabled")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true);
                let stock_enabled = table
                    .get("slimefun-enabled")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true);
                let disabled_items = table
                    .get("items")
                    .and_then(toml::Value::as_table)
                    .into_iter()
                    .flatten()
                    .filter(|(_, value)| value.as_bool() == Some(false))
                    .map(|(id, _)| id.clone())
                    .collect();
                Some((
                    dimension.clone(),
                    World {
                        enabled,
                        stock_enabled,
                        disabled_items,
                    },
                ))
            })
            .collect();
        Self {
            disabled_items,
            worlds,
        }
    }

    fn world_enabled(&self, dimension: &str) -> bool {
        self.worlds.get(dimension).is_none_or(|world| world.enabled)
    }

    fn world_allows(&self, dimension: &str, id: &str) -> bool {
        self.worlds.get(dimension).is_none_or(|world| {
            world.enabled && world.stock_enabled && !world.disabled_items.contains(id)
        })
    }

    fn use_denial(&self, tickers: Tickers, dimension: &str, id: &str) -> Option<Denied> {
        if !tickers.allows(id) || self.disabled_items.contains(id) {
            Some(Denied::Item)
        } else if !self.world_allows(dimension, id) {
            Some(Denied::World)
        } else {
            None
        }
    }
}

pub(crate) fn configure(table: &toml::Table) {
    *RULES.write().unwrap() = Rules::read(table);
}

/// Explicit global item state excludes this item from Paper's enabled registry.
/// URID has a separate early-return path, and dimensional policies never remove
/// an existing ticker ID from Paper's global registry.
pub(crate) fn globally_enabled(id: &str) -> bool {
    !RULES.read().unwrap().disabled_items.contains(id)
}

pub(crate) fn world_enabled(dimension: &str) -> bool {
    RULES.read().unwrap().world_enabled(dimension)
}

pub(crate) fn has_use_restrictions(dimension: &str) -> bool {
    let rules = RULES.read().unwrap();
    !Tickers::current().enabled()
        || !rules.disabled_items.is_empty()
        || rules.worlds.get(dimension).is_some_and(|world| {
            !world.enabled || !world.stock_enabled || !world.disabled_items.is_empty()
        })
}

pub(crate) fn visible(tickers: Tickers, dimension: Option<&str>, id: &str) -> bool {
    let rules = RULES.read().unwrap();
    tickers.allows(id)
        && !rules.disabled_items.contains(id)
        && dimension.is_none_or(|dimension| rules.world_allows(dimension, id))
}

pub(crate) fn use_denial(dimension: &str, id: &str) -> Option<Denied> {
    RULES
        .read()
        .unwrap()
        .use_denial(Tickers::current(), dimension, id)
}

pub(crate) fn stack_denial(dimension: &str, stack: &ItemStack) -> Option<Denied> {
    use_paper_item_denial(dimension, &item_id(stack)?)
}

pub(crate) fn use_paper_item_denial(dimension: &str, id: &str) -> Option<Denied> {
    // VanillaItem outputs have no Slimefun PDC. A Paper fallback stays vanilla
    // even if an old or externally supplied copy carries a Slimefun identity.
    if vanilla_fallback(id) && !globally_enabled(id) {
        return None;
    }
    use_denial(dimension, id)
}

fn vanilla_fallback(id: &str) -> bool {
    matches!(
        id,
        "CHAIN_HELMET"
            | "CHAIN_CHESTPLATE"
            | "CHAIN_LEGGINGS"
            | "CHAIN_BOOTS"
            | "ENCHANTED_GOLDEN_APPLE"
            | "ELYTRA"
            | "TRIDENT"
            | "TOTEM_OF_UNDYING"
    )
}

pub(crate) fn paper_id(kind: Kind) -> &'static str {
    match kind {
        Kind::Vanilla("minecraft:enchanted_golden_apple") => "ENCHANTED_GOLDEN_APPLE",
        Kind::Vanilla("minecraft:elytra") => "ELYTRA",
        Kind::Vanilla("minecraft:trident") => "TRIDENT",
        Kind::Vanilla("minecraft:totem_of_undying") => "TOTEM_OF_UNDYING",
        _ => crate::armor_forge::paper_id(kind),
    }
}

pub(crate) fn recipe_registered(tickers: Tickers, output: Kind) -> bool {
    tickers.allows(output.id()) && globally_enabled(paper_id(output))
}

pub(crate) fn output_denial(dimension: &str, output: Kind) -> Option<Denied> {
    match output {
        Kind::Slimefun(_) => use_denial(dimension, output.id()),
        Kind::Vanilla(_) | Kind::Potion(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_item_world_and_ticker_policies_are_independent() {
        let rules = Rules::read(
            &r#"
            [item-settings.CARGO_MANAGER]
            enabled = false
            [item-settings.CARGO_NODE_OUTPUT]
            enabled = true
            [item-settings.FILLED_FLASK_OF_KNOWLEDGE]
            enabled = false
            [worlds."minecraft:overworld".items]
            CARGO_NODE_OUTPUT = false
            [worlds."minecraft:the_end"]
            enabled = false
            [worlds."custom:stock_disabled"]
            slimefun-enabled = false
        "#
            .parse()
            .unwrap(),
        );
        let enabled = Tickers::default();
        let overworld = "minecraft:overworld";
        let nether = "minecraft:the_nether";
        assert_eq!(
            rules.use_denial(enabled, overworld, "CARGO_MANAGER"),
            Some(Denied::Item)
        );
        assert_eq!(
            rules.use_denial(enabled, overworld, "CARGO_NODE_OUTPUT"),
            Some(Denied::World)
        );
        assert_eq!(rules.use_denial(enabled, nether, "CARGO_NODE_OUTPUT"), None);
        assert!(rules.world_enabled(overworld));
        assert!(!rules.world_enabled("minecraft:the_end"));
        assert!(rules.world_enabled(nether));
        assert!(rules.world_enabled("custom:stock_disabled"));
        assert_eq!(
            rules.use_denial(enabled, "custom:stock_disabled", "CARGO_NODE"),
            Some(Denied::World)
        );
        assert!(!rules.disabled_items.contains("FILLED_FLASK_OF_KNOWLEDGE"));
        assert!(rules.world_allows(overworld, "CARGO_NODE"));
        assert_eq!(
            rules.use_denial(Tickers::disabled(), nether, "ENERGY_REGULATOR"),
            Some(Denied::Item)
        );
        assert_eq!(
            rules.use_denial(Tickers::disabled(), nether, "SOLAR_GENERATOR"),
            None
        );
    }

    #[test]
    fn unbranded_vanilla_recipe_results_resolve_to_paper_owners() {
        for (vanilla, paper) in [
            ("minecraft:chainmail_helmet", "CHAIN_HELMET"),
            ("minecraft:chainmail_boots", "CHAIN_BOOTS"),
            ("minecraft:elytra", "ELYTRA"),
            ("minecraft:enchanted_golden_apple", "ENCHANTED_GOLDEN_APPLE"),
        ] {
            assert_eq!(paper_id(Kind::Vanilla(vanilla)), paper);
            assert!(vanilla_fallback(paper));
        }
        assert!(!vanilla_fallback(paper_id(Kind::Vanilla(
            "minecraft:chain"
        ))));
    }
}
