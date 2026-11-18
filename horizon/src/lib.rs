// SPDX-License-Identifier: GPL-3.0-only

use std::sync::{Arc, Mutex};

use mod_api::{
    BlockDropsEvent, Command, CommandArgumentValue, CommandSource, Config, ConfigFile, ConfigKind,
    ConfigLoadingEvent, ConfigReloadingEvent, Flow, GameMode, Hand, Hooks, HostSide,
    InteractionResult, ItemKey, LevelHost, LevelTickPostEvent, PlayerLoggedInEvent,
    RegisterCommandsEvent, RegisterConfigsEvent, RightClickBlockEvent, RightClickItemEvent,
    RunModCommandEvent, ScheduledTaskEvent, ServerHost, ServerStoppingEvent, Text, TriState,
};

mod advanced_processing;
mod auto_brewer;
mod backpacks;
mod circuits;
mod compressor;
mod crucible;
mod electric_processing;
mod factory;
mod food_machines;
mod fuel_generators;
mod geology;
mod gold_pan;
mod hand_machine;
mod heated_pressure;
mod items;
mod juice;
mod machine;
mod network_recipes;
mod network_runtime;
mod networks;
mod output_chest;
mod random;
mod recipes;
mod research;
mod storage;
mod thermal_machines;

use compressor::DelayedCrafts;
use machine::{Machine, Settings};
use random::Rolls;
use recipes::{
    ALLOY_RECIPES, COMPRESSING, CRUSHING, DOUBLING, GRINDING, JUICING, PRESSING, WASHER_DUSTS,
    WASHING, crafting, researches, smelting,
};

struct Slimefun {
    delayed_crafts: DelayedCrafts,
    crucibles: crucible::Crucibles,
    settings: Mutex<Settings>,
    rolls: Rolls,
    chest: Mutex<Option<ItemKey>>,
    networks: Arc<Mutex<network_runtime::Networks>>,
    backpacks: backpacks::BackpackMenus,
}

impl Default for Slimefun {
    fn default() -> Self {
        Self {
            delayed_crafts: DelayedCrafts::default(),
            crucibles: crucible::Crucibles::default(),
            settings: Mutex::new(Settings::default()),
            rolls: Rolls::default(),
            chest: Mutex::new(None),
            networks: Arc::default(),
            backpacks: backpacks::BackpackMenus::default(),
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
            output_chest::set_enabled(true);
            self.crucibles.configure(false);
        }
        let Ok(table) = config.text.parse::<toml::Table>() else {
            return;
        };
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
    }
}

#[mod_api::horizon_mod]
impl Hooks for Slimefun {
    fn register(&self, registrar: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let chest = registrar.item_key("minecraft:chest");
        registrar.item_part(chest, output_chest::ChestPlacement);
        *self.chest.lock().unwrap() = Some(chest);
        for id in [
            "minecraft:player_head",
            "minecraft:daylight_detector",
            "minecraft:furnace",
            "minecraft:smoker",
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
        crucible::register(registrar);
        factory::register(registrar);
        backpacks::register(registrar);
    }

    fn register_configs(&self, _: HostSide<'_>, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(Config::new(
            "slimefun",
            ConfigKind::Server,
            "double-ores = true\ncrucible-allow-water-in-nether = false\nlegacy-ore-washer = false\nlegacy-dust-washer = false\nlegacy-ore-grinder = true\nelectric-gold-pan-override-output-limit = [false, false, false]\nfreezer-use-vanilla-ratios = [false, false, false]\nfire-breaking-chance = 34\noutput-chest-enabled = true\ndrop-from-golems = true\ngolem-drop-chance = 75\n",
        ));
    }

    fn config_loading(&self, _: HostSide<'_>, event: &mut ConfigLoadingEvent<'_>) {
        self.read_config(event.config, true);
    }

    fn config_reloading(&self, _: HostSide<'_>, event: &mut ConfigReloadingEvent<'_>) {
        self.read_config(event.config, false);
    }

    fn block_drops(&self, level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
        if let Some(chest) = *self.chest.lock().unwrap() {
            output_chest::restore_drop(level, event, chest);
        }
        network_runtime::restore_drop(level, event);
        factory::restore_drops(level, event);
        food_machines::restore_drop(level, event);
        Flow::Continue
    }

    fn block_changed(&self, level: &dyn LevelHost, event: &mut mod_api::BlockChangedEvent) {
        network_runtime::observe_plain_change(level, event);
        self.crucibles.anchor_changed(level, event);
        network_runtime::observe_gps_change(&self.networks, level, event);
    }

    fn piston_pre(&self, level: &dyn LevelHost, event: &mut mod_api::PistonPreEvent<'_>) -> Flow {
        if network_runtime::protect_piston(level, event) {
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

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            Command::new("slimefun"),
            Command::new("sf"),
            Command::new("slimefun guide"),
            Command::new("sf guide"),
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
            Some("storage") => storage::command(host, event, path.next(), &self.networks),
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
            None | Some("guide") => vec![
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
        if !matches!(
            player.game_mode(host),
            Some(GameMode::Survival | GameMode::Creative)
        ) {
            return Flow::Continue;
        }
        if *event.use_item != TriState::False
            && (self.backpacks.use_held(host, player, event.click.hand)
                || network_runtime::trash::portable(host, player, event.click.hand))
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        if event.click.hand != Hand::Main {
            return Flow::Continue;
        }
        let Some(level) = player.entity().level(host) else {
            return Flow::Continue;
        };
        if self.crucibles.use_at(
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
            &self.rolls,
        ) {
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
        if let HostSide::Server(host) = host
            && let Some(player) = host.player(event.click.player)
            && matches!(
                player.game_mode(host),
                Some(GameMode::Survival | GameMode::Creative)
            )
            && (self.backpacks.use_held(host, player, event.click.hand)
                || network_runtime::trash::portable(host, player, event.click.hand))
        {
            *event.cancellation_result = InteractionResult::Consume;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        if let Some(player) = host.player(event.player) {
            research::import_legacy(host, player);
        }
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut mod_api::MenuClickedEvent) {
        network_runtime::menu_click(&self.networks, host, event);
    }

    fn menu_closed(&self, _: &mut dyn ServerHost, event: &mut mod_api::MenuClosedEvent) {
        self.backpacks.closed(event);
        factory::VIEWERS.closed(event);
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
                if result == Flow::Continue {
                    juice::start(host, event)
                } else {
                    result
                }
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
        }
    }

    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            let settings = *self.settings.lock().unwrap();
            network_runtime::tick(&self.networks, host, event.level, settings, &self.rolls);
            factory::VIEWERS.refresh(host, event.level);
            self.crucibles.tick(host, event.level);
        }
    }

    fn scheduled_task(&self, host: &mut dyn ServerHost, event: &mut ScheduledTaskEvent<'_>) {
        self.delayed_crafts.scheduled_task(host, event);
    }

    fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        self.backpacks.clear();
        factory::VIEWERS.clear();
        self.networks.lock().unwrap().clear();
        self.delayed_crafts.clear();
        self.crucibles.clear();
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
    let mut lines = Vec::new();
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
    for recipe in crafting().filter(|recipe| outputs(recipe.output.kind.id())) {
        let mut header = format!(
            "Enhanced Crafting Table | {} × {}",
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
