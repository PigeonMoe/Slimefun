// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    ComponentId, Host, HostTransaction, Identifier, ItemPlace, ItemResource, ItemStack, PlayerRef,
    ServerHost, SoundSource, Tag, Text,
};

use crate::{items::text, machine::Dispenser, network_runtime::ignition_chamber_at};

pub(crate) fn lore() -> Tag {
    Tag::List(vec![
        Tag::String("".into()),
        text("Prevents the Smeltery from using up fire.", "white", false),
        text("Just fill it up with \"Flint and Steel\"", "white", false),
        text(
            "and place it adjacent to the Smeltery's dispenser",
            "white",
            false,
        ),
    ])
}

pub(crate) fn retain_fire(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    dispenser: Dispenser,
) -> bool {
    let level = dispenser.level;
    let Some(position) =
        neighbors(dispenser.position).find(|&position| ignition_chamber_at(host, level, position))
    else {
        return false;
    };
    let Some(holder) = host
        .block_entity(level, position)
        .filter(|holder| holder.lock(host).is_none())
    else {
        return false;
    };
    let material = Identifier::parse("minecraft:flint_and_steel").unwrap();
    let Some(flint) = host.item_key(&material) else {
        return false;
    };
    let tool = (0..9).find_map(|slot| {
        let place = ItemPlace::BlockEntity {
            level,
            pos: position,
            side: None,
            slot,
        };
        let stack = place.stack(host).filter(|stack| stack.item() == flint)?;
        Some((ItemPlace::Strict(Box::new(place)), stack))
    });
    let Some((place, original)) = tool else {
        let _ = player.tell(
            host,
            Text::literal("Ignition Chamber is missing Flint and Steel."),
        );
        return false;
    };
    if !level.allow_inventory_transfer(host, position, dispenser.position, &original)
        || host.block_entity(level, position) != Some(holder)
        || place.stack(host).as_ref() != Some(&original)
    {
        return false;
    }
    let Some(maximum) = host.max_damage(flint).filter(|&maximum| maximum > 0) else {
        return false;
    };
    let replacement = after_use(host, &original, maximum);
    if replacement.as_ref() != Some(&original)
        && host
            .transaction(|host, tx| {
                let count = original.count();
                if place.extract(host, tx, &ItemResource::of(&original), count) != count {
                    return Err(());
                }
                if let Some(replacement) = &replacement
                    && place.insert(host, tx, &ItemResource::of(replacement), count) != count
                {
                    return Err(());
                }
                Ok(())
            })
            .is_err()
    {
        return false;
    }
    let sound = dispenser.position.map(|axis| f64::from(axis) + 0.5);
    for _ in 0..(1 + u32::from(replacement.is_none())) {
        let _ = level.play_sound(
            host,
            "minecraft:entity.item.break",
            sound,
            SoundSource::Blocks,
            1.0,
            1.0,
        );
    }
    true
}

fn neighbors([x, y, z]: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    [
        z.checked_sub(1).map(|z| [x, y, z]),
        x.checked_add(1).map(|x| [x, y, z]),
        z.checked_add(1).map(|z| [x, y, z]),
        x.checked_sub(1).map(|x| [x, y, z]),
    ]
    .into_iter()
    .flatten()
}

fn after_use(host: &dyn Host, flint: &ItemStack, maximum: u32) -> Option<ItemStack> {
    let unbreakable = ComponentId::new("minecraft:unbreakable").unwrap();
    if ItemResource::of(flint).value(host, &unbreakable).is_some() {
        return Some(flint.clone());
    }
    let damage = flint.damage().saturating_add(1);
    if damage >= maximum {
        return None;
    }
    let mut flint = flint.clone();
    flint.set_damage(damage);
    Some(flint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Compound, prelude::ItemsHost, testing::FakeHost};

    #[test]
    fn the_paper_chamber_searches_north_east_south_then_west() {
        assert_eq!(
            neighbors([4, 9, 2]).collect::<Vec<_>>(),
            [[4, 9, 1], [5, 9, 2], [4, 9, 3], [3, 9, 2]]
        );
    }

    #[test]
    fn flint_takes_one_damage_and_breaks_at_the_material_limit() {
        let mut host = FakeHost::new();
        let material = Identifier::parse("minecraft:flint_and_steel").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let mut tool = ItemStack::new(host.item_key(&material).unwrap(), 1).unwrap();
        tool.apply_components(&Compound::from([(
            "minecraft:repair_cost".into(),
            Tag::Int(4),
        )]))
        .unwrap();
        tool.set_damage(62);
        let worn = after_use(&host, &tool, 64).unwrap();
        assert_eq!(worn.damage(), 63);
        assert_eq!(tool.components(), worn.components());
        assert_eq!(after_use(&host, &worn, 64), None);

        let mut unbreakable = worn;
        unbreakable
            .apply_components(&Compound::from([(
                "minecraft:unbreakable".into(),
                Tag::Compound(Compound::new()),
            )]))
            .unwrap();
        assert_eq!(after_use(&host, &unbreakable, 64), Some(unbreakable));
    }
}
