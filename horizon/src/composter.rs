// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashMap, sync::Mutex};

use mod_api::{
    BlockState, BlockUpdateFlags, Hand, Host, HostTransaction, ItemPlace, ItemResource, ItemStack,
    LevelRef, PlayerRef, ServerHost, SoundSource, Tag, Text, Transaction, UseOnEvent,
};

use crate::{
    items::{Kind, SlimefunItem, text},
    network_runtime, output_chest,
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

type Position = [i32; 3];
const PULSE_INTERVAL: i64 = 30;
const PULSES: u8 = 10;
const FINISH_DELAY: i64 = 20;

pub(crate) const COMPOSTER: Kind = Kind::Slimefun(&SlimefunItem {
    id: "COMPOSTER",
    material: "minecraft:cauldron",
    name: "Composter",
    color: "green",
    tier: None,
    texture: None,
    juice: None,
});

pub(crate) const RESEARCH: Research = Research {
    id: "composter",
    name: "Composting Dirt",
    cost: 3,
};

const SLAB: Option<Kind> = Some(Kind::Vanilla("minecraft:oak_slab"));

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        SLAB,
        None,
        SLAB,
        SLAB,
        None,
        SLAB,
        SLAB,
        Some(Kind::Vanilla("minecraft:cauldron")),
        SLAB,
    ],
    output: Stack {
        kind: COMPOSTER,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore(id: &str) -> Option<Tag> {
    (id == COMPOSTER.id()).then(|| {
        Tag::List(vec![
            text("", "green", false),
            text("Can convert various Materials over Time...", "green", true),
        ])
    })
}

pub(crate) fn refuses_placement(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> bool {
    if !COMPOSTER.matches(host, event.stack) {
        return false;
    }
    let Some(player) = host.player(event.player) else {
        return true;
    };
    if research::can_use(host, player, &RESEARCH) {
        return false;
    }
    research_hint(host, player);
    true
}

fn research_hint(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = player.tell(
        host,
        Text::literal("Unlock the Composter with /sf research composter."),
    );
}

pub(crate) const MATERIAL_RECIPES: &[(&str, u32, &str)] = &[
    ("minecraft:stone", 4, "minecraft:netherrack"),
    ("minecraft:sand", 2, "minecraft:soul_sand"),
    ("minecraft:wheat", 4, "minecraft:nether_wart"),
];

fn recipe(host: &dyn Host, input: &ItemStack) -> Option<(u32, Kind, String)> {
    if input.components().is_some() || input.damage_component().is_some() {
        return None;
    }
    let material = input.item().id(host);
    let (count, product) = match MATERIAL_RECIPES.iter().find(|&&(name, _, _)| name == material) {
        Some(&(_, count, product)) => (count, product),
        None => {
            let block = host.block_key(&material)?;
            if ["minecraft:leaves", "minecraft:saplings"]
                .iter()
                .all(|tag| host.block_in_tag(block, tag) != Some(true))
            {
                return None;
            }
            (8, "minecraft:dirt")
        }
    };
    if input.count() < count {
        return None;
    }
    Some((count, Kind::Vanilla(product), material))
}

fn consume(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    player: PlayerRef,
    input: &ItemStack,
    mut count: u32,
) -> bool {
    let resource = ItemResource::of(input);
    // CraftInventory.removeItem visits storage in slot order, including the
    // hotbar. The clicked stack still has to contain a full recipe on its own.
    for slot in 0..36 {
        let place = ItemPlace::Strict(Box::new(ItemPlace::PlayerSlot {
            player: player.uuid(),
            slot,
        }));
        count -= place.extract(host, tx, &resource, count);
        if count == 0 {
            return true;
        }
    }
    false
}

struct Job {
    anchor: Position,
    above: Position,
    finish_sound: Option<PlayerRef>,
    effect: BlockState,
    product: ItemStack,
    due: i64,
    pulses: u8,
}

#[derive(Default)]
pub(crate) struct Composters(Mutex<HashMap<LevelRef, Vec<Job>>>);

impl Composters {
    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }

    pub(crate) fn use_at(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        anchor: Position,
        use_item: bool,
    ) -> bool {
        let Some(state) = network_runtime::manual_composter_at(host, level, anchor) else {
            return false;
        };
        if !use_item {
            return true;
        }
        if !research::can_use(host, player, &RESEARCH) {
            research_hint(host, player);
            return true;
        }
        // Use the actual player and marked cauldron for land authorization.
        if level.replace_block_for_player(
            host,
            player,
            anchor,
            state,
            state,
            BlockUpdateFlags::CLIENTS,
        ) != Ok(true)
        {
            return true;
        }
        let hand = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand: Hand::Main,
        };
        let Some((input, count, product, effect)) = hand.stack(host).and_then(|input| {
            let (count, product, material) = recipe(host, &input)?;
            // Paper tests Material.isBlock. Wheat has both an item and a crop
            // block entry, so it uses the crop state just like other block inputs.
            let effect = host
                .block_state_id(&material)
                .or_else(|| host.block_state_id("minecraft:hay_block"))?;
            Some((input, count, product.stack(host, 1)?, effect))
        }) else {
            let _ = player.tell(
                host,
                Text::literal("Hold a full Composter recipe in your main hand."),
            );
            return true;
        };
        let [x, y, z] = anchor;
        let Some(above) = y.checked_add(1).map(|above| [x, above, z]) else {
            return true;
        };
        let Some(now) = level.game_time(host) else {
            return true;
        };
        if host
            .transaction(|host, tx| {
                consume(host, tx, player, &input, count)
                    .then_some(())
                    .ok_or(())
            })
            .is_ok()
        {
            let job = Job {
                anchor,
                above,
                finish_sound: Some(player),
                effect,
                product,
                due: now.saturating_add(PULSE_INTERVAL),
                pulses: PULSES,
            };
            self.0.lock().unwrap().entry(level).or_default().push(job);
        }
        true
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef, rolls: &Rolls) {
        let Some(now) = level.game_time(host) else {
            return;
        };
        let Some(mut jobs) = self.0.lock().unwrap().remove(&level) else {
            return;
        };
        jobs.retain_mut(|job| now < job.due || job.advance(host, level, rolls));
        if !jobs.is_empty() {
            let mut pending = self.0.lock().unwrap();
            jobs.append(pending.entry(level).or_default());
            pending.insert(level, jobs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, prelude::PlayersHost, testing::FakeHost};

    fn host() -> FakeHost {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:stone",
                    "minecraft:sand",
                    "minecraft:wheat",
                    "minecraft:cauldron",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        host
    }

    #[test]
    fn recipes_require_a_complete_undecorated_clicked_stack_and_use_paper_materials() {
        let host = host();
        for (id, count, product, appearance) in [
            (
                "minecraft:stone",
                4,
                "minecraft:netherrack",
                "minecraft:stone",
            ),
            ("minecraft:sand", 2, "minecraft:soul_sand", "minecraft:sand"),
            (
                "minecraft:wheat",
                4,
                "minecraft:nether_wart",
                "minecraft:wheat",
            ),
        ] {
            let item = Kind::Vanilla(id);
            assert!(recipe(&host, &item.stack(&host, count - 1).unwrap()).is_none());
            let mut full = item.stack(&host, count + 1).unwrap();
            assert_eq!(
                recipe(&host, &full),
                Some((count, Kind::Vanilla(product), appearance.into()))
            );
            COMPOSTER.decorate(&mut full);
            assert!(recipe(&host, &full).is_none());
        }
    }

    #[test]
    fn paper_inventory_order_removes_only_enough_and_rolls_back_failed_payment() {
        let mut host = host();
        let wheat =
            |host: &FakeHost, count| Kind::Vanilla("minecraft:wheat").stack(host, count).unwrap();
        let first = wheat(&host, 2);
        let selected = wheat(&host, 7);
        let fixture = host.join(1, 12);
        fixture.selected_slot = 5;
        fixture.slots.insert(0, first);
        fixture.slots.insert(5, selected.clone());
        let player = host.player(1).unwrap();
        let pay = |host: &mut FakeHost, count| {
            host.transaction(|host, tx| {
                consume(host, tx, player, &selected, count)
                    .then_some(())
                    .ok_or(())
            })
        };
        assert_eq!(pay(&mut host, 10), Err(()));
        assert_eq!(host.players.get(1).unwrap().slots[&0].count(), 2);
        assert_eq!(host.players.get(1).unwrap().slots[&5].count(), 7);
        assert_eq!(pay(&mut host, 4), Ok(()));
        assert!(!host.players.get(1).unwrap().slots.contains_key(&0));
        assert_eq!(host.players.get(1).unwrap().slots[&5].count(), 5);
    }
}

impl Job {
    fn advance(&mut self, host: &mut dyn ServerHost, level: LevelRef, rolls: &Rolls) -> bool {
        if self.pulses > 0 {
            let _ = level.block_break_effect(host, self.anchor, self.effect);
            self.pulses -= 1;
            self.due = self.due.saturating_add(if self.pulses == 0 {
                FINISH_DELAY
            } else {
                PULSE_INTERVAL
            });
            return true;
        }
        if let Some(player) = self.finish_sound.take() {
            player.play_sound(
                host,
                "minecraft:entity.arrow.hit_player",
                SoundSource::Players,
                1.0,
                1.0,
            );
        }
        if output_chest::try_deposit_checked(host, level, self.anchor, &self.product)
            || self.drop_above(host, level, rolls)
        {
            return false;
        }
        // A temporarily unavailable or protected destination must not consume the output.
        self.due = self.due.saturating_add(FINISH_DELAY);
        true
    }

    fn drop_above(&self, host: &mut dyn ServerHost, level: LevelRef, rolls: &Rolls) -> bool {
        if !level.allow_inventory_transfer(host, self.anchor, self.above, &self.product) {
            return false;
        }
        let offset = |roll| f64::from(roll) / 2_000_000.0 - 0.25;
        let mut at = self.above.map(|axis| {
            f64::from(axis) + offset(rolls.next(host, level, 1_000_000).unwrap_or(500_000))
        });
        // CraftWorld.dropItemNaturally offsets the requested corner and subtracts half
        // the item entity's height. Check the real destination as well as the named one.
        at[1] -= 0.125;
        let physical = at.map(|axis| axis.floor() as i32);
        if physical != self.above
            && physical != self.anchor
            && !level.allow_inventory_transfer(host, self.anchor, physical, &self.product)
        {
            at = self.above.map(|axis| f64::from(axis) + 0.5);
        }
        level
            .spawn_item(host, at[0], at[1], at[2], self.product.clone(), 10)
            .is_ok_and(|item| item.is_some())
    }
}
