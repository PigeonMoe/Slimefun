// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;

use mod_api::{
    Compound, Host, ItemStack, PayloadId, Registrar, ServerHost, encode_payload, wire::Writer,
};

use super::{
    Availability, GROUPS, availability, catalog::{Entry, Group}, entry_icon, entry_name,
    group_entries, icon, ingredient_icon, locale, lookup, strip_colors, unlocked,
};
use crate::items::Kind;

const PAGE_SIZE: usize = 16;
const MAX_PAGE_BYTES: usize = 512 * 1024;
const MAX_COMPONENT_BYTES: usize = 8192;
// Far above the stock catalog, but bounded before multiplying a client's page.
const MAX_PAGE: i32 = 1023;

#[derive(Debug, PartialEq, Eq, mod_api::PayloadCodec)]
struct CatalogQuery {
    nonce: i32,
    page: i32,
}

#[derive(Debug, PartialEq, Eq, mod_api::PayloadCodec)]
struct CatalogPage {
    nonce: i32,
    page: i32,
    next_page: Option<i32>,
    recipes: Vec<CatalogRecipe>,
}

#[derive(Debug, PartialEq, Eq, mod_api::PayloadCodec)]
struct CatalogRecipe {
    id: String,
    title: String,
    station: String,
    requirement: String,
    columns: u8,
    inputs: Vec<CatalogIngredient>,
    output: CatalogItem,
}

#[derive(Debug, PartialEq, Eq, mod_api::PayloadCodec)]
struct CatalogIngredient {
    slot: u8,
    alternatives: Vec<CatalogItem>,
}

#[derive(Debug, PartialEq, Eq, mod_api::PayloadCodec)]
struct CatalogItem {
    key: String,
    id: String,
    name: String,
    count: i32,
    components: Vec<u8>,
}

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let mut handlers = registrar.payload_handlers();
    let mut channels = handlers.registrar("1").optional();
    let pages = channels.play_to_client_typed::<CatalogPage>(
        PayloadId("pigeon_slimefun_catalog:page".into()),
        |_, _| Ok(()),
    );
    channels.play_to_server_typed(
        PayloadId("pigeon_slimefun_catalog:query".into()),
        move |query: CatalogQuery, context| {
            let Ok(player) = context.player() else {
                return Ok(());
            };
            let Ok(host) = context.server_host() else {
                return Ok(());
            };
            let saved = player.persistent_data(host);
            let response = page(host, saved.as_ref(), query);
            let _ = pages.reply(context, &response);
            Ok(())
        },
    );
}

fn page(host: &dyn ServerHost, saved: Option<&Compound>, query: CatalogQuery) -> CatalogPage {
    let mut page = CatalogPage {
        nonce: query.nonce,
        page: query.page,
        next_page: None,
        recipes: Vec::new(),
    };
    let dimension = host.dimension();
    if !(0..=MAX_PAGE).contains(&query.page) || !availability::world_enabled(dimension) {
        return page;
    }
    let availability = Availability::for_dimension(dimension);
    let month = host.server_local_minute()[1];
    let mut entries = visible_entries(saved, availability, month)
        .skip(query.page as usize * PAGE_SIZE);
    page.recipes.extend(entries.by_ref().take(PAGE_SIZE).filter_map(|entry| recipe(host, entry, saved, availability)));
    if entries.next().is_some() && query.page < MAX_PAGE {
        page.next_page = Some(query.page + 1);
    }
    if encode_payload(&page, MAX_PAGE_BYTES).is_err() {
        // Preserve page boundaries if a future guide adds unusually large item lore.
        let per_recipe = (MAX_PAGE_BYTES - 64) / PAGE_SIZE;
        page.recipes.retain(|recipe| encode_payload(recipe, per_recipe).is_ok());
    }
    page
}

