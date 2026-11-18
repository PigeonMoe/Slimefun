// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockEntityRef, Capability, Compound, Hand,
    Holder, Host, ITEMS, ItemPlace, ItemResource, ItemStack, LevelRef, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, ParticleKey, ParticleOptions, PlayerRef, Registrar,
    ResourceHandler, ServerHost, SoundSource, Tag, Text, Transaction,
};

use crate::{
    items::{HeadTexture, Kind, REDSTONE_ALLOY, item, item_id, text},
    network_recipes::{BLISTERING_INGOT_3, CARGO_MOTOR},
    network_runtime,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    CRAFTING_MOTOR,
    "player_head",
    "Crafting Motor",
    "gold",
    None,
    Some(HeadTexture {
        profile: [-395576959, 1258698793, -1407735318, 136132388],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzEwMDM2MjA4OTlmMWFmYTI3MWU4ZTUyMWVjYmVlMjk3N2EwNmM4NTI5ZDNmMzg5ZThjYzA0YWYwNmQ4Yzc5NDAifX19",
    })
);
item!(
    ENHANCED_AUTO_CRAFTER,
    "player_head",
    "Auto-Crafter (Enhanced)",
    "dark_green",
    None,
    Some(HeadTexture {
        profile: [2009036015, -426362241, -1423845617, -268730100],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzUwMzgyOTgzMDZhNWUyODU4NGRmMzllODg4OTY5MTdjMzhkNDBhMzI2MjI2ZDhjODMwNzA3MjNjOTU3OThiMjQifX19",
    })
);

pub(crate) const CAPACITY: u32 = 256;
pub(crate) const COST: u32 = 16;
pub(crate) const RESEARCH: Research = Research {
    id: "auto_crafting",
    name: "Automatic Crafting",
    cost: 30,
};

const TABLE: Option<Kind> = Some(Kind::Vanilla("minecraft:crafting_table"));
pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    CraftingRecipe {
        inputs: [
            TABLE,
            Some(BLISTERING_INGOT_3),
            TABLE,
            Some(REDSTONE_ALLOY),
            Some(CARGO_MOTOR),
            Some(REDSTONE_ALLOY),
            TABLE,
            Some(BLISTERING_INGOT_3),
            TABLE,
        ],
        output: Stack {
            kind: CRAFTING_MOTOR,
            count: 2,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(CRAFTING_MOTOR),
            None,
            TABLE,
            Some(Kind::Vanilla("minecraft:dispenser")),
            TABLE,
            None,
            Some(CARGO_MOTOR),
            None,
        ],
        output: Stack {
            kind: ENHANCED_AUTO_CRAFTER,
            count: 1,
        },
        research: &RESEARCH,
    },
];

const PREVIEW_SLOTS: [usize; 9] = [11, 12, 13, 20, 21, 22, 29, 30, 31];
const RESULT: usize = 24;
const CONTROL: usize = 49;
static PARTICLE: OnceLock<ParticleKey> = OnceLock::new();

pub(crate) fn register(registrar: &mut dyn Registrar) {
    let _ = PARTICLE.set(registrar.particle_key("minecraft:happy_villager"));
}

pub(crate) fn refuses_placement(
    host: &mut dyn ServerHost,
    event: &mod_api::UseOnEvent<'_>,
) -> bool {
    if CRAFTING_MOTOR.matches(host, event.stack) {
        return true;
    }
    if !ENHANCED_AUTO_CRAFTER.matches(host, event.stack) {
        return false;
    }
    let Some(player) = host.player(event.player) else {
        return true;
    };
    if research::can_use(host, player, &RESEARCH) {
        return false;
    }
    tell(
        host,
        player,
        "Unlock the Auto-Crafter with /sf research auto_crafting.",
    );
    true
}

pub(crate) fn item_by_id(id: &str) -> Option<Kind> {
    [CRAFTING_MOTOR, ENHANCED_AUTO_CRAFTER]
        .into_iter()
        .find(|kind| kind.id() == id)
}

