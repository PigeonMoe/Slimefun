// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    Compound, Hand, Host, HostTransaction, ItemPlace, ItemStack, PlayerRef, ServerHost, Tag,
};

use crate::{
    factory::charging_bench,
    hologram_projector::{colored_label, colored_lore},
    items::{
        BILLON_INGOT, CARBONADO, DAMASCUS_STEEL_INGOT, DURALUMIN_INGOT, Kind,
        REINFORCED_ALLOY_INGOT, SMALL_CAPACITOR, SOLDER_INGOT, STEEL_INGOT, item, item_id,
    },
    jet_boots::CARBONADO_TOOLS,
    network_recipes::{LARGE_CAPACITOR, MEDIUM_CAPACITOR},
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

const USE_COST: f32 = 0.3;
const CUSTOM_DATA: &str = "minecraft:custom_data";
const PUBLIC_VALUES: &str = "PublicBukkitValues";
const MODE_KEY: &str = "slimefun:multitool_mode";
const LORE: &str = "minecraft:lore";
const MODE_LINE: &str = "⇨ Mode: ";

pub(crate) const RESEARCH: Research = Research {
    id: "multitools",
    name: "Multi Tools",
    cost: 18,
};

struct Tier {
    item: Kind,
    level: &'static str,
    material: &'static str,
    capacity: f32,
    recipe: CraftingRecipe,
}

item!(
    DURALUMIN_MULTI_TOOL,
    "shears",
    "Multi Tool",
    "blue",
    None,
    None
);
item!(
    SOLDER_MULTI_TOOL,
    "shears",
    "Multi Tool",
    "blue",
    None,
    None
);
item!(
    BILLON_MULTI_TOOL,
    "shears",
    "Multi Tool",
    "blue",
    None,
    None
);
item!(STEEL_MULTI_TOOL, "shears", "Multi Tool", "blue", None, None);
item!(
    DAMASCUS_STEEL_MULTI_TOOL,
    "shears",
    "Multi Tool",
    "blue",
    None,
    None
);
item!(
    REINFORCED_ALLOY_MULTI_TOOL,
    "shears",
    "Multi Tool",
    "blue",
    None,
    None
);
item!(
    CARBONADO_MULTI_TOOL,
    "shears",
    "Multi Tool",
    "blue",
    None,
    None
);

const fn tier(
    item: Kind,
    level: &'static str,
    material: &'static str,
    capacity: f32,
    ingot: Kind,
    capacitor: Kind,
    research: &'static Research,
) -> Tier {
    Tier {
        item,
        level,
        material,
        capacity,
        recipe: CraftingRecipe {
            inputs: [
                Some(ingot),
                None,
                Some(ingot),
                Some(ingot),
                Some(capacitor),
                Some(ingot),
                None,
                Some(ingot),
                None,
            ],
            output: Stack {
                kind: item,
                count: 1,
            },
            research,
        },
    }
}

const TIERS: [Tier; 7] = [
    tier(
        DURALUMIN_MULTI_TOOL,
        "I",
        "Duralumin",
        20.0,
        DURALUMIN_INGOT,
        SMALL_CAPACITOR,
        &RESEARCH,
    ),
    tier(
        SOLDER_MULTI_TOOL,
        "II",
        "Solder",
        30.0,
        SOLDER_INGOT,
        SMALL_CAPACITOR,
        &RESEARCH,
    ),
    tier(
        BILLON_MULTI_TOOL,
        "III",
        "Billon",
        40.0,
        BILLON_INGOT,
        SMALL_CAPACITOR,
        &RESEARCH,
    ),
    tier(
        STEEL_MULTI_TOOL,
        "IV",
        "Steel",
        50.0,
        STEEL_INGOT,
        SMALL_CAPACITOR,
        &RESEARCH,
    ),
    tier(
        DAMASCUS_STEEL_MULTI_TOOL,
        "V",
        "Damascus Steel",
        60.0,
        DAMASCUS_STEEL_INGOT,
        SMALL_CAPACITOR,
        &RESEARCH,
    ),
    tier(
        REINFORCED_ALLOY_MULTI_TOOL,
        "VI",
        "Reinforced Alloy",
        75.0,
        REINFORCED_ALLOY_INGOT,
        MEDIUM_CAPACITOR,
        &RESEARCH,
    ),
    tier(
        CARBONADO_MULTI_TOOL,
        "VII",
        "Carbonado",
        100.0,
        CARBONADO,
        LARGE_CAPACITOR,
        &CARBONADO_TOOLS,
    ),
];

pub(crate) fn recipes() -> impl Iterator<Item = &'static CraftingRecipe> {
    TIERS.iter().map(|tier| &tier.recipe)
}

