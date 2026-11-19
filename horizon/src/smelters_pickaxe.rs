// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    BlockDropsEvent, BlockState, BreakBlockEvent, ComponentId, Compound, Flow, GameMode, Hand,
    Host, HostTransaction, ItemDrop, ItemKey, ItemPlace, ItemResource, ItemStack, LevelRef,
    PlayerDestroyedBlockEvent, PlayerRef, ScheduledTaskEvent, ServerHost, SoundSource, Tag, TaskId,
    Text,
};

use crate::{
    elemental_staves::LAVA_CRYSTAL,
    items::{FERROSILICON, item, item_id, text},
    network_runtime,
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    SMELTERS_PICKAXE,
    "diamond_pickaxe",
    "Smelter's Pickaxe",
    "gold",
    None,
    None
);

pub(crate) const RESEARCH: Research = Research {
    id: "smelters_pickaxe",
    name: "Smelters Pickaxe",
    cost: 17,
};

pub(crate) const CRAFTING: CraftingRecipe = CraftingRecipe {
    inputs: [
        Some(LAVA_CRYSTAL),
        Some(LAVA_CRYSTAL),
        Some(LAVA_CRYSTAL),
        None,
        Some(FERROSILICON),
        None,
        None,
        Some(FERROSILICON),
        None,
    ],
    output: Stack {
        kind: SMELTERS_PICKAXE,
        count: 1,
    },
    research: &RESEARCH,
};

pub(crate) fn decorate(id: &str, components: &mut Compound) {
    if id != SMELTERS_PICKAXE.id() {
        return;
    }
    let mut enchant = text("Auto-Smelting", "red", false);
    if let Tag::Compound(style) = &mut enchant {
        style.insert("bold".into(), Tag::Byte(1));
    }
    components.insert(
        "minecraft:lore".into(),
        Tag::List(vec![
            enchant,
            Tag::String("".into()),
            text("Works with Fortune", "dark_blue", false),
        ]),
    );
}

// Paper's smelters_pickaxe_blocks includes all three ore tags and ancient debris.
fn ore(id: &str) -> bool {
    let Some(id) = id.strip_prefix("minecraft:") else {
        return false;
    };
    match id {
        "nether_gold_ore" | "nether_quartz_ore" | "ancient_debris" => true,
        _ => matches!(
            id.strip_prefix("deepslate_").unwrap_or(id),
            "coal_ore"
                | "iron_ore"
                | "copper_ore"
                | "gold_ore"
                | "redstone_ore"
                | "emerald_ore"
                | "lapis_ore"
                | "diamond_ore"
        ),
    }
}

fn enchantment(host: &dyn Host, tool: &ItemStack, enchantment: &str) -> u32 {
    let Some(component) = ComponentId::new("minecraft:enchantments") else {
        return 0;
    };
    ItemResource::of(tool)
        .value(host, &component)
        .and_then(|levels| match levels.get(enchantment) {
            Some(Tag::Int(level)) => u32::try_from(*level).ok(),
            _ => None,
        })
        .unwrap_or(0)
        .min(255)
}

fn fortune_amount(fortune: u32, silk: bool, block: &str, mut draw: impl FnMut(u32) -> u32) -> u32 {
    if fortune == 0 || silk {
        return 1;
    }
    let rolls = draw(fortune.saturating_add(2)).saturating_sub(1).max(1) + 1;
    if block == "minecraft:lapis_ore" {
        (4 + draw(5)) * rolls
    } else {
        rolls
    }
}

fn furnace_output(
    host: &dyn Host,
    recipes: &[mod_api::Recipe],
    input: &ItemStack,
) -> Option<ItemKey> {
    let id = input.item().id(host);
    recipes.iter().find_map(|recipe| {
        let [ingredient] = recipe.ingredients.as_slice() else {
            return None;
        };
        // Paper's MaterialChoice compares materials, not the source stack's metadata.
        let matches = if ingredient.custom.is_none()
            && ingredient.children.is_none()
            && ingredient.rule.is_none()
        {
            ingredient.items.contains(&id)
        } else {
            ingredient.test(host, input)
        };
        if !matches {
            return None;
        }
        recipe.results.first().map(ItemStack::item)
    })
}

