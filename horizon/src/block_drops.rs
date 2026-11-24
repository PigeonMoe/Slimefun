// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{BlockDropsEvent, ItemDrop, ItemStack, LevelHost};

use crate::{
    ancient_altar, android_machine, block_placer, factory, food_machines, hologram_projector,
    items::Kind, network_runtime, output_chest, rainbow,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shell {
    /// Replace a prepared vanilla drop using its motion and position.
    Existing,
    /// Also produce the marked item when there is no prepared vanilla drop.
    Create,
    /// Leave a creative break's drop list alone except for menu contents.
    Omit,
    /// Omit the marked item and suppress the matching vanilla carrier.
    Remove,
}

impl Shell {
    pub(crate) fn restore(
        self,
        level: &dyn LevelHost,
        event: &mut BlockDropsEvent<'_>,
        kind: Kind,
    ) {
        if self == Self::Omit {
            return;
        }
        let Some(key) = level.item_key_by_name(kind.material()) else {
            return;
        };
        if self == Self::Remove {
            event.drops.retain(|drop| drop.stack.item() != key);
        } else if let Some(drop) = event.drops.iter_mut().find(|drop| drop.stack.item() == key) {
            kind.decorate(&mut drop.stack);
        } else if self == Self::Create
            && let Some(mut stack) = ItemStack::new(key, 1)
        {
            kind.decorate(&mut stack);
            add_stack(event, stack);
        }
    }
}

pub(crate) fn drop_block_creative(options: Option<&toml::Value>) -> bool {
    options
        .and_then(|section| section.get("drop-block-creative"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
}

pub(crate) fn add_stack(event: &mut BlockDropsEvent<'_>, stack: ItemStack) {
    let position = [event.pos.x, event.pos.y, event.pos.z].map(|axis| f64::from(axis) + 0.5);
    event
        .drops
        .push(ItemDrop::new(stack, position, [0.0, 0.2, 0.0], 10));
}

pub(crate) fn restore_drops(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>, shell: Shell) {
    block_placer::restore_drop(level, event, shell);
    output_chest::restore_drop(level, event, shell);
    network_runtime::restore_drop(level, event, shell);
    // Paper has machine and device break handlers spill their real inventory
    // directly, outside the list gated by options.drop-block-creative.
    factory::restore_drops(level, event);
    android_machine::drop_items(level, event);
    food_machines::restore_drop(level, event, shell);
    ancient_altar::restore_drops(level, event, shell);
    if shell != Shell::Omit {
        rainbow::placed::drop_item(level, event, shell);
        hologram_projector::restore_drop(level, event, shell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        BlockEntityRef, BlockState, BlockStateRecord, Compound, CreativeBlockDropsEvent, Flow,
        Hooks, Identifier, ItemKey, LevelDimension, Tag, testing::FakeHost,
    };

    use crate::{Slimefun, items::ELECTRIC_FURNACE};

    struct Furnace {
        entity: BlockEntityRef,
        furnace: ItemKey,
        ingot: ItemKey,
        marked: bool,
    }

    impl LevelHost for Furnace {
        fn state_block(&self, _: BlockState) -> Option<&str> {
            Some("minecraft:furnace")
        }

        fn item_key_by_name(&self, id: &str) -> Option<ItemKey> {
            match id {
                "minecraft:furnace" => Some(self.furnace),
                "minecraft:iron_ingot" => Some(self.ingot),
                _ => None,
            }
        }

        fn block_entity_saved_attachment(
            &self,
            entity: BlockEntityRef,
            namespace: &str,
            name: &str,
        ) -> Option<Compound> {
            if entity != self.entity || namespace != "slimefun" || !self.marked {
                return None;
            }
            match name {
                "network_state" => Some(Compound::from([(
                    "id".into(),
                    Tag::String("ELECTRIC_FURNACE".into()),
                )])),
                "electric_furnace" => Some(Compound::from([(
                    "24".into(),
                    Tag::compound([
                        ("id", Tag::String("minecraft:iron_ingot".into())),
                        ("count", Tag::Int(3)),
                    ]),
                )])),
                _ => None,
            }
        }
    }

    #[test]
    fn creative_drop_option_controls_marked_shell_but_preserves_machine_contents() {
        let mut host = FakeHost::new();
        let ids =
            ["minecraft:furnace", "minecraft:iron_ingot"].map(|id| Identifier::parse(id).unwrap());
        host.registries.resolve_items(ids.clone()).unwrap();
        let [furnace, ingot] =
            ids.map(|id| (&host as &dyn mod_api::ServerHost).item_key(&id).unwrap());
        let pos = mod_api::BlockPos::new(1, 64, 2);
        let entity = mod_api::__private::game::block_entity_ref(host.level.level(), [1, 64, 2], 1);
        let block = mod_api::__private::parts_installation::block_key(0);
        let state = BlockStateRecord::new(block, 0).into();
        let plugin = Slimefun::default();
        assert!(plugin.settings.lock().unwrap().drop_block_creative);
        for (options, marked, expected) in [
            (
                "",
                true,
                vec![
                    ELECTRIC_FURNACE.stack(&host, 1).unwrap(),
                    ItemStack::new(ingot, 3).unwrap(),
                ],
            ),
            (
                "[options]\ndrop-block-creative = false",
                true,
                vec![ItemStack::new(ingot, 3).unwrap()],
            ),
            ("[options]\ndrop-block-creative = true", false, vec![]),
        ] {
            let table: toml::Table = options.parse().unwrap();
            plugin.settings.lock().unwrap().drop_block_creative =
                drop_block_creative(table.get("options"));
            let mut drops = Vec::new();
            let level = Furnace {
                entity,
                furnace,
                ingot,
                marked,
            };
            let mut event = CreativeBlockDropsEvent {
                dimension: LevelDimension::Overworld,
                pos,
                state,
                breaker: Some(3),
                breaker_entity: None,
                block_entity: Some(entity),
                tool: None,
                drops: &mut drops,
                experience: 0,
                cancelled: false,
            };
            assert!(matches!(
                plugin.creative_block_drops(&level, &mut event),
                Flow::Continue
            ));
            assert_eq!(
                drops.into_iter().map(|drop| drop.stack).collect::<Vec<_>>(),
                expected
            );
        }

        plugin.settings.lock().unwrap().drop_block_creative = false;
        let level = Furnace {
            entity,
            furnace,
            ingot,
            marked: true,
        };
        let prepared = ItemDrop::new(
            ItemStack::new(furnace, 1).unwrap(),
            [1.2, 64.2, 2.3],
            [0.1, 0.2, 0.3],
            9,
        );
        let mut drops = vec![prepared.clone()];
        let mut event = BlockDropsEvent {
            dimension: LevelDimension::Overworld,
            pos,
            state,
            breaker: Some(3),
            breaker_entity: None,
            block_entity: Some(entity),
            tool: None,
            drops: &mut drops,
            experience: 0,
            cancelled: false,
        };
        assert!(matches!(
            plugin.block_drops(&level, &mut event),
            Flow::Continue
        ));
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].stack, ELECTRIC_FURNACE.stack(&host, 1).unwrap());
        assert_eq!(drops[0].position, prepared.position);
        assert_eq!(drops[0].velocity, prepared.velocity);
        assert_eq!(drops[0].pickup_delay, prepared.pickup_delay);
        assert_eq!(drops[1].stack, ItemStack::new(ingot, 3).unwrap());
    }
}
