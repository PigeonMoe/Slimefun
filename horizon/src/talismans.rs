// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeSet, sync::Mutex};

use mod_api::{
    ActiveEffect, BlockState, Hand, Host, HostSide, HostTransaction, InteractionResult, ItemDrop,
    ItemPart, ItemStack, LevelRef, PlayerRef, Registrar, ServerHost, Tag, Text, UseOnEvent,
};

use crate::{
    ancient_altar::ANCIENT_RUNE_EARTH,
    elemental_staves::{LAVA_CRYSTAL, STAFF_ELEMENTAL_WIND},
    factory::experience_collector::FILLED_FLASK_OF_KNOWLEDGE,
    items::{
        ENDER_LUMP_3, GOLD_8K, GOLD_DUST, Kind, MAGIC_LUMP_2, MAGIC_LUMP_3, SIFTED_ORE, item, text,
    },
    magic_eye::MAGIC_EYE_OF_ENDER,
    network_recipes::SYNTHETIC_SAPPHIRE,
    random::{Rolls, natural_drop_position},
    recipes::{CraftingRecipe, Research, Stack},
    research,
    smelters_pickaxe::{enchantment, fortune_amount, ore},
};

#[path = "anvil_talisman.rs"]
pub(crate) mod anvil;

item!(
    COMMON_TALISMAN,
    "emerald",
    "Common Talisman",
    "gold",
    None,
    None
);
item!(
    MAGICAL_GLASS,
    "glass_pane",
    "Magical Glass",
    "gold",
    None,
    None
);
item!(
    MAGICIAN_TALISMAN,
    "emerald",
    "Talisman of the Magician",
    "green",
    None,
    None
);
item!(
    ENDER_MAGICIAN_TALISMAN,
    "emerald",
    "Ender Talisman of the Magician",
    "dark_purple",
    None,
    None
);
item!(
    WIZARD_TALISMAN,
    "emerald",
    "Talisman of the Wizard",
    "green",
    None,
    None
);
item!(
    ENDER_WIZARD_TALISMAN,
    "emerald",
    "Ender Talisman of the Wizard",
    "dark_purple",
    None,
    None
);
item!(
    WISE_TALISMAN,
    "emerald",
    "Talisman of the Wise",
    "green",
    None,
    None
);
item!(
    ENDER_WISE_TALISMAN,
    "emerald",
    "Ender Talisman of the Wise",
    "dark_purple",
    None,
    None
);

item!(
    FIRE_TALISMAN,
    "emerald",
    "Talisman of the Firefighter",
    "green",
    None,
    None
);
item!(
    ENDER_FIRE_TALISMAN,
    "emerald",
    "Ender Talisman of the Firefighter",
    "dark_purple",
    None,
    None
);
item!(
    ANGEL_TALISMAN,
    "emerald",
    "Talisman of the Angel",
    "green",
    None,
    None
);
item!(
    ENDER_ANGEL_TALISMAN,
    "emerald",
    "Ender Talisman of the Angel",
    "dark_purple",
    None,
    None
);
item!(
    TRAVELLER_TALISMAN,
    "emerald",
    "Talisman of the Traveller",
    "green",
    None,
    None
);
item!(
    ENDER_TRAVELLER_TALISMAN,
    "emerald",
    "Ender Talisman of the Traveller",
    "dark_purple",
    None,
    None
);
item!(
    MINER_TALISMAN,
    "emerald",
    "Talisman of the Miner",
    "green",
    None,
    None
);
item!(
    ENDER_MINER_TALISMAN,
    "emerald",
    "Ender Talisman of the Miner",
    "dark_purple",
    None,
    None
);
item!(
    FARMER_TALISMAN,
    "emerald",
    "Talisman of the Farmer",
    "green",
    None,
    None
);
item!(
    ENDER_FARMER_TALISMAN,
    "emerald",
    "Ender Talisman of the Farmer",
    "dark_purple",
    None,
    None
);
item!(
    CAVEMAN_TALISMAN,
    "emerald",
    "Talisman of the Caveman",
    "green",
    None,
    None
);
item!(
    ENDER_CAVEMAN_TALISMAN,
    "emerald",
    "Ender Talisman of the Caveman",
    "dark_purple",
    None,
    None
);

const FIRE_RESEARCH: Research = Research {
    id: "fire_talisman",
    name: "Talisman of the Firefighter",
    cost: 18,
};
const ANGEL_RESEARCH: Research = Research {
    id: "angel_talisman",
    name: "Talisman of the Angel",
    cost: 18,
};
const TRAVELLER_RESEARCH: Research = Research {
    id: "traveller_talisman",
    name: "Talisman of the Traveller",
    cost: 20,
};
const MINER_RESEARCH: Research = Research {
    id: "miner_talisman",
    name: "Talisman of the Miner",
    cost: 18,
};
const FARMER_RESEARCH: Research = Research {
    id: "farmer_talisman",
    name: "Talisman of the Farmer",
    cost: 18,
};
const CAVEMAN_RESEARCH: Research = Research {
    id: "caveman_talisman",
    name: "Talisman of the Caveman",
    cost: 20,
};
const COMMON_RESEARCH: Research = Research {
    id: "common_talisman",
    name: "Common Talisman",
    cost: 14,
};
pub(crate) const GLASS_RESEARCH: Research = Research {
    id: "enchantment_rune",
    name: "Ancient Enchanting",
    cost: 24,
};
const MAGICIAN_RESEARCH: Research = Research {
    id: "magician_talisman",
    name: "Talisman of the Magician",
    cost: 20,
};
const WIZARD_RESEARCH: Research = Research {
    id: "wizard_talisman",
    name: "Talisman of the Wizard",
    cost: 22,
};
const WISE_RESEARCH: Research = Research {
    id: "wise_talisman",
    name: "Talisman of the Wise",
    cost: 20,
};
const ENDER_RESEARCH: Research = Research {
    id: "ender_talismans",
    name: "Ender Talismans",
    cost: 28,
};
pub(crate) const RESEARCHES: &[&Research] = &[
    &COMMON_RESEARCH,
    &anvil::RESEARCH,
    &GLASS_RESEARCH,
    &MAGICIAN_RESEARCH,
    &WIZARD_RESEARCH,
    &WISE_RESEARCH,
    &FIRE_RESEARCH,
    &ANGEL_RESEARCH,
    &TRAVELLER_RESEARCH,
    &MINER_RESEARCH,
    &FARMER_RESEARCH,
    &CAVEMAN_RESEARCH,
    &ENDER_RESEARCH,
];