fn change_drop_type(source: &ItemStack, item: ItemKey, count: u32) -> Option<ItemStack> {
    let mut stack = ItemStack::new(item, count)?;
    if let Some(components) = source.components() {
        stack.apply_components(components.as_nbt()).ok()?;
    }
    if let Some(damage) = source.damage_component() {
        stack.set_damage(damage);
    }
    Some(stack)
}

fn prepare_drops(
    host: &dyn Host,
    inputs: &[ItemStack],
    amount: u32,
) -> Option<(Vec<ItemStack>, usize)> {
    let recipes = host.recipes_of_type("minecraft:smelting");
    let mut outputs = Vec::new();
    let mut flares = 0;
    for input in inputs {
        let smelted = furnace_output(host, &recipes, input);
        flares += usize::from(smelted.is_some());
        let item = smelted.unwrap_or_else(|| input.item());
        let limit = host.max_stack_size(item)?.max(1);
        let mut remaining = amount;
        while remaining > 0 {
            let count = remaining.min(limit);
            outputs.push(change_drop_type(input, item, count)?);
            remaining -= count;
        }
    }
    Some((outputs, flares))
}

type Address = (u128, String, [i32; 3]);

struct Prepared {
    state: BlockState,
    tool: ItemStack,
    drops: Vec<ItemStack>,
    flares: usize,
}

struct Ready {
    state: BlockState,
    drops: Vec<ItemStack>,
}

#[derive(Default)]
struct Pending {
    ready: BTreeMap<Address, Ready>,
    prepared: BTreeMap<Address, Prepared>,
}

#[derive(Default)]
pub(crate) struct SmeltersPickaxes {
    pending: Mutex<Pending>,
    worn_out: Mutex<BTreeMap<TaskId, (PlayerRef, ItemStack)>>,
}

fn held_pickaxe(host: &mut dyn ServerHost, player: PlayerRef) -> Option<ItemStack> {
    let place = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    };
    place
        .stack(host)
        .filter(|stack| SMELTERS_PICKAXE.matches(host, stack))
}

impl SmeltersPickaxes {
    pub(crate) fn prepare(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        event: &BreakBlockEvent<'_>,
        rolls: &Rolls,
    ) -> Flow {
        let Some(tool) = held_pickaxe(host, player) else {
            return Flow::Continue;
        };
        let Some(dimension) = level.dimension().id(host) else {
            return Flow::Continue;
        };
        let address = (player.uuid(), dimension, event.pos);
        self.pending.lock().unwrap().prepared.remove(&address);
        if player.is_fake(host) == Some(true) {
            return Flow::Continue;
        }
        if !research::can_use(host, player, &RESEARCH) {
            let _ = player.tell(
                host,
                Text::literal("Unlock Smelters Pickaxe with /sf research smelters_pickaxe."),
            );
            return Flow::Cancel;
        }
        let block = event.state.block().id(host);
        if !ore(&block) || network_runtime::has_block_marker(host, level, event.pos) {
            return Flow::Continue;
        }
        let amount = fortune_amount(
            enchantment(host, &tool, "minecraft:fortune"),
            enchantment(host, &tool, "minecraft:silk_touch") > 0,
            &block,
            |bound| rolls.next(host, level, bound).unwrap_or(0),
        );
        let Some(raw) = level
            .block_drops(host, event.pos, Some(&tool), None)
            .ok()
            .flatten()
        else {
            return Flow::Continue;
        };
        let Some((drops, flares)) = prepare_drops(host, &raw, amount) else {
            return Flow::Continue;
        };
        self.pending.lock().unwrap().prepared.insert(
            address,
            Prepared {
                state: event.state,
                tool,
                drops,
                flares,
            },
        );
        Flow::Continue
    }

