// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockPart, BlockUpdateFlags, CanDropFromExplosionEvent, Compound, Identifier, LevelHost,
    LevelRef, OnBlockExplodedEvent, OnMobBlockBreakEvent, Registrar, ServerHost,
};

use crate::{
    block_placer, crucible, food_machines, hologram_projector,
    network_runtime::{self, NetworkBreak},
    output_chest, rainbow, spawner_repair,
};

const CHEST: &str = "minecraft:chest";
const DISPENSER: &str = "minecraft:dispenser";
const DROPPER: &str = "minecraft:dropper";
const FURNACE: &str = "minecraft:furnace";
const HOPPER: &str = "minecraft:hopper";
const HEAD: &str = "minecraft:player_head";
const WALL_HEAD: &str = "minecraft:player_wall_head";
const SOLAR: &str = "minecraft:daylight_detector";
const CARRIERS: [&str; 9] = [
    CHEST,
    DISPENSER,
    DROPPER,
    FURNACE,
    HOPPER,
    HEAD,
    WALL_HEAD,
    SOLAR,
    spawner_repair::CARRIER,
];
const CHUNK_NETWORK_CARRIERS: [&str; 5] = [
    "minecraft:gold_block",
    "minecraft:iron_block",
    "minecraft:stone_pressure_plate",
    crate::gps_transport::PYLON_CARRIERS[0],
    crate::gps_transport::PYLON_CARRIERS[1],
];

pub(crate) fn register(registrar: &mut dyn Registrar) {
    for &carrier in CARRIERS
        .iter()
        .chain(crucible::CARRIERS)
        .chain(&CHUNK_NETWORK_CARRIERS)
        .chain(&[hologram_projector::CARRIER])
    {
        let block = registrar.block_key(carrier);
        registrar.block_part(block, Destruction);
        if matches!(carrier, HEAD | WALL_HEAD | hologram_projector::CARRIER) {
            registrar.block_part(block, MarkedBreakEffects);
        }
    }
    for carrier in rainbow::carriers() {
        if !CHUNK_NETWORK_CARRIERS.contains(&carrier.as_str()) {
            let block = registrar.block_key(&carrier);
            registrar.block_part(block, Destruction);
        }
    }
}

fn marked(carrier: &str, read: impl Fn(&str, &str) -> Option<Compound>) -> bool {
    match carrier {
        CHEST => output_chest::has_saved_marker(read),
        DISPENSER if block_placer::has_saved_marker(&read) => true,
        DISPENSER | DROPPER | FURNACE | HOPPER => {
            network_runtime::stock_inventory_marker(carrier, read)
        }
        HEAD | WALL_HEAD if food_machines::has_saved_marker(&read) => true,
        HEAD | WALL_HEAD | SOLAR => network_runtime::network_break_marker(carrier, read).is_some(),
        spawner_repair::CARRIER => spawner_repair::has_saved_marker(read),
        _ => false,
    }
}

fn plain_network_carrier(carrier: &str) -> bool {
    crucible::carrier(carrier) || CHUNK_NETWORK_CARRIERS.contains(&carrier)
}

struct Destruction;

