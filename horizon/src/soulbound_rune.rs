// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    Compound, ExplosionCalculator, ExplosionInteraction, ExplosionSettings, Host, HostTransaction,
    ItemEntityRef, ItemStack, ItemTossEvent, LevelRef, RandomSource, ScheduledTaskEvent,
    ServerHost, SoundSource, Tag, TaskId, Text, text::TextStyle,
};

use crate::{
    items::{item, text},
    recipes::Research,
    research,
};

item!(
    ANCIENT_RUNE_SOULBOUND,
    "firework_star",
    "Ancient Rune [Soulbound]",
    "gray",
    None,
    None
);
pub(crate) const RESEARCH: Research = Research {
    id: "soulbound_rune",
    name: "Soulbound Rune",
    cost: 60,
};

const MARKER: &str = "slimefun:soulbound";
const RANGE: f64 = 1.5;
const FAILED: &str = "You can only bind one item to your soul at a time.";
const SUCCEEDED: &str =
    "You have successfully bound this item to your soul! You will keep it when you die.";

#[derive(Default)]
pub(crate) struct Rituals(Mutex<Queue>);

#[derive(Default)]
struct Queue {
    pending: BTreeMap<TaskId, Ritual>,
    ready: Vec<Ritual>,
}

struct Ritual {
    level: LevelRef,
    player: u128,
    rune: ItemEntityRef,
    stage: Stage,
}

enum Stage {
    Search,
    Bind {
        origin: [f64; 3],
        target: ItemEntityRef,
        original: ItemStack,
    },
}

impl Rituals {
    pub(crate) fn tossed(&self, host: &mut dyn ServerHost, event: &ItemTossEvent) {
        let Some(stack) = event.item.stack(host) else {
            return;
        };
        if !ANCIENT_RUNE_SOULBOUND.matches(host, &stack) {
            return;
        }
        let Some(player) = host.player(event.player) else {
            return;
        };
        if !player
            .persistent_data(host)
            .as_ref()
            .is_some_and(|saved| research::unlocked(saved, &RESEARCH))
        {
            tell(
                host,
                event.player,
                "Unlock this research with /sf research soulbound_rune.",
                "red",
            );
            return;
        }
        let Some(level) = player.entity().level(host) else {
            return;
        };
        self.schedule(
            host,
            20,
            Ritual {
                level,
                player: event.player,
                rune: event.item,
                stage: Stage::Search,
            },
        );
    }

    fn schedule(&self, host: &mut dyn ServerHost, delay: u32, ritual: Ritual) {
        if let Ok(id) = host.schedule_task(delay, 0) {
            self.0.lock().unwrap().pending.insert(id, ritual);
        }
    }

    pub(crate) fn due(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let ritual = self.0.lock().unwrap().pending.remove(&event.id);
        if let Some(ritual) = ritual {
            if ritual.level.in_place(host) {
                self.advance(host, ritual);
            } else {
                self.0.lock().unwrap().ready.push(ritual);
            }
        }
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, level: LevelRef) {
        let ready: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .ready
            .extract_if(.., |ritual| ritual.level == level)
            .collect();
        for ritual in ready {
            self.advance(host, ritual);
        }
    }

    fn advance(&self, host: &mut dyn ServerHost, ritual: Ritual) {
        if let Some(ritual) = ritual.advance(host) {
            self.schedule(host, 10, ritual);
        }
    }

    pub(crate) fn unload(&self, host: &mut dyn ServerHost, dimension: &str) {
        let cancelled = {
            let mut queue = self.0.lock().unwrap();
            let still_open =
                |ritual: &Ritual| ritual.level.dimension().id(host).as_deref() != Some(dimension);
            queue.ready.retain(still_open);
            queue
                .pending
                .extract_if(.., |_, ritual| !still_open(ritual))
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        };
        for id in cancelled {
            host.cancel_task(id);
        }
    }

    pub(crate) fn clear(&self) {
        *self.0.lock().unwrap() = Queue::default();
    }
}

