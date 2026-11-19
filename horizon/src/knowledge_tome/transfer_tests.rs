// SPDX-License-Identifier: GPL-3.0-only

use super::*;

use mod_api::{
    __private::game::{BackendMutRefOf, BackendRefOf},
    BackendMutRef, BackendRef, ConfigEntry, ConfigFileError, ConfigFiles, ConfigPath, ConfigStat,
    Host, PlayerDataRefusal, StackWriteError, TextFormError, Transaction, TransactionSeal,
    prelude::{
        ConfigServerHost, ItemsBackendMut, ModStateServerHost, PlayersBackend, PlayersBackendMut,
        PlayersClientBackend, PlayersHost, PlayersServerHost,
    },
    testing::{FakeHost, FakeJournal},
};

const DONOR: u128 = 21;
const RECIPIENT: u128 = 22;
const INITIAL: &str = "slimefun:tome_of_knowledge_sharing";
const GRIND_STONE: &str = "slimefun:grind_stone";
const TABLE_SAW: &str = "slimefun:table_saw";

type PlayerState = (Option<ItemStack>, Compound);

struct TomeHost {
    identity: FakeHost,
    state: PlayerState,
    journal: FakeJournal<PlayerState>,
    donor: Result<Option<Vec<Identifier>>, PlayerFlagReadError>,
    legacy: Option<String>,
    mode: GameMode,
    deny_data_write: bool,
    messages: usize,
}

impl Host for TomeHost {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(BackendRef::of(self))
    }
}
impl ServerHost for TomeHost {
    fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
        Some(BackendMutRef::of(self))
    }
}
mod_api::impl_host_areas!(TomeHost, except(players));
mod_api::impl_server_host_areas!(TomeHost, except(mod_state, config, players));
mod_api::impl_backend_areas!(TomeHost, except(players));
mod_api::impl_backend_mut_areas!(TomeHost, except(items, players));
impl mod_api::Backend for TomeHost {}
impl mod_api::BackendMut for TomeHost {}
impl PlayersClientBackend for TomeHost {}

impl PlayersHost for TomeHost {
    fn player(&self, uuid: u128) -> Option<PlayerRef> {
        self.identity.player(uuid)
    }
}
impl PlayersServerHost for TomeHost {
    fn player_saved_true_flags(
        &self,
        uuid: u128,
        keys: &[Identifier],
    ) -> Result<Option<Vec<Identifier>>, PlayerFlagReadError> {
        assert_eq!(uuid, DONOR);
        assert!(keys.contains(&Identifier::parse(GRIND_STONE).unwrap()));
        self.donor.clone()
    }
}
impl PlayersBackend for TomeHost {
    fn player_game_mode(&self, _: PlayerRef) -> Option<GameMode> {
        Some(self.mode)
    }

    fn player_name(&self, player: PlayerRef) -> Option<String> {
        player.name(&self.identity)
    }

    fn player_selected_slot(&self, _: PlayerRef) -> Option<usize> {
        Some(3)
    }

    fn player_persistent_data(&self, _: PlayerRef) -> Option<Compound> {
        Some(self.state.1.clone())
    }
}
impl PlayersBackendMut for TomeHost {
    fn player_set_inventory_slot(
        &mut self,
        _: PlayerRef,
        slot: u32,
        stack: Option<ItemStack>,
    ) -> Result<(), PlayerDataRefusal> {
        assert_eq!(slot, 3);
        self.state.0 = stack;
        Ok(())
    }

    fn player_set_persistent_data(
        &mut self,
        _: PlayerRef,
        data: Compound,
    ) -> Result<(), PlayerDataRefusal> {
        if self.deny_data_write {
            return Err(PlayerDataRefusal::Stale);
        }
        self.state.1 = data;
        Ok(())
    }

    fn player_tell(&mut self, _: PlayerRef, _: Text) -> Result<bool, TextFormError> {
        self.messages += 1;
        Ok(true)
    }

    fn player_play_sound(&mut self, _: PlayerRef, _: &str, _: SoundSource, _: f32, _: f32) -> bool {
        true
    }
}
impl ItemsBackendMut for TomeHost {
    fn game_stack(&mut self, _: &ItemPlace) -> Option<ItemStack> {
        self.state.0.clone()
    }

