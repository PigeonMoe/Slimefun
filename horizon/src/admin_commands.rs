// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    CommandArgumentValue, CommandInvocation, CommandSource, Host, HostTransaction, ItemStack,
    PlayerRef, Players, ServerHost, Text,
};

use crate::{factory::charging_bench, guide, items, recipes, research, ticker_policy::Tickers};

pub(crate) fn operator(host: &dyn Host, player: PlayerRef) -> bool {
    player.permission_level(host).is_some_and(|level| level > 0)
}

pub(crate) fn teleporter(
    host: &mut dyn ServerHost,
    source: &CommandSource,
    command: &CommandInvocation,
    transport: &crate::gps_transport::Transport,
) -> Result<(), String> {
    let CommandSource::Player { player, .. } = *source else {
        return Err("这个指令只能在游戏内使用。".into());
    };
    if !operator(host, player) {
        return Err("你没有权限使用此命令。".into());
    }
    let owner = waypoint_owner(host.players(), player.uuid(), command.argument("target"))?;
    transport
        .open_admin(host, player, owner)
        .map_err(str::to_owned)
}

fn waypoint_owner(
    players: &dyn Players,
    caller: u128,
    target: Option<&CommandArgumentValue>,
) -> Result<u128, String> {
    let Some(CommandArgumentValue::String(name)) = target else {
        return Ok(caller);
    };
    players
        .known_player_uuid(name)
        .ok_or_else(|| format!("未知玩家：{name}"))
}

pub(crate) fn charge(host: &mut dyn ServerHost, source: &CommandSource) -> &'static str {
    let CommandSource::Player { player, .. } = *source else {
        return "只有玩家可以为手中的物品充电。";
    };
    if !operator(host, player) {
        return "你没有权限使用此命令。";
    }
    let Some(slot) = player.selected_slot(host).filter(|slot| *slot < 9) else {
        return "无法读取主手物品。";
    };
    let charged = player.inventory_slots(host).and_then(|slots| {
        let (_, held) = slots.into_iter().find(|(id, _)| *id == slot as u32)?;
        charging_bench::fully_charge(host, &held)
    });
    let Some(charged) = charged else {
        return "主手物品无法充电。";
    };
    match player.set_inventory_slot(host, slot as u32, Some(charged)) {
        Ok(()) => "已将主手物品充满电。",
        Err(_) => "远古工艺无法更新主手物品。",
    }
}

pub(crate) fn give(
    host: &mut dyn ServerHost,
    source: &CommandSource,
    command: &CommandInvocation,
    drop_excess: bool,
) -> String {
    let authorized = match *source {
        CommandSource::Player { player, .. } => operator(host, player),
        _ => source.permission().level() >= 2,
    };
    if !authorized {
        return "你没有权限使用此命令。".into();
    }
    let (Some(CommandArgumentValue::String(target)), Some(CommandArgumentValue::String(id))) =
        (command.argument("target"), command.argument("item"))
    else {
        return "用法：/sf give <玩家名> <物品 ID> [数量]".into();
    };
    let Some(player) = host.players().player_by_name(target) else {
        return format!("玩家 {target} 当前不在线。");
    };
    let id = id.to_ascii_uppercase();
    let kind = match guide::command_item(&id) {
        Some(Ok(kind)) => kind,
        Some(Err(message)) => return message.into(),
        None => return format!("没有找到远古工艺物品 {id}。"),
    };
    let count = match command.argument("amount") {
        None => 1,
        Some(CommandArgumentValue::Integer(count)) if *count > 0 => *count as u32,
        _ => return "物品数量必须是大于零的整数。".into(),
    };
    let Some(mut stack) = kind.stack(host, 1) else {
        return format!("当前服务器无法生成物品 {id}。");
    };
    let name = guide::localized_plain_item_name(&id).unwrap_or(id);
    let Ok(remaining) = insert(host, player, &stack, count) else {
        return format!("无法写入玩家 {target} 的背包。");
    };
    let stored = count - remaining;
    if stored > 0 {
        let _ = player.tell(host, Text::literal(format!("收到 {stored} 个{name}。")));
    }
    if remaining == 0 {
        return format!("已发给 {target} {stored} 个{name}。");
    }
    if drop_excess
        && let Some((level, [x, y, z])) = player
            .entity()
            .level(host)
            .zip(player.entity().position(host))
        && stack.set_count(remaining).is_ok()
        && matches!(level.spawn_item(host, x, y, z, stack, 10), Ok(Some(_)))
    {
        return format!(
            "已发给 {target} {stored} 个{name}，其余 {remaining} 个掉落在玩家所在位置。"
        );
    }
    format!("已发给 {target} {stored} 个{name}，其余 {remaining} 个未发放。")
}

