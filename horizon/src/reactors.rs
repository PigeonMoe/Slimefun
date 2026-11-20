// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    ActiveEffect, BlockUpdateFlags, Compound, ExplosionInteraction, ExplosionSettings, Host,
    ItemResource, ItemStack, LevelRef, ServerHost, Tag, Transaction,
};

use super::{
    ElectricMachine, Position, RecipeFamily, Storage, VIEWERS, data_at, display_item, max_stack,
    paper_progress_item, place, save, stack_in,
};
use crate::{
    elemental_staves::LAVA_CRYSTAL,
    heated_pressure::{BOOSTED_URANIUM, NETHER_ICE_COOLANT_CELL, PLUTONIUM, URANIUM},
    items::{
        CORINTHIAN_BRONZE_INGOT, ELECTRIC_MOTOR, HeadTexture, Kind, LEAD_INGOT, REINFORCED_PLATE,
        item,
    },
    network_recipes::{BLISTERING_INGOT_3, CARBONADO_EDGED_CAPACITOR, CARGO_MOTOR},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    thermal_machines::{COOLING_UNIT, REACTOR_COLLANT_CELL},
};

item!(
    NUCLEAR_REACTOR,
    "player_head",
    "Nuclear Reactor",
    "dark_green",
    None,
    Some(HeadTexture {
        profile: [1557198628, 2100508903, -1727649628, 754311549],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2ZhNWRlMGJjMmJmYjVjYzJkMjNlYjcyZjk2NDAyYWRhNDc5NTI0ZGQwZGU0MDRiYzIzYjZkYWNlZTNmZmQwODAifX19",
    })
);
item!(
    NETHERSTAR_REACTOR,
    "player_head",
    "Nether Star Reactor",
    "white",
    None,
    Some(HeadTexture {
        profile: [491915222, 1953773825, -1187264006, 1582719723],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlL2ExMWVkMWQxYjI1YjYyNDY2NWVjZGRkYzNkM2E1ZGZmMGI5ZjM1ZTNkZTc3YTEyZjUxNmU2MGZlODUwMWNjOGQifX19",
    })
);
item!(
    NEPTUNIUM,
    "player_head",
    "Neptunium",
    "green",
    None,
    Some(HeadTexture {
        profile: [1110175385, -180669349, -2085143706, 2138597310],
        value: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHBzOi8vdGV4dHVyZXMubWluZWNyYWZ0Lm5ldC90ZXh0dXJlLzRlZGVhNmJmZDM3ZTQ5ZGU0M2YxNTRmZTZmY2E2MTdkNDEyOWU2MWI5NTc1OWEzZDQ5YTE1OTM1YTFjMmRjZjAifX19",
    })
);
item!(
    REACTOR_ACCESS_PORT,
    "cyan_terracotta",
    "Reactor Access Port",
    "dark_green",
    None,
    None
);

pub(crate) const NUCLEAR_RESEARCH: Research = Research {
    id: "nuclear_reactor",
    name: "Nuclear Power Plant",
    cost: 48,
};
pub(crate) const NETHER_RESEARCH: Research = Research {
    id: "nether_star_reactor",
    name: "Nether Star Reactor",
    cost: 60,
};
pub(crate) const ACCESS_RESEARCH: Research = Research {
    id: "reactor_access_port",
    name: "Reactor Interaction",
    cost: 18,
};
pub(crate) const RESEARCHES: &[Research] = &[NUCLEAR_RESEARCH, NETHER_RESEARCH, ACCESS_RESEARCH];

#[derive(Clone, Copy)]
pub(crate) struct Fuel {
    pub(crate) kind: Kind,
    pub(crate) ticks: u32,
    pub(crate) byproduct: Option<Kind>,
}

impl Fuel {
    const fn new(kind: Kind, seconds: u32, byproduct: Option<Kind>) -> Self {
        Self {
            kind,
            // Paper's MachineFuel converts nominal seconds to two Slimefun ticks each.
            ticks: seconds * 2,
            byproduct,
        }
    }
}

