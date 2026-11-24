// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    ActiveEffect, AttachmentCodec, AttachmentError, AttachmentType, Compound, GameMode, Holder,
    Identifier, ItemStack, LivingRef, PlayerRef, ServerHost, Tag, Text, text::TextStyle,
};

use crate::hologram_projector::colored_lore;

const MAX_EXPOSURE: u8 = 100;
const TICKS_PER_SECOND: u32 = 20;
const MAX_INTERVAL_SECONDS: u32 = (i32::MAX as u32 - TICKS_PER_SECOND) / TICKS_PER_SECOND;
const HAZMAT: [&str; 4] = [
    "RUBBER_BOOTS",
    "HAZMAT_LEGGINGS",
    "HAZMAT_CHESTPLATE",
    "SCUBA_HELMET",
];
const SYMPTOMS: [(u8, &str, i32); 5] = [
    (10, "minecraft:slowness", 3),
    (25, "minecraft:wither", 0),
    (50, "minecraft:blindness", 4),
    (75, "minecraft:wither", 3),
    (100, "minecraft:instant_damage", 49),
];

static EXPOSURE: AttachmentType<u8> = AttachmentType {
    namespace: "slimefun",
    name: "radiation",
    default: || 0,
    codec: Some(AttachmentCodec {
        encode: |value| Ok(Compound::from([("level".into(), Tag::Byte(*value as i8))])),
        decode: |data| match data.get(&"level".into()) {
            Some(Tag::Byte(value @ 0..=100)) => Ok(*value as u8),
            _ => Err(AttachmentError::Decode),
        },
    }),
    // The death tail writes zero before a respawn. An End exit is not a death.
    copy_on_death: true,
    sync: false,
};

#[derive(Clone, Copy)]
struct Rules {
    enabled: bool,
    interval: u32,
    grace_ms: u64,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: TICKS_PER_SECOND,
            grace_ms: 15_000,
        }
    }
}

impl Rules {
    fn read(section: Option<&toml::Value>) -> Self {
        let mut rules = Self::default();
        let Some(section) = section else {
            return rules;
        };
        if let Some(value) = section.get("enabled").and_then(toml::Value::as_bool) {
            rules.enabled = value;
        }
        if let Some(value) = section
            .get("interval-seconds")
            .and_then(toml::Value::as_integer)
            && let Ok(seconds @ 1..=MAX_INTERVAL_SECONDS) = u32::try_from(value)
        {
            rules.interval = seconds * TICKS_PER_SECOND;
        }
        if let Some(value) = section
            .get("grace-seconds")
            .and_then(toml::Value::as_integer)
            && let Ok(seconds) = u32::try_from(value)
        {
            rules.grace_ms = u64::from(seconds) * 1000;
        }
        rules
    }

    fn duration(self) -> i32 {
        (self.interval + TICKS_PER_SECOND) as i32
    }
}

#[derive(Default)]
struct State {
    rules: Rules,
    ticks_until_sample: u32,
    due: bool,
    grace: BTreeMap<u128, u64>,
}

#[derive(Default)]
pub(crate) struct Radiation(Mutex<State>);

impl Radiation {
    pub(crate) fn configure(&self, config: Option<&toml::Value>) {
        let mut state = self.0.lock().unwrap();
        state.rules = Rules::read(config);
        state.ticks_until_sample = 0;
    }

    pub(crate) fn begin_tick(&self, now: impl FnOnce() -> u64) {
        let mut state = self.0.lock().unwrap();
        state.due = state.rules.enabled && state.ticks_until_sample == 0;
        if state.due {
            state.ticks_until_sample = state.rules.interval;
            if !state.grace.is_empty() {
                let now = now();
                state.grace.retain(|_, deadline| now <= *deadline);
            }
        }
        state.ticks_until_sample = state.ticks_until_sample.saturating_sub(1);
    }

    pub(crate) fn clear(&self) {
        let mut state = self.0.lock().unwrap();
        state.grace.clear();
        state.ticks_until_sample = 0;
        state.due = false;
    }