pub(crate) fn insert(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    item: &ItemStack,
    count: u32,
) -> Result<u32, ()> {
    let mut slots: [Option<ItemStack>; 36] = std::array::from_fn(|_| None);
    for (index, stack) in player.inventory_slots(host).ok_or(())? {
        if let Some(slot) = slots.get_mut(index as usize) {
            *slot = Some(stack);
        }
    }
    let (changes, remaining) = plan_slots(&slots, item, items::stack_limit(host, item), count)?;
    host.transaction(|host, _| {
        for (slot, updated) in changes {
            player
                .set_inventory_slot(host, slot, Some(updated))
                .map_err(|_| ())?;
        }
        Ok(remaining)
    })
}

fn plan_slots(
    slots: &[Option<ItemStack>; 36],
    item: &ItemStack,
    limit: u32,
    mut remaining: u32,
) -> Result<(Vec<(u32, ItemStack)>, u32), ()> {
    let mut changes = Vec::new();
    // Paper fills compatible partial stacks before it occupies empty slots.
    for (index, slot) in slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| {
            slot.as_ref()
                .is_some_and(|stored| stored.is_stackable_with(item))
        })
        .chain(slots.iter().enumerate().filter(|(_, slot)| slot.is_none()))
    {
        if remaining == 0 {
            break;
        }
        let existing = slot.as_ref().map_or(0, ItemStack::count);
        let amount = limit.saturating_sub(existing).min(remaining);
        if amount == 0 {
            continue;
        }
        let mut updated = item.clone();
        updated.set_count(existing + amount).map_err(|_| ())?;
        changes.push((index as u32, updated));
        remaining -= amount;
    }
    Ok((changes, remaining))
}

// The stock Paper ranks, in the order of config.yml.
const DEFAULT_RANKS: &[&str] = &[
    "Chicken",
    "Cow",
    "Spider",
    "Villager",
    "Beginner",
    "Clumsy Student",
    "Unskilled Student",
    "Incompetent Student",
    "Student",
    "Clumsy Apprentice",
    "Apprentice",
    "Skilled Apprentice",
    "Upcoming Mage",
    "Excellent Mage",
    "Young Magician",
    "Incompetent Magician",
    "Common Magician",
    "Magician",
    "Proficient Magician",
    "Skilled Magician",
    "Accustomed Magician",
    "Great Magician",
    "Clumsy Wizard",
    "Wizard",
    "Common Wizard",
    "Skilled Wizard",
    "Great Wizard",
    "Impressive Wizard",
    "Young Master",
    "Clumsy Master",
    "Upcoming Master",
    "Competent Master",
    "Master",
    "Skilled Master",
    "Excellent Master",
    "Great Master",
    "Proficient Master",
    "The Chosen One",
];

#[derive(Default)]
pub(crate) struct ResearchRanks(Mutex<Vec<String>>);

