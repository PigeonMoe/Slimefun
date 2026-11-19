// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    ActiveEffect, Compound, Flow, GameMode, Hand, HostTransaction, ItemPlace, ItemResource,
    ItemStack, LivingEntityUseItemFinishEvent, LivingEntityUseItemStartEvent, LivingRef, PlayerRef,
    ServerHost, SoundSource, Tag, Text,
};

use crate::{
    backpacks::CLOTH,
    food_machines::{CAN, FIRST_AID},
    items::{Kind, item, item_id, text},
    recipes::{CraftingRecipe, INGREDIENTS_AND_CHEESE, Research, Stack},
    research,
};

item!(RAG, "paper", "Rag", "red", None, None);
item!(BANDAGE, "paper", "Bandage", "red", None, None);
item!(SPLINT, "stick", "Splint", "red", None, None);
item!(VITAMINS, "nether_wart", "Vitamins", "red", None, None);
item!(MEDICINE, "potion", "Medicine", "red", None, None);
item!(HEAVY_CREAM, "snowball", "Heavy Cream", "white", None, None);

const STRING: Option<Kind> = Some(Kind::Vanilla("minecraft:string"));
const IRON: Option<Kind> = Some(Kind::Vanilla("minecraft:iron_ingot"));
const STICK: Option<Kind> = Some(Kind::Vanilla("minecraft:stick"));

