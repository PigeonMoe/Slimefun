// SPDX-License-Identifier: GPL-3.0-only

use std::sync::OnceLock;

use mod_api::{
    EntityInteractEvent, GameMode, HostTransaction, InteractionResult, ItemPlace, ItemResource,
    ParticleKey, ParticleOptions, Registrar, ServerHost, SoundSource, Tag, Text,
};

use crate::{
    ancient_altar::ANCIENT_RUNE_FIRE,
    availability,
    fuel_generators::STRANGE_NETHER_GOO,
    hologram_projector::colored_lore,
    items::{Kind, MAGIC_LUMP_3, item},
    nether_goo::RESEARCH,
    random::Rolls,
    recipes::{CraftingRecipe, Stack},
    research,
    talismans::MAGICAL_GLASS,
};

item!(
    ANCIENT_RUNE_VILLAGERS,
    "firework_star",
    "Ancient Rune [Villagers]",
    "gray",
    None,
    None
);

const CRYING_OBSIDIAN: Kind = Kind::Vanilla("minecraft:crying_obsidian");

pub(crate) const ALTAR: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(MAGIC_LUMP_3),
        Some(MAGICAL_GLASS),
        Some(CRYING_OBSIDIAN),
        Some(STRANGE_NETHER_GOO),
        Some(ANCIENT_RUNE_FIRE),
        Some(STRANGE_NETHER_GOO),
        Some(CRYING_OBSIDIAN),
        Some(MAGICAL_GLASS),
        Some(MAGIC_LUMP_3),
    ],
    output: Stack {
        kind: ANCIENT_RUNE_VILLAGERS,
        count: 3,
    },
    research: &RESEARCH,
};

const LORE: [&str; 5] = [
    "&eRight click a villager to clear",
    "&etheir current job and trades.",
    "&eThe villager will start looking",
    "&efor a job again after some",
    "&etime has passed.",
];
static PARTICLES: OnceLock<[ParticleKey; 2]> = OnceLock::new();

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = PARTICLES
        .set(["minecraft:crimson_spore", "minecraft:enchant"].map(|id| registrar.particle_key(id)));
}

pub(crate) fn lore() -> Tag {
    Tag::List(LORE.into_iter().map(colored_lore).collect())
}

pub(crate) fn interact(
    host: &mut dyn ServerHost,
    event: &EntityInteractEvent<'_>,
    rolls: &Rolls,
) -> Option<InteractionResult> {
    let stack = event
        .stack
        .filter(|stack| ANCIENT_RUNE_VILLAGERS.matches(host, stack))?;
    let player = host.players().acting_player()?;
    if let Some(denied) = availability::stack_denial(host.dimension(), stack) {
        denied.tell(host, player);
        return Some(InteractionResult::Consume);
    }
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Reset Villager Trades with /sf research villager_rune."),
        );
        return Some(InteractionResult::Consume);
    }
    if event.kind != "minecraft:villager" {
        return None;
    }
    let target = host.entity(u32::try_from(event.entity).ok()?)?;
    let level = target.level(host)?;
    let position = target.position(host)?;
    let merchant = host.merchant(target)?;
    if matches!(
        merchant.profession(host)?.as_str(),
        "minecraft:none" | "minecraft:nitwit"
    ) {
        return None;
    }

    let hand = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: event.hand,
    };
    let resource = ItemResource::of(stack);
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    host.transaction(|host, tx| {
        if !creative && hand.extract(host, tx, &resource, 1) != 1 {
            return Err(());
        }
        merchant.reset_villager_career(host).then_some(()).ok_or(())
    })
    .ok()?;

    let _ = level.play_sound(
        host,
        "minecraft:entity.villager.celebrate",
        position,
        SoundSource::Neutral,
        1.0,
        1.4,
    );
    if let Some(&[spore, enchant]) = PARTICLES.get() {
        const SAMPLES: u32 = 1 << 24;
        let spread_y =
            f64::from(rolls.next(host, level, SAMPLES).unwrap_or(0)) / (4.0 * f64::from(SAMPLES));
        let _ = level.send_particles(
            host,
            &ParticleOptions::simple(spore),
            None,
            position,
            [0.0, spread_y, 0.0],
            0.0,
            10,
        );
        let _ = level.send_particles(
            host,
            &ParticleOptions::simple(enchant),
            None,
            position,
            [0.04, 1.0, 0.04],
            1.0,
            5,
        );
    }
    Some(InteractionResult::Consume)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn villager_rune_keeps_paper_altar_recipe_and_shares_goo_research() {
        let recipe = crate::ancient_altar::CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == ANCIENT_RUNE_VILLAGERS)
            .unwrap();
        assert_eq!(
            recipe.inputs.map(|kind| kind.unwrap().id()),
            [
                "MAGIC_LUMP_3",
                "MAGICAL_GLASS",
                "minecraft:crying_obsidian",
                "STRANGE_NETHER_GOO",
                "ANCIENT_RUNE_FIRE",
                "STRANGE_NETHER_GOO",
                "minecraft:crying_obsidian",
                "MAGICAL_GLASS",
                "MAGIC_LUMP_3",
            ]
        );
        assert_eq!(recipe.output.count, 3);
        assert_eq!(recipe.research.id, RESEARCH.id);
        assert_eq!(recipe.research.cost, 26);
    }
}
