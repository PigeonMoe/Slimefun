// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    ActiveEffect, Compound, Flow, GameMode, Hand, Host, ItemStack, LivingEntityUseItemFinishEvent,
    LivingEntityUseItemStartEvent, LivingRef, PlayerRef, ScheduledTaskEvent, ServerHost,
    SoundSource, Tag, TaskId, Text,
};

use crate::{
    armor_forge::ELYTRA_SCALE,
    item_clicks::consume_one,
    items::{Kind, SALT, item, item_id, text},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    FORTUNE_COOKIE,
    "cookie",
    "Fortune Cookie",
    "gold",
    None,
    None
);
item!(DIET_COOKIE, "cookie", "Diet Cookie", "gold", None, None);
item!(MAGIC_SUGAR, "sugar", "Magic Sugar", "gold", None, None);
item!(
    MONSTER_JERKY,
    "rotten_flesh",
    "Monster Jerky",
    "gold",
    None,
    None
);

const FORTUNE: Research = Research {
    id: "fortune_cookie",
    name: "Fortune Cookie",
    cost: 1,
};
const DIET: Research = Research {
    id: "diet_cookie",
    name: "Diet Cookie",
    cost: 3,
};
const SUGAR: Research = Research {
    id: "magic_sugar",
    name: "Magic Sugar",
    cost: 4,
};
const JERKY: Research = Research {
    id: "monster_jerky",
    name: "Monster Jerky",
    cost: 5,
};

pub(crate) const RESEARCHES: &[&Research] = &[&FORTUNE, &DIET, &SUGAR, &JERKY];

const COOKIE: Option<Kind> = Some(Kind::Vanilla("minecraft:cookie"));
const ROTTEN_FLESH: Kind = Kind::Vanilla("minecraft:rotten_flesh");

const fn craft(row: [Option<Kind>; 3], kind: Kind, research: &'static Research) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [row[0], row[1], row[2], None, None, None, None, None, None],
        output: Stack { kind, count: 1 },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        [COOKIE, Some(Kind::Vanilla("minecraft:paper")), None],
        FORTUNE_COOKIE,
        &FORTUNE,
    ),
    craft(
        [
            Some(Kind::Vanilla("minecraft:sugar")),
            Some(Kind::Vanilla("minecraft:redstone")),
            Some(Kind::Vanilla("minecraft:glowstone_dust")),
        ],
        MAGIC_SUGAR,
        &SUGAR,
    ),
    craft(
        [Some(SALT), Some(ROTTEN_FLESH), None],
        MONSTER_JERKY,
        &JERKY,
    ),
];

pub(crate) const DIET_RECIPE: CraftingRecipe =
    craft([COOKIE, Some(ELYTRA_SCALE), None], DIET_COOKIE, &DIET);

// These defaults come from messages.fortune-cookie in the same project's Paper English catalog.
const FORTUNES: &str = include_str!("fortune-messages.en.txt");
const LEVITATION: (&str, i32, i32) = ("minecraft:levitation", 60, 1);
const SPEED: (&str, i32, i32) = ("minecraft:speed", 600, 3);
const SATURATION: (&str, i32, i32) = ("minecraft:saturation", 5, 0);

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let lore = match id {
        "FORTUNE_COOKIE" => text("Tells you stuff about your Future :o", "green", true),
        "DIET_COOKIE" => Tag::compound([
            ("text", Tag::String("".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![
                    text("A very ", "green", false),
                    text("lightweight ", "green", true),
                    text("cookie.", "green", false),
                ]),
            ),
        ]),
        "MAGIC_SUGAR" => text("Feel the Power of Hermes!", "green", true),
        "MONSTER_JERKY" => text("No longer hungry", "green", true),
        _ => return,
    };
    components.insert(
        "minecraft:lore".into(),
        Tag::List(vec![Tag::String("".into()), lore]),
    );
}

fn sugar_exempt(block: &str) -> bool {
    matches!(block, "minecraft:dispenser" | "minecraft:enchanting_table")
}

