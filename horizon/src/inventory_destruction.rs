// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockPart, BlockUpdateFlags, CanDropFromExplosionEvent, Compound, LevelHost, LevelRef,
    OnMobBlockBreakEvent, Registrar, ServerHost,
};

use crate::{block_placer, network_runtime, output_chest};

const CHEST: &str = "minecraft:chest";
const DISPENSER: &str = "minecraft:dispenser";
const DROPPER: &str = "minecraft:dropper";
const FURNACE: &str = "minecraft:furnace";
const HOPPER: &str = "minecraft:hopper";
const CARRIERS: [&str; 5] = [CHEST, DISPENSER, DROPPER, FURNACE, HOPPER];

pub(crate) fn register(registrar: &mut dyn Registrar) {
    for carrier in CARRIERS {
        let block = registrar.block_key(carrier);
        registrar.block_part(block, Destruction(carrier));
    }
}

fn marked(carrier: &str, read: impl Fn(&str, &str) -> Option<Compound>) -> bool {
    match carrier {
        CHEST => output_chest::has_saved_marker(read),
        DISPENSER if block_placer::has_saved_marker(&read) => true,
        DISPENSER | DROPPER | FURNACE | HOPPER => {
            network_runtime::stock_inventory_marker(carrier, read)
        }
        _ => false,
    }
}

struct Destruction(&'static str);

#[mod_api::horizon_part]
impl BlockPart for Destruction {
    fn can_drop_from_explosion(
        &self,
        level: &dyn LevelHost,
        event: &mut CanDropFromExplosionEvent<'_>,
    ) -> Option<bool> {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        // Paper removes these marks from the blast list, blocking shell drops.
        // Horizon's subsequent vanilla replacement still spills the contents.
        level
            .block_entity_at(pos)
            .is_some_and(|entity| {
                marked(self.0, |namespace, name| {
                    level.block_entity_saved_attachment(entity, namespace, name)
                })
            })
            .then_some(false)
    }

    fn on_mob_block_break(&self, host: &mut dyn ServerHost, event: &mut OnMobBlockBreakEvent<'_>) {
        if matches!(event.kind, "minecraft:wither" | "minecraft:wither_skull")
            && let Some(level) = host.level(host.dimension())
        {
            break_without_shell(host, level, [event.pos.x, event.pos.y, event.pos.z]);
        }
    }
}

/// Processes an already admitted synthetic explosion or direct Wither break.
/// Paper's setType(AIR) keeps the ordinary block entity removal side effects.
pub(crate) fn break_without_shell(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> bool {
    let Some(carrier) = level.block(host, pos).map(|block| block.id(host)) else {
        return false;
    };
    if !CARRIERS.contains(&carrier.as_str())
        || !host.block_entity(level, pos).is_some_and(|entity| {
            marked(&carrier, |namespace, name| {
                host.block_entity_saved_attachment(entity, namespace, name)
            })
        })
    {
        return false;
    }
    let Some(air) = host.block_state_id("minecraft:air") else {
        return false;
    };
    let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
    level.set_block(host, pos, air, flags) == Ok(true)
}

#[cfg(test)]
mod tests {
    use mod_api::Tag;

    use super::*;

    fn network_marker(id: &str) -> (&str, Compound) {
        (
            "network_state",
            Compound::from([("id".into(), Tag::String(id.into()))]),
        )
    }

    fn matches_marker(carrier: &str, marker: (&str, Compound)) -> bool {
        marked(carrier, |namespace, name| {
            (namespace == "slimefun" && name == marker.0).then(|| marker.1.clone())
        })
    }

    #[test]
    fn every_paper_inventory_marker_applies_only_to_its_matching_carrier() {
        let cases = [
            (
                CHEST,
                (
                    "output_chest",
                    Compound::from([("placed".into(), Tag::Byte(1))]),
                ),
            ),
            (DISPENSER, ("block_placer", Compound::new())),
            (DISPENSER, network_marker("ANDROID_INTERFACE_ITEMS")),
            (DISPENSER, network_marker("ANDROID_INTERFACE_FUEL")),
            (DROPPER, network_marker("IGNITION_CHAMBER")),
            (HOPPER, network_marker("INFUSED_HOPPER")),
            (FURNACE, network_marker("ENHANCED_FURNACE")),
            (FURNACE, network_marker("REINFORCED_FURNACE")),
            (FURNACE, network_marker("CARBONADO_EDGED_FURNACE")),
        ];
        for (carrier, marker) in cases {
            for actual in CARRIERS {
                assert_eq!(matches_marker(actual, marker.clone()), actual == carrier);
            }
        }
        for tier in 2..=11 {
            let id = format!("ENHANCED_FURNACE_{tier}");
            assert!(matches_marker(FURNACE, network_marker(&id)), "{id}");
        }
    }

    #[test]
    fn vanilla_and_other_slimefun_blocks_keep_their_drop_paths() {
        for carrier in CARRIERS {
            assert!(!marked(carrier, |_, _| None));
            for id in ["CARGO_MANAGER", "ANCIENT_PEDESTAL", "WITHER_PROOF_GLASS"] {
                assert!(!matches_marker(carrier, network_marker(id)));
            }
        }
        assert!(!matches_marker(
            CHEST,
            (
                "output_chest",
                Compound::from([("placed".into(), Tag::Byte(0))])
            )
        ));
        assert!(!matches_marker(
            DISPENSER,
            (
                "block_placer",
                Compound::from([("owner".into(), Tag::Byte(0))])
            )
        ));
    }
}