const LUMP_2: Option<Kind> = Some(MAGIC_LUMP_2);
const LUMP_3: Option<Kind> = Some(MAGIC_LUMP_3);
const ENDER: Option<Kind> = Some(ENDER_LUMP_3);
const XP: Option<Kind> = Some(FILLED_FLASK_OF_KNOWLEDGE);
const GOLD: Option<Kind> = Some(GOLD_8K);
const GLASS: Option<Kind> = Some(MAGICAL_GLASS);
const EYE: Option<Kind> = Some(MAGIC_EYE_OF_ENDER);
const TABLE: Option<Kind> = Some(Kind::Vanilla("minecraft:enchanting_table"));

const fn recipe(
    output: Kind,
    research: &'static Research,
    count: u32,
    inputs: [Option<Kind>; 9],
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

const fn infused(output: Kind, base: Kind, count: u32) -> CraftingRecipe {
    recipe(
        output,
        &ENDER_RESEARCH,
        count,
        [
            ENDER,
            None,
            ENDER,
            None,
            Some(base),
            None,
            ENDER,
            None,
            ENDER,
        ],
    )
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    recipe(
        COMMON_TALISMAN,
        &COMMON_RESEARCH,
        1,
        [
            LUMP_2,
            GOLD,
            LUMP_2,
            None,
            Some(Kind::Vanilla("minecraft:emerald")),
            None,
            LUMP_2,
            GOLD,
            LUMP_2,
        ],
    ),
    recipe(
        MAGICAL_GLASS,
        &GLASS_RESEARCH,
        1,
        [
            LUMP_2,
            Some(GOLD_DUST),
            LUMP_2,
            XP,
            Some(Kind::Vanilla("minecraft:glass_pane")),
            XP,
            LUMP_2,
            XP,
            LUMP_2,
        ],
    ),
    recipe(
        MAGICIAN_TALISMAN,
        &MAGICIAN_RESEARCH,
        1,
        [
            ENDER,
            None,
            ENDER,
            TABLE,
            Some(COMMON_TALISMAN),
            TABLE,
            ENDER,
            None,
            ENDER,
        ],
    ),
    recipe(
        WIZARD_TALISMAN,
        &WIZARD_RESEARCH,
        1,
        [
            ENDER,
            None,
            ENDER,
            EYE,
            Some(MAGICIAN_TALISMAN),
            EYE,
            ENDER,
            None,
            ENDER,
        ],
    ),
    recipe(
        WISE_TALISMAN,
        &WISE_RESEARCH,
        1,
        [
            LUMP_3,
            GLASS,
            LUMP_3,
            XP,
            Some(MAGICIAN_TALISMAN),
            XP,
            LUMP_3,
            GLASS,
            LUMP_3,
        ],
    ),
    infused(ENDER_MAGICIAN_TALISMAN, MAGICIAN_TALISMAN, 1),
    infused(ENDER_WIZARD_TALISMAN, WIZARD_TALISMAN, 1),
    infused(ENDER_WISE_TALISMAN, WISE_TALISMAN, 1),
    recipe(
        FIRE_TALISMAN,
        &FIRE_RESEARCH,
        4,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(LAVA_CRYSTAL),
            Some(COMMON_TALISMAN),
            Some(LAVA_CRYSTAL),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_FIRE_TALISMAN, FIRE_TALISMAN, 4),
    recipe(
        ANGEL_TALISMAN,
        &ANGEL_RESEARCH,
        1,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(Kind::Vanilla("minecraft:feather")),
            Some(COMMON_TALISMAN),
            Some(Kind::Vanilla("minecraft:feather")),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    recipe(
        TRAVELLER_TALISMAN,
        &TRAVELLER_RESEARCH,
        1,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(STAFF_ELEMENTAL_WIND),
            Some(ANGEL_TALISMAN),
            Some(STAFF_ELEMENTAL_WIND),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_ANGEL_TALISMAN, ANGEL_TALISMAN, 1),
    infused(ENDER_TRAVELLER_TALISMAN, TRAVELLER_TALISMAN, 1),
    recipe(
        MINER_TALISMAN,
        &MINER_RESEARCH,
        1,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(SYNTHETIC_SAPPHIRE),
            Some(COMMON_TALISMAN),
            Some(SIFTED_ORE),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_MINER_TALISMAN, MINER_TALISMAN, 1),
    recipe(
        FARMER_TALISMAN,
        &FARMER_RESEARCH,
        1,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(Kind::Vanilla("minecraft:diamond_hoe")),
            Some(COMMON_TALISMAN),
            Some(Kind::Vanilla("minecraft:diamond_hoe")),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_FARMER_TALISMAN, FARMER_TALISMAN, 1),
    recipe(
        CAVEMAN_TALISMAN,
        &CAVEMAN_RESEARCH,
        1,
        [
            LUMP_3,
            None,
            LUMP_3,
            Some(Kind::Vanilla("minecraft:golden_pickaxe")),
            Some(MINER_TALISMAN),
            Some(ANCIENT_RUNE_EARTH),
            LUMP_3,
            None,
            LUMP_3,
        ],
    ),
    infused(ENDER_CAVEMAN_TALISMAN, CAVEMAN_TALISMAN, 1),
];

const MAGICIAN: Talisman = Talisman {
    normal: MAGICIAN_TALISMAN,
    ender: ENDER_MAGICIAN_TALISMAN,
    research: &MAGICIAN_RESEARCH,
    chance: 80,
    message: "Your Talisman gave you an additional Enchantment",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it gives",
        "you a 80% Luck Bonus on Enchanting",
        "You will sometimes get an Extra Enchantment",
    ],
};
const WIZARD: Talisman = Talisman {
    normal: WIZARD_TALISMAN,
    ender: ENDER_WIZARD_TALISMAN,
    research: &WIZARD_RESEARCH,
    chance: 60,
    message: "Your Talisman has given you a better Fortune Level but maybe also lowered some other Enchantment Levels",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it allows you to",
        "obtain Fortune Level 4/5 however",
        "it also has a chance to lower the",
        "Level of some Enchantments on your Item",
    ],
};
const WISE: Talisman = Talisman {
    normal: WISE_TALISMAN,
    ender: ENDER_WISE_TALISMAN,
    research: &WISE_RESEARCH,
    chance: 20,
    message: "Your Talisman has doubled your experience drops",
    lore: &[
        "While you have this Talisman",
        "in your inventory it gives",
        "you a 20% chance of doubling",
        "any experience you obtain",
    ],
};

