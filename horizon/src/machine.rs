// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    BlockUpdateFlags, Host, HostTransaction, ItemPlace, ItemResource, ItemStack, LevelRef,
    PlayerRef, ServerHost, SoundSource, Text, Transaction,
};

use crate::compressor::DelayedCrafts;
use crate::items::{OUTPUT_CHEST, SIFTED_ORE, STONE_CHUNK};
use crate::output_chest;
use crate::random::Rolls;
use crate::recipes::{
    ALLOY_RECIPES, COMPRESSING, CRUSHING, DOUBLING, GRINDING, JUICER, JUICING, MAKESHIFT_SMELTERY,
    ORE_WASHER, PRESSING, PRESSURE_CHAMBER, SMELTERY, SmeltingRecipe, Stack, WASHER_DUSTS, WASHING,
    crafting, output_research, smelting,
};
use crate::research;

#[derive(Clone, Copy)]
pub(crate) enum Machine {
    Grinding,
    Crafting,
    Crushing,
    Compressing,
    Pressing,
    Juicing,
    Washing,
    Smelting { heat: [i32; 3], makeshift: bool },
}

#[derive(Clone, Copy)]
pub(crate) struct Dispenser {
    pub level: LevelRef,
    pub position: [i32; 3],
}

#[derive(Clone, Copy)]
pub(crate) struct Settings {
    pub double_ores: bool,
    pub legacy_ore_washer: bool,
    pub legacy_dust_washer: bool,
    pub legacy_ore_grinder: bool,
    pub gold_pan_override_output_limit: [bool; 3],
    pub freezer_use_vanilla_ratios: [bool; 3],
    pub fire_breaking_chance: u32,
    pub drop_from_golems: bool,
    pub golem_drop_chance: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            double_ores: true,
            legacy_ore_washer: false,
            legacy_dust_washer: false,
            legacy_ore_grinder: true,
            gold_pan_override_output_limit: [false; 3],
            freezer_use_vanilla_ratios: [false; 3],
            fire_breaking_chance: 34,
            drop_from_golems: true,
            golem_drop_chance: 75,
        }
    }
}

impl Dispenser {
    fn place(self, slot: usize) -> ItemPlace {
        ItemPlace::BlockEntity {
            level: self.level,
            pos: self.position,
            side: None,
            slot,
        }
    }

    fn slots(self, host: &mut dyn ServerHost) -> [Option<ItemStack>; 9] {
        std::array::from_fn(|slot| self.place(slot).stack(host))
    }
}

