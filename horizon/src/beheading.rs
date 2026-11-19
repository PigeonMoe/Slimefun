// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Mutex;

use mod_api::{
    Compound, GameProfile, Hand, ItemDrop, ItemPlace, LivingDropsEvent, ServerHost, Tag,
};

use crate::{
    items::{Kind, MAGIC_LUMP_2, item, text},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    SWORD_OF_BEHEADING,
    "iron_sword",
    "Sword of Beheading",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "sword_of_beheading",
    name: "Sword of Beheading",
    cost: 6,
};

const EMERALD: Option<Kind> = Some(Kind::Vanilla("minecraft:emerald"));
pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        EMERALD,
        None,
        Some(MAGIC_LUMP_2),
        EMERALD,
        Some(MAGIC_LUMP_2),
        None,
        Some(Kind::Vanilla("minecraft:blaze_rod")),
        None,
    ],
    output: Stack {
        kind: SWORD_OF_BEHEADING,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn lore() -> Tag {
    Tag::List(vec![
        text("Beheading II", "gray", false),
        Tag::String("".into()),
        text("Has a chance to behead Mobs", "white", false),
        text(
            "(even a higher chance for Wither Skeletons)",
            "white",
            false,
        ),
    ])
}

struct Target {
    kind: &'static str,
    setting: &'static str,
    head: &'static str,
    default_chance: u8,
}

const TARGETS: [Target; 6] = [
    Target {
        kind: "minecraft:zombie",
        setting: "ZOMBIE",
        head: "minecraft:zombie_head",
        default_chance: 40,
    },
    Target {
        kind: "minecraft:skeleton",
        setting: "SKELETON",
        head: "minecraft:skeleton_skull",
        default_chance: 40,
    },
    Target {
        kind: "minecraft:wither_skeleton",
        setting: "WITHER_SKELETON",
        head: "minecraft:wither_skeleton_skull",
        default_chance: 25,
    },
    Target {
        kind: "minecraft:creeper",
        setting: "CREEPER",
        head: "minecraft:creeper_head",
        default_chance: 40,
    },
    Target {
        kind: "minecraft:piglin",
        setting: "PIGLIN",
        head: "minecraft:piglin_head",
        default_chance: 40,
    },
    Target {
        kind: "minecraft:player",
        setting: "PLAYER",
        head: "minecraft:player_head",
        default_chance: 70,
    },
];

pub(crate) const DEFAULT_CONFIG: &str = "\n[sword-of-beheading.chance]\nZOMBIE = 40\nSKELETON = 40\nWITHER_SKELETON = 25\nCREEPER = 40\nPIGLIN = 40\nPLAYER = 70\n";

pub(crate) struct Beheading {
    chances: Mutex<[u8; TARGETS.len()]>,
}

impl Default for Beheading {
    fn default() -> Self {
        Self {
            chances: Mutex::new(TARGETS.map(|target| target.default_chance)),
        }
    }
}

impl Beheading {
    pub(crate) fn configure(&self, table: Option<&toml::Value>) {
        let chances = table.and_then(|table| table.get("chance"));
        *self.chances.lock().unwrap() = TARGETS.map(|target| {
            chances
                .and_then(|chances| chances.get(target.setting))
                .and_then(toml::Value::as_integer)
                .filter(|chance| (0..=100).contains(chance))
                .map_or(target.default_chance, |chance| chance as u8)
        });
    }

    pub(crate) fn drop_head(
        &self,
        host: &mut dyn ServerHost,
        event: &mut LivingDropsEvent<'_>,
        rolls: &Rolls,
    ) {
        let Some((index, target)) = TARGETS
            .iter()
            .enumerate()
            .find(|(_, target)| target.kind == event.kind)
        else {
            return;
        };
        let Some(killer) = event.credited_player.and_then(|uuid| host.player(uuid)) else {
            return;
        };
        let hand = ItemPlace::PlayerHand {
            player: killer.uuid(),
            hand: Hand::Main,
        };
        if !hand
            .stack(host)
            .is_some_and(|stack| SWORD_OF_BEHEADING.matches(host, &stack))
            || !research::can_use(host, killer, &RESEARCH)
        {
            return;
        }
        let Some(victim) = host.entity(event.entity) else {
            return;
        };
        let Some(level) = victim.level(host) else {
            return;
        };
        let chance = u32::from(self.chances.lock().unwrap()[index]);
        if !rolls
            .next(host, level, 100)
            .is_some_and(|roll| roll < chance)
        {
            return;
        }

        let Some(mut skull) = Kind::Vanilla(target.head).stack(host, 1) else {
            return;
        };
        let player_head = target.kind == "minecraft:player";
        let (position, pickup_delay) = if player_head {
            let Some(profile) = event
                .player
                .and_then(|uuid| host.player(uuid))
                .and_then(|player| player.profile(host))
            else {
                return;
            };
            if skull
                .apply_components(&Compound::from([(
                    "minecraft:profile".into(),
                    profile_tag(profile),
                )]))
                .is_err()
            {
                return;
            }
            let Some([x, eyes, z]) = victim.eye_position(host) else {
                return;
            };
            ([x, eyes - f64::from(0.3_f32), z], 40)
        } else {
            let Some(position) = victim.position(host) else {
                return;
            };
            (position, 10)
        };

        // Match the original player death toss and the mob's ordinary drop motion.
        let unit = || {
            const STEPS: u32 = 1 << 24;
            f64::from(rolls.next(host, level, STEPS).unwrap_or(0)) / f64::from(STEPS)
        };
        let velocity = if player_head {
            let power = unit() * 0.5;
            let direction = unit() * std::f64::consts::TAU;
            [-direction.sin() * power, 0.2, direction.cos() * power]
        } else {
            [(unit() - 0.5) * 0.2, 0.2, (unit() - 0.5) * 0.2]
        };
        event
            .drops
            .push(ItemDrop::new(skull, position, velocity, pickup_delay));
    }
}

fn profile_tag(profile: GameProfile) -> Tag {
    let id = profile.uuid;
    let mut fields = Compound::from([
        ("name".into(), Tag::String(profile.name.as_str().into())),
        (
            "id".into(),
            Tag::IntArray(vec![
                (id >> 96) as i32,
                (id >> 64) as i32,
                (id >> 32) as i32,
                id as i32,
            ]),
        ),
    ]);
    if !profile.properties.is_empty() {
        fields.insert(
            "properties".into(),
            Tag::List(
                profile
                    .properties
                    .into_iter()
                    .map(|property| {
                        let mut value = Compound::from([
                            ("name".into(), Tag::String(property.name.as_str().into())),
                            ("value".into(), Tag::String(property.value.as_str().into())),
                        ]);
                        if let Some(signature) = property.signature {
                            value
                                .insert("signature".into(), Tag::String(signature.as_str().into()));
                        }
                        Tag::Compound(value)
                    })
                    .collect(),
            ),
        );
    }
    Tag::Compound(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::ProfileProperty;

    #[test]
    fn paper_config_uses_exact_variants_and_accepts_chance_limits() {
        let beheading = Beheading::default();
        assert_eq!(*beheading.chances.lock().unwrap(), [40, 40, 25, 40, 40, 70]);
        let table = toml::from_str::<toml::Table>(
            "[sword-of-beheading.chance]\nZOMBIE = 0\nSKELETON = 100\nWITHER_SKELETON = -1\nCREEPER = 101\nPIGLIN = \"40\"",
        ).unwrap();
        beheading.configure(table.get("sword-of-beheading"));
        assert_eq!(*beheading.chances.lock().unwrap(), [0, 100, 25, 40, 40, 70]);
        assert!(!TARGETS.iter().any(|target| matches!(
            target.kind,
            "minecraft:husk"
                | "minecraft:zombie_villager"
                | "minecraft:piglin_brute"
                | "minecraft:stray"
        )));
    }

    #[test]
    fn the_skull_keeps_uuid_name_and_signed_login_properties() {
        let Tag::Compound(profile) = profile_tag(GameProfile::new(
            0x0011_2233_4455_6677_8899_aabb_ccdd_eeff,
            "Example".into(),
            vec![ProfileProperty::new(
                "textures".into(),
                "e30=".into(),
                Some("proof".into()),
            )],
        )) else {
            panic!("skull profile must be a compound");
        };
        assert_eq!(
            profile.get(&"id".into()),
            Some(&Tag::IntArray(vec![
                0x0011_2233,
                0x4455_6677,
                0x8899_aabb_u32 as i32,
                0xccdd_eeff_u32 as i32,
            ]))
        );
        assert_eq!(
            profile.get(&"name".into()),
            Some(&Tag::String("Example".into()))
        );
        assert_eq!(
            profile.get(&"properties".into()),
            Some(&Tag::List(vec![Tag::compound([
                ("name", Tag::String("textures".into())),
                ("value", Tag::String("e30=".into())),
                ("signature", Tag::String("proof".into())),
            ])]))
        );
    }
}
