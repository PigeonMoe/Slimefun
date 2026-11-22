// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ActiveEffect, HostTransaction, PlayerEntityDamagePreEvent, ServerHost};

use super::{
    COMMON_TALISMAN, LUMP_3, Talisman, Talismans, chance, consume_carried, infused, notify, recipe,
};
use crate::{
    items::{GILDED_IRON, REINFORCED_ALLOY_INGOT, item},
    random::Rolls,
    recipes::{CraftingRecipe, Research},
};

item!(
    WARRIOR_TALISMAN,
    "emerald",
    "Talisman of the Warrior",
    "green",
    None,
    None
);
item!(
    ENDER_WARRIOR_TALISMAN,
    "emerald",
    "Ender Talisman of the Warrior",
    "dark_purple",
    None,
    None
);
item!(
    KNIGHT_TALISMAN,
    "emerald",
    "Talisman of the Knight",
    "green",
    None,
    None
);
item!(
    ENDER_KNIGHT_TALISMAN,
    "emerald",
    "Ender Talisman of the Knight",
    "dark_purple",
    None,
    None
);

pub(super) const WARRIOR_RESEARCH: Research = Research {
    id: "warrior_talisman",
    name: "Talisman of the Warrior",
    cost: 20,
};
pub(super) const KNIGHT_RESEARCH: Research = Research {
    id: "knight_talisman",
    name: "Talisman of the Knight",
    cost: 20,
};

pub(super) const CRAFTING: [CraftingRecipe; 4] = [
    recipe(
        WARRIOR_TALISMAN,
        &WARRIOR_RESEARCH,
        4,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(REINFORCED_ALLOY_INGOT),
            Some(COMMON_TALISMAN),
            Some(REINFORCED_ALLOY_INGOT),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_WARRIOR_TALISMAN, WARRIOR_TALISMAN, 4),
    recipe(
        KNIGHT_TALISMAN,
        &KNIGHT_RESEARCH,
        4,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(GILDED_IRON),
            Some(WARRIOR_TALISMAN),
            Some(GILDED_IRON),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_KNIGHT_TALISMAN, KNIGHT_TALISMAN, 4),
];

pub(super) const WARRIOR: Talisman = Talisman {
    normal: WARRIOR_TALISMAN,
    ender: ENDER_WARRIOR_TALISMAN,
    research: &WARRIOR_RESEARCH,
    chance: 100,
    message: "Your Talisman has improved your Strength for a While",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it gives",
        "you Strength III whenever you get hit",
        "but will then be consumed",
    ],
};
pub(super) const KNIGHT: Talisman = Talisman {
    normal: KNIGHT_TALISMAN,
    ender: ENDER_KNIGHT_TALISMAN,
    research: &KNIGHT_RESEARCH,
    chance: 30,
    message: "Your Talisman gave you 5 Seconds of Regeneration",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it gives",
        "you a 30% Chance for 5 Seconds of Regeneration",
        "whenever You get hit",
        "but will then be consumed",
    ],
};

fn is_paper_entity_attack(damage_type: &str, projectile: bool) -> bool {
    // CraftEventFactory defaults direct entities to ENTITY_ATTACK after its
    // explicit causes. Projectile and potion entities have their own causes.
    !projectile
        && !matches!(
            damage_type,
            "minecraft:fireworks"
                | "minecraft:explosion"
                | "minecraft:player_explosion"
                | "minecraft:thorns"
                | "minecraft:sonic_boom"
                | "minecraft:falling_stalactite"
                | "minecraft:falling_block"
                | "minecraft:falling_anvil"
                | "minecraft:lightning_bolt"
                | "minecraft:dragon_breath"
                | "minecraft:magic"
        )
}

pub(crate) fn react(
    host: &mut dyn ServerHost,
    talismans: &Talismans,
    event: &PlayerEntityDamagePreEvent<'_>,
    rolls: &Rolls,
) -> bool {
    if !is_paper_entity_attack(event.source.damage_type, event.projectile) {
        return false;
    }
    let player = event.player;
    let (Some(level), Some(living)) = (player.level(host), host.living(player.entity())) else {
        return false;
    };
    let reactions = [
        (&KNIGHT, "minecraft:regeneration", 100, 3),
        (&WARRIOR, "minecraft:strength", 3600, 2),
    ];
    let mut canceled = false;
    for (talisman, effect, ticks, amplifier) in reactions {
        // One Paper listener calls both talismans in this order even if the Knight
        // already canceled the original hit. Each owns its own consumption/effect.
        if !chance(host, level, talisman, rolls) {
            continue;
        }
        let Some(key) = host.effect_key(effect) else {
            continue;
        };
        if living.effect(host, key).is_some() {
            continue;
        }
        let granted = host.transaction(|host, _| {
            consume_carried(host, player, talisman)?;
            let effect = ActiveEffect::new(key, ticks, amplifier, true, true, true);
            living.add_effect(host, &effect).then_some(()).ok_or(())
        });
        if granted.is_ok() {
            let actionbar = talismans.0.lock().unwrap().actionbar;
            notify(host, player, talisman, actionbar);
            canceled = true;
        }
    }
    canceled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_papers_entity_attack_category_triggers_both_consumable_recipes() {
        assert!(is_paper_entity_attack("minecraft:mob_attack", false));
        assert!(is_paper_entity_attack("minecraft:player_attack", false));
        assert!(is_paper_entity_attack("minecraft:generic", false));
        assert!(!is_paper_entity_attack("minecraft:thrown", true));
        assert!(!is_paper_entity_attack("minecraft:thorns", false));
        assert!(!is_paper_entity_attack("minecraft:sonic_boom", false));
        assert!(!is_paper_entity_attack("minecraft:player_explosion", false));
        assert_eq!(CRAFTING.map(|recipe| recipe.output.count), [4; 4]);
        assert_eq!(CRAFTING[2].inputs[4], Some(WARRIOR_TALISMAN));
        assert_eq!(CRAFTING[3].inputs[4], Some(KNIGHT_TALISMAN));
    }
}
