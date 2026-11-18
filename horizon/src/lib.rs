// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{
    BlockDropsEvent, Command, CommandArgumentValue, CommandSource, Config, ConfigFile, ConfigKind,
    ConfigLoadingEvent, ConfigReloadingEvent, Flow, GameMode, Hand, Hooks, HostSide,
    InteractionResult, ItemKey, LevelHost, PlayerLoggedInEvent, RegisterCommandsEvent,
    RegisterConfigsEvent, RightClickBlockEvent, RunModCommandEvent, ScheduledTaskEvent, ServerHost,
    ServerStoppingEvent, Text, TriState,
};

mod compressor;
mod gold_pan;
mod items;
mod machine;
mod output_chest;
mod random;
mod recipes;
mod research;

use compressor::Compressor;
use machine::{Machine, Settings};
use random::Rolls;
use recipes::{
    ALLOY_RECIPES, COMPRESSING, CRAFTING, CRUSHING, DOUBLING, GRINDING, RESEARCHES, SMELTING,
    WASHER_DUSTS, WASHING,
};

struct Slimefun {
    compressor: Compressor,
    settings: Mutex<Settings>,
    rolls: Rolls,
    chest: Mutex<Option<ItemKey>>,
}

impl Default for Slimefun {
    fn default() -> Self {
        Self {
            compressor: Compressor::default(),
            settings: Mutex::new(Settings::default()),
            rolls: Rolls::default(),
            chest: Mutex::new(None),
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
        }
        let Ok(table) = config.text.parse::<toml::Table>() else {
            return;
        };
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
    }
}

#[mod_api::horizon_mod]
impl Hooks for Slimefun {
    fn register(&self, registrar: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let chest = registrar.item_key("minecraft:chest");
        registrar.item_part(chest, output_chest::ChestPlacement);
        *self.chest.lock().unwrap() = Some(chest);
    }

    fn register_configs(&self, _: HostSide<'_>, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(Config::new(
            "slimefun",
            ConfigKind::Server,
            "double-ores = true\nlegacy-ore-washer = false\nfire-breaking-chance = 34\noutput-chest-enabled = true\n",
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
            Some("status") => vec![format!(
                "Slimefun Horizon development | Grind Stone {} | Enhanced Crafting Table {} | Ore Crusher {} | Compressor {} | Ore Washer {} dusts and {} recipes | Smeltery {} recipes | Research {}",
                GRINDING.len(),
                CRAFTING.len(),
                CRUSHING.len() + DOUBLING.len(),
                COMPRESSING.len(),
                WASHER_DUSTS.len(),
                WASHING.len(),
                SMELTING.len() + ALLOY_RECIPES.len(),
                RESEARCHES.len()
            )],
            Some("recipe") => match event.command.argument("item") {
                Some(CommandArgumentValue::String(item)) => describe_recipe(item, self.settings.lock().unwrap().double_ores),
                _ => return Flow::Continue,
            },
            Some("research") => {
                let Some(CommandArgumentValue::String(id)) = event.command.argument("key") else {
                    return Flow::Continue;
                };
                let Some(research) = RESEARCHES.iter().find(|research| research.id.eq_ignore_ascii_case(id)) else {
                    let ids = RESEARCHES.iter().map(|research| research.id).collect::<Vec<_>>().join(", ");
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
                "Wash Sifted Ore by placing a dispenser on a wooden fence with an empty cauldron below it and right-clicking the fence.".into(),
                "For a Makeshift Smeltery use a wooden fence above a dispenser, bricks on opposite sides of the dispenser, and fire below it. Use a nether brick fence and nether bricks for the regular Smeltery.".into(),
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
        if event.click.hand != Hand::Main || *event.use_block == TriState::False {
            return Flow::Continue;
        }
        let Some(player) = host.player(event.click.player) else {
            return Flow::Continue;
        };
        if !matches!(
            player.game_mode(host),
            Some(GameMode::Survival | GameMode::Creative)
        ) || player.entity().is_crouching(host) != Some(false)
        {
            return Flow::Continue;
        }
        let Some(level) = player.entity().level(host) else {
            return Flow::Continue;
        };
        let is_gold_pan = event
            .click
            .stack
            .is_some_and(|stack| items::GOLD_PAN.matches(host, stack));
        if is_gold_pan {
            gold_pan::use_pan(host, player, level, event.click.pos, &self.rolls);
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
            &self.compressor,
            settings,
            &self.rolls,
        );
        *event.cancellation_result = InteractionResult::Consume;
        Flow::Cancel
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        if let Some(player) = host.player(event.player) {
            research::import_legacy(host, player);
        }
    }

    fn scheduled_task(&self, host: &mut dyn ServerHost, event: &mut ScheduledTaskEvent<'_>) {
        self.compressor.scheduled_task(host, event);
    }

    fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        self.compressor.clear();
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

    for &kind in WASHER_DUSTS {
        if outputs(kind.id()) {
            lines.push(format!(
                "Ore Washer | 1 × SIFTED_ORE -> 1 × {} (one of nine dusts)",
                kind.id()
            ));
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
    for recipe in ALLOY_RECIPES
        .iter()
        .chain(SMELTING)
        .filter(|recipe| outputs(recipe.output.kind.id()))
    {
        let name = if machine::makeshift_recipe(recipe) {
            "Smeltery or Makeshift Smeltery"
        } else {
            "Smeltery"
        };
        let inputs = recipe
            .inputs
            .iter()
            .map(|stack| format!("{} × {}", stack.count, stack.kind.id()))
            .collect::<Vec<_>>()
            .join(" + ");
        lines.push(format!(
            "{name} | {inputs} -> {} × {}",
            recipe.output.count,
            recipe.output.kind.id()
        ));
    }
    for &(weight, kind) in recipes::PAN_DROPS {
        if outputs(kind.id()) {
            lines.push(format!(
                "Gold Pan | minecraft:gravel -> 1 × {} ({weight}%)",
                kind.id()
            ));
        }
    }

    for recipe in CRAFTING
        .iter()
        .filter(|recipe| outputs(recipe.output.kind.id()))
    {
        lines.push(format!(
            "Enhanced Crafting Table | {} × {} | research {}",
            recipe.output.count,
            recipe.output.kind.id(),
            recipe.research.id
        ));
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