pub(crate) fn use_sugar(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
    clicked: Option<[i32; 3]>,
) -> bool {
    if !MAGIC_SUGAR.matches(host, stack) {
        return false;
    }
    let Some(mode @ (GameMode::Survival | GameMode::Adventure | GameMode::Creative)) =
        player.game_mode(host)
    else {
        return false;
    };
    if !allowed(host, player, &SUGAR)
        || clicked.is_some_and(|pos| {
            player
                .entity()
                .level(host)
                .and_then(|level| level.block(host, pos))
                .is_some_and(|block| sugar_exempt(&block.id(host)))
        })
    {
        return true;
    }
    if mode != GameMode::Creative && !consume_one(host, player, hand, stack) {
        return true;
    }
    play_eating_sound(host, player);
    add_effect(host, LivingRef::new(player.entity()), SPEED);
    true
}

fn food(host: &dyn Host, stack: &ItemStack) -> Option<(Kind, &'static Research)> {
    let (kind, research) = match item_id(stack)?.as_str() {
        "FORTUNE_COOKIE" => (FORTUNE_COOKIE, &FORTUNE),
        "DIET_COOKIE" => (DIET_COOKIE, &DIET),
        "MONSTER_JERKY" => (MONSTER_JERKY, &JERKY),
        _ => return None,
    };
    kind.matches(host, stack).then_some((kind, research))
}

fn allowed(host: &mut dyn ServerHost, player: PlayerRef, research: &'static Research) -> bool {
    if research::can_use(host, player, research) {
        return true;
    }
    let _ = player.tell(
        host,
        Text::literal(format!(
            "Unlock {} with /sf research {}.",
            research.name, research.id
        )),
    );
    false
}

pub(crate) fn start(host: &mut dyn ServerHost, event: &LivingEntityUseItemStartEvent<'_>) -> Flow {
    if event.entity.is_none()
        && let Some((_, research)) = food(host, event.stack)
        && let Some(player) = host.players().acting_player()
        && !allowed(host, player, research)
    {
        return Flow::Cancel;
    }
    Flow::Continue
}

#[derive(Default)]
pub(crate) struct Snacks {
    pending: Mutex<BTreeMap<TaskId, PlayerRef>>,
}

impl Snacks {
    pub(crate) fn finish(
        &self,
        host: &mut dyn ServerHost,
        event: &LivingEntityUseItemFinishEvent<'_>,
        rolls: &Rolls,
    ) {
        if event.entity.is_some() {
            return;
        }
        let Some((item, research)) = food(host, event.stack) else {
            return;
        };
        let Some(player) = host.players().acting_player() else {
            return;
        };
        if !research::can_use(host, player, research) {
            return;
        }
        match item {
            FORTUNE_COOKIE => {
                if let Some(level) = player.entity().level(host)
                    && let Some(index) = rolls.next(host, level, FORTUNES.lines().count() as u32)
                    && let Some(fortune) = FORTUNES.lines().nth(index as usize)
                {
                    let mut message = Text::literal(fortune);
                    message.style.color = Some("gray".into());
                    let _ = player.tell(host, message);
                }
            }
            DIET_COOKIE => {
                let mut message = Text::literal("You are starting to feel very light...");
                message.style.color = Some("yellow".into());
                let _ = player.tell(host, message);
                play_eating_sound(host, player);
                let living = LivingRef::new(player.entity());
                if let Some(levitation) = host.effect_key(LEVITATION.0) {
                    let _ = living.remove_effect(host, levitation);
                    let _ = living.add_effect(
                        host,
                        &ActiveEffect::new(
                            levitation,
                            LEVITATION.1,
                            LEVITATION.2,
                            false,
                            true,
                            true,
                        ),
                    );
                }
            }
            MONSTER_JERKY => match host.schedule_task(1, 0) {
                Ok(task) => {
                    self.pending.lock().unwrap().insert(task, player);
                }
                Err(_) => finish_monster_jerky(host, player),
            },
            _ => unreachable!("only recognized snacks reach consumption"),
        }
    }

