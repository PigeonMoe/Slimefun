// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, ItemStack, MenuReplace, PlayerRef, ServerHost, SoundSource, Text};

use crate::{items::item_id, recipes::Research, research};

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
    if stack.item().id(host) != "minecraft:player_head"
        || item_id(stack).as_deref() != Some("ENDER_BACKPACK")
    {
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
