// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{Compound, EntityRef, Host, ItemStack, LevelRef, ServerHost, Tag};

use super::Pos;

const PREFIX: &str = "ALTAR Probe - ";
const UNAVAILABLE: &str = "Could not inspect the Paper pedestal's loaded entities. The archive and entities remain in place.";
const AMBIGUOUS: &str = "The Paper pedestal lacks a unique, intact probe and armor stand pair. The archive and entities remain in place.";

pub(super) struct Projection {
    pub(super) item: EntityRef,
    pub(super) stand: EntityRef,
    pub(super) ingredient: ItemStack,
}

fn strip_legacy_colors(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut plain = String::new();
    while let Some(character) = chars.next() {
        if character == '§'
            && chars
                .peek()
                .is_some_and(|code| code.is_ascii_alphanumeric())
        {
            chars.next();
        } else {
            plain.push(character);
        }
    }
    plain
}

fn probe_id(text: &str) -> Option<i64> {
    let plain = strip_legacy_colors(text);
    let token = plain.strip_prefix(PREFIX)?;
    let id = token.parse::<i64>().ok()?;
    (id.to_string() == token).then_some(id)
}

/// Paper saves a legacy literal component, potentially split at style boundaries.
fn literal(tag: &Tag, text: &mut String, unstyled: bool, depth: usize) -> Option<()> {
    if depth >= 16 || text.len() > 4096 {
        return None;
    }
    match tag {
        Tag::String(value) => text.push_str(&value.to_string().ok()?),
        Tag::Compound(fields) => {
            if unstyled
                && fields
                    .keys()
                    .any(|key| !matches!(key.to_string_lossy().as_str(), "text" | "extra"))
            {
                return None;
            }
            let Tag::String(body) = fields.get(&"text".into())? else {
                return None;
            };
            text.push_str(&body.to_string().ok()?);
            match fields.get(&"extra".into()) {
                None => {}
                Some(Tag::List(parts)) => {
                    for part in parts {
                        literal(part, text, unstyled, depth + 1)?;
                    }
                }
                _ => return None,
            }
        }
        Tag::List(parts) => {
            for part in parts {
                literal(part, text, unstyled, depth + 1)?;
            }
        }
        _ => return None,
    }
    Some(())
}

fn displayed_name(stack: &ItemStack) -> Option<&Tag> {
    stack
        .components()?
        .as_nbt()
        .get(&"minecraft:custom_name".into())
}

fn stack_probe(stack: &ItemStack) -> Option<String> {
    let mut text = String::new();
    literal(displayed_name(stack)?, &mut text, false, 0)?;
    strip_legacy_colors(&text)
        .starts_with(PREFIX)
        .then_some(text)
}

fn string(tag: Option<&Tag>) -> Option<String> {
    let Tag::String(value) = tag? else {
        return None;
    };
    value.to_string().ok()
}

/// Paper cancels ItemDespawnEvent for its colored legacy display probes.
pub(super) fn keep_until_restore(stack: &ItemStack) -> bool {
    let Some(name) = displayed_name(stack) else {
        return false;
    };
    if let Some(legacy) = string(Some(name)) {
        return legacy.starts_with("§dALTAR §3Probe - §e") && probe_id(&legacy).is_some();
    }
    let Some(Tag::List(parts)) = name.get("extra") else {
        return false;
    };
    let [altar, probe, serial] = parts.as_slice() else {
        return false;
    };
    string(name.get("text")).as_deref() == Some("")
        && string(altar.get("text")).as_deref() == Some("ALTAR ")
        && string(altar.get("color")).as_deref() == Some("light_purple")
        && string(probe.get("text")).as_deref() == Some("Probe - ")
        && string(probe.get("color")).as_deref() == Some("dark_aqua")
        && string(serial.get("color")).as_deref() == Some("yellow")
        && stack_probe(stack)
            .and_then(|name| probe_id(&name))
            .is_some()
}

fn flag(data: &Compound, key: &str) -> bool {
    data.get(&key.into()) == Some(&Tag::Byte(1))
}

fn pedestal_stand(data: &Compound, expected: i64) -> bool {
    let mut text = String::new();
    let name_matches = data.get(&"CustomName".into()).is_some_and(|name| {
        literal(name, &mut text, false, 0).is_some() && probe_id(&text) == Some(expected)
    });
    name_matches
        && ["Invisible", "Silent", "Marker", "NoGravity", "NoBasePlate"]
            .into_iter()
            .all(|key| flag(data, key))
        && !data.contains_key(&"equipment".into())
        && !data.contains_key(&"ArmorItems".into())
        && !data.contains_key(&"HandItems".into())
}

fn sole_saved_passenger(stand: &Compound, item: &Compound) -> bool {
    let Some(Tag::List(passengers)) = stand.get(&"Passengers".into()) else {
        return false;
    };
    let [Tag::Compound(passenger)] = passengers.as_slice() else {
        return false;
    };
    let uuid = item.get(&"UUID".into());
    matches!(uuid, Some(Tag::IntArray(parts)) if parts.len() == 4)
        && passenger.get(&"UUID".into()) == uuid
        && !item.contains_key(&"Passengers".into())
}

