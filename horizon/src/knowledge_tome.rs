// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

use mod_api::{
    Compound, GameMode, Hand, HostTransaction, Identifier, ItemPlace, ItemResource, ItemStack,
    PlayerFlagReadError, PlayerRef, ServerHost, SoundSource, Tag, Text,
};

use crate::{
    elemental_staves::MAGICAL_BOOK_COVER,
    guide::{Guide, strip_colors},
    hologram_projector::colored_lore,
    items::{Kind, item},
    network_runtime::{paper_text, parse_uuid},
    recipes::{self, CraftingRecipe, Research, Stack},
    research,
};

item!(
    TOME_OF_KNOWLEDGE_SHARING,
    "enchanted_book",
    "Tome of Knowledge Sharing",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "tome_of_knowledge_sharing",
    name: "Sharing with friends",
    cost: 30,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        None,
        Some(Kind::Vanilla("minecraft:feather")),
        None,
        Some(Kind::Vanilla("minecraft:ink_sac")),
        Some(MAGICAL_BOOK_COVER),
        Some(Kind::Vanilla("minecraft:glass_bottle")),
        None,
        Some(Kind::Vanilla("minecraft:writable_book")),
        None,
    ],
    output: Stack {
        kind: TOME_OF_KNOWLEDGE_SHARING,
        count: 1,
    },
    research: &RESEARCH,
};

const LORE: [&str; 7] = [
    "&7Owner: &bNone",
    "",
    "&eRight Click&7 to bind this Tome to yourself",
    "",
    "",
    "&eRight Click&7 to obtain all Researches by",
    "&7the previously assigned Owner",
];

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == TOME_OF_KNOWLEDGE_SHARING.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(LORE.into_iter().map(colored_lore).collect()),
        );
    }
}

fn tome_lore(stack: &ItemStack) -> Result<&[Tag], &'static str> {
    match stack
        .components()
        .and_then(|parts| parts.as_nbt().get(&"minecraft:lore".into()))
    {
        Some(Tag::List(lines)) if lines.len() >= 2 => Ok(lines),
        _ => Err("This Tome has damaged Paper lore."),
    }
}

fn owner(stack: &ItemStack) -> Result<Option<u128>, &'static str> {
    let line = paper_text(&tome_lore(stack)?[1]).ok_or("This Tome has damaged Paper lore.")?;
    let text = strip_colors(&line);
    if text.is_empty() {
        Ok(None)
    } else {
        parse_uuid(&text)
            .map(Some)
            .ok_or("This Tome has an invalid Paper owner UUID.")
    }
}

fn uuid_string(uuid: u128) -> String {
    let text = format!("{uuid:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..]
    )
}

fn bind(stack: &ItemStack, holder: u128, name: &str) -> Result<ItemStack, &'static str> {
    if owner(stack)?.is_some() {
        return Err("This Tome is already bound.");
    }
    let mut lines = tome_lore(stack)?.to_vec();
    lines[0] = colored_lore(&format!("&7Owner: &b{name}"));
    lines[1] = colored_lore(&format!("&0{}", uuid_string(holder)));
    let mut updated = stack.clone();
    updated
        .apply_components(&Compound::from([(
            "minecraft:lore".into(),
            Tag::List(lines),
        )]))
        .map_err(|_| "Could not bind the Tome.")?;
    Ok(updated)
}

static TOPICS: LazyLock<BTreeMap<String, &'static str>> = LazyLock::new(|| {
    let mut topics = BTreeMap::new();
    // Exact IDs and titles from Paper's ResearchSetup. An archived research is
    // still transferable even if its gameplay hasn't yet been ported.
    for (id, name) in include_str!("knowledge_researches.tsv")
        .lines()
        .filter_map(|row| row.split_once('\t'))
    {
        topics.insert(format!("slimefun:{id}"), name);
    }
    for topic in recipes::researches().chain(crate::guide::research_topics()) {
        if !topic.id.is_empty() {
            topics.insert(format!("slimefun:{}", topic.id), topic.name);
        }
    }
    topics
});

static FLAGS: LazyLock<Vec<Identifier>> = LazyLock::new(|| {
    TOPICS
        .keys()
        .map(|key| Identifier::parse(key).expect("an authored Slimefun research key"))
        .collect()
});

fn owner_flags(host: &dyn ServerHost, uuid: u128) -> Result<BTreeSet<String>, &'static str> {
    let online = host.player(uuid).is_some();
    let stored = host
        .player_saved_true_flags(uuid, &FLAGS)
        .map_err(|failure| match failure {
            PlayerFlagReadError::Corrupt => "The original owner's saved research data is damaged.",
            _ => "The original owner's research is unavailable.",
        })?;
    let imported = if online {
        None
    } else {
        research::legacy_research_flags(host, uuid)?
    };
    if stored.is_none() && imported.is_none() {
        return Err("The original owner's research has not been saved or imported.");
    }
    Ok(stored
        .into_iter()
        .flatten()
        .map(|id| id.to_string())
        .chain(imported.into_iter().flatten())
        .filter(|id| TOPICS.contains_key(id))
        .collect())
}