fn visible_entries<'a>(
    saved: Option<&'a Compound>,
    availability: Availability<'a>,
    month: i32,
) -> impl Iterator<Item = &'static Entry> + 'a {
    let mut seen = BTreeSet::new();
    GROUPS.iter()
        .filter(move |group| group.in_season(month) && group_unlocked(group, saved, availability))
        .flat_map(group_entries)
        .filter(move |entry| availability.allows(entry) && unlocked(saved, entry))
        .filter(move |entry| seen.insert(entry.id))
}

fn group_unlocked(group: &Group, saved: Option<&Compound>, availability: Availability<'_>) -> bool {
    group.parent
        .and_then(|key| GROUPS.iter().find(|parent| parent.key == key))
        .is_none_or(|parent| group_entries(parent)
            .filter(|entry| availability.allows(entry))
            .all(|entry| unlocked(saved, entry)))
}

fn recipe(
    host: &dyn Host,
    entry: &Entry,
    saved: Option<&Compound>,
    availability: Availability<'_>,
) -> Option<CatalogRecipe> {
    let id = format!("slimefun:{}", entry.id);
    let title = strip_colors(&entry_name(entry));
    let mut inputs = Vec::new();
    for (slot, ingredient) in entry.ingredients.into_iter().enumerate() {
        let Some(ingredient) = ingredient else { continue };
        let locked = lookup(ingredient.kind.id())
            .filter(|linked| availability.allows(linked) && !unlocked(saved, linked));
        let (stack, name) = if let Some(locked) = locked {
            let name = entry_name(locked);
            let stack = icon(host, "minecraft:barrier", &name, &["&4&l未解锁"])?;
            (stack, strip_colors(&name))
        } else {
            let name = entry.visuals.iter().find(|visual| visual.slot == slot)
                .map(|visual| strip_colors(visual.label))
                .unwrap_or_else(|| item_name(ingredient.kind));
            (ingredient_icon(host, entry, slot, ingredient)?, name)
        };
        let mut display = item(host, &stack, item_key(ingredient.kind), name)?;
        display.count = ingredient.count as i32;
        inputs.push(CatalogIngredient {
            slot: slot as u8,
            alternatives: vec![display],
        });
    }
    let station = match entry.recipe_type.name {
        "Barter Drop" => "猪灵交易",
        station => station,
    };
    let requirement = entry.research.map_or_else(String::new, |topic| {
        format!("研究 · {}", locale::research_name(topic.id))
    });
    let output = entry_icon(host, entry, entry.item.count)?;
    let output_key = if entry.label.is_some() { id.clone() } else { item_key(entry.item.kind) };
    Some(CatalogRecipe {
        id,
        station: strip_colors(station),
        requirement,
        columns: 3,
        inputs,
        output: item(host, &output, output_key, title.clone())?,
        title,
    })
}

fn item_key(kind: Kind) -> String {
    match kind {
        Kind::Slimefun(item) => format!("slimefun:{}", item.id),
        _ => kind.id().to_owned(),
    }
}

fn item_name(kind: Kind) -> String {
    match kind {
        Kind::Slimefun(item) => strip_colors(locale::item_name(item.id).unwrap_or(item.name)),
        Kind::Potion(_) => "药水".to_owned(),
        Kind::Vanilla(id) => id.trim_start_matches("minecraft:").replace('_', " "),
    }
}

