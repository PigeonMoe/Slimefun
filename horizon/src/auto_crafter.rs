// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockEntityRef, Capability, Compound, Hand,
    Holder, Host, ITEMS, Identifier, Ingredient, ItemPlace, ItemResource, ItemStack, LevelRef,
    MenuClickedEvent, MenuClosedEvent, MenuReplace, ParticleKey, ParticleOptions, PlayerRef,
    Registrar, ResourceHandler, ServerHost, SoundSource, Tag, Text, Transaction,
};

use crate::{
    items::{ELECTRIC_MOTOR, HeadTexture, Kind, REDSTONE_ALLOY, item, item_id, text},
    network_recipes::{BLISTERING_INGOT_3, CARGO_MOTOR},
    network_runtime,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

#[path = "vanilla_crafter.rs"]
mod vanilla;

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
item!(
    VANILLA_AUTO_CRAFTER,
    "player_head",
    "Auto-Crafter (Vanilla)",
    "dark_green",
    None,
    Some(HeadTexture {
        profile: [1495638746, -1734004258, -1911383768, 150787901],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzgwYTQzMzRmNmE2MWU0MGMwYzYzZGViNjY1ZmE3YjU4MWU2ZWIyNTlmN2EzMjA3Y2VkN2ExZmY4YmRjOGE5ZjkifX19",
    })
);
item!(
    ARMOR_AUTO_CRAFTER,
    "player_head",
    "Auto-Crafter (Armor Forge)",
    "dark_green",
    None,
    Some(HeadTexture {
        profile: [-1063345249, -607178630, -1706725876, -302181572],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzVjYmQ5ZjVlYzFlZDAwNzI1OTk5NjQ5MWU2OWZmNjQ5YTMxMDZjZjkyMDIyN2IxYmIzYTcxZWU3YTg5ODYzZiJ9fX0=",
    })
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Enhanced,
    Vanilla,
    Armor,
}

impl Variant {
    fn from_item(id: &str) -> Option<Self> {
        if id == ENHANCED_AUTO_CRAFTER.id() {
            Some(Self::Enhanced)
        } else if id == VANILLA_AUTO_CRAFTER.id() {
            Some(Self::Vanilla)
        } else if id == ARMOR_AUTO_CRAFTER.id() {
            Some(Self::Armor)
        } else {
            None
        }
    }

    fn station(self) -> &'static str {
        match self {
            Self::Enhanced => "Enhanced Crafting Table",
            Self::Vanilla => "Crafting Table",
            Self::Armor => "Armor Forge",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Enhanced => "Auto-Crafter (Enhanced)",
            Self::Vanilla => "Auto-Crafter (Vanilla)",
            Self::Armor => "Auto-Crafter (Armor Forge)",
        }
    }

    fn cost(self) -> u32 {
        match self {
            Self::Enhanced | Self::Vanilla => 16,
            Self::Armor => 32,
        }
    }

    fn static_recipe(self, id: &str) -> Option<&'static CraftingRecipe> {
        match self {
            Self::Enhanced => recipe(id),
            Self::Vanilla => None,
            Self::Armor => crate::armor_forge::RECIPES
                .iter()
                .find(|recipe| crate::armor_forge::paper_id(recipe.output.kind) == id),
        }
    }
}

pub(crate) const CAPACITY: u32 = 256;
pub(crate) const RESEARCH: Research = Research {
    id: "auto_crafting",
    name: "Automatic Crafting",
    cost: 30,
};

const TABLE: Option<Kind> = Some(Kind::Vanilla("minecraft:crafting_table"));
const DISPENSER: Option<Kind> = Some(Kind::Vanilla("minecraft:dispenser"));
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
            DISPENSER,
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
    CraftingRecipe {
        inputs: [
            None,
            Some(CARGO_MOTOR),
            None,
            TABLE,
            Some(CRAFTING_MOTOR),
            TABLE,
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
        output: Stack {
            kind: VANILLA_AUTO_CRAFTER,
            count: 1,
        },
        research: &RESEARCH,
    },
    CraftingRecipe {
        inputs: [
            None,
            Some(CRAFTING_MOTOR),
            None,
            DISPENSER,
            Some(Kind::Vanilla("minecraft:anvil")),
            DISPENSER,
            TABLE,
            Some(ELECTRIC_MOTOR),
            TABLE,
        ],
        output: Stack {
            kind: ARMOR_AUTO_CRAFTER,
            count: 1,
        },
        research: &RESEARCH,
    },
];