#[mod_api::horizon_part]
impl BlockPart for Destruction {
    fn can_drop_from_explosion(
        &self,
        level: &dyn LevelHost,
        event: &mut CanDropFromExplosionEvent<'_>,
    ) -> Option<bool> {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let carrier = level.state_block(event.state)?;
        let dimension = event.explosion.dimension;
        if carrier == hologram_projector::CARRIER {
            return hologram_projector::saved_marker(level, dimension, pos).then_some(false);
        }
        if plain_network_carrier(carrier) {
            let id = network_runtime::saved_factory_identity(level, dimension, pos, carrier);
            if network_runtime::plain_break_without_shell(carrier, id.as_deref()) {
                return Some(false);
            }
        }
        if rainbow::is_carrier(carrier) {
            return rainbow::placed::saved_marker(level, dimension, pos, carrier).then_some(false);
        }
        // Paper removes these marks from the blast list, blocking shell drops.
        // Vanilla containers still spill their contents on replacement.
        level
            .block_entity_at(pos)
            .is_some_and(|entity| {
                marked(carrier, |namespace, name| {
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

struct MarkedBreakEffects;

#[mod_api::horizon_part]
impl BlockPart for MarkedBreakEffects {
    fn on_block_exploded(&self, host: &mut dyn ServerHost, event: &mut OnBlockExplodedEvent<'_>) {
        // Capture and retire any virtual state while the live marker is readable.
        if let Some(level) = host.level(event.explosion.dimension) {
            break_without_shell(host, level, [event.pos.x, event.pos.y, event.pos.z]);
        }
    }
}

/// Processes a marked block in an already admitted blast or Wither break.
/// Paper's setType(AIR) keeps the ordinary block entity removal side effects.
pub(crate) fn break_without_shell(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> bool {
    let Some(carrier) = level.block(host, pos).map(|block| block.id(host)) else {
        return false;
    };
    let projector = carrier == hologram_projector::CARRIER;
    let (filters, remove_status) = if projector {
        if !hologram_projector::marked(host, level, pos) {
            return false;
        }
        (Vec::new(), false)
    } else if plain_network_carrier(&carrier) || rainbow::is_carrier(&carrier) {
        let plain = plain_network_carrier(&carrier) && {
            let id = network_runtime::placed_id(host, level, pos);
            network_runtime::plain_break_without_shell(&carrier, id.as_deref())
        };
        if !plain && !(level.in_place(host) && rainbow::placed::marked(host, level, pos)) {
            return false;
        }
        (Vec::new(), false)
    } else {
        if !CARRIERS.contains(&carrier.as_str()) {
            return false;
        }
        let Some(entity) = host.block_entity(level, pos) else {
            return false;
        };
        let read = |namespace: &str, name: &str| {
            host.block_entity_saved_attachment(entity, namespace, name)
        };
        match carrier.as_str() {
            HEAD | WALL_HEAD if food_machines::has_saved_marker(&read) => (Vec::new(), false),
            HEAD | WALL_HEAD | SOLAR => match network_runtime::network_break_marker(&carrier, read)
            {
                Some(NetworkBreak::CargoFilters(data)) => (
                    network_runtime::cargo_filter_items(&data, |name| {
                        host.item_key(&Identifier::parse(name).ok()?)
                    }),
                    false,
                ),
                Some(NetworkBreak::RemoveStatus) => (Vec::new(), true),
                Some(NetworkBreak::NoItems) => (Vec::new(), false),
                None => return false,
            },
            _ if marked(&carrier, read) => (Vec::new(), false),
            _ => return false,
        }
    };
    let Some(air) = host.block_state_id("minecraft:air") else {
        return false;
    };
    let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
    if level.set_block(host, pos, air, flags) != Ok(true) {
        return false;
    }
    if remove_status {
        network_runtime::remove_network_status(host, level, pos);
    }
    if projector {
        hologram_projector::remove_projection(host, level, pos);
    }
    for stack in filters {
        let [x, y, z] = crate::random::natural_drop_position(host, level, pos);
        let _ = level.spawn_item(host, x, y, z, stack, 10);
    }
    true
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
    fn paper_plain_stock_uses_its_exact_carrier_without_selecting_special_handlers() {
        let cases = [
            ("minecraft:gold_block", "GOLD_24K_BLOCK"),
            ("minecraft:iron_block", "GPS_TELEPORTATION_MATRIX"),
            (
                "minecraft:stone_pressure_plate",
                "GPS_ACTIVATION_DEVICE_SHARED",
            ),
            (
                "minecraft:stone_pressure_plate",
                "GPS_ACTIVATION_DEVICE_PERSONAL",
            ),
            ("minecraft:stone_pressure_plate", "ELEVATOR_PLATE"),
            ("minecraft:purple_stained_glass", "GPS_TELEPORTER_PYLON"),
            ("minecraft:cyan_stained_glass", "GPS_TELEPORTER_PYLON"),
        ];
        for &(carrier, _) in &cases {
            assert!(!network_runtime::plain_break_without_shell(carrier, None));
            for &(_, id) in &cases {
                let matches = cases.contains(&(carrier, id));
                assert_eq!(
                    network_runtime::plain_break_without_shell(carrier, Some(id)),
                    matches
                );
            }
        }
        assert!(!network_runtime::plain_break_without_shell(
            "minecraft:purple_stained_glass",
            Some("WITHER_PROOF_GLASS"),
        ));
        assert!(!network_runtime::plain_break_without_shell(
            "minecraft:iron_block",
            Some("IRON_GOLEM_ASSEMBLER"),
        ));
    }

    #[test]
    fn paper_food_and_spawner_markers_do_not_cover_ordinary_carriers() {
        for product in food_machines::PRODUCE {
            for id in [product.food.id(), product.fertilizer.id()] {
                let attachment = Compound::from([("id".into(), Tag::String(id.into()))]);
                for carrier in CARRIERS {
                    assert_eq!(
                        matches_marker(carrier, ("placed_produce", attachment.clone())),
                        matches!(carrier, HEAD | WALL_HEAD),
                        "{id} on {carrier}",
                    );
                }
            }
        }
        for (flag, expected) in [
            (Tag::Byte(1), true),
            (Tag::Byte(0), false),
            (Tag::Int(1), false),
        ] {
            let attachment = Compound::from([("repaired".into(), flag)]);
            assert_eq!(
                matches_marker(spawner_repair::CARRIER, ("reinforced_spawner", attachment)),
                expected
            );
        }
    }

    #[test]
    fn ingredient_decorative_and_fixed_gps_heads_use_only_marked_head_carriers() {
        for id in [
            "CHEESE",
            "BUTTER",
            "POWER_CRYSTAL",
            "STONE_CHUNK",
            "SYNTHETIC_SAPPHIRE",
            "CARBONADO",
            "RAW_CARBONADO",
            "GPS_GEO_SCANNER",
            "GPS_CONTROL_PANEL",
            "GPS_EMERGENCY_TRANSMITTER",
        ] {
            for carrier in CARRIERS {
                assert_eq!(
                    matches_marker(carrier, network_marker(id)),
                    matches!(carrier, HEAD | WALL_HEAD),
                    "{id} on {carrier}"
                );
            }
        }
    }

    #[test]
    fn only_matching_chunk_records_protect_rainbow_and_projector_shells() {
        struct Saved {
            name: &'static str,
            entry: Tag,
        }
        impl LevelHost for Saved {
            fn chunk_saved_attachment(
                &self,
                chunk: [i32; 2],
                namespace: &str,
                name: &str,
            ) -> Option<Compound> {
                (chunk == [3, -2] && namespace == "slimefun" && name == self.name)
                    .then(|| Compound::from([("49,80,-17".into(), self.entry.clone())]))
            }
        }
        let pos = [49, 80, -17];
        let dimension = "minecraft:overworld";
        let rainbow = Saved {
            name: "rainbow_blocks",
            entry: Tag::String("RAINBOW_WOOL_XMAS".into()),
        };
        assert!(rainbow::placed::saved_marker(
            &rainbow,
            dimension,
            pos,
            "minecraft:green_wool"
        ));
        assert!(!rainbow::placed::saved_marker(
            &rainbow,
            dimension,
            pos,
            "minecraft:blue_wool"
        ));
        assert!(!rainbow::placed::saved_marker(
            &rainbow,
            dimension,
            pos,
            "minecraft:green_concrete"
        ));
        assert!(!rainbow::placed::saved_marker(
            &rainbow,
            dimension,
            [50, 80, -17],
            "minecraft:green_wool"
        ));
        assert!(!hologram_projector::saved_marker(&rainbow, dimension, pos));
        let mut projector = Saved {
            name: "hologram_projectors",
            entry: Tag::compound([
                (
                    "owner",
                    Tag::String("00000000000000000000000000000001".into()),
                ),
                ("generation", Tag::Long(1)),
                ("text", Tag::String("Projected text".into())),
                ("offset", Tag::Long(50)),
            ]),
        };
        assert!(hologram_projector::saved_marker(&projector, dimension, pos));
        assert!(!hologram_projector::saved_marker(
            &projector,
            dimension,
            [50, 80, -17]
        ));
        projector.entry = Tag::String("malformed".into());
        assert!(!hologram_projector::saved_marker(
            &projector, dimension, pos
        ));
    }

    #[test]
    fn only_marked_manual_devices_suppress_cauldron_shells() {
        for &carrier in crucible::CARRIERS {
            assert!(network_runtime::plain_break_without_shell(
                carrier,
                Some("CRUCIBLE")
            ));
            assert!(network_runtime::plain_break_without_shell(
                carrier,
                Some("COMPOSTER")
            ));
            assert!(!network_runtime::plain_break_without_shell(carrier, None));
            assert!(!network_runtime::plain_break_without_shell(
                carrier,
                Some("ELECTRIC_CRUCIBLE")
            ));
        }
        for carrier in CARRIERS {
            assert!(!network_runtime::plain_break_without_shell(
                carrier,
                Some("CRUCIBLE")
            ));
            assert!(!network_runtime::plain_break_without_shell(
                carrier,
                Some("COMPOSTER")
            ));
        }
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
            (WALL_HEAD, network_marker("CARGO_NODE_INPUT")),
            (WALL_HEAD, network_marker("CARGO_NODE_OUTPUT_ADVANCED")),
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
            for id in ["GPS_TRANSMITTER", "ANCIENT_PEDESTAL", "WITHER_PROOF_GLASS"] {
                assert!(!matches_marker(carrier, network_marker(id)));
            }
        }
        assert!(!matches_marker(HEAD, network_marker("CARGO_NODE_INPUT")));
        assert!(!matches_marker(HEAD, network_marker("INDUSTRIAL_MINER")));
        assert!(!matches_marker(WALL_HEAD, network_marker("CRAFTING_MOTOR")));
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

    #[test]
    fn shell_only_network_devices_never_use_virtual_filters() {
        let heads = &[HEAD, WALL_HEAD];
        let cases: &[(&str, &[&str], bool)] = &[
            ("CARGO_MANAGER", heads, true),
            ("ENERGY_REGULATOR", heads, true),
            ("VANILLA_AUTO_CRAFTER", heads, false),
            ("ENHANCED_AUTO_CRAFTER", heads, false),
            ("ARMOR_AUTO_CRAFTER", heads, false),
            ("TRASH_CAN_BLOCK", heads, false),
            ("CARGO_NODE", heads, false),
            ("CARGO_NODE_OUTPUT", &[WALL_HEAD], false),
            ("ENERGY_CONNECTOR", heads, false),
            ("SMALL_CAPACITOR", heads, false),
            ("MEDIUM_CAPACITOR", heads, false),
            ("BIG_CAPACITOR", heads, false),
            ("LARGE_CAPACITOR", heads, false),
            ("CARBONADO_EDGED_CAPACITOR", heads, false),
            ("ENERGIZED_CAPACITOR", heads, false),
            ("SOLAR_GENERATOR", &[SOLAR], false),
            ("SOLAR_GENERATOR_2", &[SOLAR], false),
            ("SOLAR_GENERATOR_3", &[SOLAR], false),
            ("SOLAR_GENERATOR_4", &[SOLAR], false),
        ];
        for &(id, expected, remove_status) in cases {
            let (marker, data) = network_marker(id);
            let mut saved = data;
            let stack = Compound::from([
                ("id".into(), Tag::String("minecraft:paper".into())),
                ("count".into(), Tag::Int(1)),
            ]);
            let cargo = Compound::from([
                ("channel".into(), Tag::Int(0)),
                (
                    "filters".into(),
                    Tag::Compound(Compound::from([("19".into(), Tag::Compound(stack))])),
                ),
            ]);
            saved.insert("cargo".into(), Tag::Compound(cargo));
            for carrier in CARRIERS {
                let kind = network_runtime::network_break_marker(carrier, |_, name| {
                    (name == marker).then(|| saved.clone())
                });
                if expected.contains(&carrier) {
                    assert!(
                        matches!(
                            (kind, remove_status),
                            (Some(NetworkBreak::RemoveStatus), true)
                                | (Some(NetworkBreak::NoItems), false)
                        ),
                        "{id} at {carrier}"
                    );
                } else {
                    assert!(kind.is_none(), "{id} at {carrier}");
                }
            }
        }
    }
}