    pub(crate) fn death(&self, host: &mut dyn ServerHost, entity: u32) {
        let Some(entity) = host.entity(entity).filter(|entity| {
            entity
                .kind(host)
                .is_some_and(|kind| kind.id(host) == "minecraft:player")
        }) else {
            return;
        };
        let Some(uuid) = entity.uuid(host) else {
            return;
        };
        let Some(player) = host.player(uuid).filter(|player| player.entity() == entity) else {
            return;
        };
        // Keep an explicit zero so the original Paper tag is never imported again.
        clear_exposure(host, player);
        let now = host.server_monotonic_millis();
        let mut state = self.0.lock().unwrap();
        let deadline = now.saturating_add(state.rules.grace_ms);
        state.grace.retain(|_, deadline| now <= *deadline);
        state.grace.insert(player.uuid(), deadline);
    }

    pub(crate) fn player_tick(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        let rules = {
            let state = self.0.lock().unwrap();
            if !state.due || state.grace.contains_key(&player.uuid()) {
                return;
            }
            state.rules
        };
        let living = LivingRef::new(player.entity());
        if player.is_fake(host) == Some(true)
            || !living.health(host).is_some_and(|health| health > 0.0)
        {
            return;
        }
        let Some(mode) = player.game_mode(host) else {
            return;
        };
        let Some(slots) = player.inventory_slots(host) else {
            return;
        };
        let Some((before, imported)) = exposure(host, player) else {
            return;
        };
        let (dose, protected) = inventory_radiation(&slots);
        let protected = protected || matches!(mode, GameMode::Creative | GameMode::Spectator);
        let after = effective_exposure(before, if protected { 0 } else { dose });
        if (after != before || imported)
            && EXPOSURE
                .set(host, Holder::Player(player.uuid()), after)
                .is_err()
        {
            return;
        }
        if protected {
            return;
        }
        if before == 0 && dose > 0 {
            let warning = colored("You have been exposed to deadly radiation! ", "dark_red")
                .append(colored(
                    "Get rid of the radioactive item or equip the complete hazmat suit!",
                    "red",
                ));
            let _ = player.tell(host, warning);
        }
        for (_, effect, amplifier) in SYMPTOMS
            .into_iter()
            .filter(|(threshold, _, _)| after >= *threshold)
        {
            if let Some(effect) = host.effect_key(effect) {
                living.add_effect(
                    host,
                    &ActiveEffect::new(effect, rules.duration(), amplifier, false, true, true),
                );
            }
        }
        if before > 0 || after > 0 {
            let meter = colored("Radiation Exposure Level: ", "gold")
                .append(colored(after.to_string(), "red"))
                .append(colored("/", "gray"))
                .append(colored("100", "yellow"));
            let _ = player.action_bar(host, meter);
        }
    }
}

fn colored(text: impl Into<String>, color: &str) -> Text {
    let mut style = TextStyle::default();
    style.color = Some(color.into());
    Text::literal(text).with_style(style)
}

pub(crate) fn clear_exposure(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = EXPOSURE.set(host, Holder::Player(player.uuid()), 0);
}

fn exposure(host: &mut dyn ServerHost, player: PlayerRef) -> Option<(u8, bool)> {
    let holder = Holder::Player(player.uuid());
    if let Some(value) = EXPOSURE.get_existing(host, holder).ok()? {
        return Some((value, false));
    }
    let key = Identifier::parse("slimefun:radiation").expect("Paper radiation id");
    match player.paper_persistent_value(host, &key) {
        Some(raw) => Some((parse_paper_exposure(&raw)?, true)),
        None => Some((0, false)),
    }
}

fn parse_paper_exposure(raw: &Tag) -> Option<u8> {
    let Tag::String(text) = raw else {
        return None;
    };
    let text = text.to_string().ok()?;
    let (value, stamp) = text.split_once(';')?;
    stamp.parse::<i64>().ok()?;
    value
        .parse::<u8>()
        .ok()
        .filter(|level| *level <= MAX_EXPOSURE)
}

fn effective_exposure(before: u8, dose: u8) -> u8 {
    if dose == 0 {
        before.saturating_sub(1)
    } else {
        before.saturating_add(dose).min(MAX_EXPOSURE)
    }
}

fn exposure_weight(id: &str) -> u32 {
    match id {
        "TINY_URANIUM" => 1,
        "SMALL_URANIUM" | "NETHER_ICE" => 2,
        "URANIUM" | "NEPTUNIUM" | "BLISTERING_INGOT" => 3,
        "PLUTONIUM"
        | "BOOSTED_URANIUM"
        | "ENRICHED_NETHER_ICE"
        | "BLISTERING_INGOT_2"
        | "BLISTERING_INGOT_3" => 5,
        _ => 0,
    }
}

