// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeSet, sync::Mutex};

use mod_api::{
    Host, HostSide, InteractionResult, ItemPart, ItemStack, LevelRef, PlayerRef, Registrar,
    ServerHost, Tag, Text, UseOnEvent,
};

use crate::{
    factory::experience_collector::FILLED_FLASK_OF_KNOWLEDGE,
    items::{ENDER_LUMP_3, GOLD_8K, GOLD_DUST, Kind, MAGIC_LUMP_2, MAGIC_LUMP_3, item, text},
    magic_eye::MAGIC_EYE_OF_ENDER,
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

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

const COMMON_RESEARCH: Research = Research {
    id: "common_talisman",
    name: "Common Talisman",
    cost: 14,
};
const GLASS_RESEARCH: Research = Research {
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
    &GLASS_RESEARCH,
    &MAGICIAN_RESEARCH,
    &WIZARD_RESEARCH,
    &WISE_RESEARCH,
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
    inputs: [Option<Kind>; 9],
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack {
            kind: output,
            count: 1,
        },
        research,
    }
}

const fn infused(output: Kind, base: Kind) -> CraftingRecipe {
    recipe(
        output,
        &ENDER_RESEARCH,
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
    infused(ENDER_MAGICIAN_TALISMAN, MAGICIAN_TALISMAN),
    infused(ENDER_WIZARD_TALISMAN, WIZARD_TALISMAN),
    infused(ENDER_WISE_TALISMAN, WISE_TALISMAN),
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

struct Talisman {
    normal: Kind,
    ender: Kind,
    research: &'static Research,
    chance: u32,
    message: &'static str,
    lore: &'static [&'static str],
}

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let blank = || Tag::String("".into());
    if id == MAGICAL_GLASS.id() {
        return Some(Tag::List(vec![
            blank(),
            text("Used for various Magical Gadgets", "green", true),
        ]));
    }
    let talisman = [&MAGICIAN, &WIZARD, &WISE]
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

fn present(host: &dyn Host, player: PlayerRef, spec: &Talisman) -> bool {
    // Bukkit checks getStorageContents. Neither offhand nor armor nor nested
    // containers can activate a normal talisman.
    let main = player.inventory_slots(host).unwrap_or_default();
    let carried = contains(host, &main, 36, spec.normal) || {
        let ender = player.ender_chest(host).unwrap_or_default();
        contains(host, &ender, 27, spec.ender)
    };
    carried
        && player
            .persistent_data(host)
            .is_some_and(|data| research::unlocked(&data, spec.research))
}

fn contains(host: &dyn Host, slots: &[(u32, ItemStack)], size: u32, expected: Kind) -> bool {
    slots
        .iter()
        .any(|(slot, stack)| *slot < size && expected.matches(host, stack))
}

fn activated(
    host: &dyn Host,
    player: PlayerRef,
    level: LevelRef,
    spec: &Talisman,
    rolls: &Rolls,
) -> bool {
    present(host, player, spec) && chance(host, level, spec, rolls)
}

fn chance(host: &dyn Host, level: LevelRef, spec: &Talisman, rolls: &Rolls) -> bool {
    // Paper rejects when nextInt(100) > chance, so the boundary succeeds.
    rolls
        .next(host, level, 100)
        .is_some_and(|value| value <= spec.chance)
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
    fn paper_storage_ignores_wrong_tiers_and_equipment() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([mod_api::Identifier::parse("minecraft:emerald").unwrap()])
            .unwrap();
        let normal = WISE.normal.stack(&host, 1).unwrap();
        let ender = WISE.ender.stack(&host, 1).unwrap();

        assert!(contains(&host, &[(35, normal.clone())], 36, WISE.normal));
        assert!(!contains(&host, &[(36, normal.clone())], 36, WISE.normal));
        assert!(!contains(&host, &[(4, normal)], 27, WISE.ender));
        assert!(!contains(&host, &[(4, ender.clone())], 36, WISE.normal));
        assert!(contains(&host, &[(26, ender)], 27, WISE.ender));
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
        assert!(CRAFTING.iter().all(|recipe| recipe.output.count == 1));
    }
}