pub(crate) struct Reactor {
    pub(crate) item: Kind,
    progress_icon: Kind,
    pub(crate) title: &'static str,
    pub(crate) research: &'static Research,
    pub(crate) capacity: u32,
    pub(crate) generation: u32,
    pub(crate) coolant: Kind,
    pub(crate) fuels: &'static [Fuel],
    pub(crate) withers: bool,
}

const STAR: Kind = Kind::Vanilla("minecraft:nether_star");
static NUCLEAR: Reactor = Reactor {
    item: NUCLEAR_REACTOR,
    progress_icon: LAVA_CRYSTAL,
    title: "Nuclear Reactor",
    research: &NUCLEAR_RESEARCH,
    capacity: 16_384,
    generation: 250,
    coolant: REACTOR_COLLANT_CELL,
    fuels: &[
        Fuel::new(URANIUM, 1_200, Some(NEPTUNIUM)),
        Fuel::new(NEPTUNIUM, 600, Some(PLUTONIUM)),
        Fuel::new(BOOSTED_URANIUM, 1_500, None),
    ],
    withers: false,
};
static NETHER: Reactor = Reactor {
    item: NETHERSTAR_REACTOR,
    progress_icon: STAR,
    title: "Nether Star Reactor",
    research: &NETHER_RESEARCH,
    capacity: 32_768,
    generation: 512,
    coolant: NETHER_ICE_COOLANT_CELL,
    fuels: &[Fuel::new(STAR, 1_800, None)],
    withers: true,
};

pub(crate) fn reactor(id: &str) -> Option<&'static Reactor> {
    [&NUCLEAR, &NETHER]
        .into_iter()
        .find(|reactor| reactor.item.id() == id)
}

pub(crate) fn coolant(host: &dyn Host, stack: &ItemStack) -> bool {
    [REACTOR_COLLANT_CELL, NETHER_ICE_COOLANT_CELL]
        .iter()
        .any(|item| item.matches(host, stack))
}