fn ingredient(
    mut display: ItemStack,
    entity: &Compound,
    default_name: impl FnOnce(&ItemStack) -> Option<String>,
) -> Option<ItemStack> {
    if display.count() != 1 || !flag(entity, "Invulnerable") || !flag(entity, "CustomNameVisible") {
        return None;
    }
    let mut components = display.components()?.as_nbt().clone();
    components.remove(&"minecraft:custom_name".into())?;
    display.set_components(components).ok()?;
    // Paper writes the original display label on the item entity itself. It removes the probe
    // name from the stack, and only reinstates a label distinct from the ordinary item name.
    if let Some(name) = entity.get(&"CustomName".into()) {
        let mut title = String::new();
        literal(name, &mut title, false, 0)?;
        let unstyled = &mut String::new();
        let ordinary =
            literal(name, unstyled, true, 0).is_some() && *unstyled == default_name(&display)?;
        if !ordinary {
            display
                .apply_components(&Compound::from([(
                    "minecraft:custom_name".into(),
                    name.clone(),
                )]))
                .ok()?;
        }
    }
    Some(display)
}

fn has_probe_name(host: &dyn Host, entity: EntityRef) -> bool {
    entity
        .name(host)
        .is_some_and(|name| strip_legacy_colors(&name).starts_with(PREFIX))
}

pub(super) fn find(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
) -> Result<Option<Projection>, &'static str> {
    let [x, y, z] = pos.map(f64::from);
    let candidates = level
        .entities_in(host, [x, y + 0.7, z], [x + 1.0, y + 1.7, z + 1.0])
        .ok_or(UNAVAILABLE)?;
    let mut stands = Vec::new();
    let mut items = Vec::new();
    for entity in candidates {
        if let Some(item) = host.item_entity(entity) {
            if item.pickup_delay(host) == Some(i32::from(i16::MAX))
                || item.stack(host).as_ref().and_then(stack_probe).is_some()
                || has_probe_name(host, entity)
            {
                items.push(entity);
            }
        } else if has_probe_name(host, entity) {
            stands.push(entity);
        }
    }
    let stand = match (stands.as_slice(), items.as_slice()) {
        ([], []) => return Ok(None),
        ([stand], _) => *stand,
        _ => return Err(AMBIGUOUS),
    };
    // Use the mount graph for the passenger. Once seated it can be above the original
    // spawn area, depending on the engine's armor-stand passenger attachment point.
    let passengers = stand.passengers(host).ok_or(UNAVAILABLE)?;
    let [item] = passengers.as_slice() else {
        return Err(AMBIGUOUS);
    };
    let item = *item;
    if items.iter().any(|candidate| *candidate != item) {
        return Err(AMBIGUOUS);
    }
    let at = stand.position(host).ok_or(UNAVAILABLE)?;
    if at
        .into_iter()
        .zip([x + 0.5, y + 1.2, z + 0.5])
        .any(|(a, b)| (a - b).abs() > 1.0e-6)
        || !stand
            .kind(host)
            .is_some_and(|kind| kind.id(host) == "minecraft:armor_stand")
        || stand.is_passenger(host) != Some(false)
        || item.vehicle(host) != Some(stand)
        || item.is_vehicle(host) != Some(false)
    {
        return Err(AMBIGUOUS);
    }
    let displayed = host.item_entity(item).ok_or(AMBIGUOUS)?;
    if displayed.pickup_delay(host) != Some(i32::from(i16::MAX)) {
        return Err(AMBIGUOUS);
    }
    let stack = displayed.stack(host).ok_or(UNAVAILABLE)?;
    let token = stack_probe(&stack)
        .and_then(|probe| probe_id(&probe))
        .ok_or(AMBIGUOUS)?;
    let stand_data = stand.nbt(host).ok_or(UNAVAILABLE)?;
    if !pedestal_stand(&stand_data, token) {
        return Err(AMBIGUOUS);
    }
    let item_data = item.nbt(host).ok_or(UNAVAILABLE)?;
    if !sole_saved_passenger(&stand_data, &item_data) {
        return Err(AMBIGUOUS);
    }
    let ingredient = ingredient(stack, &item_data, |stack| {
        host.item_default_english_name(stack)
    })
    .ok_or(AMBIGUOUS)?;
    Ok(Some(Projection {
        item,
        stand,
        ingredient,
    }))
}

#[cfg(test)]
mod tests {
    use mod_api::{Identifier, prelude::ItemsHost, testing::FakeHost};

    use super::*;

    fn component(text: &str, color: &str) -> Tag {
        Tag::compound([
            ("text", Tag::String(text.into())),
            ("color", Tag::String(color.into())),
        ])
    }

