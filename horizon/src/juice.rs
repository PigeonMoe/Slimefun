// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    ActiveEffect, Flow, ItemStack, LivingEntityUseItemFinishEvent, LivingEntityUseItemStartEvent,
    ServerHost, Text,
};

use crate::items::{GOLDEN_APPLE_JUICE, Kind};
use crate::recipes::{GOLDEN_POTION, JUICER, JUICING};
use crate::research;

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

fn identify(host: &dyn ServerHost, stack: &ItemStack) -> Option<Kind> {
    let id = crate::items::item_id(stack)?;
    JUICING
        .iter()
        .map(|recipe| recipe.output.kind)
        .find(|kind| kind.id() == id && kind.matches(host, stack))
}

#[cfg(test)]
mod tests {
    use mod_api::{Compound, Identifier, Tag, testing::FakeHost};

    use super::*;

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
