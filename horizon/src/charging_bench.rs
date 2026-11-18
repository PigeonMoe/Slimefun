// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Host, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine},
    items::{BATTERY, ELECTRO_MAGNET, Kind, SMALL_CAPACITOR, item, item_id},
    recipes::{CraftingRecipe, Research, Stack},
};

const CUSTOM_DATA: &str = "minecraft:custom_data";
const PUBLIC_VALUES: &str = "PublicBukkitValues";
const CHARGE_KEY: &str = "slimefun:item_charge";
const LORE: &str = "minecraft:lore";
const CHARGE_PREFIX: &str = "⇨ ⚡ ";

const CAPACITY: u32 = 128;
pub(crate) const COST: u32 = 10;

item!(
    CHARGING_BENCH,
    "crafting_table",
    "Charging Bench",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "charging_bench",
    name: "Charging Bench",
    cost: 8,
};

pub(crate) const MACHINE: Machine = Machine {
    item: CHARGING_BENCH,
    title: "Charging Bench",
    family: Family::ChargingBench,
    capacity: CAPACITY,
    cost: COST,
    speed: 1,
    research: &RESEARCH,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(ELECTRO_MAGNET),
        None,
        Some(BATTERY),
        Some(Kind::Vanilla("minecraft:crafting_table")),
        Some(BATTERY),
        None,
        Some(SMALL_CAPACITOR),
        None,
    ],
    output: Stack {
        kind: CHARGING_BENCH,
        count: 1,
    },
    research: &RESEARCH,
};

fn rechargeable(id: &str) -> Option<(&'static str, f32)> {
    Some(match id {
        "DURALUMIN_JETPACK" => ("minecraft:leather_chestplate", 20.0),
        "SOLDER_JETPACK" => ("minecraft:leather_chestplate", 30.0),
        "BILLON_JETPACK" => ("minecraft:leather_chestplate", 45.0),
        "STEEL_JETPACK" => ("minecraft:leather_chestplate", 60.0),
        "DAMASCUS_STEEL_JETPACK" => ("minecraft:leather_chestplate", 75.0),
        "REINFORCED_ALLOY_JETPACK" => ("minecraft:leather_chestplate", 100.0),
        "CARBONADO_JETPACK" => ("minecraft:leather_chestplate", 150.0),
        "ARMORED_JETPACK" => ("minecraft:iron_chestplate", 50.0),
        "DURALUMIN_JETBOOTS" => ("minecraft:leather_boots", 20.0),
        "SOLDER_JETBOOTS" => ("minecraft:leather_boots", 30.0),
        "BILLON_JETBOOTS" => ("minecraft:leather_boots", 40.0),
        "STEEL_JETBOOTS" => ("minecraft:leather_boots", 50.0),
        "DAMASCUS_STEEL_JETBOOTS" => ("minecraft:leather_boots", 75.0),
        "REINFORCED_ALLOY_JETBOOTS" => ("minecraft:leather_boots", 100.0),
        "CARBONADO_JETBOOTS" => ("minecraft:leather_boots", 125.0),
        "ARMORED_JETBOOTS" => ("minecraft:iron_boots", 50.0),
        "DURALUMIN_MULTI_TOOL" => ("minecraft:shears", 20.0),
        "SOLDER_MULTI_TOOL" => ("minecraft:shears", 30.0),
        "BILLON_MULTI_TOOL" => ("minecraft:shears", 40.0),
        "STEEL_MULTI_TOOL" => ("minecraft:shears", 50.0),
        "DAMASCUS_STEEL_MULTI_TOOL" => ("minecraft:shears", 60.0),
        "REINFORCED_ALLOY_MULTI_TOOL" => ("minecraft:shears", 75.0),
        "CARBONADO_MULTI_TOOL" => ("minecraft:shears", 100.0),
        "PORTABLE_TELEPORTER" => ("minecraft:compass", 50.0),
        _ => return None,
    })
}

pub(crate) enum Action {
    Charge(ItemStack),
    ReturnFull,
    ReturnOther,
}

// A migrated literal may be an NBT string, a component with "extra", or
// a list of components. Resolve none of the dynamic text forms.
fn literal(tag: &Tag, output: &mut String, depth: u32) -> Option<()> {
    if depth > 32 {
        return None;
    }
    match tag {
        Tag::String(text) => output.push_str(&text.to_string().ok()?),
        Tag::List(children) => {
            for child in children {
                literal(child, output, depth + 1)?;
            }
        }
        Tag::Compound(node) => {
            let Tag::String(text) = node.get(&"text".into())? else {
                return None;
            };
            output.push_str(&text.to_string().ok()?);
            if let Some(extra) = node.get(&"extra".into()) {
                literal(extra, output, depth + 1)?;
            }
        }
        _ => return None,
    }
    Some(())
}