const fn craft(
    kind: Kind,
    research: &'static Research,
    inputs: [Option<Kind>; 9],
) -> CraftingRecipe {
    CraftingRecipe {
        inputs,
        output: Stack { kind, count: 1 },
        research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    craft(
        NUCLEAR_REACTOR,
        &NUCLEAR_RESEARCH,
        [
            Some(BLISTERING_INGOT_3),
            Some(CARBONADO_EDGED_CAPACITOR),
            Some(BLISTERING_INGOT_3),
            Some(REINFORCED_PLATE),
            Some(COOLING_UNIT),
            Some(REINFORCED_PLATE),
            Some(LEAD_INGOT),
            Some(REINFORCED_PLATE),
            Some(LEAD_INGOT),
        ],
    ),
    craft(
        NETHERSTAR_REACTOR,
        &NETHER_RESEARCH,
        [
            Some(BOOSTED_URANIUM),
            Some(CARBONADO_EDGED_CAPACITOR),
            Some(BOOSTED_URANIUM),
            Some(REINFORCED_PLATE),
            Some(STAR),
            Some(REINFORCED_PLATE),
            Some(CORINTHIAN_BRONZE_INGOT),
            Some(REINFORCED_PLATE),
            Some(CORINTHIAN_BRONZE_INGOT),
        ],
    ),
    craft(
        REACTOR_ACCESS_PORT,
        &ACCESS_RESEARCH,
        [
            None,
            Some(BLISTERING_INGOT_3),
            None,
            Some(LEAD_INGOT),
            Some(CARGO_MOTOR),
            Some(LEAD_INGOT),
            None,
            Some(ELECTRIC_MOTOR),
            None,
        ],
    ),
];

pub(super) const SLOTS: &[usize] = &[19, 28, 37, 25, 34, 43, 40];
const FUEL_SLOTS: [usize; 3] = [0, 1, 2];
const COOLANT_SLOTS: [usize; 3] = [3, 4, 5];
const OUTPUT: usize = 6;
const OPERATION: &str = "reactor_operation";
const COOLANT_PHASE: &str = "coolant_progress";
const FUEL_INDEX: &str = "fuel_index";
const MELTDOWN: &str = "missing_water";
const COOLANT_DURATION: u32 = 50;
const SURROUNDING: [[i32; 2]; 8] = [
    [0, -1],
    [1, -1],
    [1, 0],
    [1, 1],
    [0, 1],
    [-1, 1],
    [-1, 0],
    [-1, -1],
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Operation {
    fuel_index: usize,
    progress: u32,
    missing_water: bool,
}

impl Operation {
    fn load(data: &Compound, spec: &Reactor) -> Result<Option<Self>, ()> {
        let Some(value) = data.get(&OPERATION.into()) else {
            return Ok(None);
        };
        let Tag::Compound(value) = value else {
            return Err(());
        };
        let read = |key: &str| match value.get(&key.into()) {
            Some(Tag::Int(value)) => u32::try_from(*value).map_err(|_| ()),
            _ => Err(()),
        };
        let fuel_index = read(FUEL_INDEX)? as usize;
        let fuel = spec.fuels.get(fuel_index).ok_or(())?;
        let progress = read(COOLANT_PHASE)?;
        if progress > fuel.ticks {
            return Err(());
        }
        Ok(Some(Self {
            fuel_index,
            progress,
            missing_water: matches!(value.get(&MELTDOWN.into()), Some(Tag::Byte(1))),
        }))
    }

    fn store(self, data: &mut Compound) {
        data.insert(
            OPERATION.into(),
            Tag::compound([
                (FUEL_INDEX, Tag::Int(self.fuel_index as i32)),
                (COOLANT_PHASE, Tag::Int(self.progress as i32)),
                (MELTDOWN, Tag::Byte(i8::from(self.missing_water))),
            ]),
        );
    }
}

pub(super) fn progress(data: &Compound, spec: &Reactor) -> Option<(u32, u32)> {
    let operation = Operation::load(data, spec).ok()??;
    Some((operation.progress, spec.fuels[operation.fuel_index].ticks))
}

pub(crate) fn cooling_percent(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    spec: &Reactor,
) -> Option<u32> {
    let data = data_at(
        host,
        Storage {
            level,
            pos,
            plain: false,
        },
    )?;
    let (done, _) = progress(&data, spec)?;
    (done > 0).then_some(100 - (done % COOLANT_DURATION) * 2)
}

pub(super) fn menu_items(
    host: &dyn Host,
    data: &Compound,
    machine: ElectricMachine,
) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; 54];
    for (slots, name) in [
        (
            &[0, 1, 2, 3, 5, 6, 7, 8, 12, 13, 14, 21, 23][..],
            "minecraft:gray_stained_glass_pane",
        ),
        (
            &[9, 10, 11, 18, 20, 27, 29, 36, 38, 45, 46, 47],
            "minecraft:lime_stained_glass_pane",
        ),
        (
            &[15, 16, 17, 24, 26, 33, 35, 42, 44, 51, 52, 53],
            "minecraft:cyan_stained_glass_pane",
        ),
        (
            &[30, 31, 32, 39, 41, 48, 50],
            "minecraft:green_stained_glass_pane",
        ),
    ] {
        let pane = display_item(host, name, " ");
        for &slot in slots {
            items[slot] = pane.clone();
        }
    }
    for (index, &slot) in SLOTS.iter().enumerate() {
        items[slot] = stack_in(host, data, index);
    }
    let (fuel, coolant) = match machine.family {
        RecipeFamily::Reactor(spec) => (spec.fuels[0].kind, spec.coolant),
        _ => (URANIUM, REACTOR_COLLANT_CELL),
    };
    items[1] = named_item(host, fuel, "Fuel Slot");
    items[7] = named_item(host, coolant, "Coolant Slot");
    items[22] = match machine.family {
        RecipeFamily::Reactor(spec) => progress(data, spec)
            .and_then(|(done, total)| {
                paper_progress_item(host, spec.progress_icon, total.saturating_sub(done), total)
            })
            .or_else(|| display_item(host, "minecraft:black_stained_glass_pane", " ")),
        _ => named_item(host, PLUTONIUM, "Byproduct Slot"),
    };
    items
}

