// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, ops::Range, sync::Mutex};

use mod_api::{
    AttachmentError, BlockEntityItemError, BlockItemPlacePreEvent, Compound, HostTransaction,
    ITEMS, ItemKey, ItemResource, ItemStack, LevelRef, ServerHost, StackWriteError, Tag, Text,
    UseOnEvent, java_double_to_string, java_float_to_string,
};

use super::{
    Component, Networks, Node, Position, factory_identity, item_id, parse_uuid, read_stack,
};

pub(super) const FILTER_SLOTS: [usize; 9] = [19, 20, 21, 28, 29, 30, 37, 38, 39];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CargoSettings {
    pub owner: Option<u128>,
    pub channel: u8,
    pub blacklist: bool,
    pub lore: bool,
    pub round_robin: bool,
    pub smart_fill: bool,
    pub imported_paper: bool,
    pub filters: BTreeMap<usize, Compound>,
}

impl CargoSettings {
    pub(super) fn legacy(channel: u8, passes_unfiltered: bool) -> Self {
        Self {
            owner: None,
            channel,
            blacklist: passes_unfiltered,
            lore: false,
            round_robin: false,
            smart_fill: false,
            imported_paper: false,
            filters: BTreeMap::new(),
        }
    }

    pub(super) fn from_paper(metadata: &toml::Table, filter: bool) -> Result<Self, &'static str> {
        let field = |name| metadata.get(name).and_then(toml::Value::as_str);
        // Paper's NUMERIC pattern is one or more ASCII digits. Its cargo
        // ticker routes other strings on channel zero.
        let channel = match field("frequency") {
            Some(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
                value
                    .parse::<u8>()
                    .ok()
                    .filter(|value| *value <= 16)
                    .ok_or("Invalid Paper cargo frequency.")?
            }
            _ => 0,
        };
        let mut result = Self::legacy(
            channel,
            !filter || field("filter-type") != Some("whitelist"),
        );
        result.lore = filter && field("filter-lore") == Some("true");
        result.round_robin = field("round-robin") == Some("true");
        result.smart_fill = field("smart-fill") == Some("true");
        result.owner = match metadata.get("owner") {
            None => None,
            Some(owner) => Some(
                owner
                    .as_str()
                    .and_then(parse_uuid)
                    .ok_or("Invalid Paper cargo owner UUID.")?,
            ),
        };
        Ok(result)
    }

    pub(super) fn merge_paper(&mut self, paper: Self) -> Result<bool, &'static str> {
        let restore_items = paper.imported_paper && self.import_filters(paper.filters)?;
        let restore_owner = self.owner.is_none() && paper.owner.is_some();
        if restore_owner {
            self.owner = paper.owner;
        }
        Ok(restore_owner || restore_items)
    }

    pub(super) fn import_filters(
        &mut self,
        filters: BTreeMap<usize, Compound>,
    ) -> Result<bool, &'static str> {
        if self.imported_paper {
            return Ok(false);
        }
        if !self.filters.is_empty() {
            return Err("Cargo inventory already contains filter items.");
        }
        self.filters = filters;
        self.imported_paper = true;
        Ok(true)
    }

    pub(super) fn save(&self) -> Compound {
        let mut data = Compound::from([
            ("channel".into(), Tag::Int(i32::from(self.channel))),
            (
                "filters".into(),
                Tag::Compound(
                    self.filters
                        .iter()
                        .map(|(slot, stack)| {
                            (
                                slot.to_string().as_str().into(),
                                Tag::Compound(stack.clone()),
                            )
                        })
                        .collect(),
                ),
            ),
        ]);
        for (name, value) in [
            ("blacklist", self.blacklist),
            ("lore", self.lore),
            ("round_robin", self.round_robin),
            ("smart_fill", self.smart_fill),
            ("paper", self.imported_paper),
        ] {
            data.insert(name.into(), Tag::Byte(i8::from(value)));
        }
        if let Some(owner) = self.owner {
            data.insert(
                "owner".into(),
                Tag::String(format!("{owner:032x}").as_str().into()),
            );
        }
        data
    }

    pub(super) fn load(data: &Compound) -> Result<Self, AttachmentError> {
        let Some(Tag::Int(channel)) = data.get(&"channel".into()) else {
            return Err(AttachmentError::Decode);
        };
        if !(0..=16).contains(channel) {
            return Err(AttachmentError::Decode);
        }
        let flag = |key: &str| match data.get(&key.into()) {
            None | Some(Tag::Byte(0)) => Ok(false),
            Some(Tag::Byte(1)) => Ok(true),
            _ => Err(AttachmentError::Decode),
        };
        let mut filters = BTreeMap::new();
        if let Some(items) = data.get(&"filters".into()) {
            let Tag::Compound(items) = items else {
                return Err(AttachmentError::Decode);
            };
            for (slot, item) in items {
                let slot = slot
                    .to_string()
                    .ok()
                    .and_then(|slot| slot.parse::<usize>().ok())
                    .filter(|slot| FILTER_SLOTS.contains(slot))
                    .ok_or(AttachmentError::Decode)?;
                let Tag::Compound(item) = item else {
                    return Err(AttachmentError::Decode);
                };
                if filters.insert(slot, item.clone()).is_some() {
                    return Err(AttachmentError::Decode);
                }
            }
        }
        Ok(Self {
            owner: match data.get(&"owner".into()) {
                None => None,
                Some(Tag::String(owner)) => Some(
                    parse_uuid(&owner.to_string().map_err(|_| AttachmentError::Decode)?)
                        .ok_or(AttachmentError::Decode)?,
                ),
                _ => return Err(AttachmentError::Decode),
            },
            channel: *channel as u8,
            blacklist: flag("blacklist")?,
            lore: flag("lore")?,
            round_robin: flag("round_robin")?,
            smart_fill: flag("smart_fill")?,
            imported_paper: flag("paper")?,
            filters,
        })
    }
}

pub(super) fn deny_unresearched_cargo(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> bool {
    let Some(component) = item_id(event.stack).and_then(|id| Component::for_item(&id)) else {
        return false;
    };
    let required = match component {
        Component::CargoManager | Component::CargoConnector => {
            &crate::network_recipes::CARGO_BASICS
        }
        Component::CargoInput | Component::CargoOutput { advanced: false } => {
            &crate::network_recipes::CARGO_NODES
        }
        Component::CargoOutput { advanced: true } => &crate::network_recipes::ADVANCED_OUTPUT_NODE,
        _ => return false,
    };
    let Some(player) = host.player(event.player) else {
        return false;
    };
    if crate::research::can_use(host, player, required) {
        return false;
    }
    let _ = player.tell(
        host,
        Text::literal(format!(
            "Unlock {} with /sf research {}.",
            required.name, required.id
        )),
    );
    true
}

pub(super) fn deny_cargo_placement(
    host: &mut dyn ServerHost,
    event: &BlockItemPlacePreEvent<'_>,
) -> bool {
    let component = item_id(event.stack).and_then(|id| Component::for_item(&id));
    if !matches!(
        component,
        Some(Component::CargoInput | Component::CargoOutput { .. })
    ) {
        return false;
    }
    let Some(level) = event.player.entity().level(host) else {
        return false;
    };
    let replaced = level
        .block_state(host, event.pos)
        .map(|state| state.block().id(host));
    if cargo_terminal_can_replace(event.pos, event.clicked_pos, replaced.as_deref()) {
        return false;
    }
    let _ = event.player.tell(
        host,
        crate::hologram_projector::colored_label("&4货运节点必须放置在箱子或机器上!"),
    );
    true
}

fn cargo_terminal_can_replace(pos: Position, clicked: Position, replaced: Option<&str>) -> bool {
    // Paper compares the placed and clicked heights and checks the original
    // material at the resolved destination, which may be the clicked block.
    pos[1] == clicked[1]
        && matches!(
            replaced,
            Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
        )
}

pub(super) fn item_key(host: &dyn ServerHost, id: &str) -> Option<ItemKey> {
    host.item_key(&mod_api::Identifier::parse(id).ok()?)
}

fn component<'a>(stack: &'a ItemStack, key: &str) -> Option<&'a Tag> {
    stack.components()?.as_nbt().get(&key.into())
}

fn paper_lore(stack: &ItemStack) -> Option<&[Tag]> {
    match component(stack, "minecraft:lore") {
        Some(Tag::List(lines)) if !lines.is_empty() => Some(lines),
        _ => None,
    }
}

pub(crate) fn paper_text(tag: &Tag) -> Option<String> {
    render_paper_text(tag, None)
}

