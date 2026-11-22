// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Compound, GameMode, GameRuleValue, Hand, Host, HostTransaction, ItemPlace, ItemStack, LevelRef,
    PlayerRef, ServerHost, SoundSource, Tag,
};

use crate::{
    enchantment_rune::ANCIENT_RUNE_LIGHTNING,
    elemental_staves::{STAFF_ELEMENTAL_WATER, STAFF_ELEMENTAL_WIND},
    hologram_projector::{colored_label, colored_lore},
    items::{ENDER_LUMP_3, Kind, item, item_id},
    random::natural_drop_position,
    recipes::{CraftingRecipe, Research, Stack},
    research,
    snacks::MAGIC_SUGAR,
};

item!(
    STAFF_ELEMENTAL_STORM,
    "stick",
    "Elemental Staff - Storm",
    "gold",
    None,
    None
);

pub(super) const RESEARCH: Research = Research {
    id: "storm_staff",
    name: "Storm Staff",
    cost: 30,
};
const LIGHTNING: Option<Kind> = Some(ANCIENT_RUNE_LIGHTNING);
const ENDER: Option<Kind> = Some(ENDER_LUMP_3);

pub(crate) const ALTAR: CraftingRecipe = CraftingRecipe {
    inputs: [
        LIGHTNING,
        ENDER,
        LIGHTNING,
        Some(STAFF_ELEMENTAL_WATER),
        Some(MAGIC_SUGAR),
        Some(STAFF_ELEMENTAL_WIND),
        LIGHTNING,
        ENDER,
        LIGHTNING,
    ],
    output: Stack {
        kind: STAFF_ELEMENTAL_STORM,
        count: 1,
    },
    research: &RESEARCH,
};

const PDC: &str = "slimefun:stormstaff_usage";
const MAX_USES: i32 = 8;

fn uses_line(count: i32, chinese: bool) -> Tag {
    if chinese {
        return colored_lore(&format!("&7剩余可用 &e{count} &7次"));
    }
    let unit = if count > 1 { "Uses" } else { "Use" };
    colored_lore(&format!("&e{count} {unit} &7left"))
}

pub(crate) fn localize_uses(line: &str) -> Option<Tag> {
    let Some((count, suffix)) = line.strip_prefix("§e")?.split_once(' ') else {
        return None;
    };
    if !matches!(suffix, "Use §7left" | "Uses §7left") {
        return None;
    }
    Some(uses_line(count.parse().ok()?, true))
}

fn is_local_uses(line: &str) -> bool {
    line.strip_prefix("§7剩余可用 §e")
        .and_then(|line| line.strip_suffix(" §7次"))
        .is_some_and(|count| count.parse::<i32>().is_ok())
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id != STAFF_ELEMENTAL_STORM.id() {
        return;
    }
    components.insert(
        "minecraft:custom_name".into(),
        colored_lore("&6Elemental Staff &7- &8&oStorm"),
    );
    components.insert(
        "minecraft:lore".into(),
        Tag::List(vec![
            colored_lore(""),
            colored_lore("&7Element: &8&oStorm"),
            colored_lore(""),
            colored_lore("&eRight Click&7 to summon a lightning"),
            uses_line(MAX_USES, false),
        ]),
    );
    components.insert(
        "minecraft:enchantments".into(),
        Tag::compound([("minecraft:unbreaking", Tag::Int(1))]),
    );
}

fn remaining(stack: &ItemStack) -> i32 {
    match stack
        .components()
        .and_then(|patch| patch.as_nbt().get(&"minecraft:custom_data".into()))
        .and_then(|custom| custom.get("PublicBukkitValues"))
        .and_then(|values| values.get(PDC))
    {
        Some(Tag::Int(uses)) => *uses,
        _ => MAX_USES,
    }
}

