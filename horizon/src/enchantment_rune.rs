// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;

use mod_api::{
    ComponentId, Host, ItemResource, ItemStack, LevelRef, ParticleOptions, ServerHost, SoundSource,
    Tag,
};

use crate::{
    ancient_altar::{ANCIENT_RUNE_AIR, ANCIENT_RUNE_WATER},
    enchantment_machines,
    items::{Kind, MAGIC_LUMP_3, item, item_id, text},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    talismans::{GLASS_RESEARCH, MAGICAL_GLASS},
};

item!(
    ANCIENT_RUNE_LIGHTNING,
    "firework_star",
    "Ancient Rune [Lightning]",
    "gray",
    None,
    None
);
item!(
    ANCIENT_RUNE_ENCHANTMENT,
    "firework_star",
    "Ancient Rune [Enchantment]",
    "gray",
    None,
    None
);

pub(crate) const LIGHTNING_RESEARCH: Research = Research {
    id: "lightning_rune",
    name: "Lightning Rune",
    cost: 24,
};

const IRON: Kind = Kind::Vanilla("minecraft:iron_ingot");
const MEMBRANE: Kind = Kind::Vanilla("minecraft:phantom_membrane");

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            Some(IRON),
            Some(MAGIC_LUMP_3),
            Some(IRON),
            Some(ANCIENT_RUNE_AIR),
            Some(MEMBRANE),
            Some(ANCIENT_RUNE_WATER),
            Some(IRON),
            Some(MAGIC_LUMP_3),
            Some(IRON),
        ],
        output: Stack {
            kind: ANCIENT_RUNE_LIGHTNING,
            count: 4,
        },
        research: &LIGHTNING_RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            Some(MAGIC_LUMP_3),
            Some(MAGICAL_GLASS),
            Some(MAGIC_LUMP_3),
            Some(MAGICAL_GLASS),
            Some(ANCIENT_RUNE_LIGHTNING),
            Some(MAGICAL_GLASS),
            Some(MAGIC_LUMP_3),
            Some(MAGICAL_GLASS),
            Some(MAGIC_LUMP_3),
        ],
        output: Stack {
            kind: ANCIENT_RUNE_ENCHANTMENT,
            count: 1,
        },
        research: &GLASS_RESEARCH,
    },
];

const DIRECT: &str = "minecraft:enchantments";

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(crate) enum Failure {
    InvalidItem,
    NoEnchantment,
}

impl Failure {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::InvalidItem => "You cannot enchant this item.",
            Self::NoEnchantment => "Couldn't find any applicable enchantment for this item.",
        }
    }
}

pub(crate) const SUCCESS: &str =
    "You have successfully applied a random applicable enchantment to this item.";

#[derive(Default)]
pub(crate) struct Rules(BTreeSet<String>);

impl Rules {
    pub(crate) fn from_config(section: Option<&toml::Value>) -> Self {
        Self(
            section
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|items| items.iter())
                .filter(|(_, item)| {
                    item.get("allow-enchanting").and_then(toml::Value::as_bool) == Some(false)
                })
                .map(|(id, _)| id.clone())
                .collect(),
        )
    }

    pub(crate) fn apply(
        &self,
        host: &dyn Host,
        level: LevelRef,
        item: &ItemStack,
        rolls: &Rolls,
    ) -> Result<ItemStack, Failure> {
        if item_id(item).is_some_and(|id| self.0.contains(&id)) {
            return Err(Failure::InvalidItem);
        }
        let mut current =
            enchantment_machines::levels(host, item, DIRECT).ok_or(Failure::InvalidItem)?;
        let previous = current
            .keys()
            .filter_map(|id| id.to_string().ok())
            .collect::<Vec<_>>();
        let possibilities = candidates(
            host.enchantment_keys().into_iter().filter_map(|key| {
                let id = key.id(host);
                let max = host.enchantment_max_level(&id)?;
                Some((id, max))
            }),
            |id| host.enchantment_supports(id, item.item()),
            |id| {
                previous
                    .iter()
                    .all(|old| old != id && host.enchantments_compatible(old, id))
            },
        );
        if possibilities.is_empty() {
            return Err(Failure::NoEnchantment);
        }
        let index = rolls
            .next(host, level, possibilities.len() as u32)
            .ok_or(Failure::InvalidItem)? as usize;
        let (id, max) = &possibilities[index];
        let amount = if *max == 1 {
            1
        } else {
            rolls.next(host, level, *max).ok_or(Failure::InvalidItem)? + 1
        };
        if item.count() != 1 {
            return Err(Failure::InvalidItem);
        }
        current.insert(id.as_str().into(), Tag::Int(amount as i32));
        ItemResource::of(item)
            .with_component(
                host,
                &ComponentId::new(DIRECT).ok_or(Failure::InvalidItem)?,
                Tag::Compound(current),
            )
            .ok()
            .and_then(|result| result.to_stack(1))
            .ok_or(Failure::InvalidItem)
    }
}

