// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ItemDrop, LivingDropsEvent, ServerHost};

use crate::{
    items::BASIC_CIRCUIT_BOARD, machine::Settings, random::Rolls,
    recipes::BASIC_CIRCUIT_BOARD_RESEARCH, research,
};

pub(crate) fn add_golem_drop(
    host: &mut dyn ServerHost,
    event: &mut LivingDropsEvent<'_>,
    settings: Settings,
    rolls: &Rolls,
) {
    if !settings.drop_from_golems || event.kind != "minecraft:iron_golem" {
        return;
    }
    let Some(player) = event.credited_player.and_then(|id| host.player(id)) else {
        return;
    };
    if !research::can_use(host, player, &BASIC_CIRCUIT_BOARD_RESEARCH) {
        return;
    }
    let victim = host.entity(event.entity);
    let Some(level) = victim
        .and_then(|entity| entity.level(host))
        .or_else(|| player.entity().level(host))
    else {
        return;
    };
    let Some(rolled) = rolls.next(host, level, 100) else {
        return;
    };
    if rolled >= settings.golem_drop_chance {
        return;
    }
    let Some(position) = victim
        .and_then(|entity| entity.position(host))
        .or_else(|| event.drops.first().map(|drop| drop.position))
    else {
        return;
    };
    if let Some(board) = BASIC_CIRCUIT_BOARD.stack(host, 1) {
        let horizontal = || {
            rolls
                .next(host, level, 201)
                .map_or(0.0, |roll| (f64::from(roll) - 100.0) / 1000.0)
        };
        event.drops.push(ItemDrop::new(
            board,
            position,
            [horizontal(), 0.2, horizontal()],
            10,
        ));
    }
}