const MAX_LEGACY_TEXT: usize = 1 << 20;
const MAX_TEXT_DEPTH: usize = 32;

fn render_paper_text(tag: &Tag, host: Option<&dyn ServerHost>) -> Option<String> {
    let mut output = String::new();
    let mut formatted = false;
    append_paper_text(tag, &mut output, &mut formatted, host, 0)?;
    Some(output)
}

// Paper's legacy ItemMeta accessors visit literal components in order and
// preserve their color and formatting codes, ignoring hover and click events.
fn append_paper_text(
    tag: &Tag,
    output: &mut String,
    formatted: &mut bool,
    host: Option<&dyn ServerHost>,
    depth: usize,
) -> Option<()> {
    if depth >= MAX_TEXT_DEPTH {
        return None;
    }
    let (literal, color, reference) = match tag {
        Tag::List(children) => {
            for child in children {
                let child = if host.is_some() {
                    unwrap_list_item(child)
                } else {
                    child
                };
                append_paper_text(child, output, formatted, host, depth + 1)?;
            }
            return Some(());
        }
        Tag::String(literal) => (literal.to_string().ok()?, None, false),
        Tag::Compound(_) => {
            let (literal, reference) = match tag.get("text") {
                Some(contents) => (text_string(contents)?, false),
                None => (nonliteral_contents(host?, tag, depth)?, true),
            };
            let color = match tag.get("color") {
                Some(Tag::String(color)) => Some(color.to_string().ok()?),
                None => None,
                _ => return None,
            };
            (literal, color, reference)
        }
        _ => return None,
    };
    if let Some(color) = color {
        const COLORS: [&str; 16] = [
            "black",
            "dark_blue",
            "dark_green",
            "dark_aqua",
            "dark_red",
            "dark_purple",
            "gold",
            "gray",
            "dark_gray",
            "blue",
            "green",
            "aqua",
            "red",
            "light_purple",
            "yellow",
            "white",
        ];
        const RGB: [u32; 16] = [
            0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa,
            0x555555, 0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
        ];
        let rgb = color
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .and_then(|hex| u32::from_str_radix(hex, 16).ok());
        let legacy = COLORS
            .iter()
            .position(|&name| name == color)
            .or_else(|| RGB.iter().position(|value| Some(*value) == rgb));
        if let Some(index) = legacy {
            output.push('§');
            output.push(char::from_digit(index as u32, 16)?);
        } else {
            output.push_str("§x");
            for digit in format!("{:06x}", rgb?).chars() {
                output.push('§');
                output.push(digit);
            }
        }
        *formatted = true;
    } else if *formatted && (!literal.is_empty() || reference) {
        output.push_str("§r");
        *formatted = false;
    }
    for (style, code) in [
        ("bold", 'l'),
        ("italic", 'o'),
        ("underlined", 'n'),
        ("strikethrough", 'm'),
        ("obfuscated", 'k'),
    ] {
        if matches!(tag.get(style), Some(Tag::Byte(value)) if *value != 0) {
            output.push('§');
            output.push(code);
            *formatted = true;
        }
    }
    if host.is_some() {
        append_bounded(output, &literal)?;
    } else {
        output.push_str(&literal);
    }
    if let Some(extra) = tag.get("extra") {
        append_paper_text(extra, output, formatted, host, depth + 1)?;
    }
    Some(())
}

fn text_string(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(text) => text.to_string().ok(),
        _ => None,
    }
}

fn unwrap_list_item(tag: &Tag) -> &Tag {
    // Minecraft uses a one-entry compound to wrap bare items in mixed NBT lists.
    match tag {
        Tag::Compound(fields) if fields.len() == 1 => tag.get("").unwrap_or(tag),
        _ => tag,
    }
}

fn append_bounded(output: &mut String, text: &str) -> Option<()> {
    // A cropped string could pass a whitelist (or escape a blacklist).
    if output.len().saturating_add(text.len()) > MAX_LEGACY_TEXT {
        return None;
    }
    output.push_str(text);
    Some(())
}

fn nonliteral_contents(host: &dyn ServerHost, tag: &Tag, depth: usize) -> Option<String> {
    if let Some(key) = tag.get("translate") {
        let key = text_string(key)?;
        let template = host
            .server_default_translation(&key)?
            .unwrap_or_else(|| tag.get("fallback").and_then(text_string).unwrap_or(key));
        let arguments = match tag.get("with") {
            Some(Tag::List(arguments)) => arguments.as_slice(),
            None => &[],
            _ => return None,
        };
        return translate_contents(host, template, arguments, depth);
    }
    if let Some(reference) = tag.get("keybind").or_else(|| tag.get("selector")) {
        return text_string(reference);
    }
    None
}

fn append_argument(
    host: &dyn ServerHost,
    tag: &Tag,
    output: &mut String,
    depth: usize,
) -> Option<()> {
    if depth >= MAX_TEXT_DEPTH {
        return None;
    }
    let text = match tag {
        Tag::List(children) => {
            for child in children {
                append_argument(host, unwrap_list_item(child), output, depth + 1)?;
            }
            return Some(());
        }
        Tag::Byte(n) => n.to_string(),
        Tag::Short(n) => n.to_string(),
        Tag::Int(n) => n.to_string(),
        Tag::Long(n) => n.to_string(),
        Tag::Float(n) => java_float_to_string(f32::from_bits(*n)),
        Tag::Double(n) => java_double_to_string(f64::from_bits(*n)),
        Tag::String(_) => text_string(tag)?,
        Tag::Compound(_) => match tag.get("text") {
            Some(text) => text_string(text)?,
            // Paper's unstyled visit of an unresolved score or NBT component is empty.
            None if tag.get("score").is_some() || tag.get("nbt").is_some() => String::new(),
            None => nonliteral_contents(host, tag, depth)?,
        },
        _ => return None,
    };
    append_bounded(output, &text)?;
    if let Some(extra) = tag.get("extra") {
        append_argument(host, extra, output, depth + 1)?;
    }
    Some(())
}

enum TranslationPart<'a> {
    Text(&'a str),
    Argument(usize),
}

// `TranslatableContents` checks the entire template before visiting the unstyled
// arguments. One missing argument or unsupported token uses the whole raw template.
fn translate_contents(
    host: &dyn ServerHost,
    template: String,
    arguments: &[Tag],
    depth: usize,
) -> Option<String> {
    if template.len() > MAX_LEGACY_TEXT {
        return None;
    }
    let mut tokens = Vec::new();
    let mut remaining = template.as_str();
    let mut sequential = 0usize;
    while let Some((before, suffix)) = remaining.split_once('%') {
        if !before.is_empty() {
            tokens.push(TranslationPart::Text(before));
        }
        if let Some(rest) = suffix.strip_prefix('%') {
            tokens.push(TranslationPart::Text("%"));
            remaining = rest;
            continue;
        }
        let (index, rest) = if let Some(rest) = suffix.strip_prefix('s') {
            let index = sequential;
            sequential += 1;
            (index, rest)
        } else {
            let after_index = suffix.trim_start_matches(|ch: char| ch.is_ascii_digit());
            let index = &suffix[..suffix.len() - after_index.len()];
            let (Ok(index), Some(rest)) = (index.parse::<i32>(), after_index.strip_prefix("$s"))
            else {
                return Some(template);
            };
            if index <= 0 {
                return Some(template);
            }
            (index as usize - 1, rest)
        };
        if index >= arguments.len() {
            return Some(template);
        }
        tokens.push(TranslationPart::Argument(index));
        remaining = rest;
    }
    let mut rendered = String::new();
    let mut rendered_arguments = BTreeMap::new();
    for token in tokens {
        match token {
            TranslationPart::Text(text) => append_bounded(&mut rendered, text)?,
            TranslationPart::Argument(index) => {
                use std::collections::btree_map::Entry;
                let argument = match rendered_arguments.entry(index) {
                    Entry::Occupied(existing) => existing.into_mut(),
                    Entry::Vacant(slot) => {
                        let mut value = String::new();
                        append_argument(
                            host,
                            unwrap_list_item(&arguments[index]),
                            &mut value,
                            depth + 1,
                        )?;
                        slot.insert(value)
                    }
                };
                append_bounded(&mut rendered, argument)?;
            }
        }
    }
    append_bounded(&mut rendered, remaining)?;
    Some(rendered)
}

fn same_text(host: &dyn ServerHost, a: &Tag, b: &Tag) -> bool {
    a == b
        || matches!(
            (render_paper_text(a, Some(host)), render_paper_text(b, Some(host))),
            (Some(a), Some(b)) if a == b
        )
}

