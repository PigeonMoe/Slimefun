// SPDX-License-Identifier: GPL-3.0-only

use mod_api::Tag;

use crate::hologram_projector::colored_lore;

const BASIC: &str = "&eBasic Machine";
const MEDIUM: &str = "&aMedium Machine";
const ADVANCED: &str = "&6Advanced Machine";
const END_GAME: &str = "&4End-Game Machine";
const GRINDING: &str = "&fWorks as an Ore Crusher and Grind Stone";
const PULVERIZING: &str = "&fPulverizes Ingots into Dust";
const SMELTING: &str = "&4Alloys-Only, doesn't smelt Dust into Ingots";
const WATER_COOLING: &str = "&8⇨ &bMust be surrounded by Water";

// The published ratings come from SlimefunItems and LoreBuilder. They describe
// the stock items and are independent of the server's current ticker settings.
pub(crate) fn lore(id: &str) -> Option<Tag> {
    match id {
        "CHARGING_BENCH" => {
            return Some(lines(&[
                "",
                "&fCharges Items such as Jetpacks",
                "",
                BASIC,
                "&8⇨ &e⚡ &7128 J Buffer",
                "&8⇨ &e⚡ &7Energy Loss: &c50%",
            ]));
        }
        "REFINERY" => return Some(lines(&["", "&fRefines Oil to create Fuel"])),
        "NUCLEAR_REACTOR" => {
            return Some(lines(&[
                "",
                "&fRequires Cooling!",
                WATER_COOLING,
                "&8⇨ &bMust be supplied with Reactor Coolant Cells",
                "",
                "&4End-Game Generator",
                "&8⇨ &e⚡ &716384 J Buffer",
                "&8⇨ &e⚡ &7500 J/s",
            ]));
        }
        "NETHERSTAR_REACTOR" => {
            return Some(lines(&[
                "",
                "&fRuns on Nether Stars",
                WATER_COOLING,
                "&8⇨ &bMust be supplied with Nether Ice Coolant Cells",
                "",
                "&4End-Game Generator",
                "&8⇨ &e⚡ &732768 J Buffer",
                "&8⇨ &e⚡ &71024 J/s",
                "&8⇨ &4Causes nearby Entities to get Withered",
            ]));
        }
        "REACTOR_ACCESS_PORT" => {
            return Some(lines(&[
                "",
                "&fAllows you to interact with a Reactor",
                "&fvia Cargo Nodes, can also be used",
                "&fas a Buffer",
                "",
                "&8⇨ &eMust be placed &a3 Blocks &eabove the Reactor",
            ]));
        }
        "XP_COLLECTOR" => {
            return Some(lines(&[
                "",
                "&fCollects nearby Exp and stores it",
                "",
                END_GAME,
                "&8⇨ &e⚡ &71024 J Buffer",
                "&8⇨ &e⚡ &720 J/s",
            ]));
        }
        "FLUID_PUMP" => return Some(lines(&["", ADVANCED, "&8⇨ &e⚡ &732 J/Block"])),
        "IRON_GOLEM_ASSEMBLER" => return Some(assembler_lore("&8⇨ &e⚡ &72048 J/Golem")),
        "WITHER_ASSEMBLER" => return Some(assembler_lore("&8⇨ &e⚡ &74096 J/Wither")),
        _ => {}
    }

    let (tier, speed, joules_per_second) = match id {
        "ELECTRIC_FURNACE" => (BASIC, 1, 4),
        "ELECTRIC_FURNACE_2" => (MEDIUM, 2, 6),
        "ELECTRIC_FURNACE_3" => (MEDIUM, 4, 10),
        "ELECTRIC_ORE_GRINDER" => (ADVANCED, 1, 12),
        "ELECTRIC_ORE_GRINDER_2" => (END_GAME, 4, 30),
        "ELECTRIC_ORE_GRINDER_3" => (END_GAME, 10, 90),
        "ELECTRIC_INGOT_PULVERIZER" => (MEDIUM, 1, 14),
        "AUTO_DRIER" => (MEDIUM, 1, 10),
        "AUTO_BREWER" => (MEDIUM, 1, 12),
        "HEATED_PRESSURE_CHAMBER" => (END_GAME, 1, 10),
        "HEATED_PRESSURE_CHAMBER_2" => (END_GAME, 5, 44),
        "ELECTRIC_SMELTERY" => (END_GAME, 1, 20),
        "ELECTRIC_SMELTERY_2" => (END_GAME, 3, 40),
        "ELECTRIC_PRESS" => (MEDIUM, 1, 16),
        "ELECTRIC_PRESS_2" => (END_GAME, 3, 40),
        "ELECTRIFIED_CRUCIBLE" => (END_GAME, 1, 48),
        "ELECTRIFIED_CRUCIBLE_2" => (END_GAME, 2, 80),
        "ELECTRIFIED_CRUCIBLE_3" => (END_GAME, 4, 120),
        "ELECTRIC_GOLD_PAN" => (BASIC, 1, 2),
        "ELECTRIC_GOLD_PAN_2" => (BASIC, 3, 4),
        "ELECTRIC_GOLD_PAN_3" => (END_GAME, 10, 14),
        "ELECTRIC_DUST_WASHER" => (BASIC, 1, 6),
        "ELECTRIC_DUST_WASHER_2" => (BASIC, 2, 10),
        "ELECTRIC_DUST_WASHER_3" => (END_GAME, 10, 30),
        "ELECTRIC_INGOT_FACTORY" => (BASIC, 1, 8),
        "ELECTRIC_INGOT_FACTORY_2" => (BASIC, 2, 14),
        "ELECTRIC_INGOT_FACTORY_3" => (END_GAME, 8, 40),
        "FREEZER" => (ADVANCED, 1, 18),
        "FREEZER_2" => (END_GAME, 2, 30),
        "FREEZER_3" => (END_GAME, 3, 42),
        "CARBON_PRESS" => (END_GAME, 1, 20),
        "CARBON_PRESS_2" => (END_GAME, 3, 50),
        "CARBON_PRESS_3" => (END_GAME, 15, 180),
        _ => return generator_lore(id),
    };
    let description = match id {
        "ELECTRIC_ORE_GRINDER" | "ELECTRIC_ORE_GRINDER_2" | "ELECTRIC_ORE_GRINDER_3" => {
            Some(GRINDING)
        }
        "ELECTRIC_INGOT_PULVERIZER" => Some(PULVERIZING),
        "ELECTRIC_SMELTERY" | "ELECTRIC_SMELTERY_2" => Some(SMELTING),
        _ => None,
    };
    let mut lore = vec![colored_lore("")];
    if let Some(description) = description {
        lore.extend([colored_lore(description), colored_lore("")]);
    }
    lore.extend([
        colored_lore(tier),
        colored_lore(&format!("&8⇨ &b⚡ &7Speed: &b{speed}.0x")),
    ]);
    if matches!(id, "FREEZER" | "FREEZER_2" | "FREEZER_3") {
        lore.push(colored_lore("&8⇨ &e⚡ &7256 J Buffer"));
    }
    lore.push(colored_lore(&format!("&8⇨ &e⚡ &7{joules_per_second} J/s")));
    Some(Tag::List(lore))
}

