// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    ActiveEffect, Flow, Host, ItemStack, LivingEntityUseItemFinishEvent,
    LivingEntityUseItemStartEvent, ServerHost, Text,
};

use crate::items::{
    CHRISTMAS_APPLE_CIDER, CHRISTMAS_CHOCOLATE_MILK, CHRISTMAS_EGG_NOG, CHRISTMAS_HOT_CHOCOLATE,
    CHRISTMAS_MILK, GOLDEN_APPLE_JUICE, Kind,
};
use crate::recipes::{GOLDEN_POTION, JUICER, JUICING};
use crate::research;

const CHRISTMAS_DRINKS: &[Kind] = &[
    CHRISTMAS_MILK,
    CHRISTMAS_CHOCOLATE_MILK,
    CHRISTMAS_EGG_NOG,
    CHRISTMAS_APPLE_CIDER,
    CHRISTMAS_HOT_CHOCOLATE,
];

pub(crate) fn start(host: &mut dyn ServerHost, event: &LivingEntityUseItemStartEvent<'_>) -> Flow {
    if event.entity.is_some() {
        return Flow::Continue;
    }
    let Some(juice) = identify(host, event.stack) else {
        return Flow::Continue;
    };
    let Some(player) = host.players().acting_player() else {
        return Flow::Continue;
    };
    if CHRISTMAS_DRINKS.contains(&juice) {
        return Flow::Continue;
    }
    let required = if juice == GOLDEN_APPLE_JUICE {
        &GOLDEN_POTION
    } else {
        &JUICER
    };
    if !research::can_use(host, player, required) {
        let message = format!("Unlock this research with /sf research {}.", required.id);
        let _ = player.tell(host, Text::literal(message));
        return Flow::Cancel;
    }
    Flow::Continue
}

pub(crate) fn finish(host: &mut dyn ServerHost, event: &mut LivingEntityUseItemFinishEvent<'_>) {
    if event.entity.is_some() {
        return;
    }
    let Some(juice) = identify(host, event.stack) else {
        return;
    };
    let Some(player) = host.players().acting_player() else {
        return;
    };
    let Some(profile) = juice.juice() else {
        return;
    };
    // Paper explicitly applies the stored effect in addition to vanilla handling.
    if let Some(living) = host.living(player.entity())
        && let Some(effect) = host.effect_key(profile.effect)
    {
        let active = ActiveEffect::new(effect, profile.duration, 0, true, true, true);
        let _ = living.add_effect(host, &active);
    }
    // Existing Paper juices can still carry the original prototype remainder.
    if event.stack.count() == 1
        && event
            .result
            .as_ref()
            .is_some_and(|stack| Kind::Vanilla("minecraft:glass_bottle").matches(host, stack))
    {
        *event.result = None;
    }
}

pub(crate) fn identify(host: &dyn Host, stack: &ItemStack) -> Option<Kind> {
    let id = crate::items::item_id(stack)?;
    JUICING
        .iter()
        .map(|recipe| recipe.output.kind)
        .chain(CHRISTMAS_DRINKS.iter().copied())
        .find(|kind| kind.id() == id && kind.matches(host, stack))
}

#[cfg(test)]
mod tests {
    use mod_api::{Compound, Identifier, Tag, testing::FakeHost};

    use super::*;