pub(crate) fn item_lore(id: &str) -> Option<Tag> {
    let weight = exposure_weight(id);
    let level = match weight {
        1 => "&eLOW",
        2 => "&eMODERATE",
        3 => "&6HIGH",
        5 => "&cVERY HIGH",
        _ => return None,
    };
    let mut lore = vec![
        colored_lore(""),
        colored_lore(&format!("&a☢&7 Radiation level: {level}")),
    ];
    if weight > 1 {
        lore.push(colored_lore("&8⇨ &4Hazmat Suit required!"));
    }
    Some(Tag::List(lore))
}

fn inventory_radiation(slots: &[(u32, ItemStack)]) -> (u8, bool) {
    let mut dose = 0;
    let mut pieces = 0usize;
    for (slot, item) in slots.iter().filter(|(slot, _)| *slot <= 40) {
        let Some(id) = crate::items::item_id(item) else {
            continue;
        };
        if item.count() == 0 {
            continue;
        }
        if (36..=39).contains(slot) && HAZMAT.contains(&id.as_str()) {
            pieces += 1;
        }
        let weight = exposure_weight(&id);
        dose = (dose + item.count().min(u32::from(MAX_EXPOSURE)) * weight)
            .min(u32::from(MAX_EXPOSURE));
    }
    (dose as u8, pieces == HAZMAT.len())
}

/// The Paper Hazmat armor listener wears each real piece and changes an accepted sting to zero damage.
pub(crate) fn protect_bee_sting(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    rolls: &crate::random::Rolls,
) -> mod_api::Flow {
    use mod_api::HostTransaction;

    let Some(dimension) = player.dimension(host) else {
        return mod_api::Flow::Continue;
    };
    let Some(level) = host.level(&dimension.to_string()) else {
        return mod_api::Flow::Continue;
    };
    let Some(slots) = player.inventory_slots(host) else {
        return mod_api::Flow::Continue;
    };
    let worn: Vec<_> = slots
        .into_iter()
        .filter(|(slot, item)| {
            (36..=39).contains(slot)
                && item.count() > 0
                && crate::items::item_id(item).is_some_and(|id| HAZMAT.contains(&id.as_str()))
        })
        .collect();
    if worn.len() != HAZMAT.len() {
        return mod_api::Flow::Continue;
    }
    let wear = host.transaction(|host, _| -> Result<(), ()> {
        for (slot, item) in worn {
            let max = host.max_damage(item.item()).ok_or(())?;
            let enchants =
                crate::enchantment_machines::levels(host, &item, "minecraft:enchantments")
                    .ok_or(())?;
            let unbreaking = enchants
                .get(&"minecraft:unbreaking".into())
                .and_then(crate::enchantment_machines::level);
            let damage = match unbreaking {
                Some(level_of_enchant) => {
                    let roll = rolls.next(host, level, 100).ok_or(())?;
                    sting_wear_amount(level_of_enchant as u32, roll)
                }
                None => 1,
            };
            if damage == 0 {
                continue;
            }
            let replacement = sting_durability(item, max, damage);
            player
                .set_inventory_slot(host, slot, replacement)
                .map_err(|_| ())?;
        }
        Ok(())
    });
    if wear.is_ok() {
        mod_api::Flow::Cancel
    } else {
        mod_api::Flow::Continue
    }
}

fn sting_wear_amount(unbreaking: u32, roll: u32) -> u32 {
    let chance_to_save = 60 + 40 / (unbreaking + 1);
    u32::from(roll >= chance_to_save)
}

