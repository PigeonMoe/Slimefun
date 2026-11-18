// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine, Plan},
    items::{
        ALUMINUM_DUST, ELECTRIC_MOTOR, HEATING_COIL, HeadTexture, Kind, REINFORCED_ALLOY_INGOT,
        STEEL_PLATE, item, text,
    },
    recipes::{CraftingRecipe, Research, Stack},
};

item!(AUTO_ANVIL, "iron_block", "Auto Anvil", "gray", None, None);
item!(
    AUTO_ANVIL_2,
    "iron_block",
    "Auto Anvil Mk.II",
    "gray",
    None,
    None
);
item!(
    DUCT_TAPE,
    "player_head",
    "Duct Tape",
    "dark_gray",
    None,
    Some(HeadTexture {
        profile: [1438171070, -750505663, -1845435093, 737968155],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYjJmYWFjZWFiNjM4NGZmZjVlZDI0YmI0NGE0YWYyZjU4NGViMTM4MjcyOWVjZDkzYTUzNjlhY2ZkNjY1NCJ9fX0=",
    })
);

const REPAIRING: Research = Research {
    id: "auto_anvil",
    name: "Automatic Anvil",
    cost: 34,
};
const DUCT_TAPE_RESEARCH: Research = Research {
    id: "duct_tape",
    name: "Duct Tape",
    cost: 14,
};
pub(crate) const RESEARCHES: &[Research] = &[REPAIRING, DUCT_TAPE_RESEARCH];

pub(crate) const MACHINES: &[Machine] = &[
    Machine {
        item: AUTO_ANVIL,
        title: "Auto Anvil",
        family: Family::AutoAnvil,
        capacity: 128,
        cost: 12,
        speed: 1,
        research: &REPAIRING,
    },
    Machine {
        item: AUTO_ANVIL_2,
        title: "Auto Anvil Mk.II",
        family: Family::AutoAnvil,
        capacity: 256,
        cost: 16,
        speed: 1,
        research: &REPAIRING,
    },
];

const ANVIL: Kind = Kind::Vanilla("minecraft:anvil");
const IRON_BLOCK: Kind = Kind::Vanilla("minecraft:iron_block");
const SLIME: Kind = Kind::Vanilla("minecraft:slime_ball");
const WHITE_WOOL: Kind = Kind::Vanilla("minecraft:white_wool");
const PAPER: Kind = Kind::Vanilla("minecraft:paper");

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(ALUMINUM_DUST),
            Some(ALUMINUM_DUST),
            Some(ALUMINUM_DUST),
            Some(SLIME),
            Some(WHITE_WOOL),
            Some(SLIME),
            Some(PAPER),
            Some(PAPER),
            Some(PAPER),
        ],
        output: Stack {
            kind: DUCT_TAPE,
            count: 2,
        },
        research: &DUCT_TAPE_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(ANVIL),
            None,
            Some(REINFORCED_ALLOY_INGOT),
            Some(ELECTRIC_MOTOR),
            Some(REINFORCED_ALLOY_INGOT),
            Some(IRON_BLOCK),
            Some(IRON_BLOCK),
            Some(IRON_BLOCK),
        ],
        output: Stack {
            kind: AUTO_ANVIL,
            count: 1,
        },
        research: &REPAIRING,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(AUTO_ANVIL),
            None,
            Some(STEEL_PLATE),
            Some(HEATING_COIL),
            Some(STEEL_PLATE),
            Some(IRON_BLOCK),
            Some(IRON_BLOCK),
            Some(IRON_BLOCK),
        ],
        output: Stack {
            kind: AUTO_ANVIL_2,
            count: 1,
        },
        research: &REPAIRING,
    },
];

pub(crate) fn lore(id: &str) -> Option<Tag> {
    let empty = Tag::String("".into());
    if id == DUCT_TAPE.id() {
        return Some(Tag::List(vec![
            empty,
            text("You can repair Items using this", "white", false),
            text("in an Auto-Anvil", "white", false),
        ]));
    }
    let machine = MACHINES.iter().find(|machine| machine.item.id() == id)?;
    let (tier, color, percent) = if id == AUTO_ANVIL.id() {
        ("Advanced Machine", "gold", 10)
    } else {
        ("End-Game Machine", "dark_red", 25)
    };
    let power = Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        (
            "extra",
            Tag::List(vec![
                text("⇨ ", "dark_gray", false),
                text("⚡ ", "yellow", false),
                text(&format!("{} J/s", machine.cost * 2), "gray", false),
            ]),
        ),
    ]);
    let factor = Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        (
            "extra",
            Tag::List(vec![
                text("⇨ ", "dark_gray", false),
                text(&format!("Repair Factor: {percent}%"), "gray", false),
            ]),
        ),
    ]);
    Some(Tag::List(vec![
        empty,
        text(tier, color, false),
        factor,
        power,
    ]))
}

