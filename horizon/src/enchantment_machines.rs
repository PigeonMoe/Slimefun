// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{ComponentId, Compound, Host, Identifier, ItemResource, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine, Plan},
    items::{
        CARBONADO, ELECTRIC_MOTOR, Kind, LEAD_INGOT, REINFORCED_PLATE, SYNTHETIC_DIAMOND, item,
        item_id,
    },
    network_recipes::{BIG_CAPACITOR, BLISTERING_INGOT_3, HARDENED_GLASS},
    recipes::{CraftingRecipe, Research, Stack},
};

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

fn enchant(host: &dyn Host, target: &ItemStack, book: &ItemStack) -> Option<(Vec<ItemStack>, u32)> {
    let mut applied = levels(host, target, DIRECT)?;
    let mut transfers = 0;
    for (id, amount) in levels(host, book, STORED)? {
        let amount = level(&amount)?;
        if host.enchantment_supports(&id.to_string().ok()?, target.item())
            && applied.get(&id).and_then(level).unwrap_or(0) < amount
        {
            // AutoEnchanter uses unsafe Bukkit writes after checking the material,
            // so mutually exclusive enchantments and over-level books are allowed.
            applied.insert(id, Tag::Int(amount));
            transfers += 1;
        }
    }
    if transfers == 0 {
        return None;
    }

    let item = ItemResource::of(target)
        .with_component(host, &ComponentId::new(DIRECT)?, Tag::Compound(applied))
        .ok()?
        .to_stack(1)?;
    Some((vec![item, BOOK.stack(host, 1)?], transfers))
}

fn disenchant(host: &dyn Host, target: &ItemStack) -> Option<(Vec<ItemStack>, u32)> {
    // Paper's EnchantedItem flag defaults to false for Staff of Fire.
    if item_id(target).as_deref() == Some("STAFF_FIRE") {
        return None;
    }
    let mut applied = levels(host, target, DIRECT)?;
    let transfers = u32::try_from(applied.len()).ok()?;
    if transfers == 0 {
        return None;
    }
    for value in applied.values_mut() {
        *value = Tag::Int(level(value)?);
    }

    let book = ENCHANTED_BOOK.stack(host, 1)?;
    let mut book_patch = Compound::from([(STORED.into(), Tag::Compound(applied))]);
    let repair = ComponentId::new(REPAIR_COST)?;
    if let Some(cost @ Tag::Int(cost_ticks)) = ItemResource::of(target).value(host, &repair)
        && cost_ticks > 0
    {
        book_patch.insert(REPAIR_COST.into(), cost);
    }
    let mut book = book;
    book.apply_components(&book_patch).ok()?;

    // Vanilla's default, reached by clearing the patch, has no enchantments
    // or anvil penalty. Preserve that representation for ordinary stacks.
    let defaults = ItemStack::new(target.item(), 1)?;
    let direct = ComponentId::new(DIRECT)?;
    let mut clean = ItemResource::of(target).to_stack(1)?;
    for (id, has_nonempty_default) in [
        (
            direct,
            matches!(host.item_default_component(target.item(), DIRECT), Some(Tag::Compound(levels)) if !levels.is_empty()),
        ),
        (
            repair,
            matches!(host.item_default_component(target.item(), REPAIR_COST), Some(Tag::Int(cost)) if cost > 0),
        ),
    ] {
        if has_nonempty_default {
            clean = ItemResource::of(&clean)
                .without_component(host, &id)
                .to_stack(1)?;
        } else {
            clean.copy_component_from(&defaults, id.as_str());
        }
    }
    Some((vec![clean, book], transfers))
}

pub(crate) fn next(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
) -> Option<Plan> {
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
    let (outputs, transfers) = match machine.family {
        Family::AutoEnchanter => enchant(host, target, slots[book]),
        Family::AutoDisenchanter => disenchant(host, target),
        _ => return None,
    }?;
    // MachineRecipe takes seconds and converts them into two Slimefun ticks per second.
    let work_ticks = (seconds.checked_mul(transfers)? / machine.speed).checked_mul(2)?;
    i32::try_from(work_ticks).ok()?;
    Some(Plan {
        consumed: slots
            .into_iter()
            .enumerate()
            .map(|(slot, stack)| (slot, stack.clone(), 1))
            .collect(),
        outputs,
        work_ticks,
    })
}
