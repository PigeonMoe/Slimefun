// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{
    CommandArgumentError, CommandArgumentKind, CommandArgumentType, CommandArgumentValue,
    CommandSender, CommandSource, CommandSuggestion, RunModCommandEvent, ServerHost, Text,
};

use crate::{
    admin_commands,
    guide::{Guide, Language},
    hologram_projector::colored_label,
};

const HELP_COMMANDS: [&str; 13] = [
    "help",
    "versions",
    "cheat",
    "guide",
    "give",
    "research",
    "stats",
    "timings",
    "teleporter",
    "open_guide",
    "search",
    "backpack",
    "charge",
];

pub(crate) const CARGO_INPUT_TESTING: &str = "slimefun_cargo_input_testing";
pub(crate) const PLAYER_PROFILE_DATA: &str = "slimefun_player_profile_data";

pub(crate) struct DebugCaseArgument;

impl CommandArgumentType for DebugCaseArgument {
    fn kind(&self) -> CommandArgumentKind {
        CommandArgumentKind::Greedy
    }

    fn parse(&self, input: &str) -> Result<String, CommandArgumentError> {
        Ok(input.split_whitespace().next().unwrap_or("").to_owned())
    }

    fn suggest(&self, _: Option<&CommandSender>, prefix: &str) -> Vec<CommandSuggestion> {
        if prefix.chars().any(char::is_whitespace) {
            return Vec::new();
        }
        let needle = prefix.to_ascii_lowercase();
        [
            CARGO_INPUT_TESTING,
            PLAYER_PROFILE_DATA,
            "slimefun_analytics",
        ]
        .into_iter()
        .filter(|case| case.contains(&needle))
        .map(CommandSuggestion::new)
        .collect()
    }
}

static TEST_CASE: Mutex<Option<String>> = Mutex::new(None);

pub(crate) fn help(language: Language) -> Vec<Text> {
    let mut lines = vec![
        Text::literal(""),
        colored_label(concat!("&aSlimefun &2v", env!("CARGO_PKG_VERSION"))),
        Text::literal(""),
    ];
    lines.extend(HELP_COMMANDS.into_iter().map(|name| {
        let description = language.command_message(name);
        colored_label(&format!("&3/sf {name} &b{description}"))
    }));
    lines
}

fn set_test_case(
    active: &mut Option<String>,
    argument: Option<&str>,
    language: Language,
) -> String {
    let message = |key| language.command_message(key);
    match argument {
        None => active.as_deref().map_or_else(
            || message("debug.none-running").into(),
            |case| message("debug.current").replace("%test_case%", case),
        ),
        Some(value)
            if value.eq_ignore_ascii_case("disable") || value.eq_ignore_ascii_case("off") =>
        {
            *active = None;
            message("debug.disabled").into()
        }
        Some(case) => {
            *active = Some(case.to_owned());
            message("debug.running").replace("%test%", case)
        }
    }
}

pub(crate) fn diagnostic(
    host: &dyn ServerHost,
    event: &RunModCommandEvent<'_>,
    guide: &Guide,
) -> Text {
    let language = guide.command_language(host, event.source);
    let allowed = match *event.source {
        CommandSource::Player { player, .. } => admin_commands::operator(host, player),
        _ => event.source.permission().level() > 0,
    };
    let argument = match event.command.argument("test") {
        Some(CommandArgumentValue::Custom { value, .. }) if !value.is_empty() => {
            Some(value.as_str())
        }
        _ => None,
    };
    let message = if allowed {
        set_test_case(&mut TEST_CASE.lock().unwrap(), argument, language)
    } else {
        language.command_message("no-permission").into()
    };
    colored_label(&format!("{}{message}", guide.chat_prefix()))
}

