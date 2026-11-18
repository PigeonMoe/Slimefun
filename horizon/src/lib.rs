// SPDX-License-Identifier: GPL-3.0-only

use std::sync::atomic::{AtomicBool, Ordering};

use mod_api::{
    Command, CommandArgumentValue, CommandSource, Config, ConfigFile, ConfigKind,
    ConfigLoadingEvent, ConfigReloadingEvent, Flow, GameMode, Hand, Hooks, HostSide,
    InteractionResult, PlayerLoggedInEvent, RegisterCommandsEvent, RegisterConfigsEvent,
    RightClickBlockEvent, RunModCommandEvent, ScheduledTaskEvent, ServerHost, ServerStoppingEvent,
    Text, TriState,
};

mod compressor;
mod items;
mod machine;
mod output_chest;
mod recipes;
mod research;

use compressor::Compressor;
use machine::Machine;
use recipes::{COMPRESSING, CRAFTING, CRUSHING, DOUBLING, GRINDING, RESEARCHES};

struct Slimefun {
    compressor: Compressor,
    double_ores: AtomicBool,
}

impl Default for Slimefun {
    fn default() -> Self {
        Self {
            compressor: Compressor::default(),
            double_ores: AtomicBool::new(true),
        }
    }
}

impl Slimefun {
    fn read_config(&self, config: &ConfigFile) {
        if config.mod_id == "slimefun"
            && config.file == "slimefun-server.toml"
            && let Ok(table) = config.text.parse::<toml::Table>()
            && let Some(value) = table.get("double-ores").and_then(toml::Value::as_bool)
        {
            self.double_ores.store(value, Ordering::Relaxed);
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Slimefun {
    fn register(&self, registrar: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
        let chest = registrar.item_key("minecraft:chest");
        registrar.item_part(chest, output_chest::ChestPlacement);
    }

    fn register_configs(&self, _: HostSide<'_>, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(Config::new(
            "slimefun",
            ConfigKind::Server,
            "double-ores = true\n",
        ));
    }

    fn config_loading(&self, _: HostSide<'_>, event: &mut ConfigLoadingEvent<'_>) {
        if event.config.mod_id == "slimefun" && event.config.file == "slimefun-server.toml" {
            self.double_ores.store(true, Ordering::Relaxed);
            self.read_config(event.config);
        }
    }

    fn config_reloading(&self, _: HostSide<'_>, event: &mut ConfigReloadingEvent<'_>) {
        self.read_config(event.config);
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
                "Slimefun Horizon development | Grind Stone {} | Enhanced Crafting Table {} | Ore Crusher {} | Compressor {} | Research {}",
                GRINDING.len(),
                CRAFTING.len(),
                CRUSHING.len() + DOUBLING.len(),
                COMPRESSING.len(),
                RESEARCHES.len()
            )],
            Some("recipe") => match event.command.argument("item") {
                Some(CommandArgumentValue::String(item)) => describe_recipe(item, self.double_ores.load(Ordering::Relaxed)),
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
                "Slimefun Horizon preview".into(),
                "Build a Grind Stone by putting a wooden fence on an upward-facing dispenser. Load the dispenser and right-click the fence.".into(),
                "Build an Enhanced Crafting Table by putting a crafting table on a dispenser. Arrange the recipe in its 3×3 grid and right-click the table.".into(),
                "An Ore Crusher uses a nether brick fence over an upward dispenser, with iron bars on opposite sides below the fence. A Compressor uses pistons instead of bars.".into(),
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
        let Some((machine, dispenser)) = Machine::at(host, level, event.click.pos) else {
            return Flow::Continue;
        };
        machine.activate(
            host,
            player,
            dispenser,
            event.click.pos,
            &self.compressor,
            self.double_ores.load(Ordering::Relaxed),
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
