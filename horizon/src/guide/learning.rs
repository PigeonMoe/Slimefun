// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{Compound, GameMode, Host, Identifier, PlayerRef, ServerHost, SoundSource, Tag};

use super::{Page, catalog, colored_label};
use crate::{recipes::Research, research};

const ANIMATION: &str = "slimefun:research_learning_animation";
const PERSISTED: &str = "PlayerPersisted";
const PROGRESS: [(u32, u8); 6] = [(5, 0), (20, 23), (40, 44), (60, 57), (80, 92), (100, 100)];

#[derive(Clone, Copy)]
struct Settings {
    free_in_creative: bool,
    disable_animation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            free_in_creative: true,
            disable_animation: false,
        }
    }
}

struct Study {
    topic: &'static Research,
    elapsed: u32,
    step: usize,
    return_path: Option<Vec<Page>>,
}

#[derive(Default)]
pub(super) struct Learning {
    settings: Mutex<Settings>,
    active: Mutex<BTreeMap<u128, Study>>,
}

impl Learning {
    pub(super) fn configure(&self, config: Option<&toml::Value>) {
        let read = |key| {
            config
                .and_then(|table| table.get(key))
                .and_then(toml::Value::as_bool)
        };
        *self.settings.lock().unwrap() = Settings {
            free_in_creative: read("free-in-creative-mode").unwrap_or(true),
            disable_animation: read("disable-learning-animation").unwrap_or(false),
        };
    }

    pub(super) fn option(&self, host: &dyn Host, player: PlayerRef) -> Option<bool> {
        if self.settings.lock().unwrap().disable_animation {
            return None;
        }
        let local = player
            .persistent_data(host)
            .as_ref()
            .and_then(local_preference);
        Some(local.unwrap_or_else(|| {
            let key = Identifier::parse(ANIMATION).expect("a registered Paper preference key");
            paper_preference(player.paper_persistent_value(host, &key).as_ref())
        }))
    }

    pub(super) fn toggle(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        let Some(enabled) = self.option(host, player) else {
            return;
        };
        let Some(mut data) = player.persistent_data(host) else {
            return;
        };
        if set_preference(&mut data, !enabled) {
            let _ = player.set_persistent_data(host, data);
        }
    }

    /// Returns whether the guide should immediately redraw the current page.
    pub(super) fn start(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        topic: &'static Research,
        path: &[Page],
    ) -> bool {
        let Settings {
            free_in_creative, ..
        } = *self.settings.lock().unwrap();
        let free = free_in_creative && player.game_mode(host) == Some(GameMode::Creative);
        let delayed = !free && self.option(host, player) == Some(true);
        if let Err(message) = research::begin(host, player, topic, free, delayed) {
            let _ = player.tell(host, colored_label(&format!("&4{message}")));
            return false;
        }
        if !delayed {
            Self::announce_completion(host, player, topic);
            return true;
        }
        self.active.lock().unwrap().insert(
            player.uuid(),
            Study {
                topic,
                elapsed: 0,
                step: 0,
                return_path: Some(path.to_vec()),
            },
        );
        let _ = player.tell(
            host,
            colored_label("&7The Ancient Spirits whisper mysterious words into your ear!"),
        );
        false
    }

    pub(super) fn resume(&self, host: &dyn Host, player: PlayerRef) {
        let Some((id, step)) = player
            .persistent_data(host)
            .as_ref()
            .and_then(research::pending)
        else {
            return;
        };
        let Some(elapsed) = completed_ticks(step) else {
            return;
        };
        let Some(topic) = catalog::research_topic(&id) else {
            return;
        };
        self.active
            .lock()
            .unwrap()
            .entry(player.uuid())
            .or_insert(Study {
                topic,
                elapsed,
                step,
                return_path: None,
            });
    }

