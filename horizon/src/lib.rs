// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Command, CommandArgumentValue, CommandSource, Flow, GameMode, Hand, Hooks, HostSide,
    InteractionResult, RegisterCommandsEvent, RightClickBlockEvent, RunModCommandEvent, ServerHost,
    Text, TriState,
};

mod items;
mod machine;
mod recipes;
mod research;

use machine::Machine;
use recipes::{CRAFTING, GRINDING, RESEARCHES};

#[derive(Default)]
struct Slimefun;

#[mod_api::horizon_mod]
impl Hooks for Slimefun {
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
                "Slimefun Horizon preview | Grind Stone: {} recipes | Enhanced Crafting Table: {} recipes | Research: {} entries",
                GRINDING.len(),
                CRAFTING.len(),
                RESEARCHES.len()
            )],
            Some("recipe") => match event.command.argument("item") {
                Some(CommandArgumentValue::String(item)) => describe_recipe(item),
                _ => return Flow::Continue,
            },
            Some("research") => {
                let Some(CommandArgumentValue::String(id)) = event.command.argument("key") else {
                    return Flow::Continue;
                };
                let Some(research) = RESEARCHES.iter().find(|research| research.id.eq_ignore_ascii_case(id)) else {
                    let _ = host.commands().feedback(Text::literal("Supported research: lumps, kelp_cookie."));
                    return Flow::Cancel;
                };
                match *event.source {
                    CommandSource::Player { player, .. } => vec![research::unlock(host, player, research)],
                    _ => vec!["A player must unlock their own research.".into()],
                }
            }
            None | Some("guide") => vec![
                "Slimefun Horizon preview".into(),
                "Grind Stone: put an oak fence on an upward-facing dispenser. Load the dispenser and right-click the fence.".into(),
                "Enhanced Crafting Table: put a crafting table on a dispenser. Load the recipe in its 3×3 grid and right-click the table.".into(),
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
        machine.activate(host, player, dispenser, event.click.pos);
        *event.cancellation_result = InteractionResult::Consume;
        Flow::Cancel
    }
}

fn describe_recipe(id: &str) -> Vec<String> {
    let outputs = |output: &str| {
        output.eq_ignore_ascii_case(id)
            || output
                .split_once(':')
                .is_some_and(|(_, name)| name.eq_ignore_ascii_case(id))
    };
    let mut lines: Vec<String> = GRINDING
        .iter()
        .filter(|recipe| outputs(recipe.output.kind.id()))
        .map(|recipe| {
            format!(
                "Grind Stone | {} × {} -> {} × {}",
                recipe.input.count,
                recipe.input.kind.id(),
                recipe.output.count,
                recipe.output.kind.id()
            )
        })
        .collect();

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