fn use_tome(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
    guide: &Guide,
) -> Result<(), &'static str> {
    if !research::can_use(host, player, &RESEARCH) {
        return Err("Unlock Sharing with friends before using a Tome of Knowledge.");
    }
    let held = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    };
    let resource = ItemResource::of(stack);
    let Some(donor) = owner(stack)? else {
        let name = player.name(host).ok_or("The Tome holder is unavailable.")?;
        let updated = bind(stack, player.uuid(), &name)?;
        host.transaction(|host, _| {
            if held.stack(host).as_ref() != Some(stack) {
                return Err("The Tome in that hand changed.");
            }
            // Binding edits the actual stack, including in creative mode.
            let slot = match hand {
                Hand::Main => player
                    .selected_slot(host)
                    .and_then(|slot| u32::try_from(slot).ok())
                    .filter(|slot| *slot < 9)
                    .ok_or("The Tome holder is unavailable.")?,
                Hand::Off => 40,
            };
            player
                .set_inventory_slot(host, slot, Some(updated))
                .map_err(|_| "Could not bind the Tome.")
        })?;
        player.play_sound(
            host,
            "minecraft:entity.player.levelup",
            SoundSource::Master,
            1.0,
            1.0,
        );
        return Ok(());
    };
    if donor == player.uuid() {
        return Err("You cannot use the Tome of Knowledge on yourself.");
    }

    let source = owner_flags(host, donor)?;
    let mut data = player
        .persistent_data(host)
        .ok_or("Your current research data is unavailable.")?;
    let granted = research::grant_shared(&mut data, &source)?;
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    host.transaction(|host, tx| {
        if held.stack(host).as_ref() != Some(stack)
            || (!creative && held.extract(host, tx, &resource, 1) != 1)
        {
            return Err("The Tome in that hand changed.");
        }
        if !granted.is_empty() {
            player
                .set_persistent_data(host, data)
                .map_err(|_| "Could not save the copied research.")?;
        }
        Ok(())
    })?;
    for id in granted {
        guide.announce_shared_research(host, player, TOPICS[&id]);
    }
    Ok(())
}

pub(crate) fn use_held(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
    guide: &Guide,
) -> bool {
    if !TOME_OF_KNOWLEDGE_SHARING.matches(host, stack) {
        return false;
    }
    if let Err(reason) = use_tome(host, player, hand, stack, guide) {
        let _ = player.tell(host, Text::literal(reason));
    }
    true
}

#[cfg(test)]
mod transfer_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Tag, testing::FakeHost};

    #[test]
    fn a_paper_tome_binds_its_exact_uuid_and_only_replaces_the_two_owner_lines() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:enchanted_book").unwrap()])
            .unwrap();
        let mut tome = TOME_OF_KNOWLEDGE_SHARING.stack(&host, 1).unwrap();
        let mut extra = Compound::new();
        decorate(TOME_OF_KNOWLEDGE_SHARING.id(), &mut extra);
        extra.insert("minecraft:repair_cost".into(), Tag::Int(7));
        tome.apply_components(&extra).unwrap();
        assert_eq!(owner(&tome), Ok(None));
        let uuid = 0x7ab8b15dd1204725ab3c992723100f4b;
        let live = bind(&tome, uuid, "OriginalName").unwrap();
        assert_eq!(owner(&live), Ok(Some(uuid)));
        assert_eq!(
            paper_text(&tome_lore(&live).unwrap()[1]).as_deref(),
            Some("§07ab8b15d-d120-4725-ab3c-992723100f4b")
        );
        assert_eq!(
            &tome_lore(&live).unwrap()[2..],
            &tome_lore(&tome).unwrap()[2..]
        );
        assert_eq!(
            live.components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:repair_cost".into()),
            Some(&Tag::Int(7))
        );
        assert!(bind(&live, 4, "AnotherName").is_err());

        let mut old = live.clone();
        let mut lines = tome_lore(&live).unwrap().to_vec();
        lines[0] = Tag::String("§7Owner: §bDifferentDisplayName".into());
        lines[1] = Tag::String("§07ab8b15d-d120-4725-ab3c-992723100f4b".into());
        old.apply_components(&Compound::from([(
            "minecraft:lore".into(),
            Tag::List(lines.clone()),
        )]))
        .unwrap();
        assert_eq!(owner(&old), Ok(Some(uuid)));
        lines[1] = colored_lore("&0not-a-valid-uuid");
        old.apply_components(&Compound::from([(
            "minecraft:lore".into(),
            Tag::List(lines),
        )]))
        .unwrap();
        assert!(owner(&old).is_err());
    }

    #[test]
    fn the_sharing_catalog_retains_paper_ids_and_excludes_settings() {
        assert_eq!(
            TOPICS["slimefun:tome_of_knowledge_sharing"],
            "Sharing with friends"
        );
        assert_eq!(TOPICS["slimefun:trident"], "Trident");
        assert!(TOPICS.contains_key("slimefun:grind_stone"));
        assert!(!TOPICS.contains_key("slimefun:research_fireworks"));
        assert!(!TOPICS.contains_key("slimefun:pending_research"));
        assert!(FLAGS.len() >= 258 && FLAGS.len() <= 512);
        assert_eq!(CRAFTING.inputs[4], Some(MAGICAL_BOOK_COVER));
        assert_eq!(
            CRAFTING.inputs[7],
            Some(Kind::Vanilla("minecraft:writable_book"))
        );
    }
}
