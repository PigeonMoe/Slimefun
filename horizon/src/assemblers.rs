// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Compound, Host, ItemResource, ItemStack, LevelRef, ServerHost, SoundSource, SpawnReason, Tag,
    Transaction,
};

use crate::{
    enchantment_machines::WITHER_PROOF_OBSIDIAN,
    items::{ADVANCED_CIRCUIT_BOARD, ELECTRIC_MOTOR, Kind, REINFORCED_ALLOY_INGOT, item},
    network_recipes::{ANDROID_MEMORY_CORE, BLISTERING_INGOT_3, CARBONADO_EDGED_CAPACITOR},
    recipes::{CraftingRecipe, Research, Stack},
};

item!(
    IRON_GOLEM_ASSEMBLER,
    "iron_block",
    "Iron Golem Assembler",
    "gold",
    None,
    None
);
item!(
    WITHER_ASSEMBLER,
    "obsidian",
    "Wither Assembler",
    "dark_purple",
    None,
    None
);

const GOLEM: Research = Research {
    id: "iron_golem_assembler",
    name: "Automated Iron Golems",
    cost: 30,
};
const WITHER: Research = Research {
    id: "wither_assembler",
    name: "Automated Wither Killer",
    cost: 35,
};
pub(crate) const RESEARCHES: &[Research] = &[GOLEM, WITHER];

const IRON: Kind = Kind::Vanilla("minecraft:iron_block");
const STAR: Kind = Kind::Vanilla("minecraft:nether_star");
pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: assembly_recipe(ADVANCED_CIRCUIT_BOARD, IRON),
        output: Stack {
            kind: IRON_GOLEM_ASSEMBLER,
            count: 1,
        },
        research: &GOLEM,
    },
    CraftingRecipe {
        inputs: assembly_recipe(STAR, WITHER_PROOF_OBSIDIAN),
        output: Stack {
            kind: WITHER_ASSEMBLER,
            count: 1,
        },
        research: &WITHER,
    },
];

const fn assembly_recipe(top: Kind, sides: Kind) -> [Option<Kind>; 9] {
    [
        Some(BLISTERING_INGOT_3),
        Some(top),
        Some(BLISTERING_INGOT_3),
        Some(sides),
        Some(ANDROID_MEMORY_CORE),
        Some(sides),
        Some(ELECTRIC_MOTOR),
        Some(REINFORCED_ALLOY_INGOT),
        Some(CARBONADO_EDGED_CAPACITOR),
    ]
}

pub(crate) const CAPACITY: u32 = 4_096;
pub(crate) const MENU_SLOTS: &[usize] = &[19, 28, 25, 34];
pub(crate) const HEAD: std::ops::Range<usize> = 0..2;
pub(crate) const BODY: std::ops::Range<usize> = 2..4;
const COOLDOWN: i64 = 600;
const ENABLED: &str = "assembler_enabled";
const OFFSET: &str = "assembler_offset_tenths";
const PAPER_CONTROLS: &str = "paper_assembler_controls";

pub(super) fn only_controls(data: &Compound) -> bool {
    data.len()
        == [ENABLED, OFFSET, PAPER_CONTROLS]
            .into_iter()
            .filter(|&key| data.contains_key(&key.into()))
            .count()
}

#[derive(Clone, Copy)]
pub(crate) struct Assembler {
    pub item: Kind,
    pub title: &'static str,
    pub research: &'static Research,
    pub cost: u32,
    pub head: Stack,
    pub body: Stack,
    mob: &'static str,
    head_colour: &'static str,
    body_colour: &'static str,
}

const ASSEMBLERS: &[Assembler] = &[
    Assembler {
        item: IRON_GOLEM_ASSEMBLER,
        title: "Iron Golem Assembler",
        research: &GOLEM,
        cost: 2_048,
        head: Stack {
            kind: Kind::Vanilla("minecraft:carved_pumpkin"),
            count: 1,
        },
        body: Stack {
            kind: IRON,
            count: 4,
        },
        mob: "minecraft:iron_golem",
        head_colour: "minecraft:orange_stained_glass_pane",
        body_colour: "minecraft:white_stained_glass_pane",
    },
    Assembler {
        item: WITHER_ASSEMBLER,
        title: "Wither Assembler",
        research: &WITHER,
        cost: 4_096,
        head: Stack {
            kind: Kind::Vanilla("minecraft:wither_skeleton_skull"),
            count: 3,
        },
        body: Stack {
            kind: Kind::Vanilla("minecraft:soul_sand"),
            count: 4,
        },
        mob: "minecraft:wither",
        head_colour: "minecraft:black_stained_glass_pane",
        body_colour: "minecraft:brown_stained_glass_pane",
    },
];