fn sting_durability(mut item: ItemStack, maximum: u32, wear: u32) -> Option<ItemStack> {
    let damage = item.damage().saturating_add(wear);
    if damage > maximum {
        return None;
    }
    item.set_damage(damage);
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{armor_forge, heated_pressure, items::Kind, network_recipes};
    use mod_api::testing::FakeHost;

    #[test]
    fn counts_paper_item_ids_and_actual_carried_stacks_but_requires_worn_hazmat() {
        let mut host = FakeHost::new();
        let suit = [
            armor_forge::RUBBER_BOOTS,
            armor_forge::HAZMAT_LEGGINGS,
            armor_forge::HAZMAT_CHESTPLATE,
            armor_forge::SCUBA_HELMET,
        ];
        let tiny = heated_pressure::TINY_URANIUM;
        let blistering = network_recipes::BLISTERING_INGOT_3;
        let vanilla = Kind::Vanilla("minecraft:gold_ingot");
        host.registries
            .resolve_items(
                suit.into_iter()
                    .chain([tiny, blistering, vanilla])
                    .map(|kind| Identifier::parse(kind.material()).unwrap()),
            )
            .unwrap();
        let mut slots = vec![
            (0, tiny.stack(&host, 3).unwrap()),
            (40, blistering.stack(&host, 2).unwrap()),
            (35, vanilla.stack(&host, 64).unwrap()),
            (41, blistering.stack(&host, 6).unwrap()),
        ];
        assert_eq!(inventory_radiation(&slots), (13, false));
        slots.extend(
            suit.iter()
                .enumerate()
                .map(|(index, kind)| (36 + index as u32, kind.stack(&host, 1).unwrap())),
        );
        assert_eq!(inventory_radiation(&slots), (13, true));
        slots.last_mut().unwrap().0 = 30;
        assert_eq!(inventory_radiation(&slots), (13, false));
        slots.push((1, blistering.stack(&host, 64).unwrap()));
        assert_eq!(inventory_radiation(&slots), (100, false));
    }

    #[test]
    fn paper_status_import_decay_and_the_saved_death_reset_keep_exact_doses() {
        assert_eq!(parse_paper_exposure(&Tag::String("25;0".into())), Some(25));
        assert_eq!(
            parse_paper_exposure(&Tag::String("100;0".into())),
            Some(100)
        );
        assert_eq!(parse_paper_exposure(&Tag::String("101;0".into())), None);
        assert_eq!(parse_paper_exposure(&Tag::String("75".into())), None);
        assert_eq!(effective_exposure(93, 13), 100);
        assert_eq!(effective_exposure(3, 0), 2);
        assert_eq!(effective_exposure(0, 0), 0);

        let mut host = FakeHost::new();
        let holder = Holder::Player(1);
        EXPOSURE.set(&mut host.attachments, holder, 76).unwrap();
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        assert_eq!(
            EXPOSURE.get_existing(&mut host.attachments, holder),
            Ok(Some(76))
        );
        // A kept inventory or copied Player attachment cannot bring the old dose back.
        EXPOSURE.set(&mut host.attachments, holder, 0).unwrap();
        host.attachments = mod_api::Attachments::from_nbt(host.attachments.to_nbt());
        assert_eq!(
            EXPOSURE.get_existing(&mut host.attachments, holder),
            Ok(Some(0))
        );
    }

    #[test]
    fn death_grace_is_elapsed_time_inclusive_and_does_not_survive_stopping() {
        let radiation = Radiation::default();
        radiation.0.lock().unwrap().grace.insert(1, 15_000);
        radiation.begin_tick(|| 15_000);
        assert!(radiation.0.lock().unwrap().grace.contains_key(&1));
        for _ in 0..20 {
            radiation.begin_tick(|| 15_001);
        }
        assert!(!radiation.0.lock().unwrap().grace.contains_key(&1));

        radiation.0.lock().unwrap().grace.insert(1, 40_000);
        radiation.clear();
        assert!(radiation.0.lock().unwrap().grace.is_empty());
        assert_eq!(Rules::default().duration(), 40);
    }

    #[test]
    fn bee_wear_uses_doughs_unbreaking_odds_and_removes_at_the_next_point() {
        assert_eq!(sting_wear_amount(1, 79), 0);
        assert_eq!(sting_wear_amount(1, 80), 1);
        assert_eq!(sting_wear_amount(3, 69), 0);
        assert_eq!(sting_wear_amount(3, 70), 1);

        let mut host = FakeHost::new();
        let cap = armor_forge::SCUBA_HELMET;
        host.registries
            .resolve_items([Identifier::parse(cap.material()).unwrap()])
            .unwrap();
        let mut item = cap.stack(&host, 1).unwrap();
        item.set_damage(54);
        let last_point = sting_durability(item, 55, 1).unwrap();
        assert_eq!(last_point.damage(), 55);
        assert_eq!(
            crate::items::item_id(&last_point).as_deref(),
            Some("SCUBA_HELMET")
        );
        assert!(sting_durability(last_point, 55, 1).is_none());
    }
}
