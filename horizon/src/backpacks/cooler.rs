// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    ActiveEffect, Host, HostTransaction, ItemStack, LivingRef, PlayerRef, ScheduledTaskEvent,
    ServerHost, SoundSource, Tag, TaskId, Text,
};

use super::BackpackStore;
use super::{BackpackId, BackpackMenus, Binding, GlobalBackpacks, binding, live_bag};
use crate::{
    items::{HeadTexture, item},
    research,
    thermal_machines::COOLING,
};

item!(
    COOLER,
    "player_head",
    "Cooler",
    "aqua",
    None,
    Some(HeadTexture {
        profile: [-373878764, 1246556139, 262893125, -2063993581],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2Q0YzE1NzI1ODRlYjVkZTIyOWRlOWY1YTRmNzc5ZDBhYWNiYWZmZDMzYmNiMzNlYjQ1MzZhNmEyYmM2YTEifX19",
    })
);

pub(super) const SIZE: u32 = 27;

#[derive(Default)]
pub(crate) struct CoolerFeed(Mutex<BTreeMap<TaskId, (PlayerRef, BackpackId)>>);

impl CoolerFeed {
    pub(crate) fn check(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        let Some(slots) = player.inventory_slots(host) else {
            return;
        };
        let mut coolers = slots
            .into_iter()
            .take_while(|(slot, _)| *slot <= 40)
            .map(|(_, stack)| stack)
            .filter(|stack| COOLER.matches(host, stack))
            .peekable();
        if coolers.peek().is_none() {
            return;
        }
        if !research::can_use(host, player, &COOLING) {
            let _ = player.tell(
                host,
                Text::literal("Unlock this research with /sf research cooler."),
            );
            return;
        }
        // Record identities are captured at the event. Paper schedules a task
        // for each Cooler, including a borrowed one in standalone mode.
        let ids: Vec<_> = coolers
            .filter_map(|stack| match binding(&stack) {
                Ok(Binding::Named(id)) => Some(id),
                _ => None,
            })
            .collect();
        for id in ids {
            if live_bag(host, id, SIZE).ok().flatten().is_some()
                && let Ok(task) = host.schedule_task(1, 0)
            {
                self.0.lock().unwrap().insert(task, (player, id));
            }
        }
    }

    pub(crate) fn due(
        &self,
        host: &mut dyn ServerHost,
        event: &ScheduledTaskEvent<'_>,
        menus: &BackpackMenus,
    ) {
        if event.owner != "slimefun" {
            return;
        }
        let pending = self.0.lock().unwrap().remove(&event.id);
        if let Some((player, id)) = pending
            && host.player(player.uuid()) == Some(player)
            && drink(host, player, id).unwrap_or_default()
        {
            menus.refresh_cooler(host, id);
        }
    }

    pub(crate) fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

fn first_named_potion<'a>(
    host: &dyn Host,
    slots: &'a BTreeMap<u32, ItemStack>,
) -> Option<(u32, &'a ItemStack)> {
    slots
        .iter()
        .find(|(_, stack)| {
            stack.item().id(host) == "minecraft:potion"
                && stack.components().is_some_and(|components| {
                    components
                        .as_nbt()
                        .contains_key(&"minecraft:custom_name".into())
                })
        })
        .map(|(&slot, stack)| (slot, stack))
}

fn number(tag: Option<&Tag>) -> Option<i32> {
    match tag? {
        Tag::Byte(n) => Some(i32::from(*n)),
        Tag::Short(n) => Some(i32::from(*n)),
        Tag::Int(n) => Some(*n),
        _ => None,
    }
}

fn custom_effects(host: &dyn Host, stack: &ItemStack) -> Vec<ActiveEffect> {
    let Some(Tag::List(effects)) = stack
        .components()
        .and_then(|patch| patch.as_nbt().get(&"minecraft:potion_contents".into()))
        .and_then(|contents| contents.get("custom_effects"))
    else {
        return Vec::new();
    };
    effects
        .iter()
        .filter_map(|effect| {
            let Tag::String(id) = effect.get("id")? else {
                return None;
            };
            let key = host.effect_key(&id.to_string().ok()?)?;
            let particles = number(effect.get("show_particles")) != Some(0);
            Some(ActiveEffect::new(
                key,
                number(effect.get("duration")).unwrap_or(0),
                i32::from(number(effect.get("amplifier")).unwrap_or(0) as u8),
                number(effect.get("ambient")).is_some_and(|value| value != 0),
                particles,
                number(effect.get("show_icon")).map_or(particles, |value| value != 0),
            ))
        })
        .collect()
}

fn take_drink(
    host: &mut dyn ServerHost,
    id: BackpackId,
) -> Result<Option<Vec<ActiveEffect>>, String> {
    host.transaction(|host, tx| {
        let Some(mut bag) = live_bag(host, id, SIZE)? else {
            return Ok::<_, String>(None);
        };
        let Some((slot, stack)) = first_named_potion(host, &bag.slots) else {
            return Ok(None);
        };
        let effects = custom_effects(host, stack);
        // This is an inventory slot removal in Paper, including stacks saved
        // before the item restrictions were added. No bottle is returned.
        bag.slots.remove(&slot);
        let data = bag.to_nbt(host)?;
        GlobalBackpacks.save(host, tx, id, data)?;
        Ok(Some(effects))
    })
}