fn named_item(host: &dyn Host, kind: Kind, title: &str) -> Option<ItemStack> {
    let mut stack = kind.stack(host, 1)?;
    stack
        .apply_components(&Compound::from([(
            "minecraft:custom_name".into(),
            Tag::String(title.into()),
        )]))
        .ok()?;
    Some(stack)
}

pub(super) fn link(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Position,
    machine: ElectricMachine,
) -> Option<Position> {
    let is_reactor = matches!(machine.family, RecipeFamily::Reactor(_));
    let paired = [pos[0], pos[1] + if is_reactor { 3 } else { -3 }, pos[2]];
    let id = crate::network_runtime::factory_identity(host, level, paired)?;
    let linked = if is_reactor {
        id == REACTOR_ACCESS_PORT.id()
    } else {
        reactor(&id).is_some()
    };
    linked.then_some(paired)
}

/// Paper adds these buttons to each instance after the preset is built, so
/// BlockMenu.save includes them among the inventory slots. Their state is derived
/// from the block metadata and adjacent reactor, and we recreate both buttons.
pub(super) fn paper_control(
    host: &dyn Host,
    machine: ElectricMachine,
    slot: u32,
    stack: &ItemStack,
) -> bool {
    if stack.count() != 1 {
        return false;
    }
    let reactor = matches!(machine.family, RecipeFamily::Reactor(_));
    if slot == 4 && reactor {
        return (NUCLEAR_REACTOR.matches(host, stack)
            && paper_button(
                stack,
                "Focus: Electricity",
                &[
                    "",
                    "Your Reactor will focus on Power Generation",
                    "If your Energy Network doesn't need Power",
                    "it will not produce any either",
                    "",
                    "⇨ Click to change the Focus to Production",
                ],
            ))
            || (PLUTONIUM.matches(host, stack)
                && paper_button(
                    stack,
                    "Focus: Production",
                    &[
                        "",
                        "Your Reactor will focus on producing goods",
                        "If your Energy Network doesn't need Power",
                        "it will continue to run and simply will",
                        "not generate any Power in the mean time",
                        "",
                        "⇨ Click to change the Focus to Power Generation",
                    ],
                ));
    }
    if slot != 49
        || !stack
            .components()
            .is_some_and(|parts| parts.as_nbt().len() == 2)
    {
        return false;
    }
    let title = if reactor { "Access Port" } else { "Reactor" };
    match (stack.item().id(host).as_str(), reactor) {
        ("minecraft:green_wool", true) => paper_button(
            stack,
            title,
            &["", "Detected", "", "> Click to view Access Port"],
        ),
        ("minecraft:green_wool", false) => paper_button(
            stack,
            title,
            &["", "Detected", "", "> Click to view Reactor"],
        ),
        ("minecraft:red_wool", true) => paper_button(
            stack,
            title,
            &[
                "",
                "Not detected",
                "",
                "Access Port must be",
                "placed 3 blocks above",
                "a reactor!",
            ],
        ),
        ("minecraft:red_wool", false) => paper_button(
            stack,
            title,
            &[
                "",
                "Not detected",
                "",
                "Reactor must be",
                "placed 3 blocks below",
                "the access port!",
            ],
        ),
        _ => false,
    }
}

fn paper_button(stack: &ItemStack, title: &str, expected_lore: &[&str]) -> bool {
    let Some(parts) = stack.components().map(|components| components.as_nbt()) else {
        return false;
    };
    let Some(Tag::List(lore)) = parts.get(&"minecraft:lore".into()) else {
        return false;
    };
    let same_text = |tag: &Tag, expected: &str| {
        crate::network_runtime::paper_text(tag)
            .is_some_and(|text| crate::guide::strip_colors(&text) == expected)
    };
    parts
        .get(&"minecraft:custom_name".into())
        .is_some_and(|name| same_text(name, title))
        && lore.len() == expected_lore.len()
        && lore
            .iter()
            .zip(expected_lore)
            .all(|(line, expected)| same_text(line, expected))
}

