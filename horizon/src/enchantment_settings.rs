// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{LazyLock, RwLock},
};

use mod_api::{Host, ItemStack, Tag};

use crate::{
    advanced_processing::{Family, Machine},
    elemental_staves::STAFF_ELEMENTAL_FIRE,
    items::item_id,
};

use super::component;

const DEFAULT_LIMIT: i32 = 10;
const LORE: &str = "minecraft:lore";
const MACHINES: [(&str, &str); 4] = [
    ("AUTO_ENCHANTER", "&5Auto Enchanter"),
    ("AUTO_ENCHANTER_2", "&5Auto Enchanter &7- &eII"),
    ("AUTO_DISENCHANTER", "&5Auto Disenchanter"),
    ("AUTO_DISENCHANTER_2", "&5Auto Disenchanter &7- &eII"),
];

#[derive(Debug)]
pub(super) struct Rules {
    machines: [MachineRules; 4],
    items: BTreeMap<String, ItemRules>,
}

#[derive(Default, Debug)]
pub(super) struct MachineRules {
    pub override_levels: bool,
    level_limit: Limit,
    count_limit: Limit,
    ignored_lore: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
struct Limit {
    enabled: bool,
    value: i32,
}

impl Default for Limit {
    fn default() -> Self {
        Self {
            enabled: false,
            value: DEFAULT_LIMIT,
        }
    }
}

impl Limit {
    fn read(section: Option<&toml::Value>, key: &str, enabled: bool, previous: Self) -> Self {
        let value = section
            .and_then(|item| item.get(key))
            .and_then(toml::Value::as_integer)
            .and_then(|value| i32::try_from(value).ok());
        let value = match value {
            Some(value) if (0..=i32::from(i16::MAX)).contains(&value) => value,
            // ItemSetting.reload rejects out-of-range integers without replacing the last value.
            Some(_) => previous.value,
            None => DEFAULT_LIMIT,
        };
        Self { enabled, value }
    }

    fn enabled(self) -> Option<i32> {
        self.enabled.then_some(self.value)
    }
}

#[derive(Debug)]
struct ItemRules {
    enchant: Option<bool>,
    disenchant: Option<bool>,
}

static CONFIG: LazyLock<RwLock<Rules>> = LazyLock::new(|| RwLock::new(Rules::default()));

impl Default for Rules {
    fn default() -> Self {
        Self {
            machines: std::array::from_fn(|_| MachineRules::default()),
            items: BTreeMap::new(),
        }
    }
}

pub(crate) fn configure(config: &toml::Table) {
    let mut previous = CONFIG.write().unwrap();
    *previous = Rules::read(config, &previous);
}

pub(super) fn with_rules<T>(use_rules: impl FnOnce(&Rules) -> T) -> T {
    use_rules(&CONFIG.read().unwrap())
}

fn default_lore(display: &str) -> String {
    format!("&7- &cCan't be used in {display}")
}

pub(crate) fn default_config() -> String {
    let mut config = String::new();
    for (id, name) in MACHINES {
        writeln!(
            config,
            "\n[enchantment-machines.{id}]\nuse-enchant-level-limit = false\nenchant-level-limit = 10\nuse-max-enchants = false\nmax-enchants = 10\nuse-ignored-lores = false\nignored-lores = [\"{}\"]",
            default_lore(name),
        ).unwrap();
        if id.starts_with("AUTO_ENCHANTER") {
            config.push_str("override-existing-enchants-lvl = false\n");
        }
    }
    writeln!(
        config,
        "\n[item-settings.{}]\nallow-enchanting = true\nallow-disenchanting = false",
        STAFF_ELEMENTAL_FIRE.id(),
    )
    .unwrap();
    config
}

impl Rules {
    pub fn read(config: &toml::Table, previous: &Self) -> Self {
        let machines = std::array::from_fn(|index| {
            let (id, display) = MACHINES[index];
            let section = config
                .get("enchantment-machines")
                .and_then(|all| all.get(id));
            let flag = |key| {
                section
                    .and_then(|section| section.get(key))
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false)
            };
            let ignored_lore = if flag("use-ignored-lores") {
                section
                    .and_then(|section| section.get("ignored-lores"))
                    .and_then(toml::Value::as_array)
                    .and_then(|entries| {
                        entries
                            .iter()
                            .map(|entry| entry.as_str().map(color_codes))
                            .collect()
                    })
                    .unwrap_or_else(|| vec![color_codes(&default_lore(display))])
            } else {
                Vec::new()
            };
            MachineRules {
                override_levels: id.starts_with("AUTO_ENCHANTER")
                    && flag("override-existing-enchants-lvl"),
                level_limit: Limit::read(
                    section,
                    "enchant-level-limit",
                    flag("use-enchant-level-limit"),
                    previous.machines[index].level_limit,
                ),
                count_limit: Limit::read(
                    section,
                    "max-enchants",
                    flag("use-max-enchants"),
                    previous.machines[index].count_limit,
                ),
                ignored_lore,
            }
        });
        let items = config
            .get("item-settings")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
            .filter_map(|(id, item)| {
                let flag = |key| item.get(key).and_then(toml::Value::as_bool);
                let enchant = flag("allow-enchanting");
                let disenchant = flag("allow-disenchanting");
                (enchant.is_some() || disenchant.is_some()).then(|| {
                    (
                        id.clone(),
                        ItemRules {
                            enchant,
                            disenchant,
                        },
                    )
                })
            })
            .collect();
        Self { machines, items }
    }

