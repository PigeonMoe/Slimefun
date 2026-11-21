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
        _ => return None,
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
        colored_lore(&format!("&8⇨ &e⚡ &7{joules_per_second} J/s")),
    ]);
    Some(Tag::List(lore))
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
            _ => return None,
        }
        .into(),
    )
}