const FIRE: Talisman = Talisman {
    normal: FIRE_TALISMAN,
    ender: ENDER_FIRE_TALISMAN,
    research: &FIRE_RESEARCH,
    chance: 100,
    message: "Your Talisman saved you from burning to death",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it will",
        "give you Fire Resistance",
        "as soon as you start burning",
        "but will then be consumed",
    ],
};

const ANGEL: Talisman = Talisman {
    normal: ANGEL_TALISMAN,
    ender: ENDER_ANGEL_TALISMAN,
    research: &ANGEL_RESEARCH,
    chance: 75,
    message: "Your Talisman saved you from taking fall damage",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it has a",
        "75% chance to prevent you",
        "from taking Fall Damage",
    ],
};

const TRAVELLER: Talisman = Talisman {
    normal: TRAVELLER_TALISMAN,
    ender: ENDER_TRAVELLER_TALISMAN,
    research: &TRAVELLER_RESEARCH,
    chance: 60,
    message: "Your Talisman gave you a Speed Boost",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it gives",
        "you a 60% Chance for a decent",
        "Speed Buff when you start sprinting",
    ],
};
const MINER: Talisman = Talisman {
    normal: MINER_TALISMAN,
    ender: ENDER_MINER_TALISMAN,
    research: &MINER_RESEARCH,
    chance: 20,
    message: "Your Talisman just doubled your drops",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it has",
        "a 20% chance of doubling",
        "all Ores you mine",
    ],
};
const FARMER: Talisman = Talisman {
    normal: FARMER_TALISMAN,
    ender: ENDER_FARMER_TALISMAN,
    research: &FARMER_RESEARCH,
    chance: 20,
    message: "Your Talisman just doubled your drops",
    lore: &[
        "While you have this Talisman",
        "in your Inventory it has",
        "a 20% chance of doubling",
        "all crops you harvest",
    ],
};
const CAVEMAN: Talisman = Talisman {
    normal: CAVEMAN_TALISMAN,
    ender: ENDER_CAVEMAN_TALISMAN,
    research: &CAVEMAN_RESEARCH,
    chance: 50,
    message: "Your Talisman gave you haste",
    lore: &[
        "While you have this Talisman",
        "in your inventory it gives",
        "you a 50% chance for a decent",
        "Haste buff when you mine any ore",
    ],
};

struct Talisman {
    normal: Kind,
    ender: Kind,
    research: &'static Research,
    chance: u32,
    message: &'static str,
    lore: &'static [&'static str],
}

pub(crate) fn lore(id: &str) -> Option<Tag> {
    if let Some(lore) = anvil::lore(id) {
        return Some(lore);
    }
    let blank = || Tag::String("".into());
    if id == MAGICAL_GLASS.id() {
        return Some(Tag::List(vec![
            blank(),
            text("Used for various Magical Gadgets", "green", true),
        ]));
    }
    let talisman = [
        &MAGICIAN, &WIZARD, &WISE, &FIRE, &ANGEL, &TRAVELLER, &MINER, &FARMER, &CAVEMAN,
    ]
    .into_iter()
    .find(|talisman| [talisman.normal.id(), talisman.ender.id()].contains(&id))?;
    let mut lines = Vec::new();
    if talisman.ender.id() == id {
        lines.push(text("Ender Infused", "gray", true));
        lines.push(blank());
    }
    lines.push(blank());
    lines.extend(talisman.lore.iter().map(|line| text(line, "white", false)));
    Some(Tag::List(lines))
}