    fn probe_component() -> Tag {
        // ComponentSerialization output from Paper 26.2's CraftChatMessage.
        let mut segments = [
            ("ALTAR ", "light_purple"),
            ("Probe - ", "dark_aqua"),
            ("-178899", "yellow"),
        ]
        .map(|(text, color)| {
            Compound::from([
                ("text".into(), Tag::String(text.into())),
                ("color".into(), Tag::String(color.into())),
                ("italic".into(), Tag::Byte(0)),
            ])
        });
        for flag in ["bold", "obfuscated", "strikethrough", "underlined"] {
            segments[0].insert(flag.into(), Tag::Byte(0));
        }
        Tag::compound([
            ("text", Tag::String("".into())),
            (
                "extra",
                Tag::List(segments.into_iter().map(Tag::Compound).collect()),
            ),
        ])
    }

    #[test]
    fn the_paper_stand_requires_the_same_probe_and_exact_empty_marker_shape() {
        assert_eq!(probe_id("§dALTAR §3Probe - §e-178899"), Some(-178899));
        assert_eq!(probe_id("ALTAR Probe - 178899extra"), None);
        assert_eq!(probe_id("ALTAR Probe - 00178899"), None);
        let mut stand = Compound::from([("CustomName".into(), probe_component())]);
        for flag in ["Invisible", "Silent", "Marker", "NoGravity", "NoBasePlate"] {
            stand.insert(flag.into(), Tag::Byte(1));
        }
        assert!(pedestal_stand(&stand, -178899));
        assert!(!pedestal_stand(&stand, -178898));
        stand.insert(
            "equipment".into(),
            Tag::compound([(
                "head",
                Tag::compound([("id", Tag::String("minecraft:diamond_helmet".into()))]),
            )]),
        );
        assert!(!pedestal_stand(&stand, -178899));
        stand.remove(&"equipment".into());
        stand.remove(&"NoGravity".into());
        assert!(!pedestal_stand(&stand, -178899));
    }

    #[test]
    fn a_preserved_extra_passenger_blocks_adoption_even_if_it_has_no_live_handle() {
        let item = Compound::from([("UUID".into(), Tag::IntArray(vec![1, 2, 3, 4]))]);
        let rider = Tag::Compound(item.clone());
        let mut stand = Compound::from([("Passengers".into(), Tag::List(vec![rider.clone()]))]);
        assert!(sole_saved_passenger(&stand, &item));
        stand.insert(
            "Passengers".into(),
            Tag::List(vec![
                rider,
                Tag::compound([("id", Tag::String("absent:entity".into()))]),
            ]),
        );
        assert!(!sole_saved_passenger(&stand, &item));
    }

    #[test]
    fn the_probe_is_removed_without_losing_the_original_name_and_components() {
        let mut host = FakeHost::new();
        let key = Identifier::parse("minecraft:diamond_pickaxe").unwrap();
        host.registries.resolve_items([key.clone()]).unwrap();
        let mut display = ItemStack::new(host.item_key(&key).unwrap(), 1).unwrap();
        display.set_damage(14);
        let lore = Tag::List(vec![component("Keep the original lore", "green")]);
        let pdc = Tag::compound([(
            "PublicBukkitValues",
            Tag::compound([("slimefun:unique", Tag::Byte(1))]),
        )]);
        display
            .set_components(Compound::from([
                ("minecraft:custom_name".into(), probe_component()),
                ("minecraft:lore".into(), lore.clone()),
                ("minecraft:custom_data".into(), pdc.clone()),
            ]))
            .unwrap();
        assert_eq!(
            stack_probe(&display).and_then(|name| probe_id(&name)),
            Some(-178899)
        );
        assert!(keep_until_restore(&display));
        let mut ordinary = display.clone();
        ordinary
            .apply_components(&Compound::from([(
                "minecraft:custom_name".into(),
                Tag::String("ALTAR Probe - -178899".into()),
            )]))
            .unwrap();
        assert!(!keep_until_restore(&ordinary));
        let original_name = component("Diamond Pickaxe", "gold");
        let mut entity = Compound::from([
            ("Invulnerable".into(), Tag::Byte(1)),
            ("CustomNameVisible".into(), Tag::Byte(1)),
            ("CustomName".into(), original_name.clone()),
        ]);
        let found =
            ingredient(display.clone(), &entity, |_| Some("Diamond Pickaxe".into())).unwrap();
        let patch = found.components().unwrap().as_nbt();
        assert_eq!(found.damage(), display.damage());
        assert_eq!(
            patch.get(&"minecraft:custom_name".into()),
            Some(&original_name)
        );
        assert_eq!(patch.get(&"minecraft:lore".into()), Some(&lore));
        assert_eq!(patch.get(&"minecraft:custom_data".into()), Some(&pdc));

        entity.insert(
            "CustomName".into(),
            Tag::compound([
                ("text", Tag::String("".into())),
                (
                    "extra",
                    Tag::List(vec![Tag::String("Diamond Pickaxe".into())]),
                ),
            ]),
        );
        assert!(ingredient(display.clone(), &entity, |_| None).is_none());
        let found =
            ingredient(display.clone(), &entity, |_| Some("Diamond Pickaxe".into())).unwrap();
        assert!(
            !found
                .components()
                .unwrap()
                .as_nbt()
                .contains_key(&"minecraft:custom_name".into())
        );
        entity.remove(&"Invulnerable".into());
        assert!(ingredient(display, &entity, |_| Some("Diamond Pickaxe".into())).is_none());
    }
}