fn tool(id: &str) -> Option<&'static Tier> {
    TIERS.iter().find(|tier| tier.item.id() == id)
}

fn held_tool(host: &dyn Host, item: &ItemStack) -> Option<&'static Tier> {
    tool(&item_id(item)?).filter(|tier| tier.item.matches(host, item))
}

pub(crate) fn is_tool(host: &dyn Host, item: &ItemStack) -> bool {
    held_tool(host, item).is_some()
}

pub(crate) fn charge_spec(id: &str) -> Option<(&'static str, f32)> {
    let tier = tool(id)?;
    Some((tier.item.material(), tier.capacity))
}

pub(crate) fn decorate(id: &str, patch: &mut Compound) {
    let Some(tier) = tool(id) else { return };
    patch.insert(
        "minecraft:custom_name".into(),
        colored_lore(&format!("&9Multi Tool &7- &e{}", tier.level)),
    );
    patch.insert(
        LORE.into(),
        Tag::List(vec![
            Tag::String("".into()),
            colored_lore(&format!("&8⇨ &7Material: &b{}", tier.material)),
            charging_bench::charge_lore(0.0, tier.capacity),
            Tag::String("".into()),
            colored_lore("&eRight Click&7 to use"),
            colored_lore("&eCrouch & Right Click&7 to switch modes"),
        ]),
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Crafter,
    Eye,
    Wind,
    Grapple,
    GoldPan,
    NetherPan,
}

const MODES: [Mode; 6] = [
    Mode::Crafter,
    Mode::Eye,
    Mode::Wind,
    Mode::Grapple,
    Mode::GoldPan,
    Mode::NetherPan,
];

impl Mode {
    fn item_id(self) -> &'static str {
        match self {
            Self::Crafter => "PORTABLE_CRAFTER",
            Self::Eye => "MAGIC_EYE_OF_ENDER",
            Self::Wind => "STAFF_ELEMENTAL_WIND",
            Self::Grapple => "GRAPPLING_HOOK",
            Self::GoldPan => "GOLD_PAN",
            Self::NetherPan => "NETHER_GOLD_PAN",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Crafter => "Portable Crafter",
            Self::Eye => "Magic Eye of Ender",
            Self::Wind => "Elemental Staff - Wind",
            Self::Grapple => "Grappling Hook",
            Self::GoldPan => "Gold Pan",
            Self::NetherPan => "Nether Gold Pan",
        }
    }
}

fn mode_index(stack: &ItemStack) -> usize {
    let Some(mode) = stack.components().and_then(|patch| {
        patch
            .as_nbt()
            .get(&CUSTOM_DATA.into())?
            .get(PUBLIC_VALUES)?
            .get(MODE_KEY)
            .cloned()
    }) else {
        return 0;
    };
    match mode {
        Tag::Int(mode) => usize::try_from(mode).unwrap_or(0),
        _ => 0,
    }
}

fn mode_count(tier: &Tier) -> usize {
    if tier.item == CARBONADO_MULTI_TOOL {
        6
    } else {
        4
    }
}

#[derive(Clone, Copy)]
struct ModeSetting {
    action: Option<Mode>,
    enabled: bool,
}

fn mode_settings(table: Option<&toml::Value>) -> [ModeSetting; 6] {
    std::array::from_fn(|index| {
        let key = index.to_string();
        let row = table.and_then(|table| table.get(key.as_str()));
        let action = if let Some(id) = row
            .and_then(|row| row.get("item"))
            .and_then(toml::Value::as_str)
        {
            MODES.into_iter().find(|mode| mode.item_id() == id)
        } else {
            Some(MODES[index])
        };
        let enabled = row
            .and_then(|row| row.get("enabled"))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        ModeSetting { action, enabled }
    })
}

fn next_index(index: usize, available: &[ModeSetting]) -> usize {
    (1..available.len())
        .map(|offset| (index + offset) % available.len())
        .find(|&next| available[next].enabled)
        .unwrap_or(index)
}

#[derive(Default)]
pub(crate) struct MultiTools {
    modes: Mutex<BTreeMap<&'static str, [ModeSetting; 6]>>,
}

impl MultiTools {
    pub(crate) fn configure(&self, section: Option<&toml::Value>) {
        let configured = TIERS
            .iter()
            .filter_map(|tier| {
                let id = tier.item.id();
                let table = section?.get(id)?.get("mode")?;
                Some((id, mode_settings(Some(table))))
            })
            .collect();
        *self.modes.lock().unwrap() = configured;
    }

