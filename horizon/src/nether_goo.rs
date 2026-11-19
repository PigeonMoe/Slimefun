// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::HashSet,
    sync::{
        LazyLock,
        atomic::{AtomicU32, Ordering},
    },
};

use mod_api::{
    ActiveEffect, Compound, EntityInteractEvent, GameMode, Host, InteractionResult, ItemStack,
    PlayerRef, RightClickBlockEvent, ServerHost, Tag, Text,
};

use crate::{
    fuel_generators::STRANGE_NETHER_GOO,
    hologram_projector::{colored_label, colored_lore},
    item_clicks,
    items::item_id,
    random::Rolls,
    recipes::Research,
    research,
};

pub(crate) const RESEARCH: Research = Research {
    id: "villager_rune",
    name: "Reset Villager Trades",
    cost: 26,
};

pub(crate) const DEFAULT_CONFIG: &str = "\n[strange-nether-goo]\nbarter-chance = 7\n";

pub(crate) struct NetherGoo(AtomicU32);

impl Default for NetherGoo {
    fn default() -> Self {
        Self(AtomicU32::new(7))
    }
}

impl NetherGoo {
    pub(crate) fn configure(&self, table: Option<&toml::Table>) {
        let chance = table
            .and_then(|table| table.get("barter-chance"))
            .and_then(toml::Value::as_integer)
            .and_then(|chance| u32::try_from(chance).ok())
            .filter(|chance| *chance <= 100)
            .unwrap_or(7);
        self.0.store(chance, Ordering::Relaxed);
    }

    pub(crate) fn replace_piglin_drop(
        &self,
        host: &dyn Host,
        piglin: u32,
        rolls: &Rolls,
    ) -> Option<ItemStack> {
        let chance = self.0.load(Ordering::Relaxed);
        if !(1..100).contains(&chance) {
            return None;
        }
        let level = host.entity(piglin)?.level(host)?;
        let roll = rolls.next(host, level, 100)?;
        rolled(chance, roll)
            .then(|| STRANGE_NETHER_GOO.stack(host, 1))
            .flatten()
    }
}

fn rolled(chance: u32, roll: u32) -> bool {
    (1..100).contains(&chance) && roll < chance
}

pub(crate) fn is_stock_item(stack: &ItemStack) -> bool {
    static STOCK_IDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        include_str!("guide/stock.tsv")
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .flat_map(|(_, ids)| ids.split_ascii_whitespace())
            .collect()
    });
    item_id(stack).is_some_and(|id| STOCK_IDS.contains(id.as_str()))
}

pub(crate) fn stop_sign_use(
    host: &dyn ServerHost,
    player: PlayerRef,
    event: &RightClickBlockEvent<'_>,
) -> bool {
    if !event
        .click
        .stack
        .is_some_and(|stack| STRANGE_NETHER_GOO.matches(host, stack))
    {
        return false;
    }
    let Some(block) = player
        .level(host)
        .and_then(|level| level.block(host, event.click.pos))
    else {
        return false;
    };
    host.block_in_tag(block, "minecraft:signs") == Some(true)
}

pub(crate) fn interact(
    host: &mut dyn ServerHost,
    event: &EntityInteractEvent<'_>,
) -> Option<InteractionResult> {
    let stack = event.stack?;
    let player = host.players().acting_player()?;
    if event.kind == "minecraft:piglin"
        && stack.item().id(host) == "minecraft:gold_ingot"
        && is_stock_item(stack)
    {
        let _ = player.tell(
            host,
            colored_label("&cYou cannot barter with Slimefun gold."),
        );
        return Some(InteractionResult::Consume);
    }
    if event.kind != "minecraft:sheep" || !STRANGE_NETHER_GOO.matches(host, stack) {
        return None;
    }
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Reset Villager Trades with /sf research villager_rune."),
        );
        return Some(InteractionResult::Consume);
    }
    let target = host.entity(u32::try_from(event.entity).ok()?)?;
    if target
        .nbt(host)
        .is_some_and(|data| data.contains_key(&"CustomName".into()))
    {
        return Some(InteractionResult::Consume);
    }
    let sheep = host.mob(target)?;
    if player.game_mode(host) != Some(GameMode::Creative)
        && !item_clicks::consume_one(host, player, event.hand, stack)
    {
        return Some(InteractionResult::Consume);
    }
    let _ = sheep.set_sheep_wool_color(host, 10);
    let _ = target.set_custom_name(host, Some(colored_label("&5Tainted Sheep")));
    if let Some(effect) = host.effect_key("minecraft:poison")
        && let Some(sheep) = host.living(target)
    {
        let _ = sheep.add_effect(host, &ActiveEffect::new(effect, 60, 2, false, true, true));
    }
    Some(InteractionResult::Consume)
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == STRANGE_NETHER_GOO.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(
                [
                    "",
                    "&fA strange bio matter that",
                    "&fcan be acquired from",
                    "&fbartering with Piglins",
                ]
                .into_iter()
                .map(colored_lore)
                .collect(),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_gold_is_distinct_from_unknown_metadata_and_goo_fuels_the_bio_reactor() {
        use crate::{
            fuel_generators::generator,
            items::{GOLD_4K, Kind},
        };
        use mod_api::{Identifier, testing::FakeHost};

        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:gold_ingot", "minecraft:purple_dye"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let mut fake = Kind::Vanilla("minecraft:gold_ingot")
            .stack(&host, 1)
            .unwrap();
        assert!(!is_stock_item(&fake));
        fake.apply_components(&Compound::from([(
            "minecraft:custom_data".into(),
            Tag::compound([(
                "PublicBukkitValues",
                Tag::compound([("slimefun:slimefun_item", Tag::String("UNKNOWN_GOLD".into()))]),
            )]),
        )]))
        .unwrap();
        assert!(!is_stock_item(&fake));
        assert!(is_stock_item(&GOLD_4K.stack(&host, 1).unwrap()));

        let goo = STRANGE_NETHER_GOO.stack(&host, 1).unwrap();
        let bio = generator("BIO_REACTOR").unwrap();
        let fuel = bio.fuel(&host, &goo).unwrap();
        assert_eq!(fuel.ticks, 32);
        assert!(!fuel.returns_bucket);
    }

    #[test]
    fn piglins_keep_vanilla_outcomes_when_the_paper_chance_is_outside_one_to_ninety_nine() {
        for roll in 0..100 {
            assert!(!rolled(0, roll));
            assert_eq!(rolled(7, roll), roll < 7);
            assert_eq!(rolled(99, roll), roll < 99);
            assert!(!rolled(100, roll));
        }
    }
}