impl Ritual {
    fn advance(mut self, host: &mut dyn ServerHost) -> Option<Self> {
        match self.stage {
            Stage::Search => {
                if !self.live_rune(host) {
                    return None;
                }
                let origin = self.rune.entity().position(host)?;
                let (target, original) = nearby_item(host, self.level, origin)?;
                if original.count() != 1 {
                    tell(host, self.player, FAILED, "red");
                    return None;
                }
                self.level.strike_lightning(host, origin, true);
                self.stage = Stage::Bind {
                    origin,
                    target,
                    original,
                };
                Some(self)
            }
            Stage::Bind {
                origin,
                target,
                ref original,
            } => {
                if self.bind(host, origin, target, original) {
                    tell(host, self.player, SUCCEEDED, "green");
                } else {
                    tell(host, self.player, FAILED, "red");
                }
                None
            }
        }
    }

    fn live_rune(&self, host: &dyn Host) -> bool {
        self.rune.entity().level(host) == Some(self.level)
            && self
                .rune
                .stack(host)
                .is_some_and(|stack| ANCIENT_RUNE_SOULBOUND.matches(host, &stack))
    }

    fn has_inputs(&self, host: &dyn Host, target: ItemEntityRef, original: &ItemStack) -> bool {
        self.live_rune(host)
            && target.entity().level(host) == Some(self.level)
            && target.stack(host).as_ref() == Some(original)
    }

