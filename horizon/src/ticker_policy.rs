// SPDX-License-Identifier: GPL-3.0-only

use std::sync::atomic::{AtomicBool, Ordering};

use mod_api::{ItemStack, PlayerRef, ServerHost, Text};

use crate::{items::item_id, recipes::CraftingRecipe};

// Paper SlimefunItemSetup entries whose actual item handler is a BlockTicker.
// Generators, cables and most cargo nodes have no BlockTicker of their own.
const TICKING_ITEMS: &[&str] = &[
    "ANIMAL_GROWTH_ACCELERATOR",
    "ARMOR_AUTO_CRAFTER",
    "AUTO_ANVIL",
    "AUTO_ANVIL_2",
    "AUTO_BREEDER",
    "AUTO_BREWER",
    "AUTO_DISENCHANTER",
    "AUTO_DISENCHANTER_2",
    "AUTO_DRIER",
    "AUTO_ENCHANTER",
    "AUTO_ENCHANTER_2",
    "BOOK_BINDER",
    "CARBONADO_EDGED_FURNACE",
    "CARBON_PRESS",
    "CARBON_PRESS_2",
    "CARBON_PRESS_3",
    "CARGO_MANAGER",
    "CHARGING_BENCH",
    "CROP_GROWTH_ACCELERATOR",
    "CROP_GROWTH_ACCELERATOR_2",
    "ELECTRIC_DUST_WASHER",
    "ELECTRIC_DUST_WASHER_2",
    "ELECTRIC_DUST_WASHER_3",
    "ELECTRIC_FURNACE",
    "ELECTRIC_FURNACE_2",
    "ELECTRIC_FURNACE_3",
    "ELECTRIC_GOLD_PAN",
    "ELECTRIC_GOLD_PAN_2",
    "ELECTRIC_GOLD_PAN_3",
    "ELECTRIC_INGOT_FACTORY",
    "ELECTRIC_INGOT_FACTORY_2",
    "ELECTRIC_INGOT_FACTORY_3",
    "ELECTRIC_INGOT_PULVERIZER",
    "ELECTRIC_ORE_GRINDER",
    "ELECTRIC_ORE_GRINDER_2",
    "ELECTRIC_ORE_GRINDER_3",
    "ELECTRIC_PRESS",
    "ELECTRIC_PRESS_2",
    "ELECTRIC_SMELTERY",
    "ELECTRIC_SMELTERY_2",
    "ELECTRIFIED_CRUCIBLE",
    "ELECTRIFIED_CRUCIBLE_2",
    "ELECTRIFIED_CRUCIBLE_3",
    "ENERGY_REGULATOR",
    "ENHANCED_AUTO_CRAFTER",
    "ENHANCED_FURNACE",
    "ENHANCED_FURNACE_10",
    "ENHANCED_FURNACE_11",
    "ENHANCED_FURNACE_2",
    "ENHANCED_FURNACE_3",
    "ENHANCED_FURNACE_4",
    "ENHANCED_FURNACE_5",
    "ENHANCED_FURNACE_6",
    "ENHANCED_FURNACE_7",
    "ENHANCED_FURNACE_8",
    "ENHANCED_FURNACE_9",
    "FLUID_PUMP",
    "FOOD_COMPOSTER",
    "FOOD_COMPOSTER_2",
    "FOOD_FABRICATOR",
    "FOOD_FABRICATOR_2",
    "FREEZER",
    "FREEZER_2",
    "FREEZER_3",
    "GEO_MINER",
    "GPS_TRANSMITTER",
    "GPS_TRANSMITTER_2",
    "GPS_TRANSMITTER_3",
    "GPS_TRANSMITTER_4",
    "HEATED_PRESSURE_CHAMBER",
    "HEATED_PRESSURE_CHAMBER_2",
    "INFUSED_HOPPER",
    "IRON_GOLEM_ASSEMBLER",
    "OIL_PUMP",
    "PRODUCE_COLLECTOR",
    "PROGRAMMABLE_ANDROID",
    "PROGRAMMABLE_ANDROID_2",
    "PROGRAMMABLE_ANDROID_2_BUTCHER",
    "PROGRAMMABLE_ANDROID_2_FARMER",
    "PROGRAMMABLE_ANDROID_2_FISHERMAN",
    "PROGRAMMABLE_ANDROID_3",
    "PROGRAMMABLE_ANDROID_3_BUTCHER",
    "PROGRAMMABLE_ANDROID_3_FISHERMAN",
    "PROGRAMMABLE_ANDROID_BUTCHER",
    "PROGRAMMABLE_ANDROID_FARMER",
    "PROGRAMMABLE_ANDROID_FISHERMAN",
    "PROGRAMMABLE_ANDROID_MINER",
    "PROGRAMMABLE_ANDROID_WOODCUTTER",
    "RAINBOW_CLAY",
    "RAINBOW_CLAY_HALLOWEEN",
    "RAINBOW_CLAY_VALENTINE",
    "RAINBOW_CLAY_XMAS",
    "RAINBOW_CONCRETE",
    "RAINBOW_CONCRETE_HALLOWEEN",
    "RAINBOW_CONCRETE_VALENTINE",
    "RAINBOW_CONCRETE_XMAS",
    "RAINBOW_GLASS",
    "RAINBOW_GLASS_HALLOWEEN",
    "RAINBOW_GLASS_PANE",
    "RAINBOW_GLASS_PANE_HALLOWEEN",
    "RAINBOW_GLASS_PANE_VALENTINE",
    "RAINBOW_GLASS_PANE_XMAS",
    "RAINBOW_GLASS_VALENTINE",
    "RAINBOW_GLASS_XMAS",
    "RAINBOW_GLAZED_TERRACOTTA",
    "RAINBOW_GLAZED_TERRACOTTA_HALLOWEEN",
    "RAINBOW_GLAZED_TERRACOTTA_VALENTINE",
    "RAINBOW_GLAZED_TERRACOTTA_XMAS",
    "RAINBOW_WOOL",
    "RAINBOW_WOOL_HALLOWEEN",
    "RAINBOW_WOOL_VALENTINE",
    "RAINBOW_WOOL_XMAS",
    "REFINERY",
    "REINFORCED_FURNACE",
    "TRASH_CAN_BLOCK",
    "TREE_GROWTH_ACCELERATOR",
    "VANILLA_AUTO_CRAFTER",
    "WITHER_ASSEMBLER",
    "XP_COLLECTOR",
];

