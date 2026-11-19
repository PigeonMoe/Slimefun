// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Arc, Mutex};

use mod_api::{
    BlockDropsEvent, Command, CommandArgumentValue, CommandSource, Config, ConfigFile, ConfigKind,
    ConfigLoadingEvent, ConfigReloadingEvent, Flow, GameMode, Hand, Hooks, HostSide,
    InteractionResult, ItemKey, LevelHost, LevelTickPostEvent, LevelTickPreEvent,
    PlayerLoggedInEvent, RegisterCommandsEvent, RegisterConfigsEvent, RightClickBlockEvent,
    RightClickItemEvent, RunModCommandEvent, ScheduledTaskEvent, ServerHost, ServerStoppingEvent,
    Text, TriState,
};

mod advanced_processing;
mod ancient_altar;
mod android_butchery;
mod android_fishing;
mod android_items;
mod android_library;
mod android_machine;
mod android_mining;
mod android_program;
mod android_ui;
mod android_woodcutting;
mod armor_forge;
mod auto_anvil;
mod auto_brewer;
mod auto_crafter;
mod backpacks;
mod bee_wings;
mod block_placer;
mod book_binder;
mod circuits;
mod composter;
mod compressor;
mod crucible;
mod electric_processing;
mod elevator;
mod elemental_staves;
mod enchantment_machines;
mod enhanced_furnaces;
mod energy_gadgets;
mod factory;
mod food_machines;
mod fuel_generators;
mod geo_deposits;
mod geology;
mod gold_pan;
mod gps_controls;
mod gps_transport;
mod gps_waypoints;
mod guide;
mod hand_machine;
mod heated_pressure;
mod hologram_projector;
mod ignition_chamber;
mod industrial_miner;
mod item_clicks;
mod infused;
mod items;
mod jet_boots;
mod jetpack;
mod juice;
mod lumber_axe;
mod machine;
mod magic_eye;
mod medical;
mod network_recipes;
mod network_runtime;
mod networks;
mod output_chest;
mod portable_crafter;
mod radiation;
mod rainbow;
mod random;
mod recipes;
mod research;
mod smelters_pickaxe;
mod snacks;
mod sneak_equipment;
mod spawner_repair;
mod soulbound_gear;
mod soulbound_rune;
mod stomper;
mod storage;
mod tape_measure;
mod thermal_machines;
mod universal_storage;

use compressor::DelayedCrafts;
use machine::{Machine, Settings};
use random::Rolls;
use recipes::{
    ALLOY_RECIPES, COMPRESSING, CRUSHING, DOUBLING, GRINDING, JUICING, PRESSING, WASHER_DUSTS,
    WASHING, crafting, researches, smelting,
};

struct Slimefun {
    delayed_crafts: DelayedCrafts,
    radiation: radiation::Radiation,
    soulbound_rituals: soulbound_rune::Rituals,
    block_placers: block_placer::Placers,
    composters: composter::Composters,
    crucibles: crucible::Crucibles,
    industrial_miners: industrial_miner::Miners,
    solar_helmet: energy_gadgets::SolarHelmet,
    magnets: infused::Magnets,
    settings: Mutex<Settings>,
    item_clicks: item_clicks::ItemClicks,
    snacks: snacks::Snacks,
    jet_boots: sneak_equipment::Task<jet_boots::JetBoots>,
    jetpacks: sneak_equipment::Task<jetpack::Flight>,
    smelters_pickaxes: smelters_pickaxe::SmeltersPickaxes,
    spawners: spawner_repair::Spawners,
    rolls: Rolls,
    chest: Mutex<Option<ItemKey>>,
    networks: Arc<Mutex<network_runtime::Networks>>,
    gps: gps_controls::Controls,
    guide: guide::Guide,
    androids: android_ui::Menus,
    butcher_androids: android_butchery::Butchers,
    holograms: hologram_projector::Editors,
    transport: gps_transport::Transport,
    elevators: elevator::Elevators,
    backpacks: backpacks::BackpackMenus,
    bee_wings: bee_wings::BeeWings,
    universal: universal_storage::UniversalStorage,
}

impl Default for Slimefun {
    fn default() -> Self {
        Self {
            delayed_crafts: DelayedCrafts::default(),
            radiation: radiation::Radiation::default(),
            soulbound_rituals: soulbound_rune::Rituals::default(),
            block_placers: block_placer::Placers::default(),
            composters: composter::Composters::default(),
            crucibles: crucible::Crucibles::default(),
            industrial_miners: industrial_miner::Miners::default(),
            solar_helmet: energy_gadgets::SolarHelmet::default(),
            magnets: infused::Magnets::default(),
            settings: Mutex::new(Settings::default()),
            item_clicks: item_clicks::ItemClicks::default(),
            snacks: snacks::Snacks::default(),
            jet_boots: sneak_equipment::Task::default(),
            jetpacks: sneak_equipment::Task::default(),
            smelters_pickaxes: smelters_pickaxe::SmeltersPickaxes::default(),
            spawners: spawner_repair::Spawners::default(),
            rolls: Rolls::default(),
            chest: Mutex::new(None),
            networks: Arc::default(),
            gps: gps_controls::Controls::default(),
            guide: guide::Guide::default(),
            androids: android_ui::Menus::default(),
            butcher_androids: android_butchery::Butchers::default(),
            holograms: hologram_projector::Editors::default(),
            transport: gps_transport::Transport::default(),
            elevators: elevator::Elevators::default(),
            backpacks: backpacks::BackpackMenus::default(),
            bee_wings: bee_wings::BeeWings::default(),
            universal: universal_storage::UniversalStorage::default(),
        }
    }
}