    fn game_extract(
        &mut self,
        _: &ItemPlace,
        resource: &ItemResource,
        amount: u32,
        _: &Transaction,
    ) -> Result<u32, StackWriteError> {
        let Some(held) = self.state.0.as_ref().filter(|held| resource.matches(held)) else {
            return Ok(0);
        };
        let split = held.clone().split(amount);
        self.state.0 = split.remainder;
        Ok(split.taken.map_or(0, |stack| stack.count()))
    }
}
impl ConfigServerHost for TomeHost {
    fn world_data_files(&self) -> &dyn ConfigFiles {
        self
    }
}
impl ConfigFiles for TomeHost {
    fn read(&self, path: &ConfigPath) -> Result<Vec<u8>, ConfigFileError> {
        assert_eq!(
            String::from(path),
            format!("research-import/{DONOR:032x}.txt")
        );
        self.legacy
            .as_ref()
            .map(|text| text.as_bytes().to_vec())
            .ok_or(ConfigFileError::NotFound)
    }

    fn write(&self, _: &ConfigPath, _: &[u8]) -> Result<(), ConfigFileError> {
        panic!("donor records are read only")
    }

    fn list(
        &self,
        _: &ConfigPath,
        _: Option<&str>,
        _: u32,
    ) -> Result<Vec<ConfigEntry>, ConfigFileError> {
        panic!("donors are selected by exact UUID")
    }

    fn stat(&self, _: &ConfigPath) -> Result<Option<ConfigStat>, ConfigFileError> {
        panic!("donor records are read directly")
    }
}
impl ModStateServerHost for TomeHost {
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
fn transfer_consumes_one_only_when_persisted_and_keeps_creative_copies() {
    let mut identity = FakeHost::new();
    identity
        .registries
        .resolve_items([Identifier::parse("minecraft:enchanted_book").unwrap()])
        .unwrap();
    identity.join(DONOR, 101);
    identity.join(RECIPIENT, 102);
    let giver = identity.player(DONOR).unwrap();
    let reader = identity.player(RECIPIENT).unwrap();
    let blank = TOME_OF_KNOWLEDGE_SHARING.stack(&identity, 1).unwrap();
    let recip_data = Compound::from([(
        "PlayerPersisted".into(),
        Tag::compound([
            (INITIAL, Tag::Byte(1)),
            ("slimefun:research_fireworks", Tag::Byte(0)),
        ]),
    )]);
    let mut host = TomeHost {
        identity,
        state: (Some(blank.clone()), recip_data.clone()),
        journal: FakeJournal::default(),
        donor: Ok(Some(vec![Identifier::parse(GRIND_STONE).unwrap()])),
        legacy: Some(format!(
            "slimefun-research-import-v1\n{DONOR:032x}\n{TABLE_SAW}\n"
        )),
        mode: GameMode::Creative,
        deny_data_write: false,
        messages: 0,
    };
    let guide = Guide::default();

    use_tome(&mut host, giver, Hand::Main, &blank, &guide).unwrap();
    let bound = host.state.0.clone().unwrap();
    assert_eq!(owner(&bound), Ok(Some(DONOR)));
    assert!(use_tome(&mut host, giver, Hand::Main, &bound, &guide).is_err());
    assert_eq!(host.state.0.as_ref(), Some(&bound));

    host.deny_data_write = true;
    host.mode = GameMode::Survival;
    assert!(use_tome(&mut host, reader, Hand::Main, &bound, &guide).is_err());
    assert_eq!(host.state, (Some(bound.clone()), recip_data.clone()));
    assert_eq!(host.messages, 0);

    host.deny_data_write = false;
    use_tome(&mut host, reader, Hand::Main, &bound, &guide).unwrap();
    let flags = &host.state.1[&"PlayerPersisted".into()];
    assert_eq!(flags.get(GRIND_STONE), Some(&Tag::Byte(1)));
    assert_eq!(flags.get(TABLE_SAW), None);
    assert!(host.state.0.is_none());

    host.identity = FakeHost::new();
    host.identity.join(RECIPIENT, 102);
    host.state = (Some(bound.clone()), recip_data);
    host.donor = Ok(None);
    host.mode = GameMode::Creative;
    use_tome(&mut host, reader, Hand::Main, &bound, &guide).unwrap();
    let flags = &host.state.1[&"PlayerPersisted".into()];
    assert_eq!(flags.get(TABLE_SAW), Some(&Tag::Byte(1)));
    assert_eq!(flags.get(GRIND_STONE), None);
    assert_eq!(host.state.0.as_ref(), Some(&bound));
    assert_eq!(host.messages, 2);
    use_tome(&mut host, reader, Hand::Main, &bound, &guide).unwrap();
    assert_eq!(host.messages, 2);

    host.donor = Err(PlayerFlagReadError::Corrupt);
    host.mode = GameMode::Survival;
    assert!(use_tome(&mut host, reader, Hand::Main, &bound, &guide).is_err());
    assert_eq!(host.state.0.as_ref(), Some(&bound));
}