static ENABLED: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tickers {
    enabled: bool,
}

impl Default for Tickers {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Tickers {
    pub(crate) fn current() -> Self {
        Self {
            enabled: ENABLED.load(Ordering::Relaxed),
        }
    }

    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        Self { enabled: false }
    }

    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }

    pub(crate) fn allows(self, id: &str) -> bool {
        self.enabled || TICKING_ITEMS.binary_search(&id).is_err()
    }

    pub(crate) fn blocks_stack(self, stack: &ItemStack) -> bool {
        !self.enabled && item_id(stack).is_some_and(|id| !self.allows(&id))
    }

    pub(crate) fn allows_recipe(self, recipe: &CraftingRecipe) -> bool {
        self.allows(recipe.output.kind.id())
    }
}

fn load_into(current: &AtomicBool, fresh: bool, urid: Option<&toml::Value>) {
    if fresh {
        current.store(
            urid.and_then(|table| table.get("enable-tickers"))
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
            Ordering::Relaxed,
        );
    }
}

pub(crate) fn load(fresh: bool, urid: Option<&toml::Value>) {
    load_into(&ENABLED, fresh, urid);
}

pub(crate) fn tell_unavailable(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = player.tell(host, Text::literal("This item has been disabled."));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_availability_is_reloaded_only_on_a_fresh_start() {
        let slot = AtomicBool::new(false);
        let setting = |line: &str| toml::Value::Table(line.parse::<toml::Table>().unwrap());
        let off = setting("enable-tickers = false");
        let on = setting("enable-tickers = true");
        let malformed = setting("enable-tickers = 0");
        load_into(&slot, true, None);
        assert!(slot.load(Ordering::Relaxed));
        load_into(&slot, false, Some(&off));
        assert!(slot.load(Ordering::Relaxed));
        load_into(&slot, true, Some(&off));
        assert!(!slot.load(Ordering::Relaxed));
        load_into(&slot, false, Some(&on));
        assert!(!slot.load(Ordering::Relaxed));
        load_into(&slot, true, Some(&on));
        assert!(slot.load(Ordering::Relaxed));
        load_into(&slot, true, Some(&off));
        load_into(&slot, true, Some(&malformed));
        assert!(slot.load(Ordering::Relaxed));
    }

    #[test]
    fn only_stock_cargo_manager_and_other_real_tickers_are_unavailable() {
        assert_eq!(TICKING_ITEMS.len(), 119);
        assert!(TICKING_ITEMS.windows(2).all(|pair| pair[0] < pair[1]));
        let off = Tickers::disabled();
        for id in [
            "CARGO_MANAGER",
            "XP_COLLECTOR",
            "TRASH_CAN_BLOCK",
            "ENHANCED_AUTO_CRAFTER",
            "ELECTRIC_FURNACE_3",
            "GEO_MINER",
            "CARBONADO_EDGED_FURNACE",
            "RAINBOW_WOOL_XMAS",
        ] {
            assert!(!off.allows(id), "{id}");
            assert!(Tickers::default().allows(id), "{id}");
        }
        for id in [
            "CARGO_NODE",
            "CARGO_NODE_INPUT",
            "CARGO_NODE_OUTPUT",
            "CARGO_NODE_OUTPUT_ADVANCED",
            "REACTOR_ACCESS_PORT",
            "SOLAR_GENERATOR_4",
            "NUCLEAR_REACTOR",
            "ENERGY_CONNECTOR",
            "BLOCK_PLACER",
            "INDUSTRIAL_MINER",
            "PORTABLE_DUSTBIN",
        ] {
            assert!(off.allows(id), "{id}");
        }
    }
}