fn drink(host: &mut dyn ServerHost, player: PlayerRef, id: BackpackId) -> Result<bool, String> {
    let living = LivingRef::new(player.entity());
    if living.health(host).is_none_or(|health| health <= 0.0) {
        return Ok(false);
    }
    if let Some(effects) = take_drink(host, id)? {
        for effect in effects {
            let _ = living.add_effect(host, &effect);
        }
        let _ = player.set_saturation(host, 6.0);
        let _ = player.play_sound(
            host,
            "minecraft:entity.generic.drink",
            SoundSource::Players,
            1.0,
            1.0,
        );
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backpacks::{Backpack, BackpackInventory, assign_item_id};
    use crate::items::{CHRISTMAS_EGG_NOG, GOLDEN_APPLE_JUICE, Kind};
    use mod_api::{
        Compound, Hand, Identifier, ItemPlace, ItemResource, ResourceHandler, testing::FakeHost,
    };

    fn fixture() -> FakeHost {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["potion", "splash_potion", "player_head", "diamond"]
                    .map(|id| Identifier::parse(&format!("minecraft:{id}")).unwrap()),
            )
            .unwrap();
        host
    }

    #[test]
    fn cooler_menu_accepts_juice_and_uses_the_transferred_paper_record() {
        let mut host = fixture();
        let id = BackpackId {
            owner: 3,
            number: 7,
        };
        let mut cooler = COOLER.stack(&host, 1).unwrap();
        assert_eq!(binding(&cooler), Ok(Binding::Blank));
        assign_item_id(&mut cooler, id).unwrap();
        host.join(5, 10).slots.insert(-106, cooler);
        host.transaction(|host, tx| {
            let record = Backpack {
                size: SIZE,
                slots: BTreeMap::new(),
                paper: true,
            }
            .to_nbt(host)?;
            GlobalBackpacks.save(host, tx, id, record)
        })
        .unwrap();
        let view = BackpackInventory {
            at: ItemPlace::PlayerHand {
                player: 5,
                hand: Hand::Off,
            },
            id,
            generation: None,
            size: SIZE,
            cooler: true,
        };
        let potion = Kind::Vanilla("minecraft:potion").stack(&host, 1).unwrap();
        let diamond = Kind::Vanilla("minecraft:diamond").stack(&host, 1).unwrap();
        let eggnog = CHRISTMAS_EGG_NOG.stack(&host, 1).unwrap();
        assert_eq!(view.size(&mut host), 27);
        host.transaction(|host, tx| {
            assert_eq!(view.insert(host, tx, 0, &ItemResource::of(&potion), 1), 0);
            assert_eq!(view.insert(host, tx, 1, &ItemResource::of(&diamond), 1), 0);
            assert_eq!(view.insert(host, tx, 26, &ItemResource::of(&eggnog), 1), 1);
            Ok::<_, ()>(())
        })
        .unwrap();
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        let saved = live_bag(&mut host, id, SIZE).unwrap().unwrap();
        assert_eq!(saved.slots, BTreeMap::from([(26, eggnog)]));
        assert!(saved.paper);
        let recipe = crate::recipes::crafting()
            .find(|recipe| recipe.output.kind == COOLER)
            .unwrap();
        assert_eq!((recipe.research.id, recipe.output.count), ("cooler", 1));
        assert_eq!(
            recipe.inputs.map(|kind| kind.unwrap().id()),
            [
                "CLOTH",
                "CLOTH",
                "CLOTH",
                "ALUMINUM_INGOT",
                "COOLING_UNIT",
                "ALUMINUM_INGOT",
                "ALUMINUM_INGOT",
                "ALUMINUM_INGOT",
                "ALUMINUM_INGOT",
            ]
        );
    }

    #[test]
    fn cooler_consumes_the_first_named_drink_whole_and_leaves_other_paper_slots() {
        let mut host = fixture();
        let id = BackpackId {
            owner: 9,
            number: 1,
        };
        let unnamed = Kind::Vanilla("minecraft:potion").stack(&host, 1).unwrap();
        let mut named = Kind::Vanilla("minecraft:potion").stack(&host, 3).unwrap();
        let name = Compound::from([("minecraft:custom_name".into(), Tag::String("Drink".into()))]);
        named.set_components(name.clone()).unwrap();
        let mut splash = Kind::Vanilla("minecraft:splash_potion")
            .stack(&host, 1)
            .unwrap();
        splash.set_components(name).unwrap();
        let golden = GOLDEN_APPLE_JUICE.stack(&host, 1).unwrap();
        host.transaction(|host, tx| {
            let record = Backpack {
                size: SIZE,
                slots: BTreeMap::from([
                    (0, unnamed.clone()),
                    (1, splash.clone()),
                    (2, named),
                    (8, golden.clone()),
                ]),
                paper: true,
            }
            .to_nbt(host)?;
            GlobalBackpacks.save(host, tx, id, record)
        })
        .unwrap();
        assert_eq!(take_drink(&mut host, id).unwrap(), Some(Vec::new()));
        let saved = live_bag(&mut host, id, SIZE).unwrap().unwrap();
        assert_eq!(
            saved.slots,
            BTreeMap::from([(0, unnamed), (1, splash), (8, golden)])
        );
    }
}