fn same_lore(host: &dyn ServerHost, a: &ItemStack, b: &ItemStack) -> bool {
    let lines = |line: &&Tag| !crate::soulbound_rune::is_soulbound_lore(line);
    fn comparable<'a>(host: &dyn ServerHost, line: &'a Tag) -> Result<String, &'a Tag> {
        render_paper_text(line, Some(host)).ok_or(line)
    }
    match (paper_lore(a), paper_lore(b)) {
        (Some(a), Some(b)) => a
            .iter()
            .filter(lines)
            .map(|line| comparable(host, line))
            .eq(b.iter().filter(lines).map(|line| comparable(host, line))),
        (None, None) => true,
        _ => false,
    }
}

fn paper_model_data(stack: &ItemStack) -> Option<i32> {
    match component(stack, "minecraft:custom_model_data")? {
        Tag::Int(value) => Some(*value),
        component => {
            let Tag::List(floats) = component.get("floats")? else {
                return None;
            };
            let Tag::Float(value) = floats.first()? else {
                return None;
            };
            Some(f32::from_bits(*value) as i32)
        }
    }
}

fn potion_type(stack: &ItemStack) -> Option<String> {
    let contents = component(stack, "minecraft:potion_contents")?;
    let Tag::String(key) = (match contents {
        Tag::String(_) => contents,
        _ => contents.get("potion")?,
    }) else {
        return None;
    };
    let key = key.to_string().ok()?;
    Some(key.strip_prefix("minecraft:").unwrap_or(&key).to_owned())
}

fn similar(host: &dyn ServerHost, a: &ItemStack, b: &ItemStack, check_lore: bool) -> bool {
    if a.item() != b.item() {
        return false;
    }
    if let (Some(a), Some(b)) = (item_id(a), item_id(b)) {
        // Paper's registered-item shortcut compares the registry templates. Two
        // bags of the same tier match even when their live ownership lore differs.
        return a == b;
    }
    let has_meta =
        |stack: &ItemStack| stack.components().is_some() || stack.damage_component().is_some();
    let same_name = match (
        component(a, "minecraft:custom_name"),
        component(b, "minecraft:custom_name"),
    ) {
        (Some(a), Some(b)) => same_text(host, a, b),
        (None, None) => true,
        _ => false,
    };
    has_meta(a) == has_meta(b)
        && same_name
        && paper_model_data(a) == paper_model_data(b)
        && (!check_lore || same_lore(host, a, b))
        && (potion_type(a) == potion_type(b)
            || !matches!(
                a.item().id(host).as_str(),
                "minecraft:potion"
                    | "minecraft:splash_potion"
                    | "minecraft:lingering_potion"
                    | "minecraft:tipped_arrow"
            ))
}

struct Filter {
    items: Vec<ItemStack>,
    blacklist: bool,
    lore: bool,
}

impl Filter {
    fn new(host: &dyn ServerHost, node: &Node, filter: &CargoSettings) -> Option<Self> {
        if !node.filters_items() {
            return Some(Self {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            });
        }
        let items = filter
            .filters
            .values()
            .map(|data| read_stack(data, |id| item_key(host, id)))
            .collect::<Option<_>>()?;
        Some(Self {
            items,
            blacklist: filter.blacklist,
            lore: filter.lore,
        })
    }

    fn accepts(&self, host: &dyn ServerHost, item: &ItemStack) -> bool {
        self.items
            .iter()
            .any(|filter| similar(host, item, filter, self.lore))
            != self.blacklist
    }
}

#[derive(Clone, Copy)]
enum InventoryKind {
    Ordinary,
    Chest,
    TrashCan,
    ReactorPort,
    ExperienceCollector,
    GeoMiner,
    InputOnly,
    Assembler(&'static crate::factory::assemblers::Assembler),
    Factory { smeltery: bool },
    HeatedPressure,
    Smelter,
    Brewer,
}

impl InventoryKind {
    fn from_block(id: &str) -> Option<Self> {
        Some(match id {
            "minecraft:furnace" | "minecraft:blast_furnace" | "minecraft:smoker" => Self::Smelter,
            "minecraft:brewing_stand" => Self::Brewer,
            "minecraft:chest" | "minecraft:trapped_chest" => Self::Chest,
            "minecraft:barrel"
            | "minecraft:hopper"
            | "minecraft:dispenser"
            | "minecraft:dropper" => Self::Ordinary,
            id if id.starts_with("minecraft:") && id.ends_with("shulker_box") => Self::Ordinary,
            _ => return None,
        })
    }

    fn extract(self, size: usize) -> Range<usize> {
        match self {
            Self::Ordinary | Self::Chest => 0..size,
            Self::TrashCan => 0..0,
            Self::ReactorPort => 6.min(size)..7.min(size),
            Self::ExperienceCollector => 0..3.min(size),
            Self::GeoMiner => 0..size,
            Self::InputOnly => 0..0,
            Self::Assembler(_) => 0..0,
            Self::Factory { .. } | Self::HeatedPressure => 2..4.min(size),
            Self::Smelter => 2..3.min(size),
            Self::Brewer => 0..3.min(size),
        }
    }

    fn insert(self, host: &dyn ServerHost, size: usize, stack: &ItemStack) -> Range<usize> {
        match self {
            Self::Ordinary | Self::Chest | Self::TrashCan => 0..size,
            Self::Factory { smeltery: true } | Self::HeatedPressure => 0..0,
            Self::Factory { smeltery: false } => 0..2.min(size),
            Self::ReactorPort => {
                if crate::factory::reactors::coolant(host, stack) {
                    3.min(size)..6.min(size)
                } else {
                    0..3.min(size)
                }
            }
            Self::ExperienceCollector | Self::GeoMiner => 0..0,
            Self::InputOnly => 0..size,
            Self::Assembler(spec) => {
                let carrier = stack.item().id(host);
                let slots = if carrier == spec.head.kind.material() {
                    crate::factory::assemblers::HEAD
                } else if carrier == spec.body.kind.material() {
                    crate::factory::assemblers::BODY
                } else {
                    0..0
                };
                slots.start.min(size)..slots.end.min(size)
            }
            Self::Smelter => {
                let fuel = host
                    .item_burn_time(stack.item())
                    .is_some_and(|time| time > 0);
                furnace_input_slots(size, fuel, || {
                    host.block_key(&stack.item().id(host))
                        .and_then(|block| host.block_in_tag(block, "minecraft:logs"))
                        == Some(true)
                })
            }
            Self::Brewer => match stack.item().id(host).as_str() {
                "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion" => {
                    0..3.min(size)
                }
                "minecraft:blaze_powder" => 4..5.min(size),
                _ => 3..4.min(size),
            },
        }
    }

    fn slot_position(
        self,
        host: &dyn ServerHost,
        level: LevelRef,
        pos: Position,
        size: usize,
        slot: usize,
    ) -> Option<Position> {
        if !matches!(self, Self::Chest) || size != 54 {
            return Some(pos);
        }
        let half = level.block_property(host, pos, "type")?;
        let left = match half.as_str() {
            "left" => true,
            "right" => false,
            _ => return None,
        };
        if left == (slot >= 27) {
            return Some(pos);
        }
        // The game exposes the right half first, including when querying at the left half.
        let facing = level.block_property(host, pos, "facing")?;
        let [dx, dz] = match facing.as_str() {
            "north" => [1, 0],
            "south" => [-1, 0],
            "east" => [0, 1],
            "west" => [0, -1],
            _ => return None,
        };
        let side = if left { 1 } else { -1 };
        Some([
            pos[0].checked_add(side * dx)?,
            pos[1],
            pos[2].checked_add(side * dz)?,
        ])
    }
}

fn furnace_input_slots(size: usize, fuel: bool, is_log: impl FnOnce() -> bool) -> Range<usize> {
    // Paper CargoUtils fast-paths smeltability by Bukkit's block LOGS tag,
    // even for furnace variants with no recipe for those logs.
    if fuel {
        usize::from(!is_log())..2.min(size)
    } else {
        0..1.min(size)
    }
}

fn attached_inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    mut pos: Position,
) -> Option<(Position, InventoryKind)> {
    let (axis, delta) = match level.block_property(host, pos, "facing")?.as_str() {
        "north" => (2, 1),
        "south" => (2, -1),
        "east" => (0, -1),
        "west" => (0, 1),
        _ => return None,
    };
    pos[axis] = pos[axis].checked_add(delta)?;
    if super::trash::registered(host, level, pos) {
        return Some((pos, InventoryKind::TrashCan));
    }
    if let Some(id) = factory_identity(host, level, pos) {
        if crate::auto_crafter::is_machine(&id) {
            return None;
        }
        if id == crate::factory::experience_collector::XP_COLLECTOR.id() {
            return Some((pos, InventoryKind::ExperienceCollector));
        }
        if id == crate::geology::GEO_MINER.id() {
            return Some((pos, InventoryKind::GeoMiner));
        }
        if id == crate::factory::tree_growth::TREE_GROWTH_ACCELERATOR.id()
            || crate::factory::animal_machines::machine(&id).is_some()
            || crate::factory::crop_growth::machine(&id).is_some()
        {
            return Some((pos, InventoryKind::InputOnly));
        }
        if let Some(spec) = crate::factory::assemblers::by_id(&id) {
            return Some((pos, InventoryKind::Assembler(spec)));
        }
        if id == crate::factory::reactors::REACTOR_ACCESS_PORT.id() {
            return Some((pos, InventoryKind::ReactorPort));
        }
        if crate::factory::reactors::reactor(&id).is_some() {
            return None;
        }
        if matches!(
            id.as_str(),
            "HEATED_PRESSURE_CHAMBER" | "HEATED_PRESSURE_CHAMBER_2"
        ) {
            return Some((pos, InventoryKind::HeatedPressure));
        }
        let smeltery = crate::factory::ElectricMachine::for_item(&id)
            .is_some_and(crate::factory::ElectricMachine::is_smeltery);
        return Some((pos, InventoryKind::Factory { smeltery }));
    }
    let state = level.block_state(host, pos)?;
    Some((pos, InventoryKind::from_block(&state.block().id(host))?))
}