    pub fn machine(&self, machine: &Machine) -> Option<&MachineRules> {
        let index = MACHINES
            .iter()
            .position(|(id, _)| *id == machine.item.id())?;
        self.machines.get(index)
    }

    pub fn allows_item(&self, target: &ItemStack, family: Family) -> bool {
        let Some(id) = item_id(target) else {
            return true;
        };
        let fire_staff = id == STAFF_ELEMENTAL_FIRE.id();
        let flag = |get: fn(&ItemRules) -> Option<bool>| {
            self.items.get(&id).and_then(get).or_else(|| {
                // Keep settings files emitted with the old Java field name usable.
                if fire_staff {
                    self.items.get("STAFF_FIRE").and_then(get)
                } else {
                    None
                }
            })
        };
        match family {
            Family::AutoEnchanter => flag(|rules| rules.enchant).unwrap_or(true),
            Family::AutoDisenchanter => flag(|rules| rules.disenchant).unwrap_or(!fire_staff),
            _ => false,
        }
    }
}

impl MachineRules {
    pub fn level_limit(&self) -> Option<i32> {
        self.level_limit.enabled()
    }

    pub fn count_limit(&self) -> Option<usize> {
        self.count_limit.enabled().map(|value| value as usize)
    }

    pub fn accepts_lore(&self, host: &dyn Host, target: &ItemStack) -> bool {
        if self.ignored_lore.is_empty() {
            return true;
        }
        let Some(Tag::List(lines)) = component(host, target, LORE) else {
            return true;
        };
        !lines
            .iter()
            .filter_map(legacy_lore)
            .any(|line| self.ignored_lore.contains(&line))
    }
}

fn color_codes(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&'
            && let Some(&code) = chars.peek()
            && matches!(code.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r' | 'x')
        {
            output.push('§');
            output.push(code.to_ascii_lowercase());
            chars.next();
        } else {
            output.push(ch);
        }
    }
    output
}

const COLORS: [(&str, u32); 16] = [
    ("black", 0x000000),
    ("dark_blue", 0x0000aa),
    ("dark_green", 0x00aa00),
    ("dark_aqua", 0x00aaaa),
    ("dark_red", 0xaa0000),
    ("dark_purple", 0xaa00aa),
    ("gold", 0xffaa00),
    ("gray", 0xaaaaaa),
    ("dark_gray", 0x555555),
    ("blue", 0x5555ff),
    ("green", 0x55ff55),
    ("aqua", 0x55ffff),
    ("red", 0xff5555),
    ("light_purple", 0xff55ff),
    ("yellow", 0xffff55),
    ("white", 0xffffff),
];

fn write_color(output: &mut String, color: &str) -> Option<()> {
    let rgb = color
        .strip_prefix('#')
        .filter(|value| value.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok());
    if let Some(index) = COLORS
        .iter()
        .position(|&(name, value)| name == color || Some(value) == rgb)
    {
        output.push('§');
        output.push(char::from_digit(index as u32, 16)?);
    } else {
        write!(output, "§x").ok()?;
        for digit in format!("{:06X}", rgb?).chars() {
            output.push('§');
            output.push(digit);
        }
    }
    Some(())
}

/// Mirrors the literal-component path used by CraftMetaItem.getLore.
/// CraftChatMessage walks each node with its own style and serializes colors
/// and decorations even when two adjacent nodes happen to have the same style.
fn legacy_lore(line: &Tag) -> Option<String> {
    let mut pending = vec![line];
    let mut output = String::new();
    let mut styled = false;
    while let Some(node) = pending.pop() {
        match node {
            Tag::String(text) => {
                let text = text.to_string().ok()?;
                if !text.is_empty() && styled {
                    output.push_str("§r");
                    styled = false;
                }
                output.push_str(&text);
            }
            Tag::List(children) => pending.extend(children.iter().rev()),
            Tag::Compound(fields) => {
                let Tag::String(text) = fields.get(&"text".into())? else {
                    return None;
                };
                let text = text.to_string().ok()?;
                if let Some(Tag::String(color)) = fields.get(&"color".into()) {
                    write_color(&mut output, &color.to_string().ok()?)?;
                    styled = true;
                } else if !text.is_empty() && styled {
                    output.push_str("§r");
                    styled = false;
                }
                for (name, code) in [
                    ("bold", 'l'),
                    ("italic", 'o'),
                    ("underlined", 'n'),
                    ("strikethrough", 'm'),
                    ("obfuscated", 'k'),
                ] {
                    if fields.get(&name.into()) == Some(&Tag::Byte(1)) {
                        output.push('§');
                        output.push(code);
                        styled = true;
                    }
                }
                output.push_str(&text);
                if let Some(Tag::List(children)) = fields.get(&"extra".into()) {
                    pending.extend(children.iter().rev());
                }
            }
            _ => return None,
        }
    }
    Some(output)
}
