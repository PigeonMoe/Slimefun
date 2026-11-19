// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Compound, ConfigPath, Hand, HostTransaction, ItemPlace, ItemResource, ItemStack, LevelRef,
    PlayerRef, ServerHost, SoundSource, Tag, Text,
};

use crate::{
    items::{GILDED_IRON, HeadTexture, Kind, SILICON, item, text},
    network_runtime::parse_uuid,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

const CUSTOM_DATA: &str = "minecraft:custom_data";
const PUBLIC_VALUES: &str = "PublicBukkitValues";
const PAPER_ANCHOR: &str = "slimefun:anchor";
const HORIZON_ANCHOR: &str = "slimefun:horizon_anchor";

item!(
    TAPE_MEASURE,
    "player_head",
    "Tape Measure",
    "gold",
    None,
    Some(HeadTexture {
        profile: [456198403, -849660347, -2038026414, 918980326],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzE4MGQ1YzQzYTZjZjViYjc3NjlmZDBjODI0MGUxZTcwZDJhZTM4ZWY5ZDc4YTFkYjQwMWFjYTZhMmNiMzZmNjUifX19",
    })
);

pub(crate) const RESEARCH: Research = Research {
    id: "tape_measure",
    name: "Tape Measure",
    cost: 7,
};
const DYE: Kind = Kind::Vanilla("minecraft:yellow_dye");
pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(SILICON),
        Some(DYE),
        Some(SILICON),
        Some(DYE),
        Some(Kind::Vanilla("minecraft:string")),
        Some(DYE),
        Some(GILDED_IRON),
        Some(DYE),
        Some(SILICON),
    ],
    output: Stack {
        kind: TAPE_MEASURE,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore() -> Tag {
    let line = |action, rest| {
        Tag::compound([
            ("text", Tag::String("".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![
                    text(action, "yellow", false),
                    text(rest, "gray", false),
                ]),
            ),
        ])
    };
    Tag::List(vec![
        Tag::String("".into()),
        line("Crouch & Right Click", " to set an anchor"),
        line("Right Click", " to measure"),
    ])
}

#[derive(Debug, PartialEq, Eq)]
enum World {
    Dimension(String),
    Paper(u128),
}

#[derive(Debug, PartialEq, Eq)]
struct Anchor {
    world: World,
    pos: [i32; 3],
}

fn anchor(stack: &ItemStack) -> Option<Anchor> {
    let Some(Tag::Compound(data)) = stack.components()?.as_nbt().get(&CUSTOM_DATA.into()) else {
        return None;
    };
    let Tag::Compound(values) = data.get(&PUBLIC_VALUES.into())? else {
        return None;
    };
    if let Some(Tag::Compound(data)) = values.get(&HORIZON_ANCHOR.into()) {
        let Tag::String(dimension) = data.get(&"dimension".into())? else {
            return None;
        };
        let parse_position = |axis: &str| match data.get(&axis.into())? {
            Tag::Int(coordinate) => Some(*coordinate),
            _ => None,
        };
        return Some(Anchor {
            world: World::Dimension(dimension.to_string().ok()?),
            pos: [
                parse_position("x")?,
                parse_position("y")?,
                parse_position("z")?,
            ],
        });
    }
    let Tag::String(raw) = values.get(&PAPER_ANCHOR.into())? else {
        return None;
    };
    let raw = raw.to_string().ok()?;
    if raw.len() > 1024 {
        return None;
    }
    let old: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let world = parse_uuid(old.get("world")?.as_str()?)?;
    let coord = |axis: &str| i32::try_from(old.get(axis)?.as_i64()?).ok();
    Some(Anchor {
        world: World::Paper(world),
        pos: [coord("x")?, coord("y")?, coord("z")?],
    })
}

fn set_anchor(mut stack: ItemStack, dimension: &str, [x, y, z]: [i32; 3]) -> Option<ItemStack> {
    let Some(Tag::Compound(mut data)) = stack
        .components()
        .and_then(|components| components.as_nbt().get(&CUSTOM_DATA.into()))
        .cloned()
    else {
        return None;
    };
    let Some(Tag::Compound(values)) = data.get_mut(&PUBLIC_VALUES.into()) else {
        return None;
    };
    values.remove(&PAPER_ANCHOR.into());
    values.insert(
        HORIZON_ANCHOR.into(),
        Tag::compound([
            ("dimension", Tag::String(dimension.into())),
            ("x", Tag::Int(x)),
            ("y", Tag::Int(y)),
            ("z", Tag::Int(z)),
        ]),
    );
    stack
        .apply_components(&Compound::from([(CUSTOM_DATA.into(), Tag::Compound(data))]))
        .ok()?;
    Some(stack)
}

fn paper_dimension(data: &toml::Table, uuid: u128) -> Option<&str> {
    let worlds = data.get("worlds")?.as_table()?;
    let mut destinations = worlds.iter().filter(|(id, _)| parse_uuid(id) == Some(uuid));
    let target = destinations.next()?.1.as_str()?;
    if destinations.next().is_some() {
        return None;
    }
    Some(target)
}

fn matches_world(host: &dyn ServerHost, world: &World, dimension: &str) -> bool {
    match world {
        World::Dimension(saved) => saved == dimension,
        World::Paper(uuid) => {
            let path = ConfigPath::new("paper-worlds.toml")
                .expect("valid Slimefun world mapping filename");
            let Ok(bytes) = host.world_data_files().read(&path) else {
                return false;
            };
            let Ok(raw) = std::str::from_utf8(&bytes) else {
                return false;
            };
            let Ok(data) = toml::from_str(raw) else {
                return false;
            };
            paper_dimension(&data, *uuid) == Some(dimension)
        }
    }
}

fn distance([x, y, z]: [i32; 3], [a, b, c]: [i32; 3]) -> String {
    let [dx, dy, dz] = [
        f64::from(x) - f64::from(a),
        f64::from(y) - f64::from(b),
        f64::from(z) - f64::from(c),
    ];
    format!("{:.3}", (dx * dx + dy * dy + dz * dz).sqrt())
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn colored(value: impl Into<String>, color: &str) -> Text {
    let mut message = Text::literal(value);
    message.style.color = Some(color.into());
    message
}

pub(crate) fn in_air(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack) -> bool {
    if !TAPE_MEASURE.matches(host, stack) {
        return false;
    }
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Tape Measure with /sf research tape_measure."),
        );
    }
    true
}