pub(super) fn tick(
    networks: &Mutex<Networks>,
    host: &mut dyn ServerHost,
    level: LevelRef,
    active: &BTreeMap<Position, (&Node, u32)>,
    group: &[Position],
    profile: &mut crate::timings::Cycle,
) -> u64 {
    let mut inputs = Vec::new();
    let mut outputs = BTreeMap::<u8, Vec<_>>::new();
    let mut input_elapsed: u64 = 0;
    for &pos in group {
        let node = active[&pos].0;
        if !matches!(
            node.component,
            Component::CargoInput | Component::CargoOutput { .. }
        ) {
            continue;
        }
        let started = (node.component == Component::CargoInput)
            .then(|| host.server_monotonic_nanos());
        let options = node.cargo_settings(host, level, pos);
        let channel = options.channel;
        let terminal = attached_inventory(host, level, pos).and_then(|(inventory, kind)| {
            let filter = Filter::new(host, node, &options)?;
            Some(Terminal {
                pos,
                inventory,
                kind,
                options,
                filter,
            })
        });
        if let Some(started) = started.filter(|_| channel < 16) {
            let elapsed = host.server_monotonic_nanos().saturating_sub(started);
            input_elapsed = input_elapsed.saturating_add(elapsed);
            profile.record(&node.id, pos, elapsed);
            inputs.extend(terminal.map(|terminal| (&node.id, terminal)));
        } else if matches!(node.component, Component::CargoOutput { .. }) {
            // Paper counts even temporarily unusable output nodes in this channel.
            outputs.entry(channel).or_default().push(terminal);
        }
    }
    for (id, input) in &inputs {
        let (_, elapsed) = profile.measure(host, id, input.pos, |host| {
            let Some(outputs) = outputs.get(&input.options.channel) else {
                return;
            };
            let cursor_key = (level, input.pos);
            let cursor = if input.options.round_robin {
                networks
                    .lock()
                    .unwrap()
                    .round_robin
                    .get(&cursor_key)
                    .copied()
                    .unwrap_or(0)
            } else {
                0
            };
            let next = route(host, level, input, outputs, cursor, |host, from, to, stack| {
                from != to && level.allow_inventory_transfer(host, from, to, stack)
            });
            if let Some(next) = next
                && input.options.round_robin
            {
                networks
                    .lock()
                    .unwrap()
                    .round_robin
                    .insert(cursor_key, next);
            }
        });
        input_elapsed = input_elapsed.saturating_add(elapsed);
    }
    input_elapsed
}

struct Terminal {
    pos: Position,
    inventory: Position,
    kind: InventoryKind,
    options: CargoSettings,
    filter: Filter,
}

fn route(
    host: &mut dyn ServerHost,
    level: LevelRef,
    input: &Terminal,
    outputs: &[Option<Terminal>],
    cursor: usize,
    mut permit: impl FnMut(&mut dyn ServerHost, Position, Position, &ItemStack) -> bool,
) -> Option<usize> {
    let source = level.capability(host, &ITEMS, input.inventory, None)?;
    let (slot, mut stack) = input.kind.extract(source.size(host)).find_map(|slot| {
        let stack = source
            .resource(host, slot)?
            .to_stack(source.amount(host, slot))?;
        input.filter.accepts(host, &stack).then_some((slot, stack))
    })?;
    // Paper skips rotation when this channel has shrunk past the saved cursor.
    // Its running index still starts at the old cursor.
    let start = if cursor < outputs.len() { cursor } else { 0 };
    for offset in 0..outputs.len() {
        let index = (start + offset) % outputs.len();
        let Some(output) = &outputs[index] else {
            continue;
        };
        if !output.filter.accepts(host, &stack) || input.inventory == output.inventory {
            continue;
        }
        let moved = move_to_first(host, level, input, slot, output, &stack, &mut permit);
        // Each destination fills its first compatible stack, and the next output can receive the remainder.
        if moved == stack.count() {
            return Some((cursor % outputs.len() + offset + 1) % outputs.len());
        }
        if moved != 0 {
            stack = stack.shrink(moved)?;
        }
    }
    None
}

// Paper spreads the same item only among inputs already holding that item.
fn smeltery_slots(
    size: usize,
    mut matching: impl FnMut(usize) -> Option<(u32, u32)>,
) -> Vec<usize> {
    let inputs = crate::factory::SMELTERY_INPUT
        .into_iter()
        .filter(|&slot| slot < size);
    let mut matching_slots: Vec<_> = inputs
        .clone()
        .filter_map(|slot| matching(slot).map(|(count, capacity)| (slot, count, capacity)))
        .collect();
    if matching_slots.is_empty() {
        return inputs.collect();
    }
    matching_slots.sort_by_key(|&(_, count, _)| count);
    matching_slots
        .into_iter()
        .filter_map(|(slot, count, capacity)| (count < capacity).then_some(slot))
        .collect()
}

// Paper uses its amount-sensitive similarity check here, so only
// existing stacks at least as large as this delivery participate in the sort.
fn pressure_slots(
    size: usize,
    incoming: u32,
    mut matching: impl FnMut(usize) -> Option<u32>,
) -> Vec<usize> {
    let slots = 0..2.min(size);
    let mut preferred: Vec<_> = slots
        .clone()
        .filter_map(|slot| {
            let count = matching(slot)?;
            (count >= incoming).then_some((slot, count))
        })
        .collect();
    if preferred.is_empty() {
        return slots.collect();
    }
    preferred.sort_by_key(|&(_, count)| count);
    preferred.into_iter().map(|(slot, _)| slot).collect()
}