impl ResearchRanks {
    pub(crate) fn configure(&self, value: Option<&toml::Value>) {
        let ranks = value.and_then(toml::Value::as_array).and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect()
        });
        *self.0.lock().unwrap() = ranks.unwrap_or_default();
    }

    fn title(&self, unlocked: usize, total: usize) -> String {
        let ranks = self.0.lock().unwrap();
        let index = |len: usize| {
            if total == 0 {
                0
            } else {
                unlocked * (len - 1) / total
            }
        };
        if ranks.is_empty() {
            DEFAULT_RANKS[index(DEFAULT_RANKS.len())].into()
        } else {
            ranks[index(ranks.len())].clone()
        }
    }

    pub(crate) fn stats(
        &self,
        host: &dyn ServerHost,
        source: &CommandSource,
        command: &CommandInvocation,
    ) -> Vec<String> {
        let player = match command.argument("target") {
            Some(CommandArgumentValue::String(target)) => {
                let authorized = match *source {
                    CommandSource::Player { player, .. } => operator(host, player),
                    CommandSource::Console { .. } => true,
                    _ => false,
                };
                if !authorized {
                    return vec!["你没有权限查看其他玩家的研究统计。".into()];
                }
                let Some(player) = host.players().player_by_name(target) else {
                    return vec![format!("玩家 {target} 当前不在线。")];
                };
                player
            }
            _ => {
                let CommandSource::Player { player, .. } = *source else {
                    return vec!["用法：/sf stats <玩家名>".into()];
                };
                player
            }
        };
        let Some(name) = player.name(host) else {
            return vec!["无法读取玩家的研究统计。".into()];
        };
        let Some(data) = player.persistent_data(host) else {
            return vec!["无法读取玩家的研究统计。".into()];
        };
        let mut topics = BTreeMap::new();
        for topic in recipes::researches().chain(guide::research_topics()) {
            if !topic.id.is_empty() {
                topics.entry(topic.id).or_insert(topic);
            }
        }
        let tickers = Tickers::current();
        let (mut unlocked, mut total, mut cost) = (0, 0, 0);
        for topic in topics.into_values() {
            let researched = research::unlocked(&data, topic);
            if researched {
                cost += topic.cost;
            }
            if guide::research_has_enabled_items(topic.id, tickers) {
                total += 1;
                unlocked += usize::from(researched);
            }
        }
        let percent = if total == 0 {
            0.0
        } else {
            unlocked as f64 * 100.0 / total as f64
        };
        vec![
            format!("玩家 {name} 的研究统计"),
            format!("称号：{}", self.title(unlocked, total)),
            format!("研究进度：{percent:.2}%（{unlocked} / {total}）"),
            format!("已解锁研究的经验等级合计：{cost}"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Slimefun;
    use mod_api::{
        Command, CommandArgument, Flow, Hooks, Identifier, PermissionLevel, RegisterCommandsEvent,
        RunModCommandEvent, testing::FakeHost,
    };

    #[test]
    fn both_command_aliases_keep_their_authority_and_route_feedback() {
        let slimefun = Slimefun::default();
        let mut host = FakeHost::new();
        let mut commands = Vec::new();
        slimefun.register_commands(
            &mut host,
            &mut RegisterCommandsEvent {
                commands: &mut commands,
            },
        );
        let console = CommandSource::Console {
            permission: PermissionLevel::Owners,
        };
        let function = CommandSource::Function {
            id: Identifier::parse("test:no_operator").unwrap(),
            permission: PermissionLevel::All,
        };
        let dimension = host.level.level().dimension();

        for alias in ["sf", "slimefun"] {
            assert!(commands.contains(&Command::op(&format!("{alias} charge"), 1)));
            assert!(commands.contains(&Command::op(
                &format!("{alias} give <target:word> <item:word> [amount:integer]"),
                1,
            )));
            assert!(commands.contains(&Command::new(&format!("{alias} stats"))));
            assert!(commands.contains(&Command::op(&format!("{alias} stats <target:word>"), 1)));
            assert!(commands.contains(&Command::op(&format!("{alias} versions"), 1)));
            assert!(commands.contains(&Command::op(
                &format!("{alias} teleporter [target:word]"),
                1,
            )));

            for (subcommand, arguments, source, feedback) in [
                ("charge", vec![], &console, "只有玩家可以为手中的物品充电。"),
                ("give", vec![], &function, "你没有权限使用此命令。"),
                ("versions", vec![], &function, "你没有权限使用此命令。"),
                ("teleporter", vec![], &console, "这个指令只能在游戏内使用。"),
                (
                    "give",
                    vec![],
                    &console,
                    "用法：/sf give <玩家名> <物品 ID> [数量]",
                ),
                ("stats", vec![], &console, "用法：/sf stats <玩家名>"),
                (
                    "stats",
                    vec![CommandArgument::new(
                        "target".into(),
                        CommandArgumentValue::String("Someone".into()),
                    )],
                    &function,
                    "你没有权限查看其他玩家的研究统计。",
                ),
            ] {
                let command = CommandInvocation::new(
                    vec![alias.into(), subcommand.into()],
                    arguments,
                    dimension,
                    [0.0; 3],
                );
                let outcome = slimefun.run_mod_command(
                    &mut host,
                    &mut RunModCommandEvent {
                        source,
                        command: &command,
                        cancelled: false,
                    },
                );
                assert_eq!(outcome, Flow::Cancel);
                assert_eq!(host.commands.feedback.pop(), Some(Text::literal(feedback)));
            }
        }
    }

    #[test]
    fn a_named_teleporter_loads_the_existing_offline_account_uuid() {
        struct AccountCache;
        impl Players for AccountCache {
            fn known_player_uuid(&self, name: &str) -> Option<u128> {
                name.eq_ignore_ascii_case("PreviousGuest").then_some(42)
            }
        }

        let named = CommandArgumentValue::String("previousguest".into());
        assert_eq!(waypoint_owner(&AccountCache, 15, None), Ok(15));
        assert_eq!(waypoint_owner(&AccountCache, 15, Some(&named)), Ok(42));
        let unknown = CommandArgumentValue::String("NoMatchingAccount".into());
        assert_eq!(
            waypoint_owner(&AccountCache, 15, Some(&unknown)),
            Err("未知玩家：NoMatchingAccount".into())
        );
    }

    #[test]
    fn give_merges_only_matching_paper_stacks_before_using_empty_slots() {
        let mut host = FakeHost::new();
        let kind = guide::command_item("FILLED_FLASK_OF_KNOWLEDGE")
            .unwrap()
            .unwrap();
        host.registries
            .resolve_items([Identifier::parse(kind.material()).unwrap()])
            .unwrap();
        let prototype = kind.stack(&host, 1).unwrap();
        let mut unrelated = ItemStack::of(prototype.item());
        unrelated.set_count(2).unwrap();
        let mut slots = std::array::from_fn(|_| Some(unrelated.clone()));
        slots[0] = None;
        slots[2] = Some(kind.stack(&host, 63).unwrap());

        let (changes, remaining) = plan_slots(&slots, &prototype, 64, 66).unwrap();
        assert_eq!(remaining, 1);
        assert_eq!(
            changes.iter().map(|(slot, _)| *slot).collect::<Vec<_>>(),
            [2, 0]
        );
        assert!(changes.iter().all(|(_, stack)| {
            stack.count() == 64
                && stack.is_stackable_with(&prototype)
                && items::item_id(stack).as_deref() == Some(kind.id())
        }));
        assert!(guide::command_item("GRIND_STONE").unwrap().is_err());
        assert!(guide::command_item("DIAMOND_BLOCK_NOT_A_SLIMEFUN_ID").is_none());
    }

    #[test]
    fn research_ranks_use_papers_truncated_fraction() {
        let ranks = ResearchRanks::default();
        assert_eq!(ranks.title(0, 0), DEFAULT_RANKS[0]);
        assert_eq!(
            ranks.title(1, 2),
            DEFAULT_RANKS[(DEFAULT_RANKS.len() - 1) / 2]
        );
        assert_eq!(ranks.title(100, 100), "The Chosen One");
        let config = "research-ranks = [\"甲\", \"乙\", \"丙\"]"
            .parse::<toml::Table>()
            .unwrap();
        ranks.configure(config.get("research-ranks"));
        assert_eq!(ranks.title(1, 2), "乙");
        ranks.configure(None);
        assert_eq!(ranks.title(100, 100), "The Chosen One");
    }
}
