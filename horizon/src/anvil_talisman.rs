// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{Host, HostTransaction, ItemStack, PlayerRef, ServerHost, Tag, TaskId};

use super::{
    COMMON_TALISMAN, LUMP_3, Talisman, Talismans, consume_carried, infused, learned, notify, recipe,
};
use crate::{
    hologram_projector::colored_lore,
    items::{Kind, item},
    recipes::{CraftingRecipe, Research},
};

item!(
    ANVIL_TALISMAN,
    "emerald",
    "Talisman of the Anvil",
    "green",
    None,
    None
);
item!(
    ENDER_ANVIL_TALISMAN,
    "emerald",
    "Ender Talisman of the Anvil",
    "dark_purple",
    None,
    None
);

pub(super) const RESEARCH: Research = Research {
    id: "anvil_talisman",
    name: "Talisman of the Anvil",
    cost: 18,
};

const ANVIL: Option<Kind> = Some(Kind::Vanilla("minecraft:anvil"));
pub(crate) const CRAFTING: [CraftingRecipe; 2] = [
    recipe(
        ANVIL_TALISMAN,
        &RESEARCH,
        4,
        [
            LUMP_3,
            None,
            LUMP_3,
            ANVIL,
            Some(COMMON_TALISMAN),
            ANVIL,
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_ANVIL_TALISMAN, ANVIL_TALISMAN, 4),
];

const TALISMAN: Talisman = Talisman {
    normal: ANVIL_TALISMAN,
    ender: ENDER_ANVIL_TALISMAN,
    research: &RESEARCH,
    chance: 100,
    message: "Your Talisman saved your tool from breaking",
    lore: &[],
};

pub(super) fn lore(id: &str) -> Option<Tag> {
    let mut lines = if id == ANVIL_TALISMAN.id() {
        Vec::new()
    } else if id == ENDER_ANVIL_TALISMAN.id() {
        vec![colored_lore("&7&oEnder Infused"), colored_lore("")]
    } else {
        return None;
    };
    lines.extend(
        [
            "",
            "&fEach Talisman can prevent",
            "&f1 Tool from breaking, but will then",
            "&fbe consumed",
            "",
            "&4&lWARNING:",
            "&4This Talisman does not work on",
            "&4Tools which are too powerful",
            "&4due to their complexity",
        ]
        .into_iter()
        .map(colored_lore),
    );
    Some(Tag::List(lines))
}

#[derive(Default)]
pub(crate) struct Repairs(Mutex<BTreeMap<TaskId, Repair>>);

struct Repair {
    player: PlayerRef,
    slot: u32,
    stack: ItemStack,
}

impl Repairs {
    pub(crate) fn broke(
        &self,
        host: &mut dyn ServerHost,
        talismans: &Talismans,
        uuid: u128,
        inventory_slot: u32,
        stack: &ItemStack,
    ) {
        let Some(player) = host.player(uuid) else {
            return;
        };
        if !learned(host, player, &TALISMAN) {
            return;
        }

        // Anvil succeeds on every Paper roll. Consumption precedes identifying the
        // target, but a supported target only pays if its next-tick job can be queued.
        let Ok(pending) = host.transaction(|host, _| -> Result<_, ()> {
            consume_carried(host, player, &TALISMAN)?;
            let Some(slot) = repair_slot(host, player, inventory_slot, stack) else {
                return Ok(None);
            };
            let id = host.schedule_task(1, 0).map_err(|_| ())?;
            let mut stack = stack.clone();
            stack.set_damage(0);
            Ok(Some((
                id,
                Repair {
                    player,
                    slot,
                    stack,
                },
            )))
        }) else {
            return;
        };
        if let Some((id, repair)) = pending {
            self.0.lock().unwrap().insert(id, repair);
        }
        let actionbar = talismans.0.lock().unwrap().actionbar;
        notify(host, player, &TALISMAN, actionbar);
    }

    pub(crate) fn due(&self, host: &mut dyn ServerHost, owner: &str, id: TaskId) {
        if owner != "slimefun" {
            return;
        }
        let repair = self.0.lock().unwrap().remove(&id);
        if let Some(Repair {
            player,
            slot,
            stack,
        }) = repair
            && host.player(player.uuid()) == Some(player)
        {
            let _ = player.set_inventory_slot(host, slot, Some(stack));
        }
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

fn repair_slot(host: &dyn Host, player: PlayerRef, broken: u32, stack: &ItemStack) -> Option<u32> {
    // The captured hotbar cell was the selected hand when the wear landed.
    if broken < 9 {
        return Some(broken);
    }
    let selected = player
        .selected_slot(host)
        .filter(|slot| *slot < 9)
        .map(|slot| slot as u32);
    let inventory = player.inventory_slots(host).unwrap_or_default();
    // Paper checked the main hand, offhand, then head through feet before the
    // shrink. Supply the snapshot at its original cell for that comparison.
    selected
        .into_iter()
        .chain([40, 39, 38, 37, 36])
        .find(|slot| {
            *slot == broken
                || inventory
                    .iter()
                    .any(|(at, held)| at == slot && held == stack)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{
        __private::game::{BackendMutRefOf, BackendRefOf},
        BackendMutRef, BackendRef, Compound, Identifier, PlayerDataRefusal, ScheduleError, Text,
        TextFormError, TransactionSeal,
        prelude::{
            ConfigHost, ItemsHost, ModStateServerHost, PlayersBackend, PlayersBackendMut,
            PlayersClientBackend, PlayersHost,
        },
        testing::{FakeHost, FakeJournal},
        wire::{Reader, Writer},
    };

    const PLAYER: u128 = 23;
    const TASK: TaskId = TaskId(67);
    type Inventory = BTreeMap<u32, ItemStack>;

    #[derive(Default)]
    struct AnvilHost {
        identity: FakeHost,
        slots: Inventory,
        ender: Inventory,
        journal: FakeJournal<(Inventory, Inventory)>,
        learned: bool,
        refuse_task: bool,
        online: bool,
        selected: usize,
        scheduled: Vec<u32>,
        messages: usize,
    }

    impl Host for AnvilHost {
        fn backend(&self) -> Option<BackendRef<'_>> {
            Some(BackendRef::of(self))
        }
    }
    impl ServerHost for AnvilHost {
        fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
            Some(BackendMutRef::of(self))
        }
    }
    mod_api::impl_host_areas!(AnvilHost, except(config, items, players));
    mod_api::impl_server_host_areas!(AnvilHost, except(mod_state));
    mod_api::impl_backend_areas!(AnvilHost, except(players));
    mod_api::impl_backend_mut_areas!(AnvilHost, except(players));
    impl mod_api::Backend for AnvilHost {}
    impl mod_api::BackendMut for AnvilHost {}
    impl PlayersClientBackend for AnvilHost {}

    impl ConfigHost for AnvilHost {
        fn dimension(&self) -> &'static str {
            "minecraft:overworld"
        }
    }

    impl ItemsHost for AnvilHost {
        fn item_key(&self, id: &Identifier) -> Option<mod_api::ItemKey> {
            self.identity.item_key(id)
        }
    }

    impl PlayersHost for AnvilHost {
        fn player(&self, uuid: u128) -> Option<PlayerRef> {
            self.online.then(|| self.identity.player(uuid)).flatten()
        }
    }

    impl PlayersBackend for AnvilHost {
        fn player_selected_slot(&self, _: PlayerRef) -> Option<usize> {
            Some(self.selected)
        }

        fn player_inventory_slots(&self, _: PlayerRef) -> Option<Vec<(u32, ItemStack)>> {
            Some(self.slots.clone().into_iter().collect())
        }

        fn player_ender_chest(&self, _: PlayerRef) -> Option<Vec<(u32, ItemStack)>> {
            Some(self.ender.clone().into_iter().collect())
        }

        fn player_persistent_data(&self, _: PlayerRef) -> Option<Compound> {
            Some(Compound::from([(
                "PlayerPersisted".into(),
                Tag::compound([("slimefun:anvil_talisman", Tag::Byte(i8::from(self.learned)))]),
            )]))
        }
    }

    impl PlayersBackendMut for AnvilHost {
        fn player_set_inventory_slot(
            &mut self,
            player: PlayerRef,
            slot: u32,
            stack: Option<ItemStack>,
        ) -> Result<(), PlayerDataRefusal> {
            assert_eq!(player.uuid(), PLAYER);
            self.slots.remove(&slot);
            self.slots.extend(stack.map(|stack| (slot, stack)));
            Ok(())
        }

        fn player_set_ender_chest_slot(
            &mut self,
            player: PlayerRef,
            slot: u32,
            stack: Option<ItemStack>,
        ) -> Result<(), PlayerDataRefusal> {
            assert_eq!(player.uuid(), PLAYER);
            self.ender.remove(&slot);
            self.ender.extend(stack.map(|stack| (slot, stack)));
            Ok(())
        }

        fn player_action_bar(&mut self, _: PlayerRef, _: Text) -> Result<bool, TextFormError> {
            self.messages += 1;
            Ok(true)
        }
    }

    impl ModStateServerHost for AnvilHost {
        fn transaction_open(&mut self) -> TransactionSeal {
            self.journal.open((self.slots.clone(), self.ender.clone()))
        }

        fn transaction_close(&mut self, seal: &TransactionSeal, commit: bool) {
            if let Some(state) = self.journal.close(seal, commit) {
                (self.slots, self.ender) = state;
            }
        }

        fn schedule_task(&mut self, delay_ticks: u32, _: u64) -> Result<TaskId, ScheduleError> {
            if self.refuse_task {
                return Err(ScheduleError::TooMany);
            }
            self.scheduled.push(delay_ticks);
            Ok(TASK)
        }
    }

    fn fixture() -> (AnvilHost, ItemStack) {
        let mut host = AnvilHost {
            online: true,
            learned: true,
            selected: 4,
            ..Default::default()
        };
        host.identity
            .registries
            .resolve_items(
                ["minecraft:emerald", "minecraft:diamond_pickaxe"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        host.identity.join(PLAYER, 100);
        let mut snapshot = Kind::Vanilla("minecraft:diamond_pickaxe")
            .stack(&host, 1)
            .unwrap();
        snapshot
            .apply_components(&Compound::from([
                ("minecraft:damage".into(), Tag::Int(1561)),
                (
                    "minecraft:custom_name".into(),
                    Tag::String("Archived pick".into()),
                ),
                (
                    "minecraft:custom_data".into(),
                    Tag::compound([("Paper", Tag::IntArray(vec![7, -1, 2]))]),
                ),
                (
                    "minecraft:lore".into(),
                    Tag::List(vec![Tag::String("Original metadata".into())]),
                ),
                ("!minecraft:tool".into(), Tag::Compound(Compound::new())),
            ]))
            .unwrap();
        let mut wire = Writer::new();
        wire.put(&snapshot);
        let decoded = Reader::decode(wire.bytes()).unwrap();
        assert_eq!(snapshot, decoded);
        (host, decoded)
    }

    #[test]
    fn repair_uses_the_captured_slot_one_tick_later_and_preserves_the_snapshot() {
        let (mut host, snapshot) = fixture();
        let ordinary = ANVIL_TALISMAN.stack(&host, 2).unwrap();
        let ender = ENDER_ANVIL_TALISMAN.stack(&host, 2).unwrap();
        host.slots.insert(0, ordinary);
        host.ender.insert(0, ender.clone());
        let talismans = Talismans::default();
        let repairs = Repairs::default();

        repairs.broke(&mut host, &talismans, PLAYER, 4, &snapshot);
        assert!(!host.slots.contains_key(&4));
        assert_eq!(host.scheduled, [1]);
        assert_eq!(host.slots[&0].count(), 1);
        assert_eq!(host.ender[&0], ender);
        assert_eq!(host.messages, 1);

        host.selected = 2;
        host.slots.insert(4, ender.clone());
        repairs.due(&mut host, "other-mod", TASK);
        assert_eq!(host.slots[&4], ender);
        repairs.due(&mut host, "slimefun", TASK);
        let mut expected = snapshot.clone();
        expected.set_damage(0);
        assert_eq!(host.slots[&4], expected);
        assert!(!host.slots.contains_key(&2));
        assert_eq!(snapshot.damage(), 1561);

        host.slots.remove(&4);
        repairs.due(&mut host, "slimefun", TASK);
        assert!(!host.slots.contains_key(&4));
    }

    #[test]
    fn ender_payment_respects_research_and_rolls_back_if_scheduling_fails() {
        let (mut host, snapshot) = fixture();
        let talismans = Talismans::default();
        let repairs = Repairs::default();
        host.slots
            .insert(36, ANVIL_TALISMAN.stack(&host, 2).unwrap());
        host.ender
            .insert(1, ENDER_ANVIL_TALISMAN.stack(&host, 2).unwrap());

        host.learned = false;
        repairs.broke(&mut host, &talismans, PLAYER, 38, &snapshot);
        assert!(host.scheduled.is_empty());
        assert_eq!(host.ender[&1].count(), 2);

        host.learned = true;
        host.refuse_task = true;
        repairs.broke(&mut host, &talismans, PLAYER, 38, &snapshot);
        assert!(host.scheduled.is_empty());
        assert_eq!(host.ender[&1].count(), 2);
        assert_eq!(host.messages, 0);

        host.refuse_task = false;
        repairs.broke(&mut host, &talismans, PLAYER, 38, &snapshot);
        assert_eq!(host.scheduled, [1]);
        assert_eq!(host.slots[&36].count(), 2);
        assert_eq!(host.ender[&1].count(), 1);
        host.online = false;
        repairs.due(&mut host, "slimefun", TASK);
        assert!(!host.slots.contains_key(&38));
    }

    #[test]
    fn uses_paper_slot_order_and_ignores_non_equipped_storage_cells() {
        let (mut host, snapshot) = fixture();
        let player = host.player(PLAYER).unwrap();
        assert_eq!(repair_slot(&host, player, 2, &snapshot), Some(2));
        assert_eq!(repair_slot(&host, player, 40, &snapshot), Some(40));
        assert_eq!(repair_slot(&host, player, 38, &snapshot), Some(38));
        assert_eq!(repair_slot(&host, player, 14, &snapshot), None);
        assert_eq!(repair_slot(&host, player, 41, &snapshot), None);
        host.slots.insert(40, snapshot.clone());
        assert_eq!(repair_slot(&host, player, 38, &snapshot), Some(40));
        host.slots.insert(4, snapshot.clone());
        assert_eq!(repair_slot(&host, player, 38, &snapshot), Some(4));
    }

    #[test]
    fn recipes_produce_four_consumable_talismans_and_use_paper_researches() {
        let [normal, ender] = &CRAFTING;
        assert_eq!((normal.output.count, ender.output.count), (4, 4));
        assert_eq!(normal.research.id, "anvil_talisman");
        assert_eq!(normal.research.cost, 18);
        assert_eq!(normal.inputs[3..6], [ANVIL, Some(COMMON_TALISMAN), ANVIL]);
        assert_eq!(ender.research.id, "ender_talismans");
        assert_eq!(ender.inputs[4], Some(ANVIL_TALISMAN));
    }
}
