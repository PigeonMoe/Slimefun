// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{CreativeBlockPickEvent, Host, ItemStack, LevelRef, ServerHost};

use crate::{
    ancient_altar, block_placer, food_machines, hologram_projector,
    items::{Kind, OUTPUT_CHEST, item_id},
    network_recipes, network_runtime, output_chest, rainbow, spawner_repair,
};

pub(crate) fn pick(host: &mut dyn ServerHost, event: &CreativeBlockPickEvent) -> Option<ItemStack> {
    let level = event.player.level(host)?;
    let carrier = event.state.block().id(host);
    let kind = placed_kind(host, level, event.pos, &carrier)?;
    let inventory = event.player.inventory_slots(host)?;
    pick_stack(host, kind, inventory)
}

fn placed_kind(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    carrier: &str,
) -> Option<Kind> {
    if !level.in_place(host) {
        return None;
    }
    if let Some(kind) = network_runtime::placed_kind(host, level, pos, carrier) {
        return Some(kind);
    }
    if let Some(kind) = rainbow::placed::placed_id(host, level, pos).and_then(rainbow::item) {
        return Some(kind);
    }
    if carrier == hologram_projector::CARRIER && hologram_projector::marked(host, level, pos) {
        return Some(network_recipes::HOLOGRAM_PROJECTOR);
    }
    let entity = host.block_entity(level, pos)?;
    match carrier {
        "minecraft:dispenser" if block_placer::marked(host, entity) => {
            Some(block_placer::BLOCK_PLACER)
        }
        "minecraft:chest" if output_chest::live_marker(host, entity) => Some(OUTPUT_CHEST),
        "minecraft:spawner" if spawner_repair::live_marker(host, entity) => {
            Some(spawner_repair::REINFORCED_SPAWNER)
        }
        "minecraft:enchanting_table" | "minecraft:dispenser" => {
            ancient_altar::live_kind(host, level, pos)
        }
        "minecraft:player_head" | "minecraft:player_wall_head" => {
            food_machines::live_kind(host, entity)
        }
        _ => None,
    }
}

fn pick_stack(
    host: &dyn Host,
    kind: Kind,
    inventory: impl IntoIterator<Item = (u32, ItemStack)>,
) -> Option<ItemStack> {
    // Paper selects the first hotbar stack with the registered ID. Return
    // that exact stack so the normal pick planner keeps its current components.
    inventory
        .into_iter()
        .filter(|(slot, stack)| *slot < 9 && item_id(stack).as_deref() == Some(kind.id()))
        .min_by_key(|(slot, _)| *slot)
        .map(|(_, stack)| stack)
        .or_else(|| kind.stack(host, 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Compound, Identifier, Tag, testing::FakeHost};

    #[test]
    fn the_first_paper_id_in_the_hotbar_keeps_its_lore_and_count() {
        let mut host = FakeHost::new();
        let kind = OUTPUT_CHEST;
        host.registries
            .resolve_items([Identifier::parse(kind.material()).unwrap()])
            .unwrap();
        let stock = kind.stack(&host, 1).unwrap();
        let mut renamed = kind.stack(&host, 3).unwrap();
        renamed
            .apply_components(&Compound::from([(
                "minecraft:lore".into(),
                Tag::List(vec![Tag::String("原来的说明".into())]),
            )]))
            .unwrap();
        let vanilla = ItemStack::new(stock.item(), 1).unwrap();
        assert_eq!(
            pick_stack(
                &host,
                kind,
                [
                    (8, stock.clone()),
                    (10, stock.clone()),
                    (4, renamed.clone()),
                    (1, vanilla)
                ],
            ),
            Some(renamed)
        );
        assert_eq!(pick_stack(&host, kind, [(10, stock.clone())]), Some(stock));
    }
}