fn used_staff(mut stack: ItemStack, uses: i32) -> Option<ItemStack> {
    let patch = stack.components().map(|patch| patch.as_nbt());
    let mut custom = match patch.and_then(|patch| patch.get(&"minecraft:custom_data".into())) {
        Some(Tag::Compound(custom)) => custom.clone(),
        None => Compound::new(),
        Some(_) => return None,
    };
    let Tag::Compound(values) = custom
        .entry("PublicBukkitValues".into())
        .or_insert_with(|| Tag::Compound(Compound::new()))
    else {
        return None;
    };
    values.insert(PDC.into(), Tag::Int(uses));
    let mut lore = match patch.and_then(|patch| patch.get(&"minecraft:lore".into())) {
        Some(Tag::List(lore)) => lore.clone(),
        None => Vec::new(),
        Some(_) => return None,
    };
    if lore.is_empty() {
        lore.push(uses_line(uses, true));
    } else if let Some((line, chinese)) = lore.iter_mut().find_map(|line| {
        let text = crate::network_runtime::paper_text(line)?;
        if is_local_uses(&text) {
            Some((line, true))
        } else {
            localize_uses(&text).map(|_| (line, false))
        }
    }) {
        *line = uses_line(uses, chinese);
    }
    stack
        .apply_components(&Compound::from([
            ("minecraft:custom_data".into(), Tag::Compound(custom)),
            ("minecraft:lore".into(), Tag::List(lore)),
        ]))
        .ok()?;
    Some(stack)
}

fn damage(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    hand: Hand,
    stack: &ItemStack,
) -> Option<()> {
    let place = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    };
    if place.stack(host).as_ref() != Some(stack) {
        return None;
    }
    let slot = match hand {
        Hand::Main => player.selected_slot(host)? as u32,
        Hand::Off => 40,
    };
    let uses = remaining(stack);
    let split = stack.clone().split(1);
    let worn = if uses == 1 {
        None
    } else {
        Some(used_staff(split.taken?, uses.wrapping_sub(1))?)
    };
    let before = player.inventory_slots(host)?;
    let away = player.entity().position(host)?;
    let maximum = match stack
        .components()
        .and_then(|patch| patch.as_nbt().get(&"minecraft:max_stack_size".into()))
    {
        Some(Tag::Int(size)) => u32::try_from(*size).ok()?,
        _ => host.max_stack_size(stack.item())?,
    };
    let overflow = host
        .transaction(|host, _| {
            if split.remainder.is_none() {
                player.set_inventory_slot(host, slot, worn)?;
                return Ok::<_, mod_api::PlayerDataRefusal>(None);
            }
            player.set_inventory_slot(host, slot, split.remainder)?;
            let Some(mut worn) = worn else {
                return Ok(None);
            };
            // Bukkit's storage inventory merges into its first compatible partial stack,
            // then puts the separate worn item in its first empty slot.
            if let Some((slot, existing)) = before
                .iter()
                .filter(|(cell, existing)| {
                    *cell < 36 && existing.count() < maximum && existing.is_stackable_with(&worn)
                })
                .min_by_key(|(cell, _)| *cell)
            {
                worn.set_count(existing.count() + 1)
                    .expect("one valid additional item");
                player.set_inventory_slot(host, *slot, Some(worn))?;
                return Ok(None);
            }
            if let Some(slot) = (0..36).find(|cell| before.iter().all(|(used, _)| used != cell)) {
                player.set_inventory_slot(host, slot, Some(worn))?;
                return Ok(None);
            }
            Ok(Some(worn))
        })
        .ok()?;
    if let Some(left) = overflow {
        let [x, y, z] = natural_drop_position(host, level, away);
        let _ = level.spawn_item(host, x, y, z, left, 10);
    }
    if uses == 1 {
        let _ = player.play_sound(
            host,
            "minecraft:entity.item.break",
            SoundSource::Players,
            1.0,
            1.0,
        );
    }
    Some(())
}