    fn bind(
        &self,
        host: &mut dyn ServerHost,
        origin: [f64; 3],
        target: ItemEntityRef,
        original: &ItemStack,
    ) -> bool {
        if !self.has_inputs(host, target, original) {
            return false;
        }
        let Some(output) = mark(original) else {
            return false;
        };
        let Some([x, y, z]) = self.level.with_random(host, |random| {
            let mut at = origin.map(|axis| axis + (random.next_double() * 0.5 - 0.25));
            at[1] -= 0.125; // Bukkit dropItemNaturally subtracts half the dropped item's height.
            at
        }) else {
            return false;
        };

        // Do not consume either input until the replacement has actually passed the join hooks.
        let Ok(Some(spawned)) = self.level.spawn_item(host, x, y, z, output.clone(), 10) else {
            return false;
        };
        let removed: Result<(), ()> = host.transaction(|host, _| {
            if !self.has_inputs(host, target, original)
                || host
                    .item_entity(spawned)
                    .and_then(|item| item.stack(host))
                    .as_ref()
                    != Some(&output)
                || !target.entity().discard(host)
                || !self.rune.entity().discard(host)
            {
                return Err(());
            }
            Ok(())
        });
        if removed.is_err() {
            spawned.discard(host);
            return false;
        }
        let _ = self.level.explode(
            host,
            None,
            origin,
            0.0,
            false,
            ExplosionInteraction::None,
            ExplosionSettings {
                calculator: Some(ExplosionCalculator {
                    damages_entities: false,
                    knockback_multiplier: Some(0.0),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        self.level.play_sound(
            host,
            "minecraft:entity.generic.explode",
            origin,
            SoundSource::Players,
            0.3,
            1.0,
        );
        true
    }
}

fn nearby_item(
    host: &dyn Host,
    level: LevelRef,
    at: [f64; 3],
) -> Option<(ItemEntityRef, ItemStack)> {
    level
        .entities_in(
            host,
            at.map(|axis| axis - RANGE),
            at.map(|axis| axis + RANGE),
        )?
        .into_iter()
        .filter_map(|entity| host.item_entity(entity))
        .find_map(|item| {
            if item.pickup_delay(host)? > 0 {
                return None;
            }
            let stack = item.stack(host)?;
            (!ANCIENT_RUNE_SOULBOUND.matches(host, &stack) && !is_bound(host, &stack))
                .then_some((item, stack))
        })
}

fn tell(host: &mut dyn ServerHost, player: u128, message: &str, color: &str) {
    if let Some(player) = host.player(player) {
        let mut style = TextStyle::default();
        style.color = Some(color.into());
        let _ = player.tell(host, Text::literal(message).with_style(style));
    }
}

pub(crate) fn is_bound(host: &dyn Host, stack: &ItemStack) -> bool {
    crate::backpacks::keep_on_death(host, stack)
        || crate::soulbound_gear::keeps_on_death(host, stack)
        || has_saved_marker(stack)
}

fn has_saved_marker(stack: &ItemStack) -> bool {
    let Some(components) = stack.components().map(|components| components.as_nbt()) else {
        return false;
    };
    if matches!(
        components
            .get(&"minecraft:custom_data".into())
            .and_then(|custom| custom.get("PublicBukkitValues"))
            .and_then(|values| values.get(MARKER)),
        Some(Tag::Byte(_))
    ) {
        return true;
    }
    let Some(Tag::List(lines)) = components.get(&"minecraft:lore".into()) else {
        return false;
    };
    lines.iter().any(|line| match line {
        Tag::String(value) => value.to_string_lossy() == "§7Soulbound",
        Tag::Compound(fields) => {
            let mut literal = String::new();
            gray_literal(fields, false, &mut literal, 0) && literal == "Soulbound"
        }
        _ => false,
    })
}

fn gray_literal(fields: &Compound, mut gray: bool, literal: &mut String, depth: u8) -> bool {
    if depth > 8
        || [
            "italic",
            "bold",
            "underlined",
            "strikethrough",
            "obfuscated",
        ]
        .into_iter()
        .any(|key| matches!(fields.get(&key.into()), Some(Tag::Byte(flag)) if *flag != 0))
    {
        return false;
    }
    if let Some(color) = fields.get(&"color".into()) {
        gray = matches!(color, Tag::String(color) if matches!(color.to_string_lossy().as_str(), "gray" | "#aaaaaa"));
    }
    let Some(Tag::String(body)) = fields.get(&"text".into()) else {
        return false;
    };
    let body = body.to_string_lossy();
    if !body.is_empty() && !gray {
        return false;
    }
    literal.push_str(&body);
    match fields.get(&"extra".into()) {
        None => true,
        Some(Tag::List(children)) => children.iter().all(|child| match child {
            Tag::Compound(fields) => gray_literal(fields, gray, literal, depth + 1),
            Tag::String(body) if gray => {
                literal.push_str(&body.to_string_lossy());
                true
            }
            _ => false,
        }),
        _ => false,
    }
}

fn mark(stack: &ItemStack) -> Option<ItemStack> {
    if stack.count() != 1 || has_saved_marker(stack) {
        return None;
    }
    let patch = stack.components().map(|components| components.as_nbt());
    let mut lore = match patch.and_then(|patch| patch.get(&"minecraft:lore".into())) {
        Some(Tag::List(lines)) => lines.clone(),
        None => Vec::new(),
        Some(_) => return None,
    };
    let mut custom = match patch.and_then(|patch| patch.get(&"minecraft:custom_data".into())) {
        Some(Tag::Compound(data)) => data.clone(),
        None => Compound::new(),
        Some(_) => return None,
    };
    let Tag::Compound(values) = custom
        .entry("PublicBukkitValues".into())
        .or_insert_with(|| Tag::Compound(Compound::new()))
    else {
        return None;
    };
    values.insert(MARKER.into(), Tag::Byte(1));
    lore.push(text("Soulbound", "gray", false));
    let mut result = stack.clone();
    result
        .apply_components(&Compound::from([
            ("minecraft:custom_data".into(), Tag::Compound(custom)),
            ("minecraft:lore".into(), Tag::List(lore)),
        ]))
        .ok()?;
    Some(result)
}

pub(crate) fn lore() -> Tag {
    let highlighted = |first: &str, first_color: &str, rest: Vec<Tag>| {
        Tag::compound([
            ("text", Tag::String(first.into())),
            ("color", Tag::String(first_color.into())),
            ("italic", Tag::Byte(0)),
            ("extra", Tag::List(rest)),
        ])
    };
    Tag::List(vec![
        text("Drop this rune onto a dropped item to", "yellow", false),
        highlighted(
            "bind ",
            "dark_purple",
            vec![text("that item to your soul.", "yellow", false)],
        ),
        Tag::String(" ".into()),
        text("It is advised that you only use this rune", "yellow", false),
        highlighted(
            "on ",
            "yellow",
            vec![
                text("important ", "gold", false),
                text("items.", "yellow", false),
            ],
        ),
        Tag::String(" ".into()),
        text(
            "Items bound to your soul won't drop on death.",
            "yellow",
            false,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, prelude::ItemsHost, testing::FakeHost};

    fn plain_item(host: &mut FakeHost) -> ItemStack {
        let id = Identifier::parse("minecraft:diamond_pickaxe").unwrap();
        host.registries.resolve_items([id.clone()]).unwrap();
        ItemStack::new(host.item_key(&id).unwrap(), 1).unwrap()
    }

    #[test]
    fn binding_a_vanilla_item_preserves_damage_enchantments_lore_and_other_pdc() {
        let mut host = FakeHost::new();
        let mut item = plain_item(&mut host);
        assert!(!is_bound(&host, &item));
        let old = text("My original lore", "red", false);
        let other = Tag::Int(42);
        item.set_damage(27);
        item.apply_components(&Compound::from([
            (
                "minecraft:custom_name".into(),
                text("Precious Pickaxe", "aqua", false),
            ),
            (
                "minecraft:custom_data".into(),
                Tag::compound([
                    ("my_other_value", Tag::Byte(9)),
                    (
                        "PublicBukkitValues",
                        Tag::compound([("another:marker", other.clone())]),
                    ),
                ]),
            ),
            ("minecraft:lore".into(), Tag::List(vec![old.clone()])),
            (
                "minecraft:enchantments".into(),
                Tag::compound([("minecraft:mending", Tag::Int(1))]),
            ),
        ]))
        .unwrap();
        let bound = mark(&item).unwrap();
        assert_eq!(bound.item(), item.item());
        assert_eq!(bound.damage(), item.damage());
        assert_eq!(bound.count(), item.count());
        assert!(is_bound(&host, &bound));
        let before = item.components().unwrap().as_nbt();
        let after = bound.components().unwrap().as_nbt();
        for field in ["minecraft:custom_name", "minecraft:enchantments"] {
            assert_eq!(before.get(&field.into()), after.get(&field.into()));
        }
        assert_eq!(
            after.get(&"minecraft:lore".into()),
            Some(&Tag::List(vec![old, text("Soulbound", "gray", false)]))
        );
        let data = after.get(&"minecraft:custom_data".into()).unwrap();
        assert_eq!(data.get("my_other_value"), Some(&Tag::Byte(9)));
        let pdc = data.get("PublicBukkitValues").unwrap();
        assert_eq!(pdc.get("another:marker"), Some(&other));
        assert_eq!(pdc.get(MARKER), Some(&Tag::Byte(1)));
        assert!(!is_bound(&host, &item));
        assert!(mark(&bound).is_none());
    }

    #[test]
    fn historical_paper_recognizes_byte_flags_and_gray_lore_on_plain_items() {
        let mut host = FakeHost::new();
        let base = plain_item(&mut host);
        let with = |key: &str, value: Tag| {
            let mut item = base.clone();
            item.set_components(Compound::from([(key.into(), value)]))
                .unwrap();
            item
        };
        for (line, expected) in [
            (Tag::String("§7Soulbound".into()), true),
            (Tag::String("Soulbound".into()), false),
            (text("Soulbound", "red", false), false),
            (
                Tag::compound([
                    ("text", Tag::String("".into())),
                    ("extra", Tag::List(vec![text("Soulbound", "gray", false)])),
                ]),
                true,
            ),
        ] {
            assert_eq!(
                is_bound(&host, &with("minecraft:lore", Tag::List(vec![line]))),
                expected
            );
        }
        let custom_data =
            |tag| Tag::compound([("PublicBukkitValues", Tag::compound([(MARKER, tag)]))]);
        assert!(is_bound(
            &host,
            &with("minecraft:custom_data", custom_data(Tag::Byte(0)))
        ));
        assert!(!is_bound(
            &host,
            &with("minecraft:custom_data", custom_data(Tag::Int(1)))
        ));
    }

    #[test]
    fn the_rune_uses_the_paper_altar_layout_and_research() {
        let recipe = crate::ancient_altar::CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == ANCIENT_RUNE_SOULBOUND)
            .unwrap();
        assert_eq!(recipe.research.id, "soulbound_rune");
        assert_eq!(recipe.research.cost, 60);
        assert_eq!(recipe.output.count, 1);
        assert_eq!(
            recipe.inputs.map(|input| input.unwrap().id()),
            [
                "MAGIC_LUMP_3",
                "ESSENCE_OF_AFTERLIFE",
                "MAGIC_LUMP_3",
                "ENDER_LUMP_3",
                "ANCIENT_RUNE_ENDER",
                "ENDER_LUMP_3",
                "MAGIC_LUMP_3",
                "ESSENCE_OF_AFTERLIFE",
                "MAGIC_LUMP_3",
            ]
        );
    }
}