pub(super) fn decorate_controls(
    host: &dyn Host,
    items: &mut [Option<ItemStack>],
    machine: ElectricMachine,
    production: bool,
    linked: bool,
) {
    let reactor = matches!(machine.family, RecipeFamily::Reactor(_));
    if reactor {
        let (icon, label) = if production {
            (PLUTONIUM, "Focus: Production")
        } else {
            (NUCLEAR_REACTOR, "Focus: Electricity")
        };
        items[4] = named_item(host, icon, label);
    }
    let title = if reactor { "Access Port" } else { "Reactor" };
    let (palette, status) = if linked {
        ("minecraft:green_wool", "Detected")
    } else {
        ("minecraft:red_wool", "Not detected")
    };
    items[49] = display_item(host, palette, &format!("{title}: {status}"));
}

#[derive(Default)]
pub(crate) struct Poll {
    pub(crate) energy: u32,
    pub(crate) wither: bool,
    pub(crate) melt: bool,
}

pub(crate) fn apply_effects(host: &mut dyn ServerHost, level: LevelRef, pos: Position, poll: Poll) {
    if poll.wither
        && let Some(effect) = host.effect_key("minecraft:wither")
    {
        let wither = ActiveEffect::new(effect, 60, 1, false, true, true);
        let min = pos.map(|coordinate| f64::from(coordinate) - 5.0);
        let max = pos.map(|coordinate| f64::from(coordinate) + 5.0);
        for entity in level.entities_in(host, min, max).unwrap_or_default() {
            if let Some(living) = host.living(entity) {
                living.add_effect(host, &wither);
            }
        }
    }
    if poll.melt
        && let Some(lava) = host.block_state_id("minecraft:lava")
        && matches!(
            level.set_block(
                host,
                pos,
                lava,
                BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS
            ),
            Ok(true)
        )
    {
        VIEWERS.close_at(host, level, pos);
        let _ = level.explode(
            host,
            None,
            pos.map(f64::from),
            0.0,
            false,
            ExplosionInteraction::Block,
            ExplosionSettings::default(),
        );
    }
}