fn number(text: &str) -> Option<f32> {
    if !text
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return None;
    }
    text.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn legacy_charge(tag: &Tag) -> Option<f32> {
    let mut text = String::new();
    literal(tag, &mut text, 0)?;
    if text.contains('§') {
        let mut plain = String::new();
        let mut letters = text.chars().peekable();
        while let Some(letter) = letters.next() {
            if letter == '§'
                && letters.peek().is_some_and(|ch| {
                    matches!(
                        ch.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r' | 'x'
                    )
                })
            {
                letters.next();
            } else {
                plain.push(letter);
            }
        }
        text = plain;
    }
    let (charge, capacity) = text
        .strip_prefix(CHARGE_PREFIX)?
        .strip_suffix(" J")?
        .split_once(" / ")?;
    number(capacity)?;
    number(charge)
}

fn display_number(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn charge_lore(current: f32, maximum: f32) -> Tag {
    let amount = format!(
        "{} / {} J",
        display_number(current),
        display_number(maximum)
    );
    Tag::compound([
        ("text", Tag::String("".into())),
        (
            "extra",
            Tag::List(vec![
                Tag::compound([
                    ("text", Tag::String("⇨ ".into())),
                    ("color", Tag::String("dark_gray".into())),
                    ("bold", Tag::Byte(0)),
                    ("italic", Tag::Byte(0)),
                    ("obfuscated", Tag::Byte(0)),
                    ("strikethrough", Tag::Byte(0)),
                    ("underlined", Tag::Byte(0)),
                ]),
                Tag::compound([
                    ("text", Tag::String("⚡ ".into())),
                    ("color", Tag::String("yellow".into())),
                    ("italic", Tag::Byte(0)),
                ]),
                Tag::compound([
                    ("text", Tag::String(amount.as_str().into())),
                    ("color", Tag::String("gray".into())),
                    ("italic", Tag::Byte(0)),
                ]),
            ]),
        ),
    ])
}

pub(crate) fn action(host: &dyn Host, stack: &ItemStack) -> Option<Action> {
    if stack.count() != 1 {
        return None;
    }
    let id = item_id(stack)?;
    let Some((carrier, maximum)) = rechargeable(&id) else {
        return Some(Action::ReturnOther);
    };
    if stack.item().id(host) != carrier {
        return None;
    }
    let mut patch = stack.components()?.as_nbt().clone();
    let mut lines = match patch.remove(&LORE.into()) {
        Some(Tag::List(lines)) => lines,
        Some(_) => return None,
        None => Vec::new(),
    };
    let first = lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| legacy_charge(line).map(|charge| (index, charge)));
    let Tag::Compound(data) = patch.get_mut(&CUSTOM_DATA.into())? else {
        return None;
    };
    let Tag::Compound(values) = data.get_mut(&PUBLIC_VALUES.into())? else {
        return None;
    };
    let current = match values.get(&CHARGE_KEY.into()) {
        Some(Tag::Float(bits)) => {
            let charge = f32::from_bits(*bits);
            if !charge.is_finite() || charge < 0.0 {
                return None;
            }
            charge
        }
        None => first.map_or(0.0, |(_, charge)| charge),
        _ => return None,
    };
    if current >= maximum {
        return Some(Action::ReturnFull);
    }
    // Paper rounds the f32 charge after addition and writes the same float to PDC and lore.
    let charge = (current + COST as f32 / 2.0).min(maximum);
    let charge = ((f64::from(charge) * 100.0).round() / 100.0) as f32;
    values.insert(CHARGE_KEY.into(), Tag::Float(charge.to_bits()));
    let display = charge_lore(charge, maximum);
    if let Some((index, _)) = first {
        lines[index] = display;
    } else {
        lines.push(display);
    }
    patch.remove(&"!minecraft:lore".into());
    patch.insert(LORE.into(), Tag::List(lines));
    let mut result = stack.clone();
    result.set_components(patch).ok()?;
    Some(Action::Charge(result))
}

#[cfg(test)]
mod tests {
    use mod_api::{Compound, Identifier, testing::FakeHost};

    use super::*;