pub(crate) fn log(case: &str, message: impl FnOnce() -> String) {
    let active = TEST_CASE.lock().unwrap().as_deref() == Some(case);
    if active {
        mod_api::log_info(&format!("[Slimefun] [DEBUG {case}] {}", message()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_completion_matches_the_stock_cases_without_rejecting_extra_cases() {
        let argument = DebugCaseArgument;
        let texts = |prefix| {
            argument
                .suggest(None, prefix)
                .into_iter()
                .map(|entry| entry.text)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            texts(""),
            [
                CARGO_INPUT_TESTING,
                PLAYER_PROFILE_DATA,
                "slimefun_analytics"
            ]
        );
        assert_eq!(texts("PROFILE"), [PLAYER_PROFILE_DATA]);
        assert!(texts("slimefun_analytics ").is_empty());
        assert_eq!(
            argument.parse("ThirdParty_Probe ignored").unwrap(),
            "ThirdParty_Probe"
        );
    }

    #[test]
    fn diagnostic_preserves_case_and_uses_stock_localization() {
        let mut active = None;
        assert_eq!(
            set_test_case(&mut active, None, Language::CHINESE),
            "&7当前没有运行的测试项!"
        );
        assert_eq!(
            set_test_case(&mut active, Some("Slimefun_Test"), Language::CHINESE),
            "&7正在调试模式中运行该测试项: &6Slimefun_Test"
        );
        assert_eq!(active.as_deref(), Some("Slimefun_Test"));
        assert_eq!(
            set_test_case(&mut active, None, Language::CHINESE),
            "&7当前测试项: &6Slimefun_Test"
        );
        assert_eq!(
            set_test_case(&mut active, Some("DiSaBlE"), Language::CHINESE),
            "&7已禁用调试模式"
        );
        assert!(active.is_none());
    }

    #[test]
    fn both_help_aliases_and_the_vanilla_help_hook_render_the_stock_help() {
        use mod_api::{
            Command, CommandInvocation, Flow, Hooks, PermissionLevel, PlayerCommandEvent,
            RegisterCommandsEvent, testing::FakeHost,
        };

        let mut host = FakeHost::new();
        host.join(17, 1).persistent_data.insert(
            "PlayerPersisted".into(),
            mod_api::Tag::compound([("slimefun:language", mod_api::Tag::String("zh-CN".into()))]),
        );
        let slimefun = crate::Slimefun::default();
        let options = r#"language = "zh-CN""#.parse::<toml::Table>().unwrap();
        slimefun
            .guide
            .configure_languages(Some(&toml::Value::Table(options)));
        let expected = help(Language::CHINESE);

        let mut commands = Vec::new();
        slimefun.register_commands(
            &mut host,
            &mut RegisterCommandsEvent {
                commands: &mut commands,
            },
        );
        let source = CommandSource::Console {
            permission: PermissionLevel::Owners,
        };
        let dimension = host.level.level().dimension();
        for alias in ["sf", "slimefun"] {
            assert!(commands.contains(&Command::new(format!("{alias} help").as_str())));
            assert!(commands.contains(&Command::new(
                format!("{alias} debug [test:slimefun:debug_case]").as_str()
            )));
            assert!(commands.contains(&Command::new(format!("{alias} debug_fish").as_str())));

            let command = CommandInvocation::new(
                vec![alias.into(), "help".into()],
                Vec::new(),
                dimension,
                [0.0; 3],
            );
            assert_eq!(
                slimefun.run_mod_command(
                    &mut host,
                    &mut RunModCommandEvent {
                        source: &source,
                        command: &command,
                        cancelled: false,
                    },
                ),
                Flow::Cancel
            );
            assert_eq!(std::mem::take(&mut host.commands.feedback), expected);
        }

        let mut replies = Vec::new();
        assert_eq!(
            slimefun.player_command(
                &mut host,
                &mut PlayerCommandEvent {
                    player: 17,
                    name: "Tester",
                    command: "HELP Slimefun",
                    replies: &mut replies,
                    cancelled: false,
                },
            ),
            Flow::Cancel
        );
        assert_eq!(replies, expected);
    }

    #[test]
    fn help_uses_the_stock_order_and_excludes_hidden_aliases() {
        let lines = help(Language::CHINESE);
        let plain = |index: usize| {
            lines[index]
                .children
                .iter()
                .map(|child| match &child.content {
                    mod_api::text::TextContent::Literal(value) => value.as_str(),
                    _ => panic!("help uses literal text"),
                })
                .collect::<String>()
        };
        assert_eq!(lines.len(), 16);
        assert_eq!(plain(3), "/sf help 显示帮助文本");
        assert_eq!(plain(15), "/sf charge 为你手持的物品充电");
    }
}
