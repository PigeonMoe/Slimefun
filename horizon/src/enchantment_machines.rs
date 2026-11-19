// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ComponentId, Compound, Host, Identifier, ItemResource, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine, Plan},
    items::{
        CARBONADO, ELECTRIC_MOTOR, Kind, LEAD_INGOT, REINFORCED_PLATE, SYNTHETIC_DIAMOND, item,
    },
    network_recipes::{BIG_CAPACITOR, BLISTERING_INGOT_3, HARDENED_GLASS},
    recipes::{CraftingRecipe, Research, Stack},
};

#[path = "enchantment_settings.rs"]
mod settings;

use settings::{MachineRules, Rules};
pub(crate) use settings::{configure, default_config};

item!(
    AUTO_ENCHANTER,
    "enchanting_table",
    "Auto Enchanter",
    "dark_purple",
    None,
    None
);
item!(
    AUTO_ENCHANTER_2,
    "enchanting_table",
    "Auto Enchanter - II",
    "dark_purple",
    None,
    None
);
item!(
    AUTO_DISENCHANTER,
    "enchanting_table",
    "Auto Disenchanter",
    "dark_purple",
    None,
    None
);
item!(
    AUTO_DISENCHANTER_2,
    "enchanting_table",
    "Auto Disenchanter - II",
    "dark_purple",
    None,
    None
);
item!(
    WITHER_PROOF_OBSIDIAN,
    "obsidian",
    "Wither-Proof Obsidian",
    "dark_purple",
    None,
    None
);

const ENCHANTING: Research = Research {
    id: "auto_enchanting",
    name: "Automatic Enchanting and Disenchanting",
    cost: 24,
};
const ADVANCED_ENCHANTING: Research = Research {
    id: "medium_tier_auto_enchanting",
    name: "Fast Automatic Enchanting and Disenchanting",
    cost: 48,
};
const WITHER_PROOF: Research = Research {
    id: "wither_proof_obsidian",
    name: "Wither-Proof Obsidian",
    cost: 21,
};
pub(crate) const RESEARCHES: &[Research] = &[ENCHANTING, ADVANCED_ENCHANTING, WITHER_PROOF];

const fn machine(item: Kind, title: &'static str, family: Family, advanced: bool) -> Machine {
    let (capacity, cost, speed, research) = if advanced {
        (1024, 24, 3, &ADVANCED_ENCHANTING)
    } else {
        (128, 9, 1, &ENCHANTING)
    };
    Machine {
        item,
        title,
        family,
        capacity,
        cost,
        speed,
        research,
    }
}

