// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::BTreeMap, sync::Mutex};

use mod_api::{
    Compound, Host, HostTransaction, ItemPlace, ItemResource, ItemStack, LivingRef, PlayerRef,
    ScheduledTaskEvent, ServerHost, SoundSource, Tag, TaskId, Text,
};

use crate::{
    factory::charging_bench,
    items::{
        ALUMINUM_BRONZE_INGOT, BILLON_INGOT, CARBONADO, DAMASCUS_STEEL_INGOT, DURALUMIN_INGOT,
        Kind, REINFORCED_ALLOY_INGOT, SMALL_CAPACITOR, SOLDER_INGOT, STEEL_INGOT, STEEL_PLATE,
        item, item_id, text,
    },
    network_recipes::{LARGE_CAPACITOR, MEDIUM_CAPACITOR, POWER_CRYSTAL},
    random::Rolls,
    recipes::{CraftingRecipe, Research, Stack},
    research,
};

item!(
    STEEL_THRUSTER,
    "bucket",
    "Steel Thruster",
    "gray",
    None,
    None
);
item!(
    DURALUMIN_JETBOOTS,
    "leather_boots",
    "Jet Boots - I",
    "blue",
    None,
    None
);
item!(
    SOLDER_JETBOOTS,
    "leather_boots",
    "Jet Boots - II",
    "blue",
    None,
    None
);
item!(
    BILLON_JETBOOTS,
    "leather_boots",
    "Jet Boots - III",
    "blue",
    None,
    None
);
item!(
    STEEL_JETBOOTS,
    "leather_boots",
    "Jet Boots - IV",
    "blue",
    None,
    None
);
item!(
    DAMASCUS_STEEL_JETBOOTS,
    "leather_boots",
    "Jet Boots - V",
    "blue",
    None,
    None
);
item!(
    REINFORCED_ALLOY_JETBOOTS,
    "leather_boots",
    "Jet Boots - VI",
    "blue",
    None,
    None
);
item!(
    CARBONADO_JETBOOTS,
    "leather_boots",
    "Jet Boots - VII",
    "blue",
    None,
    None
);
item!(
    ARMORED_JETBOOTS,
    "iron_boots",
    "Armored Jet Boots",
    "blue",
    None,
    None
);

const THRUSTERS: Research = Research {
    id: "steel_thruster",
    name: "Steel Thruster",
    cost: 14,
};
const JET_BOOTS: Research = Research {
    id: "jetboots",
    name: "Jet Boots",
    cost: 25,
};
const CARBONADO_TOOLS: Research = Research {
    id: "carbonado_tools",
    name: "Top Tier Machines",
    cost: 24,
};
const ARMORED: Research = Research {
    id: "armored_jetboots",
    name: "Armored Jet Boots",
    cost: 27,
};

pub(crate) const RESEARCHES: &[&Research] = &[&THRUSTERS, &JET_BOOTS, &CARBONADO_TOOLS, &ARMORED];

struct Spec {
    item: Kind,
    tier: Option<&'static str>,
    frame: Kind,
    material: &'static str,
    capacitor: Kind,
    maximum: f32,
    speed: f64,
    accuracy: (&'static str, &'static str),
    dye: Option<i32>,
    research: &'static Research,
}

const SILVER: Option<i32> = Some(0xc0c0c0);
const BLACK: Option<i32> = Some(0x000000);