fn targeted_block(host: &dyn Host, player: PlayerRef, level: LevelRef) -> Option<[i32; 3]> {
    let entity = player.entity();
    let (bottom, height) = level.height(host)?;
    let mut target = None;
    let [yaw, pitch] = entity.rotation(host)?;
    for pos in crate::seismic_axe::sight_cells(
        entity.eye_position(host)?,
        crate::seismic_axe::direction([yaw, pitch]),
        30.0,
    ) {
        target = Some(pos);
        if pos[1] < bottom || pos[1] >= bottom + height {
            continue;
        }
        let block = level.block(host, pos)?.id(host);
        if !matches!(
            block.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        ) {
            break;
        }
    }
    target.filter(|&pos| level.is_loaded(host, pos))
}

/// A recognized staff returns whether Paper also canceled the vanilla click.
pub(crate) fn use_held(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
) -> Option<bool> {
    let material = stack.item().id(host);
    let shears = material == "minecraft:shears"
        && item_id(stack).as_deref() == Some(STAFF_ELEMENTAL_STORM.id());
    if !shears && !STAFF_ELEMENTAL_STORM.matches(host, stack) {
        return None;
    }
    let mode = player.game_mode(host)?;
    if mode == GameMode::Spectator {
        return Some(false);
    }
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(host, colored_label("&c请先研究雷之法杖。"));
        return Some(true);
    }
    let food = player.food(host)?.level;
    if food < 4 && mode != GameMode::Creative {
        let _ = player.tell(host, colored_label("&c你太饿了, 先吃点东西再试试吧!"));
        return Some(false);
    }
    let level = player.level(host)?;
    let Some(target) = targeted_block(host, player, level) else {
        return Some(false);
    };
    if !matches!(
        level.game_rule(host, "minecraft:pvp"),
        Ok(Some(GameRuleValue::Bool(true)))
    ) {
        let _ = player.tell(host, colored_label("&c你不能在这里攻击其他玩家!"));
        return Some(false);
    }
    // The Bukkit strike is naturally attributed even for a player who cast it.
    let _ = level.strike_lightning(host, target.map(f64::from), false);
    if !shears {
        if mode != GameMode::Creative {
            let _ = player.set_food_level(host, food - 4);
        }
        damage(host, player, level, hand, stack);
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn paper_storm_usage_survives_stack_updates_with_localized_and_legacy_lore() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:stick").unwrap()])
            .unwrap();
        let fresh = STAFF_ELEMENTAL_STORM.stack(&host, 1).unwrap();
        assert_eq!(remaining(&fresh), 8);
        let mut imported = fresh.clone();
        imported
            .apply_components(&Compound::from([
                (
                    "minecraft:lore".into(),
                    Tag::List(vec![colored_lore("&bPrevious lore"), uses_line(2, false)]),
                ),
                (
                    "minecraft:custom_data".into(),
                    Tag::compound([
                        ("Witness", Tag::Int(100)),
                        (
                            "PublicBukkitValues",
                            Tag::compound([
                                (PDC, Tag::Int(2)),
                                (
                                    "slimefun:slimefun_item",
                                    Tag::String(STAFF_ELEMENTAL_STORM.id().into()),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]))
            .unwrap();
        assert_eq!(remaining(&imported), 2);
        let used = used_staff(imported, 1).unwrap();
        assert_eq!(remaining(&used), 1);

        let lore = |stack: &ItemStack| {
            let patch = stack.components().unwrap().as_nbt();
            let Tag::List(lines) = patch.get(&"minecraft:lore".into()).unwrap() else {
                panic!("expected a list");
            };
            lines
                .iter()
                .filter_map(crate::network_runtime::paper_text)
                .collect::<Vec<_>>()
        };
        assert!(
            lore(&used)
                .iter()
                .any(|line| line.contains("§e1 Use §7left"))
        );
        assert!(
            lore(&used)
                .iter()
                .any(|line| line.contains("Previous lore"))
        );
        let values = used.components().unwrap().as_nbt();
        assert_eq!(
            values
                .get(&"minecraft:custom_data".into())
                .unwrap()
                .get("Witness"),
            Some(&Tag::Int(100))
        );
        assert!(
            lore(&used_staff(fresh, 7).unwrap())
                .iter()
                .any(|line| line.contains("剩余可用 §e7 §7次"))
        );
    }
}