pub(crate) const MACHINES: &[Machine] = &[
    machine(
        AUTO_ENCHANTER,
        "Auto Enchanter",
        Family::AutoEnchanter,
        false,
    ),
    machine(
        AUTO_ENCHANTER_2,
        "Auto Enchanter - II",
        Family::AutoEnchanter,
        true,
    ),
    machine(
        AUTO_DISENCHANTER,
        "Auto Disenchanter",
        Family::AutoDisenchanter,
        false,
    ),
    machine(
        AUTO_DISENCHANTER_2,
        "Auto Disenchanter - II",
        Family::AutoDisenchanter,
        true,
    ),
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    fn line(parts: &[(&str, &str)]) -> Tag {
        Tag::compound([
            ("text", Tag::String("".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(
                    parts
                        .iter()
                        .map(|&(text, color)| crate::items::text(text, color, false))
                        .collect(),
                ),
            ),
        ])
    }

    let empty = Tag::String("".into());
    let lore = if id == WITHER_PROOF_OBSIDIAN.id() {
        vec![
            empty,
            line(&[("Withstands Explosions", "white")]),
            line(&[("Withstands Wither Bosses", "white")]),
        ]
    } else {
        let Some(machine) = MACHINES.iter().find(|machine| machine.item.id() == id) else {
            return;
        };
        let tier = if machine.speed == 1 {
            ("Medium Machine", "green")
        } else {
            let name = machine.title.strip_suffix(" - II").unwrap_or(machine.title);
            components.insert(
                "minecraft:custom_name".into(),
                line(&[(name, "dark_purple"), (" - ", "gray"), ("II", "yellow")]),
            );
            ("End-Game Machine", "dark_red")
        };
        vec![
            empty,
            line(&[tier]),
            line(&[
                ("⇨ ", "dark_gray"),
                ("⚡ ", "aqua"),
                ("Speed: ", "gray"),
                (&format!("{}.0x", machine.speed), "aqua"),
            ]),
            line(&[
                ("⇨ ", "dark_gray"),
                ("⚡ ", "yellow"),
                (&format!("{} J/s", machine.cost * 2), "gray"),
            ]),
        ]
    };
    components.insert("minecraft:lore".into(), Tag::List(lore));
}

const TABLE: Kind = Kind::Vanilla("minecraft:enchanting_table");
const ANVIL: Kind = Kind::Vanilla("minecraft:anvil");
const OBSIDIAN: Kind = Kind::Vanilla("minecraft:obsidian");
const REDSTONE: Kind = Kind::Vanilla("minecraft:redstone");
const BOOK: Kind = Kind::Vanilla("minecraft:book");
const ENCHANTED_BOOK: Kind = Kind::Vanilla("minecraft:enchanted_book");

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(LEAD_INGOT),
            Some(OBSIDIAN),
            Some(LEAD_INGOT),
            Some(OBSIDIAN),
            Some(HARDENED_GLASS),
            Some(OBSIDIAN),
            Some(LEAD_INGOT),
            Some(OBSIDIAN),
            Some(LEAD_INGOT),
        ],
        output: Stack {
            kind: WITHER_PROOF_OBSIDIAN,
            count: 4,
        },
        research: &WITHER_PROOF,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(TABLE),
            None,
            Some(CARBONADO),
            Some(ELECTRIC_MOTOR),
            Some(CARBONADO),
            Some(WITHER_PROOF_OBSIDIAN),
            Some(WITHER_PROOF_OBSIDIAN),
            Some(WITHER_PROOF_OBSIDIAN),
        ],
        output: Stack {
            kind: AUTO_ENCHANTER,
            count: 1,
        },
        research: &ENCHANTING,
    },
    CraftingRecipe {
        inputs: [
            Some(REINFORCED_PLATE),
            Some(BIG_CAPACITOR),
            Some(REINFORCED_PLATE),
            Some(BLISTERING_INGOT_3),
            Some(AUTO_ENCHANTER),
            Some(BLISTERING_INGOT_3),
            Some(WITHER_PROOF_OBSIDIAN),
            Some(SYNTHETIC_DIAMOND),
            Some(WITHER_PROOF_OBSIDIAN),
        ],
        output: Stack {
            kind: AUTO_ENCHANTER_2,
            count: 1,
        },
        research: &ADVANCED_ENCHANTING,
    },
    CraftingRecipe {
        inputs: [
            Some(REDSTONE),
            Some(ANVIL),
            Some(REDSTONE),
            Some(CARBONADO),
            Some(AUTO_ENCHANTER),
            Some(CARBONADO),
            Some(WITHER_PROOF_OBSIDIAN),
            Some(WITHER_PROOF_OBSIDIAN),
            Some(WITHER_PROOF_OBSIDIAN),
        ],
        output: Stack {
            kind: AUTO_DISENCHANTER,
            count: 1,
        },
        research: &ENCHANTING,
    },
    CraftingRecipe {
        inputs: [
            Some(REINFORCED_PLATE),
            Some(ANVIL),
            Some(REINFORCED_PLATE),
            Some(BLISTERING_INGOT_3),
            Some(AUTO_DISENCHANTER),
            Some(BLISTERING_INGOT_3),
            Some(WITHER_PROOF_OBSIDIAN),
            Some(BIG_CAPACITOR),
            Some(WITHER_PROOF_OBSIDIAN),
        ],
        output: Stack {
            kind: AUTO_DISENCHANTER_2,
            count: 1,
        },
        research: &ADVANCED_ENCHANTING,
    },
];

const DIRECT: &str = "minecraft:enchantments";
pub(crate) const STORED: &str = "minecraft:stored_enchantments";
const REPAIR_COST: &str = "minecraft:repair_cost";

fn component(host: &dyn Host, stack: &ItemStack, id: &str) -> Option<Tag> {
    ItemResource::of(stack).value(host, &ComponentId::new(id)?)
}

pub(crate) fn levels(host: &dyn Host, stack: &ItemStack, component_id: &str) -> Option<Compound> {
    match component(host, stack, component_id) {
        Some(Tag::Compound(levels)) => Some(levels),
        None => Some(Compound::new()),
        _ => None,
    }
}