pub(crate) fn next(
    host: &dyn Host,
    machine: &Machine,
    inputs: &[Option<ItemStack>],
) -> Option<Plan> {
    let [Some(first), Some(second)] = inputs else {
        return None;
    };
    let items = [first, second];
    let divisor = match machine.item {
        AUTO_ANVIL => 10,
        AUTO_ANVIL_2 => 4,
        _ => return None,
    };

    for (slot, item) in items.into_iter().enumerate() {
        let Some(damage) = item.damage_component().filter(|&damage| damage != 0) else {
            continue;
        };
        // Bukkit's Material.getMaxDurability uses the material's prototype.
        let Some(maximum) = host.max_damage(item.item()).filter(|&value| value != 0) else {
            continue;
        };
        let tape = items[1 - slot];
        if !DUCT_TAPE.matches(host, tape) {
            return None;
        }
        let mut repaired = item.clone();
        // The repair consumes one tool. Preserve that amount even on imported
        // stacks carrying unusual nonvanilla count overrides.
        repaired.set_count(1).ok()?;
        let damage = damage.saturating_sub(maximum / divisor);
        if damage == 0 {
            let plain = ItemStack::new(item.item(), 1)?;
            repaired.copy_component_from(&plain, "minecraft:damage");
        } else {
            repaired.set_damage(damage);
        }
        return Some(Plan {
            consumed: vec![(slot, item.clone(), 1), (1 - slot, tape.clone(), 1)],
            outputs: vec![repaired],
            work_ticks: 60,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Compound, Identifier, testing::FakeHost};

    #[test]
    fn repair_uses_paper_material_durability_and_keeps_item_components() {
        let mut host = FakeHost::new();
        let materials = [
            "minecraft:diamond_pickaxe",
            "minecraft:shears",
            "minecraft:player_head",
        ];
        host.registries
            .resolve_items(materials.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        host.registries
            .item_component(materials[0], "minecraft:max_damage", Tag::Int(1561))
            .item_component(materials[1], "minecraft:max_damage", Tag::Int(238));
        let tape = DUCT_TAPE.stack(&host, 16).unwrap();
        let components = Compound::from([
            ("minecraft:repair_cost".into(), Tag::Int(17)),
            (
                "minecraft:unbreakable".into(),
                Tag::Compound(Compound::new()),
            ),
            (
                "minecraft:enchantments".into(),
                Tag::compound([("minecraft:mending", Tag::Int(1))]),
            ),
        ]);
        for (material, repairs) in [
            (materials[0], [Some(45), None]),
            (materials[1], [Some(178), Some(142)]),
        ] {
            let tool = Kind::Vanilla(material).stack(&host, 1).unwrap();
            let mut damaged = tool.clone();
            damaged.set_damage(201);
            damaged.apply_components(&components).unwrap();
            for (machine, damage) in MACHINES.iter().zip(repairs) {
                let inputs = [Some(tape.clone()), Some(damaged.clone())];
                let plan = next(&host, machine, &inputs).unwrap();
                assert_eq!(plan.work_ticks, 60);
                assert_eq!(
                    plan.consumed,
                    [(1, damaged.clone(), 1), (0, tape.clone(), 1)]
                );
                assert_eq!(plan.outputs[0].damage_component(), damage);
                assert_eq!(plan.outputs[0].components().unwrap().as_nbt(), &components);
            }
            assert!(next(&host, &MACHINES[0], &[Some(tool), Some(tape.clone())]).is_none());
        }
        let vanilla_head = Kind::Vanilla("minecraft:player_head")
            .stack(&host, 1)
            .unwrap();
        assert!(!DUCT_TAPE.matches(&host, &vanilla_head));
    }
}
