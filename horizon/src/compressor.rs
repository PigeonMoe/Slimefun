// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::sync::Mutex;

use mod_api::{
    BlockEntityRef, BossBarColor, BossBarRef, Host, HostTransaction, ItemStack, LevelRef, PlayerRef,
    ScheduledTaskEvent, ServerHost, SoundSource, Tag, TaskId, Text,
};

use crate::guide::strip_colors;
use crate::machine::{Dispenser, Refusal, deposit_stack};
use crate::network_runtime::paper_text;
use crate::output_chest;
use crate::random::Rolls;

const STEPS: &[(u32, &str)] = &[
    (0, "minecraft:block.piston.extend"),
    (20, "minecraft:block.piston.contract"),
    (40, "minecraft:block.piston.extend"),
];
const MACHINE_TICKS: u32 = 60;
const PANNING_TICKS: u32 = 120;
const PROGRESS_BAR_PREFIX: &str = "slimefun:craft_progress_";
const PROGRESS_STEP: i64 = 4;

#[derive(Default)]
pub(crate) struct DelayedCrafts {
    tasks: Mutex<BTreeMap<TaskId, Action>>,
    progress: Mutex<BTreeMap<TaskId, CraftProgress>>,
    progress_colors: Rolls,
}

struct CraftProgress {
    bar: BossBarRef,
    level: LevelRef,
    started: i64,
    duration: i32,
    shown: i32,
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
            .map(|&(delay, sound)| (delay, Action::Sound(player, sound)));
        self.queue(
            host,
            (player, dispenser.level, "压缩中"),
            actions,
            (MACHINE_TICKS, Action::Output(output)),
        )
    }

    pub fn queue_pressure_chamber(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
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
        self.queue(
            host,
            (player, dispenser.level, "压缩中"),
            steps,
            (MACHINE_TICKS, Action::Output(output)),
        )
    }

    pub fn queue_magic_crafting(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
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
        self.queue(
            host,
            (player, dispenser.level, "合成中"),
            steps,
            (MACHINE_TICKS, Action::Output(output)),
        )
    }

    pub fn queue_armor_forge(
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
        let sounds = [
            (0, "minecraft:entity.arrow.hit_player"),
            (20, "minecraft:entity.arrow.hit_player"),
            (40, "minecraft:entity.arrow.hit_player"),
            (MACHINE_TICKS, "minecraft:block.anvil.use"),
        ];
        self.queue(
            host,
            (player, dispenser.level, "锻造中"),
            sounds.map(|(delay, sound)| (delay, Action::BlockSound(dispenser, sound))),
            (MACHINE_TICKS, Action::Output(output)),
        )
    }

    pub fn queue_panning(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
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
            (player, level, "自动淘金机淘洗中"),
            steps,
            (
                PANNING_TICKS,
                Action::PanOutput {
                    level,
                    position,
                    cauldron,
                    stack,
                },
            ),
        )
    }

    fn queue(
        &self,
        host: &mut dyn ServerHost,
        (player, level, title): (PlayerRef, LevelRef, &'static str),
        actions: impl IntoIterator<Item = (u32, Action)>,
        finish: (u32, Action),
    ) -> Result<(), Refusal> {
        let duration = finish.0 as i32;
        let title = match &finish.1 {
            Action::Output(output) => {
                product_name(host, &output.stack).append(Text::literal(format!(" {title}")))
            }
            _ => Text::literal(title),
        };
        let mut scheduled = Vec::new();
        for (delay, action) in actions.into_iter().chain([finish]) {
            let Ok(id) = host.schedule_task(delay, 0) else {
                for (pending, _) in scheduled {
                    host.cancel_task(pending);
                }
                return Err(Refusal::Unavailable);
            };
            scheduled.push((id, action));
        }
        let completion = scheduled.last().expect("the output task is scheduled last").0;
        self.tasks.lock().unwrap().extend(scheduled);
        if let Some(started) = level.game_time(host)
            && let Ok(Some(bar)) = host.create_boss_bar(
                &format!("{PROGRESS_BAR_PREFIX}{}", completion.0),
                title,
            )
        {
            let color = self
                .progress_colors
                .next(host, level, BossBarColor::ALL.len() as u32)
                .map_or(BossBarColor::White, |index| BossBarColor::ALL[index as usize]);
            bar.set_color(host, color);
            if bar.set_max(host, duration) && bar.add_player(host, player) {
                self.progress.lock().unwrap().insert(completion, CraftProgress {
                    bar,
                    level,
                    started,
                    duration,
                    shown: 0,
                });
            } else {
                bar.remove(host);
            }
        }
        Ok(())
    }

    pub fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let mut progress = self.progress.lock().unwrap();
        if progress.is_empty() {
            return;
        }
        let Some(now) = level.game_time(host) else { return };
        for craft in progress.values_mut().filter(|craft| craft.level == level) {
            let elapsed = now.saturating_sub(craft.started).max(0);
            let value = (elapsed / PROGRESS_STEP * PROGRESS_STEP)
                .min(i64::from(craft.duration - 1)) as i32;
            if value > craft.shown {
                craft.bar.set_value(host, value);
                craft.shown = value;
            }
        }
    }

    pub fn scheduled_task(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let action = self.tasks.lock().unwrap().remove(&event.id);
        if let Some(craft) = self.progress.lock().unwrap().remove(&event.id) {
            craft.bar.remove(host);
        }
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

    pub fn remove_saved_progress(host: &mut dyn ServerHost) {
        for bar in host.boss_bars() {
            if bar.id(host).is_some_and(|id| id.starts_with(PROGRESS_BAR_PREFIX)) {
                bar.remove(host);
            }
        }
    }

    pub fn clear(&self, host: &mut dyn ServerHost) {
        for (_, craft) in std::mem::take(&mut *self.progress.lock().unwrap()) {
            craft.bar.remove(host);
        }
        self.tasks.lock().unwrap().clear();
        self.progress_colors.clear();
    }
}

fn product_name(host: &dyn Host, stack: &ItemStack) -> Text {
    let patch = stack.components().map(|components| components.as_nbt());
    let default = host.item_default_component(stack.item(), "minecraft:item_name");
    let names = ["minecraft:custom_name", "minecraft:item_name"]
        .into_iter()
        .filter_map(|key| patch?.get(&key.into()))
        .chain(default.as_ref());
    for name in names {
        if let Some(Tag::String(key)) = name.get("translate") {
            return Text::translate(key.to_string_lossy(), Vec::new());
        }
        if let Some(label) = paper_text(name).map(|name| strip_colors(&name))
            && !label.is_empty()
        {
            return Text::literal(label);
        }
    }

    let id = stack.item().id(host);
    let kind = if host.block_key(&id).is_some() { "block" } else { "item" };
    Text::translate(format!("{kind}.{}", id.replace([':', '/'], ".")), Vec::new())
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
