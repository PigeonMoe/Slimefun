// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    GameMode, Hand, Host, HostTransaction, Identifier, ItemPlace, ItemResource, ItemStack,
    LevelRef, PlayerRef, ServerHost, SoundSource, Text,
};

use crate::compressor::DelayedCrafts;
use crate::gold_pan::PANS;
use crate::machine::{has_block, opposite_blocks};
use crate::output_chest;
use crate::random::Rolls;
use crate::recipes::{AUTOMATED_PANNING_MACHINE, TABLE_SAW};
use crate::research;

pub(crate) enum HandMachine {
    TableSaw,
    AutomatedPan { cauldron: [i32; 3] },
}

impl HandMachine {
    pub(crate) fn at(host: &dyn Host, level: LevelRef, [x, y, z]: [i32; 3]) -> Option<Self> {
        let pos = [x, y, z];
        let below = [x, y.checked_sub(1)?, z];
        let block = level.block_state(host, pos)?.block().id(host);
        if block == "minecraft:stonecutter"
            && has_block(host, level, below, "minecraft:iron_block")
            && opposite_blocks(host, level, pos, &["minecraft:smooth_stone_slab"])
        {
            return Some(Self::TableSaw);
        }
        if (block == "minecraft:oak_trapdoor" || in_tag(host, &block, "minecraft:wooden_trapdoors"))
            && has_block(host, level, below, "minecraft:cauldron")
        {
            return Some(Self::AutomatedPan { cauldron: below });
        }
        None
    }

    pub(crate) fn activate(
        self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        pos: [i32; 3],
        delayed_crafts: &DelayedCrafts,
        rolls: &Rolls,
    ) {
        let required = match self {
            Self::TableSaw => &TABLE_SAW,
            Self::AutomatedPan { .. } => &AUTOMATED_PANNING_MACHINE,
        };
        if !research::can_use(host, player, required) {
            let _ = player.tell(
                host,
                Text::literal(format!(
                    "Unlock this research with /sf research {}.",
                    required.id
                )),
            );
            return;
        }

        let hand = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand: Hand::Main,
        };
        let Some(input) = hand.stack(host) else {
            wrong_item(host, player);
            return;
        };
        let material = input.item().id(host);
        let creative = player.game_mode(host) == Some(GameMode::Creative);
        let consume = |host: &mut dyn ServerHost, tx: &mod_api::Transaction| {
            creative || hand.extract(host, tx, &ItemResource::of(&input), 1) == 1
        };
        let result = match self {
            Self::TableSaw => {
                let Some(output) = saw_result(host, &material) else {
                    wrong_item(host, player);
                    return;
                };
                host.transaction(|host, tx| {
                    if !consume(host, tx)
                        || !output_chest::route_or_drop(host, level, pos, pos, &output)
                    {
                        return Err(());
                    }
                    Ok(())
                })
            }
            Self::AutomatedPan { cauldron } => {
                let Some((pan, sound)) = PANS
                    .iter()
                    .find_map(|pan| pan.target_sound(&material).map(|sound| (pan, sound)))
                else {
                    wrong_item(host, player);
                    return;
                };
                let Some(output) = rolls
                    .next(host, level, 100)
                    .and_then(|roll| pan.drop_for(roll))
                    .and_then(|kind| kind.stack(host, 1))
                else {
                    return;
                };
                host.transaction(|host, tx| {
                    if !consume(host, tx) {
                        return Err(());
                    }
                    delayed_crafts
                        .queue_panning(host, level, pos, cauldron, sound, output)
                        .map_err(|_| ())
                })
            }
        };
        if result.is_err() {
            let _ = player.tell(host, Text::literal("Slimefun could not process this item."));
        } else if matches!(self, Self::TableSaw) {
            let at = pos.map(|axis| f64::from(axis) + 0.5);
            let sound = saw_sound(&material);
            let _ = level.play_sound(host, sound, at, SoundSource::Blocks, 1.0, 1.0);
        }
    }
}

fn wrong_item(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = player.tell(
        host,
        Text::literal("Hold a valid input item in your main hand."),
    );
}

fn in_tag(host: &dyn Host, material: &str, tag: &str) -> bool {
    host.registry_entry("minecraft:block", material)
        .and_then(|entry| host.registry_entry_in_tag(&entry, tag))
        .unwrap_or(false)
}

fn saw_result(host: &dyn Host, material: &str) -> Option<ItemStack> {
    let (name, count) = if in_tag(host, material, "minecraft:logs") {
        (planks_for(material)?, 8)
    } else if in_tag(host, material, "minecraft:planks") {
        ("minecraft:stick".into(), 4)
    } else {
        return None;
    };
    let identifier = Identifier::parse(&name).ok()?;
    ItemStack::new(host.item_key(&identifier)?, count)
}

fn planks_for(material: &str) -> Option<String> {
    let name = material.strip_prefix("minecraft:")?;
    let name = name.strip_prefix("stripped_").unwrap_or(name);
    let (wood, _) = name.rsplit_once('_')?;
    Some(format!("minecraft:{wood}_planks"))
}

fn saw_sound(material: &str) -> &'static str {
    let name = material.strip_prefix("minecraft:").unwrap_or(material);
    let name = name.strip_prefix("stripped_").unwrap_or(name);
    if name.starts_with("bamboo_") {
        "minecraft:block.bamboo_wood.break"
    } else if name.starts_with("crimson_") || name.starts_with("warped_") {
        "minecraft:block.nether_wood.break"
    } else {
        "minecraft:block.wood.break"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_saw_maps_log_names_like_paper() {
        for (input, expected) in [
            ("minecraft:stripped_oak_wood", "minecraft:oak_planks"),
            ("minecraft:cherry_log", "minecraft:cherry_planks"),
            ("minecraft:warped_hyphae", "minecraft:warped_planks"),
            ("minecraft:bamboo_block", "minecraft:bamboo_planks"),
            ("minecraft:pale_oak_log", "minecraft:pale_oak_planks"),
        ] {
            assert_eq!(planks_for(input).as_deref(), Some(expected));
        }
    }
}