fn move_to_first(
    host: &mut dyn ServerHost,
    level: LevelRef,
    input: &Terminal,
    source_slot: usize,
    output: &Terminal,
    stack: &ItemStack,
    mut permit: impl FnMut(&mut dyn ServerHost, Position, Position, &ItemStack) -> bool,
) -> u32 {
    let source_resource = ItemResource::of(stack);
    let source_id = item_id(stack);
    host.transaction(|host, tx| {
        let source = level
            .capability(host, &ITEMS, input.inventory, None)
            .ok_or(())?;
        let destination = level
            .capability(host, &ITEMS, output.inventory, None)
            .ok_or(())?;
        let size = destination.size(host);
        let source_size = source.size(host);
        let from = input
            .kind
            .slot_position(host, level, input.inventory, source_size, source_slot)
            .ok_or(())?;
        let mut endpoint_allowed = BTreeMap::new();
        let preferred = match output.kind {
            InventoryKind::Factory { smeltery: true } => smeltery_slots(size, |slot| {
                let resource = destination.resource(host, slot)?;
                let held = destination.amount(host, slot);
                let current = resource.to_stack(held)?;
                similar(host, &current, stack, true)
                    .then(|| (held, destination.capacity(host, slot, &resource)))
            }),
            InventoryKind::HeatedPressure => pressure_slots(size, stack.count(), |slot| {
                let resource = destination.resource(host, slot)?;
                let count = destination.amount(host, slot);
                let current = resource.to_stack(count)?;
                similar(host, &current, stack, true).then_some(count)
            }),
            _ => Vec::new(),
        };
        for slot in preferred
            .into_iter()
            .chain(output.kind.insert(host, size, stack))
        {
            let direct_ingredient = matches!(output.kind, InventoryKind::Brewer)
                && slot == 3
                && destination.owner() == "the game";
            let stored = destination.resource(host, slot);
            let mut insert_resource = &source_resource;
            if let Some(held) = &stored
                && let Some(current) = held.to_stack(destination.amount(host, slot))
            {
                if !similar(host, &current, stack, true) {
                    continue;
                }
                // For a matching Slimefun ID, Paper grows the existing stack in place.
                // Use its components for capacity and insertion and keep the source's
                // components for the withdrawal.
                if source_id.is_some() && item_id(&current) == source_id {
                    insert_resource = held;
                }
                if !direct_ingredient
                    && input.options.smart_fill
                    && current.count() >= destination.capacity(host, slot, insert_resource)
                {
                    return Ok(0);
                }
            }
            let to = output
                .kind
                .slot_position(host, level, output.inventory, size, slot)
                .ok_or(())?;
            if !*endpoint_allowed
                .entry(to)
                .or_insert_with(|| permit(host, from, to, stack))
            {
                continue;
            }
            if insert_resource != &source_resource
                && destination.resource(host, slot).as_ref() != Some(insert_resource)
            {
                return Err(());
            }
            let moved = if direct_ingredient {
                // Paper writes the brewer's inventory directly. ITEMS follows hopper rules
                // and rejects non-reagents even though Paper cargo sends them to this slot.
                let brewer = host.block_entity(level, to).ok_or(())?;
                let held = brewer.item_slot(host, 3).map_err(|_| ())?;
                insert_brewing_ingredient(stack, insert_resource, held, |replacement| {
                    brewer.set_item_slot(host, tx, 3, Some(replacement))
                })
                .map_err(|_| ())?
            } else {
                destination.insert(host, tx, slot, insert_resource, stack.count())
            };
            if moved == 0 {
                // A brewer has no other ingredient slot. A full matching stack
                // leaves the delivery available to the next destination.
                if direct_ingredient {
                    return Ok(0);
                }
                continue;
            }
            return if source.extract(host, tx, source_slot, &source_resource, moved) == moved {
                Ok(moved)
            } else {
                Err(())
            };
        }
        Ok(0)
    })
    .unwrap_or(0)
}

