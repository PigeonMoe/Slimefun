// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Host, HostSide, InteractionResult, ItemPart, ItemStack, MenuReplace, PlayerRef, Registrar,
    ServerHost, SoundSource, Tag, Text, UseOnEvent,
};

use crate::{
    items::{ENDER_LUMP_2, HeadTexture, Kind, MAGIC_LUMP_3, item, item_id, text},
    recipes::{CraftingRecipe, Research},
    research,
};

pub(super) const SOULBOUND_STORAGE: Research = Research {
    id: "bound_backpack",
    name: "Soulbound Storage",
    cost: 22,
};

pub(super) const ENDER_STORAGE: Research = Research {
    id: "ender_backpack",
    name: "Ender Backpack",
    cost: 4,
};

pub(super) const NECROMANCY: Research = Research {
    id: "essence_of_afterlife",
    name: "Necromancy",
    cost: 19,
};

const ENDER_TEXTURE: HeadTexture = HeadTexture {
    profile: [1878180726, 439170381, -1871712218, -1563387552],
    value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzJhM2IzNDg2MmI5YWZiNjNjZjhkNTc3OTk2NmQzZmJhNzBhZjgyYjA0ZTgzZjNlYWY2NDQ5YWViYSJ9fX0=",
};

item!(
    ENDER_BACKPACK,
    "player_head",
    "Ender Backpack",
    "gold",
    None,
    Some(ENDER_TEXTURE)
);
item!(
    BOUND_BACKPACK,
    "player_head",
    "Soulbound Backpack",
    "red",
    None,
    Some(ENDER_TEXTURE)
);
item!(
    NECROTIC_SKULL,
    "player_head",
    "Necrotic Skull",
    "red",
    None,
    Some(HeadTexture {
        profile: [1965553155, -262258907, -1795193236, 1821522216],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzc5NTNiNmM2ODQ0OGU3ZTZiNmJmOGZiMjczZDcyMDNhY2Q4ZTFiZTE5ZTgxNDgxZWFkNTFmNDVkZTU5YTgifX19",
    })
);
item!(
    ESSENCE_OF_AFTERLIFE,
    "gunpowder",
    "Essence of Afterlife",
    "dark_red",
    None,
    None
);

pub(crate) const MAGIC_CRAFTING: &[CraftingRecipe] = &[
    super::recipe(
        [
            Some(ENDER_LUMP_2),
            super::LEATHER,
            Some(ENDER_LUMP_2),
            super::LEATHER,
            Some(Kind::Vanilla("minecraft:chest")),
            super::LEATHER,
            Some(ENDER_LUMP_2),
            super::LEATHER,
            Some(ENDER_LUMP_2),
        ],
        ENDER_BACKPACK,
        &ENDER_STORAGE,
    ),
    super::recipe(
        [
            Some(MAGIC_LUMP_3),
            None,
            Some(MAGIC_LUMP_3),
            None,
            Some(Kind::Vanilla("minecraft:wither_skeleton_skull")),
            None,
            Some(MAGIC_LUMP_3),
            None,
            Some(MAGIC_LUMP_3),
        ],
        NECROTIC_SKULL,
        &NECROMANCY,
    ),
    super::recipe(
        [
            Some(ENDER_LUMP_2),
            None,
            Some(ENDER_LUMP_2),
            Some(ESSENCE_OF_AFTERLIFE),
            Some(super::WOVEN_BACKPACK),
            Some(ESSENCE_OF_AFTERLIFE),
            Some(ENDER_LUMP_2),
            None,
            Some(ENDER_LUMP_2),
        ],
        BOUND_BACKPACK,
        &SOULBOUND_STORAGE,
    ),
];

pub(super) fn ender_lore() -> Tag {
    Tag::List(vec![
        text("A portable Ender Chest", "green", true),
        Tag::String("".into()),
        super::open_lore(),
    ])
}

pub(super) fn register(registrar: &mut dyn Registrar) {
    let skull = registrar.item_key(NECROTIC_SKULL.material());
    registrar.item_part(skull, NecroticSkull);
}

struct NecroticSkull;

#[mod_api::horizon_part]
impl ItemPart for NecroticSkull {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        NECROTIC_SKULL
            .matches(host, event.stack)
            .then_some(InteractionResult::Fail)
    }
}

pub(crate) fn keep_on_death(host: &dyn Host, stack: &ItemStack) -> bool {
    if stack.item().id(host) != "minecraft:player_head" {
        return false;
    }
    let Some(id) = item_id(stack) else {
        return false;
    };
    id == "BOUND_BACKPACK"
        || ((super::tier(&id).is_some()
            || matches!(
                id.as_str(),
                "COOLER" | "RESTORED_BACKPACK" | "ENDER_BACKPACK"
            ))
            && super::soulbound_marker(stack).is_some())
}

pub(super) fn open_ender(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack) -> bool {
    if !ENDER_BACKPACK.matches(host, stack) {
        return false;
    }
    if !research::can_use(host, player, &ENDER_STORAGE) {
        let _ = player.tell(
            host,
            Text::literal("Unlock this research with /sf research ender_backpack."),
        );
        return true;
    }
    if player
        .open_ender_chest_menu(host, MenuReplace::CloseFirst)
        .is_some()
    {
        let _ = player.play_sound(
            host,
            "minecraft:entity.enderman.teleport",
            SoundSource::Players,
            1.0,
            1.0,
        );
    }
    true
}