    pub(super) fn tick(&self, host: &mut dyn ServerHost, player: PlayerRef) -> Option<Vec<Page>> {
        let uuid = player.uuid();
        let (topic, step, percent) = {
            let mut active = self.active.lock().unwrap();
            let study = active.get_mut(&uuid)?;
            let (deadline, percent) = PROGRESS[study.step];
            study.elapsed = (study.elapsed + 1).min(deadline);
            if study.elapsed < deadline {
                return None;
            }
            (study.topic, study.step, percent)
        };
        let next = (step + 1 < PROGRESS.len()).then_some((step + 1) as i8);
        match research::advance(host, player, topic, step, next) {
            Ok(false) => {
                self.forget(uuid);
                None
            }
            Err(()) => None,
            Ok(true) if next.is_some() => {
                if let Some(study) = self.active.lock().unwrap().get_mut(&uuid) {
                    study.step += 1;
                }
                player.play_sound(
                    host,
                    "minecraft:entity.bat.takeoff",
                    SoundSource::Master,
                    0.7,
                    1.0,
                );
                let _ = player.tell(
                    host,
                    colored_label(&format!(
                        "&7You start to wonder about &b{} &e({percent}%)",
                        topic.name,
                    )),
                );
                None
            }
            Ok(true) => {
                let study = self.active.lock().unwrap().remove(&uuid);
                Self::announce_completion(host, player, topic);
                study?.return_path
            }
        }
    }

    fn announce_completion(host: &mut dyn ServerHost, player: PlayerRef, topic: &Research) {
        let _ = player.tell(
            host,
            colored_label(&format!("&bYou have unlocked &7\"{}\"", topic.name)),
        );
    }

    pub(super) fn forget(&self, player: u128) {
        self.active.lock().unwrap().remove(&player);
    }

    pub(super) fn clear(&self) {
        self.active.lock().unwrap().clear();
    }
}

fn completed_ticks(step: usize) -> Option<u32> {
    if step < PROGRESS.len() {
        Some(
            step.checked_sub(1)
                .map_or(0, |previous| PROGRESS[previous].0),
        )
    } else {
        None
    }
}

fn local_preference(data: &Compound) -> Option<bool> {
    let value = data.get(&PERSISTED.into())?.get(ANIMATION)?;
    match value {
        Tag::Byte(value) => Some(*value == 1),
        _ => None,
    }
}

fn paper_preference(value: Option<&Tag>) -> bool {
    !matches!(value, Some(Tag::Byte(value)) if *value != 1)
}