    pub(crate) fn destroyed(
        &self,
        host: &mut dyn ServerHost,
        event: &PlayerDestroyedBlockEvent,
        rolls: &Rolls,
    ) {
        if self.pending.lock().unwrap().prepared.is_empty() {
            return;
        }
        let Some(player) = host.players().acting_player() else {
            return;
        };
        let Some(level) = player.entity().level(host) else {
            return;
        };
        let Some(dimension) = level.dimension().id(host) else {
            return;
        };
        let address = (player.uuid(), dimension, event.pos);
        let plan = self.pending.lock().unwrap().prepared.remove(&address);
        let Some(plan) = plan.filter(|plan| {
            plan.state == event.state && held_pickaxe(host, player).as_ref() == Some(&plan.tool)
        }) else {
            return;
        };
        let creative = player.game_mode(host) == Some(GameMode::Creative);
        self.extra_wear(host, player, level, &plan.tool, creative, rolls);
        for _ in 0..plan.flares {
            let _ = level.level_event(host, 2004, event.pos, 1);
        }
        if !creative && !plan.drops.is_empty() {
            self.pending.lock().unwrap().ready.insert(
                address,
                Ready {
                    state: plan.state,
                    drops: plan.drops,
                },
            );
        }
    }

    fn extra_wear(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        level: LevelRef,
        tool: &ItemStack,
        creative: bool,
        rolls: &Rolls,
    ) {
        let unbreaking = enchantment(host, tool, "minecraft:unbreaking");
        if unbreaking > 0 && rolls.next(host, level, unbreaking.saturating_add(1)) != Some(0) {
            return;
        }
        let Some(unbreakable) = ComponentId::new("minecraft:unbreakable") else {
            return;
        };
        if ItemResource::of(tool).value(host, &unbreakable).is_some() {
            return;
        }
        let Some(max) = host.max_damage(tool.item()).filter(|max| *max != 0) else {
            return;
        };
        let damage = tool.damage();
        if damage >= max {
            if creative {
                break_tool(host, player, tool);
            } else if let Ok(task) = host.schedule_task(0, 0) {
                // The game's harvest and XP calculation still needs the original tool now.
                self.worn_out
                    .lock()
                    .unwrap()
                    .insert(task, (player, tool.clone()));
            }
            return;
        }
        let held = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand: Hand::Main,
        };
        let _ = host.transaction(|host, tx| {
            held.edit(host, tx, |stack| stack.set_damage(damage + 1))
                .then_some(())
                .ok_or(())
        });
    }

    pub(crate) fn drop_items(&self, event: &mut BlockDropsEvent<'_>) {
        let Some(uuid) = event.breaker else {
            return;
        };
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let address = (
            uuid,
            network_runtime::dimension_name(&event.dimension).into(),
            pos,
        );
        let plan = self.pending.lock().unwrap().ready.remove(&address);
        let Some(plan) = plan.filter(|plan| plan.state == event.state) else {
            return;
        };
        if !event
            .tool
            .is_some_and(|tool| item_id(tool).as_deref() == Some(SMELTERS_PICKAXE.id()))
        {
            return;
        }
        let mut previous = std::mem::take(event.drops).into_iter();
        for stack in plan.drops {
            if let Some(mut drop) = previous.next() {
                drop.stack = stack;
                event.drops.push(drop);
            } else {
                let at = pos.map(|axis| f64::from(axis) + 0.5);
                event
                    .drops
                    .push(ItemDrop::new(stack, at, [0.0, 0.2, 0.0], 10));
            }
        }
    }

    pub(crate) fn due(&self, host: &mut dyn ServerHost, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        let entry = self.worn_out.lock().unwrap().remove(&event.id);
        if let Some((player, tool)) = entry
            && host.player(player.uuid()) == Some(player)
        {
            break_tool(host, player, &tool);
        }
    }

    pub(crate) fn begin_tick(&self) {
        *self.pending.lock().unwrap() = Pending::default();
    }

    pub(crate) fn clear(&self) {
        self.begin_tick();
        self.worn_out.lock().unwrap().clear();
    }
}