pub(crate) fn level(tag: &Tag) -> Option<i32> {
    let number = match tag {
        Tag::Byte(value) => i32::from(*value),
        Tag::Short(value) => i32::from(*value),
        Tag::Int(value) => *value,
        _ => return None,
    };
    (1..=255).contains(&number).then_some(number)
}

fn book_slot(host: &dyn Host, inputs: [&ItemStack; 2], book_kind: Kind) -> Option<usize> {
    // Paper checks the actual material even if the book has custom metadata.
    let book = host.item_key(&Identifier::parse(book_kind.material()).ok()?)?;
    match inputs.map(|stack| stack.item() == book) {
        [true, false] => Some(0),
        [false, true] => Some(1),
        _ => None,
    }
}

type Attempt<T> = Option<Result<T, i32>>;

fn select_levels(
    incoming: Compound,
    rules: &MachineRules,
    viewed: bool,
    applies: impl Fn(&str) -> bool,
) -> Attempt<Compound> {
    let mut selected = Compound::new();
    for (id, raw) in incoming {
        if !applies(&id.to_string().ok()?) {
            continue;
        }
        let amount = level(&raw)?;
        if let Some(limit) = rules.level_limit().filter(|limit| amount > *limit) {
            if viewed {
                return Some(Err(limit));
            }
        } else {
            selected.insert(id, Tag::Int(amount));
        }
    }
    Some(Ok(selected))
}

fn enchant(
    host: &dyn Host,
    target: &ItemStack,
    book: &ItemStack,
    rules: &MachineRules,
    viewed: bool,
) -> Attempt<(Vec<ItemStack>, u32)> {
    let mut applied = levels(host, target, DIRECT)?;
    let incoming = levels(host, book, STORED)?;
    // Paper counts every enchantment already present on both the item and book,
    // including one that will also be replaced by the new enchantment.
    let overlap = applied
        .keys()
        .filter(|id| incoming.contains_key(*id))
        .count();
    let mut selected = match select_levels(incoming, rules, viewed, |id| {
        host.enchantment_supports(id, target.item())
    })? {
        Ok(selected) => selected,
        Err(limit) => return Some(Err(limit)),
    };
    if !rules.override_levels {
        selected.retain(|id, amount| {
            applied.get(id).and_then(level).unwrap_or(0) < level(amount).unwrap_or(0)
        });
    }
    let transfers = u32::try_from(selected.len()).ok()?;
    if transfers == 0
        || rules
            .count_limit()
            .is_some_and(|limit| selected.len() + overlap > limit)
    {
        return None;
    }
    // Paper uses unsafe Bukkit writes after the material and configured limits
    // check, including for mutually exclusive or over-vanilla-level enchantments.
    applied.extend(selected);
    let item = ItemResource::of(target)
        .with_component(host, &ComponentId::new(DIRECT)?, Tag::Compound(applied))
        .ok()?
        .to_stack(1)?;
    Some(Ok((vec![item, BOOK.stack(host, 1)?], transfers)))
}

fn disenchant(
    host: &dyn Host,
    target: &ItemStack,
    rules: &MachineRules,
    viewed: bool,
) -> Attempt<(Vec<ItemStack>, u32)> {
    let mut applied = levels(host, target, DIRECT)?;
    let selected = match select_levels(applied.clone(), rules, viewed, |_| true)? {
        Ok(selected) => selected,
        Err(limit) => return Some(Err(limit)),
    };
    let transfers = u32::try_from(selected.len()).ok()?;
    if transfers == 0
        || rules
            .count_limit()
            .is_some_and(|limit| selected.len() > limit)
    {
        return None;
    }
    applied.retain(|id, _| !selected.contains_key(id));

    let mut book = ENCHANTED_BOOK.stack(host, 1)?;
    let mut book_patch = Compound::from([(STORED.into(), Tag::Compound(selected))]);
    let repair = ComponentId::new(REPAIR_COST)?;
    if let Some(cost @ Tag::Int(cost_ticks)) = ItemResource::of(target).value(host, &repair)
        && cost_ticks > 0
    {
        book_patch.insert(REPAIR_COST.into(), cost);
    }
    book.apply_components(&book_patch).ok()?;

    let defaults = ItemStack::new(target.item(), 1)?;
    let mut clean = ItemResource::of(target).to_stack(1)?;
    if applied.is_empty() {
        clear_component(host, &mut clean, &defaults, DIRECT)?;
    } else {
        clean = ItemResource::of(&clean)
            .with_component(host, &ComponentId::new(DIRECT)?, Tag::Compound(applied))
            .ok()?
            .to_stack(1)?;
    }
    clear_component(host, &mut clean, &defaults, REPAIR_COST)?;
    Some(Ok((vec![clean, book], transfers)))
}

