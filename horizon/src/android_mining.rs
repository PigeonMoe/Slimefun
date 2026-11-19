// SPDX-License-Identifier: GPL-3.0-only

use std::sync::OnceLock;

use mod_api::{
    BlockUpdateFlags, HostTransaction, LevelRef, ParticleKey, ParticleOptions, Registrar,
    ServerHost, SoundSource,
};

use crate::{
    android_machine::{self, Position},
    items::Kind,
    network_runtime,
};

const CONNECTED: [[i32; 3]; 4] = [[0, 0, -1], [1, 0, 0], [0, 0, 1], [-1, 0, 0]];
static SMOKE: OnceLock<ParticleKey> = OnceLock::new();

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = SMOKE.set(registrar.particle_key("minecraft:smoke"));
}

pub(crate) fn dig(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: Position,
    target: Position,
    owner: u128,
    reduced_block_updates: bool,
) -> Option<()> {
    let old = level.block_state(host, target)?;
    let kind = old.block().id(host);
    if unbreakable(&kind) {
        return None;
    }
    let pickaxe = Kind::Vanilla("minecraft:diamond_pickaxe").stack(host, 1)?;
    let drops = level
        .block_drops(host, target, Some(&pickaxe), None)
        .ok()??;
    if drops.is_empty()
        || !android_machine::inside_border(host, level, target)
        || network_runtime::has_block_marker(host, level, target)
        || !level.allow_device_breaking(host, owner, target)
    {
        return None;
    }
    let retained = reduced_block_updates
        && infinite_generator(&kind, |offset| {
            let pos = android_machine::offset(target, offset)?;
            level.block(host, pos).map(|block| block.id(host))
        });
    let removed = host.transaction(|host, _| {
        let mut machine = android_machine::data(host, level, from).ok_or(())?;
        for stack in drops {
            android_machine::push(host, &mut machine, stack);
        }
        if !android_machine::save(host, from, machine)
            || level.block_state(host, target) != Some(old)
        {
            return Err(());
        }
        if !retained {
            let air = host.block_state_id("minecraft:air").ok_or(())?;
            if !level
                .set_block(
                    host,
                    target,
                    air,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                )
                .map_err(|_| ())?
            {
                return Err(());
            }
        }
        Ok(())
    });
    if removed.is_ok() {
        level.block_break_effect(host, target, old);
        if retained {
            generator_effect(host, level, target);
        }
    }
    Some(())
}

fn generator_effect(host: &mut dyn ServerHost, level: LevelRef, [x, y, z]: Position) {
    let [x, y, z] = [x, y, z].map(f64::from);
    let _ = level.play_sound(
        host,
        "minecraft:block.fire.extinguish",
        [x, y, z],
        SoundSource::Blocks,
        0.075,
        0.8,
    );
    if let Some(&key) = SMOKE.get() {
        level.send_particles(
            host,
            &ParticleOptions::simple(key),
            None,
            [x + 0.5, y + 1.25, z + 0.5],
            [0.5; 3],
            0.015,
            8,
        );
    }
}

fn infinite_generator(kind: &str, mut block_at: impl FnMut(Position) -> Option<String>) -> bool {
    let needs = match kind {
        "minecraft:cobblestone" => &["minecraft:water", "minecraft:lava"][..],
        "minecraft:stone" if block_at([0, 1, 0]).as_deref() == Some("minecraft:lava") => {
            &["minecraft:water"][..]
        }
        "minecraft:basalt" if block_at([0, -1, 0]).as_deref() == Some("minecraft:soul_soil") => {
            &["minecraft:lava", "minecraft:blue_ice"][..]
        }
        _ => return false,
    };
    let sides = CONNECTED.map(block_at);
    needs.iter().all(|required| {
        sides
            .iter()
            .any(|material| material.as_deref() == Some(*required))
    })
}

fn unbreakable(id: &str) -> bool {
    matches!(
        id,
        "minecraft:command_block"
            | "minecraft:chain_command_block"
            | "minecraft:repeating_command_block"
            | "minecraft:bedrock"
            | "minecraft:barrier"
            | "minecraft:nether_portal"
            | "minecraft:end_portal"
            | "minecraft:end_portal_frame"
            | "minecraft:end_gateway"
            | "minecraft:structure_block"
            | "minecraft:structure_void"
            | "minecraft:jigsaw"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(kind: &str, area: &[(Position, &str)]) -> bool {
        infinite_generator(kind, |pos| {
            area.iter()
                .find_map(|(at, block)| (*at == pos).then(|| (*block).into()))
        })
    }

    #[test]
    fn miner_uses_paper_generator_geometry_and_unbreakable_tag() {
        assert!(found(
            "minecraft:cobblestone",
            &[
                ([0, 0, -1], "minecraft:water"),
                ([1, 0, 0], "minecraft:lava"),
            ]
        ));
        assert!(!found(
            "minecraft:cobblestone",
            &[
                ([0, 1, 0], "minecraft:water"),
                ([1, 0, 0], "minecraft:lava"),
            ]
        ));
        assert!(found(
            "minecraft:stone",
            &[
                ([0, 0, -1], "minecraft:water"),
                ([0, 1, 0], "minecraft:lava"),
            ]
        ));
        assert!(found(
            "minecraft:basalt",
            &[
                ([0, 0, -1], "minecraft:lava"),
                ([0, -1, 0], "minecraft:soul_soil"),
                ([1, 0, 0], "minecraft:blue_ice"),
            ]
        ));
        assert!(!found(
            "minecraft:basalt",
            &[
                ([0, 0, -1], "minecraft:lava"),
                ([1, 0, 0], "minecraft:blue_ice"),
            ]
        ));
        assert!(unbreakable("minecraft:jigsaw"));
        assert!(unbreakable("minecraft:chain_command_block"));
        assert!(!unbreakable("minecraft:obsidian"));
    }
}