    fn modes_for(&self, tier: &Tier) -> [ModeSetting; 6] {
        self.modes
            .lock()
            .unwrap()
            .get(tier.item.id())
            .copied()
            .unwrap_or_else(|| mode_settings(None))
    }
}

fn with_mode(stack: &ItemStack, index: usize, name: &str) -> Option<ItemStack> {
    let mut components = stack.components()?.as_nbt().clone();
    let Tag::Compound(data) = components.get_mut(&CUSTOM_DATA.into())? else {
        return None;
    };
    let Tag::Compound(values) = data.get_mut(&PUBLIC_VALUES.into())? else {
        return None;
    };
    values.insert(MODE_KEY.into(), Tag::Int(i32::try_from(index).ok()?));
    let replacement = colored_lore(&format!("&8⇨ &7Mode: {name}"));
    let mut lore = match components.remove(&LORE.into()) {
        Some(Tag::List(lore)) => lore,
        Some(_) => return None,
        None => Vec::new(),
    };
    let previous = lore.iter_mut().find(|line| {
        let mut plain = String::new();
        charging_bench::literal(line, &mut plain, 0).is_some() && plain.starts_with(MODE_LINE)
    });
    if let Some(previous) = previous {
        *previous = replacement;
    } else {
        lore.insert(lore.len().min(2), replacement);
    }
    components.remove(&"!minecraft:lore".into());
    components.insert(LORE.into(), Tag::List(lore));
    let mut result = stack.clone();
    result.set_components(components).ok()?;
    Some(result)
}

/// Both clicks use the actual selected hand. Crouching switches modes for
/// free; a normal click pays before invoking the selected Paper handler.
impl MultiTools {
    pub(crate) fn use_held(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        hand: Hand,
        clicked: &ItemStack,
    ) -> Option<(Mode, ItemStack)> {
        let tier = held_tool(host, clicked)?;
        if !research::can_use(host, player, tier.recipe.research) {
            return None;
        }
        let held = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        };
        let stack = held.stack(host).filter(|stack| stack == clicked)?;
        let index = mode_index(&stack);
        let index = if index < mode_count(tier) { index } else { 0 };
        let modes = self.modes_for(tier);
        let sneaking = player.entity().is_shift_key_down(host)?;
        let (replacement, result) = if sneaking {
            let next = next_index(index, &modes[..mode_count(tier)]);
            let selected = modes[next].action;
            (
                with_mode(&stack, next, selected.map_or("Unknown", Mode::name))?,
                selected,
            )
        } else {
            let action = modes[index].action?;
            (
                charging_bench::discharge(host, &stack, USE_COST)?,
                Some(action),
            )
        };
        let slot = match hand {
            Hand::Main => u32::try_from(player.selected_slot(host)?).ok()?,
            Hand::Off => 40,
        };
        host.transaction(|host, _| {
            if held.stack(host).as_ref() != Some(&stack) {
                return Err(());
            }
            player
                .set_inventory_slot(host, slot, Some(replacement.clone()))
                .map_err(|_| ())
        })
        .ok()?;
        if sneaking {
            let _ = player.tell(
                host,
                colored_label(&format!(
                    "&bMulti Tool mode changed to: &9{}",
                    result.map_or("Unknown", Mode::name)
                )),
            );
            None
        } else {
            result.map(|action| (action, replacement))
        }
    }
}

pub(crate) fn refuse_shears(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = player.tell(
        host,
        colored_label("&cA Multi Tool cannot be used as shears!"),
    );
}

pub(crate) fn main_hand_has_tool(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    let hand = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    };
    hand.stack(host).is_some_and(|stack| is_tool(host, &stack))
}

type PanBreak = (u128, mod_api::LevelRef, [i32; 3]);

std::thread_local! {
    static PAN_BREAK: std::cell::Cell<Option<PanBreak>> = const { std::cell::Cell::new(None) };
}

/// The pan invokes the normal protection check. Only this tool's refusal
/// for ordinary mining should skip its own synthetic break.
pub(crate) struct PanUse(Option<PanBreak>);

impl PanUse {
    pub(crate) fn new(player: PlayerRef, level: mod_api::LevelRef, pos: [i32; 3]) -> Self {
        Self(PAN_BREAK.replace(Some((player.uuid(), level, pos))))
    }
}

impl Drop for PanUse {
    fn drop(&mut self) {
        PAN_BREAK.set(self.0);
    }
}