pub(crate) enum Refusal {
    Locked,
    Disabled,
    MissingRecipe,
    MissingResearch(&'static str),
    NoOutputSpace,
    Unavailable,
}

impl Machine {
    pub fn at(host: &dyn Host, level: LevelRef, clicked: [i32; 3]) -> Option<(Self, Dispenser)> {
        let [x, y, z] = clicked;
        let block = level.block_state(host, clicked)?.block().id(host);
        if block == "minecraft:cauldron"
            && let Some(dispenser) = pressure_chamber_at(host, level, clicked)
        {
            return Some((Self::Pressing, dispenser));
        }
        let below = [x, y.checked_sub(1)?, z];
        let wooden_fence = is_wooden_fence(host, &block);
        let dispenser = host.block_key("minecraft:dispenser")?;
        if wooden_fence && has_block(host, level, below, "minecraft:cauldron") {
            let above = [x, y.checked_add(1)?, z];
            if level
                .block_state(host, above)
                .is_some_and(|state| state.block() == dispenser)
            {
                return Some((
                    Self::Washing,
                    Dispenser {
                        level,
                        position: above,
                    },
                ));
            }
        }
        if level.block_state(host, below)?.block() != dispenser {
            return None;
        }

        let machine = match block.as_str() {
            "minecraft:crafting_table" => Self::Crafting,
            "minecraft:nether_brick_fence"
                if y.checked_add(1)
                    .is_some_and(|top| has_block(host, level, [x, top, z], "minecraft:glass")) =>
            {
                Self::Juicing
            }
            _ if wooden_fence || block == "minecraft:nether_brick_fence" => {
                let heat = [x, y.checked_sub(2)?, z];
                let bricks = if wooden_fence {
                    "minecraft:bricks"
                } else {
                    "minecraft:nether_bricks"
                };
                let heat_block = level
                    .block_state(host, heat)
                    .map(|state| state.block().id(host));
                if matches!(
                    heat_block.as_deref(),
                    Some("minecraft:fire" | "minecraft:soul_fire")
                ) && opposite_blocks(host, level, below, &[bricks])
                {
                    Self::Smelting {
                        heat,
                        makeshift: wooden_fence,
                    }
                } else if wooden_fence {
                    Self::Grinding
                } else if opposite_blocks(host, level, below, &["minecraft:piston"]) {
                    Self::Compressing
                } else if opposite_blocks(host, level, below, &["minecraft:iron_bars"]) {
                    Self::Crushing
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some((
            machine,
            Dispenser {
                level,
                position: below,
            },
        ))
    }

    pub fn activate(
        self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        dispenser: Dispenser,
        clicked: [i32; 3],
        delayed_crafts: &DelayedCrafts,
        settings: Settings,
        rolls: &Rolls,
    ) {
        let outcome = self.process(host, player, dispenser, delayed_crafts, settings, rolls);

        let message = match outcome {
            Ok(()) if matches!(self, Self::Compressing | Self::Pressing) => return,
            Ok(()) => {
                let sound = match self {
                    Self::Washing | Self::Juicing => "minecraft:entity.player.splash",
                    Self::Smelting { heat, makeshift } => {
                        if makeshift
                            || rolls
                                .next(host, dispenser.level, 100)
                                .is_some_and(|roll| roll < settings.fire_breaking_chance)
                        {
                            exhaust_fire(host, dispenser.level, heat);
                        }
                        "minecraft:block.lava.pop"
                    }
                    _ => "minecraft:block.wooden_button.click_on",
                };
                dispenser.level.play_sound(
                    host,
                    sound,
                    clicked.map(|axis| f64::from(axis) + 0.5),
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                );
                if matches!(self, Self::Juicing) {
                    let _ = dispenser.level.play_sound(
                        host,
                        "minecraft:block.grass.break",
                        clicked.map(|axis| f64::from(axis) + 0.5),
                        SoundSource::Blocks,
                        1.0,
                        1.0,
                    );
                }
                return;
            }
            Err(Refusal::Locked) => "The dispenser is locked.".to_owned(),
            Err(Refusal::Disabled) => "The machine output is disabled in this world.".to_owned(),
            Err(Refusal::MissingRecipe) => {
                "No matching Slimefun recipe in the dispenser.".to_owned()
            }
            Err(Refusal::MissingResearch(id)) => {
                format!("Unlock this research with /sf research {id}.")
            }
            Err(Refusal::NoOutputSpace) => "The dispenser has no room for the output.".to_owned(),
            Err(Refusal::Unavailable) => "Slimefun could not change this dispenser.".to_owned(),
        };
        let _ = player.tell(host, Text::literal(message));
    }

    fn process(
        self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        dispenser: Dispenser,
        delayed_crafts: &DelayedCrafts,
        settings: Settings,
        rolls: &Rolls,
    ) -> Result<(), Refusal> {
        let required = match self {
            Self::Pressing => Some(&PRESSURE_CHAMBER),
            Self::Juicing => Some(&JUICER),
            Self::Washing => Some(&ORE_WASHER),
            Self::Smelting {
                makeshift: true, ..
            } => Some(&MAKESHIFT_SMELTERY),
            Self::Smelting {
                makeshift: false, ..
            } => Some(&SMELTERY),
            _ => None,
        };
        if let Some(research) = required
            && !research::can_use(host, player, research)
        {
            return Err(Refusal::MissingResearch(research.id));
        }
        let holder = host
            .block_entity(dispenser.level, dispenser.position)
            .ok_or(Refusal::Unavailable)?;
        if holder.lock(host).is_some() {
            return Err(Refusal::Locked);
        }

        host.transaction(|host, tx| {
            let slots = dispenser.slots(host);
            match self {
                Self::Grinding
                | Self::Crushing
                | Self::Compressing
                | Self::Pressing
                | Self::Juicing => {
                    let no_bonus: &[_] = &[];
                    let (base, bonus) = match self {
                        Self::Grinding => (GRINDING, no_bonus),
                        Self::Crushing => (CRUSHING, DOUBLING),
                        Self::Compressing => (COMPRESSING, no_bonus),
                        Self::Pressing => (PRESSING, no_bonus),
                        Self::Juicing => (JUICING, no_bonus),
                        _ => unreachable!(),
                    };
                    for (slot, input) in slots.iter().enumerate() {
                        let Some(input) = input else { continue };
                        let candidates = base.iter().map(|recipe| (recipe, 1)).chain(
                            bonus
                                .iter()
                                .map(|recipe| (recipe, 1 + u32::from(settings.double_ores))),
                        );
                        let Some((recipe, multiplier)) =
                            candidates.into_iter().find(|(recipe, _)| {
                                input.count() >= recipe.input.count
                                    && recipe.input.kind.matches(host, input)
                            })
                        else {
                            continue;
                        };
                        let output = Stack {
                            count: recipe.output.count * multiplier,
                            ..recipe.output
                        };
                        if matches!(self, Self::Crushing)
                            && let Some(research) = output_research(output.kind)
                            && !research::can_use(host, player, research)
                        {
                            return Err(Refusal::MissingResearch(research.id));
                        }
                        let consumption = if matches!(self, Self::Grinding) {
                            1
                        } else {
                            recipe.input.count
                        };
                        if matches!(self, Self::Compressing | Self::Pressing) {
                            let stack = output
                                .kind
                                .stack(host, output.count)
                                .ok_or(Refusal::Unavailable)?;
                            // Check available space before consuming, as the Paper machine does.
                            host.transaction(|host, tx| {
                                let result = deposit(host, tx, dispenser, output);
                                Err::<(), _>(result)
                            })
                            .expect_err("space check always rolls back")?;
                            consume(host, tx, &dispenser.place(slot), input, consumption)?;
                            return if matches!(self, Self::Pressing) {
                                let [x, y, z] = dispenser.position;
                                delayed_crafts.queue_pressure_chamber(
                                    host,
                                    holder,
                                    dispenser,
                                    [x, y - 2, z],
                                    stack,
                                )
                            } else {
                                delayed_crafts
                                    .queue_compressor(host, player, holder, dispenser, stack)
                            };
                        }

                        deposit(host, tx, dispenser, output)?;
                        return consume(host, tx, &dispenser.place(slot), input, consumption);
                    }
                    Err(Refusal::MissingRecipe)
                }
                Self::Washing => {
                    for (slot, item) in slots.iter().enumerate() {
                        let Some(item) = item else { continue };
                        if SIFTED_ORE.matches(host, item) {
                            let index = rolls
                                .next(host, dispenser.level, WASHER_DUSTS.len() as u32)
                                .ok_or(Refusal::Unavailable)?;
                            let dust = WASHER_DUSTS[index as usize]
                                .stack(host, 1)
                                .ok_or(Refusal::Unavailable)?;
                            let stone = STONE_CHUNK.stack(host, 1).ok_or(Refusal::Unavailable)?;
                            let require_empty = !settings.legacy_ore_washer;
                            if !output_chest::try_deposit_with_extra(
                                host,
                                dispenser.level,
                                dispenser.position,
                                &dust,
                                Some(&stone),
                                require_empty,
                            ) && !store_washed(host, tx, dispenser, &dust, &stone, require_empty)
                            {
                                return Err(Refusal::NoOutputSpace);
                            }
                            return consume(host, tx, &dispenser.place(slot), item, 1);
                        }
                        if let Some(recipe) = WASHING.iter().find(|recipe| {
                            item.count() >= recipe.input.count
                                && recipe.input.kind.matches(host, item)
                        }) {
                            deposit(host, tx, dispenser, recipe.output)?;
                            return consume(
                                host,
                                tx,
                                &dispenser.place(slot),
                                item,
                                recipe.input.count,
                            );
                        }
                    }
                    Err(Refusal::MissingRecipe)
                }
                Self::Smelting { makeshift, .. } => {
                    let alloys = if makeshift { &[] } else { ALLOY_RECIPES };
                    let (recipe, consumption) = alloys
                        .iter()
                        .chain(smelting().filter(|recipe| !makeshift || makeshift_recipe(recipe)))
                        .find_map(|recipe| {
                            smelting_inputs(host, &slots, recipe).map(|slots| (recipe, slots))
                        })
                        .ok_or(Refusal::MissingRecipe)?;
                    if let Some(research) = output_research(recipe.output.kind)
                        && !research::can_use(host, player, research)
                    {
                        return Err(Refusal::MissingResearch(research.id));
                    }
                    deposit(host, tx, dispenser, recipe.output)?;
                    for (slot, count) in consumption.into_iter().enumerate() {
                        if count != 0 {
                            let item = slots[slot].as_ref().ok_or(Refusal::Unavailable)?;
                            consume(host, tx, &dispenser.place(slot), item, count)?;
                        }
                    }
                    Ok(())
                }
                Self::Crafting => {
                    let recipe = crafting()
                        .find(|recipe| {
                            recipe.inputs.iter().zip(&slots).all(|(input, actual)| {
                                match (input, actual) {
                                    (None, None) => true,
                                    (Some(input), Some(actual)) => input.matches(host, actual),
                                    _ => false,
                                }
                            })
                        })
                        .ok_or(Refusal::MissingRecipe)?;
                    if recipe.output.kind == OUTPUT_CHEST && !output_chest::enabled() {
                        return Err(Refusal::Disabled);
                    }
                    if !research::can_use(host, player, recipe.research) {
                        return Err(Refusal::MissingResearch(recipe.research.id));
                    }
                    for (slot, item) in slots.iter().enumerate() {
                        if let Some(item) = item {
                            consume(host, tx, &dispenser.place(slot), item, 1)?;
                        }
                    }
                    deposit(host, tx, dispenser, recipe.output)
                }
            }
        })
    }
}

pub(crate) fn has_block(host: &dyn Host, level: LevelRef, pos: [i32; 3], name: &str) -> bool {
    level
        .block_state(host, pos)
        .is_some_and(|state| Some(state.block()) == host.block_key(name))
}

fn pressure_chamber_at(host: &dyn Host, level: LevelRef, [x, y, z]: [i32; 3]) -> Option<Dispenser> {
    let glass = [x, y.checked_add(1)?, z];
    let dispenser = [x, y.checked_add(2)?, z];
    let center_matches = has_block(host, level, glass, "minecraft:glass")
        && has_block(host, level, dispenser, "minecraft:dispenser");
    let sides_match = [0, 2].into_iter().any(|axis| {
        [
            ([x, y, z], "minecraft:piston"),
            (glass, "minecraft:piston"),
            (dispenser, "minecraft:smooth_stone_slab"),
        ]
        .into_iter()
        .all(|(pos, name)| opposite_on_axis(host, level, pos, axis, &[name]))
    });
    (center_matches && sides_match).then_some(Dispenser {
        level,
        position: dispenser,
    })
}

fn exhaust_fire(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
    let current = level
        .block_state(host, pos)
        .map(|state| state.block().id(host));
    if matches!(
        current.as_deref(),
        Some("minecraft:fire" | "minecraft:soul_fire")
    ) && let Some(air) = host.block_state_id("minecraft:air")
    {
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        let _ = level.set_block(host, pos, air, flags);
    }
}

pub(crate) fn makeshift_recipe(recipe: &SmeltingRecipe) -> bool {
    matches!(recipe.inputs, [input] if input.kind.id().ends_with("_DUST"))
}

fn smelting_inputs(
    host: &dyn Host,
    slots: &[Option<ItemStack>; 9],
    recipe: &SmeltingRecipe,
) -> Option<[u32; 9]> {
    let mut consumed = [0; 9];
    for ingredient in recipe.inputs {
        let index = slots.iter().enumerate().position(|(slot, item)| {
            item.as_ref().is_some_and(|item| {
                item.count().saturating_sub(consumed[slot]) >= ingredient.count
                    && ingredient.kind.matches(host, item)
            })
        })?;
        consumed[index] += ingredient.count;
    }
    Some(consumed)
}

fn store_washed(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    dispenser: Dispenser,
    dust: &ItemStack,
    stone: &ItemStack,
    require_empty: bool,
) -> bool {
    if require_empty && dispenser.slots(host).iter().all(Option::is_some) {
        return false;
    }
    if deposit_stack(host, tx, dispenser, dust, dust.count()) != 0 {
        return false;
    }
    // Paper adds the by-product to the already selected inventory and discards overflow.
    let _ = deposit_stack(host, tx, dispenser, stone, stone.count());
    true
}

fn is_wooden_fence(host: &dyn Host, block: &str) -> bool {
    host.registry_entry("minecraft:block", block)
        .and_then(|entry| host.registry_entry_in_tag(&entry, "minecraft:wooden_fences"))
        .unwrap_or(block == "minecraft:oak_fence")
}

pub(crate) fn opposite_blocks(
    host: &dyn Host,
    level: LevelRef,
    center: [i32; 3],
    supported: &[&str],
) -> bool {
    [0, 2]
        .into_iter()
        .any(|axis| opposite_on_axis(host, level, center, axis, supported))
}

fn opposite_on_axis(
    host: &dyn Host,
    level: LevelRef,
    center: [i32; 3],
    axis: usize,
    supported: &[&str],
) -> bool {
    [-1, 1].into_iter().all(|delta| {
        let mut pos = center;
        let Some(offset) = pos[axis].checked_add(delta) else {
            return false;
        };
        pos[axis] = offset;
        level
            .block_state(host, pos)
            .is_some_and(|state| supported.contains(&state.block().id(host).as_str()))
    })
}

fn consume(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    place: &ItemPlace,
    stack: &ItemStack,
    count: u32,
) -> Result<(), Refusal> {
    (place.extract(host, tx, &ItemResource::of(stack), count) == count)
        .then_some(())
        .ok_or(Refusal::Unavailable)
}

fn deposit(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    dispenser: Dispenser,
    output: Stack,
) -> Result<(), Refusal> {
    let stack = output
        .kind
        .stack(host, output.count)
        .ok_or(Refusal::Unavailable)?;
    if output_chest::try_deposit(host, dispenser.level, dispenser.position, &stack)
        || deposit_stack(host, tx, dispenser, &stack, output.count) == 0
    {
        Ok(())
    } else {
        Err(Refusal::NoOutputSpace)
    }
}

pub(crate) fn deposit_stack(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    dispenser: Dispenser,
    stack: &ItemStack,
    count: u32,
) -> u32 {
    deposit_into(
        host,
        tx,
        dispenser.level,
        dispenser.position,
        9,
        stack,
        count,
    )
}

pub(crate) fn deposit_into(
    host: &mut dyn ServerHost,
    tx: &Transaction,
    level: LevelRef,
    pos: [i32; 3],
    slot_count: usize,
    stack: &ItemStack,
    mut count: u32,
) -> u32 {
    let resource = ItemResource::of(stack);
    let place_at = |slot| ItemPlace::BlockEntity {
        level,
        pos,
        side: None,
        slot,
    };
    let slots: Vec<_> = (0..slot_count)
        .map(|slot| place_at(slot).stack(host))
        .collect();
    let occupied = slots.iter().enumerate().filter_map(|(index, current)| {
        current
            .as_ref()
            .filter(|&current| resource.matches(current))
            .map(|_| index)
    });
    let empty = slots
        .iter()
        .enumerate()
        .filter_map(|(index, stack)| stack.is_none().then_some(index));

    for index in occupied.chain(empty) {
        let place = ItemPlace::Strict(Box::new(place_at(index)));
        count -= place.insert(host, tx, &resource, count);
        if count == 0 {
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Kind;
    use crate::recipes::{CRAFTING, SMELTING};
    use mod_api::Identifier;
    use mod_api::testing::FakeHost;

    const POSITION: [i32; 3] = [0, 64, 0];
    const BLAZE_ROD: Kind = Kind::Vanilla("minecraft:blaze_rod");
    const BLAZE_POWDER: Kind = Kind::Vanilla("minecraft:blaze_powder");

    #[test]
    fn carbonado_uses_pressure_chamber_diamond_and_full_smeltery() {
        use crate::items::{CARBON_CHUNK, CARBONADO, RAW_CARBONADO, SYNTHETIC_DIAMOND};

        assert_eq!(PRESSING.len(), 2);
        assert_eq!(PRESSING[0].input.kind, CARBON_CHUNK);
        assert_eq!(PRESSING[0].output.kind, SYNTHETIC_DIAMOND);
        assert_eq!(PRESSING[1].input.kind, RAW_CARBONADO);
        assert_eq!(PRESSING[1].output.kind, CARBONADO);

        let raw_recipe = SMELTING
            .iter()
            .find(|recipe| recipe.output.kind == RAW_CARBONADO)
            .unwrap();
        assert!(!makeshift_recipe(raw_recipe));
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [
                    "minecraft:diamond",
                    "minecraft:player_head",
                    "minecraft:glass_pane",
                ]
                .map(|name| Identifier::parse(name).unwrap()),
            )
            .unwrap();
        let mut slots = std::array::from_fn(|_| None);
        slots[0] = Kind::Vanilla("minecraft:diamond").stack(&host, 1);
        slots[1] = CARBON_CHUNK.stack(&host, 1);
        slots[2] = Kind::Vanilla("minecraft:glass_pane").stack(&host, 1);
        assert_eq!(smelting_inputs(&host, &slots, raw_recipe), None);
        slots[0] = SYNTHETIC_DIAMOND.stack(&host, 1);
        assert_eq!(
            smelting_inputs(&host, &slots, raw_recipe),
            Some([1, 1, 1, 0, 0, 0, 0, 0, 0])
        );
    }

    #[test]
    fn lead_progression_from_gravel_to_output_chest_has_all_required_recipes() {
        use crate::items::{LEAD_DUST, LEAD_INGOT, OUTPUT_CHEST};
        use crate::recipes::PAN_DROPS;

        assert!(
            PAN_DROPS
                .iter()
                .any(|&(weight, item)| weight == 35 && item == SIFTED_ORE)
        );
        assert_eq!(
            WASHER_DUSTS
                .iter()
                .filter(|&&item| item == LEAD_DUST)
                .count(),
            1
        );

        let recipe = SMELTING
            .iter()
            .find(|recipe| recipe.output.kind == LEAD_INGOT)
            .unwrap();
        assert!(makeshift_recipe(recipe));
        assert!(matches!(recipe.inputs, [input] if input.kind == LEAD_DUST && input.count == 1));

        let chest = CRAFTING
            .iter()
            .find(|recipe| recipe.output.kind == OUTPUT_CHEST)
            .unwrap();
        assert_eq!(
            chest
                .inputs
                .iter()
                .filter(|&&input| input == Some(LEAD_INGOT))
                .count(),
            5
        );
        assert_eq!(chest.research.cost, 20);
    }

    #[test]
    fn makeshift_smeltery_copies_only_single_dust_inputs() {
        assert_eq!(
            SMELTING
                .iter()
                .filter(|recipe| makeshift_recipe(recipe))
                .count(),
            9
        );
        assert!(!ALLOY_RECIPES.iter().any(makeshift_recipe));
    }

    #[test]
    fn output_merges_before_using_empty_slots() {
        let (mut host, dispenser) = dispenser();
        let powder = BLAZE_POWDER.stack(&host, 62).unwrap();
        set_slot(&mut host, 4, powder);

        let output = BLAZE_POWDER.stack(&host, 4).unwrap();
        let result = (&mut host as &mut dyn ServerHost).transaction(|host, tx| {
            (deposit_stack(host, tx, dispenser, &output, 4) == 0)
                .then_some(())
                .ok_or(Refusal::NoOutputSpace)
        });

        assert!(result.is_ok());
        let slots = dispenser.slots(&mut host);
        assert_eq!(slots[4].as_ref().unwrap().count(), 64);
        assert_eq!(slots[0].as_ref().unwrap().count(), 2);
    }

    #[test]
    fn failed_output_rolls_back_a_partial_stack_merge() {
        let (mut host, dispenser) = dispenser();
        let filler = BLAZE_ROD.stack(&host, 64).unwrap();
        for slot in 0..9 {
            set_slot(&mut host, slot, filler.clone());
        }
        let powder = BLAZE_POWDER.stack(&host, 63).unwrap();
        set_slot(&mut host, 4, powder);

        let output = BLAZE_POWDER.stack(&host, 4).unwrap();
        let result = (&mut host as &mut dyn ServerHost).transaction(|host, tx| {
            (deposit_stack(host, tx, dispenser, &output, 4) == 0)
                .then_some(())
                .ok_or(Refusal::NoOutputSpace)
        });

        assert!(matches!(result, Err(Refusal::NoOutputSpace)));
        let slots = dispenser.slots(&mut host);
        assert_eq!(slots[4].as_ref().unwrap().count(), 63);
        assert_eq!(slots[0].as_ref().unwrap().count(), 64);
    }

    fn dispenser() -> (FakeHost, Dispenser) {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                [BLAZE_ROD.id(), BLAZE_POWDER.id()].map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        host.level.block_entity(POSITION).with_items(9);
        let dispenser = Dispenser {
            level: host.level.level(),
            position: POSITION,
        };
        (host, dispenser)
    }

    fn set_slot(host: &mut FakeHost, slot: usize, stack: ItemStack) {
        host.level
            .block_entity(POSITION)
            .items
            .as_mut()
            .unwrap()
            .slots[slot] = Some(stack);
    }
}
