// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use mod_api::{
    __private::game::{BackendMutRefOf, BackendRefOf},
    BackendMutRef, BackendRef, BlockKey, BlockState, BlockStateRecord, EntityRef, GameMode,
    Identifier, ItemKey, LegacyRandom, ModEntry, RandomSource, Registrar, RegistrationContext,
    prelude::{
        ItemsHost, KeysBackend, LevelsBackend, LevelsBackendMut, PlayersBackend, PlayersBackendMut,
        PlayersHost,
    },
    testing::FakeHost,
};
use std::sync::{Arc, OnceLock};

const TARGET: [i32; 3] = [21, 66, -8];

struct Blocks(Arc<OnceLock<[BlockKey; 2]>>);

#[mod_api::__subscriptions]
impl mod_api::Hooks for Blocks {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        self.0
            .set([
                registrar.block_key("minecraft:gravel"),
                registrar.block_key("minecraft:soul_soil"),
            ])
            .unwrap();
    }
}

struct PanHost {
    game: FakeHost,
    block: BlockKey,
    name: &'static str,
    rng: LegacyRandom,
    allowed: bool,
    present: bool,
    drops: Vec<([f64; 3], ItemStack, u32)>,
}

impl Host for PanHost {
    fn backend(&self) -> Option<BackendRef<'_>> {
        Some(BackendRef::of(self))
    }
}
impl ServerHost for PanHost {
    fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
        Some(BackendMutRef::of(self))
    }
}
mod_api::impl_host_areas!(PanHost, except(items));
mod_api::impl_server_host_areas!(PanHost);
mod_api::impl_backend_areas!(PanHost, except(keys, levels, players));
mod_api::impl_backend_mut_areas!(PanHost, except(levels, players));
impl mod_api::Backend for PanHost {}
impl mod_api::BackendMut for PanHost {}

impl ItemsHost for PanHost {
    fn item_key(&self, id: &Identifier) -> Option<ItemKey> {
        self.game.item_key(id)
    }
}
impl KeysBackend for PanHost {
    fn block_key_id(&self, key: BlockKey) -> String {
        assert_eq!(key, self.block);
        self.name.into()
    }
    fn item_key_id(&self, key: ItemKey) -> String {
        self.game.item_key_id(key)
    }
}
impl LevelsBackend for PanHost {
    fn level_block_state(&self, level: LevelRef, pos: [i32; 3]) -> Option<BlockState> {
        assert_eq!((level, pos), (self.game.level.level(), TARGET));
        self.present
            .then(|| BlockStateRecord::new(self.block, 0).into())
    }
    fn level_random(&self, _: LevelRef) -> Option<LegacyRandom> {
        Some(self.rng.clone())
    }
}
impl LevelsBackendMut for PanHost {
    fn level_set_random(&mut self, _: LevelRef, rng: LegacyRandom) -> bool {
        self.rng = rng;
        true
    }
    fn level_spawn_item(
        &mut self,
        _: LevelRef,
        x: f64,
        y: f64,
        z: f64,
        stack: ItemStack,
        delay: u32,
    ) -> Result<Option<EntityRef>, mod_api::StackWriteError> {
        assert!(!self.present);
        self.drops.push(([x, y, z], stack, delay));
        Ok(None)
    }
    fn level_play_sound(
        &mut self,
        _: LevelRef,
        _: &str,
        _: [f64; 3],
        _: SoundSource,
        _: f32,
        _: f32,
    ) -> bool {
        assert!(!self.present);
        true
    }
}
impl PlayersBackend for PanHost {
    fn player_game_mode(&self, player: PlayerRef) -> Option<GameMode> {
        assert_eq!(Some(player), self.game.player(1));
        Some(GameMode::Creative)
    }
}
impl mod_api::prelude::PlayersClientBackend for PanHost {}
impl PlayersBackendMut for PanHost {
    fn player_remove_block_without_drops(
        &mut self,
        player: PlayerRef,
        pos: [i32; 3],
        expected: BlockState,
    ) -> bool {
        assert_eq!(Some(player), self.game.player(1));
        assert_eq!(
            Some(expected),
            self.level_block_state(self.game.level.level(), pos)
        );
        if self.allowed {
            self.present = false;
        }
        self.allowed
    }
}

#[test]
fn protected_panning_preserves_block_and_random_stream_while_allowed_drops_use_papers_offsets() {
    let blocks = Arc::new(OnceLock::new());
    mod_api::__private::part_tests::install_parts_fixture(&[ModEntry::new(
        "pan_fixture",
        Box::new(Blocks(blocks.clone())),
    )])
    .unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let [gravel, soul_soil] = *blocks.get().unwrap();
    for (pan, block, name, reward) in [
        (&PANS[0], gravel, "minecraft:gravel", "minecraft:flint"),
        (
            &PANS[1],
            soul_soil,
            "minecraft:soul_soil",
            "minecraft:quartz",
        ),
    ] {
        let mut game = FakeHost::new();
        game.registries
            .resolve_items(
                ["minecraft:flint", "minecraft:quartz", "minecraft:bowl"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        game.join(1, 1);
        let player = game.player(1).unwrap();
        let level = game.level.level();
        let mut host = PanHost {
            game,
            block,
            name,
            rng: LegacyRandom::new(881),
            allowed: false,
            present: true,
            drops: Vec::new(),
        };
        let bowl = Kind::Vanilla("minecraft:bowl").stack(&host, 1).unwrap();
        assert!(held(&host, &bowl).is_none());
        let pan_item = pan.item.stack(&host, 1).unwrap();
        let marked = held(&host, &pan_item).unwrap();
        assert_eq!(marked.item, pan.item);
        assert!(!marked.allows_entity_use(&mut host, player, "minecraft:mooshroom"));
        assert!(marked.allows_entity_use(&mut host, player, "minecraft:item_frame"));
        assert!(marked.allows_entity_use(&mut host, player, "minecraft:glow_item_frame"));

        let rolls = Rolls::default();
        let expected_rolls = Rolls::default();
        let weights = Weights {
            overworld: [1, 0, 0, 0],
            nether: [1, 0, 0, 0, 0, 0],
        };

        pan.use_on(&mut host, player, level, TARGET, &rolls, &weights);
        assert!(host.present && host.drops.is_empty());
        assert_eq!(
            rolls.next(&host, level, SAMPLE_COUNT),
            expected_rolls.next(&host, level, SAMPLE_COUNT),
        );

        let mut world_rng = host.rng.clone();
        let mut expected =
            TARGET.map(|axis| f64::from(axis) + world_rng.next_double() * 0.5 - 0.25);
        expected[1] -= 0.125;
        let _ = expected_rolls.next(&host, level, SAMPLE_COUNT);
        host.allowed = true;
        pan.use_on(&mut host, player, level, TARGET, &rolls, &weights);
        assert!(!host.present);
        let [(at, stack, delay)] = host.drops.as_slice() else {
            panic!("expected one drop")
        };
        assert_eq!(
            (*at, stack.item().id(&host).as_str(), stack.count(), *delay),
            (expected, reward, 1, 10)
        );
        assert_eq!(host.rng.next_double(), world_rng.next_double());
        assert_eq!(
            rolls.next(&host, level, SAMPLE_COUNT),
            expected_rolls.next(&host, level, SAMPLE_COUNT),
        );
    }
}
