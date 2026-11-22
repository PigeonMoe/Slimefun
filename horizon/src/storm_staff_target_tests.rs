// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use mod_api::{
    __private::game::{BackendMutRefOf, BackendRefOf},
    BackendMutRef, BackendRef, BlockKey, EntityRef, Identifier, ModEntry, PlayerDataRefusal,
    PlayerFood, Registrar, RegistrationContext, Text, TextFormError, TransactionSeal,
    prelude::{
        EntitiesBackend, ItemsBackendMut, ItemsHost, KeysBackend, LevelsBackend, LevelsBackendMut,
        ModStateServerHost, PlayersBackend, PlayersBackendMut, PlayersClientBackend, PlayersHost,
    },
    testing::FakeHost,
};
use std::sync::{Arc, OnceLock};

const TARGET: [i32; 3] = [3, 65, 8];

struct Blocks(Arc<OnceLock<(BlockKey, BlockKey)>>);

#[mod_api::__subscriptions]
impl mod_api::Hooks for Blocks {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        self.0
            .set((
                registrar.block_key("minecraft:air"),
                registrar.block_key("minecraft:stone"),
            ))
            .unwrap();
    }
}

struct CastHost {
    game: FakeHost,
    air: BlockKey,
    stone: BlockKey,
    player: PlayerRef,
    held: Option<ItemStack>,
    food: i32,
    allowed: bool,
    queries: std::cell::RefCell<Vec<[i32; 3]>>,
    strikes: Vec<[f64; 3]>,
    messages: usize,
}

impl Host for CastHost {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(BackendRef::of(self))
    }
}
impl ServerHost for CastHost {
    fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
        Some(BackendMutRef::of(self))
    }
}
mod_api::impl_host_areas!(CastHost, except(items));
mod_api::impl_server_host_areas!(CastHost, except(mod_state));
mod_api::impl_backend_areas!(CastHost, except(entities, keys, levels, players));
mod_api::impl_backend_mut_areas!(CastHost, except(items, levels, players));
impl mod_api::Backend for CastHost {}
impl mod_api::BackendMut for CastHost {}
impl PlayersClientBackend for CastHost {}