    fn host() -> FakeHost {
        let mut host = FakeHost::new();
        let materials = [
            "minecraft:compass",
            "minecraft:leather_chestplate",
            "minecraft:shears",
            "minecraft:iron_boots",
        ];
        host.registries
            .resolve_items(materials.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        host
    }

    fn paper_stack(
        host: &dyn Host,
        paper_id: &str,
        material: &'static str,
        pdc_charge: Option<f32>,
        lore: Tag,
    ) -> ItemStack {
        let mut stack = Kind::Vanilla(material).stack(host, 1).unwrap();
        let mut data = Compound::from([
            (crate::items::ID_KEY.into(), Tag::String(paper_id.into())),
            ("example:other".into(), Tag::Int(81)),
        ]);
        if let Some(charge) = pdc_charge {
            data.insert(CHARGE_KEY.into(), Tag::Float(charge.to_bits()));
        }
        stack
            .set_components(Compound::from([
                (
                    CUSTOM_DATA.into(),
                    Tag::compound([(PUBLIC_VALUES, Tag::Compound(data))]),
                ),
                (
                    LORE.into(),
                    Tag::List(vec![Tag::String("Other metadata".into()), lore]),
                ),
            ]))
            .unwrap();
        stack
    }

    fn assert_charge(stack: &ItemStack, charge: f32, capacity: f32) {
        let components = stack.components().unwrap().as_nbt();
        let Tag::Compound(data) = &components[&CUSTOM_DATA.into()] else {
            panic!("Missing Paper custom data")
        };
        let Tag::Compound(values) = &data[&PUBLIC_VALUES.into()] else {
            panic!("Missing Paper PDC")
        };
        assert_eq!(values[&CHARGE_KEY.into()], Tag::Float(charge.to_bits()));
        assert_eq!(values[&"example:other".into()], Tag::Int(81));
        assert_eq!(
            components[&LORE.into()],
            Tag::List(vec![
                Tag::String("Other metadata".into()),
                charge_lore(charge, capacity),
            ])
        );
    }

    #[test]
    fn paper_portable_teleporter_uses_pdc_first_and_stops_at_fifty() {
        let host = host();
        let item = paper_stack(
            &host,
            "PORTABLE_TELEPORTER",
            "minecraft:compass",
            Some(47.25),
            charge_lore(1.5, 50.0),
        );
        let Some(Action::Charge(charged)) = action(&host, &item) else {
            panic!("Expected a final charge increment")
        };
        assert_charge(&charged, 50.0, 50.0);
        assert!(matches!(action(&host, &charged), Some(Action::ReturnFull)));
    }

    #[test]
    fn paper_jetpack_uses_legacy_lore_and_retains_independent_equipment_components() {
        let host = host();
        let mut item = paper_stack(
            &host,
            "DURALUMIN_JETPACK",
            "minecraft:leather_chestplate",
            None,
            Tag::String("§8⇨ §e⚡ §714.75 / 20.0 J".into()),
        );
        item.set_damage(7);
        let Some(Action::Charge(charged)) = action(&host, &item) else {
            panic!("Expected a charge increment")
        };
        assert_charge(&charged, 19.75, 20.0);
        assert_eq!(charged.damage_component(), Some(7));
        let Some(Action::Charge(full)) = action(&host, &charged) else {
            panic!("Expected to finish charging")
        };
        assert_charge(&full, 20.0, 20.0);
    }

    #[test]
    fn paper_multitool_validates_the_real_material_and_is_returned_when_full() {
        let host = host();
        let mut item = paper_stack(
            &host,
            "CARBONADO_MULTI_TOOL",
            "minecraft:shears",
            Some(99.25),
            charge_lore(99.25, 100.0),
        );
        let Some(Action::Charge(full)) = action(&host, &item) else {
            panic!("Expected to finish charging")
        };
        assert_charge(&full, 100.0, 100.0);
        assert!(matches!(action(&host, &full), Some(Action::ReturnFull)));
        item.set_count(2).unwrap();
        assert!(action(&host, &item).is_none());
        let wrong_material = paper_stack(
            &host,
            "CARBONADO_MULTI_TOOL",
            "minecraft:iron_boots",
            None,
            charge_lore(0.0, 100.0),
        );
        assert!(action(&host, &wrong_material).is_none());
        let unmarked = Kind::Vanilla("minecraft:shears").stack(&host, 1).unwrap();
        assert!(action(&host, &unmarked).is_none());
    }
}