pub(crate) fn by_id(id: &str) -> Option<&'static Assembler> {
    ASSEMBLERS
        .iter()
        .find(|assembler| assembler.item.id() == id)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Controls {
    enabled: bool,
    offset_tenths: i32,
}

impl Controls {
    pub(super) fn read(data: &Compound) -> Self {
        Self {
            enabled: matches!(data.get(&ENABLED.into()), Some(Tag::Byte(1))),
            offset_tenths: match data.get(&OFFSET.into()) {
                Some(Tag::Int(offset)) => *offset,
                _ => 30,
            },
        }
    }

    pub(super) fn write(self, data: &mut Compound) {
        data.insert(ENABLED.into(), Tag::Byte(i8::from(self.enabled)));
        data.insert(OFFSET.into(), Tag::Int(self.offset_tenths));
    }

    pub(super) fn click(self, slot: i32, button: i32) -> Option<Self> {
        match slot {
            22 => Some(Self {
                enabled: !self.enabled,
                ..self
            }),
            31 => {
                let change = if button == 1 { -1 } else { 1 };
                Some(Self {
                    offset_tenths: self.offset_tenths.checked_add(change)?,
                    ..self
                })
            }
            _ => None,
        }
    }

    pub(super) fn from_paper(paper: &toml::Table) -> Result<Self, String> {
        let enabled = match paper.get("enabled").and_then(toml::Value::as_str) {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => return Err("Invalid Paper assembler enabled value.".into()),
        };
        let offset_tenths = match paper.get("offset").and_then(toml::Value::as_str) {
            Some(value) => {
                let tenths = value
                    .parse::<f64>()
                    .map_err(|_| "Invalid Paper assembler offset.")?
                    * 10.0;
                let rounded = tenths.round();
                if !rounded.is_finite()
                    || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&rounded)
                {
                    return Err("Invalid Paper assembler offset.".into());
                }
                rounded as i32
            }
            None => 30,
        };
        Ok(Self {
            enabled,
            offset_tenths,
        })
    }

    pub(super) fn restore(self, data: &mut Compound) -> bool {
        if data.contains_key(&ENABLED.into()) || data.contains_key(&PAPER_CONTROLS.into()) {
            return false;
        }
        self.write(data);
        data.insert(PAPER_CONTROLS.into(), Tag::Byte(1));
        true
    }
}

/// BlockMenu saves these two instance-specific buttons because neither is a
/// preset slot. Recreate them from BlockStorage metadata, not from the archive.
pub(super) fn paper_control(host: &dyn Host, slot: u32, item: &ItemStack) -> bool {
    if item.count() != 1 || !matches!(slot, 22 | 31) {
        return false;
    }
    let Some(parts) = item.components().map(|components| components.as_nbt()) else {
        return false;
    };
    let Some(Tag::List(lore)) = parts.get(&"minecraft:lore".into()) else {
        return false;
    };
    let Some(title) = parts.get(&"minecraft:custom_name".into()).and_then(paper_text) else {
        return false;
    };
    let expected: &[&str] = match (slot, item.item().id(host).as_str()) {
        (22, "minecraft:gunpowder") if title == "§7Enabled: §4✘" => {
            &["", "§e> Click to enable this Machine"]
        }
        (22, "minecraft:redstone") if title == "§7Enabled: §2✔" => {
            &["", "§e> Click to disable this Machine"]
        }
        (31, "minecraft:piston") if paper_offset(&title) => {
            &["", "§fLeft Click: §7+0.1", "§fRight Click: §7-0.1"]
        }
        _ => return false,
    };
    parts.len() == 2
        && lore.len() == expected.len()
        && lore
            .iter()
            .zip(expected)
            .all(|(line, &text)| paper_text(line).as_deref() == Some(text))
}

fn paper_text(tag: &Tag) -> Option<String> {
    fn literal(tag: &Tag, depth: usize) -> bool {
        if depth >= 8 {
            return false;
        }
        match tag {
            Tag::String(_) => true,
            Tag::List(children) => children.iter().all(|child| literal(child, depth + 1)),
            Tag::Compound(parts) => {
                parts.iter().all(|(name, value)| match name.to_string().as_deref() {
                    Ok("text" | "color") => matches!(value, Tag::String(_)),
                    Ok("bold" | "italic" | "underlined" | "strikethrough" | "obfuscated") => {
                        matches!(value, Tag::Byte(0))
                    }
                    Ok("extra") => literal(value, depth + 1),
                    _ => false,
                })
            }
            _ => false,
        }
    }

    literal(tag, 0)
        .then(|| crate::network_runtime::paper_text(tag))
        .flatten()
}