const SPECS: [Spec; 8] = [
    Spec {
        item: DURALUMIN_JETBOOTS,
        tier: Some("I"),
        frame: DURALUMIN_INGOT,
        material: "Duralumin",
        capacitor: SMALL_CAPACITOR,
        maximum: 20.0,
        speed: 0.35,
        accuracy: ("50%", "red"),
        dye: SILVER,
        research: &JET_BOOTS,
    },
    Spec {
        item: SOLDER_JETBOOTS,
        tier: Some("II"),
        frame: SOLDER_INGOT,
        material: "Solder",
        capacitor: SMALL_CAPACITOR,
        maximum: 30.0,
        speed: 0.4,
        accuracy: ("60%", "gold"),
        dye: SILVER,
        research: &JET_BOOTS,
    },
    Spec {
        item: BILLON_JETBOOTS,
        tier: Some("III"),
        frame: BILLON_INGOT,
        material: "Billon",
        capacitor: SMALL_CAPACITOR,
        maximum: 40.0,
        speed: 0.45,
        accuracy: ("65%", "gold"),
        dye: SILVER,
        research: &JET_BOOTS,
    },
    Spec {
        item: STEEL_JETBOOTS,
        tier: Some("IV"),
        frame: STEEL_INGOT,
        material: "Steel",
        capacitor: SMALL_CAPACITOR,
        maximum: 50.0,
        speed: 0.5,
        accuracy: ("70%", "yellow"),
        dye: SILVER,
        research: &JET_BOOTS,
    },
    Spec {
        item: DAMASCUS_STEEL_JETBOOTS,
        tier: Some("V"),
        frame: DAMASCUS_STEEL_INGOT,
        material: "Damascus Steel",
        capacitor: SMALL_CAPACITOR,
        maximum: 75.0,
        speed: 0.55,
        accuracy: ("75%", "green"),
        dye: SILVER,
        research: &JET_BOOTS,
    },
    Spec {
        item: REINFORCED_ALLOY_JETBOOTS,
        tier: Some("VI"),
        frame: REINFORCED_ALLOY_INGOT,
        material: "Reinforced Alloy",
        capacitor: MEDIUM_CAPACITOR,
        maximum: 100.0,
        speed: 0.6,
        accuracy: ("80%", "red"),
        dye: SILVER,
        research: &JET_BOOTS,
    },
    Spec {
        item: CARBONADO_JETBOOTS,
        tier: Some("VII"),
        frame: CARBONADO,
        material: "Carbonado",
        capacitor: LARGE_CAPACITOR,
        maximum: 125.0,
        speed: 0.7,
        accuracy: ("99.9%", "red"),
        dye: BLACK,
        research: &CARBONADO_TOOLS,
    },
    Spec {
        item: ARMORED_JETBOOTS,
        tier: None,
        frame: STEEL_PLATE,
        material: "Steel",
        capacitor: MEDIUM_CAPACITOR,
        maximum: 50.0,
        speed: 0.45,
        accuracy: ("70%", "yellow"),
        dye: None,
        research: &ARMORED,
    },
];

const fn recipe(spec: &Spec) -> CraftingRecipe {
    CraftingRecipe {
        inputs: [
            None,
            None,
            None,
            Some(spec.frame),
            Some(POWER_CRYSTAL),
            Some(spec.frame),
            Some(STEEL_THRUSTER),
            Some(spec.capacitor),
            Some(STEEL_THRUSTER),
        ],
        output: Stack {
            kind: spec.item,
            count: 1,
        },
        research: spec.research,
    }
}

pub(crate) const CRAFTING: &[CraftingRecipe] = &[
    recipe(&SPECS[0]),
    recipe(&SPECS[1]),
    recipe(&SPECS[2]),
    recipe(&SPECS[3]),
    recipe(&SPECS[4]),
    recipe(&SPECS[5]),
    recipe(&SPECS[6]),
    recipe(&SPECS[7]),
    CraftingRecipe {
        inputs: [
            None,
            Some(Kind::Vanilla("minecraft:redstone")),
            None,
            Some(ALUMINUM_BRONZE_INGOT),
            Some(ALUMINUM_BRONZE_INGOT),
            Some(ALUMINUM_BRONZE_INGOT),
            Some(STEEL_PLATE),
            Some(Kind::Vanilla("minecraft:fire_charge")),
            Some(STEEL_PLATE),
        ],
        output: Stack {
            kind: STEEL_THRUSTER,
            count: 1,
        },
        research: &THRUSTERS,
    },
];

fn spec(id: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|spec| spec.item.id() == id)
}

pub(crate) fn charge_spec(id: &str) -> Option<(&'static str, f32)> {
    spec(id).map(|spec| (spec.item.material(), spec.maximum))
}

fn boots(host: &dyn Host, stack: &ItemStack) -> Option<&'static Spec> {
    let spec = spec(&item_id(stack)?)?;
    spec.item.matches(host, stack).then_some(spec)
}

