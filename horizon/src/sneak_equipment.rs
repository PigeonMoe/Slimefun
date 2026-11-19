// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    EntityRef, HostTransaction, ItemPlace, ItemResource, ItemStack, LevelRef, LivingRef, PlayerRef,
    ScheduledTaskEvent, ServerHost, SoundSource, TaskId, Text,
};

use crate::{factory::charging_bench, random::Rolls, recipes::Research, research};

pub(crate) trait Drive: Copy {
    const PERIOD: u32;

    fn begin(host: &mut dyn ServerHost, player: PlayerRef) -> Option<Self>;
    fn pulse(self, host: &mut dyn ServerHost, player: PlayerRef, rolls: &Rolls) -> bool;
}

pub(crate) fn research_for(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    required: &Research,
) -> bool {
    if research::can_use(host, player, required) {
        return true;
    }
    let _ = player.tell(
        host,
        Text::literal(format!(
            "Unlock {} with /sf research {}.",
            required.name, required.id
        )),
    );
    false
}

#[derive(Clone, Copy)]
struct Active<D> {
    player: PlayerRef,
    drive: D,
    task: Option<TaskId>,
}

pub(crate) struct Task<D> {
    // A seen crouch remains here after its charge is exhausted, until the next release.
    crouched: Mutex<BTreeMap<u128, Option<Active<D>>>>,
}

impl<D> Default for Task<D> {
    fn default() -> Self {
        Self {
            crouched: Mutex::new(BTreeMap::new()),
        }
    }
}

impl<D: Drive> Task<D> {
    pub(crate) fn toggle(&self, host: &mut dyn ServerHost, player: PlayerRef, sneaking: bool) {
        let uuid = player.uuid();
        if !sneaking {
            self.forget(host, uuid);
            return;
        }
        if self.crouched.lock().unwrap().contains_key(&uuid) {
            return;
        }
        let active = D::begin(host, player).and_then(|drive| {
            let task = host.schedule_task(0, 0).ok()?;
            Some(Active {
                player,
                drive,
                task: Some(task),
            })
        });
        self.crouched.lock().unwrap().insert(uuid, active);
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, player: PlayerRef, rolls: &Rolls) {
        let Some(sneaking) = player.is_secondary_use_active(host) else {
            return;
        };
        // Integrated players do not send the guest-only PlayerToggleSneak packet.
        self.toggle(host, player, sneaking);
        if !sneaking {
            return;
        }
        let uuid = player.uuid();
        let active = self
            .crouched
            .lock()
            .unwrap()
            .get(&uuid)
            .copied()
            .flatten()
            .filter(|active| active.task.is_none() && active.player == player);
        let Some(active) = active else {
            return;
        };
        let living = LivingRef::new(player.entity());
        let alive = living.health(host).is_some_and(|health| health > 0.0);
        let next = if alive && active.drive.pulse(host, player, rolls) {
            host.schedule_task(D::PERIOD, 0).ok().map(|task| Active {
                task: Some(task),
                ..active
            })
        } else {
            None
        };
        let mut riders = self.crouched.lock().unwrap();
        if let Some(slot) = riders.get_mut(&uuid)
            && slot.is_some_and(|slot| slot.task.is_none() && slot.player == player)
        {
            *slot = next;
        }
    }

    pub(crate) fn due(&self, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        // The next player tick runs in that player's actual level; a scheduled
        // task itself may run while their dimension is parked.
        if let Some(active) = self
            .crouched
            .lock()
            .unwrap()
            .values_mut()
            .flatten()
            .find(|active| active.task == Some(event.id))
        {
            active.task = None;
        }
    }

    pub(crate) fn forget(&self, host: &mut dyn ServerHost, uuid: u128) {
        let active = self.crouched.lock().unwrap().remove(&uuid).flatten();
        if let Some(task) = active.and_then(|active| active.task) {
            let _ = host.cancel_task(task);
        }
    }

    pub(crate) fn clear(&self) {
        self.crouched.lock().unwrap().clear();
    }
}

pub(crate) fn charged_thrust(
    host: &mut dyn ServerHost,
    entity: EntityRef,
    (slot, stack, cost): (ItemPlace, &ItemStack, f32),
    motion: impl FnOnce(&mut dyn ServerHost, LevelRef) -> Option<[f64; 3]>,
    sound: &str,
) -> bool {
    let Some(replacement) = charging_bench::discharge(host, stack, cost) else {
        return false;
    };
    let (Some(level), Some(pos)) = (entity.level(host), entity.position(host)) else {
        return false;
    };
    let Some(motion) = motion(host, level) else {
        return false;
    };
    let paid = host.transaction(|host, tx| {
        if slot.resource(host).as_ref() != Some(&ItemResource::of(stack))
            || slot.exchange(host, tx, &ItemResource::of(&replacement), 1) != 1
            || !entity.set_velocity(host, motion)
        {
            return Err(());
        }
        Ok(())
    });
    if paid.is_err() {
        return false;
    }
    let _ = level.play_sound(host, sound, pos, SoundSource::Players, 0.25, 1.0);
    if let Some(block) = entity.block_position(host) {
        let _ = level.level_event(host, 2000, block, 1);
    }
    let _ = entity.reset_fall_distance(host);
    true
}