struct UnplaceableGlass;

#[mod_api::horizon_part]
impl ItemPart for UnplaceableGlass {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        MAGICAL_GLASS
            .matches(host, event.stack)
            .then_some(InteractionResult::Fail)
    }
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let key = registrar.item_key(MAGICAL_GLASS.material());
    registrar.item_part(key, UnplaceableGlass);
}

pub(crate) const DEFAULT_CONFIG: &str =
    "\n[talismans]\nuse-actionbar = true\n\n[magician-talisman]\nallow-enchantment-books = false\n";

#[derive(Clone)]
struct Rules {
    actionbar: bool,
    allow_books: bool,
    disabled_entries: BTreeSet<(String, u32)>,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            actionbar: true,
            allow_books: false,
            disabled_entries: BTreeSet::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct Talismans(Mutex<Rules>);

impl Talismans {
    pub(crate) fn caveman(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        state: BlockState,
        rolls: &Rolls,
    ) {
        if !ore(&state.block().id(host)) {
            return;
        }
        let (Some(living), Some(haste)) = (
            host.living(player.entity()),
            host.effect_key("minecraft:haste"),
        ) else {
            return;
        };
        if !chance(host, level, &CAVEMAN, rolls)
            || living.effect(host, haste).is_some()
            || !present(host, player, &CAVEMAN)
        {
            return;
        }
        let effect = ActiveEffect::new(haste, 800, 2, true, true, true);
        if living.add_effect(host, &effect) {
            let actionbar = self.0.lock().unwrap().actionbar;
            notify(host, player, &CAVEMAN, actionbar);
        }
    }

    pub(crate) fn harvest_drops(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        pos: [i32; 3],
        state: BlockState,
        drops: &[ItemDrop],
        rolls: &Rolls,
    ) {
        let key = state.block();
        let block = key.id(host);
        let in_crops = host.block_in_tag(key, "minecraft:crops") == Some(true);
        let talismans = harvest_talismans(&block, in_crops);
        if talismans.iter().all(Option::is_none) {
            return;
        }
        let Some(level) = player.level(host) else {
            return;
        };
        let Some(tool) = host
            .living(player.entity())
            .and_then(|living| living.hand_item(host, Hand::Main))
        else {
            return;
        };
        if enchantment(host, &tool, "minecraft:silk_touch") != 0 {
            return;
        }
        let fortune = enchantment(host, &tool, "minecraft:fortune");
        // Paper tries Miner first, then Farmer, always against the original event drops.
        for talisman in talismans.into_iter().flatten() {
            if !chance(host, level, talisman, rolls) || !present(host, player, talisman) {
                continue;
            }
            let amount = fortune_amount(fortune, false, &block, |bound| {
                rolls.next(host, level, bound).unwrap_or(0)
            });
            let mut doubled = false;
            for drop in drops {
                let is_block = host.block_key(&drop.stack.item().id(host)).is_some();
                if let Some(stack) = harvest_bonus(&drop.stack, amount, is_block) {
                    let [x, y, z] = natural_drop_position(host, level, pos);
                    if matches!(level.spawn_item(host, x, y, z, stack, 10), Ok(Some(_))) {
                        doubled = true;
                    }
                }
            }
            if doubled {
                let actionbar = self.0.lock().unwrap().actionbar;
                notify(host, player, talisman, actionbar);
            }
        }
    }