    pub(crate) fn due(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let player = self.pending.lock().unwrap().remove(&event.id);
        if let Some(player) = player
            && host.player(player.uuid()) == Some(player)
        {
            finish_monster_jerky(host, player);
        }
    }

    pub(crate) fn clear(&self) {
        self.pending.lock().unwrap().clear();
    }
}

fn finish_monster_jerky(host: &mut dyn ServerHost, player: PlayerRef) {
    let living = LivingRef::new(player.entity());
    if let Some(hunger) = host.effect_key("minecraft:hunger") {
        let _ = living.remove_effect(host, hunger);
    }
    add_effect(host, living, SATURATION);
}

fn add_effect(
    host: &mut dyn ServerHost,
    living: LivingRef,
    (id, duration, amplifier): (&str, i32, i32),
) {
    if let Some(effect) = host.effect_key(id) {
        let _ = living.add_effect(
            host,
            &ActiveEffect::new(effect, duration, amplifier, false, true, true),
        );
    }
}

fn play_eating_sound(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = player.play_sound(
        host,
        "minecraft:entity.generic.eat",
        SoundSource::Players,
        1.0,
        1.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn exact_paper_snack_recipes_keep_their_preexisting_prerequisites_and_research() {
        let expected = [
            (
                FORTUNE_COOKIE,
                "fortune_cookie",
                1,
                ["minecraft:cookie", "minecraft:paper", ""],
            ),
            (
                MAGIC_SUGAR,
                "magic_sugar",
                4,
                [
                    "minecraft:sugar",
                    "minecraft:redstone",
                    "minecraft:glowstone_dust",
                ],
            ),
            (
                MONSTER_JERKY,
                "monster_jerky",
                5,
                ["SALT", "minecraft:rotten_flesh", ""],
            ),
            (
                DIET_COOKIE,
                "diet_cookie",
                3,
                ["minecraft:cookie", "ELYTRA_SCALE", ""],
            ),
        ];
        for (recipe, (kind, id, cost, expected)) in
            CRAFTING.iter().chain([&DIET_RECIPE]).zip(expected)
        {
            assert_eq!((recipe.output.kind, recipe.output.count), (kind, 1));
            assert_eq!((recipe.research.id, recipe.research.cost), (id, cost));
            assert_eq!(
                recipe.inputs[..3]
                    .iter()
                    .map(|input| input.map_or("", Kind::id))
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(recipe.inputs[3..].iter().all(Option::is_none));
        }
    }

    #[test]
    fn the_edible_items_keep_vanilla_nutrition_and_leave_no_custom_remainder() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:cookie",
                    "minecraft:rotten_flesh",
                    "minecraft:sugar",
                ]
                .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        for (item, required) in [
            (FORTUNE_COOKIE, &FORTUNE),
            (DIET_COOKIE, &DIET),
            (MONSTER_JERKY, &JERKY),
        ] {
            let stack = item.stack(&host, 1).unwrap();
            assert_eq!(
                food(&host, &stack).map(|(_, research)| research.id),
                Some(required.id)
            );
            for component in [
                "minecraft:consumable",
                "minecraft:food",
                "minecraft:use_remainder",
            ] {
                let components = stack.components().unwrap().as_nbt();
                assert!(!components.contains_key(&component.into()));
                assert!(!components.contains_key(&format!("!{component}").as_str().into()));
            }
        }
        assert!(food(&host, &MAGIC_SUGAR.stack(&host, 1).unwrap()).is_none());
        assert!(food(&host, &ROTTEN_FLESH.stack(&host, 1).unwrap()).is_none());
    }

    #[test]
    fn sugar_skips_paper_altar_carriers_and_cookies_use_the_shipped_fortune_catalog() {
        assert!(sugar_exempt("minecraft:dispenser"));
        assert!(sugar_exempt("minecraft:enchanting_table"));
        assert!(!sugar_exempt("minecraft:dropper"));
        assert!(!sugar_exempt("minecraft:crafting_table"));
        assert_eq!(FORTUNES.lines().count(), 14);
        assert!(FORTUNES.lines().all(|fortune| !fortune.is_empty()));
    }
}