fn clear_component(
    host: &dyn Host,
    stack: &mut ItemStack,
    defaults: &ItemStack,
    id: &str,
) -> Option<()> {
    let has_nonempty_default = match host.item_default_component(stack.item(), id) {
        Some(Tag::Compound(levels)) => !levels.is_empty(),
        Some(Tag::Int(cost)) => cost > 0,
        _ => false,
    };
    if has_nonempty_default {
        *stack = ItemResource::of(stack)
            .without_component(host, &ComponentId::new(id)?)
            .to_stack(1)?;
    } else {
        stack.copy_component_from(defaults, id);
    }
    Some(())
}

fn evaluate(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
    config: &Rules,
    viewed: bool,
) -> Attempt<Plan> {
    let rules = config.machine(machine)?;
    let (book_kind, seconds) = match machine.family {
        Family::AutoEnchanter => (ENCHANTED_BOOK, 75u32),
        Family::AutoDisenchanter => (BOOK, 90u32),
        _ => return None,
    };
    let [Some(first), Some(second)] = inputs else {
        return None;
    };
    let slots = [first, second];
    let book = book_slot(host, slots, book_kind)?;
    let target = slots[1 - book];
    if !config.allows_item(target, machine.family) || !rules.accepts_lore(host, target) {
        return None;
    }
    let attempt = match machine.family {
        Family::AutoEnchanter => enchant(host, target, slots[book], rules, viewed),
        Family::AutoDisenchanter => disenchant(host, target, rules, viewed),
        _ => return None,
    }?;
    let (outputs, transfers) = match attempt {
        Ok(outputs) => outputs,
        Err(limit) => return Some(Err(limit)),
    };
    // MachineRecipe takes seconds and converts them into two Slimefun ticks per second.
    let work_ticks = (seconds.checked_mul(transfers)? / machine.speed).checked_mul(2)?;
    i32::try_from(work_ticks).ok()?;
    Some(Ok(Plan {
        consumed: slots
            .into_iter()
            .enumerate()
            .map(|(slot, stack)| (slot, stack.clone(), 1))
            .collect(),
        outputs,
        work_ticks,
    }))
}

pub(crate) fn next(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
    viewed: bool,
) -> Option<Plan> {
    settings::with_rules(|config| evaluate(host, machine, inputs, config, viewed)?.ok())
}

pub(crate) fn warning(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
) -> Option<i32> {
    settings::with_rules(|config| {
        config.machine(machine)?.level_limit()?;
        evaluate(host, machine, inputs, config, true)?.err()
    })
}

pub(crate) fn warning_item(host: &dyn Host, limit: i32) -> Option<ItemStack> {
    let mut barrier = Kind::Vanilla("minecraft:barrier").stack(host, 1)?;
    barrier
        .apply_components(&Compound::from([
            (
                "minecraft:custom_name".into(),
                crate::items::text(" ", "white", false),
            ),
            (
                "minecraft:lore".into(),
                Tag::List(vec![crate::items::text(
                    &format!("Cannot (dis)enchant an enchantment level higher than {limit}!"),
                    "red",
                    false,
                )]),
            ),
        ]))
        .ok()?;
    Some(barrier)
}

#[cfg(test)]
mod tests {
    use mod_api::testing::FakeHost;

    use super::*;

    const SWORD: &str = "minecraft:diamond_sword";