fn label<const N: usize>(parts: [Tag; N]) -> Tag {
    Tag::compound([
        ("text", Tag::String("".into())),
        ("italic", Tag::Byte(0)),
        ("extra", Tag::List(parts.into())),
    ])
}

pub(crate) fn decorate(id: &str, data: &mut Compound) {
    if id == STEEL_THRUSTER.id() {
        let mut name = text("Steel Thruster", "gray", false);
        if let Tag::Compound(style) = &mut name {
            style.insert("bold".into(), Tag::Byte(1));
        }
        data.insert("minecraft:custom_name".into(), name);
        return;
    }
    let Some(spec) = spec(id) else {
        return;
    };
    if let Some(tier) = spec.tier {
        data.insert(
            "minecraft:custom_name".into(),
            label([
                text("Jet Boots", "dark_blue", false),
                text(" - ", "gray", false),
                text(tier, "yellow", false),
            ]),
        );
    } else {
        data.insert(
            "minecraft:custom_name".into(),
            text("Armored Jet Boots", "dark_blue", false),
        );
    }
    if let Some(color) = spec.dye {
        data.insert("minecraft:dyed_color".into(), Tag::Int(color));
    }
    let blank = || Tag::String("".into());
    let arrow = || text("⇨ ", "dark_gray", false);
    let (accuracy, color) = spec.accuracy;
    let lore = vec![
        blank(),
        label([
            arrow(),
            text("Material: ", "gray", false),
            text(spec.material, "aqua", false),
        ]),
        label([
            arrow(),
            text("⚡ ", "yellow", false),
            text(&format!("0 / {} J", spec.maximum as u32), "gray", false),
        ]),
        label([
            arrow(),
            text("⚡ ", "aqua", false),
            text("Speed: ", "gray", false),
            text(&format!("{}x", spec.speed), "aqua", false),
        ]),
        label([
            arrow(),
            text("Accuracy: ", "gray", false),
            text(accuracy, color, false),
        ]),
        blank(),
        label([
            text("Crouch", "yellow", false),
            text(" to use", "gray", false),
        ]),
    ];
    data.insert("minecraft:lore".into(), Tag::List(lore));
}

const PERIOD: u32 = 2;
const COST: f32 = 0.075;

fn thrust(speed: f64, [x, _, z]: [f64; 3], positive: bool) -> [f64; 3] {
    let accuracy = ((speed - 0.7) * 100.0).round() / 100.0;
    let offset = if positive { accuracy } else { -accuracy };
    [x * speed + offset, 0.04, z * speed - offset]
}

fn feet(player: PlayerRef) -> ItemPlace {
    ItemPlace::PlayerSlot {
        player: player.uuid(),
        slot: 100,
    }
}

#[derive(Clone, Copy)]
struct Active {
    player: PlayerRef,
    spec: &'static Spec,
    task: Option<TaskId>,
}

#[derive(Default)]
pub(crate) struct JetBoots {
    // A seen crouch remains here after its charge is exhausted, until the next release.
    crouched: Mutex<BTreeMap<u128, Option<Active>>>,
}

impl JetBoots {
    pub(crate) fn toggle(&self, host: &mut dyn ServerHost, player: PlayerRef, sneaking: bool) {
        let uuid = player.uuid();
        if !sneaking {
            self.forget(host, uuid);
            return;
        }
        if self.crouched.lock().unwrap().contains_key(&uuid) {
            return;
        }
        let active = feet(player)
            .stack(host)
            .and_then(|stack| boots(host, &stack))
            .and_then(|spec| {
                if !research::can_use(host, player, spec.research) {
                    let _ = player.tell(
                        host,
                        Text::literal(format!(
                            "Unlock {} with /sf research {}.",
                            spec.research.name, spec.research.id
                        )),
                    );
                    return None;
                }
                let task = host.schedule_task(0, 0).ok()?;
                Some(Active {
                    player,
                    spec,
                    task: Some(task),
                })
            });
        self.crouched.lock().unwrap().insert(uuid, active);
    }

