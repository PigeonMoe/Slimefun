// SPDX-License-Identifier: GPL-3.0-only

use std::sync::atomic::{AtomicBool, Ordering};

use mod_api::{EntityUuid, ItemStack, PlayerRef, RegistrationContext, ServerHost};

use crate::{items::item_id, machine::Machine};

static ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn register(context: &RegistrationContext) {
    ENABLED.store(context.mods().get("lifeskills").is_some(), Ordering::Relaxed);
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Production {
    player: EntityUuid,
    skill: &'static str,
    recipe: String,
    count: u32,
}

impl Production {
    pub fn for_recipe(machine: Machine, player: PlayerRef, recipe: &str, count: u32) -> Option<Self> {
        if !ENABLED.load(Ordering::Relaxed) {
            return None;
        }
        Self::new(machine, EntityUuid(player.uuid()), recipe, count)
    }

    pub fn for_stack(machine: Machine, player: PlayerRef, stack: &ItemStack) -> Option<Self> {
        if !ENABLED.load(Ordering::Relaxed) {
            return None;
        }
        Self::new(
            machine,
            EntityUuid(player.uuid()),
            &item_id(stack)?,
            stack.count(),
        )
    }

    fn new(machine: Machine, player: EntityUuid, recipe: &str, count: u32) -> Option<Self> {
        if !(1..=64).contains(&count) {
            return None;
        }
        // Select by both machine and result. Bulk processing, reversible
        // conversions and automation are outside this manual recipe list.
        let skill = match (machine, recipe) {
            (Machine::Crafting, "OUTPUT_CHEST")
            | (Machine::Compressing, "STEEL_PLATE")
            | (Machine::ArmorCrafting, "SLIME_HELMET") => "manufacturing",
            (Machine::Crafting, "FORTUNE_COOKIE" | "MONSTER_JERKY")
            | (
                Machine::Juicing,
                "APPLE_JUICE"
                | "CARROT_JUICE"
                | "MELON_JUICE"
                | "PUMPKIN_JUICE"
                | "SWEET_BERRY_JUICE"
                | "GOLDEN_APPLE_JUICE"
                | "GLOW_BERRY_JUICE",
            ) => "cooking",
            _ => return None,
        };
        Some(Self {
            player,
            skill,
            recipe: recipe.into(),
            count,
        })
    }

    fn command(&self) -> String {
        format!(
            "lifeskills bridge slimefun {:032x} {} {} {}",
            self.player.0, self.skill, self.recipe, self.count
        )
    }

    pub fn complete(self, host: &mut dyn ServerHost) {
        if ENABLED.load(Ordering::Relaxed) {
            // Run once, after the production transaction commits. A missing
            // or disabled optional receiver must not affect the output.
            let _ = host.commands().run_command(&self.command());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_production_uses_the_registered_recipe_and_real_player_uuid() {
        let player = EntityUuid(0x12345678_0123_4567_fedc_0123456789ab);
        let manufacturing = Production::new(Machine::ArmorCrafting, player, "SLIME_HELMET", 1)
            .expect("manual forge report");
        assert_eq!(
            manufacturing.command(),
            "lifeskills bridge slimefun 1234567801234567fedc0123456789ab manufacturing SLIME_HELMET 1"
        );
        let cooking = Production::new(Machine::Juicing, player, "APPLE_JUICE", 1)
            .expect("hand juicer report");
        assert_eq!(
            cooking.command(),
            "lifeskills bridge slimefun 1234567801234567fedc0123456789ab cooking APPLE_JUICE 1"
        );
    }

    #[test]
    fn reversible_unknown_and_wrong_machine_products_are_not_credited() {
        let player = EntityUuid(1);
        assert!(Production::new(Machine::Compressing, player, "CARBON", 1).is_none());
        assert!(Production::new(Machine::Crushing, player, "minecraft:coal", 8).is_none());
        assert!(Production::new(Machine::Crafting, player, "APPLE_JUICE", 1).is_none());
        assert!(Production::new(Machine::Crafting, player, "OUTPUT_CHEST", 65).is_none());
        assert!(Production::new(Machine::Crafting, player, "OUTPUT_CHEST", 0).is_none());
    }
}