fn set_preference(data: &mut Compound, enabled: bool) -> bool {
    let Tag::Compound(saved) = data
        .entry(PERSISTED.into())
        .or_insert_with(|| Tag::Compound(Compound::new()))
    else {
        return false;
    };
    saved.insert(ANIMATION.into(), Tag::Byte(i8::from(enabled)));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    use mod_api::{
        __private::{
            built,
            game::{BackendMutRefOf, BackendRefOf},
        },
        BackendMutRef, BackendRef, PlayerDataRefusal, PlayerExperience, Text, TextFormError,
        TransactionSeal,
        prelude::{
            ModStateServerHost, PlayersBackend, PlayersBackendMut, PlayersClientBackend,
            PlayersHost,
        },
        testing::{FakeHost, FakeJournal},
    };

    struct StudyHost {
        state: (Compound, i32),
        journal: FakeJournal<(Compound, i32)>,
        deny_experience_write: bool,
        sounds: usize,
        messages: usize,
    }

    impl Host for StudyHost {
        fn backend(&self) -> Option<BackendRef<'_>> {
            Some(BackendRef::of(self))
        }
    }

    impl ServerHost for StudyHost {
        fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
            Some(BackendMutRef::of(self))
        }
    }

    mod_api::impl_host_areas!(StudyHost);
    mod_api::impl_server_host_areas!(StudyHost, except(mod_state));
    mod_api::impl_backend_areas!(StudyHost, except(players));
    mod_api::impl_backend_mut_areas!(StudyHost, except(players));
    impl mod_api::Backend for StudyHost {}
    impl mod_api::BackendMut for StudyHost {}
    impl PlayersClientBackend for StudyHost {}

    impl PlayersBackend for StudyHost {
        fn player_game_mode(&self, _: PlayerRef) -> Option<GameMode> {
            Some(GameMode::Survival)
        }

        fn player_persistent_data(&self, _: PlayerRef) -> Option<Compound> {
            Some(self.state.0.clone())
        }

        fn player_paper_persistent_value(&self, _: PlayerRef, _: &Identifier) -> Option<Tag> {
            None
        }

        fn player_experience(&self, _: PlayerRef) -> Option<PlayerExperience> {
            Some(
                built::PlayerExperience {
                    level: self.state.1,
                    progress: 0.0,
                    total: 0,
                }
                .into(),
            )
        }
    }

    impl PlayersBackendMut for StudyHost {
        fn player_set_persistent_data(
            &mut self,
            _: PlayerRef,
            data: Compound,
        ) -> Result<(), PlayerDataRefusal> {
            self.state.0 = data;
            Ok(())
        }

        fn player_set_experience_levels(&mut self, _: PlayerRef, level: i32) -> bool {
            if self.deny_experience_write {
                return false;
            }
            self.state.1 = level;
            true
        }

        fn player_tell(&mut self, _: PlayerRef, _: Text) -> Result<bool, TextFormError> {
            self.messages += 1;
            Ok(true)
        }

        fn player_play_sound(
            &mut self,
            _: PlayerRef,
            _: &str,
            _: SoundSource,
            _: f32,
            _: f32,
        ) -> bool {
            self.sounds += 1;
            true
        }
    }

    impl ModStateServerHost for StudyHost {
        fn transaction_open(&mut self) -> TransactionSeal {
            self.journal.open(self.state.clone())
        }

        fn transaction_close(&mut self, seal: &TransactionSeal, commit: bool) {
            if let Some(state) = self.journal.close(seal, commit) {
                self.state = state;
            }
        }
    }

    #[test]
    fn failed_payment_rolls_back_and_resume_only_finishes_the_already_paid_research() {
        let mut identity = FakeHost::new();
        identity.join(1, 100);
        let player = identity.player(1).unwrap();
        let topic = catalog::research_topic("grind_stone").unwrap();
        let mut host = StudyHost {
            state: (Compound::new(), topic.cost + 4),
            journal: FakeJournal::default(),
            deny_experience_write: true,
            sounds: 0,
            messages: 0,
        };

        let guide = Learning::default();
        assert!(!guide.start(&mut host, player, topic, &[Page::Main]));
        assert_eq!(host.state, (Compound::new(), topic.cost + 4));
        assert_eq!(host.journal.closed(), [(1, false)]);

        host.deny_experience_write = false;
        assert!(!guide.start(&mut host, player, topic, &[Page::Main]));
        assert_eq!(host.state.1, 4);
        for _ in 0..20 {
            assert!(guide.tick(&mut host, player).is_none());
        }
        assert_eq!(host.sounds, 2);
        assert_eq!(research::pending(&host.state.0), Some((topic.id.into(), 2)));
        assert!(!research::unlocked(&host.state.0, topic));

        let reopened = Learning::default();
        reopened.resume(&host, player);
        assert!(!reopened.start(&mut host, player, topic, &[Page::Main]));
        for _ in 0..79 {
            assert!(reopened.tick(&mut host, player).is_none());
        }
        assert!(!research::unlocked(&host.state.0, topic));
        assert!(reopened.tick(&mut host, player).is_none());
        assert!(research::unlocked(&host.state.0, topic));
        assert_eq!(research::pending(&host.state.0), None);
        assert_eq!(host.state.1, 4);
        assert_eq!(host.sounds, 5);
    }

    #[test]
    fn reads_a_paper_preference_and_keeps_other_saved_settings_when_changed() {
        assert!(paper_preference(None));
        assert!(paper_preference(Some(&Tag::Int(0))));
        assert!(paper_preference(Some(&Tag::Byte(1))));
        assert!(!paper_preference(Some(&Tag::Byte(0))));
        assert!(!paper_preference(Some(&Tag::Byte(2))));

        let mut data = Compound::from([(
            PERSISTED.into(),
            Tag::compound([("some-mod:setting", Tag::Byte(1))]),
        )]);
        assert_eq!(local_preference(&data), None);
        assert!(set_preference(&mut data, false));
        assert_eq!(local_preference(&data), Some(false));
        assert!(set_preference(&mut data, true));
        assert_eq!(local_preference(&data), Some(true));
        assert_eq!(
            data.get(&PERSISTED.into()).unwrap().get("some-mod:setting"),
            Some(&Tag::Byte(1))
        );
    }

    #[test]
    fn recovered_learning_progress_resumes_from_the_last_paper_milestone() {
        assert_eq!(
            (0..PROGRESS.len()).map(completed_ticks).collect::<Vec<_>>(),
            [Some(0), Some(5), Some(20), Some(40), Some(60), Some(80)],
        );
        assert_eq!(PROGRESS[5].0, 100);
        assert_eq!(completed_ticks(PROGRESS.len()), None);
    }
}
