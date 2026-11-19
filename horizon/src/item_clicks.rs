// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    Hand, HostTransaction, ItemKey, ItemPlace, ItemResource, ItemStack, PlayerRef, ServerHost,
};

use crate::items::item_id;

// A stock client also sends its air-use packet when its own block
// prediction passes. That packet can follow the first across server ticks.
const FALLTHROUGH_MS: u64 = 150;

#[derive(Default)]
pub(crate) struct ItemClicks(Mutex<BTreeMap<(u128, bool), BlockUse>>);

struct BlockUse {
    time: u64,
    item: ItemKey,
    id: String,
}

impl ItemClicks {
    pub(crate) fn record_block_use(
        &self,
        host: &dyn ServerHost,
        player: u128,
        hand: Hand,
        stack: &ItemStack,
    ) {
        let Some(id) = item_id(stack) else { return };
        self.record(
            host.server_monotonic_millis(),
            player,
            hand,
            stack.item(),
            id,
        );
    }

    fn record(&self, time: u64, player: u128, hand: Hand, item: ItemKey, id: String) {
        let key = (player, hand == Hand::Off);
        self.0
            .lock()
            .unwrap()
            .insert(key, BlockUse { time, item, id });
    }

    pub(crate) fn consume_fallthrough(
        &self,
        host: &dyn ServerHost,
        player: u128,
        hand: Hand,
        stack: &ItemStack,
    ) -> bool {
        self.consume(host.server_monotonic_millis(), player, hand, stack)
    }

    fn consume(&self, now: u64, player: u128, hand: Hand, stack: &ItemStack) -> bool {
        let key = (player, hand == Hand::Off);
        let Some(previous) = self.0.lock().unwrap().remove(&key) else {
            return false;
        };
        now.saturating_sub(previous.time) <= FALLTHROUGH_MS
            && previous.item == stack.item()
            && item_id(stack).as_deref() == Some(previous.id.as_str())
    }

    pub(crate) fn logout(&self, player: u128) {
        let mut map = self.0.lock().unwrap();
        map.remove(&(player, false));
        map.remove(&(player, true));
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

pub(crate) fn consume_one(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    item: &ItemStack,
) -> bool {
    let held = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    };
    host.transaction(|host, tx| {
        (held.extract(host, tx, &ItemResource::of(item), 1) == 1)
            .then_some(())
            .ok_or(())
    })
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medical::{BANDAGE, RAG};
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn right_click_consumption_removes_exactly_one_identical_item_from_the_clicked_hand() {
        use crate::snacks::MAGIC_SUGAR;
        use mod_api::prelude::PlayersHost;

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(MAGIC_SUGAR.material()).unwrap()])
            .unwrap();
        let plain_sugar = crate::items::Kind::Vanilla("minecraft:sugar")
            .stack(&host, 2)
            .unwrap();
        let enchanted_sugar = MAGIC_SUGAR.stack(&host, 2).unwrap();
        let joined = host.join(59, 15);
        joined.slots.insert(0, plain_sugar.clone());
        joined.slots.insert(-106, enchanted_sugar.clone());
        let player = host.player(59).unwrap();

        assert!(!consume_one(
            &mut host,
            player,
            Hand::Main,
            &enchanted_sugar
        ));
        assert!(consume_one(&mut host, player, Hand::Off, &enchanted_sugar));
        let player = host.players.get(59).unwrap();
        assert_eq!(player.slots.get(&0), Some(&plain_sugar));
        assert_eq!(
            player.slots.get(&-106),
            MAGIC_SUGAR.stack(&host, 1).as_ref()
        );
    }

    #[test]
    fn the_same_hand_and_identity_suppress_only_the_following_predicted_packet() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(RAG.material()).unwrap()])
            .unwrap();
        let rag = RAG.stack(&host, 2).unwrap();
        let next = RAG.stack(&host, 1).unwrap();
        let bandage = BANDAGE.stack(&host, 1).unwrap();
        let clicks = ItemClicks::default();
        let record = || clicks.record(1000, 1, Hand::Main, rag.item(), RAG.id().to_owned());
        record();
        assert!(!clicks.consume(1005, 2, Hand::Main, &rag));
        assert!(!clicks.consume(1005, 1, Hand::Off, &rag));
        assert!(clicks.consume(1100, 1, Hand::Main, &next));
        assert!(!clicks.consume(1101, 1, Hand::Main, &next));
        record();
        assert!(!clicks.consume(1151, 1, Hand::Main, &next));
        record();
        assert!(!clicks.consume(1005, 1, Hand::Main, &bandage));
        record();
        clicks.logout(1);
        assert!(!clicks.consume(1010, 1, Hand::Main, &next));
    }
}