const PREVIEW_SLOTS: [usize; 9] = [11, 12, 13, 20, 21, 22, 29, 30, 31];
const RESULT: usize = 24;
const CONTROL: usize = 49;
const PREVIOUS: usize = 46;
const NEXT: usize = 52;
const PREVIEW_TICKS: i64 = 14;
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
    if ![
        ENHANCED_AUTO_CRAFTER,
        VANILLA_AUTO_CRAFTER,
        ARMOR_AUTO_CRAFTER,
    ]
    .into_iter()
    .any(|item| item.matches(host, event.stack))
    {
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
    [
        CRAFTING_MOTOR,
        ENHANCED_AUTO_CRAFTER,
        VANILLA_AUTO_CRAFTER,
        ARMOR_AUTO_CRAFTER,
    ]
    .into_iter()
    .find(|kind| kind.id() == id)
}

pub(crate) fn is_machine(id: &str) -> bool {
    Variant::from_item(id).is_some()
}

pub(crate) fn operation_cost(id: &str) -> Option<u32> {
    Variant::from_item(id).map(Variant::cost)
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
    } else if let Some(variant) = Variant::from_item(id) {
        components.insert(
            "minecraft:lore".into(),
            Tag::List(vec![
                Tag::String("".into()),
                text("Place this machine on top of a", "white", false),
                text("chest or similar and make it craft", "white", false),
                text(
                    match variant {
                        Variant::Enhanced | Variant::Armor => {
                            "anything that can be crafted using an"
                        }
                        Variant::Vanilla => "anything that can be crafted using a",
                    },
                    "white",
                    false,
                ),
                text(variant.station(), "yellow", false),
                Tag::String("".into()),
                text("Advanced Machine", "red", true),
                text(&format!("⇨ ⚡ {} J/Item", variant.cost()), "gray", false),
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
                .filter(|id| enhanced_key(id) || vanilla_key(id))
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

fn enhanced_key(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn vanilla_key(id: &str) -> bool {
    id.len() <= 256 && id.contains(':') && Identifier::parse(id).is_ok()
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
    id: &str,
    row: &toml::Table,
) -> Result<(), String> {
    let variant = Variant::from_item(id).ok_or("Unknown Auto-Crafter variant.")?;
    if row.contains_key("paper_inventory") {
        crate::storage::validate_inventory_preset(row, id)?;
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
    let selected = paper_selection(&data)?;
    if selected
        .recipe
        .as_deref()
        .is_some_and(|recipe| !match variant {
            Variant::Enhanced | Variant::Armor => enhanced_key(recipe),
            Variant::Vanilla => vanilla_key(recipe),
        })
    {
        return Err("The Paper recipe key belongs to a different Auto-Crafter variant.".into());
    }
    store(host, pos, selected)?;
    Ok(())
}

fn recipe(selected: &str) -> Option<&'static CraftingRecipe> {
    let tickers = crate::ticker_policy::Tickers::current();
    crate::recipes::crafting()
        .find(|recipe| recipe.output.kind.id() == selected && tickers.allows_recipe(recipe))
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

#[derive(Clone, Copy)]
enum IngredientKind<'a> {
    Slimefun(Kind),
    Vanilla(&'a Ingredient),
}

impl IngredientKind<'_> {
    fn matches(self, host: &dyn Host, stack: &ItemStack) -> bool {
        match self {
            Self::Slimefun(kind) => kind.matches(host, stack),
            Self::Vanilla(ingredient) => {
                crate::items::usable_in_workbench(stack) && ingredient.test(host, stack)
            }
        }
    }
}

enum RecipeView<'a> {
    Slimefun(&'a CraftingRecipe),
    Vanilla(std::sync::Arc<vanilla::VanillaRecipe>),
}

fn selected_recipe(host: &dyn Host, variant: Variant, id: &str) -> Option<RecipeView<'static>> {
    match variant {
        Variant::Vanilla => vanilla::find(host, id).map(RecipeView::Vanilla),
        _ => variant.static_recipe(id).map(RecipeView::Slimefun),
    }
}

impl RecipeView<'_> {
    fn input(&self) -> [Option<IngredientKind<'_>>; 9] {
        match self {
            Self::Slimefun(recipe) => recipe.inputs.map(|kind| kind.map(IngredientKind::Slimefun)),
            Self::Vanilla(recipe) => recipe.input().map(|kind| kind.map(IngredientKind::Vanilla)),
        }
    }

    fn icons(&self, host: &dyn Host, frame: usize) -> ([Option<ItemStack>; 9], Option<ItemStack>) {
        match self {
            Self::Slimefun(recipe) => (
                recipe
                    .inputs
                    .map(|kind| kind.and_then(|kind| kind.stack(host, 1))),
                recipe.output.kind.stack(host, recipe.output.count),
            ),
            Self::Vanilla(recipe) => (recipe.preview(host, frame), Some(recipe.output())),
        }
    }
}

struct PaymentPlan {
    used: Vec<Payment>,
    grid: Vec<Option<ItemStack>>,
}

fn payments(
    host: &dyn Host,
    inputs: [Option<IngredientKind<'_>>; 9],
    slots: Vec<Option<ItemStack>>,
) -> Option<PaymentPlan> {
    // Paper requires a free slot before consuming any ingredient.
    if slots.iter().all(Option::is_some) {
        return None;
    }
    let mut available: Vec<_> = slots
        .iter()
        .map(|stack| stack.as_ref().map_or(0, ItemStack::count))
        .collect();
    let mut grid = vec![None; 9];
    for (cell, expected) in inputs
        .into_iter()
        .enumerate()
        .filter_map(|(index, expected)| Some((index, expected?)))
    {
        let slot = slots.iter().enumerate().find_map(|(slot, stack)| {
            (available[slot] > 0 && expected.matches(host, stack.as_ref()?)).then_some(slot)
        })?;
        available[slot] -= 1;
        grid[cell] = ItemResource::of(slots[slot].as_ref()?).to_stack(1);
    }
    let used = slots
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
        .collect();
    Some(PaymentPlan { used, grid })
}

fn leftover(host: &dyn Host, payment: &Payment) -> Result<Option<ItemStack>, ()> {
    let Some(container) = crate::recipes::crafting_container(&payment.stack.item().id(host)) else {
        return Ok(None);
    };
    container.stack(host, payment.amount).map(Some).ok_or(())
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
    id: &str,
) -> Result<(), ()> {
    let variant = Variant::from_item(id).ok_or(())?;
    let selected = read(host, pos)
        .map_err(|_| ())?
        .filter(|selected| !selected.disabled)
        .ok_or(())?;
    let recipe = selected
        .recipe
        .as_deref()
        .and_then(|id| selected_recipe(host, variant, id))
        .ok_or(())?;
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
    run(
        host,
        tx,
        level,
        &storage,
        recipe,
        |host, stack, from_chest| {
            [Some(below), partner].into_iter().flatten().all(|target| {
                let (from, to) = if from_chest {
                    (target, pos)
                } else {
                    (pos, target)
                };
                level.allow_inventory_transfer(host, from, to, stack)
            })
        },
    )
}

fn run(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    storage: &Inventory,
    recipe: RecipeView<'_>,
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
    let plan = payments(host, recipe.input(), slots).ok_or(())?;
    let mut output = match recipe {
        RecipeView::Slimefun(recipe) => recipe.output.kind.stack(host, recipe.output.count),
        RecipeView::Vanilla(recipe) => recipe.assemble(host, level, plan.grid),
    }
    .ok_or(())?;
    crate::backpacks::prepare_automated_craft(
        host,
        tx,
        plan.used.iter().map(|payment| &payment.stack),
        &mut output,
    )
    .map_err(|_| ())?;
    for payment in &plan.used {
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
    // Paper returns these specific containers for all Auto-Crafters, even if
    // a datapack's crafting remainder list declares something different.
    for payment in &plan.used {
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
struct Pages {
    recipes: Vec<String>,
    index: usize,
}

impl Pages {
    fn active(&self) -> &str {
        &self.recipes[self.index]
    }

    fn turn(&mut self, slot: usize) -> bool {
        let next = match slot {
            PREVIOUS => self.index.checked_sub(1),
            NEXT => self.index.checked_add(1),
            _ => None,
        };
        if let Some(index) = next.filter(|&index| index < self.recipes.len()) {
            self.index = index;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct Viewer {
    level: LevelRef,
    pos: Pos,
    body: BlockEntityRef,
    window: i32,
    variant: Variant,
    saved: Selection,
    candidate: Option<Pages>,
    preview_start: i64,
}

impl Viewer {
    fn key(&self) -> Option<&str> {
        chosen_key(&self.saved, self.candidate.as_ref())
    }
}

static VIEWERS: Mutex<BTreeMap<u128, Viewer>> = Mutex::new(BTreeMap::new());

pub(crate) fn clear() {
    VIEWERS.lock().unwrap().clear();
    invalidate_vanilla_recipes();
}

pub(crate) fn invalidate_vanilla_recipes() {
    vanilla::invalidate();
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

fn page_icon(
    host: &dyn Host,
    label: &str,
    page: usize,
    total: usize,
    available: bool,
) -> Option<ItemStack> {
    let (material, color) = if available {
        ("minecraft:lime_stained_glass_pane", "white")
    } else {
        ("minecraft:black_stained_glass_pane", "dark_gray")
    };
    let mut icon = named_icon(host, material, label, color)?;
    icon.apply_components(&Compound::from([(
        "minecraft:lore".into(),
        Tag::List(vec![
            text("", "gray", false),
            text(&format!("({page} / {total})"), "gray", false),
        ]),
    )]))
    .ok()?;
    Some(icon)
}

fn preview(
    host: &dyn Host,
    recipe: &RecipeView<'_>,
    variant: Variant,
    candidate: Option<&Pages>,
    disabled: bool,
    frame: usize,
) -> Vec<Option<ItemStack>> {
    let background = named_icon(host, "minecraft:gray_stained_glass_pane", " ", "gray");
    let mut items = vec![background; 54];
    let (ingredients, result) = recipe.icons(host, frame);
    for (slot, icon) in PREVIEW_SLOTS.into_iter().zip(ingredients) {
        items[slot] = icon;
    }
    items[RESULT] = result;
    if let Some(pages) = candidate.filter(|_| variant == Variant::Vanilla) {
        let current = pages.index + 1;
        let total = pages.recipes.len();
        items[PREVIOUS] = page_icon(host, "⇦ Previous page", current, total, current > 1);
        items[NEXT] = page_icon(host, "Next page ⇨", current, total, current < total);
    }
    let (control, label, color) = if candidate.is_some() {
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

fn chosen_key<'a>(saved: &'a Selection, candidate: Option<&'a Pages>) -> Option<&'a str> {
    candidate.map(Pages::active).or(saved.recipe.as_deref())
}

fn check_candidate(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    recipe: &RecipeView<'_>,
    key: &str,
) -> Result<(), &'static str> {
    match recipe {
        RecipeView::Slimefun(recipe)
            if !recipe.research.id.is_empty()
                && !research::can_use(host, player, recipe.research) =>
        {
            Err("Unlock this recipe in your Slimefun research before selecting it.")
        }
        RecipeView::Vanilla(_) => {
            if vanilla::limited(host, level)?
                && player.has_discovered_recipe(host, key) != Some(true)
            {
                Err("Unlock this recipe in your recipe book before selecting it.")
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn present(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    variant: Variant,
    saved: Selection,
    candidate: Option<Pages>,
) -> Result<(), &'static str> {
    let selected = chosen_key(&saved, candidate.as_ref())
        .ok_or("Sneak and right-click with the item to set an Auto-Crafter recipe.")?;
    let recipe = selected_recipe(host, variant, selected)
        .ok_or("The selected Auto-Crafter recipe is unavailable.")?;
    if candidate.is_some() {
        check_candidate(host, player, level, &recipe, selected)?;
    }
    let body = host
        .block_entity(level, pos)
        .ok_or("The Auto-Crafter is no longer loaded.")?;
    let items = preview(
        host,
        &recipe,
        variant,
        candidate.as_ref(),
        saved.disabled,
        0,
    );
    let preview_start = level.game_time(host).unwrap_or_default();
    let window = host
        .open_block_inventory_menu(
            player,
            body,
            6,
            Text::literal(variant.title()),
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
                variant,
                saved,
                candidate,
                preview_start,
            },
        );
        click_sound(host, player);
    }
    Ok(())
}

fn candidates(
    host: &dyn Host,
    player: PlayerRef,
    level: LevelRef,
    variant: Variant,
    item: &ItemStack,
) -> Result<Pages, &'static str> {
    let recipes = match variant {
        Variant::Vanilla => vanilla::candidates(host, level, player, item)?,
        _ => vec![
            item_id(item)
                .filter(|id| {
                    variant.static_recipe(id).is_some_and(|recipe| {
                        // Paper's vanilla chainmail has no Slimefun ID when crafted. Older tagged
                        // stacks can still select a recipe and receive the normal vanilla output.
                        match recipe.output.kind {
                            Kind::Vanilla(material) => item.item().id(host) == material,
                            kind => kind.matches(host, item),
                        }
                    })
                })
                .ok_or_else(|| match variant {
                    Variant::Armor => {
                        "Hold a Slimefun item made by the Armor Forge to set its recipe."
                    }
                    _ => "Hold an item made by the Enhanced Crafting Table to set its recipe.",
                })?,
        ],
    };
    if recipes.is_empty() {
        Err("No available Crafting Table recipe produces the item in your hand.")
    } else {
        Ok(Pages { recipes, index: 0 })
    }
}

pub(crate) fn use_at(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    pos: Pos,
    use_item: bool,
) -> bool {
    let Some(variant) = network_runtime::factory_identity(host, level, pos)
        .as_deref()
        .and_then(Variant::from_item)
    else {
        return false;
    };
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
            let item = use_item
                .then(|| place.stack(host))
                .flatten()
                .ok_or("Hold the desired crafting result to select its recipe.")?;
            Some(candidates(host, player, level, variant, &item)?)
        } else {
            None
        };
        present(host, player, level, pos, variant, selected, candidate)
    };
    if let Err(message) = activate(host) {
        tell(host, player, message);
    }
    true
}

pub(crate) fn clicked(host: &mut dyn ServerHost, event: &MenuClickedEvent) {
    let Ok(slot) = usize::try_from(event.slot) else {
        return;
    };
    if !matches!(slot, PREVIOUS | CONTROL | NEXT)
        || event.mode != 0
        || !matches!(event.button, 0 | 1)
    {
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
        || network_runtime::factory_identity(host, viewer.level, viewer.pos)
            .as_deref()
            .and_then(Variant::from_item)
            != Some(viewer.variant)
        || !research::can_use(host, player, &RESEARCH)
    {
        host.close_menu(event.uuid);
        return;
    }
    let result = apply_click(host, player, viewer, slot, event.button);
    if let Err(message) = result {
        host.close_menu(event.uuid);
        tell(host, player, message);
    }
}

fn apply_click(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    mut viewer: Viewer,
    slot: usize,
    button: i32,
) -> Result<(), &'static str> {
    let mut saved = read(host, viewer.pos)?.unwrap_or_default();
    if saved != viewer.saved || inventory(host, viewer.level, viewer.pos).is_none() {
        return Err("The Auto-Crafter settings or inventory changed. Reopen its menu.");
    }
    if slot != CONTROL {
        if let Some(pages) = &mut viewer.candidate
            && viewer.variant == Variant::Vanilla
            && pages.turn(slot)
        {
            send_preview(host, player.uuid(), &viewer, 0)?;
            viewer.preview_start = viewer.level.game_time(host).unwrap_or_default();
            VIEWERS.lock().unwrap().insert(player.uuid(), viewer);
            click_sound(host, player);
        }
        return Ok(());
    }
    if let Some(candidate) = viewer.candidate {
        let selected = candidate.active();
        let recipe = selected_recipe(host, viewer.variant, selected)
            .ok_or("The selected Auto-Crafter recipe is unavailable.")?;
        check_candidate(host, player, viewer.level, &recipe, selected)?;
        saved.recipe = Some(selected.to_owned());
        store(host, viewer.pos, saved.clone())?;
        tell(host, player, "The Auto-Crafter recipe was set.");
        return present(
            host,
            player,
            viewer.level,
            viewer.pos,
            viewer.variant,
            saved,
            None,
        );
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

fn send_preview(
    host: &mut dyn ServerHost,
    uuid: u128,
    viewer: &Viewer,
    frame: usize,
) -> Result<(), &'static str> {
    let recipe = viewer
        .key()
        .and_then(|id| selected_recipe(host, viewer.variant, id))
        .ok_or("The selected Auto-Crafter recipe is unavailable.")?;
    let items = preview(
        host,
        &recipe,
        viewer.variant,
        viewer.candidate.as_ref(),
        viewer.saved.disabled,
        frame,
    );
    host.menu_set_items(uuid, viewer.window, items)
        .map_err(|_| "The Auto-Crafter preview could not be updated.")
}

pub(crate) fn refresh(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(now) = level.game_time(host) else {
        return;
    };
    let active: Vec<_> = VIEWERS
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(&uuid, view)| {
            let elapsed = now.saturating_sub(view.preview_start);
            (view.level == level
                && view.variant == Variant::Vanilla
                && elapsed > 0
                && elapsed % PREVIEW_TICKS == 0)
                .then(|| (uuid, view.clone(), (elapsed / PREVIEW_TICKS) as usize))
        })
        .collect();
    for (uuid, view, frame) in active {
        if host.menu(uuid, view.window).is_some() {
            let animated = view
                .key()
                .and_then(|id| vanilla::find(host, id))
                .is_some_and(|recipe| recipe.animated());
            if animated {
                let _ = send_preview(host, uuid, &view, frame);
            }
        }
    }
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
                run(
                    host,
                    tx,
                    level,
                    &storage,
                    RecipeView::Slimefun(&recipe),
                    |_, _, from_chest| output_allowed || from_chest,
                )
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
        let view = RecipeView::Slimefun(&recipe);
        let inputs = view.input();
        assert!(payments(&host, inputs, vec![first.clone(), next.clone()]).is_none());
        let raw = payments(&host, inputs, vec![first, None, next]).unwrap();
        assert_eq!(
            raw.used
                .into_iter()
                .map(|payment| (payment.slot, payment.amount))
                .collect::<Vec<_>>(),
            vec![(0, 1), (2, 2)]
        );
    }

    #[test]
    fn armor_crafter_restores_the_paper_key_and_produces_untagged_chainmail() {
        let mut host = FakeHost::new();
        let chain = crate::armor_forge::CHAIN;
        let boots = Kind::Vanilla("minecraft:chainmail_boots");
        let boots_id = "CHAIN_BOOTS";
        let saved = paper_selection(&Compound::from([(
            "PublicBukkitValues".into(),
            Tag::compound([
                ("slimefun:recipe_key", Tag::String(boots_id.into())),
                ("slimefun:recipe_enabled", Tag::Byte(1)),
            ]),
        )]))
        .unwrap();
        assert!(saved.disabled);
        let id = ARMOR_AUTO_CRAFTER.id();
        assert_eq!(operation_cost(id), Some(32));
        assert_eq!(operation_cost(VANILLA_AUTO_CRAFTER.id()), Some(16));
        assert!(Variant::Enhanced.static_recipe(boots_id).is_none());
        assert_eq!(
            recipe(id).unwrap().inputs[4],
            Some(Kind::Vanilla("minecraft:anvil"))
        );
        let armor = Variant::from_item(id).unwrap();
        let recipe = selected_recipe(&host, armor, saved.recipe.as_deref().unwrap()).unwrap();

        host.registries
            .resolve_items([chain, boots].map(|kind| Identifier::parse(kind.material()).unwrap()))
            .unwrap();
        let raw = chain.stack(&host, 4).unwrap();
        let chest = [0, 65, 0];
        let level = host.level.level();
        let container = host
            .level
            .block_entity(chest)
            .with_items(2)
            .items
            .as_mut()
            .unwrap();
        container.slots[0] = Some(raw);
        host.transaction(|host, tx| {
            let storage = level.capability(host, &ITEMS, chest, None).unwrap();
            run(host, tx, level, &storage, recipe, |_, _, _| true)
        })
        .unwrap();
        let contents = host
            .level
            .block_entity_at(chest)
            .unwrap()
            .items
            .as_ref()
            .unwrap();
        assert_eq!(contents.slots[0].as_ref(), boots.stack(&host, 1).as_ref());
        assert!(contents.slots[1].is_none());
    }

    #[test]
    fn a_vanilla_saved_key_and_two_pages_remain_exact() {
        let mut host = FakeHost::new();
        let black = Kind::Vanilla("minecraft:black_stained_glass_pane");
        let lime = Kind::Vanilla("minecraft:lime_stained_glass_pane");
        host.registries
            .resolve_items([black, lime].map(|kind| Identifier::parse(kind.id()).unwrap()))
            .unwrap();
        let previous = page_icon(&host, "⇦ Previous page", 1, 2, false).unwrap();
        let next = page_icon(&host, "Next page ⇨", 1, 2, true).unwrap();
        assert_eq!(previous.item(), black.stack(&host, 1).unwrap().item());
        assert_eq!(next.item(), lime.stack(&host, 1).unwrap().item());
        assert_eq!(
            next.components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:lore".into()),
            Some(&Tag::List(vec![
                text("", "gray", false),
                text("(1 / 2)", "gray", false),
            ])),
        );
        let saved = paper_selection(&Compound::from([(
            "PublicBukkitValues".into(),
            Tag::compound([
                (
                    "slimefun:recipe_key",
                    Tag::String("demo:processing/oak_sticks".into()),
                ),
                ("slimefun:recipe_enabled", Tag::Byte(0)),
            ]),
        )]))
        .unwrap();
        assert_eq!(saved.recipe.as_deref(), Some("demo:processing/oak_sticks"));
        assert!(saved.disabled);
        assert!(!vanilla_key("demo:INVALID"));
        let mut pages = Pages {
            recipes: vec![
                "minecraft:stick".into(),
                "demo:processing/oak_sticks".into(),
            ],
            index: 0,
        };
        assert!(!pages.turn(PREVIOUS));
        assert!(pages.turn(NEXT));
        assert_eq!(pages.active(), "demo:processing/oak_sticks");
        assert!(!pages.turn(NEXT));
        assert!(pages.turn(PREVIOUS));
        assert!(!pages.turn(CONTROL));
        assert_eq!(pages.active(), "minecraft:stick");
        let recipe = recipe("VANILLA_AUTO_CRAFTER").unwrap();
        assert_eq!(
            recipe.inputs.map(|part| part.map(Kind::id)),
            [
                None,
                Some("CARGO_MOTOR"),
                None,
                Some("minecraft:crafting_table"),
                Some("CRAFTING_MOTOR"),
                Some("minecraft:crafting_table"),
                None,
                Some("ELECTRIC_MOTOR"),
                None
            ]
        );
    }
}