pub(crate) fn is_machine(id: &str) -> bool {
    id == ENHANCED_AUTO_CRAFTER.id()
}

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id == CRAFTING_MOTOR.id() {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(vec![
                Tag::String("".into()),
                text("Important component of Auto-Crafters", "gray", false),
            ]),
        );
    } else if is_machine(id) {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(vec![
                Tag::String("".into()),
                text("Place this machine on top of a", "white", false),
                text("chest or similar and make it craft", "white", false),
                text("anything that can be crafted using an", "white", false),
                text("Enhanced Crafting Table", "yellow", false),
                Tag::String("".into()),
                text("Advanced Machine", "red", true),
                text("⇨ ⚡ 16 J/Item", "gray", false),
            ]),
        );
    }
}

type Pos = [i32; 3];
type Inventory = Capability<dyn ResourceHandler<ItemResource>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Selection {
    recipe: Option<String>,
    disabled: bool,
}

const STATE: AttachmentType<Selection> = AttachmentType {
    namespace: "slimefun",
    name: "auto_crafter",
    default: Selection::default,
    codec: Some(AttachmentCodec {
        encode: |selected| {
            let mut record = Compound::new();
            if let Some(recipe) = &selected.recipe {
                record.insert("recipe".into(), Tag::String(recipe.as_str().into()));
                if selected.disabled {
                    record.insert("disabled".into(), Tag::Byte(1));
                }
            }
            Ok(record)
        },
        decode: |record| {
            read_selection(record, "recipe", "disabled").map_err(|_| AttachmentError::Decode)
        },
    }),
    copy_on_death: false,
    sync: false,
};

fn read_selection(
    record: &Compound,
    recipe: &str,
    disabled: &str,
) -> Result<Selection, &'static str> {
    let recipe = match record.get(&recipe.into()) {
        None => None,
        Some(Tag::String(id)) => Some(
            id.to_string()
                .ok()
                .filter(|id| {
                    !id.is_empty()
                        && id.len() <= 128
                        && id
                            .bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                })
                .ok_or("The existing Auto-Crafter recipe ID is invalid.")?,
        ),
        _ => return Err("The existing Auto-Crafter recipe ID is invalid."),
    };
    let disabled = match record.get(&disabled.into()) {
        None => false,
        Some(Tag::Byte(_)) if recipe.is_some() => true,
        Some(_) => return Err("The existing Auto-Crafter enabled flag is invalid."),
    };
    Ok(Selection { recipe, disabled })
}

fn read(host: &mut dyn ServerHost, pos: Pos) -> Result<Option<Selection>, &'static str> {
    STATE
        .get_existing(host, Holder::BlockEntity(pos))
        .map_err(|_| "The saved Auto-Crafter recipe could not be read.")
}

fn store(host: &mut dyn ServerHost, pos: Pos, state: Selection) -> Result<(), &'static str> {
    STATE
        .set(host, Holder::BlockEntity(pos), state)
        .map_err(|_| "The Auto-Crafter recipe could not be saved.")
}

fn paper_selection(data: &Compound) -> Result<Selection, &'static str> {
    match data.get(&"PublicBukkitValues".into()) {
        Some(Tag::Compound(pdc)) => {
            read_selection(pdc, "slimefun:recipe_key", "slimefun:recipe_enabled")
        }
        None => Ok(Selection::default()),
        Some(_) => Err("The Paper skull's persistent recipe data is malformed."),
    }
}

pub(crate) fn restore_paper(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    row: &toml::Table,
) -> Result<(), String> {
    if row.contains_key("paper_inventory") {
        crate::storage::validate_inventory_preset(row, ENHANCED_AUTO_CRAFTER.id())?;
        if !crate::storage::decode_archived_slots(host, row, 54)?.is_empty() {
            return Err("Paper Auto-Crafter recipes belong to the skull's saved data. The menu items remain archived.".into());
        }
    }
    if read(host, pos)?.is_some() {
        return Ok(());
    }
    let body = host
        .block_entity(level, pos)
        .ok_or("Load the Paper Auto-Crafter skull first.")?;
    let data = body
        .data(host)
        .ok_or("The Paper Auto-Crafter skull data could not be read.")?;
    store(host, pos, paper_selection(&data)?)?;
    Ok(())
}

fn recipe(selected: &str) -> Option<&'static CraftingRecipe> {
    crate::recipes::crafting().find(|recipe| recipe.output.kind.id() == selected)
}

fn storage_block(id: &str) -> bool {
    matches!(
        id,
        "minecraft:chest"
            | "minecraft:trapped_chest"
            | "minecraft:barrel"
            | "minecraft:shulker_box"
    ) || id
        .strip_prefix("minecraft:")
        .is_some_and(|path| path.ends_with("_shulker_box"))
}

