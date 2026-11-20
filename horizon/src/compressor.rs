// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::sync::Mutex;

use mod_api::{
    BlockEntityRef, HostTransaction, ItemStack, LevelRef, PlayerRef, ScheduledTaskEvent,
    ServerHost, SoundSource, TaskId,
};

use crate::machine::{Dispenser, Refusal, deposit_stack};
use crate::output_chest;

const STEPS: &[(u32, &str)] = &[
    (0, "minecraft:block.piston.extend"),
    (20, "minecraft:block.piston.contract"),
    (40, "minecraft:block.piston.extend"),
];

#[derive(Default)]
pub(crate) struct DelayedCrafts {
    tasks: Mutex<BTreeMap<TaskId, Action>>,
}

struct Output {
    holder: BlockEntityRef,
    dispenser: Dispenser,
    stack: ItemStack,
}

enum Action {
    Sound(PlayerRef, &'static str),
    BlockSound(Dispenser, &'static str),
    PressureStep {
        level: LevelRef,
        position: [i32; 3],
        finish: bool,
    },
    MagicStep {
        level: LevelRef,
        position: [i32; 3],
        finish: bool,
    },
    PanStep {
        level: LevelRef,
        cauldron: [i32; 3],
        sound: &'static str,
    },
    PanOutput {
        level: LevelRef,
        position: [i32; 3],
        cauldron: [i32; 3],
        stack: Option<ItemStack>,
    },
    Output(Output),
}

impl DelayedCrafts {
    pub fn queue_compressor(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        holder: BlockEntityRef,
        dispenser: Dispenser,
        stack: ItemStack,
    ) -> Result<(), Refusal> {
        let output = Output {
            holder,
            dispenser,
            stack,
        };
        let actions = STEPS
            .iter()
            .map(|&(delay, sound)| (delay, Action::Sound(player, sound)))
            .chain([(60, Action::Output(output))]);
        self.queue(host, actions)
    }

    pub fn queue_pressure_chamber(
        &self,
        host: &mut dyn ServerHost,
        holder: BlockEntityRef,
        dispenser: Dispenser,
        position: [i32; 3],
        stack: ItemStack,
    ) -> Result<(), Refusal> {
        let steps = (0..4).map(|step| {
            (
                step * 20,
                Action::PressureStep {
                    level: dispenser.level,
                    position,
                    finish: step == 3,
                },
            )
        });
        let output = Output {
            holder,
            dispenser,
            stack,
        };
        self.queue(host, steps.chain([(60, Action::Output(output))]))
    }

    pub fn queue_magic_crafting(
        &self,
        host: &mut dyn ServerHost,
        holder: BlockEntityRef,
        dispenser: Dispenser,
        position: [i32; 3],
        stack: ItemStack,
    ) -> Result<(), Refusal> {
        let steps = (0..4).map(|step| {
            (
                step * 20,
                Action::MagicStep {
                    level: dispenser.level,
                    position,
                    finish: step == 3,
                },
            )
        });
        let output = Output {
            holder,
            dispenser,
            stack,
        };
        self.queue(host, steps.chain([(60, Action::Output(output))]))
    }

    pub fn queue_armor_forge(
        &self,
        host: &mut dyn ServerHost,
        holder: BlockEntityRef,
        dispenser: Dispenser,
        stack: ItemStack,
    ) -> Result<(), Refusal> {
        let output = Output {
            holder,
            dispenser,
            stack,
        };
        let sounds = [
            (0, "minecraft:entity.arrow.hit_player"),
            (20, "minecraft:entity.arrow.hit_player"),
            (40, "minecraft:entity.arrow.hit_player"),
            (60, "minecraft:block.anvil.use"),
        ];
        self.queue(
            host,
            sounds
                .map(|(delay, sound)| (delay, Action::BlockSound(dispenser, sound)))
                .into_iter()
                .chain([(60, Action::Output(output))]),
        )
    }

    pub fn queue_panning(
        &self,
        host: &mut dyn ServerHost,
        level: LevelRef,
        position: [i32; 3],
        cauldron: [i32; 3],
        sound: &'static str,
        stack: Option<ItemStack>,
    ) -> Result<(), Refusal> {
        // Dough schedules five successive 20-tick delays, then 20 ticks for output.
        let steps = (1..=5).map(|step| {
            (
                step * 20,
                Action::PanStep {
                    level,
                    cauldron,
                    sound,
                },
            )
        });
        self.queue(
            host,
            steps.chain([(
                120,
                Action::PanOutput {
                    level,
                    position,
                    cauldron,
                    stack,
                },
            )]),
        )
    }

    fn queue(
        &self,
        host: &mut dyn ServerHost,
        actions: impl IntoIterator<Item = (u32, Action)>,
    ) -> Result<(), Refusal> {
        let mut scheduled = Vec::new();
        for (delay, action) in actions {
            let Ok(id) = host.schedule_task(delay, 0) else {
                for (pending, _) in scheduled {
                    host.cancel_task(pending);
                }
                return Err(Refusal::Unavailable);
            };
            scheduled.push((id, action));
        }
        self.tasks.lock().unwrap().extend(scheduled);
        Ok(())
    }

    pub fn scheduled_task(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let action = self.tasks.lock().unwrap().remove(&event.id);
        match action {
            Some(Action::Sound(player, sound)) => {
                player.play_sound(host, sound, SoundSource::Blocks, 1.0, 1.0);
            }
            Some(Action::BlockSound(dispenser, sound)) => {
                let position = dispenser.position.map(|axis| f64::from(axis) + 0.5);
                let _ = dispenser.level.play_sound(
                    host,
                    sound,
                    position,
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                );
            }
            Some(Action::PressureStep {
                level,
                position,
                finish,
            }) => pressure_step(host, level, position, finish),
            Some(Action::MagicStep {
                level,
                position,
                finish,
            }) => {
                let _ = level.level_event(host, 2004, position, 0);
                let _ = level.level_event(host, 2003, position, 0);
                let sound = if finish {
                    "minecraft:entity.arrow.hit_player"
                } else {
                    "minecraft:block.wooden_button.click_on"
                };
                let at = position.map(|axis| f64::from(axis) + 0.5);
                let _ = level.play_sound(host, sound, at, SoundSource::Blocks, 1.0, 1.0);
            }
            Some(Action::PanStep {
                level,
                cauldron,
                sound,
            }) => {
                let location = cauldron.map(|axis| f64::from(axis) + 0.5);
                let _ = level.play_sound(host, sound, location, SoundSource::Blocks, 1.0, 1.0);
            }
            Some(Action::PanOutput {
                level,
                position,
                cauldron,
                stack,
            }) => {
                let location = position.map(|axis| f64::from(axis) + 0.5);
                let Some(stack) = stack else {
                    let _ = level.play_sound(
                        host,
                        "minecraft:entity.armor_stand.break",
                        location,
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                    );
                    return;
                };
                if output_chest::route_or_drop(host, level, cauldron, position, &stack) {
                    let _ = level.play_sound(
                        host,
                        "minecraft:entity.arrow.hit_player",
                        location,
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                    );
                } else if let Ok(task) = host.schedule_task(20, 0) {
                    self.tasks.lock().unwrap().insert(
                        task,
                        Action::PanOutput {
                            level,
                            position,
                            cauldron,
                            stack: Some(stack),
                        },
                    );
                }
            }
            Some(Action::Output(output)) => {
                if let Err(output) = output.deliver(host)
                    && let Ok(task) = host.schedule_task(20, 0)
                {
                    self.tasks
                        .lock()
                        .unwrap()
                        .insert(task, Action::Output(output));
                }
            }
            None => {}
        }
    }

    pub fn clear(&self) {
        self.tasks.lock().unwrap().clear();
    }
}

fn pressure_step(host: &mut dyn ServerHost, level: LevelRef, position: [i32; 3], finish: bool) {
    let center = position.map(|axis| f64::from(axis) + 0.5);
    let working = "minecraft:entity.tnt.primed";
    let _ = level.play_sound(host, working, center, SoundSource::Blocks, 1.0, 1.0);
    let [x, y, z] = position;
    for _ in 0..3 {
        // Paper's Effect.SMOKE is level event 2000, with direction 4.
        let _ = level.level_event(host, 2000, [x, y + 1, z], 4);
    }
    let second = if finish {
        "minecraft:entity.arrow.hit_player"
    } else {
        working
    };
    let _ = level.play_sound(host, second, center, SoundSource::Blocks, 1.0, 1.0);
}

impl Output {
    fn deliver(self, host: &mut dyn ServerHost) -> Result<(), Self> {
        let loaded = host.block_entity(self.dispenser.level, self.dispenser.position);
        let same_unlocked_dispenser =
            loaded.is_some_and(|holder| holder == self.holder && holder.lock(host).is_none());
        let result: Result<(), ()> = host.transaction(|host, tx| {
            if output_chest::try_deposit(
                host,
                self.dispenser.level,
                self.dispenser.position,
                &self.stack,
            ) {
                return Ok(());
            }
            let remaining = if same_unlocked_dispenser {
                deposit_stack(host, tx, self.dispenser, &self.stack, self.stack.count())
            } else {
                self.stack.count()
            };
            if remaining == 0 {
                return Ok(());
            }
            let mut stack = self.stack.clone();
            stack.set_count(remaining).map_err(|_| ())?;
            let [x, y, z] = self.dispenser.position.map(|axis| f64::from(axis) + 0.5);
            self.dispenser
                .level
                .spawn_item(host, x, y, z, stack, 10)
                .map_err(|_| ())?
                .ok_or(())?;
            Ok(())
        });
        result.map_err(|_| self)
    }
}

#[cfg(test)]
mod tests {
    use mod_api::testing::FakeHost;

    use super::*;

    #[test]
    fn panning_with_no_configured_output_finishes_with_papers_failure_sound_once() {
        let tasks = DelayedCrafts::default();
        let mut host = FakeHost::new();
        let id = TaskId(1);
        tasks.tasks.lock().unwrap().insert(
            id,
            Action::PanOutput {
                level: host.level.level(),
                position: [1, 65, 2],
                cauldron: [1, 64, 2],
                stack: None,
            },
        );
        let event = ScheduledTaskEvent {
            id,
            owner: "slimefun",
            token: 0,
        };
        tasks.scheduled_task(&mut host, &event);
        tasks.scheduled_task(&mut host, &event);
        assert!(tasks.tasks.lock().unwrap().is_empty());
        assert_eq!(host.sounds.len(), 1);
        assert_eq!(host.sounds[0].sound, "minecraft:entity.armor_stand.break");
        assert_eq!(host.sounds[0].at, [1.5, 65.5, 2.5]);
    }
}