const fn recipe(
    inputs: [Option<Kind>; 9],
    output: Kind,
    count: u32,
    research: &'static Research,
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack {
            kind: output,
            count,
        },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    recipe(
        [
            Some(CLOTH),
            Some(CLOTH),
            Some(CLOTH),
            STRING,
            None,
            STRING,
            Some(CLOTH),
            Some(CLOTH),
            Some(CLOTH),
        ],
        RAG,
        2,
        &FIRST_AID,
    ),
    recipe(
        [
            Some(RAG),
            STRING,
            Some(RAG),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        BANDAGE,
        4,
        &FIRST_AID,
    ),
    recipe(
        [None, IRON, None, STICK, STICK, STICK, None, IRON, None],
        SPLINT,
        4,
        &FIRST_AID,
    ),
    recipe(
        [
            Some(CAN),
            Some(Kind::Vanilla("minecraft:apple")),
            Some(Kind::Vanilla("minecraft:red_mushroom")),
            Some(Kind::Vanilla("minecraft:sugar")),
            None,
            None,
            None,
            None,
            None,
        ],
        VITAMINS,
        1,
        &FIRST_AID,
    ),
    recipe(
        [
            Some(VITAMINS),
            Some(Kind::Vanilla("minecraft:glass_bottle")),
            Some(HEAVY_CREAM),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        MEDICINE,
        1,
        &FIRST_AID,
    ),
    recipe(
        [
            Some(Kind::Vanilla("minecraft:milk_bucket")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        HEAVY_CREAM,
        2,
        &INGREDIENTS_AND_CHEESE,
    ),
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    let blank = || Tag::String("".into());
    let (tier, hearts, extinguish, cures, right_click) = match id {
        "RAG" => ("I", "2", true, false, true),
        "BANDAGE" => ("II", "4", true, false, true),
        "SPLINT" => ("I", "2", false, false, true),
        "VITAMINS" => ("III", "4", true, true, true),
        "MEDICINE" => {
            components.insert(
                "minecraft:potion_contents".into(),
                Tag::compound([("custom_color", Tag::Int(0xff0000))]),
            );
            ("III", "4", true, true, false)
        }
        _ => return,
    };
    let mut lore = vec![
        blank(),
        text(&format!("Level {tier} - Medical Supply"), "green", false),
        blank(),
        text(&format!("Restores {hearts} Hearts"), "white", false),
    ];
    if extinguish {
        lore.push(text("Extinguishes Fire", "white", false));
    }
    if cures {
        lore.push(text("Cures Poison/Wither/Radiation", "white", false));
    }
    if right_click {
        lore.extend([
            blank(),
            Tag::compound([
                ("text", Tag::String("".into())),
                ("italic", Tag::Byte(0)),
                (
                    "extra",
                    Tag::List(vec![
                        text("Right Click", "yellow", false),
                        text(" to use", "gray", false),
                    ]),
                ),
            ]),
        ]);
    }
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

const CURES: [&str; 8] = [
    "minecraft:poison",
    "minecraft:wither",
    "minecraft:slowness",
    "minecraft:mining_fatigue",
    "minecraft:weakness",
    "minecraft:nausea",
    "minecraft:blindness",
    "minecraft:bad_omen",
];

#[derive(Clone, Copy)]
enum Treatment {
    Wrap { amplifier: i32 },
    Splint,
    Vitamins,
}

impl Treatment {
    fn identify(host: &dyn ServerHost, stack: &ItemStack) -> Option<Self> {
        let (kind, result) = match item_id(stack)?.as_str() {
            "RAG" => (RAG, Self::Wrap { amplifier: 0 }),
            "BANDAGE" => (BANDAGE, Self::Wrap { amplifier: 1 }),
            "SPLINT" => (SPLINT, Self::Splint),
            "VITAMINS" => (VITAMINS, Self::Vitamins),
            _ => return None,
        };
        kind.matches(host, stack).then_some(result)
    }

    fn useful(self, health: f32, max_health: f32, fire: i32) -> bool {
        matches!(self, Self::Vitamins) || health < max_health || fire > 0
    }
}

pub(crate) fn use_supply(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    item: &ItemStack,
) -> bool {
    let Some(treatment) = Treatment::identify(host, item) else {
        return false;
    };
    let Some(mode @ (GameMode::Survival | GameMode::Adventure | GameMode::Creative)) =
        player.game_mode(host)
    else {
        return false;
    };
    if !can_use(host, player) {
        return true;
    }
    let living = LivingRef::new(player.entity());
    let (Some(health), Some(max_health), Some(fire)) = (
        living.health(host),
        living.max_health(host),
        player.entity().fire_ticks(host),
    ) else {
        return false;
    };
    if health <= 0.0 || !treatment.useful(health, max_health, fire) {
        return false;
    }
    if mode != GameMode::Creative {
        let slot = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let paid = host.transaction(|host, tx| {
            (slot.extract(host, tx, &ItemResource::of(item), 1) == 1)
                .then_some(())
                .ok_or(())
        });
        if paid.is_err() {
            return true;
        }
    }
    match treatment {
        Treatment::Wrap { amplifier } => {
            if let Some(level) = player.entity().level(host)
                && let Some(pos) = player.entity().block_position(host)
                && let Some(wool) = host.block_state_id("minecraft:white_wool")
            {
                let _ = level.block_break_effect(host, pos, wool);
            }
            instant_health(host, living, amplifier);
            let _ = player.entity().set_remaining_fire_ticks(host, 0);
        }
        Treatment::Splint => {
            let _ = player.play_sound(
                host,
                "minecraft:entity.skeleton.hurt",
                SoundSource::Players,
                1.0,
                1.0,
            );
            instant_health(host, living, 0);
        }
        Treatment::Vitamins => {
            let _ = player.play_sound(
                host,
                "minecraft:entity.generic.eat",
                SoundSource::Players,
                1.0,
                1.0,
            );
            cure(host, player);
        }
    }
    true
}

fn instant_health(host: &mut dyn ServerHost, living: LivingRef, amplifier: i32) {
    if let Some(effect) = host.effect_key("minecraft:instant_health") {
        let _ = living.add_effect(
            host,
            &ActiveEffect::new(effect, 1, amplifier, false, true, true),
        );
    }
}

fn cure(host: &mut dyn ServerHost, player: PlayerRef) {
    let living = LivingRef::new(player.entity());
    let _ = player.entity().set_remaining_fire_ticks(host, 0);
    for effect in CURES {
        if let Some(key) = host.effect_key(effect) {
            let _ = living.remove_effect(host, key);
        }
    }
    crate::radiation::clear_exposure(host, player);
    if let (Some(health), Some(max_health)) = (living.health(host), living.max_health(host)) {
        let _ = living.set_health(host, (health + 8.0).min(max_health));
    }
}

fn can_use(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    if research::can_use(host, player, &FIRST_AID) {
        return true;
    }
    let _ = player.tell(
        host,
        Text::literal("Unlock First Aid with /sf research first_aid."),
    );
    false
}

pub(crate) fn start(host: &mut dyn ServerHost, event: &LivingEntityUseItemStartEvent<'_>) -> Flow {
    if event.entity.is_none()
        && MEDICINE.matches(host, event.stack)
        && let Some(player) = host.players().acting_player()
        && !can_use(host, player)
    {
        return Flow::Cancel;
    }
    Flow::Continue
}

pub(crate) fn finish(host: &mut dyn ServerHost, event: &LivingEntityUseItemFinishEvent<'_>) {
    if event.entity.is_none()
        && MEDICINE.matches(host, event.stack)
        && let Some(player) = host.players().acting_player()
        && research::can_use(host, player, &FIRST_AID)
    {
        cure(host, player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn exact_paper_medical_recipes_include_first_aid_and_cuisine_prerequisites() {
        let expected = [
            (
                RAG,
                2,
                [
                    "CLOTH",
                    "CLOTH",
                    "CLOTH",
                    "minecraft:string",
                    "",
                    "minecraft:string",
                    "CLOTH",
                    "CLOTH",
                    "CLOTH",
                ],
            ),
            (
                BANDAGE,
                4,
                ["RAG", "minecraft:string", "RAG", "", "", "", "", "", ""],
            ),
            (
                SPLINT,
                4,
                [
                    "",
                    "minecraft:iron_ingot",
                    "",
                    "minecraft:stick",
                    "minecraft:stick",
                    "minecraft:stick",
                    "",
                    "minecraft:iron_ingot",
                    "",
                ],
            ),
            (
                VITAMINS,
                1,
                [
                    "CAN",
                    "minecraft:apple",
                    "minecraft:red_mushroom",
                    "minecraft:sugar",
                    "",
                    "",
                    "",
                    "",
                    "",
                ],
            ),
            (
                MEDICINE,
                1,
                [
                    "VITAMINS",
                    "minecraft:glass_bottle",
                    "HEAVY_CREAM",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                ],
            ),
            (
                HEAVY_CREAM,
                2,
                ["minecraft:milk_bucket", "", "", "", "", "", "", "", ""],
            ),
        ];
        assert_eq!(CRAFTING.len(), expected.len());
        for (recipe, (kind, count, inputs)) in CRAFTING.iter().zip(expected) {
            assert_eq!(recipe.output.kind, kind);
            assert_eq!(recipe.output.count, count);
            assert_eq!(recipe.inputs.map(|kind| kind.map_or("", Kind::id)), inputs);
            assert_eq!(
                recipe.research.id,
                if kind == HEAVY_CREAM {
                    "ingredients_and_cheese"
                } else {
                    "first_aid"
                }
            );
        }
        assert_eq!(FIRST_AID.cost, 2);
    }

    #[test]
    fn wraps_and_splints_handle_injury_or_fire_and_vitamins_need_neither() {
        for treatment in [
            Treatment::Wrap { amplifier: 0 },
            Treatment::Wrap { amplifier: 1 },
            Treatment::Splint,
        ] {
            assert!(!treatment.useful(24.0, 24.0, 0));
            assert!(treatment.useful(24.0, 24.0, 1));
            assert!(treatment.useful(23.0, 24.0, -2));
        }
        assert!(Treatment::Vitamins.useful(24.0, 24.0, 0));
        assert_eq!(
            CURES,
            [
                "minecraft:poison",
                "minecraft:wither",
                "minecraft:slowness",
                "minecraft:mining_fatigue",
                "minecraft:weakness",
                "minecraft:nausea",
                "minecraft:blindness",
                "minecraft:bad_omen"
            ]
        );
    }

    #[test]
    fn medicine_uses_native_potion_without_removing_the_bottle_remainder() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:potion").unwrap()])
            .unwrap();
        let medicine = MEDICINE.stack(&host, 1).unwrap();
        assert!(MEDICINE.matches(&host, &medicine));
        assert!(Treatment::identify(&host, &medicine).is_none());
        let components = medicine.components().unwrap().as_nbt();
        assert_eq!(
            components.get(&"minecraft:potion_contents".into()),
            Some(&Tag::compound([("custom_color", Tag::Int(0xff0000))]))
        );
        assert!(!components.contains_key(&"!minecraft:use_remainder".into()));
    }
}
