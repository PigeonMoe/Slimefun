// SPDX-License-Identifier: GPL-3.0-only

use std::sync::atomic::{AtomicU32, Ordering};

use mod_api::{GameMode, ItemStack, PlayerRef, ServerHost, Tag, Text};

use crate::{
    elemental_staves::MAGICAL_BOOK_COVER,
    items::{ENDER_LUMP_3, Kind, item, text},
    magic_eye::MAGIC_EYE_OF_ENDER,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    SCROLL_OF_DIMENSIONAL_TELEPOSITION,
    "paper",
    "Scroll of Dimensional Teleposition",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "scroll_of_dimensional_teleposition",
    name: "Turning things around",
    cost: 38,
};

const LUMP: Option<Kind> = Some(ENDER_LUMP_3);
const EYE: Option<Kind> = Some(MAGIC_EYE_OF_ENDER);
pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        LUMP,
        EYE,
        LUMP,
        Some(MAGICAL_BOOK_COVER),
        LUMP,
        EYE,
        LUMP,
        None,
    ],
    output: Stack {
        kind: SCROLL_OF_DIMENSIONAL_TELEPOSITION,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore() -> Tag {
    Tag::List(vec![
        Tag::String("".into()),
        text("This Scroll is capable of creating", "red", false),
        text("a temporary black Hole which pulls", "red", false),
        text("nearby Entities into itself and sends", "red", false),
        text("them into another Dimension where", "red", false),
        text("everything is turned around", "red", false),
        Tag::String("".into()),
        text(
            "In other words: Makes Entities turn by 180 Degrees",
            "white",
            false,
        ),
    ])
}

pub(crate) const DEFAULT_CONFIG: &str = "\n[scroll-of-dimensional-teleposition]\nradius = 10\n";
const RADIUS: u32 = 10;

pub(crate) struct Scroll(AtomicU32);

impl Default for Scroll {
    fn default() -> Self {
        Self(AtomicU32::new(RADIUS))
    }
}

impl Scroll {
    pub(crate) fn configure(&self, config: Option<&toml::Value>) {
        let radius = config
            .and_then(|section| section.get("radius"))
            .and_then(toml::Value::as_integer)
            .filter(|radius| (1..=i64::from(i32::MAX)).contains(radius))
            .map_or(RADIUS, |radius| radius as u32);
        self.0.store(radius, Ordering::Relaxed);
    }

    pub(crate) fn use_held(
        &self,
        host: &mut dyn ServerHost,
        caster: PlayerRef,
        stack: &ItemStack,
    ) -> bool {
        if !SCROLL_OF_DIMENSIONAL_TELEPOSITION.matches(host, stack) {
            return false;
        }
        if !research::can_use(host, caster, &RESEARCH) {
            let _ = caster.tell(
                host,
                Text::literal(format!(
                    "Unlock {} with /sf research {}.",
                    RESEARCH.name, RESEARCH.id
                )),
            );
            return true;
        }
        let (Some(level), Some(bounds)) = (caster.level(host), caster.entity().bounding_box(host))
        else {
            return true;
        };
        let radius = f64::from(self.0.load(Ordering::Relaxed));
        let search = expanded(bounds, radius);
        let Some(mut targets) = level.entities_in(host, search[0], search[1]) else {
            return true;
        };
        // The CraftEntity nearby-entity predicate includes spectators. The
        // generic Horizon intersection query leaves them out.
        for player in level.online_players(host).unwrap_or_default() {
            if player.game_mode(host) == Some(GameMode::Spectator)
                && player
                    .entity()
                    .bounding_box(host)
                    .is_some_and(|bounds| overlaps(search, bounds))
            {
                targets.push(player.entity());
            }
        }
        let Some(dimension) = level.dimension().id(host) else {
            return true;
        };
        for entity in targets {
            if entity == caster.entity()
                || host
                    .living(entity)
                    .and_then(|living| living.health(host))
                    .is_none_or(|health| health <= 0.0)
            {
                continue;
            }
            let Some(kind) = entity.kind(host) else {
                continue;
            };
            let name = kind.id(host);
            if name == "minecraft:armor_stand" {
                continue;
            }
            let (Some(position), Some([yaw, pitch])) =
                (entity.position(host), entity.rotation(host))
            else {
                continue;
            };
            let rotation = Some([turned(yaw), pitch]);
            if name == "minecraft:player" {
                if let Some(player) = entity
                    .uuid(host)
                    .and_then(|uuid| host.player(uuid))
                    .filter(|player| player.entity() == entity)
                {
                    let _ = player.teleport_checked(host, &dimension, position, rotation);
                }
            } else {
                let _ = entity.teleport_to(host, position, rotation);
            }
        }
        true
    }
}

fn expanded([min, max]: [[f64; 3]; 2], radius: f64) -> [[f64; 3]; 2] {
    [
        min.map(|value| value - radius),
        max.map(|value| value + radius),
    ]
}

fn overlaps([min, max]: [[f64; 3]; 2], [other_min, other_max]: [[f64; 3]; 2]) -> bool {
    (0..3).all(|axis| min[axis] < other_max[axis] && other_min[axis] < max[axis])
}

fn turned(yaw: f32) -> f32 {
    let yaw = yaw + 180.0;
    if yaw > 360.0 { yaw - 360.0 } else { yaw }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_is_an_expanded_body_box_and_turn_keeps_the_paper_wrap() {
        let player = [[0.7, 64.0, 0.7], [1.3, 65.8, 1.3]];
        let search = expanded(player, 10.0);
        assert!(overlaps(search, [[10.9, 75.5, 10.9], [11.5, 76.1, 11.5]]));
        assert!(!overlaps(search, [[11.3, 75.5, 11.3], [12.0, 76.1, 12.0]]));
        assert_eq!(turned(0.0), 180.0);
        assert_eq!(turned(180.0), 360.0);
        assert_eq!(turned(181.0), 1.0);
        assert_eq!(turned(-180.0), 0.0);
    }
}