    #[test]
    fn christmas_drinks_keep_paper_effects_and_split_the_crafted_batches() {
        use mod_api::HostTransaction;

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:potion").unwrap()])
            .unwrap();
        host.registries
            .item_component("minecraft:potion", "minecraft:max_stack_size", Tag::Int(1));
        let level = host.level.level();
        host.players.join(1, 2, level);
        host.players.acting = Some(1);
        for (drink, color, effect_ticks, hunger) in [
            (CHRISTMAS_MILK, 0xffffff, 4, "2.5"),
            (CHRISTMAS_CHOCOLATE_MILK, 0x800000, 11, "6.0"),
            (CHRISTMAS_EGG_NOG, 0x808080, 6, "3.5"),
            (CHRISTMAS_APPLE_CIDER, 0xff0000, 13, "7.0"),
            (CHRISTMAS_HOT_CHOCOLATE, 0x800000, 13, "7.0"),
        ] {
            let stack = drink.stack(&host, 1).unwrap();
            assert_eq!(identify(&host, &stack), Some(drink));
            let patch = stack.components().unwrap().as_nbt();
            let potion = patch.get(&"minecraft:potion_contents".into()).unwrap();
            assert_eq!(potion.get("custom_color"), Some(&Tag::Int(color)));
            let Some(Tag::List(effects)) = potion.get("custom_effects") else {
                panic!("the Paper drink has no saturation effect");
            };
            assert_eq!(effects[0].get("duration"), Some(&Tag::Int(effect_ticks)));
            let Some(Tag::List(lines)) = patch.get(&"minecraft:lore".into()) else {
                panic!("the Paper drink has no hunger lore");
            };
            assert!(
                crate::network_runtime::paper_text(&lines[1])
                    .unwrap()
                    .contains(hunger)
            );
            assert!(patch.contains_key(&"!minecraft:use_remainder".into()));
            assert!(matches!(
                start(
                    &mut host,
                    &LivingEntityUseItemStartEvent {
                        stack: &stack,
                        entity: None,
                        hand: mod_api::Hand::Main,
                        duration: 32,
                        cancelled: false,
                    }
                ),
                Flow::Continue
            ));
        }

        for (x, kind, count, inputs) in [
            (
                1,
                CHRISTMAS_MILK,
                4,
                ["minecraft:milk_bucket", "minecraft:glass_bottle"],
            ),
            (
                2,
                CHRISTMAS_CHOCOLATE_MILK,
                2,
                ["CHRISTMAS_MILK", "minecraft:cocoa_beans"],
            ),
            (3, CHRISTMAS_EGG_NOG, 2, ["CHRISTMAS_MILK", "minecraft:egg"]),
            (
                4,
                CHRISTMAS_APPLE_CIDER,
                2,
                ["APPLE_JUICE", "minecraft:sugar"],
            ),
        ] {
            let recipe = crate::recipes::crafting()
                .find(|recipe| recipe.output.kind == kind)
                .unwrap();
            assert_eq!(recipe.output.count, count);
            assert_eq!(
                recipe.inputs[..2]
                    .iter()
                    .map(|kind| kind.unwrap().id())
                    .collect::<Vec<_>>(),
                inputs
            );
            assert!(recipe.inputs[2..].iter().all(Option::is_none));
            assert!(recipe.research.id.is_empty());

            let pos = [x, 64, 1];
            host.level.block_entity(pos).with_items(9);
            let batch = kind.stack(&host, count).unwrap();
            host.transaction(|host, tx| {
                assert_eq!(
                    crate::machine::deposit_into(host, tx, level, pos, 9, &batch, count),
                    0
                );
                Ok::<_, ()>(())
            })
            .unwrap();
            let one = kind.stack(&host, 1).unwrap();
            let slots = &host.level.block_entity(pos).items.as_ref().unwrap().slots;
            assert_eq!(slots.iter().flatten().count() as u32, count);
            assert!(slots.iter().flatten().all(|stack| stack == &one));
        }
    }

    #[test]
    fn archived_eggnog_and_cider_keep_their_ids_and_need_no_research() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:potion").unwrap()])
            .unwrap();
        let level = host.level.level();
        host.players.join(1, 2, level);
        host.players.acting = Some(1);
        for (id, drink, hand) in [
            ("CHRISTMAS_EGG_NOG", CHRISTMAS_EGG_NOG, mod_api::Hand::Main),
            (
                "CHRISTMAS_APPLE_CIDER",
                CHRISTMAS_APPLE_CIDER,
                mod_api::Hand::Off,
            ),
        ] {
            let mut legacy = drink.stack(&host, 1).unwrap();
            legacy
                .set_components(Compound::from([(
                    "minecraft:custom_data".into(),
                    Tag::compound([(
                        "PublicBukkitValues",
                        Tag::compound([(crate::items::ID_KEY, Tag::String(id.into()))]),
                    )]),
                )]))
                .unwrap();
            assert_eq!(identify(&host, &legacy), Some(drink));
            assert_eq!(
                start(
                    &mut host,
                    &LivingEntityUseItemStartEvent {
                        stack: &legacy,
                        entity: None,
                        hand,
                        duration: 32,
                        cancelled: false,
                    }
                ),
                Flow::Continue
            );
        }
    }

    #[test]
    fn juicer_produces_seven_drinkable_paper_potions_without_remainders() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:potion").unwrap()])
            .unwrap();
        assert_eq!(JUICING.len(), 7);
        for (recipe, (input, effect, duration)) in JUICING.iter().zip([
            ("minecraft:apple", "minecraft:saturation", 5),
            ("minecraft:carrot", "minecraft:saturation", 5),
            ("minecraft:melon_slice", "minecraft:saturation", 5),
            ("minecraft:pumpkin", "minecraft:saturation", 5),
            ("minecraft:sweet_berries", "minecraft:saturation", 5),
            ("minecraft:golden_apple", "minecraft:absorption", 400),
            ("minecraft:glow_berries", "minecraft:saturation", 5),
        ]) {
            assert_eq!(recipe.input.kind.id(), input);
            assert_eq!(recipe.input.count, 1);
            assert_eq!(recipe.output.count, 1);
            let stack = recipe.output.kind.stack(&host, 1).unwrap();
            assert_eq!(identify(&host, &stack), Some(recipe.output.kind));
            let components = stack.components().unwrap().as_nbt();
            assert_eq!(
                components.get(&"!minecraft:use_remainder".into()),
                Some(&Tag::Compound(Compound::new()))
            );
            let potion = components.get(&"minecraft:potion_contents".into()).unwrap();
            let Some(Tag::List(effects)) = potion.get("custom_effects") else {
                panic!("juice should preserve its Paper potion effect");
            };
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].get("id"), Some(&Tag::String(effect.into())));
            assert_eq!(effects[0].get("duration"), Some(&Tag::Int(duration)));
        }
    }
}
