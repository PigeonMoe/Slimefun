// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::sync::Mutex;

use mod_api::{
    BlockEntityRef, HostTransaction, ItemStack, PlayerRef, ScheduledTaskEvent, ServerHost,
    SoundSource, TaskId,
};

use crate::machine::{Dispenser, Refusal, deposit_stack};
use crate::output_chest;

const STEPS: &[(u32, &str)] = &[
    (0, "minecraft:block.piston.extend"),
    (20, "minecraft:block.piston.contract"),
    (40, "minecraft:block.piston.extend"),
];

#[derive(Default)]
pub(crate) struct Compressor {
    tasks: Mutex<BTreeMap<TaskId, Action>>,
}

struct Output {
    holder: BlockEntityRef,
    dispenser: Dispenser,
    stack: ItemStack,
}

enum Action {
    Sound(PlayerRef, &'static str),
    Output(Output),
}

impl Compressor {
    pub fn queue(
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
