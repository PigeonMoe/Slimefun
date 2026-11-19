// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Compound, EquipmentSlot, GameMode, Host, Identifier, ItemStack, LivingRef, PlayerRef,
    ServerHost, SoundSource, Tag, Text,
};

use crate::{
    armor_forge::{ENDER_BOOTS, ENDER_CHESTPLATE, ENDER_HELMET, ENDER_LEGGINGS},
    items::{ENDER_LUMP_2, Kind, item, text},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    MAGIC_EYE_OF_ENDER,
    "ender_eye",
    "Magic Eye of Ender",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "magic_eye_of_ender",
    name: "Magic Eye of Ender",
    cost: 4,
};

const PEARL: Kind = Kind::Vanilla("minecraft:ender_pearl");
const ARMOR: [(EquipmentSlot, Kind); 4] = [
    (EquipmentSlot::Head, ENDER_HELMET),
    (EquipmentSlot::Chest, ENDER_CHESTPLATE),
    (EquipmentSlot::Legs, ENDER_LEGGINGS),
    (EquipmentSlot::Feet, ENDER_BOOTS),
];

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(ENDER_LUMP_2),
        Some(PEARL),
        Some(ENDER_LUMP_2),
        Some(PEARL),
        Some(Kind::Vanilla("minecraft:ender_eye")),
        Some(PEARL),
        Some(ENDER_LUMP_2),
        Some(PEARL),
        Some(ENDER_LUMP_2),
    ],
    output: Stack {
        kind: MAGIC_EYE_OF_ENDER,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn decorate(id: &str, data: &mut Compound) {
    if id != MAGIC_EYE_OF_ENDER.id() {
        return;
    }
    let mut title = text("Magic Eye of Ender", "gold", false);
    if let Tag::Compound(style) = &mut title {
        style.insert("bold".into(), Tag::Byte(1));
    }
    data.insert("minecraft:custom_name".into(), title);
    let mut requirement = text("Requires full Ender Armor", "dark_red", false);
    if let Tag::Compound(style) = &mut requirement {
        style.insert("bold".into(), Tag::Byte(1));
    }
    let instruction = Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        (
            "extra",
            Tag::List(vec![
                text("Right Click", "yellow", false),
                text(" to shoot an Ender Pearl", "gray", false),
            ]),
        ),
    ]);
    data.insert(
        "minecraft:lore".into(),
        Tag::List(vec![requirement, Tag::String("".into()), instruction]),
    );
}

fn has_full_armor(
    host: &dyn Host,
    mut equipped: impl FnMut(EquipmentSlot) -> Option<ItemStack>,
) -> bool {
    ARMOR
        .into_iter()
        .all(|(slot, kind)| equipped(slot).is_some_and(|stack| kind.matches(host, &stack)))
}

pub(crate) fn use_held(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack) -> bool {
    if !MAGIC_EYE_OF_ENDER.matches(host, stack) {
        return false;
    }
    if !matches!(
        player.game_mode(host),
        Some(GameMode::Survival | GameMode::Creative | GameMode::Adventure)
    ) {
        return false;
    }
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Magic Eye of Ender with /sf research magic_eye_of_ender."),
        );
        return true;
    }
    let living = LivingRef::new(player.entity());
    if !has_full_armor(host, |slot| living.equipment(host, slot)) {
        return true;
    }
    let Some(pearl) = PEARL.stack(host, 1) else {
        return true;
    };
    let kind = Identifier::parse(PEARL.id()).expect("vanilla ender pearl identifier");
    // Paper CraftLivingEntity.launchProjectile(EnderPearl) calls shootFromRotation
    // with the player's angles, no pitch offset, power 1.5 and inaccuracy 1.
    let _ = host.spawn_projectile_from_rotation(living, &kind, &pearl, 0.0, 1.5, 1.0);
    let _ = player.play_sound(
        host,
        "minecraft:entity.enderman.teleport",
        SoundSource::Players,
        1.0,
        1.0,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn magic_eye_keeps_the_paper_recipe_and_exact_research() {
        assert_eq!(
            CRAFTING.inputs.map(|kind| kind.unwrap().id()),
            [
                "ENDER_LUMP_2",
                "minecraft:ender_pearl",
                "ENDER_LUMP_2",
                "minecraft:ender_pearl",
                "minecraft:ender_eye",
                "minecraft:ender_pearl",
                "ENDER_LUMP_2",
                "minecraft:ender_pearl",
                "ENDER_LUMP_2",
            ]
        );
        assert_eq!(
            (CRAFTING.research.id, CRAFTING.research.cost),
            ("magic_eye_of_ender", 4)
        );
        assert_eq!(
            (CRAFTING.output.kind, CRAFTING.output.count),
            (MAGIC_EYE_OF_ENDER, 1)
        );
    }

    #[test]
    fn each_equipment_slot_requires_authentic_slimefun_ender_armor() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(ARMOR.map(|(_, kind)| Identifier::parse(kind.material()).unwrap()))
            .unwrap();
        let real = |slot| {
            ARMOR
                .iter()
                .find(|(candidate, _)| *candidate == slot)?
                .1
                .stack(&host, 1)
        };
        assert!(has_full_armor(&host, real));
        for (missing, kind) in ARMOR {
            assert!(!has_full_armor(&host, |slot| if slot == missing {
                None
            } else {
                real(slot)
            }));
            assert!(!has_full_armor(&host, |slot| if slot == missing {
                Kind::Vanilla(kind.material()).stack(&host, 1)
            } else {
                real(slot)
            }));
        }
    }
}