pub(crate) fn poll(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: Position,
    spec: &Reactor,
    charge: u32,
    production_mode: bool,
    rolls: &Rolls,
) -> Result<Poll, ()> {
    let storage = Storage {
        level,
        pos,
        plain: false,
    };
    let initial = data_at(host, storage).ok_or(())?;
    let port = [pos[0], pos[1] + 3, pos[2]];
    let port = crate::network_runtime::factory_identity(host, level, port)
        .is_some_and(|id| id == REACTOR_ACCESS_PORT.id())
        .then_some(port);
    let Some(mut operation) = Operation::load(&initial, spec)? else {
        // Paper finds the next fuel before restocking, so port-only fuel
        // starts its operation on the following network tick.
        let fuel = spec.fuels.iter().enumerate().find_map(|(index, fuel)| {
            (0..OUTPUT).find_map(|slot| {
                let stack = stack_in(host, &initial, slot)?;
                fuel.kind
                    .matches(host, &stack)
                    .then_some((index, slot, stack))
            })
        });
        if let Some(port) = port {
            let from = Storage {
                level,
                pos: port,
                plain: true,
            };
            restock_fuel(host, tx, from, storage, spec)?;
        }
        if let Some((fuel_index, slot, stack)) = fuel {
            if place(level, pos, slot).extract(host, tx, &ItemResource::of(&stack), 1) != 1 {
                return Err(());
            }
            let mut next = data_at(host, storage).ok_or(())?;
            Operation {
                fuel_index,
                progress: 0,
                missing_water: false,
            }
            .store(&mut next);
            save(host, storage, next).then_some(()).ok_or(())?;
        }
        return Ok(Poll::default());
    };

    let mut outcome = Poll {
        wither: spec.withers,
        ..Poll::default()
    };
    if operation.missing_water {
        outcome.melt = true;
        return Ok(outcome);
    }
    let fuel = spec.fuels[operation.fuel_index];
    if operation.progress == fuel.ticks {
        if let Some(product) = fuel.byproduct {
            let output = product.stack(host, 1).ok_or(())?;
            let _ = place(level, pos, OUTPUT).insert(host, tx, &ItemResource::of(&output), 1);
        }
        if let Some(port) = port {
            let from = place(level, pos, OUTPUT);
            let to = place(level, port, OUTPUT);
            if let Some(stack) = from.stack(host) {
                move_stack(host, tx, level, pos, port, from, [to], &stack)?;
            }
        }
        let mut next = data_at(host, storage).ok_or(())?;
        next.remove(&OPERATION.into());
        save(host, storage, next).then_some(()).ok_or(())?;
        return Ok(outcome);
    }

    let can_generate = spec.capacity.saturating_sub(charge) >= spec.generation;
    if !can_generate && !production_mode {
        return Ok(outcome);
    }
    let [x, z] = SURROUNDING[rolls.next(host, level, 8).ok_or(())? as usize];
    let water = [pos[0] + x, pos[1], pos[2] + z];
    if !level.is_loaded(host, water) {
        return Ok(outcome);
    }
    operation.progress += 1;
    operation.missing_water = !level
        .block_state(host, water)
        .is_some_and(|state| state.block().id(host) == "minecraft:water");
    if operation.progress % COOLANT_DURATION == 0 {
        if let Some(port) = port {
            let from = Storage {
                level,
                pos: port,
                plain: true,
            };
            restock_coolant(host, tx, from, storage, spec.coolant)?;
        }
        let data = data_at(host, storage).ok_or(())?;
        let cooled = COOLANT_SLOTS.into_iter().any(|slot| {
            let Some(stack) =
                stack_in(host, &data, slot).filter(|stack| spec.coolant.matches(host, stack))
            else {
                return false;
            };
            place(level, pos, slot).extract(host, tx, &ItemResource::of(&stack), 1) == 1
        });
        if !cooled {
            outcome.melt = true;
            return Ok(outcome);
        }
    }
    let mut next = data_at(host, storage).ok_or(())?;
    operation.store(&mut next);
    save(host, storage, next).then_some(()).ok_or(())?;
    if can_generate {
        outcome.energy = spec.generation;
    }
    Ok(outcome)
}

fn restock_fuel(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    port: Storage,
    reactor: Storage,
    spec: &Reactor,
) -> Result<(), ()> {
    let incoming = data_at(host, port).ok_or(())?;
    let stored = data_at(host, reactor).ok_or(())?;
    if let Some((slot, stack)) = restock_candidate(host, spec, &incoming, &stored) {
        let from = place(port.level, port.pos, slot);
        let slots = FUEL_SLOTS.map(|slot| place(reactor.level, reactor.pos, slot));
        move_stack(
            host,
            tx,
            reactor.level,
            port.pos,
            reactor.pos,
            from,
            slots,
            &stack,
        )?;
    }
    Ok(())
}

fn restock_candidate(
    host: &dyn Host,
    spec: &Reactor,
    incoming: &Compound,
    stored: &Compound,
) -> Option<(usize, ItemStack)> {
    FUEL_SLOTS.into_iter().find_map(|slot| {
        let stack = stack_in(host, incoming, slot)?;
        if !spec
            .fuels
            .iter()
            .any(|fuel| fuel.kind.matches(host, &stack))
        {
            return None;
        }
        // Paper fits one item before pushing the stack. A non-fitting first fuel
        // must not block a later compatible stack in the access port.
        let resource = ItemResource::of(&stack);
        let limit = max_stack(host, &resource);
        let fits = limit > 0
            && FUEL_SLOTS.into_iter().any(|target| {
                stack_in(host, stored, target)
                    .is_none_or(|item| resource.matches(&item) && item.count() < limit)
            });
        fits.then_some((slot, stack))
    })
}

