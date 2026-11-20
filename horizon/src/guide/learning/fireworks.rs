// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, ItemStack, PlayerRef, ServerHost, Tag};

use crate::{network_runtime::paper_text, random::Rolls};

const ROCKET: &str = "minecraft:firework_rocket";
const NAME: &str = "Slimefun Research";
const COLORS: [i32; 17] = [
    0x00ffff, 0x000000, 0x0000ff, 0xff00ff, 0x808080, 0x008000, 0x00ff00, 0x800000, 0x000080,
    0x808000, 0xffa500, 0x800080, 0xff0000, 0xc0c0c0, 0x008080, 0xffffff, 0xffff00,
];

pub(crate) fn launch_random(host: &mut dyn ServerHost, player: PlayerRef, rolls: &Rolls, count: u32) {
    let Some(level) = player.level(host) else {
        return;
    };
    let Some(position) = player.entity().position(host) else {
        return;
    };
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    for _ in 0..count {
        let Some((dx, dz, nbt)) = rocket_nbt(count, |bound| rolls.next(host, level, bound)) else {
            return;
        };
        let mut pos = position;
        pos[0] += f64::from(dx);
        pos[2] += f64::from(dz);
        let _ = host.summon(&dimension, ROCKET, pos, nbt);
    }
}

fn rocket_nbt(radius: u32, mut roll: impl FnMut(u32) -> Option<u32>) -> Option<(i32, i32, Compound)> {
    let dx = roll(radius * 2)? as i32 - radius as i32;
    let dz = roll(radius * 2)? as i32 - radius as i32;
    let color = COLORS[roll(COLORS.len() as u32)? as usize];
    let twinkle = roll(2)? != 0;
    let shape = if roll(2)? != 0 {
        "small_ball"
    } else {
        "large_ball"
    };
    let trail = roll(2)? != 0;
    let flight = roll(2)? as i32 + 1;
    let mut triangle = || {
        Some(
            (f64::from(roll(1 << 24)?) - f64::from(roll(1 << 24)?))
                * (0.002297 / f64::from(1 << 24)),
        )
    };
    let motion = [triangle()?, 0.05, triangle()?];
    let lifetime = 10 * (1 + flight) + roll(6)? as i32 + roll(7)? as i32;
    let star = Tag::compound([
        ("shape", Tag::String(shape.into())),
        ("colors", Tag::IntArray(vec![color])),
        ("has_twinkle", Tag::Byte(i8::from(twinkle))),
        ("has_trail", Tag::Byte(i8::from(trail))),
    ]);
    let rocket = Tag::compound([
        ("id", Tag::String(ROCKET.into())),
        ("count", Tag::Int(1)),
        (
            "components",
            Tag::compound([
                (
                    "minecraft:custom_name",
                    Tag::compound([
                        ("text", Tag::String(NAME.into())),
                        ("color", Tag::String("green".into())),
                    ]),
                ),
                (
                    "minecraft:fireworks",
                    Tag::compound([
                        ("flight_duration", Tag::Int(flight)),
                        ("explosions", Tag::List(vec![star])),
                    ]),
                ),
            ]),
        ),
    ]);
    Some((
        dx,
        dz,
        Compound::from([
            ("LifeTime".into(), Tag::Int(lifetime)),
            (
                "Motion".into(),
                Tag::List(
                    motion
                        .into_iter()
                        .map(|value| Tag::Double(value.to_bits()))
                        .collect(),
                ),
            ),
            ("FireworksItem".into(), rocket),
        ]),
    ))
}

pub(in crate::guide) fn research_rocket(item: &ItemStack) -> bool {
    item.components()
        .and_then(|patch| patch.as_nbt().get(&"minecraft:custom_name".into()))
        .is_some_and(is_research_name)
}

fn is_research_name(tag: &Tag) -> bool {
    paper_text(tag).as_deref() == Some("§aSlimefun Research")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_paper_green_research_name_marks_the_rocket() {
        for (name, cancels) in [
            (Tag::String("Slimefun Research".into()), false),
            (Tag::String("§aSlimefun Research".into()), true),
            (
                crate::hologram_projector::colored_lore("&aSlimefun Research"),
                true,
            ),
            (
                crate::hologram_projector::colored_lore("&2Slimefun Research"),
                false,
            ),
            (
                crate::hologram_projector::colored_lore("&aSlimefun &eResearch"),
                false,
            ),
            (
                crate::hologram_projector::colored_lore("&a&lSlimefun Research"),
                false,
            ),
            (
                crate::hologram_projector::colored_lore("&aSlimefun Researcher"),
                false,
            ),
        ] {
            assert_eq!(is_research_name(&name), cancels, "{name:?}");
        }
    }

    #[test]
    fn a_generated_research_rocket_is_ownerless_and_retains_burst_components() {
        let (dx, dz, nbt) = rocket_nbt(1, |_| Some(0)).unwrap();
        assert_eq!((dx, dz), (-1, -1));
        assert_eq!(nbt.get(&"LifeTime".into()), Some(&Tag::Int(20)));
        assert!(!nbt.contains_key(&"Owner".into()));
        let components = nbt
            .get(&"FireworksItem".into())
            .unwrap()
            .get("components")
            .unwrap();
        assert!(is_research_name(
            components.get("minecraft:custom_name").unwrap()
        ));
        let Some(Tag::List(stars)) = components
            .get("minecraft:fireworks")
            .unwrap()
            .get("explosions")
        else {
            panic!("rocket has no burst");
        };
        assert_eq!(stars.len(), 1);
        assert_eq!(
            stars[0].get("shape"),
            Some(&Tag::String("large_ball".into()))
        );
        assert_eq!(
            stars[0].get("colors"),
            Some(&Tag::IntArray(vec![COLORS[0]]))
        );
    }
}