fn candidates(
    registry: impl IntoIterator<Item = (String, u32)>,
    supports: impl Fn(&str) -> bool,
    compatible: impl Fn(&str) -> bool,
) -> Vec<(String, u32)> {
    registry
        .into_iter()
        .filter(|(id, max)| {
            *max > 0
                && !matches!(
                    id.as_str(),
                    "minecraft:binding_curse" | "minecraft:vanishing_curse"
                )
                && supports(id)
                && compatible(id)
        })
        .map(|(id, max)| (id, max.min(255)))
        .collect()
}

pub(crate) fn lore() -> Tag {
    Tag::List(vec![
        text("Drop this rune onto a dropped item to", "yellow", false),
        Tag::compound([
            ("text", Tag::String("enchant ".into())),
            ("color", Tag::String("gold".into())),
            ("italic", Tag::Byte(0)),
            (
                "extra",
                Tag::List(vec![text(
                    "that item with a random enchantment.",
                    "yellow",
                    false,
                )]),
            ),
        ]),
    ])
}

pub(crate) fn finish_fx(host: &mut dyn ServerHost, level: LevelRef, origin: [f64; 3]) {
    if let Some(particle) = crate::ancient_altar::enchanted_hit_particle() {
        level.send_particles(
            host,
            &ParticleOptions::simple(particle),
            None,
            origin,
            [0.0; 3],
            0.0,
            1,
        );
    }
    level.play_sound(
        host,
        "minecraft:entity.zombie_villager.cure",
        origin,
        SoundSource::Players,
        1.0,
        1.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_rune_excludes_two_vanilla_curses_and_only_selects_supported_new_enchantments() {
        let registry = [
            ("minecraft:binding_curse", 1),
            ("minecraft:vanishing_curse", 1),
            ("pack:accepted_curse", 1),
            ("minecraft:fortuneless", 2),
            ("minecraft:incompatible", 3),
            ("minecraft:efficiency", 5),
        ]
        .map(|(id, max)| (id.to_owned(), max));
        assert_eq!(
            candidates(
                registry,
                |id| id != "minecraft:fortuneless",
                |id| id != "minecraft:incompatible",
            ),
            [
                ("pack:accepted_curse".into(), 1),
                ("minecraft:efficiency".into(), 5)
            ]
        );
    }

    #[test]
    fn lightning_and_enchantment_runes_keep_the_maturing_altar_recipe() {
        assert_eq!(
            CRAFTING[0].inputs.map(|item| item.unwrap().id()),
            [
                "minecraft:iron_ingot",
                "MAGIC_LUMP_3",
                "minecraft:iron_ingot",
                "ANCIENT_RUNE_AIR",
                "minecraft:phantom_membrane",
                "ANCIENT_RUNE_WATER",
                "minecraft:iron_ingot",
                "MAGIC_LUMP_3",
                "minecraft:iron_ingot",
            ]
        );
        assert_eq!(CRAFTING[0].output.count, 4);
        assert_eq!(
            CRAFTING[1].inputs.map(|item| item.unwrap().id()),
            [
                "MAGIC_LUMP_3",
                "MAGICAL_GLASS",
                "MAGIC_LUMP_3",
                "MAGICAL_GLASS",
                "ANCIENT_RUNE_LIGHTNING",
                "MAGICAL_GLASS",
                "MAGIC_LUMP_3",
                "MAGICAL_GLASS",
                "MAGIC_LUMP_3",
            ]
        );
        assert_eq!(CRAFTING[1].research.id, "enchantment_rune");
        assert_eq!(CRAFTING[1].output.count, 1);
    }
}