fn restock_coolant(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    port: Storage,
    reactor: Storage,
    coolant: Kind,
) -> Result<(), ()> {
    let data = data_at(host, port).ok_or(())?;
    for slot in COOLANT_SLOTS {
        if let Some(stack) =
            stack_in(host, &data, slot).filter(|stack| coolant.matches(host, stack))
        {
            let from = place(port.level, port.pos, slot);
            let slots = COOLANT_SLOTS.map(|slot| place(reactor.level, reactor.pos, slot));
            move_stack(
                host,
                tx,
                reactor.level,
                port.pos,
                reactor.pos,
                from,
                slots,
                &stack,
            )?;
        }
    }
    Ok(())
}

fn move_stack(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    from_pos: Position,
    to_pos: Position,
    from: mod_api::ItemPlace,
    to: impl IntoIterator<Item = mod_api::ItemPlace>,
    stack: &mod_api::ItemStack,
) -> Result<(), ()> {
    if !level.allow_inventory_transfer(host, from_pos, to_pos, stack) {
        return Ok(());
    }
    let item = ItemResource::of(stack);
    let mut left = stack.count();
    for target in to {
        left -= target.insert(host, tx, &item, left);
        if left == 0 {
            break;
        }
    }
    let moved = stack.count() - left;
    (moved == 0 || from.extract(host, tx, &item, moved) == moved)
        .then_some(())
        .ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn reactor_indicators_use_the_paper_objects_without_losing_the_skull_profile() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:player_head", "minecraft:nether_star"]
                    .map(|item| Identifier::parse(item).unwrap()),
            )
            .unwrap();
        for (reactor, time) in [(&NUCLEAR, "15m 0s"), (&NETHER, "22m 30s")] {
            let mut data = Compound::new();
            Operation {
                fuel_index: 0,
                progress: reactor.fuels[0].ticks / 4,
                missing_water: false,
            }
            .store(&mut data);
            let machine = ElectricMachine::for_item(reactor.item.id()).unwrap();
            let displayed = menu_items(&host, &data, machine);
            let indicator = displayed[22].as_ref().unwrap();
            let original = reactor.progress_icon.stack(&host, 1).unwrap();
            assert_eq!(indicator.item(), original.item());
            assert_eq!(indicator.damage_component(), Some(0));
            let components = indicator.components().unwrap().as_nbt();
            let Some(Tag::List(lines)) = components.get(&"minecraft:lore".into()) else {
                panic!("a running Paper reactor displays a progress bar");
            };
            assert_eq!(lines[2], crate::items::text(time, "gray", false));
            if reactor.item == NUCLEAR_REACTOR {
                assert!(LAVA_CRYSTAL.matches(&host, indicator));
                assert_eq!(
                    components.get(&"minecraft:profile".into()),
                    original
                        .components()
                        .unwrap()
                        .as_nbt()
                        .get(&"minecraft:profile".into())
                );
            }
        }
    }

    #[test]
    fn paper_reactors_keep_fuel_and_progress_through_a_snapshot() {
        let nuclear = reactor("NUCLEAR_REACTOR").unwrap();
        assert_eq!((nuclear.capacity, nuclear.generation), (16_384, 250));
        assert_eq!(
            nuclear
                .fuels
                .iter()
                .map(|fuel| (fuel.kind.id(), fuel.ticks, fuel.byproduct.map(Kind::id)))
                .collect::<Vec<_>>(),
            vec![
                ("URANIUM", 2_400, Some("NEPTUNIUM")),
                ("NEPTUNIUM", 1_200, Some("PLUTONIUM")),
                ("BOOSTED_URANIUM", 3_000, None),
            ]
        );
        let nether = reactor("NETHERSTAR_REACTOR").unwrap();
        assert_eq!(
            (nether.capacity, nether.generation, nether.fuels[0].ticks),
            (32_768, 512, 3_600)
        );

        let mut data = Compound::new();
        let saved = Operation {
            fuel_index: 1,
            progress: 600,
            missing_water: false,
        };
        saved.store(&mut data);
        assert_eq!(progress(&data, nuclear), Some((600, 1_200)));
        assert_eq!(Operation::load(&data, nuclear), Ok(Some(saved)));
        assert!(Operation::load(&data, nether).is_err());
        Operation {
            progress: 1_201,
            ..saved
        }
        .store(&mut data);
        assert!(Operation::load(&data, nuclear).is_err());
    }

    #[test]
    fn a_saved_reactor_finishes_on_the_paper_duration_not_the_previous_cutoff() {
        use mod_api::HostTransaction;

        let mut host = FakeHost::new();
        let level = host.level.level();
        let pos = [0, 64, 0];
        host.level.block_entity(pos);
        let storage = Storage {
            level,
            pos,
            plain: false,
        };
        let rolls = Rolls::default();

        for (elapsed, still_burning) in [(1_500, true), (3_000, false)] {
            let operation = Operation {
                fuel_index: 2,
                progress: elapsed,
                missing_water: false,
            };
            let mut data = Compound::new();
            operation.store(&mut data);
            assert!(save(&mut host, storage, data));
            let result = host
                .transaction(|host, tx| {
                    poll(
                        host,
                        tx,
                        level,
                        pos,
                        &NUCLEAR,
                        NUCLEAR.capacity,
                        false,
                        &rolls,
                    )
                })
                .unwrap();
            assert_eq!(result.energy, 0);
            assert!(!result.melt);
            let saved = data_at(&mut host, storage).unwrap();
            assert_eq!(
                Operation::load(&saved, &NUCLEAR),
                Ok(still_burning.then_some(operation))
            );
        }
    }

    #[test]
    fn a_blocked_access_port_fuel_does_not_hide_the_next_fuel_that_fits() {
        use super::super::write_stack;

        let mut host = FakeHost::new();
        let carrier = "minecraft:player_head";
        host.registries
            .resolve_items([Identifier::parse(carrier).unwrap()])
            .unwrap();
        host.registries
            .item_component(carrier, "minecraft:max_stack_size", Tag::Int(64));
        let uranium = URANIUM.stack(&host, 16).unwrap();
        let neptunium = NEPTUNIUM.stack(&host, 3).unwrap();
        let mut incoming = Compound::new();
        assert!(write_stack(&host, &mut incoming, 0, Some(uranium.clone())));
        assert!(write_stack(
            &host,
            &mut incoming,
            1,
            Some(neptunium.clone())
        ));
        let mut stored = Compound::new();
        for (slot, count) in [63, 64, 64].into_iter().enumerate() {
            let stack = NEPTUNIUM.stack(&host, count).unwrap();
            assert!(write_stack(&host, &mut stored, slot, Some(stack)));
        }
        assert_eq!(
            restock_candidate(&host, &NUCLEAR, &incoming, &stored),
            Some((1, neptunium))
        );
        let full = NEPTUNIUM.stack(&host, 64).unwrap();
        assert!(write_stack(&host, &mut stored, 0, Some(full)));
        assert_eq!(restock_candidate(&host, &NUCLEAR, &incoming, &stored), None);
        assert!(write_stack(&host, &mut stored, 0, None));
        assert_eq!(
            restock_candidate(&host, &NUCLEAR, &incoming, &stored),
            Some((0, uranium))
        );
    }

    #[test]
    fn reactors_and_access_ports_restore_original_paper_slots() {
        for item in [NUCLEAR_REACTOR, NETHERSTAR_REACTOR, REACTOR_ACCESS_PORT] {
            let machine = ElectricMachine::for_item(item.id()).unwrap();
            assert_eq!(machine.menu_slots(), &[19, 28, 37, 25, 34, 43, 40]);
            assert!(machine.input_slot(5));
            assert!(!machine.input_slot(6));
        }
        let mut host = FakeHost::new();
        host.registries
            .resolve_items([Identifier::parse("minecraft:player_head").unwrap()])
            .unwrap();
        assert!(coolant(
            &host,
            &REACTOR_COLLANT_CELL.stack(&host, 1).unwrap()
        ));
        assert!(coolant(
            &host,
            &NETHER_ICE_COOLANT_CELL.stack(&host, 1).unwrap()
        ));
        assert!(!coolant(&host, &NEPTUNIUM.stack(&host, 1).unwrap()));
    }
}