fn set_held_anchor(
    host: &mut dyn ServerHost,
    hand: &ItemPlace,
    expected: ItemStack,
    dimension: &str,
    pos: [i32; 3],
) -> bool {
    let count = expected.count();
    let original = ItemResource::of(&expected);
    let Some(updated) = set_anchor(expected, dimension, pos) else {
        return false;
    };
    host.transaction(|host, tx| {
        if hand.resource(host).as_ref() != Some(&original)
            || hand.exchange(host, tx, &ItemResource::of(&updated), count) != count
        {
            return Err(());
        }
        Ok(())
    })
    .is_ok()
}

pub(crate) fn use_at(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    hand: Hand,
    pos: [i32; 3],
) -> bool {
    let held = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    };
    let Some(stack) = held
        .stack(host)
        .filter(|stack| TAPE_MEASURE.matches(host, stack))
    else {
        return false;
    };
    if !research::can_use(host, player, &RESEARCH) {
        let _ = player.tell(
            host,
            Text::literal("Unlock Tape Measure with /sf research tape_measure."),
        );
        return true;
    }
    let Some(dimension) = level.dimension().id(host) else {
        return true;
    };
    if player.is_secondary_use_active(host) == Some(true) {
        if set_held_anchor(host, &held, stack, &dimension, pos) {
            let [x, y, z] = pos;
            let message = colored("Successfully set the anchor: ", "green")
                .append(colored(format!("{x} | {y} | {z}"), "yellow"));
            let _ = player.tell(host, message);
        }
        return true;
    }
    let error = match anchor(&stack) {
        None => "You need to set an anchor before you can start to measure!",
        Some(anchor) if matches_world(host, &anchor.world, &dimension) => {
            let _ = level.play_sound(
                host,
                "minecraft:item.book.put",
                pos.map(f64::from),
                SoundSource::Players,
                1.0,
                0.7,
            );
            let message = colored("Measurement taken. ", "gray").append(colored(
                format!("Distance: {}", distance(anchor.pos, pos)),
                "yellow",
            ));
            let _ = player.tell(host, message);
            return true;
        }
        Some(_) => "Your anchor seems to be in a different world!",
    };
    let _ = player.tell(host, colored(error, "red"));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn exact_paper_recipe_and_block_center_distance() {
        assert_eq!(
            CRAFTING.inputs.map(|entry| entry.unwrap().id()),
            [
                "SILICON",
                "minecraft:yellow_dye",
                "SILICON",
                "minecraft:yellow_dye",
                "minecraft:string",
                "minecraft:yellow_dye",
                "GILDED_IRON",
                "minecraft:yellow_dye",
                "SILICON"
            ]
        );
        assert_eq!(RESEARCH.cost, 7);
        assert_eq!(distance([3, -4, 0], [0, 0, 0]), "5");
        assert_eq!(distance([4, 4, 4], [4, 4, 4]), "0");
        assert_eq!(distance([-1, -1, -1], [0, 0, 0]), "1.732");
        assert_eq!(
            distance([30_000_000, 65, 0], [-30_000_000, 65, 0]),
            "60000000"
        );
    }

    #[test]
    fn paper_anchor_keeps_exact_world_and_setting_either_hand_preserves_other_pdc() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse(TAPE_MEASURE.material()).unwrap()])
            .unwrap();
        let mut tape = TAPE_MEASURE.stack(&host, 3).unwrap();
        let id = "12345678-1234-5678-1234-567812345678";
        tape.apply_components(&Compound::from([(
            CUSTOM_DATA.into(),
            Tag::compound([(
                PUBLIC_VALUES,
                Tag::compound([
                    (crate::items::ID_KEY, Tag::String(TAPE_MEASURE.id().into())),
                    (
                        PAPER_ANCHOR,
                        Tag::String(
                            format!(r#"{{"world":"{id}","x":-10,"y":20,"z":30}}"#)
                                .as_str()
                                .into(),
                        ),
                    ),
                    ("other:key", Tag::Long(90)),
                ]),
            )]),
        )]))
        .unwrap();
        let uuid = parse_uuid(id).unwrap();
        assert_eq!(
            anchor(&tape),
            Some(Anchor {
                world: World::Paper(uuid),
                pos: [-10, 20, 30]
            })
        );
        let map =
            toml::from_str(&format!("[worlds]\n\"{id}\" = \"minecraft:overworld\"\n")).unwrap();
        assert_eq!(paper_dimension(&map, uuid), Some("minecraft:overworld"));
        assert_eq!(paper_dimension(&map, uuid + 1), None);
        let player = host.join(35, 87);
        player.slots.insert(0, tape.clone());
        player.slots.insert(-106, tape.clone());
        let off_hand = ItemPlace::PlayerHand {
            player: 35,
            hand: Hand::Off,
        };
        assert!(set_held_anchor(
            &mut host,
            &off_hand,
            tape,
            "minecraft:the_nether",
            [1, 60, 4]
        ));
        let player = host.players.get(35).unwrap();
        assert!(matches!(
            anchor(&player.slots[&0]).unwrap().world,
            World::Paper(_)
        ));
        let tape = &player.slots[&-106];
        assert_eq!(tape.count(), 3);
        assert_eq!(
            anchor(&tape),
            Some(Anchor {
                world: World::Dimension("minecraft:the_nether".into()),
                pos: [1, 60, 4]
            })
        );
        let Tag::Compound(data) = tape
            .components()
            .unwrap()
            .as_nbt()
            .get(&CUSTOM_DATA.into())
            .unwrap()
        else {
            panic!()
        };
        let Tag::Compound(pdc) = data.get(&PUBLIC_VALUES.into()).unwrap() else {
            panic!()
        };
        assert_eq!(pdc.get(&"other:key".into()), Some(&Tag::Long(90)));
        assert!(!pdc.contains_key(&PAPER_ANCHOR.into()));
        assert!(TAPE_MEASURE.matches(&host, &tape));
    }
}