pub(crate) fn is_pan_break(player: PlayerRef, level: mod_api::LevelRef, pos: [i32; 3]) -> bool {
    PAN_BREAK.get() == Some((player.uuid(), level, pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, prelude::PlayersHost, testing::FakeHost};

    #[test]
    fn paper_tiers_keep_their_ingredients_capacitors_research_and_exact_modes() {
        let expected = [
            ("DURALUMIN_INGOT", "SMALL_CAPACITOR", 20.0),
            ("SOLDER_INGOT", "SMALL_CAPACITOR", 30.0),
            ("BILLON_INGOT", "SMALL_CAPACITOR", 40.0),
            ("STEEL_INGOT", "SMALL_CAPACITOR", 50.0),
            ("DAMASCUS_STEEL_INGOT", "SMALL_CAPACITOR", 60.0),
            ("REINFORCED_ALLOY_INGOT", "MEDIUM_CAPACITOR", 75.0),
            ("CARBONADO", "LARGE_CAPACITOR", 100.0),
        ];
        for (index, (tier, (ingot, capacitor, capacity))) in TIERS.iter().zip(expected).enumerate()
        {
            assert_eq!(
                tier.recipe.inputs.map(|kind| kind.map_or("", Kind::id)),
                [ingot, "", ingot, ingot, capacitor, ingot, "", ingot, ""]
            );
            assert_eq!(
                charge_spec(tier.item.id()),
                Some(("minecraft:shears", capacity))
            );
            assert_eq!(mode_count(tier), if index == 6 { 6 } else { 4 });
            assert_eq!(
                tier.recipe.research.id,
                if index == 6 {
                    "carbonado_tools"
                } else {
                    "multitools"
                }
            );
        }
        assert_eq!(MODES[3], Mode::Grapple);
    }

    #[test]
    fn disabled_modes_are_skipped_and_configured_ids_keep_their_saved_indices() {
        let settings: toml::Table = toml::from_str(
            r#"
            [0]
            item = "GRAPPLING_HOOK"
            [1]
            enabled = false
            [2]
            item = "MISSING_ADDON_ITEM"
        "#,
        )
        .unwrap();
        let modes = mode_settings(Some(&toml::Value::Table(settings)));
        assert_eq!(modes[0].action, Some(Mode::Grapple));
        assert_eq!(modes[2].action, None);
        assert_eq!(next_index(0, &modes[..4]), 2);
        assert_eq!(next_index(3, &modes[..4]), 0);
    }

    #[test]
    fn the_pan_permission_belongs_only_to_its_exact_player_and_block() {
        let mut host = FakeHost::new();
        host.join(1, 10);
        host.join(2, 11);
        let first = host.player(1).unwrap();
        let second = host.player(2).unwrap();
        let level = host.level.level();
        let source = [5, 7, 2];

        {
            let _pan = PanUse::new(first, level, source);
            assert!(is_pan_break(first, level, source));
            assert!(!is_pan_break(first, level, [5, 7, 3]));
            assert!(!is_pan_break(second, level, source));
        }
        assert!(!is_pan_break(first, level, source));
    }

    #[test]
    fn cycling_and_paying_preserves_the_paper_mode_lore_and_charge() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:shears").unwrap()])
            .unwrap();
        let stack = CARBONADO_MULTI_TOOL.stack(&host, 1).unwrap();
        assert_eq!(mode_index(&stack), 0);
        assert!(charging_bench::discharge(&host, &stack, USE_COST).is_none());

        let charged = charging_bench::recharge(&host, &stack, 0.6).unwrap();
        let grappling = with_mode(&charged, 3, Mode::Grapple.name()).unwrap();
        let paid = charging_bench::discharge(&host, &grappling, USE_COST).unwrap();
        assert_eq!(mode_index(&paid), 3);
        assert_eq!(paid.count(), 1);
        let Tag::List(lines) = paid
            .components()
            .unwrap()
            .as_nbt()
            .get(&LORE.into())
            .unwrap()
            .clone()
        else {
            panic!("missing charge and mode lore");
        };
        let mut lore = String::new();
        assert!(charging_bench::literal(&lines[2], &mut lore, 0).is_some());
        assert_eq!(lore, "⇨ Mode: Grappling Hook");
        let spent = charging_bench::discharge(&host, &paid, USE_COST).unwrap();
        assert!(charging_bench::discharge(&host, &spent, USE_COST).is_none());
        assert_eq!(
            mode_index(&with_mode(&spent, 5, Mode::NetherPan.name()).unwrap()),
            5
        );
    }
}