    pub(crate) fn tick(&self, host: &mut dyn ServerHost, player: PlayerRef, rolls: &Rolls) {
        let Some(sneaking) = player.is_secondary_use_active(host) else {
            return;
        };
        // Integrated players do not send the guest-only PlayerToggleSneak packet.
        self.toggle(host, player, sneaking);
        if !sneaking {
            return;
        }
        let uuid = player.uuid();
        let active = self
            .crouched
            .lock()
            .unwrap()
            .get(&uuid)
            .copied()
            .flatten()
            .filter(|active| active.task.is_none() && active.player == player);
        let Some(active) = active else {
            return;
        };
        let living = LivingRef::new(player.entity());
        let alive = living.health(host).is_some_and(|health| health > 0.0);
        let next = if alive && pulse(host, active, rolls) {
            host.schedule_task(PERIOD, 0).ok().map(|task| Active {
                task: Some(task),
                ..active
            })
        } else {
            None
        };
        let mut riders = self.crouched.lock().unwrap();
        if let Some(slot) = riders.get_mut(&uuid)
            && slot.is_some_and(|slot| slot.task.is_none() && slot.player == player)
        {
            *slot = next;
        }
    }

    pub(crate) fn due(&self, event: &ScheduledTaskEvent<'_>) {
        if event.owner != "slimefun" {
            return;
        }
        // The next player tick runs in that player's actual level; a scheduled
        // task itself may run while their dimension is parked.
        if let Some(active) = self
            .crouched
            .lock()
            .unwrap()
            .values_mut()
            .flatten()
            .find(|active| active.task == Some(event.id))
        {
            active.task = None;
        }
    }

    pub(crate) fn forget(&self, host: &mut dyn ServerHost, uuid: u128) {
        let active = self.crouched.lock().unwrap().remove(&uuid).flatten();
        if let Some(task) = active.and_then(|active| active.task) {
            let _ = host.cancel_task(task);
        }
    }

    pub(crate) fn clear(&self) {
        self.crouched.lock().unwrap().clear();
    }
}