fn item(host: &dyn Host, stack: &ItemStack, key: String, name: String) -> Option<CatalogItem> {
    let mut writer = Writer::new();
    writer.limit(MAX_COMPONENT_BYTES);
    if let Some(components) = stack.components() {
        writer.put(components.as_nbt());
    }
    if writer.error().is_some() {
        return None;
    }
    Some(CatalogItem {
        key,
        id: stack.item().id(host),
        name,
        count: stack.count() as i32,
        components: writer.into_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, Tag, decode_payload, testing::FakeHost, wire::Reader};

    fn availability() -> Availability<'static> {
        Availability {
            output_chest: true,
            tickers: crate::ticker_policy::Tickers::default(),
            dimension: None,
        }
    }

    fn learned(id: &str) -> Compound {
        let key = format!("slimefun:{id}");
        Compound::from([(
            "PlayerPersisted".into(),
            Tag::compound([(key.as_str(), Tag::Byte(1))]),
        )])
    }

    #[test]
    fn query_and_page_match_typed_wire_format() {
        let query = CatalogQuery { nonce: 23, page: 2 };
        let query_bytes = encode_payload(&query, 8).unwrap();
        assert_eq!(query_bytes.len(), 8);
        assert_eq!(decode_payload::<CatalogQuery>(&query_bytes).unwrap(), query);
        let response = CatalogPage {
            nonce: 23,
            page: 2,
            next_page: Some(3),
            recipes: Vec::new(),
        };
        let bytes = encode_payload(&response, MAX_PAGE_BYTES).unwrap();
        assert_eq!(decode_payload::<CatalogPage>(&bytes).unwrap(), response);
    }

    #[test]
    fn visible_entries_follow_player_research() {
        let none: BTreeSet<_> = visible_entries(None, availability(), 1)
            .map(|entry| entry.id)
            .collect();
        assert!(none.contains("ENHANCED_CRAFTING_TABLE"));
        assert!(!none.contains("GRIND_STONE"));
        let unrelated = learned("not_a_real_research");
        assert!(!visible_entries(Some(&unrelated), availability(), 1)
            .any(|entry| entry.id == "GRIND_STONE"));
        let grind_stone = learned("grind_stone");
        assert!(visible_entries(Some(&grind_stone), availability(), 1)
            .any(|entry| entry.id == "GRIND_STONE"));
    }

    #[test]
    fn recipe_encodes_actual_slimefun_profile_and_grid() {
        mod_api::__private::initialize().unwrap();
        let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
        let mut host = FakeHost::new();
        let entry = lookup("BASIC_CIRCUIT_BOARD").unwrap();
        let needed = entry.ingredients.iter().flatten().map(|stack| stack.kind.material())
            .chain([entry.item.kind.material(), "minecraft:barrier"])
            .collect::<BTreeSet<_>>();
        host.registries
            .resolve_items(needed.into_iter().map(|id| Identifier::parse(id).unwrap()))
            .unwrap();
        let saved = learned(entry.research.unwrap().id);
        let recipe = recipe(&host, entry, Some(&saved), availability()).unwrap();
        assert_eq!(recipe.id, "slimefun:BASIC_CIRCUIT_BOARD");
        assert_eq!(recipe.output.id, "minecraft:activator_rail");
        assert_eq!(recipe.station, "生物掉落");
        assert_eq!(recipe.output.key, recipe.id);
        assert_eq!(recipe.output.count, entry.item.count as i32);
        assert_eq!(recipe.columns, 3);
        let slots: Vec<_> = entry.ingredients.iter().enumerate()
            .filter_map(|(slot, ingredient)| ingredient.map(|_| slot as u8))
            .collect();
        assert_eq!(recipe.inputs.iter().map(|input| input.slot).collect::<Vec<_>>(), slots);
        let components = Reader::decode::<Compound>(&recipe.output.components).unwrap();
        let expected = Tag::String("BASIC_CIRCUIT_BOARD".into());
        assert_eq!(
            components
                .get(&"minecraft:custom_data".into())
                .and_then(|custom| custom.get("PublicBukkitValues"))
                .and_then(|bukkit| bukkit.get(crate::items::ID_KEY)),
            Some(&expected),
        );
        let golem = &recipe.inputs.iter().find(|input| input.slot == 4).unwrap().alternatives[0];
        assert_eq!(golem.id, "minecraft:player_head");
        assert_eq!(golem.name, "铁傀儡");
        let golem_components = Reader::decode::<Compound>(&golem.components).unwrap();
        assert!(golem_components.contains_key(&"minecraft:profile".into()));
        let response = CatalogPage {
            nonce: 23,
            page: 0,
            next_page: None,
            recipes: vec![recipe],
        };
        let bytes = encode_payload(&response, MAX_PAGE_BYTES).unwrap();
        assert_eq!(decode_payload::<CatalogPage>(&bytes).unwrap(), response);
    }
}