impl ItemsHost for CastHost {
    fn item_key(&self, id: &Identifier) -> Option<mod_api::ItemKey> {
        self.game.item_key(id)
    }
    fn max_stack_size(&self, id: mod_api::ItemKey) -> Option<u32> {
        self.game.max_stack_size(id)
    }
}
impl KeysBackend for CastHost {
    fn block_key_id(&self, key: BlockKey) -> String {
        if key == self.air {
            "minecraft:air".into()
        } else {
            assert_eq!(key, self.stone);
            "minecraft:stone".into()
        }
    }
    fn item_key_id(&self, key: mod_api::ItemKey) -> String {
        self.game.item_key_id(key)
    }
}
impl LevelsBackend for CastHost {
    fn level_height(&self, _: LevelRef) -> Option<(i32, i32)> {
        Some((-64, 384))
    }
    fn level_block(&self, _: LevelRef, pos: [i32; 3]) -> Option<BlockKey> {
        Some(if pos == TARGET { self.stone } else { self.air })
    }
    fn level_is_loaded(&self, _: LevelRef, pos: [i32; 3]) -> bool {
        pos == TARGET
    }
    fn level_allow_player_attack_at(
        &self,
        level: LevelRef,
        player: PlayerRef,
        pos: [i32; 3],
    ) -> bool {
        assert_eq!((level, player), (self.game.level.level(), self.player));
        self.queries.borrow_mut().push(pos);
        self.allowed
    }
}
impl EntitiesBackend for CastHost {
    fn entity_position(&self, _: EntityRef) -> Option<[f64; 3]> {
        Some([3.5, 64.0, 6.5])
    }
    fn entity_eye_position(&self, _: EntityRef) -> Option<[f64; 3]> {
        Some([3.5, 65.6, 6.5])
    }
    fn entity_rotation(&self, _: EntityRef) -> Option<[f32; 2]> {
        Some([0.0, 0.0])
    }
}
impl PlayersBackend for CastHost {
    fn player_level(&self, _: PlayerRef) -> Option<LevelRef> {
        Some(self.game.level.level())
    }
    fn player_game_mode(&self, _: PlayerRef) -> Option<GameMode> {
        Some(GameMode::Survival)
    }
    fn player_food(&self, _: PlayerRef) -> Option<PlayerFood> {
        Some(
            mod_api::__private::built::PlayerFood {
                level: self.food,
                saturation: 0.0,
                exhaustion: 0.0,
            }
            .into(),
        )
    }
    fn player_selected_slot(&self, _: PlayerRef) -> Option<usize> {
        Some(0)
    }
    fn player_inventory_slots(&self, _: PlayerRef) -> Option<Vec<(u32, ItemStack)>> {
        Some(
            self.held
                .clone()
                .into_iter()
                .map(|stack| (0, stack))
                .collect(),
        )
    }
    fn player_persistent_data(&self, _: PlayerRef) -> Option<Compound> {
        Some(Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([("slimefun:storm_staff", Tag::Byte(1))]),
        )]))
    }
}
impl PlayersBackendMut for CastHost {
    fn player_set_food_level(&mut self, _: PlayerRef, level: i32) -> bool {
        self.food = level;
        true
    }
    fn player_set_inventory_slot(
        &mut self,
        _: PlayerRef,
        slot: u32,
        stack: Option<ItemStack>,
    ) -> Result<(), PlayerDataRefusal> {
        assert_eq!(slot, 0);
        self.held = stack;
        Ok(())
    }
    fn player_tell(&mut self, _: PlayerRef, _: Text) -> Result<bool, TextFormError> {
        self.messages += 1;
        Ok(true)
    }
}
impl ItemsBackendMut for CastHost {
    fn game_stack(&mut self, place: &ItemPlace) -> Option<ItemStack> {
        assert_eq!(
            *place,
            ItemPlace::PlayerHand {
                player: self.player.uuid(),
                hand: Hand::Main
            }
        );
        self.held.clone()
    }
}
impl LevelsBackendMut for CastHost {
    fn level_strike_lightning(&mut self, _: LevelRef, at: [f64; 3], visual_only: bool) -> bool {
        assert!(!visual_only);
        self.strikes.push(at);
        true
    }
}
impl ModStateServerHost for CastHost {
    fn transaction_open(&mut self) -> TransactionSeal {
        self.game.transaction_open()
    }
    fn transaction_close(&mut self, seal: &TransactionSeal, commit: bool) {
        assert!(commit);
        self.game.transaction_close(seal, commit);
    }
}

#[test]
fn a_denied_landing_block_leaves_the_click_hunger_and_staff_untouched() {
    let stone = Arc::new(OnceLock::new());
    mod_api::__private::part_tests::install_parts_fixture(&[ModEntry::new(
        "storm_fixture",
        Box::new(Blocks(stone.clone())),
    )])
    .unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut game = FakeHost::new();
    game.registries
        .resolve_items([Identifier::parse("minecraft:stick").unwrap()])
        .unwrap();
    game.join(13, 13);
    let player = game.player(13).unwrap();
    let staff = STAFF_ELEMENTAL_STORM.stack(&game, 1).unwrap();
    let (air, stone) = *stone.get().unwrap();
    let mut host = CastHost {
        game,
        air,
        stone,
        player,
        held: Some(staff.clone()),
        food: 20,
        allowed: false,
        queries: Default::default(),
        strikes: Vec::new(),
        messages: 0,
    };

    assert_eq!(use_held(&mut host, player, Hand::Main, &staff), Some(false));
    assert_eq!(*host.queries.borrow(), [TARGET]);
    assert!(host.strikes.is_empty());
    assert_eq!(
        (host.food, host.held.as_ref(), host.messages),
        (20, Some(&staff), 1)
    );

    host.allowed = true;
    assert_eq!(use_held(&mut host, player, Hand::Main, &staff), Some(true));
    assert_eq!(*host.queries.borrow(), [TARGET, TARGET]);
    assert_eq!(host.strikes, [TARGET.map(f64::from)]);
    assert_eq!(
        (
            host.food,
            remaining(host.held.as_ref().unwrap()),
            host.messages
        ),
        (16, 7, 1)
    );
}