fn pulse(host: &mut dyn ServerHost, active: Active, rolls: &Rolls) -> bool {
    let player = active.player;
    let entity = player.entity();
    let feet = feet(player);
    let Some(stack) = feet.stack(host) else {
        // Paper leaves the sneak task running while the boot slot is briefly empty.
        return true;
    };
    if boots(host, &stack).is_none() {
        return false;
    }
    let Some(replacement) = charging_bench::discharge(host, &stack, COST) else {
        return false;
    };
    let (Some(level), Some(pos), Some(view)) = (
        entity.level(host),
        entity.position(host),
        entity.view_vector(host),
    ) else {
        return false;
    };
    let motion = thrust(
        active.spec.speed,
        view,
        rolls.next(host, level, 2) == Some(1),
    );
    let paid = host.transaction(|host, tx| {
        if feet.resource(host).as_ref() != Some(&ItemResource::of(&stack))
            || feet.exchange(host, tx, &ItemResource::of(&replacement), 1) != 1
            || !entity.set_velocity(host, motion)
        {
            return Err(());
        }
        Ok(())
    });
    if paid.is_err() {
        return false;
    }
    let _ = level.play_sound(
        host,
        "minecraft:entity.tnt.primed",
        pos,
        SoundSource::Players,
        0.25,
        1.0,
    );
    if let Some(block) = entity.block_position(host) {
        let _ = level.level_event(host, 2000, block, 1);
    }
    let _ = entity.reset_fall_distance(host);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{Identifier, testing::FakeHost};

    #[test]
    fn all_eight_jet_boots_and_the_thruster_copy_paper_inputs_and_research() {
        let rows = [
            (
                "DURALUMIN_JETBOOTS",
                "DURALUMIN_INGOT",
                "SMALL_CAPACITOR",
                20.0,
                0.35,
                "jetboots",
            ),
            (
                "SOLDER_JETBOOTS",
                "SOLDER_INGOT",
                "SMALL_CAPACITOR",
                30.0,
                0.4,
                "jetboots",
            ),
            (
                "BILLON_JETBOOTS",
                "BILLON_INGOT",
                "SMALL_CAPACITOR",
                40.0,
                0.45,
                "jetboots",
            ),
            (
                "STEEL_JETBOOTS",
                "STEEL_INGOT",
                "SMALL_CAPACITOR",
                50.0,
                0.5,
                "jetboots",
            ),
            (
                "DAMASCUS_STEEL_JETBOOTS",
                "DAMASCUS_STEEL_INGOT",
                "SMALL_CAPACITOR",
                75.0,
                0.55,
                "jetboots",
            ),
            (
                "REINFORCED_ALLOY_JETBOOTS",
                "REINFORCED_ALLOY_INGOT",
                "MEDIUM_CAPACITOR",
                100.0,
                0.6,
                "jetboots",
            ),
            (
                "CARBONADO_JETBOOTS",
                "CARBONADO",
                "LARGE_CAPACITOR",
                125.0,
                0.7,
                "carbonado_tools",
            ),
            (
                "ARMORED_JETBOOTS",
                "STEEL_PLATE",
                "MEDIUM_CAPACITOR",
                50.0,
                0.45,
                "armored_jetboots",
            ),
        ];
        assert_eq!(CRAFTING.len(), rows.len() + 1);
        for (recipe, (id, frame, capacitor, charge, speed, research)) in CRAFTING.iter().zip(rows) {
            let tuple = spec(id).unwrap();
            assert_eq!(
                (tuple.maximum, tuple.speed, tuple.research.id),
                (charge, speed, research)
            );
            assert_eq!((recipe.output.kind.id(), recipe.output.count), (id, 1));
            assert_eq!(
                recipe.inputs.map(|kind| kind.map_or("", Kind::id)),
                [
                    "",
                    "",
                    "",
                    frame,
                    "POWER_CRYSTAL",
                    frame,
                    "STEEL_THRUSTER",
                    capacitor,
                    "STEEL_THRUSTER",
                ]
            );
        }
        let thruster = CRAFTING.last().unwrap();
        assert_eq!(thruster.research.cost, 14);
        assert_eq!(
            thruster.inputs.map(|kind| kind.map_or("", Kind::id)),
            [
                "",
                "minecraft:redstone",
                "",
                "ALUMINUM_BRONZE_INGOT",
                "ALUMINUM_BRONZE_INGOT",
                "ALUMINUM_BRONZE_INGOT",
                "STEEL_PLATE",
                "minecraft:fire_charge",
                "STEEL_PLATE"
            ]
        );
    }

    #[test]
    fn engine_thrust_uses_the_horizontal_eye_vector_and_paper_offset() {
        assert_eq!(PERIOD, 2);
        assert_eq!(thrust(0.35, [1.0, 999.0, 0.0], true), [0.0, 0.04, 0.35]);
        assert_eq!(thrust(0.35, [1.0, -999.0, 0.0], false), [0.7, 0.04, -0.35]);
        assert_eq!(thrust(0.7, [0.0, 1.0, -1.0], true), [0.0, 0.04, -0.7]);
        let armored = thrust(0.45, [0.0, 1.0, 0.0], false);
        assert_eq!(armored, [0.25, 0.04, -0.25]);
    }

    #[test]
    fn new_boots_use_the_same_paper_charge_and_legacy_lore_as_the_bench() {
        let mut host = FakeHost::new();
        host.registries
            .resolve_items(
                ["minecraft:leather_boots", "minecraft:iron_boots"]
                    .map(|id| Identifier::parse(id).unwrap()),
            )
            .unwrap();
        for (spec, expected) in [
            (&SPECS[0], Some(0xc0c0c0)),
            (&SPECS[6], Some(0)),
            (&SPECS[7], None),
        ] {
            let empty = spec.item.stack(&host, 1).unwrap();
            let color = empty
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:dyed_color".into());
            assert_eq!(color.cloned(), expected.map(Tag::Int));
            assert!(charging_bench::discharge(&host, &empty, COST).is_none());
            let charged = charging_bench::recharge(&host, &empty, 1.0).unwrap();
            let moved = charging_bench::discharge(&host, &charged, COST).unwrap();
            let values = moved
                .components()
                .unwrap()
                .as_nbt()
                .get(&"minecraft:custom_data".into())
                .unwrap()
                .get("PublicBukkitValues")
                .unwrap();
            assert_eq!(
                values.get("slimefun:item_charge"),
                Some(&Tag::Float(0.93_f32.to_bits()))
            );
            assert_eq!(boots(&host, &moved).unwrap().item, spec.item);
        }
    }
}