fn assembler_lore(assembly_cost: &str) -> Tag {
    lines(&[
        "",
        END_GAME,
        "&8⇨ &7Cooldown: &b30 Seconds",
        "&8⇨ &e⚡ &74096 J Buffer",
        assembly_cost,
    ])
}

fn generator_lore(id: &str) -> Option<Tag> {
    let (tier, buffer, power) = match id {
        "SOLAR_GENERATOR" => ("&eBasic Generator", 0, 4),
        "SOLAR_GENERATOR_2" => ("&aMedium Generator", 0, 16),
        "SOLAR_GENERATOR_3" => ("&4End-Game Generator", 0, 64),
        "SOLAR_GENERATOR_4" => {
            return Some(lines(&[
                "",
                "&9Works at Night",
                "",
                "&4End-Game Generator",
                "&8⇨ &e⚡ &70 J Buffer",
                "&8⇨ &e⚡ &7256 J/s (Day)",
                "&8⇨ &e⚡ &7128 J/s (Night)",
            ]));
        }
        "COAL_GENERATOR" => ("&6Average Generator", 64, 16),
        "COAL_GENERATOR_2" => ("&6Advanced Generator", 256, 30),
        "LAVA_GENERATOR" => ("&6Average Generator", 512, 20),
        "LAVA_GENERATOR_2" => ("&6Advanced Generator", 1024, 40),
        "BIO_REACTOR" => ("&6Average Generator", 128, 8),
        "MAGNESIUM_GENERATOR" => ("&aMedium Generator", 128, 36),
        "COMBUSTION_REACTOR" => ("&6Advanced Generator", 256, 24),
        _ => return None,
    };
    Some(lines(&[
        "",
        tier,
        &format!("&8⇨ &e⚡ &7{buffer} J Buffer"),
        &format!("&8⇨ &e⚡ &7{power} J/s"),
    ]))
}