fn inventory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    [x, y, z]: Pos,
) -> Option<(Pos, Inventory)> {
    let below = [x, y.checked_sub(1)?, z];
    let block = level.block(host, below)?;
    if !storage_block(&block.id(host)) {
        return None;
    }
    Some((below, level.capability(host, &ITEMS, below, None)?))
}

struct Payment {
    slot: usize,
    stack: ItemStack,
    amount: u32,
}

fn payments(
    host: &dyn Host,
    recipe: &CraftingRecipe,
    slots: Vec<Option<ItemStack>>,
) -> Option<Vec<Payment>> {
    // Paper requires a free slot before consuming any ingredient.
    if slots.iter().all(Option::is_some) {
        return None;
    }
    let mut available: Vec<_> = slots
        .iter()
        .map(|stack| stack.as_ref().map_or(0, ItemStack::count))
        .collect();
    for expected in recipe.inputs.iter().flatten() {
        let slot = slots.iter().enumerate().find_map(|(slot, stack)| {
            (available[slot] > 0 && expected.matches(host, stack.as_ref()?)).then_some(slot)
        })?;
        available[slot] -= 1;
    }
    Some(
        slots
            .into_iter()
            .zip(available)
            .enumerate()
            .filter_map(|(slot, (stack, left))| {
                let stack = stack?;
                let amount = stack.count() - left;
                (amount != 0).then_some(Payment {
                    slot,
                    stack,
                    amount,
                })
            })
            .collect(),
    )
}

fn leftover(host: &dyn Host, payment: &Payment) -> Result<Option<ItemStack>, ()> {
    let material = payment.stack.item().id(host);
    let item = match material.as_str() {
        "minecraft:water_bucket" | "minecraft:lava_bucket" | "minecraft:milk_bucket" => {
            "minecraft:bucket"
        }
        "minecraft:dragon_breath" | "minecraft:potion" | "minecraft:honey_bottle" => {
            "minecraft:glass_bottle"
        }
        _ => return Ok(None),
    };
    Kind::Vanilla(item)
        .stack(host, payment.amount)
        .map(Some)
        .ok_or(())
}

fn insert_all(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    storage: &Inventory,
    size: usize,
    stack: &ItemStack,
) -> Result<(), ()> {
    let resource = ItemResource::of(stack);
    let mut left = stack.count();
    for occupied in [true, false] {
        for slot in 0..size {
            if (storage.amount(host, slot) != 0) == occupied {
                left = left
                    .checked_sub(storage.insert(host, tx, slot, &resource, left))
                    .ok_or(())?;
                if left == 0 {
                    return Ok(());
                }
            }
        }
    }
    Err(())
}

pub(crate) fn stage(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: Pos,
) -> Result<(), ()> {
    let selected = read(host, pos)
        .map_err(|_| ())?
        .filter(|selected| !selected.disabled)
        .ok_or(())?;
    let recipe = selected.recipe.as_deref().and_then(recipe).ok_or(())?;
    let (below, storage) = inventory(host, level, pos).ok_or(())?;
    let partner = if storage.size(host) > 27 {
        Some(
            crate::output_chest::double_chest_partner(
                below,
                level.block_property(host, below, "facing").as_deref(),
                level.block_property(host, below, "type").as_deref(),
            )
            .ok_or(())?,
        )
    } else {
        None
    };
    run(host, tx, &storage, recipe, |host, stack, from_chest| {
        [Some(below), partner].into_iter().flatten().all(|target| {
            let (from, to) = if from_chest {
                (target, pos)
            } else {
                (pos, target)
            };
            level.allow_inventory_transfer(host, from, to, stack)
        })
    })
}