fn insert_brewing_ingredient(
    incoming: &ItemStack,
    resource: &ItemResource,
    held: Option<ItemStack>,
    mut write: impl FnMut(ItemStack) -> Result<bool, BlockEntityItemError>,
) -> Result<u32, BlockEntityItemError> {
    if held.as_ref().is_some_and(|held| !resource.matches(held)) {
        return Ok(0);
    }
    let existing = held.as_ref().map_or(0, ItemStack::count);
    let mut replacement = held.unwrap_or_else(|| incoming.clone());
    let mut proposed = existing.saturating_add(incoming.count());
    while proposed > existing {
        replacement
            .set_count(proposed)
            .expect("room above the existing count");
        // The API validates item components and their limit before checking the holder.
        proposed = match write(replacement.clone()) {
            Ok(true) => return Ok(proposed - existing),
            Ok(false) => return Ok(0),
            Err(BlockEntityItemError::StackRejected(StackWriteError::CountExceeded {
                maximum,
            })) if maximum < proposed => maximum,
            Err(BlockEntityItemError::StackTooLarge { limit, .. }) if limit < proposed => limit,
            Err(error) => return Err(error),
        };
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, prelude::ItemsHost, testing::FakeHost};

    #[test]
    fn cargo_terminal_placement_requires_air_at_the_resolved_position() {
        let clicked = [10, 70, 5];
        assert!(!cargo_terminal_can_replace(
            clicked,
            clicked,
            Some("minecraft:short_grass")
        ));
        assert!(!cargo_terminal_can_replace(
            [10, 71, 5],
            clicked,
            Some("minecraft:air")
        ));
        for air in ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"] {
            assert!(cargo_terminal_can_replace([11, 70, 5], clicked, Some(air)));
        }
    }

    fn restored_filter(host: &dyn ServerHost, stack: &ItemStack) -> Filter {
        let node = Node {
            id: "CARGO_NODE_INPUT".to_owned(),
            component: Component::CargoInput,
            channel: 0,
            passes_unfiltered: false,
        };
        let mut settings = CargoSettings::legacy(0, false);
        settings.lore = true;
        settings.imported_paper = true;
        settings
            .filters
            .insert(FILTER_SLOTS[0], stack.to_nbt(host).unwrap());
        Filter::new(host, &node, &CargoSettings::load(&settings.save()).unwrap()).unwrap()
    }

    #[test]
    fn paper_cargo_uses_channel_zero_for_missing_or_nonnumeric_frequency() {
        let channel = |metadata: &toml::Table| {
            CargoSettings::from_paper(metadata, false).map(|settings| settings.channel)
        };
        assert_eq!(channel(&toml::Table::new()), Ok(0));
        for (frequency, expected) in [
            ("", Ok(0)),
            ("broken", Ok(0)),
            ("-1", Ok(0)),
            ("+2", Ok(0)),
            ("０", Ok(0)),
            ("03", Ok(3)),
            ("16", Ok(16)),
            ("17", Err("Invalid Paper cargo frequency.")),
            ("99999999999", Err("Invalid Paper cargo frequency.")),
        ] {
            let metadata = toml::Table::from_iter([("frequency".into(), frequency.into())]);
            assert_eq!(channel(&metadata), expected, "{frequency:?}");
        }
        assert_eq!(channel(&"frequency = 1".parse().unwrap()), Ok(0));
    }

    fn round_robin_fixture() -> (FakeHost, ItemKey, Terminal, [Option<Terminal>; 3]) {
        let mut host = FakeHost::new();
        let material = Identifier::parse("minecraft:stone").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let item = host.item_key(&material).unwrap();
        let mut terminal = |x| {
            let pos = [x, 70, 0];
            host.level.block_entity(pos).with_items(1);
            Terminal {
                pos,
                inventory: pos,
                kind: InventoryKind::Ordinary,
                options: CargoSettings::legacy(0, true),
                filter: Filter {
                    items: Vec::new(),
                    blacklist: true,
                    lore: false,
                },
            }
        };
        let input = terminal(0);
        let outputs = [1, 2, 3].map(|x| Some(terminal(x)));
        (host, item, input, outputs)
    }

    fn cargo_test_count(host: &FakeHost, pos: Position) -> u32 {
        host.level
            .block_entity_at(pos)
            .unwrap()
            .items
            .as_ref()
            .unwrap()
            .slots[0]
            .as_ref()
            .map_or(0, ItemStack::count)
    }

    #[test]
    fn round_robin_keeps_the_order_when_an_output_inventory_returns() {
        let (mut host, item, source, mut outputs) = round_robin_fixture();
        let level = host.level.level();
        let unavailable = outputs[0].take().unwrap();
        host.level.remove_block_entity(unavailable.inventory);
        let restock = |host: &mut FakeHost| {
            host.level
                .block_entity(source.inventory)
                .items
                .as_mut()
                .unwrap()
                .slots[0] = ItemStack::new(item, 1);
        };
        let deliver = |host: &mut FakeHost, outputs: &[Option<Terminal>], cursor| {
            route(host, level, &source, outputs, cursor, |_, _, _, _| true)
        };

        restock(&mut host);
        let cursor = deliver(&mut host, &outputs, 0).unwrap();
        assert_eq!(cursor, 2);
        assert_eq!(cargo_test_count(&host, [2, 70, 0]), 1);
        assert_eq!(cargo_test_count(&host, source.inventory), 0);

        host.level.block_entity(unavailable.inventory).with_items(1);
        outputs[0] = Some(unavailable);
        restock(&mut host);
        assert_eq!(deliver(&mut host, &outputs, cursor), Some(0));
        assert_eq!(cargo_test_count(&host, [1, 70, 0]), 0);
        assert_eq!(cargo_test_count(&host, [2, 70, 0]), 1);
        assert_eq!(cargo_test_count(&host, [3, 70, 0]), 1);
        assert_eq!(cargo_test_count(&host, source.inventory), 0);
    }

    #[test]
    fn round_robin_uses_the_previous_index_after_the_output_channel_shrinks() {
        let (mut host, item, source, outputs) = round_robin_fixture();
        let level = host.level.level();
        let outputs = &outputs[..2];
        host.level
            .block_entity(source.inventory)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = ItemStack::new(item, 2);

        assert_eq!(
            route(&mut host, level, &source, outputs, 3, |_, _, _, _| false),
            None
        );
        assert_eq!(cargo_test_count(&host, source.inventory), 2);
        let cursor = route(&mut host, level, &source, outputs, 3, |_, _, _, _| true);
        assert_eq!(cursor, Some(0));
        assert_eq!(cargo_test_count(&host, [1, 70, 0]), 2);
        assert_eq!(cargo_test_count(&host, [2, 70, 0]), 0);
        assert_eq!(cargo_test_count(&host, source.inventory), 0);
    }

    #[test]
    fn cargo_tries_burnable_logs_in_input_before_fuel_even_for_blast_and_smoking() {
        for block in [
            "minecraft:furnace",
            "minecraft:blast_furnace",
            "minecraft:smoker",
        ] {
            let kind = InventoryKind::from_block(block).unwrap();
            assert!(matches!(kind, InventoryKind::Smelter));
            assert_eq!(kind.extract(3), 2..3);
        }
        assert_eq!(furnace_input_slots(3, true, || true), 0..2);
        assert_eq!(furnace_input_slots(3, true, || false), 1..2);
        assert_eq!(furnace_input_slots(3, false, || unreachable!()), 0..1);
    }

    #[test]
    fn cargo_brewing_ingredient_preserves_components_and_retries_both_stack_limits() {
        let mut host = FakeHost::new();
        let material = Identifier::parse("minecraft:dirt").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let dirt = host.item_key(&material).unwrap();
        let mut incoming = ItemStack::new(dirt, 3).unwrap();
        incoming
            .set_components(Compound::from([(
                "minecraft:custom_data".into(),
                Tag::compound([("cargo", Tag::Int(7))]),
            )]))
            .unwrap();
        let resource = ItemResource::of(&incoming);
        let brewer = InventoryKind::from_block("minecraft:brewing_stand").unwrap();
        assert_eq!(brewer.insert(&host, 5, &incoming), 3..4);
        assert_eq!(brewer.extract(5), 0..3);

        assert_eq!(
            insert_brewing_ingredient(&incoming, &resource, None, |replacement| {
                assert_eq!(replacement, incoming);
                Ok(true)
            }),
            Ok(3)
        );

        let held = resource.to_stack(63);
        let mut proposals = Vec::new();
        assert_eq!(
            insert_brewing_ingredient(&incoming, &resource, held, |replacement| {
                assert!(resource.matches(&replacement));
                let count = replacement.count();
                proposals.push(count);
                match count {
                    66 => Err(StackWriteError::CountExceeded { maximum: 65 }.into()),
                    65 => Err(BlockEntityItemError::StackTooLarge { count, limit: 64 }),
                    64 => Ok(true),
                    _ => panic!("unexpected write count {count}"),
                }
            }),
            Ok(1)
        );
        assert_eq!(proposals, [66, 65, 64]);
        assert_eq!(
            insert_brewing_ingredient(&incoming, &resource, ItemStack::new(dirt, 1), |_| {
                panic!("a different component patch must never be overwritten")
            }),
            Ok(0)
        );
        assert_eq!(
            insert_brewing_ingredient(&incoming, &resource, resource.to_stack(64), |stack| {
                Err(BlockEntityItemError::StackTooLarge {
                    count: stack.count(),
                    limit: 64,
                })
            }),
            Ok(0)
        );
    }

    #[test]
    fn restored_potion_filters_use_the_base_type_for_every_paper_potion_material() {
        let mut host = FakeHost::new();
        let materials = [
            "minecraft:potion",
            "minecraft:splash_potion",
            "minecraft:lingering_potion",
            "minecraft:tipped_arrow",
        ];
        host.registries
            .resolve_items(materials.map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        for material in materials {
            let make = |potion: &str, color: i32| {
                let mut stack = ItemStack::new(item_key(&host, material).unwrap(), 1).unwrap();
                stack
                    .set_components(Compound::from([(
                        "minecraft:potion_contents".into(),
                        Tag::compound([
                            ("potion", Tag::String(potion.into())),
                            ("custom_color", Tag::Int(color)),
                        ]),
                    )]))
                    .unwrap();
                stack
            };
            let filter = restored_filter(&host, &make("minecraft:poison", 0));
            assert!(filter.accepts(&host, &make("poison", 0x1e21d3)));
            assert!(!filter.accepts(&host, &make("minecraft:strong_poison", 0)));
            assert!(!filter.accepts(&host, &make("minecraft:water", 0)));
        }
    }

    #[test]
    fn restored_backpack_filters_keep_papers_registered_tier_shortcut() {
        use crate::backpacks::{MEDIUM_BACKPACK, SMALL_BACKPACK};

        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(SMALL_BACKPACK.material()).unwrap()])
            .unwrap();
        let make = |kind: crate::items::Kind, label: &str| {
            let mut stack = kind.stack(&host, 1).unwrap();
            stack
                .apply_components(&Compound::from([(
                    "minecraft:lore".into(),
                    Tag::List(vec![crate::items::text(label, "gray", false)]),
                )]))
                .unwrap();
            stack
        };
        let first = make(SMALL_BACKPACK, "ID: 2728cc32-8191-44e7-86c9-2120aa3896c4#1");
        let other_owner = "ID: 23938676-252c-4992-9a7a-1879732cdbe0#8";
        let filter = restored_filter(&host, &first);
        assert!(filter.accepts(&host, &make(SMALL_BACKPACK, other_owner)));
        assert!(!filter.accepts(&host, &make(MEDIUM_BACKPACK, other_owner)));
    }

    #[test]
    fn restored_lore_filters_ignore_only_the_soulbound_line_and_use_papers_model_number() {
        let mut host = FakeHost::new();
        let id = "minecraft:stick";
        host.registries
            .resolve_items([Identifier::parse(id).unwrap()])
            .unwrap();
        let make = |lines: Vec<Tag>, model: Tag| {
            let mut stack = ItemStack::new(item_key(&host, id).unwrap(), 1).unwrap();
            stack
                .set_components(Compound::from([
                    ("minecraft:lore".into(), Tag::List(lines)),
                    ("minecraft:custom_model_data".into(), model),
                ]))
                .unwrap();
            stack
        };
        let label = crate::items::text("Cargo", "green", false);
        let gray = crate::items::text("Soulbound", "gray", false);
        let model = |primary: f32, secondary: f32| {
            Tag::compound([(
                "floats",
                Tag::List(vec![
                    Tag::Float(primary.to_bits()),
                    Tag::Float(secondary.to_bits()),
                ]),
            )])
        };
        let original = make(vec![label.clone()], model(12.0, 5.0));
        let filter = restored_filter(&host, &original);
        assert!(filter.accepts(
            &host,
            &make(vec![label.clone(), gray.clone()], model(12.9, 1.0))
        ));
        assert!(!filter.accepts(
            &host,
            &make(
                vec![label.clone(), crate::items::text("Soulbound", "red", false)],
                model(12.0, 5.0)
            )
        ));
        assert!(!filter.accepts(&host, &make(vec![label], model(13.0, 5.0))));
        let gray_only = restored_filter(&host, &make(vec![gray], model(12.0, 5.0)));
        assert!(!gray_only.accepts(&host, &make(Vec::new(), model(12.0, 5.0))));
    }

    #[test]
    fn restored_filter_compares_paper_text_instead_of_the_component_tag_shape() {
        let mut host = FakeHost::new();
        let id = "minecraft:stick";
        host.registries
            .resolve_items([Identifier::parse(id).unwrap()])
            .unwrap();
        let make = |name: Tag, line: Tag| {
            let mut stack = ItemStack::new(item_key(&host, id).unwrap(), 1).unwrap();
            stack
                .set_components(Compound::from([
                    ("minecraft:custom_name".into(), name),
                    ("minecraft:lore".into(), Tag::List(vec![line])),
                ]))
                .unwrap();
            stack
        };
        let original = make(
            Tag::String("Archived Cargo".into()),
            crate::items::text("Restricted", "green", false),
        );
        let name = Tag::compound([
            ("text", Tag::String("Archived Cargo".into())),
            ("italic", Tag::Byte(0)),
        ]);
        let equivalent = Tag::compound([
            ("text", Tag::String("".into())),
            (
                "extra",
                Tag::List(vec![Tag::compound([
                    ("text", Tag::String("Restricted".into())),
                    ("color", Tag::String("#55ff55".into())),
                ])]),
            ),
        ]);
        let filter = restored_filter(&host, &original);
        assert!(filter.accepts(&host, &make(name.clone(), equivalent)));
        assert!(!filter.accepts(
            &host,
            &make(name, crate::items::text("Restricted", "aqua", false))
        ));

        let literal = |text: &str| Tag::String(text.into());
        let colored_lore = |contents_field, color| {
            Tag::compound([
                (contents_field, literal("warehouse.control.key")),
                ("color", literal(color)),
                ("extra", Tag::List(vec![literal(" 后缀")])),
            ])
        };
        let original = make(
            crate::items::text("@s", "gray", false),
            colored_lore("text", "aqua"),
        );
        let filter = restored_filter(&host, &original);
        let selector = Tag::compound([("selector", literal("@s")), ("color", literal("gray"))]);
        let keybind = colored_lore("keybind", "aqua");
        assert_eq!(
            render_paper_text(&keybind, Some(&host)).as_deref(),
            Some("§bwarehouse.control.key§r 后缀")
        );
        assert!(filter.accepts(&host, &make(selector.clone(), keybind)));
        assert!(!filter.accepts(&host, &make(selector, colored_lore("keybind", "gray"))));
    }

    #[test]
    fn cargo_compares_translatable_item_meta_using_paper_formatting() {
        use mod_api::{
            BackendRef, Host,
            prelude::{ConfigServerHost, ItemsHost},
        };

        struct TestServer {
            game: FakeHost,
            language_available: bool,
        }
        impl Host for TestServer {
            fn backend(&self) -> Option<BackendRef<'_>> {
                self.game.backend()
            }
        }
        impl ServerHost for TestServer {}
        mod_api::impl_host_areas!(TestServer, except(items));
        mod_api::impl_server_host_areas!(TestServer, except(config));
        impl ItemsHost for TestServer {
            fn item_key(&self, id: &Identifier) -> Option<ItemKey> {
                self.game.item_key(id)
            }
        }
        impl ConfigServerHost for TestServer {
            fn server_default_translation(&self, key: &str) -> Option<Option<String>> {
                self.language_available.then(|| match key {
                    "item.minecraft.apple" => Some("Apple".into()),
                    "chat.type.text" => Some("<%s> %s".into()),
                    _ => None,
                })
            }
        }

        let mut host = TestServer {
            game: FakeHost::new(),
            language_available: true,
        };
        let id = Identifier::parse("minecraft:stick").unwrap();
        host.game.registries.resolve_items([id.clone()]).unwrap();
        let item = host.item_key(&id).unwrap();
        let stack = |name: Tag, line: Tag| {
            let mut stack = ItemStack::new(item, 1).unwrap();
            stack
                .set_components(Compound::from([
                    ("minecraft:custom_name".into(), name),
                    ("minecraft:lore".into(), Tag::List(vec![line])),
                ]))
                .unwrap();
            stack
        };
        let literal = |value: &str| Tag::String(value.into());
        let apple = Tag::compound([
            ("translate", literal("item.minecraft.apple")),
            ("fallback", literal("苹果")),
        ]);
        let lore = Tag::compound([
            ("translate", literal("chat.type.text")),
            ("color", literal("aqua")),
            (
                "with",
                Tag::List(vec![crate::items::text("雪", "red", false), apple.clone()]),
            ),
            (
                "extra",
                Tag::List(vec![Tag::compound([("", literal(" 后缀"))])]),
            ),
        ]);
        assert_eq!(
            render_paper_text(&lore, Some(&host)).as_deref(),
            Some("§b<雪> Apple§r 后缀")
        );
        let archive = stack(literal("Apple"), literal("§b<雪> Apple§r 后缀"));
        let filter = restored_filter(&host, &archive);
        let translated = stack(apple.clone(), lore.clone());
        assert!(filter.accepts(&host, &translated));
        assert_eq!(
            component(&translated, "minecraft:custom_name"),
            Some(&apple)
        );
        assert_eq!(paper_lore(&translated), Some([lore].as_slice()));

        let unmatched = literal("warehouse.item.missing");
        for (fallback, with, expected) in [
            (None, vec![], "warehouse.item.missing"),
            (Some("锡粉"), vec![], "锡粉"),
            (
                Some("%2$s / %1$s / %s / %%"),
                vec![literal("雪"), literal("锡")],
                "锡 / 雪 / 雪 / %",
            ),
            (Some("Value %s!"), vec![], "Value %s!"),
            (Some("Value %d!"), vec![Tag::Int(9)], "Value %d!"),
            (
                Some("%s / %s / %s / %s"),
                vec![
                    Tag::Byte(1),
                    Tag::compound([("", Tag::Float(123.0f32.to_bits()))]),
                    Tag::Double(1.0e20f64.to_bits()),
                    Tag::Int(9),
                ],
                "1 / 123.0 / 1.0E20 / 9",
            ),
        ] {
            let mut tag = Compound::from([
                ("translate".into(), unmatched.clone()),
                ("with".into(), Tag::List(with)),
            ]);
            if let Some(fallback) = fallback {
                tag.insert("fallback".into(), literal(fallback));
            }
            assert_eq!(
                render_paper_text(&Tag::Compound(tag), Some(&host)).as_deref(),
                Some(expected)
            );
        }

        let too_long = Tag::compound([
            ("translate", unmatched),
            ("fallback", literal("%1$s%1$s")),
            (
                "with",
                Tag::List(vec![literal(&"x".repeat(MAX_LEGACY_TEXT / 2 + 1))]),
            ),
        ]);
        assert!(render_paper_text(&too_long, Some(&host)).is_none());

        host.language_available = false;
        assert!(!filter.accepts(&host, &translated));
        assert!(restored_filter(&host, &translated).accepts(&host, &translated));
    }

    #[test]
    fn smeltery_cargo_prefers_existing_partial_stacks_and_refuses_full_matching_inputs() {
        assert_eq!(smeltery_slots(8, |_| None), [4, 5, 0, 1, 6, 7]);
        assert_eq!(
            smeltery_slots(8, |slot| match slot {
                4 => Some((12, 64)),
                0 => Some((3, 64)),
                6 => Some((64, 64)),
                _ => None,
            }),
            [0, 4]
        );
        assert!(smeltery_slots(8, |slot| (slot == 4).then_some((64, 64))).is_empty());
    }

    #[test]
    fn a_heated_pressure_chamber_preserves_the_empty_second_ingredient_slot() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let material = Identifier::parse("minecraft:sugar").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let item = host.item_key(&material).unwrap();
        let source = [3, 70, 4];
        let target = [5, 70, 4];
        host.level.block_entity(source).with_items(1);
        host.level.block_entity(target).with_items(4);
        let terminal = |pos, kind| Terminal {
            pos,
            inventory: pos,
            kind,
            options: CargoSettings::legacy(0, true),
            filter: Filter {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            },
        };
        let input = terminal(source, InventoryKind::Ordinary);
        let output = terminal(target, InventoryKind::HeatedPressure);
        let put = |host: &mut FakeHost, pos, slot, count| {
            host.level.block_entity(pos).items.as_mut().unwrap().slots[slot] =
                ItemStack::new(item, count);
        };
        let count = |host: &FakeHost, pos, slot: usize| {
            host.level
                .block_entity_at(pos)
                .unwrap()
                .items
                .as_ref()
                .unwrap()
                .slots[slot]
                .as_ref()
                .map_or(0, ItemStack::count)
        };
        let incoming = ItemStack::new(item, 2).unwrap();
        let deliver = |host: &mut FakeHost| {
            move_to_first(
                host,
                level,
                &input,
                0,
                &output,
                &incoming,
                |_, from, to, _| {
                    assert_eq!((from, to), (source, target));
                    true
                },
            )
        };

        // Both Paper tiers expose input 19 and 20 as internal slots 0 and 1.
        // A full matching stack in 20 reserves 19 for the other ingredient.
        put(&mut host, source, 0, 2);
        put(&mut host, target, 1, 64);
        assert_eq!(deliver(&mut host), 0);
        assert_eq!(
            (
                count(&host, source, 0),
                count(&host, target, 0),
                count(&host, target, 1)
            ),
            (2, 0, 64)
        );
        // Space made in a matching stack is used before the earlier empty slot.
        put(&mut host, target, 1, 20);
        assert_eq!(deliver(&mut host), 2);
        assert_eq!((count(&host, target, 0), count(&host, target, 1)), (0, 22));
        // A stack smaller than the incoming stack does not take part in the preference.
        put(&mut host, source, 0, 2);
        put(&mut host, target, 1, 1);
        assert_eq!(deliver(&mut host), 2);
        assert_eq!((count(&host, target, 0), count(&host, target, 1)), (2, 1));
        assert_eq!(
            pressure_slots(4, 2, |slot| [Some(12), Some(3)][slot]),
            [1, 0]
        );
    }

    #[test]
    fn cargo_merges_english_paper_dust_into_the_localized_stack_without_replacing_its_data() {
        use crate::items::{TIN_DUST, ZINC_DUST, text};

        let mut host = FakeHost::new();
        let level = host.level.level();
        host.registries
            .resolve_items(
                [TIN_DUST.material(), "minecraft:stone"].map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let source = [3, 70, 4];
        let target = [5, 70, 4];
        let terminal = |pos| Terminal {
            pos,
            inventory: pos,
            kind: InventoryKind::Ordinary,
            options: CargoSettings::legacy(0, true),
            filter: Filter {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            },
        };
        let input = terminal(source);
        let mut smart_input = terminal(source);
        smart_input.options.smart_fill = true;
        let output = terminal(target);

        let paper_data = |batch: &'static str| {
            Tag::compound([(
                "PublicBukkitValues",
                Tag::compound([
                    ("slimefun:slimefun_item", Tag::String("TIN_DUST".into())),
                    ("pigeonqa:batch", Tag::String(batch.into())),
                ]),
            )])
        };
        let mut incoming = TIN_DUST.stack(&host, 8).unwrap();
        incoming
            .apply_components(&Compound::from([
                (
                    "minecraft:custom_name".into(),
                    text("Tin Dust", "gold", false),
                ),
                (
                    "minecraft:lore".into(),
                    Tag::List(vec![text("Paper warehouse", "gray", false)]),
                ),
                ("minecraft:custom_data".into(), paper_data("source")),
                // The original stack's own limit must not be used for the held target.
                ("minecraft:max_stack_size".into(), Tag::Int(16)),
            ]))
            .unwrap();
        let mut held = TIN_DUST.stack(&host, 60).unwrap();
        held.apply_components(&Compound::from([
            (
                "minecraft:lore".into(),
                Tag::List(vec![text("锡粉仓库", "gray", false)]),
            ),
            ("minecraft:custom_data".into(), paper_data("target")),
        ]))
        .unwrap();
        assert!(
            paper_text(component(&held, "minecraft:custom_name").unwrap())
                .unwrap()
                .contains("锡粉")
        );
        assert!(similar(&host, &held, &incoming, true));
        assert!(!ItemResource::of(&incoming).matches(&held));
        let blocker = ItemStack::new(item_key(&host, "minecraft:stone").unwrap(), 64).unwrap();
        host.level.block_entity(source).with_items(1);
        let slots = &mut host
            .level
            .block_entity(target)
            .with_items(4)
            .items
            .as_mut()
            .unwrap()
            .slots;
        slots.fill(Some(blocker));
        let put = |host: &mut FakeHost, pos, stack| {
            host.level.block_entity(pos).items.as_mut().unwrap().slots[0] = Some(stack);
        };
        let slot = |host: &FakeHost, pos| {
            host.level
                .block_entity_at(pos)
                .unwrap()
                .items
                .as_ref()
                .unwrap()
                .slots[0]
                .clone()
        };
        let deliver = |host: &mut FakeHost, input: &Terminal, stack: &ItemStack| {
            move_to_first(
                host,
                level,
                input,
                0,
                &output,
                stack,
                |_, from, to, offered| {
                    assert_eq!((from, to, offered), (source, target, stack));
                    true
                },
            )
        };
        let remaining = incoming.clone().shrink(4).unwrap();
        let target_resource = ItemResource::of(&held);
        let full = target_resource.to_stack(64);
        for input in [&input, &smart_input] {
            put(&mut host, source, incoming.clone());
            put(&mut host, target, held.clone());
            assert_eq!(deliver(&mut host, input, &incoming), 4);
            assert_eq!(slot(&host, source), Some(remaining.clone()));
            assert_eq!(slot(&host, target), full);
        }

        // A stale offer is allowed to attempt the insert, then the insufficient
        // matching source causes the enclosing transaction to restore both sides.
        let partly_empty = ItemResource::of(&held).to_stack(58).unwrap();
        put(&mut host, target, partly_empty.clone());
        assert_eq!(deliver(&mut host, &input, &incoming), 0);
        assert_eq!(slot(&host, source), Some(remaining.clone()));
        assert_eq!(slot(&host, target), Some(partly_empty));

        // Refuse to copy the earlier target's components into a newly emptied slot.
        put(&mut host, target, held.clone());
        assert_eq!(
            move_to_first(
                &mut host,
                level,
                &input,
                0,
                &output,
                &remaining,
                |host, _, _, _| {
                    let removed = host
                        .transaction(|host, tx| {
                            let storage = level.capability(host, &ITEMS, target, None).unwrap();
                            Ok::<_, ()>(storage.extract(host, tx, 0, &target_resource, 60))
                        })
                        .unwrap();
                    assert_eq!(removed, 60);
                    true
                }
            ),
            0
        );
        assert_eq!(slot(&host, source), Some(remaining.clone()));
        assert_eq!(slot(&host, target), Some(held.clone()));

        let zinc = ZINC_DUST.stack(&host, 60).unwrap();
        put(&mut host, target, zinc.clone());
        assert_eq!(deliver(&mut host, &input, &remaining), 0);
        assert_eq!(slot(&host, source), Some(remaining.clone()));
        assert_eq!(slot(&host, target), Some(zinc));

        host.level
            .block_entity(target)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = None;
        assert_eq!(deliver(&mut host, &input, &remaining), 4);
        assert_eq!(slot(&host, source), None);
        assert_eq!(slot(&host, target), Some(remaining));

        let vanilla = |count, batch| {
            let mut stack = ItemStack::new(incoming.item(), count).unwrap();
            stack
                .apply_components(&Compound::from([(
                    "minecraft:custom_data".into(),
                    Tag::compound([("witness", Tag::Int(batch))]),
                )]))
                .unwrap();
            stack
        };
        let vanilla_source = vanilla(8, 1);
        let vanilla_target = vanilla(60, 2);
        put(&mut host, source, vanilla_source.clone());
        put(&mut host, target, vanilla_target.clone());
        assert_eq!(deliver(&mut host, &input, &vanilla_source), 0);
        assert_eq!(slot(&host, source), Some(vanilla_source));
        assert_eq!(slot(&host, target), Some(vanilla_target));
    }

    #[test]
    fn only_the_moved_amount_leaves_source_and_a_stale_source_rolls_back_destination() {
        let mut host = FakeHost::new();
        let level = host.level.level();
        let material = Identifier::parse("minecraft:stone").unwrap();
        host.registries.resolve_items([material.clone()]).unwrap();
        let item = host.item_key(&material).unwrap();
        let source = [3, 70, 4];
        let target = [5, 70, 4];
        host.level
            .block_entity(source)
            .with_items(1)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = ItemStack::new(item, 3);
        host.level
            .block_entity(target)
            .with_items(1)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = ItemStack::new(item, 63);
        let terminal = |pos| Terminal {
            pos,
            inventory: pos,
            kind: InventoryKind::Ordinary,
            options: CargoSettings::legacy(0, true),
            filter: Filter {
                items: Vec::new(),
                blacklist: true,
                lore: false,
            },
        };
        let (input, output) = (terminal(source), terminal(target));
        let stack = ItemStack::new(item, 3).unwrap();
        let permit = |_: &mut dyn ServerHost, from, to, _: &ItemStack| {
            assert_eq!((from, to), (source, target));
            true
        };
        assert_eq!(
            move_to_first(&mut host, level, &input, 0, &output, &stack, permit),
            1
        );
        let count = |host: &FakeHost, pos| {
            host.level
                .block_entity_at(pos)
                .unwrap()
                .items
                .as_ref()
                .unwrap()
                .slots[0]
                .as_ref()
                .map_or(0, ItemStack::count)
        };
        assert_eq!((count(&host, source), count(&host, target)), (2, 64));
        host.level
            .block_entity(target)
            .items
            .as_mut()
            .unwrap()
            .slots[0] = None;
        assert_eq!(
            move_to_first(&mut host, level, &input, 0, &output, &stack, permit),
            0
        );
        assert_eq!((count(&host, source), count(&host, target)), (2, 0));
    }
}