fn break_tool(host: &mut dyn ServerHost, player: PlayerRef, tool: &ItemStack) {
    if crate::item_clicks::consume_one(host, player, Hand::Main, tool) {
        let _ = player.play_sound(
            host,
            "minecraft:entity.item.break",
            SoundSource::Players,
            1.0,
            1.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Kind;
    use mod_api::{Identifier, Ingredient, Recipe, testing::FakeHost};
    use std::sync::Arc;

    #[test]
    fn exact_paper_recipe_and_eligible_ore_families() {
        assert_eq!(
            CRAFTING.inputs.map(|item| item.map_or("", Kind::id)),
            [
                "LAVA_CRYSTAL",
                "LAVA_CRYSTAL",
                "LAVA_CRYSTAL",
                "",
                "FERROSILICON",
                "",
                "",
                "FERROSILICON",
                "",
            ]
        );
        assert_eq!(
            (CRAFTING.output.kind, CRAFTING.output.count),
            (SMELTERS_PICKAXE, 1)
        );
        assert_eq!((RESEARCH.id, RESEARCH.cost), ("smelters_pickaxe", 17));
        for material in [
            "coal", "iron", "copper", "gold", "redstone", "emerald", "lapis", "diamond",
        ] {
            assert!(ore(&format!("minecraft:{material}_ore")));
            assert!(ore(&format!("minecraft:deepslate_{material}_ore")));
        }
        for material in ["ancient_debris", "nether_quartz_ore", "nether_gold_ore"] {
            assert!(ore(&format!("minecraft:{material}")));
        }
        for material in [
            "minecraft:deepslate_nether_gold_ore",
            "minecraft:raw_iron_block",
            "example:iron_ore",
            "minecraft:glowstone",
        ] {
            assert!(!ore(material));
        }
    }

    #[test]
    fn fortune_uses_paper_extra_roll_and_only_stone_lapis_has_the_lapis_factor() {
        for (fortune, silk) in [(0, false), (3, true)] {
            assert_eq!(
                fortune_amount(fortune, silk, "minecraft:lapis_ore", |_| panic!(
                    "no fortune draw"
                )),
                1
            );
        }
        assert_eq!(fortune_amount(3, false, "minecraft:iron_ore", |_| 0), 2);
        assert_eq!(
            fortune_amount(3, false, "minecraft:deepslate_lapis_ore", |_| 4),
            4
        );
        let mut samples = [4, 2].into_iter();
        assert_eq!(
            fortune_amount(3, false, "minecraft:lapis_ore", |_| samples.next().unwrap()),
            24
        );
        assert!(samples.next().is_none());
    }

    #[test]
    fn changing_type_keeps_the_source_components_not_the_furnace_result_stack() {
        let mut host = FakeHost::new();
        let raw = Kind::Vanilla("minecraft:raw_iron");
        let ingot = Kind::Vanilla("minecraft:iron_ingot");
        host.registries
            .resolve_items(
                [raw.material(), ingot.material()].map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        let mut source = raw.stack(&host, 7).unwrap();
        source
            .apply_components(&Compound::from([(
                "minecraft:custom_data".into(),
                Tag::compound([("oracle", Tag::Int(3))]),
            )]))
            .unwrap();
        let target = ingot.stack(&host, 1).unwrap().item();
        let mut recipe = Recipe::new(
            "test:smelter_output",
            "minecraft:smelting",
            Arc::new(r#"{"type":"minecraft:smelting"}"#.parse().unwrap()),
        );
        recipe
            .ingredients
            .push(Ingredient::of(vec![raw.material().into()]));
        recipe.results.push(ingot.stack(&host, 4).unwrap());
        let recipes = [recipe];

        assert_eq!(furnace_output(&host, &recipes, &source), Some(target));
        assert_eq!(
            furnace_output(&host, &recipes, &ingot.stack(&host, 1).unwrap()),
            None
        );
        let changed = change_drop_type(&source, target, 2).unwrap();
        assert_eq!((changed.item(), changed.count()), (target, 2));
        assert_eq!(changed.components(), source.components());
    }
}