fn run(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    storage: &Inventory,
    recipe: &CraftingRecipe,
    mut allows: impl FnMut(&mut dyn ServerHost, &ItemStack, bool) -> bool,
) -> Result<(), ()> {
    let size = storage.size(host);
    let slots = (0..size)
        .map(|slot| {
            storage
                .resource(host, slot)?
                .to_stack(storage.amount(host, slot))
        })
        .collect();
    let payments = payments(host, recipe, slots).ok_or(())?;
    let mut output = recipe
        .output
        .kind
        .stack(host, recipe.output.count)
        .ok_or(())?;
    crate::backpacks::prepare_automated_craft(
        host,
        tx,
        payments.iter().map(|payment| &payment.stack),
        &mut output,
    )
    .map_err(|_| ())?;
    for payment in &payments {
        let resource = ItemResource::of(&payment.stack);
        let transfer = resource.to_stack(payment.amount).ok_or(())?;
        if !allows(host, &transfer, true)
            || storage.extract(host, tx, payment.slot, &resource, payment.amount) != payment.amount
        {
            return Err(());
        }
    }
    if !allows(host, &output, false) {
        return Err(());
    }
    insert_all(host, tx, storage, size, &output)?;
    for payment in &payments {
        if let Some(stack) = leftover(host, payment)? {
            if !allows(host, &stack, false) {
                return Err(());
            }
            insert_all(host, tx, storage, size, &stack)?;
        }
    }
    Ok(())
}

pub(crate) fn complete(host: &mut dyn ServerHost, level: LevelRef, [x, y, z]: Pos) {
    if let Some(&particle) = PARTICLE.get() {
        let at = [f64::from(x) + 0.5, f64::from(y) + 0.8, f64::from(z) + 0.5];
        let _ = level.send_particles(
            host,
            &ParticleOptions::simple(particle),
            None,
            at,
            [0.0; 3],
            0.0,
            6,
        );
    }
}

#[derive(Clone)]
struct Viewer {
    level: LevelRef,
    pos: Pos,
    body: BlockEntityRef,
    window: i32,
    saved: Selection,
    candidate: Option<String>,
}

static VIEWERS: Mutex<BTreeMap<u128, Viewer>> = Mutex::new(BTreeMap::new());

pub(crate) fn clear() {
    VIEWERS.lock().unwrap().clear();
}

pub(crate) fn closed(event: &MenuClosedEvent) {
    let mut viewers = VIEWERS.lock().unwrap();
    if viewers
        .get(&event.uuid)
        .is_some_and(|viewer| viewer.window == event.window)
    {
        viewers.remove(&event.uuid);
    }
}

fn named_icon(host: &dyn Host, id: &'static str, name: &str, color: &str) -> Option<ItemStack> {
    let mut icon = Kind::Vanilla(id).stack(host, 1)?;
    icon.set_components(Compound::from([(
        "minecraft:custom_name".into(),
        text(name, color, false),
    )]))
    .ok()?;
    Some(icon)
}

fn preview(
    host: &dyn Host,
    recipe: &CraftingRecipe,
    candidate: bool,
    disabled: bool,
) -> Vec<Option<ItemStack>> {
    let background = named_icon(host, "minecraft:gray_stained_glass_pane", " ", "gray");
    let mut items = vec![background; 54];
    for (slot, kind) in PREVIEW_SLOTS.into_iter().zip(recipe.inputs) {
        items[slot] = kind.and_then(|kind| kind.stack(host, 1));
    }
    items[RESULT] = recipe.output.kind.stack(host, recipe.output.count);
    let (control, label, color) = if candidate {
        ("minecraft:crafting_table", "Select this recipe", "green")
    } else if disabled {
        (
            "minecraft:redstone_torch",
            "Disabled. Click to resume or right-click to delete.",
            "red",
        )
    } else {
        (
            "minecraft:barrier",
            "Enabled. Click to pause or right-click to delete.",
            "green",
        )
    };
    items[CONTROL] = named_icon(host, control, label, color);
    items
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, message: &str) {
    let _ = player.tell(host, Text::literal(message));
}

fn click_sound(host: &mut dyn ServerHost, player: PlayerRef) {
    let _ = player.play_sound(
        host,
        "minecraft:ui.button.click",
        SoundSource::Players,
        1.0,
        1.0,
    );
}

fn present(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    saved: Selection,
    candidate: Option<String>,
) -> Result<(), &'static str> {
    let selected = candidate
        .as_deref()
        .or(saved.recipe.as_deref())
        .ok_or("Sneak and right-click with the item to set an Auto-Crafter recipe.")?;
    let recipe =
        recipe(selected).ok_or("That item has no available Enhanced Crafting Table recipe.")?;
    if candidate.is_some()
        && !recipe.research.id.is_empty()
        && !research::can_use(host, player, recipe.research)
    {
        return Err("Unlock this recipe in your Slimefun research before selecting it.");
    }
    let body = host
        .block_entity(level, pos)
        .ok_or("The Auto-Crafter is no longer loaded.")?;
    let items = preview(host, recipe, candidate.is_some(), saved.disabled);
    let window = host
        .open_block_inventory_menu(
            player,
            body,
            6,
            Text::literal("Auto-Crafter (Enhanced)"),
            items,
            MenuReplace::InPlace,
        )
        .map_err(|_| "The Auto-Crafter menu could not open.")?;
    if let Some(window) = window {
        VIEWERS.lock().unwrap().insert(
            player.uuid(),
            Viewer {
                level,
                pos,
                body,
                window,
                saved,
                candidate,
            },
        );
        click_sound(host, player);
    }
    Ok(())
}