    pub(crate) fn configure(&self, table: &toml::Table) {
        let mut rules = Rules::default();
        if let Some(actionbar) = table
            .get("talismans")
            .and_then(|section| section.get("use-actionbar"))
            .and_then(toml::Value::as_bool)
        {
            rules.actionbar = actionbar;
        }
        if let Some(magician) = table.get("magician-talisman") {
            rules.allow_books = magician
                .get("allow-enchantment-books")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            if let Some(namespaces) = magician
                .get("allow-enchantments")
                .and_then(toml::Value::as_table)
            {
                for (namespace, enchantments) in namespaces {
                    if let Some(enchantments) = enchantments.as_table() {
                        for (name, config) in enchantments {
                            if let Some(levels) =
                                config.get("level").and_then(toml::Value::as_table)
                            {
                                for (level, allowed) in levels {
                                    if allowed.as_bool() == Some(false)
                                        && let Ok(level) = level.parse::<u32>()
                                    {
                                        rules
                                            .disabled_entries
                                            .insert((format!("{namespace}:{name}"), level));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        *self.0.lock().unwrap() = rules;
    }

    pub(crate) fn enchant(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        input: &ItemStack,
        enchants: &mut Vec<(String, u32)>,
        book_extra: &mut Vec<(String, u32)>,
        rolls: &Rolls,
    ) {
        let Some(level) = player.level(host) else {
            return;
        };
        let rules = self.0.lock().unwrap().clone();
        let loose_book = rules.allow_books && input.item().id(host) == "minecraft:book";
        if present(host, player, &MAGICIAN) {
            let candidates = possible_magic_enchants(
                host.enchantment_keys().into_iter().filter_map(|key| {
                    let id = key.id(host);
                    let max_level = host.enchantment_max_level(&id)?;
                    Some((id, max_level))
                }),
                |id| loose_book || host.enchantment_supports(id, input.item()),
                |id| {
                    enchants
                        .iter()
                        .all(|(old, _)| host.enchantments_compatible(old, id))
                },
                &rules.disabled_entries,
            );
            if !candidates.is_empty()
                && let Some(selected) = rolls
                    .next(host, level, candidates.len() as u32)
                    .and_then(|index| candidates.get(index as usize).cloned())
                && chance(host, level, &MAGICIAN, rolls)
            {
                let output = if loose_book {
                    book_extra
                } else {
                    &mut *enchants
                };
                set_enchantment(output, selected);
                notify(host, player, &MAGICIAN, rules.actionbar);
            }
        }

        if !enchants.iter().any(|(id, _)| id == "minecraft:silk_touch")
            && host.enchantment_supports("minecraft:fortune", input.item())
            && activated(host, player, level, &WIZARD, rolls)
        {
            for (_, level_value) in enchants.iter_mut() {
                if *level_value > 1 && rolls.next(host, level, 100).is_some_and(|roll| roll < 40) {
                    *level_value -= 1;
                }
            }
            if let Some(bonus) = rolls.next(host, level, 3) {
                set_enchantment(enchants, ("minecraft:fortune".into(), bonus + 3));
                notify(host, player, &WIZARD, rules.actionbar);
            }
        }
    }

    pub(crate) fn protect_from_fire(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        damage_type: &str,
    ) -> bool {
        if damage_type != "minecraft:in_fire" || !learned(host, player, &FIRE) {
            return false;
        }
        let Some(living) = host.living(player.entity()) else {
            return false;
        };
        let Some(effect) = host.effect_key("minecraft:fire_resistance") else {
            return false;
        };
        if living.effect(host, effect).is_some() {
            return false;
        }
        let activated = host.transaction(|host, _| {
            consume_carried(host, player, &FIRE)?;
            let resistance = ActiveEffect::new(effect, 3600, 4, false, true, true);
            living.add_effect(host, &resistance).then_some(()).ok_or(())
        });
        if activated.is_ok() {
            let actionbar = self.0.lock().unwrap().actionbar;
            notify(host, player, &FIRE, actionbar);
            true
        } else {
            false
        }
    }

    pub(crate) fn protect_from_fall(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        rolls: &Rolls,
    ) -> bool {
        let Some(level) = player.level(host) else {
            return false;
        };
        if !chance(host, level, &ANGEL, rolls) || !present(host, player, &ANGEL) {
            return false;
        }
        let actionbar = self.0.lock().unwrap().actionbar;
        notify(host, player, &ANGEL, actionbar);
        true
    }

    pub(crate) fn start_sprinting(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        rolls: &Rolls,
    ) {
        let (Some(level), Some(living), Some(speed)) = (
            player.level(host),
            host.living(player.entity()),
            host.effect_key("minecraft:speed"),
        ) else {
            return;
        };
        if !chance(host, level, &TRAVELLER, rolls)
            || living.effect(host, speed).is_some()
            || !present(host, player, &TRAVELLER)
        {
            return;
        }
        let speed = ActiveEffect::new(speed, 3600, 2, true, true, true);
        let _ = living.add_effect(host, &speed);
        let actionbar = self.0.lock().unwrap().actionbar;
        notify(host, player, &TRAVELLER, actionbar);
    }

    pub(crate) fn orb_xp(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        amount: i32,
        rolls: &Rolls,
    ) -> i32 {
        if amount > 0
            && let Some(level) = player.level(host)
            && activated(host, player, level, &WISE, rolls)
        {
            let actionbar = self.0.lock().unwrap().actionbar;
            notify(host, player, &WISE, actionbar);
            amount.saturating_mul(2)
        } else {
            amount
        }
    }
}

fn set_enchantment(enchants: &mut Vec<(String, u32)>, (name, level): (String, u32)) {
    if let Some((_, current)) = enchants.iter_mut().find(|(id, _)| id == &name) {
        *current = level;
    } else {
        enchants.push((name, level));
    }
}

fn possible_magic_enchants(
    registered: impl IntoIterator<Item = (String, u32)>,
    supported: impl Fn(&str) -> bool,
    compatible: impl Fn(&str) -> bool,
    disabled: &BTreeSet<(String, u32)>,
) -> Vec<(String, u32)> {
    let mut candidates = Vec::new();
    for (id, max_level) in registered {
        if supported(&id) && compatible(&id) {
            for level in 1..=max_level.min(255) {
                let candidate = (id.clone(), level);
                if !disabled.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
    }
    candidates
}

fn present(host: &dyn ServerHost, player: PlayerRef, spec: &Talisman) -> bool {
    carried_talisman(host, player, spec).is_some() && learned(host, player, spec)
}

fn miner_triggers(block: &str) -> bool {
    block == "minecraft:gilded_blackstone" || (block != "minecraft:ancient_debris" && ore(block))
}

fn harvest_talismans(block: &str, in_crops: bool) -> [Option<&'static Talisman>; 2] {
    let farmer = in_crops
        || matches!(
            block,
            "minecraft:beetroots"
                | "minecraft:carrots"
                | "minecraft:potatoes"
                | "minecraft:wheat"
                | "minecraft:cocoa"
                | "minecraft:sweet_berry_bush"
                | "minecraft:melon"
        );
    [
        miner_triggers(block).then_some(&MINER),
        farmer.then_some(&FARMER),
    ]
}

fn harvest_bonus(original: &ItemStack, amount: u32, is_block: bool) -> Option<ItemStack> {
    if is_block {
        return None;
    }
    let count = (amount * 2).saturating_sub(original.count()).max(1);
    let mut stack = original.clone();
    stack.set_count(count).ok()?;
    Some(stack)
}

fn learned(host: &dyn ServerHost, player: PlayerRef, spec: &Talisman) -> bool {
    player
        .persistent_data(host)
        .is_some_and(|data| can_activate(host.dimension(), spec, &data))
}

fn can_activate(dimension: &str, spec: &Talisman, data: &mod_api::Compound) -> bool {
    // Paper calls the base talisman's canUse even when it found the Ender item.
    crate::availability::use_denial(dimension, spec.normal.id()).is_none()
        && research::unlocked(data, spec.research)
}

enum StorageSlot {
    Inventory(u32),
    EnderChest(u32),
}

fn carried_talisman(
    host: &dyn Host,
    player: PlayerRef,
    spec: &Talisman,
) -> Option<(StorageSlot, ItemStack)> {
    // Bukkit's getStorageContents checks ordinary inventory first. A normal
    // talisman in armor, offhand, or a nested container does not activate.
    let main = player.inventory_slots(host).unwrap_or_default();
    if let Some((slot, stack)) = first_matching(host, main, 36, spec.normal) {
        return Some((StorageSlot::Inventory(slot), stack));
    }
    let ender = player.ender_chest(host).unwrap_or_default();
    first_matching(host, ender, 27, spec.ender)
        .map(|(slot, stack)| (StorageSlot::EnderChest(slot), stack))
}

fn consume_carried(host: &mut dyn ServerHost, player: PlayerRef, spec: &Talisman) -> Result<(), ()> {
    let (slot, current) = carried_talisman(host, player, spec).ok_or(())?;
    let remainder = current.shrink(1);
    match slot {
        StorageSlot::Inventory(slot) => player.set_inventory_slot(host, slot, remainder),
        StorageSlot::EnderChest(slot) => player.set_ender_chest_slot(host, slot, remainder),
    }
    .map_err(|_| ())
}

fn first_matching(
    host: &dyn Host,
    slots: impl IntoIterator<Item = (u32, ItemStack)>,
    size: u32,
    expected: Kind,
) -> Option<(u32, ItemStack)> {
    slots
        .into_iter()
        .filter(|(slot, stack)| *slot < size && expected.matches(host, stack))
        .min_by_key(|(slot, _)| *slot)
}

fn activated(
    host: &dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    spec: &Talisman,
    rolls: &Rolls,
) -> bool {
    present(host, player, spec) && chance(host, level, spec, rolls)
}

fn chance(host: &dyn Host, level: LevelRef, spec: &Talisman, rolls: &Rolls) -> bool {
    rolls
        .next(host, level, 100)
        .is_some_and(|value| roll_succeeds(spec, value))
}

fn roll_succeeds(spec: &Talisman, value: u32) -> bool {
    // Paper rejects when nextInt(100) > chance, so the boundary succeeds.
    value <= spec.chance
}

fn notify(host: &mut dyn ServerHost, player: PlayerRef, spec: &Talisman, actionbar: bool) {
    let mut line = Text::literal(spec.message);
    line.style.color = Some("green".into());
    line.style.italic = Some(true);
    if actionbar {
        let _ = player.action_bar(host, line);
    } else {
        let _ = player.tell(host, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::testing::FakeHost;

    #[test]
    fn miner_augments_each_nonblock_drop_with_papers_fortune_amount() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([mod_api::Identifier::parse("minecraft:lapis_lazuli").unwrap()])
            .unwrap();
        let mut original = Kind::Vanilla("minecraft:lapis_lazuli")
            .stack(&host, 23)
            .unwrap();
        original
            .apply_components(&mod_api::Compound::from([(
                "minecraft:custom_name".into(),
                Tag::String("Mining sample".into()),
            )]))
            .unwrap();
        let mut draws = [(5, 4), (5, 1)].into_iter();
        let amount = fortune_amount(3, false, "minecraft:lapis_ore", |bound| {
            let (expected, value) = draws.next().unwrap();
            assert_eq!(bound, expected);
            value
        });
        let extra = harvest_bonus(&original, amount, false).unwrap();
        assert_eq!(amount, 20);
        assert_eq!(extra.count(), 17);
        assert_eq!(
            extra.components().unwrap().as_nbt(),
            original.components().unwrap().as_nbt()
        );
        assert_eq!(original.count(), 23);
        assert_eq!(harvest_bonus(&original, 1, false).unwrap().count(), 1);
        assert!(harvest_bonus(&original, amount, true).is_none());
        assert!(roll_succeeds(&MINER, 20));
        assert!(!roll_succeeds(&MINER, 21));
    }

    #[test]
    fn farmer_uses_paper_crop_tags_and_keeps_miner_first_when_tags_overlap() {
        let markers = |block, tagged| {
            harvest_talismans(block, tagged).map(|entry| entry.map(|talisman| talisman.normal.id()))
        };
        for block in [
            "minecraft:beetroots",
            "minecraft:carrots",
            "minecraft:potatoes",
            "minecraft:wheat",
            "minecraft:cocoa",
            "minecraft:sweet_berry_bush",
            "minecraft:melon",
        ] {
            assert_eq!(markers(block, false), [None, Some("FARMER_TALISMAN")]);
        }
        assert_eq!(markers("minecraft:pumpkin", false), [None, None]);
        assert_eq!(
            markers("minecraft:nether_wart", true),
            [None, Some("FARMER_TALISMAN")]
        );
        assert_eq!(
            markers("minecraft:diamond_ore", true),
            [Some("MINER_TALISMAN"), Some("FARMER_TALISMAN")]
        );
        assert!(roll_succeeds(&FARMER, 20));
        assert!(!roll_succeeds(&FARMER, 21));
    }

    #[test]
    fn farmer_research_recipes_and_localized_lore_cover_both_tiers() {
        let recipe = |output| {
            CRAFTING
                .iter()
                .find(|recipe| recipe.output.kind == output)
                .unwrap()
        };
        let base = recipe(FARMER_TALISMAN);
        let ender = recipe(ENDER_FARMER_TALISMAN);
        let hoe = Some(Kind::Vanilla("minecraft:diamond_hoe"));
        assert_eq!(
            base.inputs,
            [
                LUMP_3,
                None,
                LUMP_3,
                hoe,
                Some(COMMON_TALISMAN),
                hoe,
                LUMP_3,
                None,
                LUMP_3
            ]
        );
        assert_eq!(
            ender.inputs,
            [
                ENDER,
                None,
                ENDER,
                None,
                Some(FARMER_TALISMAN),
                None,
                ENDER,
                None,
                ENDER
            ]
        );
        assert_eq!(
            (base.output.count, base.research.id, base.research.cost),
            (1, "farmer_talisman", 18)
        );
        assert_eq!(
            (ender.output.count, ender.research.id),
            (1, "ender_talismans")
        );
        for id in [FARMER_TALISMAN.id(), ENDER_FARMER_TALISMAN.id()] {
            let original = lore(id).unwrap();
            let mut components = mod_api::Compound::from([("minecraft:lore".into(), original)]);
            crate::guide::localize_item_components(id, &mut components);
            let Tag::List(lines) = components.get(&"minecraft:lore".into()).unwrap() else {
                panic!("talisman lore must be a list");
            };
            let translated = lines
                .iter()
                .filter_map(crate::network_runtime::paper_text)
                .collect::<Vec<_>>();
            assert!(
                translated
                    .iter()
                    .any(|line| line.contains("收获作物有 20% 的概率"))
            );
            assert!(translated.iter().any(|line| line.contains("获得双倍掉落")));
            assert_eq!(
                translated.iter().any(|line| line.contains("末影灌注")),
                id.starts_with("ENDER_")
            );
        }
    }

    #[test]
    fn caveman_uses_the_miner_recipe_chain_and_its_own_ore_tag() {
        assert!(ore("minecraft:ancient_debris"));
        assert!(!miner_triggers("minecraft:ancient_debris"));
        assert!(!ore("minecraft:gilded_blackstone"));
        assert!(miner_triggers("minecraft:gilded_blackstone"));
        assert!(ore("minecraft:deepslate_diamond_ore"));
        assert!(miner_triggers("minecraft:deepslate_diamond_ore"));
        assert!(roll_succeeds(&CAVEMAN, 50));
        assert!(!roll_succeeds(&CAVEMAN, 51));
        let recipe = |output| CRAFTING.iter().find(|r| r.output.kind == output).unwrap();
        let mineral = recipe(MINER_TALISMAN);
        let cave = recipe(CAVEMAN_TALISMAN);
        assert_eq!(
            mineral.inputs[3..6],
            [
                Some(SYNTHETIC_SAPPHIRE),
                Some(COMMON_TALISMAN),
                Some(SIFTED_ORE)
            ]
        );
        assert_eq!(
            cave.inputs[3..6],
            [
                Some(Kind::Vanilla("minecraft:golden_pickaxe")),
                Some(MINER_TALISMAN),
                Some(ANCIENT_RUNE_EARTH)
            ]
        );
        for spec in [&MINER, &CAVEMAN] {
            let base = recipe(spec.normal);
            let ender = recipe(spec.ender);
            assert_eq!(ender.inputs[4], Some(spec.normal));
            assert_eq!(ender.research.id, ENDER_RESEARCH.id);
            assert_eq!(base.research.id, spec.research.id);
            assert_eq!((base.output.count, ender.output.count), (1, 1));
        }
    }

    #[test]
    fn paper_storage_ignores_wrong_tiers_and_equipment() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([mod_api::Identifier::parse("minecraft:emerald").unwrap()])
            .unwrap();
        let normal = WISE.normal.stack(&host, 1).unwrap();
        let ender = WISE.ender.stack(&host, 1).unwrap();

        assert!(first_matching(&host, [(35, normal.clone())], 36, WISE.normal).is_some());
        assert!(first_matching(&host, [(36, normal.clone())], 36, WISE.normal).is_none());
        assert!(first_matching(&host, [(4, normal)], 27, WISE.ender).is_none());
        assert!(first_matching(&host, [(4, ender.clone())], 36, WISE.normal).is_none());
        assert!(first_matching(&host, [(26, ender)], 27, WISE.ender).is_some());
    }

    #[test]
    fn passive_talismans_use_the_base_items_availability_for_ender_copies() {
        struct ResetAvailability;
        impl Drop for ResetAvailability {
            fn drop(&mut self) {
                crate::availability::configure(&toml::Table::new());
            }
        }
        let _reset = ResetAvailability;
        crate::availability::configure(
            &r#"
                [worlds."fixture:disabled_base".items]
                FIRE_TALISMAN = false
                ENDER_FIRE_TALISMAN = true
                [worlds."fixture:disabled_ender".items]
                FIRE_TALISMAN = true
                ENDER_FIRE_TALISMAN = false
            "#
            .parse()
            .unwrap(),
        );
        let key = format!("slimefun:{}", FIRE.research.id);
        let learned = mod_api::Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([(key.as_str(), Tag::Byte(1))]),
        )]);
        assert!(!can_activate("fixture:disabled_base", &FIRE, &learned));
        assert!(can_activate("fixture:disabled_ender", &FIRE, &learned));
        assert!(!can_activate(
            "fixture:disabled_ender",
            &FIRE,
            &mod_api::Compound::new()
        ));
    }

    #[test]
    fn magician_filters_each_supported_compatible_level_through_config() {
        let talismans = Talismans::default();
        talismans.configure(
            &r#"
                [talismans]
                use-actionbar = false
                [magician-talisman]
                allow-enchantment-books = true
                [magician-talisman.allow-enchantments.minecraft.efficiency.level]
                "2" = false
                "3" = true
            "#
            .parse()
            .unwrap(),
        );
        let rules = talismans.0.lock().unwrap();
        assert!(!rules.actionbar);
        assert!(rules.allow_books);

        let registered = [
            ("minecraft:efficiency", 3),
            ("minecraft:fortune", 3),
            ("minecraft:aqua_affinity", 1),
        ]
        .map(|(id, max_level)| (id.into(), max_level));
        let candidates = possible_magic_enchants(
            registered,
            |id| id != "minecraft:aqua_affinity",
            |id| id != "minecraft:fortune",
            &rules.disabled_entries,
        );
        assert_eq!(
            candidates,
            [
                ("minecraft:efficiency".into(), 1),
                ("minecraft:efficiency".into(), 3)
            ]
        );
    }

    #[test]
    fn recipes_share_the_glass_chain_and_four_ender_lumps_per_upgrade() {
        let inputs = |index: usize| CRAFTING[index].inputs.map(|kind| kind.map_or("", Kind::id));
        assert_eq!(
            inputs(1),
            [
                "MAGIC_LUMP_2",
                "GOLD_DUST",
                "MAGIC_LUMP_2",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "minecraft:glass_pane",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "MAGIC_LUMP_2",
                "FILLED_FLASK_OF_KNOWLEDGE",
                "MAGIC_LUMP_2",
            ]
        );
        assert_eq!(
            inputs(7),
            [
                "ENDER_LUMP_3",
                "",
                "ENDER_LUMP_3",
                "",
                "WISE_TALISMAN",
                "",
                "ENDER_LUMP_3",
                "",
                "ENDER_LUMP_3",
            ]
        );
        assert!(CRAFTING[..8].iter().all(|recipe| recipe.output.count == 1));
        assert_eq!(
            inputs(8),
            [
                "MAGIC_LUMP_3",
                "",
                "MAGIC_LUMP_3",
                "LAVA_CRYSTAL",
                "COMMON_TALISMAN",
                "LAVA_CRYSTAL",
                "MAGIC_LUMP_3",
                "",
                "MAGIC_LUMP_3",
            ]
        );
        assert_eq!(CRAFTING[8].output.count, 4);
        assert_eq!(CRAFTING[9].output.count, 4);
        assert_eq!(CRAFTING[9].inputs[4], Some(FIRE_TALISMAN));
    }

    #[test]
    fn angel_and_traveller_share_the_paper_crafting_chain_and_exact_roll_boundaries() {
        for (spec, side, center, research, cost, successful_rolls) in [
            (
                &ANGEL,
                Kind::Vanilla("minecraft:feather"),
                COMMON_TALISMAN,
                "angel_talisman",
                18,
                76,
            ),
            (
                &TRAVELLER,
                STAFF_ELEMENTAL_WIND,
                ANGEL_TALISMAN,
                "traveller_talisman",
                20,
                61,
            ),
        ] {
            let recipe = CRAFTING
                .iter()
                .find(|r| r.output.kind == spec.normal)
                .unwrap();
            let infused = CRAFTING
                .iter()
                .find(|r| r.output.kind == spec.ender)
                .unwrap();
            assert_eq!((spec.research.id, spec.research.cost), (research, cost));
            assert_eq!(
                recipe.inputs,
                [
                    LUMP_3,
                    None,
                    LUMP_3,
                    Some(side),
                    Some(center),
                    Some(side),
                    LUMP_3,
                    None,
                    LUMP_3
                ]
            );
            assert_eq!(
                infused.inputs,
                [
                    ENDER,
                    None,
                    ENDER,
                    None,
                    Some(spec.normal),
                    None,
                    ENDER,
                    None,
                    ENDER
                ]
            );
            assert_eq!((recipe.output.count, infused.output.count), (1, 1));
            assert!(roll_succeeds(spec, successful_rolls - 1));
            assert!(!roll_succeeds(spec, successful_rolls));
            assert_eq!(
                (0..100).filter(|&value| roll_succeeds(spec, value)).count(),
                successful_rolls as usize
            );
        }
    }
}
