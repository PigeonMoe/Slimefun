// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{CommandArgumentValue, CommandInvocation, CommandSource, LevelRef, ServerHost};

use crate::{admin_commands, guide};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Order {
    #[default]
    Highest,
    Average,
    Lowest,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    order: Order,
    verbose: bool,
}

impl Options {
    fn parse(flags: &str, console: bool) -> Result<Self, Vec<String>> {
        let mut options = Self::default();
        let mut errors = Vec::new();
        for flag in flags.split_whitespace().map(str::to_ascii_lowercase) {
            match flag.as_str() {
                "--verbose" => options.verbose = true,
                "--avg" => options.order = Order::Average,
                "--low" if options.order == Order::Highest => options.order = Order::Lowest,
                "--low" => {}
                other if other.starts_with("--") => errors.push(format!("未知标记：{other}")),
                _ => {}
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        if options.verbose && !console {
            return Err(vec!["只有控制台可以使用 --verbose。".into()]);
        }
        Ok(options)
    }
}

type Position = [i32; 3];

pub(crate) struct Cycle {
    pub(crate) dimension: String,
    pub(crate) round: i64,
    locations: BTreeMap<Position, (String, u64)>,
}

impl Cycle {
    pub(crate) fn for_level(host: &dyn ServerHost, level: LevelRef, interval: u32) -> Option<Self> {
        if !level.in_place(host) {
            return None;
        }
        let round = round_at(level.game_time(host)?, interval)?;
        let dimension = level.dimension().id(host)?;
        Some(Self::new(dimension, round))
    }

    fn new(dimension: String, round: i64) -> Self {
        Self {
            dimension,
            round,
            locations: BTreeMap::new(),
        }
    }

    pub(crate) fn measure<R>(
        &mut self,
        host: &mut dyn ServerHost,
        id: &str,
        pos: Position,
        operation: impl FnOnce(&mut dyn ServerHost) -> R,
    ) -> (R, u64) {
        let start = host.server_monotonic_nanos();
        let result = operation(host);
        let elapsed = host.server_monotonic_nanos().saturating_sub(start);
        self.record(id, pos, elapsed);
        (result, elapsed)
    }

    pub(crate) fn record(&mut self, id: &str, pos: Position, elapsed: u64) {
        let (_, total) = self
            .locations
            .entry(pos)
            .or_insert_with(|| (id.to_owned(), 0));
        *total = total.saturating_add(elapsed);
    }

    pub(crate) fn record_parent(
        &mut self,
        host: &dyn ServerHost,
        id: &str,
        pos: Position,
        start: u64,
        children: u64,
    ) {
        let elapsed = host.server_monotonic_nanos().saturating_sub(start);
        self.record(id, pos, elapsed.saturating_sub(children));
    }
}

fn round_at(time: i64, interval: u32) -> Option<i64> {
    let interval = i64::from(interval.max(1));
    (time.rem_euclid(interval) == 0).then_some(time)
}

#[derive(Default)]
struct State {
    running: Vec<Cycle>,
    completed: Vec<Cycle>,
}

#[derive(Default)]
pub(crate) struct Profiler(Mutex<State>);

impl Profiler {
    pub(crate) fn begin_tick(&self) {
        self.0.lock().unwrap().running.clear();
    }

    pub(crate) fn collect(&self, cycle: Cycle) {
        self.0.lock().unwrap().running.push(cycle);
    }

    pub(crate) fn finish_tick(&self) {
        let mut state = self.0.lock().unwrap();
        if !state.running.is_empty() {
            state.completed = std::mem::take(&mut state.running);
        }
    }

    pub(crate) fn clear(&self) {
        *self.0.lock().unwrap() = State::default();
    }

    pub(crate) fn report(
        &self,
        host: &dyn ServerHost,
        source: &CommandSource,
        command: &CommandInvocation,
        interval: u32,
    ) -> Vec<String> {
        let authorized = match *source {
            CommandSource::Player { player, .. } => admin_commands::operator(host, player),
            _ => source.permission().level() > 0,
        };
        if !authorized {
            return vec!["你没有权限使用此命令。".into()];
        }
        let flags = match command.argument("flags") {
            Some(CommandArgumentValue::String(flags)) => flags,
            _ => "",
        };
        let options = match Options::parse(flags, matches!(source, CommandSource::Console { .. })) {
            Ok(options) => options,
            Err(errors) => return errors,
        };
        summarize(&self.0.lock().unwrap().completed, options, interval)
    }
}

#[derive(Default)]
struct Group {
    blocks: usize,
    elapsed: u128,
}

impl Group {
    fn add(&mut self, elapsed: u64) {
        self.blocks += 1;
        self.elapsed += u128::from(elapsed);
    }

    fn average(&self) -> u128 {
        self.elapsed / self.blocks.max(1) as u128
    }
}

fn summarize(cycles: &[Cycle], options: Options, interval: u32) -> Vec<String> {
    if cycles.is_empty() {
        return vec!["尚无已完成的远古工艺计时采样。".into()];
    }

    let (mut items, mut chunks) = (BTreeMap::new(), BTreeMap::new());
    let mut total = Group::default();
    for cycle in cycles {
        for (&[x, _, z], (id, elapsed)) in &cycle.locations {
            items
                .entry(id.clone())
                .or_insert_with(Group::default)
                .add(*elapsed);
            chunks
                .entry(format!(
                    "{} ({}, {})",
                    cycle.dimension,
                    x.div_euclid(16),
                    z.div_euclid(16)
                ))
                .or_insert_with(Group::default)
                .add(*elapsed);
            total.add(*elapsed);
        }
    }

    let mut lines = vec![
        "远古工艺性能报告 · 最近一轮已完成的采样".into(),
        "采样范围包括网络设备、发电与货运节点，以及彩虹方块换色。".into(),
        format!(
            "设备周期 {interval} 刻 · {} 个已采样方块 · 总耗时 {}",
            total.blocks,
            millis(total.elapsed)
        ),
    ];
    if total.blocks > 0 {
        section(&mut lines, "物品", items, &options, true);
        section(&mut lines, "区块", chunks, &options, false);
        section(
            &mut lines,
            "插件",
            BTreeMap::from([("远古工艺".into(), total)]),
            &options,
            false,
        );
    }
    lines
}

fn section(
    lines: &mut Vec<String>,
    label: &str,
    groups: BTreeMap<String, Group>,
    options: &Options,
    items: bool,
) {
    lines.push(format!("{} 个{label}", groups.len()));
    let mut rows: Vec<_> = groups.into_iter().collect();
    let weight = |entry: &(String, Group)| {
        if items && options.order == Order::Average {
            entry.1.average()
        } else {
            entry.1.elapsed
        }
    };
    rows.sort_by(|left, right| {
        let ordering = weight(left).cmp(&weight(right));
        let ordering = if options.order == Order::Lowest {
            ordering
        } else {
            ordering.reverse()
        };
        ordering.then_with(|| left.0.cmp(&right.0))
    });
    let mut shown = 0;
    let mut hidden = 0;
    for (id, group) in rows {
        if !options.verbose && (shown >= 20 || (shown >= 6 && group.elapsed <= 260_000)) {
            hidden += 1;
            continue;
        }
        shown += 1;
        let timing = match (items, group.blocks > 1, options.order) {
            (true, true, Order::Average) => {
                format!(
                    "平均 {} · 共 {}",
                    millis(group.average()),
                    millis(group.elapsed)
                )
            }
            (true, true, _) => {
                format!(
                    "{} · 平均 {}",
                    millis(group.elapsed),
                    millis(group.average())
                )
            }
            _ => millis(group.elapsed),
        };
        let name = items.then(|| guide::localized_item_name(&id)).flatten();
        let label = match name {
            Some(name) if name != id => format!("{name}（{id}）"),
            _ => id,
        };
        lines.push(format!("  {label} · {} 个方块 · {timing}", group.blocks));
    }
    if hidden > 0 {
        lines.push(format!("  另有 {hidden} 项"));
    }
}

fn millis(nanos: u128) -> String {
    format!("{:.3} ms", nanos as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_match_papers_precedence_and_report_all_unknown_flags() {
        for flags in ["--AVG --Low", "--Low --AVG"] {
            assert_eq!(Options::parse(flags, true).unwrap().order, Order::Average);
        }
        assert_eq!(
            Options::parse("--low words", true).unwrap().order,
            Order::Lowest
        );
        assert!(Options::parse("--verbose", true).unwrap().verbose);
        assert!(Options::parse("--verbose", false).is_err());
        assert_eq!(
            Options::parse("--Verbose --ZZ --x", false).unwrap_err(),
            ["未知标记：--zz", "未知标记：--x"],
        );
    }

    #[test]
    fn the_shared_cycle_uses_the_configured_interval_on_the_signed_server_clock() {
        assert_eq!(round_at(10, 10), Some(10));
        assert_eq!(round_at(10, 25), None);
        assert_eq!(round_at(40, 25), None);
        assert_eq!(round_at(25, 25), Some(25));
        assert_eq!(round_at(50, 25), Some(50));
        assert_eq!(round_at(-25, 25), Some(-25));
    }

    #[test]
    fn reports_order_genuine_aggregates_and_never_publish_a_running_cycle() {
        let profiler = Profiler::default();
        let mut cycle = Cycle::new("minecraft:the_nether".into(), 1);
        cycle.record("ENERGY_REGULATOR", [-1, 63, -16], 2_000_000);
        cycle.record("ENERGY_REGULATOR", [-1, 63, -16], 1_000_000);
        cycle.record("ENERGY_REGULATOR", [-2, 63, -16], 3_000_000);
        cycle.record("CARGO_MANAGER", [0, 64, 0], 4_000_000);
        profiler.collect(cycle);
        assert!(profiler.0.lock().unwrap().completed.is_empty());
        profiler.finish_tick();
        let state = profiler.0.lock().unwrap();
        let report = |order| {
            summarize(
                &state.completed,
                Options {
                    order,
                    verbose: false,
                },
                10,
            )
        };
        let highest = report(Order::Highest).join("\n");
        let highest_chunks = &highest[highest.find("2 个区块").unwrap()..];
        assert!(highest.find("ENERGY_REGULATOR").unwrap() < highest.find("CARGO_MANAGER").unwrap());
        assert!(highest_chunks.contains("minecraft:the_nether (-1, -1) · 2 个方块 · 6.000 ms"));
        assert!(highest.contains("3 个已采样方块 · 总耗时 10.000 ms"));
        let average = report(Order::Average).join("\n");
        assert!(average.find("CARGO_MANAGER").unwrap() < average.find("ENERGY_REGULATOR").unwrap());
        assert!(average.contains("平均 3.000 ms · 共 6.000 ms"));
        let lowest = report(Order::Lowest).join("\n");
        assert!(lowest.find("CARGO_MANAGER").unwrap() < lowest.find("ENERGY_REGULATOR").unwrap());
    }
}