pub(crate) fn use_at(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    use_item: bool,
) -> bool {
    if !network_runtime::factory_identity(host, level, pos)
        .as_deref()
        .is_some_and(is_machine)
    {
        return false;
    }
    let activate = |host: &mut dyn ServerHost| {
        if !research::can_use(host, player, &RESEARCH) {
            return Err("Unlock the Auto-Crafter with /sf research auto_crafting.");
        }
        if inventory(host, level, pos).is_none() {
            return Err(
                "Place the Auto-Crafter above a chest, trapped chest, barrel or shulker box.",
            );
        }
        let selected = read(host, pos)?.unwrap_or_default();
        let candidate = if player.entity().is_crouching(host) == Some(true) {
            let place = ItemPlace::PlayerHand {
                player: player.uuid(),
                hand: Hand::Main,
            };
            let missing = "Hold an item made by the Enhanced Crafting Table to set its recipe.";
            let item = use_item
                .then(|| place.stack(host))
                .flatten()
                .ok_or(missing)?;
            let selected = item_id(&item)
                .filter(|id| {
                    recipe(id).is_some_and(|recipe| recipe.output.kind.matches(host, &item))
                })
                .ok_or(missing)?;
            Some(selected)
        } else {
            None
        };
        present(host, player, level, pos, selected, candidate)
    };
    if let Err(message) = activate(host) {
        tell(host, player, message);
    }
    true
}

pub(crate) fn clicked(host: &mut dyn ServerHost, event: &MenuClickedEvent) {
    if event.slot != CONTROL as i32 || event.mode != 0 || !matches!(event.button, 0 | 1) {
        return;
    }
    let viewer = VIEWERS.lock().unwrap().get(&event.uuid).cloned();
    let Some(viewer) = viewer.filter(|viewer| viewer.window == event.window) else {
        return;
    };
    let Some(player) = host.player(event.uuid) else {
        return;
    };
    if player.entity().level(host) != Some(viewer.level)
        || host.block_entity(viewer.level, viewer.pos) != Some(viewer.body)
        || !network_runtime::factory_identity(host, viewer.level, viewer.pos)
            .as_deref()
            .is_some_and(is_machine)
        || !research::can_use(host, player, &RESEARCH)
    {
        host.close_menu(event.uuid);
        return;
    }
    let result = apply_click(host, player, viewer, event.button);
    if let Err(message) = result {
        host.close_menu(event.uuid);
        tell(host, player, message);
    }
}