impl Slimefun {
    fn read_config(&self, config: &ConfigFile, fresh: bool) {
        if config.mod_id != "slimefun" || config.file != "slimefun-server.toml" {
            return;
        }
        let mut settings = self.settings.lock().unwrap();
        if fresh {
            *settings = Settings::default();
            enchantment_machines::configure(&toml::Table::new());
            self.radiation.configure(None);
            self.block_placers.configure(None);
            output_chest::set_enabled(true);
            self.crucibles.configure(false);
            self.industrial_miners.configure(None);
            self.solar_helmet.configure(None);
            ancient_altar::configure(None);
        }
        let Ok(table) = config.text.parse::<toml::Table>() else {
            return;
        };
        self.radiation.configure(table.get("radiation"));
        self.block_placers.configure(
            table
                .get("block-placer")
                .and_then(|section| section.get("unplaceable-blocks"))
                .and_then(toml::Value::as_array)
                .and_then(|entries| {
                    entries
                        .iter()
                        .map(|entry| entry.as_str().map(str::to_owned))
                        .collect()
                }),
        );
        self.transport.configure(&table);
        self.industrial_miners.configure(Some(&table));
        self.solar_helmet.configure(table.get("solar-helmet"));
        settings.book_binder = book_binder::Rules::read_config(&table);
        settings.wind_power = elemental_staves::wind_power(table.get("wind-staff"));
        enchantment_machines::configure(&table);
        settings.infused = infused::Settings::read_config(&table);
        if let Some(value) = table
            .get("miner-android")
            .and_then(|section| section.get("reduced-block-updates"))
            .and_then(toml::Value::as_bool)
        {
            settings.android_reduced_block_updates = value;
        }
        if let Some(value) = table
            .get("reinforced-spawner")
            .and_then(|section| section.get("allow-spawn-eggs"))
            .and_then(toml::Value::as_bool)
        {
            self.spawners.configure(value);
        }
        ancient_altar::configure(
            table
                .get("ancient-altar")
                .and_then(|section| section.get("step-delay"))
                .and_then(toml::Value::as_integer)
                .and_then(|delay| i32::try_from(delay).ok())
                .filter(|&delay| delay >= 0),
        );
        if let Some(range) = table
            .get("produce-collector")
            .and_then(|section| section.get("range"))
            .and_then(toml::Value::as_integer)
            .filter(|range| (1..=32).contains(range))
        {
            settings.produce_collector_range = range as u8;
        }
        if let Some(value) = table
            .get("crucible-allow-water-in-nether")
            .and_then(toml::Value::as_bool)
        {
            self.crucibles.configure(value);
        }
        if let Some(value) = table.get("double-ores").and_then(toml::Value::as_bool) {
            settings.double_ores = value;
        }
        if let Some(value) = table
            .get("legacy-ore-washer")
            .and_then(toml::Value::as_bool)
        {
            settings.legacy_ore_washer = value;
        }
        if let Some(value) = table
            .get("legacy-ore-grinder")
            .and_then(toml::Value::as_bool)
        {
            settings.legacy_ore_grinder = value;
        }
        if let Some(value) = table
            .get("legacy-dust-washer")
            .and_then(toml::Value::as_bool)
        {
            settings.legacy_dust_washer = value;
        }
        if let Some(values) = table
            .get("electric-gold-pan-override-output-limit")
            .and_then(toml::Value::as_array)
            && let [first, second, third] = values.as_slice()
            && let (Some(first), Some(second), Some(third)) =
                (first.as_bool(), second.as_bool(), third.as_bool())
        {
            settings.gold_pan_override_output_limit = [first, second, third];
        }
        if let Some(values) = table
            .get("freezer-use-vanilla-ratios")
            .and_then(toml::Value::as_array)
            && let [first, second, third] = values.as_slice()
            && let (Some(first), Some(second), Some(third)) =
                (first.as_bool(), second.as_bool(), third.as_bool())
        {
            settings.freezer_use_vanilla_ratios = [first, second, third];
        }
        if let Some(value) = table
            .get("fire-breaking-chance")
            .and_then(toml::Value::as_integer)
            && let Ok(value @ 0..=100) = u32::try_from(value)
        {
            settings.fire_breaking_chance = value;
        }
        if let Some(value) = table
            .get("output-chest-enabled")
            .and_then(toml::Value::as_bool)
        {
            output_chest::set_enabled(value);
        }
        if let Some(value) = table.get("drop-from-golems").and_then(toml::Value::as_bool) {
            settings.drop_from_golems = value;
        }
        if let Some(value) = table
            .get("golem-drop-chance")
            .and_then(toml::Value::as_integer)
            && let Ok(value @ 0..=100) = u32::try_from(value)
        {
            settings.golem_drop_chance = value;
        }
        if let Some(delay) = table
            .get("URID")
            .and_then(|section| section.get("custom-ticker-delay"))
            .and_then(toml::Value::as_integer)
            .and_then(|value| u32::try_from(value.max(1)).ok())
        {
            settings.block_ticker_delay = delay;
        }
        if let Some(network) = table.get("networks").and_then(toml::Value::as_table) {
            if let Some(budget) = network
                .get("max-size")
                .and_then(toml::Value::as_integer)
                .and_then(|value| u32::try_from(value.max(1)).ok())
            {
                settings.network_discovery_budget = budget;
            }
            if let Some(value) = network
                .get("enable-visualizer")
                .and_then(toml::Value::as_bool)
            {
                settings.cargo_visualizer = value;
            }
            if let Some(value) = network
                .get("cargo-ticker-delay")
                .and_then(toml::Value::as_integer)
                && let Ok(delay) = u32::try_from(value.max(0))
            {
                settings.cargo_tick_delay = delay;
            }
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Slimefun {
    fn register(&self, registrar: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let dispenser = registrar.item_key("minecraft:dispenser");
        registrar.item_part(dispenser, block_placer::Placement);
        let chest = registrar.item_key("minecraft:chest");
        registrar.item_part(chest, output_chest::ChestPlacement);
        *self.chest.lock().unwrap() = Some(chest);
        for id in [
            "minecraft:player_head",
            "minecraft:daylight_detector",
            "minecraft:furnace",
            "minecraft:hopper",
            "minecraft:smoker",
            "minecraft:enchanting_table",
            "minecraft:dropper",
            "minecraft:dispenser",
        ]
        .into_iter()
        .chain(network_runtime::PLAIN_CARRIERS.iter().copied())
        {
            let key = registrar.item_key(id);
            registrar.item_part(
                key,
                network_runtime::NetworkPlacement(self.networks.clone()),
            );
        }
        let skull = registrar.item_key("minecraft:player_head");
        registrar.item_part(skull, food_machines::Placement);
        network_runtime::register_cargo(registrar);
        gps_transport::register(registrar);
        android_machine::register(registrar);
        android_mining::register(registrar);
        crucible::register(registrar);
        industrial_miner::register(registrar);
        factory::register(registrar);
        backpacks::register(registrar);
        ancient_altar::register(registrar);
        self.spawners.register(registrar);
        rainbow::register(registrar);
        hologram_projector::register(registrar);
        auto_crafter::register(registrar);
        self.universal.register(registrar);
    }

    fn register_configs(&self, _: HostSide<'_>, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(Config::new(
            "slimefun",
            ConfigKind::Server,
            &format!("{}{}",
            "double-ores = true\ncrucible-allow-water-in-nether = false\nlegacy-ore-washer = false\nlegacy-dust-washer = false\nlegacy-ore-grinder = true\nelectric-gold-pan-override-output-limit = [false, false, false]\nfreezer-use-vanilla-ratios = [false, false, false]\nfire-breaking-chance = 34\noutput-chest-enabled = true\ndrop-from-golems = true\ngolem-drop-chance = 75\n\n[URID]\ncustom-ticker-delay = 10\n\n[networks]\nmax-size = 200\nenable-visualizer = true\ncargo-ticker-delay = 0\n\n[wind-staff]\npower = 4\n\n[book-binder]\nbypass-vanilla-max-level = false\nhas-custom-max-level = false\ncustom-max-level = 15\n\n[portable-teleporter]\nteleportation-cost = 10\n\n[produce-collector]\nrange = 2\n\n[reinforced-spawner]\nallow-spawn-eggs = true\n\n[ancient-altar]\nstep-delay = 8\n\n[miner-android]\nreduced-block-updates = true\n\n[industrial-miner]\ncan-mine-ancient-debris = false\ncan-mine-deepslate-ores = true\n\n[advanced-industrial-miner]\ncan-mine-ancient-debris = false\ncan-mine-deepslate-ores = true\n",
                enchantment_machines::default_config()),
        ));
    }

    fn config_loading(&self, _: HostSide<'_>, event: &mut ConfigLoadingEvent<'_>) {
        self.read_config(event.config, true);
    }

    fn config_reloading(&self, _: HostSide<'_>, event: &mut ConfigReloadingEvent<'_>) {
        self.read_config(event.config, false);
    }

    #[listener(priority = -100)]
    fn break_block(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::BreakBlockEvent<'_>,
    ) -> Flow {
        let Some(player) = host.players().acting_player() else {
            return Flow::Continue;
        };
        let Some(level) = player.entity().level(host) else {
            return Flow::Continue;
        };
        if let Some(android) = android_machine::data(host, level, event.pos)
            && android.owner != player.uuid()
            && !player
                .permission_level(host)
                .is_some_and(|level| level >= 2)
        {
            return Flow::Cancel;
        }
        if lumber_axe::allow_break(host, player) == Flow::Cancel
            || self.spawners.prepare(host, player, level, event) == Flow::Cancel
        {
            return Flow::Cancel;
        }
        self.smelters_pickaxes.prepare(host, player, level, event, &self.rolls)
    }

    fn player_destroyed_block(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerDestroyedBlockEvent,
    ) {
        self.spawners.destroyed(host, event);
        self.smelters_pickaxes.destroyed(host, event, &self.rolls);
        lumber_axe::destroyed(host, event);
    }

    fn block_drops(&self, level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
        block_placer::restore_drop(level, event);
        if let Some(chest) = *self.chest.lock().unwrap() {
            output_chest::restore_drop(level, event, chest);
        }
        network_runtime::restore_drop(level, event);
        factory::restore_drops(level, event);
        android_machine::drop_items(level, event);
        food_machines::restore_drop(level, event);
        ancient_altar::restore_drops(level, event);
        rainbow::placed::drop_item(level, event);
        hologram_projector::restore_drop(level, event);
        self.smelters_pickaxes.drop_items(event);
        self.spawners.suppress_base_drops(level, event);
        Flow::Continue
    }

    fn block_changed(&self, level: &dyn LevelHost, event: &mut mod_api::BlockChangedEvent) {
        network_runtime::observe_plain_change(level, event);
        self.crucibles.anchor_changed(level, event);
        hologram_projector::observe_change(level, event);
        rainbow::placed::changed(level, event);
        network_runtime::observe_gps_change(&self.networks, level, event);
    }

    #[listener(priority = -100)]
    fn entity_interact(
        &self,
        host: HostSide<'_>,
        event: &mut mod_api::EntityInteractEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        (matches!(event.kind, "minecraft:cow" | "minecraft:mooshroom")
            && event.stack.is_some_and(|held| jet_boots::STEEL_THRUSTER.matches(host, held)))
            .then_some(InteractionResult::Consume)
    }

    fn entity_place(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::EntityPlaceEvent,
    ) -> Flow {
        let Some(player) = host.players().acting_player() else {
            return Flow::Continue;
        };
        let Some(level) = player.entity().level(host) else {
            return Flow::Continue;
        };
        let denied = match &event.placement {
            mod_api::Placement::Multi(blocks) => blocks
                .iter()
                .any(|block| ancient_altar::denies_placement(host, level, block.pos)),
            _ => ancient_altar::denies_placement(host, level, event.pos),
        };
        if denied {
            let _ = player.tell(
                host,
                Text::literal("You cannot place blocks on an Ancient Pedestal."),
            );
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn dispenser_selected(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::DispenserSelectedEvent<'_>,
    ) -> Flow {
        self.block_placers.selected(host, event)
    }

    fn block_dispense_pre(
        &self,
        host: &dyn mod_api::Host,
        event: &mut mod_api::BlockDispensePreEvent<'_>,
    ) -> Flow {
        if host.level(event.dimension).is_some_and(|level| {
            ancient_altar::is_pedestal(host, level, event.pos)
                || network_runtime::saved_android_interface(host, level, event.pos)
        }) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn inventory_move_item(
        &self,
        host: &dyn mod_api::Host,
        event: &mut mod_api::InventoryMoveItemEvent<'_>,
    ) -> Flow {
        if host.level(host.dimension()).is_some_and(|level| {
            [event.source, event.destination]
                .into_iter()
                .any(|pos| ancient_altar::is_pedestal(host, level, pos))
        }) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn piston_pre(&self, level: &dyn LevelHost, event: &mut mod_api::PistonPreEvent<'_>) -> Flow {
        if network_runtime::protect_piston(level, event) || hologram_projector::piston_protected(level, event) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn living_drops(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::LivingDropsEvent<'_>,
    ) -> Flow {
        let settings = *self.settings.lock().unwrap();
        circuits::add_golem_drop(host, event, settings, &self.rolls);
        Flow::Continue
    }

    fn living_destroy_block(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::LivingDestroyBlockEvent<'_>,
    ) -> Flow {
        if event.kind == "minecraft:wither"
            && let Some(level) = host.level(host.dimension())
            && network_runtime::blast_resistant_at(host, level, event.pos, true)
        {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn explosion_detonate(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::ExplosionDetonateEvent<'_>,
    ) {
        if let Some(level) = host.level(event.dimension) {
            event
                .blocks
                .retain(|pos| !network_runtime::blast_resistant_at(host, level, *pos, false));
        }
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            Command::new("slimefun"),
            Command::new("sf"),
            Command::new("slimefun guide"),
            Command::new("sf guide"),
            Command::new("slimefun open_guide"),
            Command::new("sf open_guide"),
            Command::new("slimefun status"),
            Command::new("sf status"),
            Command::new("slimefun recipe <item:text>"),
            Command::new("sf recipe <item:text>"),
            Command::new("slimefun research <key:word>"),
            Command::new("sf research <key:word>"),
            Command::new("slimefun restore-backpack"),
            Command::new("sf restore-backpack"),
            Command::op("slimefun storage", 2),
            Command::op("sf storage", 2),
            Command::op("slimefun storage restore-universal <preset:text>", 2),
            Command::op("sf storage restore-universal <preset:text>", 2),
            Command::op("slimefun storage open-universal <preset:text>", 2),
            Command::op("sf storage open-universal <preset:text>", 2),
            Command::op(
                "slimefun storage inspect <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "sf storage inspect <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "slimefun storage restore-output <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "sf storage restore-output <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "slimefun storage restore-network <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "sf storage restore-network <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "slimefun storage restore-inventory <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "sf storage restore-inventory <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "slimefun storage restore-chunk <dimension:dimension> <pos:block_pos> [offset:integer]",
                2,
            ),
            Command::op(
                "sf storage restore-chunk <dimension:dimension> <pos:block_pos> [offset:integer]",
                2,
            ),
            Command::op(
                "slimefun storage restore-geo <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "sf storage restore-geo <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "slimefun storage restore-oil <dimension:dimension> <pos:block_pos>",
                2,
            ),
            Command::op(
                "sf storage restore-oil <dimension:dimension> <pos:block_pos>",
                2,
            ),

        ]);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let mut path = event.command.literals.iter().map(String::as_str);
        if !matches!(path.next(), Some("slimefun" | "sf")) {
            return Flow::Continue;
        }
        let lines = match path.next() {
            Some(action @ ("guide" | "open_guide")) => {
                if let CommandSource::Player { player, .. } = *event.source {
                    if action == "guide" {
                        guide::give(host, player);
                    } else {
                        self.guide.open(host, player);
                    }
                } else {
                    let _ = host.commands().feedback(Text::literal("Only players can use this command."));
                }
                return Flow::Cancel;
            }
            Some("storage") => storage::command(host, event, path.next(), &self.networks, &self.universal),
            Some("restore-backpack") => {
                let message = match *event.source {
                    CommandSource::Player { player, .. } => match backpacks::restore_held(
                        host,
                        &backpacks::GlobalBackpacks,
                        player,
                        Hand::Main,
                        event.source.permission(),
                    ) {
                        Ok(backpacks::Restored::Installed) => "Restored the held Paper backpack.".into(),
                        Ok(backpacks::Restored::AlreadyInstalled) => "This Paper backpack is already restored.".into(),
                        Err(error) => error,
                    },
                    _ => "Hold the Paper backpack in your main hand and run this as a player.".into(),
                };
                vec![message]
            }
            Some("status") => vec![format!(
                "Slimefun Horizon development | Grind Stone {} | Enhanced Crafting Table {} | Ore Crusher {} | Compressor {} | Pressure Chamber {} | Juicer {} | Ore Washer {} dusts and {} recipes | Smeltery {} recipes | Research {}",
                GRINDING.len(),
                crafting().count(),
                CRUSHING.len() + DOUBLING.len(),
                COMPRESSING.len(),
                PRESSING.len(),
                JUICING.len(),
                WASHER_DUSTS.len(),
                WASHING.len(),
                smelting().count() + ALLOY_RECIPES.len(),
                researches().count()
            )],
            Some("recipe") => match event.command.argument("item") {
                Some(CommandArgumentValue::String(item)) => describe_recipe(item, self.settings.lock().unwrap().double_ores),
                _ => return Flow::Continue,
            },
            Some("research") => {
                let Some(CommandArgumentValue::String(id)) = event.command.argument("key") else {
                    return Flow::Continue;
                };
                let Some(research) = researches().find(|research| research.id.eq_ignore_ascii_case(id)) else {
                    let ids = researches().map(|research| research.id).collect::<Vec<_>>().join(", ");
                    let _ = host.commands().feedback(Text::literal(format!("Available research includes {ids}.")));
                    return Flow::Cancel;
                };
                match *event.source {
                    CommandSource::Player { player, .. } => vec![research::unlock(host, player, research)],
                    _ => vec!["A player must unlock their own research.".into()],
                }
            }
            None => vec![
                "Slimefun Horizon development".into(),
                "Build a Grind Stone by putting a wooden fence above a dispenser. Load the dispenser and right-click the fence.".into(),
                "Build an Enhanced Crafting Table by putting a crafting table on a dispenser. Arrange the recipe in its 3×3 grid and right-click the table.".into(),
                "An Ore Crusher uses a nether brick fence over a dispenser, with iron bars on opposite sides of the dispenser. A Compressor uses pistons instead of bars.".into(),
                "For the Pressure Chamber, put glass above a cauldron and a dispenser above the glass. Flank the cauldron and glass with pistons and the dispenser with smooth stone slabs on the same axis. Right-click the cauldron.".into(),
                "Use a Gold Pan on gravel and a Nether Gold Pan on soul sand or soul soil.".into(),
                "For Automated Panning, put a wooden trapdoor above a cauldron and right-click it holding gravel, soul sand or soul soil.".into(),
                "For the Table Saw, put a stonecutter on an iron block and flank the stonecutter with smooth stone slabs. Right-click it holding a log or planks.".into(),
                "For the Juicer, place a dispenser below a nether brick fence and glass above the fence. Put fruit in the dispenser and right-click the fence.".into(),
                "Craft an Electric Furnace and connect it to a powered Energy Regulator. Open the furnace to load vanilla smelting ingredients.".into(),
                "The Electric Ore Grinder uses Grind Stone and Ore Crusher recipes. The Electric Ingot Pulverizer reverses single-dust smeltery recipes.".into(),
                "Power an Electric Gold Pan, Electric Dust Washer and Electric Ingot Factory to turn gravel into sifted ore, dust and ingots. The Electric Press compacts materials and parts.".into(),
                "Place and power your GPS Transmitters to reach 600 complexity. Use a GPS Geo-Scanner and an Oil Pump in the same chunk to extract oil.".into(),
                "Use a placed Slimefun Crucible while holding stone or leaves to fill the block above with lava or water.".into(),
                "The Electric Smeltery processes multi-ingredient recipes. The Electrified Crucible fills empty buckets with water or lava, and the Refinery turns buckets of oil into fuel.".into(),
                "The Heated Pressure Chamber uses two input slots to refine uranium, blistering ingots, and fuel ingredients. Use /sf recipe to find its inputs.".into(),
                "Wash Sifted Ore by placing a dispenser on a wooden fence with an empty cauldron below it and right-clicking the fence.".into(),
                "For a Makeshift Smeltery use a wooden fence above a dispenser, bricks on opposite sides of the dispenser, and fire below it. Use a nether brick fence and nether bricks for the regular Smeltery.".into(),
                "An Electric Food Fabricator cans crops using Tin Cans. A Food Composter turns the canned food into fertilizer.".into(),
                "The Charging Bench recharges individual Slimefun gadgets. Cargo can collect them from the output once they are fully charged.".into(),
                "Load the Auto Brewer with a potion and one brewing ingredient. It accepts regular, splash and lingering bottles.".into(),
                "Supply a powered Produce Collector with buckets near adult cows or goats, or bowls near adult mooshrooms.".into(),
                "Use /sf recipe <item> to read a supported recipe, /sf research <key> to unlock research, and /sf status to view coverage.".into(),
            ],
            _ => return Flow::Continue,
        };
        for line in lines {
            let _ = host.commands().feedback(Text::literal(line));
        }
        Flow::Cancel
    }

    // Region/authorization hooks that veto the player's click run first.
    #[listener(priority = -100)]
    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        if *event.use_block == TriState::False {
            return Flow::Continue;
        }
        let Some(player) = host.player(event.click.player) else {
            return Flow::Continue;
        };
        if *event.use_item != TriState::False && let Some(held) = event.click.stack {
            if self.guide.right_click(host, player, held) {
                *event.cancellation_result = InteractionResult::Consume;
                return Flow::Cancel;
            }
            if jet_boots::STEEL_THRUSTER.matches(host, held)
                || jetpack::CHAIN.matches(host, held)
                || player.entity().level(host).is_some_and(|level| {
                    self.spawners.prevents_egg_use(host, level, event.click.pos, held)
                })
            {
                *event.cancellation_result = InteractionResult::Fail;
                return Flow::Cancel;
            }
            if medical::use_supply(host, player, event.click.hand, held)
                || magic_eye::use_held(host, player, held)
            {
                self.item_clicks
                    .record_block_use(host, event.click.player, event.click.hand, held);
                *event.cancellation_result = InteractionResult::Consume;
                return Flow::Cancel;
            }
            if snacks::use_sugar(host, player, event.click.hand, held, Some(event.click.pos))
                || elemental_staves::use_staff(host, player, held, || {
                    self.settings.lock().unwrap().wind_power
                })
            {
                self.item_clicks
                    .record_block_use(host, event.click.player, event.click.hand, held);
            }
        }
        if !matches!(
            player.game_mode(host),
            Some(GameMode::Survival | GameMode::Creative)
        ) {
            return Flow::Continue;
        }
        if *event.use_item != TriState::False
            && let Some(held) = event.click.stack
            && lumber_axe::strip(host, player, held, event.click.pos) == Flow::Cancel
        {
            *event.cancellation_result = InteractionResult::Fail;
            return Flow::Cancel;
        }
        if *event.use_item != TriState::False
            && let Some(held) = event.click.stack
            && let Some(level) = player.entity().level(host)
            && (energy_gadgets::measure(host, player, level, event.click.pos, held)
                || tape_measure::use_at(host, player, level, event.click.hand, event.click.pos))
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if event.click.hand == Hand::Main
            && let Some(level) = player.entity().level(host)
            && (auto_crafter::use_at(
                host,
                player,
                level,
                event.click.pos,
                *event.use_item != TriState::False,
            ) || ancient_altar::use_at(
                host,
                player,
                level,
                event.click.pos,
                *event.use_item != TriState::False,
            ))
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if *event.use_item != TriState::False
            && (event.click.stack.is_some_and(|stack| portable_crafter::use_held(host, player, stack))
                || self.backpacks.use_held(host, player, event.click.hand)
                || network_runtime::trash::portable(host, player, event.click.hand)
                || self
                    .transport
                    .use_portable(&self.networks, host, player, event.click.hand))
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        let Some(level) = player.entity().level(host) else {
            return Flow::Continue;
        };
        if *event.use_item != TriState::False
            && let Some(item) = geology::held_scanner_item(host, player, event.click.hand)
        {
            if item == geology::PORTABLE_GEO_SCANNER {
                network_runtime::open_scanner(
                    &self.networks,
                    host,
                    player,
                    level,
                    event.click.pos,
                    true,
                    &self.rolls,
                );
            } else {
                self.gps
                    .start_waypoint(host, player, level, event.click.pos, event.click.hit.face);
            }
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if event.click.hand != Hand::Main {
            return Flow::Continue;
        }
        if self.composters.use_at(
            host,
            player,
            level,
            event.click.pos,
            *event.use_item != TriState::False,
        ) || self.crucibles.use_at(
            host,
            player,
            level,
            event.click.pos,
            *event.use_item != TriState::False,
        ) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if network_runtime::open_scanner(
            &self.networks,
            host,
            player,
            level,
            event.click.pos,
            false,
            &self.rolls,
        ) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if self
            .gps
            .use_panel(&self.networks, host, player, level, event.click.pos)
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if self.holograms.use_at(host, player, level, event.click.pos) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if self.elevators.editor(host, player, level, event.click.pos) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if self.androids.use_at(host, player, level, event.click.pos) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if factory::open(host, player, level, event.click.pos) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if network_runtime::use_node(&self.networks, host, player, level, event.click.pos) {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if player.entity().is_crouching(host) != Some(false) {
            return Flow::Continue;
        }
        if let Some(stack) = event.click.stack
            && let Some(pan) = gold_pan::PANS
                .iter()
                .find(|pan| pan.item.matches(host, stack))
        {
            pan.use_on(host, player, level, event.click.pos, &self.rolls);
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if self
            .industrial_miners
            .activate(host, level, player, event.click.pos)
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if let Some(machine) = hand_machine::HandMachine::at(host, level, event.click.pos) {
            machine.activate(
                host,
                player,
                level,
                event.click.pos,
                &self.delayed_crafts,
                &self.rolls,
            );
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        let Some((machine, dispenser)) = Machine::at(host, level, event.click.pos) else {
            return Flow::Continue;
        };
        let settings = *self.settings.lock().unwrap();
        machine.activate(
            host,
            player,
            dispenser,
            event.click.pos,
            &self.delayed_crafts,
            settings,
            &self.rolls,
        );
        *event.cancellation_result = InteractionResult::Consume;
        Flow::Cancel
    }

    #[listener(priority = -100)]
    fn right_click_item(&self, host: HostSide<'_>, event: &mut RightClickItemEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let Some(player) = host.player(event.click.player) else {
            return Flow::Continue;
        };
        if self.guide.right_click(host, player, event.click.stack)
            || self.item_clicks.consume_fallthrough(
            host,
            event.click.player,
            event.click.hand,
            event.click.stack,
        )
            || jet_boots::STEEL_THRUSTER.matches(host, event.click.stack)
            || jetpack::CHAIN.matches(host, event.click.stack)
            || medical::HEAVY_CREAM.matches(host, event.click.stack)
            || medical::use_supply(host, player, event.click.hand, event.click.stack)
            || magic_eye::use_held(host, player, event.click.stack)
            || snacks::use_sugar(host, player, event.click.hand, event.click.stack, None)
            || elemental_staves::use_staff(host, player, event.click.stack, || {
                self.settings.lock().unwrap().wind_power
            })
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if matches!(
            player.game_mode(host),
            Some(GameMode::Survival | GameMode::Creative)
        ) && (tape_measure::in_air(host, player, event.click.stack)
                || portable_crafter::use_held(host, player, event.click.stack)
                || self.backpacks.use_held(host, player, event.click.hand)
                || network_runtime::trash::portable(host, player, event.click.hand)
                || self
                    .transport
                    .use_portable(&self.networks, host, player, event.click.hand)
                || geology::held_scanner_item(host, player, event.click.hand).is_some()
                || factory::experience_collector::use_flask(host, player, event.click.hand))
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn player_keep_death_item(
        &self,
        host: &dyn mod_api::Host,
        event: &mut mod_api::PlayerKeepDeathItemEvent<'_>,
    ) -> Flow {
        if soulbound_rune::is_bound(host, event.stack) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn item_toss(&self, host: &mut dyn ServerHost, event: &mut mod_api::ItemTossEvent) -> Flow {
        self.soulbound_rituals.tossed(host, event);
        Flow::Continue
    }

    fn item_expire(&self, _: &mut dyn ServerHost, event: &mut mod_api::ItemExpireEvent<'_>) {
        ancient_altar::item_expire(event);
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        if let Some(player) = host.player(event.player) {
            research::import_legacy(host, player);
            if let Err(error) = gps_waypoints::import(host, event.player) {
                let _ = player.tell(host, Text::literal(error));
            }
        }
    }

    fn player_logged_out(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerLoggedOutEvent<'_>,
    ) {
        self.item_clicks.logout(event.player);
        self.jet_boots.forget(host, event.player);
        self.jetpacks.forget(host, event.player);
        self.spawners.forget(event.player);
        self.bee_wings.stop(event.player);
        self.gps.logout(event.player);
        self.guide.logout(event.player);
        self.androids.logout(event.player);
        self.magnets.stop(event.player);
        self.holograms.logout(event.player);
        self.transport.logout(event.player);
        self.elevators.logout(event.player);
    }

    #[listener(priority = -100)]
    fn server_chat(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::ServerChatEvent<'_>,
    ) -> Flow {
        if let Some(reply) = self
            .gps
            .take_chat(host, event.player, event.raw_text)
            .or_else(|| self.androids.take_chat(host, event.player, event.raw_text))
            .or_else(|| self.holograms.take_chat(host, event.player, event.raw_text))
            .or_else(|| self.elevators.chat(host, event.player, event.raw_text))
        {
            event.replies.push(reply);
            return Flow::Cancel;
        }
        Flow::Continue
    }

    #[listener(priority = -100)]
    fn block_trigger_entity_pre(
        &self,
        level: &dyn LevelHost,
        event: &mut mod_api::BlockTriggerEntityPreEvent<'_>,
    ) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        if let Some(player) = event.player
            && level
                .block_state(pos)
                .and_then(|state| level.state_block(state))
                == Some("minecraft:stone_pressure_plate")
            && let Some(id) = network_runtime::plate_marker(level, &event.dimension, pos)
        {
            let dimension = network_runtime::dimension_name(&event.dimension);
            match id.as_str() {
                "ELEVATOR_PLATE" => self.elevators.step(dimension, player, pos),
                "GPS_ACTIVATION_DEVICE_SHARED" | "GPS_ACTIVATION_DEVICE_PERSONAL" => {
                    self.transport.step(dimension, player, pos);
                }
                _ => {}
            }
        }
        Flow::Continue
    }

    fn player_teleported(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerTeleportedEvent<'_>,
    ) {
        self.transport
            .teleported(host, event.player, event.dimension, event.to);
    }

    fn player_toggle_sneak(&self, host: &mut dyn ServerHost, event: &mut mod_api::PlayerToggleSneakEvent<'_>) {
        if let Some(player) = host.player(event.player) {
            self.magnets.toggle(host, player, event.sneaking, self.settings.lock().unwrap().infused);
            self.jetpacks.toggle(host, player, event.sneaking);
            self.jet_boots.toggle(host, player, event.sneaking);
        }
    }

    fn entity_toggle_glide(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::EntityToggleGlideEvent<'_>,
    ) {
        if event.gliding {
            self.bee_wings.start(host, event.player);
        } else {
            self.bee_wings.stop(event.player);
        }
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut mod_api::MenuClickedEvent) {
        self.guide.clicked(host, event);
        auto_crafter::clicked(host, event);
        factory::VIEWERS.clicked(host, event);
        self.gps.click(&self.networks, host, event);
        self.holograms.clicked(host, event);
        self.androids.clicked(host, event);
        self.transport.clicked(host, event);
        self.elevators.clicked(host, event);
        network_runtime::menu_click(&self.networks, host, event);
    }

    fn menu_closed(&self, _: &mut dyn ServerHost, event: &mut mod_api::MenuClosedEvent) {
        self.guide.closed(event);
        auto_crafter::closed(event);
        self.backpacks.closed(event);
        self.universal.closed(event);
        factory::VIEWERS.closed(event);
        self.gps.close(event);
        self.androids.closed(event);
        self.holograms.closed(event);
        self.transport.close(event);
        self.elevators.closed(event);
        network_runtime::menu_closed(&self.networks, event);
    }

    fn living_entity_use_item_start(
        &self,
        host: HostSide<'_>,
        event: &mut mod_api::LivingEntityUseItemStartEvent<'_>,
    ) -> Flow {
        match host {
            HostSide::Server(host) => {
                let result = thermal_machines::start_eating_jerky(host, event);
                if result != Flow::Continue {
                    return result;
                }
                let result = medical::start(host, event);
                if result != Flow::Continue {
                    return result;
                }
                let result = snacks::start(host, event);
                if result != Flow::Continue {
                    return result;
                }
                juice::start(host, event)
            }
            _ => Flow::Continue,
        }
    }

    fn living_entity_use_item_finish(
        &self,
        host: HostSide<'_>,
        event: &mut mod_api::LivingEntityUseItemFinishEvent<'_>,
    ) {
        if let HostSide::Server(host) = host {
            juice::finish(host, event);
            thermal_machines::finish_eating_jerky(host, event);
            medical::finish(host, event);
            self.snacks.finish(host, event, &self.rolls);
        }
    }

    fn tags_updated(&self, _: HostSide<'_>, event: &mut mod_api::TagsUpdatedEvent) {
        if matches!(event.cause, mod_api::TagsUpdateCause::ServerDataLoad { .. }) {
            auto_crafter::invalidate_vanilla_recipes();
        }
    }

    fn player_footwear_reaction(
        &self,
        _: &dyn mod_api::LoopContext,
        event: &mut mod_api::PlayerFootwearReactionEvent<'_>,
    ) -> Flow {
        armor_forge::footwear(event)
    }

    fn player_bee_sting(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerBeeStingEvent,
    ) -> Flow {
        radiation::protect_bee_sting(host, event.player, &self.rolls)
    }

    fn player_fall_reaction(
        &self,
        _: &dyn mod_api::LoopContext,
        event: &mut mod_api::PlayerFallReactionEvent<'_>,
    ) -> Flow {
        stomper::select(event)
    }

    fn player_fall_reaction_post(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::PlayerFallReactionPostEvent,
    ) {
        stomper::apply(host, event);
    }

    fn player_gliding_impact(
        &self,
        _: &dyn mod_api::LoopContext,
        event: &mut mod_api::PlayerGlidingImpactEvent<'_>,
    ) -> Flow {
        armor_forge::gliding_impact(event)
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut mod_api::ServerTickEvent) {
        if let mod_api::ServerTickPhase::ServerTickStart { world_running } = event.phase {
            self.radiation.begin_tick(|| host.server_monotonic_millis());
            self.smelters_pickaxes.begin_tick();
            if world_running {
                rainbow::placed::begin_tick();
            }
        }
    }

    fn player_tick_pre(&self, host: HostSide<'_>, event: &mut mod_api::PlayerTickPreEvent) {
        if let HostSide::Server(host) = host {
            self.jetpacks.tick(host, event.player, &self.rolls);
            self.jet_boots.tick(host, event.player, &self.rolls);
        }
    }

    fn player_tick_post(&self, host: HostSide<'_>, event: &mut mod_api::PlayerTickPostEvent) {
        if let HostSide::Server(host) = host {
            self.radiation.player_tick(host, event.player);
        }
    }

    fn player_died(&self, host: &mut dyn ServerHost, event: &mut mod_api::PlayerDiedEvent<'_>) {
        gps_waypoints::deathpoint(host, event.player);
    }

    fn living_death_post(
        &self,
        host: &mut dyn ServerHost,
        event: &mut mod_api::LivingDeathPostEvent<'_>,
    ) {
        self.radiation.death(host, event.entity);
        self.butcher_androids.death(host, event.entity);
    }

    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            let settings = *self.settings.lock().unwrap();
            self.butcher_androids.tick(host, event.level, &self.rolls);
            network_runtime::tick(
                &self.networks,
                host,
                event.level,
                settings,
                &self.rolls,
                &self.butcher_androids,
            );
            factory::VIEWERS.refresh(host, event.level);
            self.crucibles.tick(host, event.level);
            self.industrial_miners.tick(host, event.level, &self.rolls);
            self.composters.tick(host, event.level, &self.rolls);
            ancient_altar::tick(host, event.level);
            rainbow::placed::tick(host, event.level);
            hologram_projector::tick(host, event.level);
            auto_crafter::refresh(host, event.level);
            armor_forge::tick_equipment(host, event.level);
            self.solar_helmet.tick(host, event.level);
            self.magnets.tick(host, event.level);
            self.bee_wings.tick(host, event.level);
            self.transport.tick(&self.networks, host, event.level);
            self.elevators.tick(host, event.level);
        }
    }

    fn furnace_process(
        &self,
        host: &dyn mod_api::Host,
        event: &mut mod_api::FurnaceProcessEvent<'_>,
    ) {
        enhanced_furnaces::process(host, event, &self.rolls);
    }

    fn level_tick_pre(&self, host: HostSide<'_>, event: &mut LevelTickPreEvent) {
        if let HostSide::Server(host) = host {
            self.soulbound_rituals.tick(host, event.level);
            self.elevators.tick_pre(host, event.level);
            self.block_placers.tick(host, event.level, &self.networks);
        }
    }

    fn chunk_load(&self, host: &mut dyn ServerHost, event: &mut mod_api::ChunkLoadEvent<'_>) {
        rainbow::placed::loaded(host, event.dimension, event.chunk);
    }

    fn chunk_unload(&self, _: &mut dyn ServerHost, event: &mut mod_api::ChunkUnloadEvent<'_>) {
        rainbow::placed::unloaded(event.dimension, event.chunk);
    }

    fn level_unload(&self, host: &mut dyn ServerHost, event: &mut mod_api::LevelUnloadEvent<'_>) {
        self.soulbound_rituals.unload(host, event.dimension);
        self.elevators.unload(host, event.dimension);
        rainbow::placed::unload_level(event.dimension);
        self.networks.lock().unwrap().unload_level(event.dimension);
        self.block_placers.unload(host, event.dimension);
        self.industrial_miners.unload(host, event.dimension);
        self.butcher_androids.unload(host, event.dimension);
    }

    fn scheduled_task(&self, host: &mut dyn ServerHost, event: &mut ScheduledTaskEvent<'_>) {
        self.snacks.due(host, event);
        self.jet_boots.due(event);
        self.jetpacks.due(event);
        self.smelters_pickaxes.due(host, event);
        self.soulbound_rituals.due(host, event);
        self.elevators.due(host, event);
        self.block_placers.due(host, event, &self.networks);
        self.delayed_crafts.scheduled_task(host, event);
    }

    fn validate_tickets(
        &self,
        _: &mut dyn ServerHost,
        event: &mut mod_api::ValidateTicketsEvent<'_>,
    ) {
        industrial_miner::validate_tickets(event);
    }

    fn server_stopping(&self, host: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        self.item_clicks.clear();
        self.snacks.clear();
        self.jet_boots.clear();
        self.jetpacks.clear();
        self.spawners.clear();
        self.smelters_pickaxes.clear();
        self.soulbound_rituals.clear();
        self.elevators.clear();
        self.block_placers.clear();
        self.radiation.clear();
        rainbow::placed::clear();
        auto_crafter::clear();
        self.backpacks.clear();
        self.bee_wings.clear();
        self.universal.clear();
        self.gps.clear();
        self.guide.clear();
        self.androids.clear();
        self.magnets.clear();
        self.butcher_androids.clear();
        self.holograms.clear();
        self.transport.clear();
        factory::VIEWERS.clear();
        self.networks.lock().unwrap().clear();
        self.delayed_crafts.clear();
        self.composters.clear();
        self.crucibles.clear();
        self.industrial_miners.clear(host);
        self.rolls.clear();
    }
}

fn describe_recipe(id: &str, double_ores: bool) -> Vec<String> {
    let outputs = |output: &str| {
        output.eq_ignore_ascii_case(id)
            || output
                .split_once(':')
                .is_some_and(|(_, name)| name.eq_ignore_ascii_case(id))
    };
    let mut lines = industrial_miner::recipe_description(id)
        .into_iter()
        .collect::<Vec<_>>();
    for (name, recipes, multiplier) in [
        ("Grind Stone", GRINDING, 1),
        ("Ore Crusher", CRUSHING, 1),
        ("Ore Crusher", DOUBLING, 1 + u32::from(double_ores)),
        ("Compressor", COMPRESSING, 1),
        ("Pressure Chamber", PRESSING, 1),
        ("Juicer", JUICING, 1),
    ] {
        lines.extend(
            recipes
                .iter()
                .filter(|recipe| outputs(recipe.output.kind.id()))
                .map(|recipe| {
                    format!(
                        "{name} | {} × {} -> {} × {}",
                        recipe.input.count,
                        recipe.input.kind.id(),
                        recipe.output.count * multiplier,
                        recipe.output.kind.id()
                    )
                }),
        );
    }

    for timed in electric_processing::PRESSING
        .iter()
        .filter(|timed| outputs(timed.recipe.output.kind.id()))
    {
        let recipe = timed.recipe;
        lines.push(format!(
            "Electric Press | {} × {} -> {} × {}",
            recipe.input.count,
            recipe.input.kind.id(),
            recipe.output.count,
            recipe.output.kind.id()
        ));
    }

    for &kind in WASHER_DUSTS {
        if outputs(kind.id()) {
            for machine in ["Ore Washer", "Electric Dust Washer"] {
                lines.push(format!(
                    "{machine} | 1 × SIFTED_ORE -> 1 × {} (one of nine dusts)",
                    kind.id()
                ));
            }
        }
    }
    for recipe in WASHING
        .iter()
        .filter(|recipe| outputs(recipe.output.kind.id()))
    {
        lines.push(format!(
            "Ore Washer | {} × {} -> {} × {}",
            recipe.input.count,
            recipe.input.kind.id(),
            recipe.output.count,
            recipe.output.kind.id(),
        ));
    }
    for resource in geo_deposits::MINER_RESOURCES {
        let item = resource.item();
        if outputs(item.id()) || outputs(geology::GEO_MINER.id()) {
            lines.push(format!(
                "GEO Miner | 1 × {} from a scanned chunk",
                item.id()
            ));
        }
    }
    if outputs(items::SALT.id()) {
        lines.push("Electric Dust Washer | 1 × minecraft:sand -> 1 × SALT".into());
    }
    if outputs(items::PURE_ORE_CLUSTER.id()) {
        lines.push("Electric Dust Washer | 1 × PULVERIZED_ORE -> 1 × PURE_ORE_CLUSTER".into());
    }

    for recipe in ALLOY_RECIPES
        .iter()
        .chain(smelting())
        .filter(|recipe| outputs(recipe.output.kind.id()))
    {
        let machines: &[&str] = if machine::makeshift_recipe(recipe) {
            &["Smeltery or Makeshift Smeltery", "Electric Ingot Factory"]
        } else {
            &["Smeltery"]
        };
        let inputs = recipe
            .inputs
            .iter()
            .map(|stack| format!("{} × {}", stack.count, stack.kind.id()))
            .collect::<Vec<_>>()
            .join(" + ");
        for machine in machines {
            lines.push(format!(
                "{machine} | {inputs} -> {} × {}",
                recipe.output.count,
                recipe.output.kind.id()
            ));
        }
    }
    for pan in gold_pan::PANS {
        for &(weight, kind) in pan.drops {
            if outputs(kind.id()) {
                let targets = pan
                    .targets
                    .iter()
                    .map(|&(target, _)| target)
                    .collect::<Vec<_>>()
                    .join(" or ");
                for machine in [pan.research.name, "Electric Gold Pan"] {
                    lines.push(format!(
                        "{machine} | {targets} -> 1 × {} ({weight}%)",
                        kind.id()
                    ));
                }
            }
        }
    }

    for recipe in heated_pressure::RECIPES
        .iter()
        .filter(|recipe| outputs(recipe.output.kind.id()))
    {
        let inputs = recipe
            .inputs
            .iter()
            .map(|stack| format!("{} × {}", stack.count, stack.kind.id()))
            .collect::<Vec<_>>()
            .join(" + ");
        lines.push(format!(
            "Heated Pressure Chamber | {inputs} -> {} × {}",
            recipe.output.count,
            recipe.output.kind.id()
        ));
    }

    for recipe in advanced_processing::smeltery_recipes()
        .iter()
        .filter(|recipe| outputs(recipe.output.kind.id()))
    {
        let inputs = recipe
            .inputs
            .iter()
            .map(|stack| format!("{} × {}", stack.count, stack.kind.id()))
            .collect::<Vec<_>>()
            .join(" + ");
        lines.push(format!(
            "Electric Smeltery | {inputs} -> {} × {}",
            recipe.output.count,
            recipe.output.kind.id(),
        ));
    }

    if outputs(advanced_processing::LAVA_BUCKET.id()) {
        lines.extend(advanced_processing::LAVA_INPUTS.iter().map(|recipe| format!(
            "Electrified Crucible | 1 × minecraft:bucket + {} × {} -> 1 × minecraft:lava_bucket",
            recipe.input.count, recipe.input.kind.id(),
        )));
    }
    if outputs(advanced_processing::WATER_BUCKET.id()) {
        lines.push("Electrified Crucible | 1 × minecraft:bucket + 16 × leaves -> 1 × minecraft:water_bucket".into());
    }
    if outputs(heated_pressure::BUCKET_OF_OIL.id()) {
        lines.push(
            "Oil Pump | 1 × minecraft:bucket + 1 unit of scanned oil -> 1 × BUCKET_OF_OIL".into(),
        );
    }
    if outputs(fuel_generators::BUCKET_OF_FUEL.id()) {
        lines.push("Refinery | 1 × BUCKET_OF_OIL -> 1 × BUCKET_OF_FUEL".into());
    }

    for crop in food_machines::PRODUCE {
        if outputs(crop.food.id()) {
            lines.push(format!(
                "Food Fabricator | 1 × CAN + 1 × {} -> 2 × {}",
                crop.crop.id(),
                crop.food.id()
            ));
        }
        if outputs(crop.fertilizer.id()) {
            lines.push(format!(
                "Food Composter | 1 × {} -> 2 × {}",
                crop.food.id(),
                crop.fertilizer.id()
            ));
        }
    }
    for &(ingredient, potion) in auto_brewer::AWKWARD_RECIPES {
        if outputs(potion) {
            lines.push(format!(
                "Auto Brewer | 1 × awkward potion + 1 × {ingredient} -> 1 × potion of {potion}"
            ));
        }
    }

    if outputs(items::BASIC_CIRCUIT_BOARD.id()) {
        lines.push(
            "Iron Golem | player kill | 1 × BASIC_CIRCUIT_BOARD | research basic_circuit_board"
                .into(),
        );
    }
    let workbenches = crafting()
        .map(|recipe| ("Enhanced Crafting Table", recipe))
        .chain(recipes::magic_crafting().map(|recipe| ("Magic Workbench", recipe)))
        .chain(
            armor_forge::RECIPES
                .iter()
                .map(|recipe| ("Armor Forge", recipe)),
        )
        .chain(
            ancient_altar::CRAFTING
                .iter()
                .chain(armor_forge::ALTAR_RECIPES)
                .chain(rainbow::ALTAR_RECIPES)
                .chain(soulbound_gear::ALTAR_RECIPES)
                .map(|recipe| ("Ancient Altar", recipe)),
        );
    for (workbench, recipe) in workbenches.filter(|(bench, recipe)| {
        outputs(recipe.output.kind.id())
            || (*bench == "Armor Forge" && outputs(armor_forge::paper_id(recipe.output.kind)))
    }) {
        let mut header = format!(
            "{workbench} | {} × {}",
            recipe.output.count,
            recipe.output.kind.id()
        );
        if !recipe.research.id.is_empty() {
            header.push_str(&format!(" | research {}", recipe.research.id));
        }
        lines.push(header);
        lines.extend(recipe.inputs.chunks_exact(3).map(|row| {
            row.iter()
                .map(|cell| cell.map_or("-", |kind| kind.id()))
                .collect::<Vec<_>>()
                .join(" | ")
        }));
    }

    if lines.is_empty() {
        lines.push("No recipe for this item in the Horizon preview.".into());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nether_pan_recipe_and_uses_have_the_paper_ingredients_and_targets() {
        assert_eq!(
            describe_recipe("NETHER_GOLD_PAN", true),
            [
                "Enhanced Crafting Table | 1 × NETHER_GOLD_PAN | research nether_gold_pan",
                "- | - | -",
                "minecraft:nether_brick | GOLD_PAN | minecraft:nether_brick",
                "minecraft:nether_brick | minecraft:nether_brick | minecraft:nether_brick",
            ],
        );
        assert!(describe_recipe("ghast_tear", true).iter().any(|line| line ==
            "Nether Gold Pan | minecraft:soul_sand or minecraft:soul_soil -> 1 × minecraft:ghast_tear (2%)"
        ));
        assert_eq!(recipes::NETHER_GOLD_PAN_RESEARCH.cost, 8);
    }

    #[test]
    fn recipe_lookup_reports_the_configured_ore_yield() {
        let normal = describe_recipe("diamond", false);
        assert!(
            normal
                .iter()
                .any(|line| line
                    == "Ore Crusher | 1 × minecraft:diamond_ore -> 1 × minecraft:diamond")
        );

        let double = describe_recipe("diamond", true);
        assert!(
            double
                .iter()
                .any(|line| line
                    == "Ore Crusher | 1 × minecraft:diamond_ore -> 2 × minecraft:diamond")
        );
    }
}