fn paper_offset(title: &str) -> bool {
    let Some(value) = title
        .strip_prefix("§7Offset: §3")
        .and_then(|title| title.strip_suffix(" Block(s)"))
    else {
        return false;
    };
    let (mantissa, exponent) = value.split_once('E').unwrap_or((value, "1"));
    let exponent = exponent.strip_prefix('-').unwrap_or(exponent);
    let mantissa = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let Some((whole, fraction)) = mantissa.split_once('.') else {
        return false;
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|ch| ch.is_ascii_digit());
    digits(whole)
        && (whole == "0" || !whole.starts_with('0'))
        && digits(fraction)
        && (fraction.len() == 1 || !fraction.ends_with('0'))
        && digits(exponent)
        && !exponent.starts_with('0')
        && value.parse::<f64>().is_ok_and(f64::is_finite)
}

pub(super) fn menu(host: &dyn Host, data: &Compound, spec: &Assembler) -> Vec<Option<ItemStack>> {
    let mut items = vec![super::display_item(host, "minecraft:gray_stained_glass_pane", " "); 45];
    let controls = Controls::read(data);
    let (toggle, label) = if controls.enabled {
        ("minecraft:redstone", "Enabled: ✔")
    } else {
        ("minecraft:gunpowder", "Enabled: ✘")
    };
    items[1] = super::display_item(host, spec.head.kind.material(), "Head Slot");
    items[7] = super::display_item(host, spec.body.kind.material(), "Body Slot");
    items[13] = super::display_item(host, "minecraft:clock", "Cooldown: 30 Seconds");
    items[22] = super::display_item(host, toggle, label);
    let offset = f64::from(controls.offset_tenths) / 10.0;
    items[31] = super::display_item(
        host,
        "minecraft:piston",
        &format!("Offset: {offset:.1} Block(s)"),
    );
    for (border, colour) in [
        (
            &[9, 10, 11, 18, 20, 27, 29, 36, 37, 38][..],
            spec.head_colour,
        ),
        (
            &[15, 16, 17, 24, 26, 33, 35, 42, 43, 44][..],
            spec.body_colour,
        ),
    ] {
        let pane = super::display_item(host, colour, " ");
        for &index in border {
            items[index] = pane.clone();
        }
    }
    for (index, &slot) in MENU_SLOTS.iter().enumerate() {
        items[slot] = super::stack_in(host, data, index);
    }
    items
}

pub(crate) fn stage(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: [i32; 3],
    spec: &'static Assembler,
) -> Option<Spawn> {
    if level.game_time(host)? % COOLDOWN != 0 {
        return None;
    }
    let storage = super::ElectricMachine::for_item(spec.item.id())?.storage(level, pos);
    let data = super::data_at(host, storage)?;
    let controls = Controls::read(&data);
    if !controls.enabled {
        return None;
    }
    for (range, required) in [(BODY, spec.body), (HEAD, spec.head)] {
        let mut left = required.count;
        for index in range {
            let place = super::place(level, pos, index);
            let Some(stack) = place
                .stack(host)
                .filter(|stack| required.kind.matches(host, stack))
            else {
                continue;
            };
            let used = left.min(stack.count());
            if place.extract(host, tx, &ItemResource::of(&stack), used) != used {
                return None;
            }
            left -= used;
            if left == 0 {
                break;
            }
        }
        if left != 0 {
            return None;
        }
    }
    Some(Spawn {
        spec,
        y_offset: f64::from(controls.offset_tenths) / 10.0,
    })
}

pub(crate) struct Spawn {
    spec: &'static Assembler,
    y_offset: f64,
}

impl Spawn {
    pub(crate) fn apply(self, host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
        let [x, y, z] = pos.map(f64::from);
        let center = [x + 0.5, y + self.y_offset, z + 0.5];
        if self.spec.item == IRON_GOLEM_ASSEMBLER {
            let _ = level.play_sound(
                host,
                "minecraft:entity.iron_golem.repair",
                center,
                SoundSource::Blocks,
                0.5,
                1.0,
            );
        }
        let _ = level.spawn_mob(host, self.spec.mob, center, SpawnReason::Triggered);
        if let Some(head) = host.block_state_id(self.spec.head.kind.material()) {
            let _ = level.block_break_effect(host, pos, head);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_controls_survive_import_and_a_repeat_keeps_local_changes() {
        let paper = "enabled = 'true'\noffset = '4.1'".parse().unwrap();
        let controls = Controls::from_paper(&paper).unwrap();
        let mut data = Compound::new();
        assert!(controls.restore(&mut data));
        let changed = Controls::read(&data)
            .click(22, 0)
            .unwrap()
            .click(31, 1)
            .unwrap();
        changed.write(&mut data);
        assert!(!controls.restore(&mut data));
        let active = Controls::read(&data);
        assert!(!active.enabled);
        assert_eq!(active.offset_tenths, 40);
        assert_eq!(MENU_SLOTS, [19, 28, 25, 34]);
        assert!(Controls::from_paper(&"offset = 'NaN'".parse().unwrap()).is_err());
    }
}