fn apply_click(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    viewer: Viewer,
    button: i32,
) -> Result<(), &'static str> {
    let mut saved = read(host, viewer.pos)?.unwrap_or_default();
    if saved != viewer.saved || inventory(host, viewer.level, viewer.pos).is_none() {
        return Err("The Auto-Crafter settings or inventory changed. Reopen its menu.");
    }
    if let Some(candidate) = viewer.candidate {
        let recipe =
            recipe(&candidate).ok_or("That Enhanced Crafting Table recipe is unavailable.")?;
        if !recipe.research.id.is_empty() && !research::can_use(host, player, recipe.research) {
            return Err("Unlock this recipe in your Slimefun research before selecting it.");
        }
        saved.recipe = Some(candidate);
        store(host, viewer.pos, saved.clone())?;
        tell(host, player, "The Auto-Crafter recipe was set.");
        return present(host, player, viewer.level, viewer.pos, saved, None);
    }
    let message = if button == 1 {
        saved = Selection::default();
        "The Auto-Crafter recipe was removed."
    } else {
        saved.disabled = !saved.disabled;
        if saved.disabled {
            "The Auto-Crafter was paused."
        } else {
            "The Auto-Crafter was enabled."
        }
    };
    store(host, viewer.pos, saved)?;
    host.close_menu(player.uuid());
    click_sound(host, player);
    tell(host, player, message);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{HostTransaction, Identifier, testing::FakeHost};

    #[test]
    fn paper_skull_pdc_keeps_byte_presence_and_rejects_a_wrong_type() {
        let pdc = |flag| {
            Compound::from([(
                "PublicBukkitValues".into(),
                Tag::compound([
                    ("slimefun:recipe_key", Tag::String("CRAFTING_MOTOR".into())),
                    ("slimefun:recipe_enabled", flag),
                    ("other-plugin:marker", Tag::String("friendly".into())),
                ]),
            )])
        };
        let raw = pdc(Tag::Byte(0));
        let restored = paper_selection(&raw).unwrap();
        assert_eq!(restored.recipe.as_deref(), Some("CRAFTING_MOTOR"));
        assert!(restored.disabled);
        let codec = STATE.codec.as_ref().unwrap();
        assert_eq!(
            (codec.decode)(&(codec.encode)(&restored).unwrap()).unwrap(),
            restored
        );
        assert!(paper_selection(&pdc(Tag::String("false".into()))).is_err());
        assert_eq!(paper_selection(&Compound::new()), Ok(Selection::default()));
        assert_eq!(recipe("CRAFTING_MOTOR").unwrap().output.count, 2);
        assert_eq!(recipe("ENHANCED_AUTO_CRAFTER").unwrap().research.cost, 30);
        assert_eq!(PREVIEW_SLOTS, [11, 12, 13, 20, 21, 22, 29, 30, 31]);
    }

    #[test]
    fn a_real_container_rolls_back_payment_and_output_if_the_remainder_does_not_fit() {
        let mut host = FakeHost::new();
        let water = Kind::Vanilla("minecraft:water_bucket");
        let bucket = Kind::Vanilla("minecraft:bucket");
        let output = Kind::Vanilla("minecraft:cake");
        host.registries
            .resolve_items(
                [water, bucket, output].map(|item| Identifier::parse(item.id()).unwrap()),
            )
            .unwrap();
        let chest = [0, 65, 0];
        let level = host.level.level();
        let raw = water.stack(&host, 1).unwrap();
        let recipe = CraftingRecipe {
            inputs: [Some(water), None, None, None, None, None, None, None, None],
            output: Stack {
                kind: output,
                count: 2,
            },
            research: &RESEARCH,
        };
        for (size, output_allowed) in [(2, true), (3, false), (3, true)] {
            let container = host
                .level
                .block_entity(chest)
                .with_items(size)
                .items
                .as_mut()
                .unwrap();
            container.limit = 1;
            container.slots[0] = Some(raw.clone());
            let result = host.transaction(|host, tx| {
                let storage = level.capability(host, &ITEMS, chest, None).unwrap();
                run(host, tx, &storage, &recipe, |_, _, from_chest| {
                    output_allowed || from_chest
                })
            });
            let contents = host
                .level
                .block_entity_at(chest)
                .unwrap()
                .items
                .as_ref()
                .unwrap();
            if size == 2 || !output_allowed {
                assert_eq!(result, Err(()));
                assert_eq!(contents.count(water.id()), 1);
                assert!(contents.slots[1..].iter().all(Option::is_none));
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(contents.count(output.id()), 2);
                assert_eq!(contents.count(bucket.id()), 1);
                assert_eq!(contents.count(water.id()), 0);
            }
        }
    }

    #[test]
    fn split_stacks_feed_paper_order_and_a_full_inventory_stays_idle() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:stone").unwrap()])
            .unwrap();
        let stone = Kind::Vanilla("minecraft:stone");
        let mut inputs = [None; 9];
        inputs[..3].fill(Some(stone));
        let recipe = CraftingRecipe {
            inputs,
            output: Stack {
                kind: stone,
                count: 2,
            },
            research: &RESEARCH,
        };
        let first = stone.stack(&host, 1);
        let next = stone.stack(&host, 9);
        assert!(payments(&host, &recipe, vec![first.clone(), next.clone()]).is_none());
        let raw = payments(&host, &recipe, vec![first, None, next]).unwrap();
        assert_eq!(
            raw.into_iter()
                .map(|payment| (payment.slot, payment.amount))
                .collect::<Vec<_>>(),
            vec![(0, 1), (2, 2)]
        );
    }
}
