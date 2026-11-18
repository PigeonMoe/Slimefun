// SPDX-License-Identifier: GPL-3.0-only

use mod_api::{
    Host, HostTransaction, ItemPlace, ItemResource, ItemStack, LevelRef, PlayerRef, ServerHost,
    SoundSource, Text, Transaction,
};

use crate::recipes::{CRAFTING, GRINDING, Stack};
use crate::research;

#[derive(Clone, Copy)]
pub(crate) enum Machine {
    Grinding,
    Crafting,
}

#[derive(Clone, Copy)]
pub(crate) struct Dispenser {
    pub level: LevelRef,
    pub position: [i32; 3],
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

enum Refusal {
    Locked,
    MissingRecipe,
    MissingResearch(&'static str),
    NoOutputSpace,
    Unavailable,
}

impl Machine {
    pub fn at(host: &dyn Host, level: LevelRef, clicked: [i32; 3]) -> Option<(Self, Dispenser)> {
        let [x, y, z] = clicked;
        let position = [x, y.checked_sub(1)?, z];
        let input = level.block_state(host, position)?;
        if Some(input.block()) != host.block_key("minecraft:dispenser") {
            return None;
        }

        let block = level.block_state(host, clicked)?;
        let machine = match block.block().id(host).as_str() {
            "minecraft:crafting_table" => Self::Crafting,
            "minecraft:oak_fence"
                if level.block_property(host, position, "facing").as_deref() == Some("up") =>
            {
                Self::Grinding
            }
            _ => return None,
        };
        Some((machine, Dispenser { level, position }))
    }

    pub fn activate(
        self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        dispenser: Dispenser,
        clicked: [i32; 3],
    ) {
        let outcome = self.process(host, player, dispenser);
        let message = match outcome {
            Ok(()) => {
                dispenser.level.play_sound(
                    host,
                    "minecraft:block.wooden_button.click_on",
                    clicked.map(|axis| f64::from(axis) + 0.5),
                    SoundSource::Blocks,
                    1.0,
                    1.0,
                );
                return;
            }
            Err(Refusal::Locked) => "The dispenser is locked.".to_owned(),
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
    ) -> Result<(), Refusal> {
        let holder = host
            .block_entity(dispenser.level, dispenser.position)
            .ok_or(Refusal::Unavailable)?;
        if holder.lock(host).is_some() {
            return Err(Refusal::Locked);
        }

        host.transaction(|host, tx| {
            let slots = dispenser.slots(host);
            match self {
                Self::Grinding => {
                    for (slot, input) in slots.iter().enumerate() {
                        let Some(input) = input else { continue };
                        let Some(recipe) = GRINDING.iter().find(|recipe| {
                            input.count() >= recipe.input.count
                                && recipe.input.kind.matches(host, input)
                        }) else {
                            continue;
                        };
                        // Like the Paper grind stone, check space before removing the input.
                        deposit(host, tx, dispenser, recipe.output)?;
                        return consume(host, tx, &dispenser.place(slot), input, 1);
                    }
                    Err(Refusal::MissingRecipe)
                }
                Self::Crafting => {
                    let recipe = CRAFTING
                        .iter()
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
    let resource = ItemResource::of(&stack);
    let slots = dispenser.slots(host);
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

    let mut count = output.count;
    for index in occupied.chain(empty) {
        let place = ItemPlace::Strict(Box::new(dispenser.place(index)));
        count -= place.insert(host, tx, &resource, count);
        if count == 0 {
            return Ok(());
        }
    }
    Err(Refusal::NoOutputSpace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Kind;
    use mod_api::Identifier;
    use mod_api::testing::FakeHost;

    const POSITION: [i32; 3] = [0, 64, 0];
    const BLAZE_ROD: Kind = Kind::Vanilla("minecraft:blaze_rod");
    const BLAZE_POWDER: Kind = Kind::Vanilla("minecraft:blaze_powder");

    #[test]
    fn output_merges_before_using_empty_slots() {
        let (mut host, dispenser) = dispenser();
        let powder = BLAZE_POWDER.stack(&host, 62).unwrap();
        set_slot(&mut host, 4, powder);

        let result = (&mut host as &mut dyn ServerHost).transaction(|host, tx| {
            deposit(
                host,
                tx,
                dispenser,
                Stack {
                    kind: BLAZE_POWDER,
                    count: 4,
                },
            )
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

        let result = (&mut host as &mut dyn ServerHost).transaction(|host, tx| {
            deposit(
                host,
                tx,
                dispenser,
                Stack {
                    kind: BLAZE_POWDER,
                    count: 4,
                },
            )
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
