// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Compound, EntityInteractEvent, EntityUuid, GameMode, HostTransaction, InteractionResult,
    ItemPlace, ItemResource, ServerHost, SoundSource, SpawnReason, Tag, Text,
};

use crate::{
    hologram_projector::colored_lore,
    items::{Kind, MAGIC_LUMP_2, item},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    MAGICAL_ZOMBIE_PILLS,
    "nether_wart",
    "Magical Zombie Pills",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "magical_zombie_pills",
    name: "De-Zombification",
    cost: 22,
};

const GOLD: Option<Kind> = Some(Kind::Vanilla("minecraft:gold_ingot"));
const APPLE: Option<Kind> = Some(Kind::Vanilla("minecraft:apple"));

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        GOLD,
        Some(MAGIC_LUMP_2),
        GOLD,
        APPLE,
        Some(Kind::Potion("minecraft:weakness")),
        APPLE,
        GOLD,
        Some(MAGIC_LUMP_2),
        GOLD,
    ],
    output: Stack {
        kind: MAGICAL_ZOMBIE_PILLS,
        count: 2,
    },
    research: &RESEARCH,
};

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == MAGICAL_ZOMBIE_PILLS.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(
                [
                    "",
                    "&eRight Click &7a Zombified Villager",
                    "&eor &7a Zombified Piglin to",
                    "&7instantly cure it from its curse",
                ]
                .into_iter()
                .map(colored_lore)
                .collect(),
            ),
        );
    }
}

pub(crate) fn interact(
    host: &mut dyn ServerHost,
    event: &EntityInteractEvent<'_>,
) -> Option<InteractionResult> {
    let villager = match event.kind {
        "minecraft:zombie_villager" => true,
        "minecraft:zombified_piglin" => false,
        _ => return None,
    };
    let stack = event
        .stack
        .filter(|stack| MAGICAL_ZOMBIE_PILLS.matches(host, stack))?;
    let player = host.players().acting_player()?;
    let target = host.entity(u32::try_from(event.entity).ok()?)?;
    let mob = host.mob(target)?;
    let level = target.level(host)?;
    let position = target.position(host)?;
    let rotation = target.rotation(host)?;
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock De-Zombification with /sf research magical_zombie_pills."),
        );
        return Some(InteractionResult::Consume);
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
        let cured = if villager {
            mob.start_zombie_villager_cure(host, 1, Some(EntityUuid(player.uuid())))
        } else {
            target.discard(host)
        };
        cured.then_some(()).ok_or(())
    })
    .ok()?;
    let _ = player.play_sound(
        host,
        "minecraft:entity.zombie_villager.converted",
        SoundSource::Master,
        1.0,
        1.0,
    );
    // Paper performs the piglin spawn after removing the zombified one, as
    // a fresh spawn. Other plugins can still veto that spawn at the location.
    if !villager
        && let Some(piglin) =
            level.spawn_mob(host, "minecraft:piglin", position, SpawnReason::Command)
    {
        let _ = piglin.set_rotation(host, rotation[0], rotation[1]);
    }
    Some(InteractionResult::Consume)
}

#[cfg(test)]
mod tests {
    use mod_api::{Identifier, testing::FakeHost};

    use super::*;

    #[test]
    fn recipe_keeps_the_unextended_drinkable_weakness_potion() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:potion", "minecraft:splash_potion"]
                    .map(|item| Identifier::parse(item).unwrap()),
            )
            .unwrap();
        let potion = CRAFTING.inputs[4].unwrap();
        let normal = potion.stack(&host, 1).unwrap();
        assert!(potion.matches(&host, &normal));
        assert!(
            !potion.matches(
                &host,
                &Kind::Potion("minecraft:long_weakness")
                    .stack(&host, 1)
                    .unwrap()
            )
        );
        let mut splash = Kind::Vanilla("minecraft:splash_potion")
            .stack(&host, 1)
            .unwrap();
        splash
            .apply_components(normal.components().unwrap().as_nbt())
            .unwrap();
        assert!(!potion.matches(&host, &splash));
        assert_eq!(CRAFTING.output.count, 2);
        assert_eq!(CRAFTING.research.cost, 22);
    }
}