fn lines(lore: &[&str]) -> Tag {
    Tag::List(lore.iter().copied().map(colored_lore).collect())
}

pub(crate) fn localize(line: &str) -> Option<String> {
    if let Some(speed) = line.strip_prefix("§8⇨ §b⚡ §7Speed: §b") {
        speed.strip_suffix('x')?.parse::<f32>().ok()?;
        return Some(format!("&8⇨ &b⚡ &7速度: &b{speed}"));
    }
    if let Some(buffer) = line
        .strip_prefix("§8⇨ §e⚡ §7")
        .and_then(|rest| rest.strip_suffix(" J Buffer"))
    {
        buffer.parse::<u32>().ok()?;
        return Some(format!("&8⇨ &e⚡ &7{buffer} J 缓冲"));
    }
    Some(
        match line {
            "§eBasic Machine" => "&e基础机器",
            "§aMedium Machine" => "&a中级机器",
            "§6Advanced Machine" => "&6高级机器",
            "§4End-Game Machine" => "&4终局机器",
            "§eBasic Generator" => "&e基础发电机",
            "§6Average Generator" => "&6普通发电机",
            "§aMedium Generator" => "&a中级发电机",
            "§6Advanced Generator" => "&6高级发电机",
            "§4End-Game Generator" => "&4终局发电机",
            "§9Works at Night" => "&9夜间也能发电",
            "§8⇨ §e⚡ §7256 J/s (Day)" => "&8⇨ &e⚡ &7256 J/s（白天）",
            "§8⇨ §e⚡ §7128 J/s (Night)" => "&8⇨ &e⚡ &7128 J/s（夜间）",
            "§fAllows you to interact with a Reactor" => "&f可通过货运节点与反应堆交互，",
            "§fvia Cargo Nodes, can also be used" => "&f也可用作",
            "§fas a Buffer" => "&f缓冲库存",
            "§8⇨ §eMust be placed §a3 Blocks §eabove the Reactor" => {
                "&8⇨ &e必须放在反应堆正上方 &a3 格 &e处"
            }
            _ => return None,
        }
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::{lines, lore};
    use crate::hologram_projector::colored_lore;
    use crate::items::Kind;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn generator_and_reactor_port_items_keep_the_paper_lines_and_chinese_descriptions() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "daylight_detector",
                    "player_head",
                    "lime_terracotta",
                    "cyan_terracotta",
                ]
                .map(|item| Identifier::parse(&format!("minecraft:{item}")).unwrap()),
            )
            .unwrap();
        let check = |id: &str, original: &[&str], translated: &[&str]| {
            assert_eq!(lore(id), Some(lines(original)), "{id}");
            let kind = crate::guide::command_item(id).unwrap().unwrap();
            let stack = kind.stack(&host, 1).unwrap();
            let components = stack.components().unwrap();
            assert_eq!(crate::items::item_id(&stack).as_deref(), Some(id));
            assert_eq!(
                components.as_nbt().get(&"minecraft:lore".into()),
                Some(&lines(translated)),
                "{id}"
            );
        };

        for (id, tier, translated, buffer, power) in [
            ("SOLAR_GENERATOR", "&eBasic Generator", "&e基础发电机", 0, 4),
            (
                "SOLAR_GENERATOR_2",
                "&aMedium Generator",
                "&a中级发电机",
                0,
                16,
            ),
            (
                "SOLAR_GENERATOR_3",
                "&4End-Game Generator",
                "&4终局发电机",
                0,
                64,
            ),
            (
                "COAL_GENERATOR",
                "&6Average Generator",
                "&6普通发电机",
                64,
                16,
            ),
            (
                "COAL_GENERATOR_2",
                "&6Advanced Generator",
                "&6高级发电机",
                256,
                30,
            ),
            (
                "LAVA_GENERATOR",
                "&6Average Generator",
                "&6普通发电机",
                512,
                20,
            ),
            (
                "LAVA_GENERATOR_2",
                "&6Advanced Generator",
                "&6高级发电机",
                1024,
                40,
            ),
            ("BIO_REACTOR", "&6Average Generator", "&6普通发电机", 128, 8),
            (
                "MAGNESIUM_GENERATOR",
                "&aMedium Generator",
                "&a中级发电机",
                128,
                36,
            ),
            (
                "COMBUSTION_REACTOR",
                "&6Advanced Generator",
                "&6高级发电机",
                256,
                24,
            ),
        ] {
            let output = format!("&8⇨ &e⚡ &7{power} J/s");
            check(
                id,
                &["", tier, &format!("&8⇨ &e⚡ &7{buffer} J Buffer"), &output],
                &[
                    "",
                    translated,
                    &format!("&8⇨ &e⚡ &7{buffer} J 缓冲"),
                    &output,
                ],
            );
        }
        check(
            "SOLAR_GENERATOR_4",
            &[
                "",
                "&9Works at Night",
                "",
                "&4End-Game Generator",
                "&8⇨ &e⚡ &70 J Buffer",
                "&8⇨ &e⚡ &7256 J/s (Day)",
                "&8⇨ &e⚡ &7128 J/s (Night)",
            ],
            &[
                "",
                "&9夜间也能发电",
                "",
                "&4终局发电机",
                "&8⇨ &e⚡ &70 J 缓冲",
                "&8⇨ &e⚡ &7256 J/s（白天）",
                "&8⇨ &e⚡ &7128 J/s（夜间）",
            ],
        );
        check(
            "REACTOR_ACCESS_PORT",
            &[
                "",
                "&fAllows you to interact with a Reactor",
                "&fvia Cargo Nodes, can also be used",
                "&fas a Buffer",
                "",
                "&8⇨ &eMust be placed &a3 Blocks &eabove the Reactor",
            ],
            &[
                "",
                "&f可通过货运节点与反应堆交互，",
                "&f也可用作",
                "&f缓冲库存",
                "",
                "&8⇨ &e必须放在反应堆正上方 &a3 格 &e处",
            ],
        );
    }

    #[test]
    fn reactor_titles_profiles_and_lore_match_paper_english_and_chinese() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();

        let cases: [(_, _, _, _, &[_], &[_]); 2] = [
            (
                "NUCLEAR_REACTOR",
                "Nuclear Reactor",
                "dark_green",
                "&2核反应堆",
                &[
                    "",
                    "&fRequires Cooling!",
                    "&8⇨ &bMust be surrounded by Water",
                    "&8⇨ &bMust be supplied with Reactor Coolant Cells",
                    "",
                    "&4End-Game Generator",
                    "&8⇨ &e⚡ &716384 J Buffer",
                    "&8⇨ &e⚡ &7500 J/s",
                ],
                &[
                    "",
                    "&r需要冷却剂!",
                    "&8⇨ &b必须被水包围",
                    "&8⇨ &b必须使用反应堆冷却剂工作",
                    "",
                    "&4终局发电机",
                    "&8⇨ &e⚡ &716384 J 缓冲",
                    "&8⇨ &e⚡ &7500 J/s",
                ],
            ),
            (
                "NETHERSTAR_REACTOR",
                "Nether Star Reactor",
                "white",
                "&r下界之星反应堆",
                &[
                    "",
                    "&fRuns on Nether Stars",
                    "&8⇨ &bMust be surrounded by Water",
                    "&8⇨ &bMust be supplied with Nether Ice Coolant Cells",
                    "",
                    "&4End-Game Generator",
                    "&8⇨ &e⚡ &732768 J Buffer",
                    "&8⇨ &e⚡ &71024 J/s",
                    "&8⇨ &4Causes nearby Entities to get Withered",
                ],
                &[
                    "",
                    "&r需要下界之星",
                    "&8⇨ &b必须被水包围",
                    "&8⇨ &b必须使用下界冰冷却剂工作",
                    "",
                    "&4终局发电机",
                    "&8⇨ &e⚡ &732768 J 缓冲",
                    "&8⇨ &e⚡ &71024 J/s",
                    "&8⇨ &4会导致附近的生物获得凋零效果",
                ],
            ),
        ];
        for (id, english_name, color, chinese_name, english_lore, chinese_lore) in cases {
            let kind = crate::guide::command_item(id).unwrap().unwrap();
            let Kind::Slimefun(template) = kind else {
                panic!("{id} is not a Slimefun item");
            };
            assert_eq!((template.name, template.color), (english_name, color));
            assert_eq!(lore(id), Some(lines(english_lore)), "{id}");

            let stack = kind.stack(&host, 1).unwrap();
            let components = stack.components().unwrap();
            let components = components.as_nbt();
            assert_eq!(crate::items::item_id(&stack).as_deref(), Some(id));
            assert_eq!(
                components.get(&"minecraft:custom_name".into()),
                Some(&colored_lore(chinese_name)),
                "{id}"
            );
            assert_eq!(
                components.get(&"minecraft:lore".into()),
                Some(&lines(chinese_lore)),
                "{id}"
            );
            assert_eq!(
                components.get(&"minecraft:profile".into()),
                Some(&template.texture.as_ref().unwrap().component()),
                "{id}"
            );
        }
    }

    #[test]
    fn electric_machine_capacity_and_operation_costs_reach_inventory_stacks() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "light_blue_stained_glass",
                    "black_stained_glass",
                    "blue_terracotta",
                    "iron_block",
                    "obsidian",
                    "player_head",
                ]
                .map(|item| Identifier::parse(&format!("minecraft:{item}")).unwrap()),
            )
            .unwrap();
        for (id, expected_lines, last_line) in [
            ("FREEZER", 5, "&8⇨ &e⚡ &718 J/s"),
            ("FREEZER_2", 5, "&8⇨ &e⚡ &730 J/s"),
            ("FREEZER_3", 5, "&8⇨ &e⚡ &742 J/s"),
            ("CARBON_PRESS", 4, "&8⇨ &e⚡ &720 J/s"),
            ("CARBON_PRESS_2", 4, "&8⇨ &e⚡ &750 J/s"),
            ("CARBON_PRESS_3", 4, "&8⇨ &e⚡ &7180 J/s"),
            ("XP_COLLECTOR", 6, "&8⇨ &e⚡ &720 J/s"),
            ("FLUID_PUMP", 3, "&8⇨ &e⚡ &732 J/个方块"),
            ("IRON_GOLEM_ASSEMBLER", 5, "&8⇨ &e⚡ &72048 J/个铁傀儡"),
            ("WITHER_ASSEMBLER", 5, "&8⇨ &e⚡ &74096 J/个凋灵"),
        ] {
            let kind = crate::guide::command_item(id).unwrap().unwrap();
            let stack = kind.stack(&host, 1).unwrap();
            let components = stack.components().unwrap();
            let Some(mod_api::Tag::List(lore)) = components.as_nbt().get(&"minecraft:lore".into())
            else {
                panic!("missing Paper lore on {id}");
            };
            assert_eq!(lore.len(), expected_lines, "{id}");
            assert_eq!(lore.last(), Some(&colored_lore(last_line)), "{id}");
            if id.starts_with("FREEZER") {
                assert_eq!(lore[3], colored_lore("&8⇨ &e⚡ &7256 J 缓冲"), "{id}");
            }
        }
    }
}