    fn host() -> FakeHost {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    SWORD,
                    "minecraft:book",
                    "minecraft:enchanted_book",
                    "minecraft:barrier",
                ]
                .map(|item| Identifier::parse(item).unwrap()),
            )
            .unwrap();
        for id in [
            "minecraft:sharpness",
            "minecraft:smite",
            "minecraft:vanishing_curse",
        ] {
            host.registries.allow_enchantment(id, SWORD);
        }
        host
    }

    fn item(
        host: &dyn Host,
        kind: Kind,
        component: &str,
        enchantments: &[(&str, i32)],
    ) -> ItemStack {
        let mut item = kind.stack(host, 1).unwrap();
        let values: Compound = enchantments
            .iter()
            .map(|&(id, amount)| (id.into(), Tag::Int(amount)))
            .collect();
        item.set_components(Compound::from([(component.into(), Tag::Compound(values))]))
            .unwrap();
        item
    }

    fn machine(id: &str) -> &'static Machine {
        MACHINES
            .iter()
            .find(|machine| machine.item.id() == id)
            .unwrap()
    }

    fn config(text: &str) -> Rules {
        Rules::read(&text.parse().unwrap(), &Rules::default())
    }

    #[test]
    fn auto_enchanter_tiers_match_paper_equal_lower_and_counting_overlap() {
        let host = host();
        let sword = item(
            &host,
            Kind::Vanilla(SWORD),
            DIRECT,
            &[("minecraft:sharpness", 6)],
        );
        let book = item(
            &host,
            ENCHANTED_BOOK,
            STORED,
            &[("minecraft:sharpness", 4), ("minecraft:smite", 2)],
        );
        let inputs = [Some(book), Some(sword.clone())];
        let one = machine("AUTO_ENCHANTER");
        let two = machine("AUTO_ENCHANTER_2");
        let rules = config(
            r#"
[enchantment-machines.AUTO_ENCHANTER]
override-existing-enchants-lvl = true
[enchantment-machines.AUTO_ENCHANTER_2]
use-max-enchants = true
max-enchants = 2
"#,
        );
        let selected = evaluate(&host, one, &inputs, &rules, false)
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(selected.work_ticks, 300);
        assert_eq!(
            levels(&host, &selected.outputs[0], DIRECT)
                .unwrap()
                .get(&"minecraft:sharpness".into()),
            Some(&Tag::Int(4))
        );
        let selected = evaluate(&host, two, &inputs, &rules, false)
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(selected.work_ticks, 50);
        assert_eq!(
            levels(&host, &selected.outputs[0], DIRECT)
                .unwrap()
                .get(&"minecraft:sharpness".into()),
            Some(&Tag::Int(6))
        );
        let count = config(
            r#"
[enchantment-machines.AUTO_ENCHANTER]
override-existing-enchants-lvl = true
use-max-enchants = true
max-enchants = 2
"#,
        );
        assert!(evaluate(&host, one, &inputs, &count, false).is_none());
        let equal = [
            Some(sword),
            Some(item(
                &host,
                ENCHANTED_BOOK,
                STORED,
                &[("minecraft:sharpness", 6)],
            )),
        ];
        assert!(evaluate(&host, one, &equal, &Rules::default(), false).is_none());
        assert_eq!(
            evaluate(&host, one, &equal, &rules, false)
                .unwrap()
                .ok()
                .unwrap()
                .work_ticks,
            150
        );
    }

    #[test]
    fn both_machine_families_warn_visible_players_and_skip_only_high_levels_when_unattended() {
        let host = host();
        let rules = config(
            r#"
[enchantment-machines.AUTO_ENCHANTER]
use-enchant-level-limit = true
enchant-level-limit = 5
[enchantment-machines.AUTO_DISENCHANTER_2]
use-enchant-level-limit = true
enchant-level-limit = 5
use-max-enchants = true
max-enchants = 1
"#,
        );
        let mixed = [("minecraft:sharpness", 7), ("minecraft:smite", 3)];
        let enchanting = [
            Kind::Vanilla(SWORD).stack(&host, 1),
            Some(item(&host, ENCHANTED_BOOK, STORED, &mixed)),
        ];
        let mut sword = item(&host, Kind::Vanilla(SWORD), DIRECT, &mixed);
        sword
            .apply_components(&Compound::from([(REPAIR_COST.into(), Tag::Int(17))]))
            .unwrap();
        let disenchanting = [BOOK.stack(&host, 1), Some(sword)];
        let enchanter = machine("AUTO_ENCHANTER");
        let disenchanter = machine("AUTO_DISENCHANTER_2");
        for (machine, inputs) in [(enchanter, &enchanting), (disenchanter, &disenchanting)] {
            assert_eq!(
                evaluate(&host, machine, inputs, &rules, true)
                    .unwrap()
                    .err(),
                Some(5)
            );
        }
        let enchants = |stack, component| levels(&host, stack, component).unwrap();
        let applied = evaluate(&host, enchanter, &enchanting, &rules, false)
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(
            enchants(&applied.outputs[0], DIRECT),
            Compound::from([("minecraft:smite".into(), Tag::Int(3))])
        );
        let removed = evaluate(&host, disenchanter, &disenchanting, &rules, false)
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(removed.work_ticks, 60);
        assert_eq!(
            enchants(&removed.outputs[0], DIRECT).get(&"minecraft:sharpness".into()),
            Some(&Tag::Int(7))
        );
        assert_eq!(
            enchants(&removed.outputs[1], STORED).get(&"minecraft:smite".into()),
            Some(&Tag::Int(3))
        );
        assert_eq!(
            component(&host, &removed.outputs[1], REPAIR_COST),
            Some(Tag::Int(17))
        );
        assert_eq!(component(&host, &removed.outputs[0], REPAIR_COST), None);
        assert_eq!(
            warning_item(&host, 5).unwrap().item().id(&host),
            "minecraft:barrier"
        );
    }

    #[test]
    fn a_paper_reload_keeps_last_valid_integers_when_values_are_out_of_range() {
        let initial = config(
            r#"
[enchantment-machines.AUTO_ENCHANTER_2]
enchant-level-limit = 8
max-enchants = 0
"#,
        );
        let source = r#"
[enchantment-machines.AUTO_ENCHANTER_2]
use-enchant-level-limit = true
use-max-enchants = true
enchant-level-limit = 32768
max-enchants = -1
"#
        .parse()
        .unwrap();
        let machine = machine("AUTO_ENCHANTER_2");
        let loaded = Rules::read(&source, &initial);
        let rules = loaded.machine(machine).unwrap();
        assert_eq!(
            (rules.level_limit(), rules.count_limit()),
            (Some(8), Some(0))
        );
        let fresh = Rules::read(&source, &Rules::default());
        let defaults = fresh.machine(machine).unwrap();
        assert_eq!(
            (defaults.level_limit(), defaults.count_limit()),
            (Some(10), Some(10))
        );
    }

    #[test]
    fn item_allow_flags_and_literal_lore_filters_are_scoped_and_color_sensitive() {
        let host = host();
        let mut sword = item(
            &host,
            Kind::Vanilla(SWORD),
            DIRECT,
            &[("minecraft:sharpness", 3)],
        );
        sword
            .apply_components(&Compound::from([
                (
                    "minecraft:custom_data".into(),
                    Tag::compound([(
                        "PublicBukkitValues",
                        Tag::compound([(crate::items::ID_KEY, Tag::String("STAFF_FIRE".into()))]),
                    )]),
                ),
                (
                    "minecraft:lore".into(),
                    Tag::List(vec![Tag::compound([
                        ("text", Tag::String("Golden ".into())),
                        ("color", Tag::String("gold".into())),
                        (
                            "extra",
                            Tag::List(vec![Tag::compound([
                                ("text", Tag::String("Blade".into())),
                                ("color", Tag::String("red".into())),
                            ])]),
                        ),
                    ])]),
                ),
            ]))
            .unwrap();

        let default = config(&default_config());
        assert!(!default.allows_item(&sword, Family::AutoDisenchanter));
        let rules = config(
            r#"
[item-settings.STAFF_FIRE]
allow-disenchanting = true
[enchantment-machines.AUTO_DISENCHANTER_2]
use-ignored-lores = true
ignored-lores = ["&6Golden &cBlade"]
"#,
        );
        let inputs = [Some(sword.clone()), BOOK.stack(&host, 1)];
        assert!(evaluate(&host, machine("AUTO_DISENCHANTER"), &inputs, &rules, false).is_some());
        assert!(
            evaluate(
                &host,
                machine("AUTO_DISENCHANTER_2"),
                &inputs,
                &rules,
                false
            )
            .is_none()
        );
        let rules = config(
            r#"
[item-settings.STAFF_FIRE]
allow-disenchanting = true
[enchantment-machines.AUTO_DISENCHANTER_2]
use-ignored-lores = true
ignored-lores = ["&6Golden &bBlade", "Golden Blade"]
"#,
        );
        assert!(
            evaluate(
                &host,
                machine("AUTO_DISENCHANTER_2"),
                &inputs,
                &rules,
                false
            )
            .is_some()
        );
    }
}
